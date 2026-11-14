//! Headless local maintenance for this fork; never emits credentials.
use crate::modules::{
    qoder_account, qoder_cn_account, qoder_cn_oauth, qoder_native, qoder_oauth,
    qoder_region::QoderRegion,
};
use serde_json::{json, Value};

fn status() -> Result<Value, String> {
    let summarize = |accounts: Vec<crate::models::qoder::QoderAccount>| {
        accounts.into_iter().map(|a| json!({"id":a.id,"region":QoderRegion::from_info(a.auth_user_info_raw.as_ref()).ok(),"hasQuota":a.auth_credit_usage_raw.is_some(),"quotaError":a.quota_query_last_error})).collect::<Vec<_>>()
    };
    Ok(
        json!({"version":env!("CARGO_PKG_VERSION"),"qoder":summarize(qoder_account::list_accounts_checked()?),"qoder_cn":summarize(qoder_cn_account::list_accounts_checked()?)}),
    )
}

/// Copy and verify before removing a legacy CN entry from the global index.
fn migrate_cn() -> Result<Value, String> {
    let mut migrated = Vec::new();
    for account in qoder_account::list_accounts_checked()? {
        if QoderRegion::from_info(account.auth_user_info_raw.as_ref())? != QoderRegion::Cn {
            continue;
        }
        let imported = qoder_cn_account::import_from_json(
            &serde_json::to_string(&account).map_err(|e| e.to_string())?,
        )?;
        let saved = imported
            .first()
            .and_then(|a| qoder_cn_account::load_account(&a.id))
            .ok_or("CN migration readback missing")?;
        if saved.user_id != account.user_id
            || saved.auth_user_info_raw != account.auth_user_info_raw
        {
            return Err("CN migration readback mismatch; source preserved".into());
        }
        qoder_account::remove_account(&account.id)?;
        migrated.push(saved.id);
    }
    Ok(json!({"migrated":migrated.len(),"ids":migrated}))
}

fn storage_self_test() -> Result<Value, String> {
    // This process handles only this CLI request; no GUI/background worker is running.
    let sandbox = tempfile::tempdir().map_err(|e| e.to_string())?;
    let previous = std::env::var_os("COCKPIT_TOOLS_TEST_DATA_DIR");
    std::env::set_var("COCKPIT_TOOLS_TEST_DATA_DIR", sandbox.path());
    let result = (|| {
        let fixture = |region: QoderRegion| {
            json!({"userInfo":{"id":"same-fixture-user","email":"fixture@example.invalid","token":"fixture-access","refreshToken":"fixture-refresh","expireTime":"1893456000000","login_source":region.source()}}).to_string()
        };
        let global = qoder_account::import_from_json(&fixture(QoderRegion::Global))?;
        let cn = qoder_cn_account::import_from_json(&fixture(QoderRegion::Cn))?;
        if qoder_account::list_accounts_checked()?.len() != 1
            || qoder_cn_account::list_accounts_checked()?.len() != 1
        {
            return Err("Independent platform storage failed".into());
        }
        if qoder_account::import_from_json(&fixture(QoderRegion::Cn)).is_ok()
            || qoder_cn_account::import_from_json(&fixture(QoderRegion::Global)).is_ok()
        {
            return Err("Cross-platform credential accepted".into());
        }
        qoder_cn_account::remove_account(&cn[0].id)?;
        if qoder_account::load_account(&global[0].id).is_none()
            || !qoder_cn_account::list_accounts_checked()?.is_empty()
        {
            return Err("Cross-platform delete isolation failed".into());
        }
        let bytes = std::fs::read(
            sandbox
                .path()
                .join("qoder_accounts")
                .join(format!("{}.json", global[0].id)),
        )
        .map_err(|e| e.to_string())?;
        if bytes
            .windows(b"fixture-access".len())
            .any(|part| part == b"fixture-access")
        {
            return Err("Credential stored unencrypted".into());
        }
        Ok(
            json!({"independentStores":true,"crossPlatformImportRejected":true,"deleteIsolated":true,"encryptedAtRest":true}),
        )
    })();
    match previous {
        Some(value) => std::env::set_var("COCKPIT_TOOLS_TEST_DATA_DIR", value),
        None => std::env::remove_var("COCKPIT_TOOLS_TEST_DATA_DIR"),
    }
    result
}

fn execute(args: &[String]) -> Result<Value, String> {
    match args.first().map(String::as_str) {
        Some("status") => status(),
        Some("migrate-cn") => migrate_cn(),
        Some("import-native") => {
            let region=QoderRegion::parse(args.get(1).map(String::as_str))?;
            let client=qoder_native::NativeClient::parse(args.get(2).ok_or("Expected desktop or cli")?)?;
            let account=qoder_native::import(region,client)?;
            Ok(json!({"imported":1,"id":account.id,"region":region}))
        },
        Some("refresh") => {
            let region=QoderRegion::parse(args.get(1).map(String::as_str))?;
            let id=args.get(2).ok_or("Expected account id")?;
            let account=tauri::async_runtime::block_on(async {match region {QoderRegion::Global=>qoder_oauth::refresh_account_from_openapi(id).await,QoderRegion::Cn=>qoder_cn_oauth::refresh_account_from_openapi(id).await}})?;
            if let Some(error)=account.quota_query_last_error{return Err(error);}
            Ok(json!({"refreshed":true,"id":account.id,"region":region,"hasQuota":account.auth_credit_usage_raw.is_some()}))
        },
        Some("self-test") => Ok(json!({"native":qoder_native::self_test()?,"platforms":storage_self_test()?})),
        _ => Err("Usage: --fork-cli status | migrate-cn | import-native <cn|global> <desktop|cli> | refresh <cn|global> <account-id> | self-test".into())
    }
}
pub fn try_run() -> Option<i32> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.first().map(String::as_str) != Some("--fork-cli") {
        return None;
    }
    let (output, code) = match execute(&args[1..]) {
        Ok(result) => (json!({"ok":true,"result":result}), 0),
        Err(error) => (json!({"ok":false,"error":error}), 1),
    };
    println!("{}", output);
    Some(code)
}
