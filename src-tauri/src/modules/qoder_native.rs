//! Native Qoder desktop/CLI adapters. Credential schemas and CLI encryption are
//! adapted from workbuddy-switch-next (MIT); realm selection is always explicit.
use crate::{
    models::qoder::QoderAccount,
    modules::{atomic_write, qoder_account, qoder_native_profile, qoder_region::QoderRegion},
};
use aes::cipher::{block_padding::Pkcs7, BlockDecryptMut, BlockEncryptMut, KeyIvInit};
use aes_gcm::{aead::Aead, Aes256Gcm, KeyInit, Nonce};
use base64::{engine::general_purpose::STANDARD, Engine};
use fs2::FileExt;
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
};

static SWITCH_LOCK: Mutex<()> = Mutex::new(());

/// 每次切号都会在 `qoder-switch-backups` 下新建一份 UUID 目录并写入旧凭据，
/// 只保留最近若干份，避免旧凭据密文无上限堆积。
const SWITCH_BACKUP_KEEP: usize = 5;

fn prune_switch_backups(root: &Path, keep: usize) -> usize {
    let Ok(entries) = fs::read_dir(root) else {
        return 0;
    };
    let mut backups: Vec<(std::time::SystemTime, PathBuf)> = entries
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            let path = entry.path();
            if !path.is_dir() {
                return None;
            }
            let modified = entry.metadata().ok()?.modified().ok()?;
            Some((modified, path))
        })
        .collect();
    if backups.len() <= keep {
        return 0;
    }
    backups.sort_by(|a, b| b.0.cmp(&a.0));
    let mut removed = 0usize;
    for (_, path) in backups.into_iter().skip(keep) {
        match fs::remove_dir_all(&path) {
            Ok(()) => removed += 1,
            Err(error) => crate::modules::logger::log_warn(&format!(
                "[Qoder Switch] prune backup failed: {} ({error})",
                path.display()
            )),
        }
    }
    removed
}

pub fn self_test() -> Result<Value, String> {
    let sandbox = tempfile::tempdir().map_err(io_error)?;
    let profile = serde_json::json!({"uid":"fixture-user","access_token":"fixture-access","refresh_token":"fixture-refresh"});
    fs::write(
        sandbox.path().join("machine_id"),
        "1234567890abcdef-fixture",
    )
    .map_err(io_error)?;
    let encrypted = encode(sandbox.path(), NativeClient::Cli, &profile)?;
    if encrypted
        .windows(b"fixture-access".len())
        .any(|bytes| bytes == b"fixture-access")
    {
        return Err("Plaintext credential in CLI output".into());
    }
    let decoded = decode(sandbox.path(), NativeClient::Cli, &encrypted)?;
    if decoded != profile {
        return Err("CLI encryption roundtrip failed".into());
    }
    // Invalid input must fail before any on-disk credential can be replaced.
    if decode(sandbox.path(), NativeClient::Cli, b"invalid ciphertext").is_ok() {
        return Err("Malformed ciphertext accepted".into());
    }
    Ok(
        serde_json::json!({"cliEncryptionRoundtrip":true,"malformedCiphertextRejected":true,"liveClientUntouched":true}),
    )
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum NativeClient {
    Desktop,
    Cli,
}
pub(crate) const CN_DESKTOP_PROCESS_NAMES: &[&str] =
    &["qoder cn.exe", "qoder cn", "qoder launcher.exe", "qoderlauncher.exe", "qoderlauncher"];

/// CN 与国际版共用 `qoder launcher` / `qoderlauncher` 这组进程名，
/// 只按名字匹配会误杀国际版客户端（Windows 上是 `taskkill /T /F`，连子进程一起杀），
/// 也会让常驻的国际版 launcher 把 CN 切号挡在 `ensure_idle`。
///
/// 判定顺序：
/// 1. 名字必须在 CN 名单内；
/// 2. 能拿到 exe 路径且已知 CN 安装目录时，要求 exe 位于该目录下；
/// 3. 拿不到路径时，只接受 CN 独有、不可能属于国际版的名字。
pub(crate) fn is_qoder_cn_desktop_process(
    name_lower: &str,
    exe: Option<&Path>,
    cn_install_dir: Option<&Path>,
) -> bool {
    if !CN_DESKTOP_PROCESS_NAMES.contains(&name_lower) {
        return false;
    }
    let unambiguous = name_lower == "qoder cn.exe" || name_lower == "qoder cn";
    if let (Some(exe), Some(dir)) = (exe, cn_install_dir) {
        if exe.as_os_str().is_empty() {
            return unambiguous;
        }
        return path_is_within(exe, dir);
    }
    unambiguous
}

fn normalize_path_key(path: &Path) -> String {
    path.to_string_lossy()
        .replace('/', "\\")
        .to_ascii_lowercase()
}

fn path_is_within(path: &Path, dir: &Path) -> bool {
    let target = normalize_path_key(path);
    let mut root = normalize_path_key(dir);
    if root.is_empty() || target.is_empty() {
        return false;
    }
    if !root.ends_with('\\') {
        root.push('\\');
    }
    target.starts_with(&root)
}

fn qoder_cn_install_dir() -> Option<PathBuf> {
    crate::modules::process::resolve_qoder_cn_launch_path()
        .ok()
        .and_then(|path| path.parent().map(Path::to_path_buf))
}

pub(crate) fn qoder_cn_desktop_pids(system: &sysinfo::System) -> Vec<u32> {
    let cn_dir = qoder_cn_install_dir();
    let mut pids: Vec<u32> = system
        .processes()
        .values()
        .filter(|process| {
            let name = process.name().to_string_lossy().to_ascii_lowercase();
            is_qoder_cn_desktop_process(&name, process.exe(), cn_dir.as_deref())
        })
        .map(|process| process.pid().as_u32())
        .collect();
    pids.sort_unstable();
    pids.dedup();
    pids
}
impl NativeClient {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "desktop" => Ok(Self::Desktop),
            "cli" => Ok(Self::Cli),
            _ => Err("QODER_CLIENT_INVALID".into()),
        }
    }
    fn root(self, region: QoderRegion) -> Result<PathBuf, String> {
        match self {
            Self::Desktop => Ok(dirs::config_dir()
                .ok_or("QODER_DATA_ROOT_UNAVAILABLE")?
                .join(region.desktop_dir())),
            Self::Cli => Ok(dirs::home_dir()
                .ok_or("QODER_DATA_ROOT_UNAVAILABLE")?
                .join(region.cli_dir())
                .join(".auth")),
        }
    }
    fn filename(self) -> &'static str {
        match self {
            Self::Desktop => "auth.v1.dat",
            Self::Cli => "user",
        }
    }
}
fn io_error(error: std::io::Error) -> String {
    format!("QODER_NATIVE_STORAGE: {error}")
}
#[cfg(windows)]
fn unprotect(bytes: &[u8]) -> Result<Vec<u8>, String> {
    use windows::Win32::{
        Foundation::{LocalFree, HLOCAL},
        Security::Cryptography::{CryptUnprotectData, CRYPT_INTEGER_BLOB},
    };
    let source = CRYPT_INTEGER_BLOB {
        cbData: u32::try_from(bytes.len()).map_err(|_| "QODER_NATIVE_KEY_INVALID")?,
        pbData: bytes.as_ptr() as *mut u8,
    };
    let mut result = CRYPT_INTEGER_BLOB::default();
    unsafe {
        CryptUnprotectData(&source, None, None, None, None, 1, &mut result)
            .map_err(|_| "QODER_NATIVE_DPAPI_FAILED")?;
        let plain = std::slice::from_raw_parts(result.pbData, result.cbData as usize).to_vec();
        let _ = LocalFree(HLOCAL(result.pbData as _));
        Ok(plain)
    }
}
fn desktop_key(root: &Path) -> Result<Vec<u8>, String> {
    #[cfg(windows)]
    {
        let state: Value =
            serde_json::from_slice(&fs::read(root.join("Local State")).map_err(io_error)?)
                .map_err(|_| "QODER_NATIVE_LOCAL_STATE_INVALID")?;
        let raw = STANDARD
            .decode(
                state
                    .pointer("/os_crypt/encrypted_key")
                    .and_then(Value::as_str)
                    .ok_or("QODER_NATIVE_KEY_MISSING")?,
            )
            .map_err(|_| "QODER_NATIVE_KEY_INVALID")?;
        if !raw.starts_with(b"DPAPI") {
            return Err("QODER_NATIVE_KEY_UNSUPPORTED".into());
        }
        let key = unprotect(&raw[5..])?;
        if key.len() != 32 {
            return Err("QODER_NATIVE_KEY_INVALID".into());
        }
        Ok(key)
    }
    #[cfg(not(windows))]
    {
        let _ = root;
        Err("Native desktop adapter currently supports Windows; use the IDE adapter on this platform".into())
    }
}
fn decode(root: &Path, client: NativeClient, bytes: &[u8]) -> Result<Value, String> {
    let plain = match client {
        NativeClient::Desktop => {
            if !bytes.starts_with(b"v10") || bytes.len() < 31 {
                return Err("QODER_NATIVE_ENVELOPE_UNSUPPORTED".into());
            }
            Aes256Gcm::new_from_slice(&desktop_key(root)?)
                .map_err(|_| "QODER_NATIVE_KEY_INVALID")?
                .decrypt(Nonce::from_slice(&bytes[3..15]), &bytes[15..])
                .map_err(|_| "QODER_NATIVE_DECRYPT_FAILED")?
        }
        NativeClient::Cli => {
            if bytes.first() == Some(&b'{') {
                bytes.to_vec()
            } else {
                let machine = fs::read_to_string(root.join("machine_id")).map_err(io_error)?;
                let key = machine
                    .trim()
                    .as_bytes()
                    .get(..16)
                    .ok_or("QODER_NATIVE_MACHINE_ID_INVALID")?;
                let mut encrypted = STANDARD
                    .decode(bytes)
                    .map_err(|_| "QODER_NATIVE_ENVELOPE_INVALID")?;
                cbc::Decryptor::<aes::Aes128>::new_from_slices(key, key)
                    .map_err(|_| "QODER_NATIVE_KEY_INVALID")?
                    .decrypt_padded_mut::<Pkcs7>(&mut encrypted)
                    .map_err(|_| "QODER_NATIVE_DECRYPT_FAILED")?
                    .to_vec()
            }
        }
    };
    serde_json::from_slice(&plain).map_err(|_| "QODER_NATIVE_JSON_INVALID".into())
}
fn encode(root: &Path, client: NativeClient, profile: &Value) -> Result<Vec<u8>, String> {
    let plain = serde_json::to_vec(profile).map_err(|_| "QODER_NATIVE_JSON_INVALID")?;
    let bytes = match client {
        NativeClient::Desktop => {
            use aes_gcm::aead::{rand_core::RngCore, OsRng};
            let mut nonce = [0u8; 12];
            OsRng.fill_bytes(&mut nonce);
            let cipher = Aes256Gcm::new_from_slice(&desktop_key(root)?)
                .map_err(|_| "QODER_NATIVE_KEY_INVALID")?
                .encrypt(Nonce::from_slice(&nonce), plain.as_slice())
                .map_err(|_| "QODER_NATIVE_ENCRYPT_FAILED")?;
            [b"v10".as_slice(), &nonce, &cipher].concat()
        }
        NativeClient::Cli => {
            let machine = fs::read_to_string(root.join("machine_id")).map_err(io_error)?;
            let key = machine
                .trim()
                .as_bytes()
                .get(..16)
                .ok_or("QODER_NATIVE_MACHINE_ID_INVALID")?;
            let mut buffer = plain.clone();
            buffer.resize(plain.len() + 16, 0);
            let encrypted = cbc::Encryptor::<aes::Aes128>::new_from_slices(key, key)
                .map_err(|_| "QODER_NATIVE_KEY_INVALID")?
                .encrypt_padded_mut::<Pkcs7>(&mut buffer, plain.len())
                .map_err(|_| "QODER_NATIVE_ENCRYPT_FAILED")?;
            STANDARD.encode(encrypted).into_bytes()
        }
    };
    if decode(root, client, &bytes)? != *profile {
        return Err("QODER_NATIVE_ROUNDTRIP_FAILED".into());
    }
    Ok(bytes)
}
pub fn import(region: QoderRegion, client: NativeClient) -> Result<QoderAccount, String> {
    // 与 switch() 共用同一把进程内锁：避免导入时读到切号过程中的中间态，
    // 把刚刚被切走、用户已不再使用的账号重新写回账号库。
    let _guard = SWITCH_LOCK.lock().map_err(|_| "QODER_ACCOUNT_BUSY")?;
    let root = client.root(region)?;
    let profile = decode(
        &root,
        client,
        &fs::read(root.join(client.filename())).map_err(io_error)?,
    )?;
    let info = qoder_native_profile::from_client(&profile, region)?;
    match region {
        QoderRegion::Global => qoder_account::upsert_account_from_snapshot(info, None, None),
        QoderRegion::Cn => {
            crate::modules::qoder_cn_account::upsert_account_from_snapshot(info, None, None)
        }
    }
}

fn local_profile(region: QoderRegion, client: NativeClient) -> Option<Value> {
    let root = client.root(region).ok()?;
    let bytes = fs::read(root.join(client.filename())).ok()?;
    let profile = decode(&root, client, &bytes).ok()?;
    qoder_native_profile::from_client(&profile, region).ok()
}

pub fn local_identity(region: QoderRegion) -> Option<String> {
    for client in [NativeClient::Desktop, NativeClient::Cli] {
        let Some(info) = local_profile(region, client) else {
            continue;
        };
        let user = info
            .get("id")
            .and_then(Value::as_str)
            .or_else(|| info.get("email").and_then(Value::as_str))?;
        return Some(serde_json::json!([region.source(), user]).to_string());
    }
    None
}

pub fn import_from_local(region: QoderRegion) -> Result<bool, String> {
    let mut found = false;
    let mut last_error = None;
    let mut imported = false;
    for client in [NativeClient::Desktop, NativeClient::Cli] {
        let path = client.root(region)?.join(client.filename());
        if !path.exists() {
            continue;
        }
        found = true;
        match import(region, client) {
            Ok(_) => imported = true,
            Err(error) => last_error = Some(error),
        }
    }
    if !found {
        return Ok(false);
    }
    if imported {
        Ok(true)
    } else {
        Err(last_error.unwrap_or_else(|| "QODER_NATIVE_IMPORT_FAILED".into()))
    }
}
fn ensure_idle(region: QoderRegion, client: NativeClient) -> Result<(), String> {
    let system = sysinfo::System::new_all();
    if system.processes().is_empty() {
        return Err("QODER_NATIVE_PROCESS_PROBE_FAILED".into());
    }
    if region == QoderRegion::Cn && client == NativeClient::Desktop {
        // 仅按名字匹配会把国际版的同名 launcher 也算进来，CN 切号会被误挡。
        if !qoder_cn_desktop_pids(&system).is_empty() {
            return Err(
                "请先关闭对应 Qoder 客户端或结束 CLI 任务，再切换账号；当前会话未被中断。".into(),
            );
        }
        return Ok(());
    }
    let names = match (region, client) {
        (QoderRegion::Cn, NativeClient::Desktop) => CN_DESKTOP_PROCESS_NAMES,
        (QoderRegion::Global, NativeClient::Desktop) => {
            &["qoder.exe", "qoder", "qoder launcher.exe", "qoderlauncher.exe", "qoderlauncher"][..]
        }
        (QoderRegion::Cn, NativeClient::Cli) => &["qoderclicn.exe", "qoderclicn"][..],
        (QoderRegion::Global, NativeClient::Cli) => &["qodercli.exe", "qodercli"][..],
    };
    if system
        .processes()
        .values()
        .any(|p| names.contains(&p.name().to_string_lossy().to_ascii_lowercase().as_str()))
    {
        return Err(
            "请先关闭对应 Qoder 客户端或结束 CLI 任务，再切换账号；当前会话未被中断。".into(),
        );
    }
    Ok(())
}
pub fn switch(region: QoderRegion, account_id: &str, client: NativeClient) -> Result<(), String> {
    let _guard = SWITCH_LOCK.lock().map_err(|_| "QODER_ACCOUNT_BUSY")?;
    let account = match region {
        QoderRegion::Global => qoder_account::load_account(account_id),
        QoderRegion::Cn => crate::modules::qoder_cn_account::load_account(account_id),
    }
    .ok_or("QODER_SESSION_CHANGED")?;
    let info = account
        .auth_user_info_raw
        .as_ref()
        .ok_or("QODER_NATIVE_TOKEN_MISSING")?;
    if QoderRegion::from_account(&account)? != region {
        return Err("QODER_PLATFORM_MISMATCH".into());
    }
    ensure_idle(region, client)?;
    let profile = qoder_native_profile::for_client(info, client == NativeClient::Desktop)?;
    let root = client.root(region)?;
    // Require an initialized client: do not synthesize machine IDs or OS keys.
    if !root.is_dir() {
        return Err("请先启动一次目标 Qoder 客户端，再切换账号。".into());
    }
    let lock = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(root.join("cockpit-account-switch.lock"))
        .map_err(io_error)?;
    lock.try_lock_exclusive()
        .map_err(|_| "QODER_ACCOUNT_BUSY")?;
    let path = root.join(client.filename());
    let old = match fs::read(&path) {
        Ok(bytes) => Some(bytes),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(io_error(e)),
    };
    let next = encode(&root, client, &profile)?;
    let backup_root = crate::modules::account::get_data_dir()?.join("qoder-switch-backups");
    prune_switch_backups(&backup_root, SWITCH_BACKUP_KEEP.saturating_sub(1));
    let backup = backup_root.join(uuid::Uuid::new_v4().to_string());
    fs::create_dir_all(&backup).map_err(io_error)?;
    if let Some(old) = &old {
        atomic_write::write_bytes_atomic(&backup.join(client.filename()), old)?;
    }
    // Clear only account-specific overlays. Save the whole set before any write.
    let overlays = ["auth-profile-overlays.v1.dat", "channel-activation.v1.json"];
    let mut paired = Vec::new();
    if client == NativeClient::Desktop {
        for name in overlays {
            let target = root.join(name);
            match fs::read(&target) {
                Ok(bytes) => {
                    atomic_write::write_bytes_atomic(&backup.join(name), &bytes)?;
                    paired.push((target, bytes));
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(io_error(e)),
            }
        }
    }
    let mut write_started = false;
    let result = (|| -> Result<(), String> {
        ensure_idle(region, client)?;
        if fs::read(&path).ok() != old {
            return Err("QODER_SESSION_CHANGED".into());
        }
        atomic_write::write_bytes_atomic(&path, &next)?;
        // 写入落盘后才进入回滚保护。write_bytes_atomic 自身失败时原文件仍然完好，
        // 若此时先行置位，回滚分支会去删除/重写一个未被修改的文件并误报 rollback incomplete。
        write_started = true;
        for (target, _) in &paired {
            atomic_write::remove_file_locked(target)?;
        }
        if decode(&root, client, &fs::read(&path).map_err(io_error)?)? != profile {
            return Err("QODER_NATIVE_READBACK_FAILED".into());
        }
        Ok(())
    })();
    if let Err(error) = result {
        if !write_started {
            return Err(error);
        }
        let restore = match &old {
            Some(bytes) => atomic_write::write_bytes_atomic(&path, bytes),
            None => fs::remove_file(&path).map_err(io_error),
        };
        let mut rollback_failed = restore.is_err();
        for (target, bytes) in &paired {
            rollback_failed |= atomic_write::write_bytes_atomic(target, bytes).is_err();
        }
        if rollback_failed {
            return Err(format!(
                "{error}; rollback incomplete; encrypted backup: {}",
                backup.display()
            ));
        }
        return Err(error);
    }
    match region {
        QoderRegion::Global => qoder_account::touch_native_switch(account_id)?,
        QoderRegion::Cn => crate::modules::qoder_cn_account::touch_native_switch(account_id)?,
    };
    Ok(())
}
