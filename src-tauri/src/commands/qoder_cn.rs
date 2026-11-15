use std::time::Instant;
use tauri::{AppHandle, Emitter};

use crate::models::qoder::{QoderAccount, QoderOAuthStartResponse};
use crate::modules::{logger, qoder_cn_account, qoder_cn_oauth};

#[tauri::command]
pub fn list_qoder_cn_accounts() -> Result<Vec<QoderAccount>, String> {
    qoder_cn_account::list_accounts_checked()
}

#[tauri::command]
pub async fn import_qoder_cn_native(
    region: Option<String>,
    client: String,
) -> Result<Vec<QoderAccount>, String> {
    let region =
        crate::modules::qoder_region::QoderRegion::parse(Some(region.as_deref().unwrap_or("cn")))?;
    if region != crate::modules::qoder_region::QoderRegion::Cn {
        return Err("QODER_PLATFORM_MISMATCH".into());
    }
    let client = crate::modules::qoder_native::NativeClient::parse(&client)?;
    tauri::async_runtime::spawn_blocking(move || {
        crate::modules::qoder_native::import(region, client).map(|account| vec![account])
    })
    .await
    .map_err(|_| "QODER_NATIVE_WORKER_FAILED".to_string())?
}

#[tauri::command]
pub async fn inject_qoder_cn_native(account_id: String, client: String) -> Result<String, String> {
    let target = crate::modules::qoder_native::NativeClient::parse(&client)?;
    qoder_cn_oauth::prepare_account_for_switch(&account_id).await?;
    let id = account_id.clone();
    tauri::async_runtime::spawn_blocking(move || {
        crate::modules::qoder_native::switch(
            crate::modules::qoder_region::QoderRegion::Cn,
            &id,
            target,
        )
    })
    .await
    .map_err(|_| "QODER_NATIVE_WORKER_FAILED".to_string())??;
    Ok(account_id)
}

#[tauri::command]
pub fn delete_qoder_cn_account(account_id: String) -> Result<(), String> {
    qoder_cn_account::remove_account(&account_id)
}

#[tauri::command]
pub fn delete_qoder_cn_accounts(account_ids: Vec<String>) -> Result<(), String> {
    qoder_cn_account::remove_accounts(&account_ids)
}

#[tauri::command]
pub fn import_qoder_cn_from_json(
    json_content: String,
    region: Option<String>,
) -> Result<Vec<QoderAccount>, String> {
    if let Some(value) = region.as_deref() {
        if crate::modules::qoder_region::QoderRegion::parse(Some(value))?
            != crate::modules::qoder_region::QoderRegion::Cn
        {
            return Err("QODER_PLATFORM_MISMATCH".into());
        }
    }
    let mut value = serde_json::from_str(&json_content).map_err(|e| format!("Qoder JSON: {e}"))?;
    crate::modules::qoder_native_profile::prepare_import(
        &mut value,
        crate::modules::qoder_region::QoderRegion::Cn,
    )?;
    qoder_cn_account::import_from_json(&serde_json::to_string(&value).map_err(|e| e.to_string())?)
}

#[tauri::command]
pub async fn import_qoder_cn_from_local() -> Result<Vec<QoderAccount>, String> {
    tauri::async_runtime::spawn_blocking(|| {
        crate::modules::qoder_native::import(
            crate::modules::qoder_region::QoderRegion::Cn,
            crate::modules::qoder_native::NativeClient::Desktop,
        )
        .map(|a| vec![a])
    })
    .await
    .map_err(|_| "QODER_NATIVE_WORKER_FAILED".to_string())?
}

#[tauri::command]
pub async fn qoder_cn_oauth_login_start(
    region: Option<String>,
) -> Result<QoderOAuthStartResponse, String> {
    let started_at = Instant::now();
    logger::log_info("[Qoder OAuth] start 命令触发");
    let region =
        crate::modules::qoder_region::QoderRegion::parse(Some(region.as_deref().unwrap_or("cn")))?;
    if region != crate::modules::qoder_region::QoderRegion::Cn {
        return Err("QODER_PLATFORM_MISMATCH".into());
    }
    let result = qoder_cn_oauth::start_login_with_region(region).await;
    match &result {
        Ok(response) => logger::log_info(&format!(
            "[Qoder OAuth] start 命令完成: login_id={}, verification_uri_len={}, callback_url={}, elapsed={}ms",
            response.login_id,
            response.verification_uri.len(),
            response.callback_url.as_deref().unwrap_or("<none>"),
            started_at.elapsed().as_millis()
        )),
        Err(err) => logger::log_warn(&format!(
            "[Qoder OAuth] start 命令失败: elapsed={}ms, error={}",
            started_at.elapsed().as_millis(),
            err
        )),
    }
    result
}

#[tauri::command]
pub fn qoder_cn_oauth_login_peek() -> Option<QoderOAuthStartResponse> {
    let pending = qoder_cn_oauth::peek_pending_login();
    if let Some(state) = pending.as_ref() {
        logger::log_info(&format!(
            "[Qoder OAuth] peek 命令命中会话: login_id={}, verification_uri_len={}",
            state.login_id,
            state.verification_uri.len()
        ));
    } else {
        logger::log_info("[Qoder OAuth] peek 命令未命中会话");
    }
    pending
}

#[tauri::command]
pub async fn qoder_cn_oauth_login_complete(
    app: AppHandle,
    login_id: String,
) -> Result<QoderAccount, String> {
    let started_at = Instant::now();
    logger::log_info(&format!(
        "[Qoder OAuth] complete 命令触发: login_id={}",
        login_id
    ));
    let account = match qoder_cn_oauth::complete_login(&login_id).await {
        Ok(account) => account,
        Err(err) => {
            logger::log_warn(&format!(
                "[Qoder OAuth] complete 命令失败: login_id={}, elapsed={}ms, error={}",
                login_id,
                started_at.elapsed().as_millis(),
                err
            ));
            return Err(err);
        }
    };
    let _ = crate::modules::tray::update_tray_menu(&app);
    logger::log_info(&format!(
        "[Qoder OAuth] complete 命令完成: login_id={}, account_id={}, elapsed={}ms",
        login_id,
        account.id,
        started_at.elapsed().as_millis()
    ));
    Ok(account)
}

#[tauri::command]
pub fn qoder_cn_oauth_login_cancel(login_id: Option<String>) -> Result<(), String> {
    logger::log_info(&format!(
        "[Qoder OAuth] cancel 命令触发: login_id={}",
        login_id.as_deref().unwrap_or("<none>")
    ));
    qoder_cn_oauth::cancel_login(login_id.as_deref())
}

#[tauri::command]
pub fn export_qoder_cn_accounts(account_ids: Vec<String>) -> Result<String, String> {
    qoder_cn_account::export_accounts(&account_ids)
}

#[tauri::command]
pub async fn refresh_qoder_cn_token(
    app: AppHandle,
    account_id: String,
) -> Result<QoderAccount, String> {
    let started_at = Instant::now();
    logger::log_info(&format!(
        "[Qoder Command] 手动刷新账号开始: account_id={}",
        account_id
    ));
    match qoder_cn_oauth::refresh_account_from_openapi(&account_id).await {
        Ok(account) => {
            let _ = crate::modules::tray::update_tray_menu(&app);
            logger::log_info(&format!(
                "[Qoder Command] 刷新完成: account_id={}, email={}, elapsed={}ms",
                account.id,
                account.email,
                started_at.elapsed().as_millis()
            ));
            if let Some(error) = &account.quota_query_last_error {
                return Err(error.clone());
            }
            Ok(account)
        }
        Err(err) => {
            logger::log_warn(&format!(
                "[Qoder Command] 刷新失败: account_id={}, elapsed={}ms, error={}",
                account_id,
                started_at.elapsed().as_millis(),
                err
            ));
            Err(err)
        }
    }
}

#[tauri::command]
pub async fn refresh_all_qoder_cn_tokens(app: AppHandle) -> Result<i32, String> {
    let started_at = Instant::now();
    logger::log_info("[Qoder Command] 批量刷新开始");
    let refreshed = qoder_cn_oauth::refresh_all_accounts_from_openapi().await?;
    let _ = crate::modules::tray::update_tray_menu(&app);
    logger::log_info(&format!(
        "[Qoder Command] 批量刷新完成: refreshed={}, elapsed={}ms",
        refreshed,
        started_at.elapsed().as_millis()
    ));
    Ok(refreshed)
}

#[tauri::command]
pub async fn get_qoder_cn_checkin_status(
    account_id: String,
) -> Result<qoder_cn_oauth::QoderCheckinStatus, String> {
    qoder_cn_oauth::get_qoder_checkin_status(&account_id).await
}

#[tauri::command]
pub async fn claim_qoder_cn_checkin(
    account_id: String,
) -> Result<qoder_cn_oauth::QoderCheckinStatus, String> {
    qoder_cn_oauth::claim_qoder_checkin(&account_id).await
}

#[tauri::command]
pub async fn inject_qoder_cn_account(account_id: String) -> Result<String, String> {
    qoder_cn_oauth::prepare_account_for_switch(&account_id).await?;
    let id = account_id.clone();
    tauri::async_runtime::spawn_blocking(move || {
        crate::modules::qoder_native::switch(
            crate::modules::qoder_region::QoderRegion::Cn,
            &id,
            crate::modules::qoder_native::NativeClient::Desktop,
        )
    })
    .await
    .map_err(|_| "QODER_NATIVE_WORKER_FAILED".to_string())??;
    Ok(account_id)
}

#[tauri::command]
pub fn update_qoder_cn_account_tags(
    account_id: String,
    tags: Vec<String>,
) -> Result<QoderAccount, String> {
    qoder_cn_account::update_account_tags(&account_id, tags)
}

#[tauri::command]
pub fn get_qoder_cn_accounts_index_path() -> Result<String, String> {
    qoder_cn_account::accounts_index_path_string()
}
