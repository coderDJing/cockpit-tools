//! Headless local maintenance for this fork; never emits credentials.
//!
//! `cockpit-tools.exe --fork-cli <command>` runs exactly one command and exits
//! without starting the GUI. Every command prints a single JSON line and exits
//! with 0 (ok) or 1 (error), so a script can chain it.
//!
//! The realm split lives in the stores, not here: the global index and the
//! public CN store own their own accounts and locks, and this module only ever
//! reports what a store already contains or asks a store to write.
use crate::models::qoder::QoderAccount;
use crate::modules::qoder_channel::{credential_source, Channel, CredentialSource};
use crate::modules::qoder_cn_account::QoderCnAccountStore;
use crate::modules::qoder_native::{self, NativeClient, NativeRegion};
use crate::modules::{qoder_account, qoder_oauth};
use serde_json::{json, Value};

/// Marker the legacy CN IDE wrote. The CN store only accepts its own marker, so
/// a migrated session is promoted while it moves.
const LEGACY_CN_MARKER: &str = "qodercn";

fn region_name(region: NativeRegion) -> &'static str {
    match region {
        NativeRegion::Global => "global",
        NativeRegion::Cn => "cn",
    }
}

/// Realm label of a stored account, resolved from its recorded marker.
///
/// The IDE-bound sources share the global client's store and endpoints, so they
/// report `global`; only the public CN marker reports `cn`. An unreadable marker
/// stays `null` instead of being guessed.
fn region_of(account: &QoderAccount) -> Value {
    match credential_source(account) {
        Ok(CredentialSource::PublicCn) | Ok(CredentialSource::LegacyCnIde) => json!("cn"),
        Ok(CredentialSource::GlobalIde) | Ok(CredentialSource::DedicatedIde) => json!("global"),
        Ok(CredentialSource::Unsupported) | Err(_) => Value::Null,
    }
}

fn summarize(accounts: Vec<QoderAccount>) -> Value {
    Value::Array(
        accounts
            .iter()
            .map(|account| {
                json!({
                    "id": account.id,
                    "region": region_of(account),
                    "hasQuota": account.auth_credit_usage_raw.is_some(),
                    "quotaError": account.quota_query_last_error,
                })
            })
            .collect(),
    )
}

fn status() -> Result<Value, String> {
    Ok(json!({
        "version": env!("CARGO_PKG_VERSION"),
        "qoder": summarize(qoder_account::list_accounts_checked()?),
        "qoder_cn": summarize(QoderCnAccountStore::default_store()?.list_accounts()?),
    }))
}

/// Copy and verify before removing a legacy CN entry from the global index.
fn migrate_cn() -> Result<Value, String> {
    let store = QoderCnAccountStore::default_store()?;
    let public_marker = Channel::for_source(CredentialSource::PublicCn)
        .map(Channel::marker)
        .ok_or("QODER_REGION_INVALID")?;
    let mut migrated = Vec::new();
    for mut account in qoder_account::list_accounts_checked()? {
        // Only the legacy IDE session needs moving: the global store refuses a
        // public CN snapshot at write time, so nothing else can live there.
        if credential_source(&account)? != CredentialSource::LegacyCnIde {
            continue;
        }
        for payload in [
            account.auth_user_info_raw.as_mut(),
            account.auth_user_plan_raw.as_mut(),
        ]
        .into_iter()
        .flatten()
        {
            if payload.get("login_source").and_then(Value::as_str) == Some(LEGACY_CN_MARKER) {
                payload["login_source"] = Value::String(public_marker.to_owned());
            }
        }
        let imported = store.import_json(
            &serde_json::to_string(&account).map_err(|error| error.to_string())?,
        )?;
        let saved = imported
            .first()
            .map(|account| account.id.clone())
            .ok_or("CN migration produced no account")?;
        let saved = store
            .load_account(&saved)?
            .ok_or("CN migration readback missing")?;
        if saved.user_id != account.user_id
            || saved.auth_user_info_raw != account.auth_user_info_raw
        {
            return Err("CN migration readback mismatch; source preserved".into());
        }
        qoder_account::remove_account(&account.id)?;
        migrated.push(saved.id);
    }
    Ok(json!({"migrated": migrated.len(), "ids": migrated}))
}

/// Storage isolation check, run inside a throwaway data directory.
fn storage_self_test() -> Result<Value, String> {
    // This process handles only this CLI request; no GUI or background worker runs.
    let sandbox = tempfile::tempdir().map_err(|error| error.to_string())?;
    let previous = std::env::var_os("COCKPIT_TOOLS_DATA_DIR");
    std::env::set_var("COCKPIT_TOOLS_DATA_DIR", sandbox.path());
    let result = (|| {
        let fixture = |source: &str| {
            json!({"userInfo":{"id":"same-fixture-user","email":"fixture@example.invalid","token":"fixture-access","refreshToken":"fixture-refresh","expireTime":"1893456000000","login_source":source}}).to_string()
        };
        let store = QoderCnAccountStore::default_store()?;
        let global = qoder_account::import_from_json(&fixture("qoder"))?;
        let cn = store.import_json(&fixture("qoder_cn"))?;
        if qoder_account::list_accounts_checked()?.len() != 1 || store.list_accounts()?.len() != 1
        {
            return Err("Independent platform storage failed".into());
        }
        // Both stores refuse a credential stamped for the other realm: the CN
        // store at its marker stamp, the global store when it writes the file.
        if qoder_account::import_from_json(&fixture("qoder_cn")).is_ok()
            || store.import_json(&fixture("qoder")).is_ok()
        {
            return Err("Cross-platform credential accepted".into());
        }
        let mut payload: Value =
            serde_json::from_str(&fixture("qoder")).map_err(|error| error.to_string())?;
        if qoder_native::prepare_cn_import(&mut payload).is_ok() {
            return Err("CN import accepted a global credential".into());
        }
        if region_of(&cn[0]) != json!("cn") || region_of(&global[0]) != json!("global") {
            return Err("Stored marker does not name its realm".into());
        }
        store.remove_account(&cn[0].id)?;
        if qoder_account::load_account(&global[0].id).is_none()
            || !store.list_accounts()?.is_empty()
        {
            return Err("Cross-platform delete isolation failed".into());
        }
        let bytes = std::fs::read(
            sandbox
                .path()
                .join("qoder_accounts")
                .join(format!("{}.json", global[0].id)),
        )
        .map_err(|error| error.to_string())?;
        if bytes
            .windows(b"fixture-access".len())
            .any(|part| part == b"fixture-access")
        {
            return Err("Credential stored unencrypted".into());
        }
        Ok(
            json!({"independentStores":true,"crossPlatformImportRejected":true,"cnImportRejectsGlobal":true,"realmMarkers":true,"deleteIsolated":true,"encryptedAtRest":true}),
        )
    })();
    match previous {
        Some(value) => std::env::set_var("COCKPIT_TOOLS_DATA_DIR", value),
        None => std::env::remove_var("COCKPIT_TOOLS_DATA_DIR"),
    }
    result
}

/// Which native client this machine has signed in, per realm. Identity only; the
/// key is `[marker, user id]` and no token ever leaves the profile.
fn native_self_test() -> Result<Value, String> {
    Ok(json!({
        "global": qoder_native::local_identity_key(NativeRegion::Global),
        "cn": qoder_native::local_identity_key(NativeRegion::Cn),
    }))
}

fn import_native(args: &[String]) -> Result<Value, String> {
    let region = NativeRegion::parse(args.get(1).map(String::as_str))?;
    let client = NativeClient::parse(args.get(2).ok_or("Expected desktop or cli")?)?;
    let account = qoder_native::import(region, client)?;
    Ok(json!({"imported": 1, "id": account.id, "region": region_name(region)}))
}

fn refresh(args: &[String]) -> Result<Value, String> {
    let region = NativeRegion::parse(args.get(1).map(String::as_str))?;
    let id = args.get(2).ok_or("Expected account id")?;
    let store = QoderCnAccountStore::default_store()?;
    // The refresh itself resolves the store that owns the id, so the realm is
    // checked here: refreshing must never reach across stores.
    match region {
        NativeRegion::Global => {
            if qoder_account::load_account(id).is_none() {
                return Err("QODER_SESSION_CHANGED".into());
            }
        }
        NativeRegion::Cn => {
            if store.load_account(id)?.is_none() {
                return Err("QODER_SESSION_CHANGED".into());
            }
            // Both stores derive ids from the same user id, so a colliding id
            // would silently refresh the global account instead.
            if qoder_account::load_account(id).is_some() {
                return Err("QODER_ACCOUNT_ID_AMBIGUOUS".into());
            }
        }
    }
    let account = tauri::async_runtime::block_on(qoder_oauth::refresh_account_from_openapi(id))?;
    if let Some(error) = account.quota_query_last_error {
        return Err(error);
    }
    Ok(json!({
        "refreshed": true,
        "id": account.id,
        "region": region_name(region),
        "hasQuota": account.auth_credit_usage_raw.is_some(),
    }))
}

fn execute(args: &[String]) -> Result<Value, String> {
    match args.first().map(String::as_str) {
        Some("status") => status(),
        Some("migrate-cn") => migrate_cn(),
        Some("import-native") => import_native(args),
        Some("refresh") => refresh(args),
        Some("self-test") => Ok(json!({
            "native": native_self_test()?,
            "platforms": storage_self_test()?,
        })),
        _ => Err("Usage: --fork-cli status | migrate-cn | import-native <cn|global> <desktop|cli> | refresh <cn|global> <account-id> | self-test".into()),
    }
}

pub fn try_run() -> Option<i32> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.first().map(String::as_str) != Some("--fork-cli") {
        return None;
    }
    // The GUI binary is built without a console, so a terminal launch only sees
    // this output after attaching to the parent console.
    #[cfg(target_os = "windows")]
    unsafe {
        use windows::Win32::System::Console::{AttachConsole, ATTACH_PARENT_PROCESS};
        let _ = AttachConsole(ATTACH_PARENT_PROCESS);
    }
    let (output, code) = match execute(&args[1..]) {
        Ok(result) => (json!({"ok": true, "result": result}), 0),
        Err(error) => (json!({"ok": false, "error": error}), 1),
    };
    println!("{output}");
    Some(code)
}
