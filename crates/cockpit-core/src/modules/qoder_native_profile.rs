//! Native credential schemas, adapted from coderDJing/workbuddy-switch-next
//! (MIT, copyright 2026 wb-switch). See resources/licenses/WorkBuddy-Switch-Next.txt.
use super::qoder_region::QoderRegion;
use serde_json::{json, Map, Value};

fn text(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        value
            .get(*key)
            .and_then(|v| {
                v.as_str()
                    .map(str::to_owned)
                    .or_else(|| v.as_i64().map(|n| n.to_string()))
            })
            .filter(|v| !v.trim().is_empty())
    })
}
pub fn millis(value: Option<&Value>) -> Option<i64> {
    let value = value?;
    let n = value.as_i64().or_else(|| value.as_str()?.parse().ok());
    if let Some(n) = n {
        return Some(if n > 100_000_000_000 {
            n
        } else {
            n.checked_mul(1000)?
        });
    }
    chrono::DateTime::parse_from_rfc3339(value.as_str()?)
        .ok()
        .map(|t| t.timestamp_millis())
}
pub fn from_client(value: &Value, region: QoderRegion) -> Result<Value, String> {
    let token = text(
        value,
        &[
            "token",
            "accessToken",
            "access_token",
            "security_oauth_token",
        ],
    )
    .ok_or("QODER_NATIVE_TOKEN_MISSING")?;
    let mut user = value
        .get("user")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    if !user.contains_key("id") {
        if let Some(uid) = text(value, &["uid", "user_id", "id"]) {
            user.insert("id".into(), json!(uid));
        }
    }
    let uid = text(&Value::Object(user.clone()), &["id", "user_id"])
        .ok_or("QODER_NATIVE_IDENTITY_MISSING")?;
    user.insert("id".into(), json!(uid));
    for (from, to) in [
        ("name", "name"),
        ("avatar_url", "avatarUrl"),
        ("email", "email"),
        ("data_policy_agreed", "data_policy_agreed"),
        ("is_data_policy_modifiable", "is_data_policy_modifiable"),
        ("user_type", "user_type"),
        ("is_personal_version", "is_personal_version"),
    ] {
        if let Some(v) = value.get(from) {
            user.entry(to).or_insert_with(|| v.clone());
        }
    }
    let mut info = user.clone();
    info.insert("token".into(), json!(token));
    info.insert("login_source".into(), json!(region.source()));
    info.insert("cockpit_native_user".into(), Value::Object(user));
    if let Some(refresh) = text(value, &["refreshToken", "refresh_token"]) {
        info.insert("refreshToken".into(), json!(refresh));
    }
    for (keys, dest) in [
        (
            &["expiresAt", "expires_at", "expire_time", "expireTime"][..],
            "expireTime",
        ),
        (
            &[
                "refreshTokenExpiresAt",
                "refresh_expires_at",
                "refresh_token_expire_time",
                "refreshTokenExpireTime",
            ][..],
            "refreshTokenExpireTime",
        ),
    ] {
        if let Some(expiry) = keys.iter().find_map(|key| millis(value.get(*key))) {
            info.insert(dest.into(), json!(expiry.to_string()));
        }
    }
    Ok(Value::Object(info))
}

pub fn for_client(info: &Value, desktop: bool) -> Result<Value, String> {
    let uid = text(info, &["id", "uid", "user_id"]).ok_or("QODER_NATIVE_IDENTITY_MISSING")?;
    let token = text(info, &["token", "securityOauthToken"]).ok_or("QODER_NATIVE_TOKEN_MISSING")?;
    let refresh = text(info, &["refreshToken"]).ok_or("QODER_REAUTH_REQUIRED")?;
    let expiry = millis(info.get("expireTime")).ok_or("QODER_NATIVE_EXPIRY_MISSING")?;
    let expiry_iso = chrono::DateTime::from_timestamp_millis(expiry)
        .ok_or("QODER_NATIVE_EXPIRY_INVALID")?
        .to_rfc3339();
    let refresh_expiry = millis(info.get("refreshTokenExpireTime"));
    let mut user = info
        .get("cockpit_native_user")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    user.insert("id".into(), json!(uid));
    for key in [
        "name",
        "email",
        "avatarUrl",
        "data_policy_agreed",
        "is_data_policy_modifiable",
        "user_type",
        "is_personal_version",
    ] {
        if let Some(value) = info.get(key) {
            user.insert(key.into(), value.clone());
        }
    }
    if desktop {
        let mut profile = json!({"schemaVersion":1,"token":token,"refreshToken":refresh,"expiresAt":expiry_iso,"user":user});
        if let Some(ms) = refresh_expiry {
            profile["refreshTokenExpiresAt"] = json!(chrono::DateTime::from_timestamp_millis(ms)
                .ok_or("QODER_NATIVE_EXPIRY_INVALID")?
                .to_rfc3339());
        }
        Ok(profile)
    } else {
        // Do not inherit another account's encrypt_user_info, key, policy or identity.
        let mut profile = Map::new();
        profile.insert("uid".into(), json!(uid));
        profile.insert("security_oauth_token".into(), json!(token));
        profile.insert("access_token".into(), json!(token));
        profile.insert("refresh_token".into(), json!(refresh));
        profile.insert("expire_time".into(), json!(expiry / 1000));
        if let Some(ms) = refresh_expiry {
            profile.insert("refresh_token_expire_time".into(), json!(ms / 1000));
        }
        for (from, to) in [
            ("name", "name"),
            ("avatarUrl", "avatar_url"),
            ("data_policy_agreed", "data_policy_agreed"),
            ("is_data_policy_modifiable", "is_data_policy_modifiable"),
            ("user_type", "user_type"),
            ("is_personal_version", "is_personal_version"),
        ] {
            if let Some(v) = user.get(from) {
                profile.insert(to.into(), v.clone());
            }
        }
        profile.insert("login_method".into(), json!("browser"));
        profile.insert(
            "login_timestamp".into(),
            json!(chrono::Utc::now().timestamp()),
        );
        profile.insert("encrypt_user_info".into(), json!(""));
        profile.insert("key".into(), json!(""));
        Ok(Value::Object(profile))
    }
}

/// Validate the whole import before the first account is saved. The region
/// selected by the user may fill a legacy omission, never override a scope.
pub fn prepare_import(value: &mut Value, region: QoderRegion) -> Result<(), String> {
    if let Some(items) = value.as_array_mut() {
        for item in items {
            prepare_import(item, region)?;
        }
        return Ok(());
    }
    let obj = value.as_object_mut().ok_or("QODER_IMPORT_INVALID")?;
    if let Some(items) = obj.get_mut("accounts") {
        return prepare_import(items, region);
    }
    if !obj.contains_key("auth_user_info_raw") && !obj.contains_key("userInfo") {
        let info = from_client(&Value::Object(obj.clone()), region)?;
        *value = json!({"userInfo":info});
        return Ok(());
    }
    let info = if obj.contains_key("auth_user_info_raw") {
        obj.get_mut("auth_user_info_raw").unwrap()
    } else {
        obj.get_mut("userInfo").unwrap()
    };
    let info = info
        .as_object_mut()
        .ok_or("QODER_IMPORT_IDENTITY_MISSING")?;
    if let Some(source) = info.get("login_source").and_then(Value::as_str) {
        let matches = source == region.source()
            || (source == "dedicated_qodercn" && region == QoderRegion::Global);
        if !matches {
            return Err(
                "导入账号与当前平台不一致，请进入对应的 Qoder 或 Qoder CN 平台后重试。".into(),
            );
        }
    } else {
        info.insert("login_source".into(), json!(region.source()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_roundtrip_preserves_identity_and_realm_for_both_clients() {
        let info=from_client(&json!({"token":"access-test","refreshToken":"refresh-test","expiresAt":"2027-01-01T00:00:00Z","user":{"id":"u1","name":"test","data_policy_agreed":true}}),QoderRegion::Cn).unwrap();
        for desktop in [true, false] {
            let native = for_client(&info, desktop).unwrap();
            let restored = from_client(&native, QoderRegion::Cn).unwrap();
            for key in ["id", "token", "refreshToken", "expireTime", "login_source"] {
                assert_eq!(info[key], restored[key], "{key}");
            }
            assert!(native.get("authenticationContext").is_none());
            assert!(native.get("profileOverlay").is_none());
        }
    }
    #[test]
    fn missing_identity_or_refresh_is_not_synthesized() {
        assert!(from_client(&json!({"token":"test"}), QoderRegion::Cn).is_err());
        assert!(for_client(
            &json!({"id":"test","token":"test","expireTime":"1800000000000"}),
            true
        )
        .is_err());
    }
    #[test]
    fn import_rejects_cross_region_overrides() {
        let mut global = json!({"userInfo":{"id":"u","token":"test","login_source":"qoder"}});
        assert!(prepare_import(&mut global, QoderRegion::Cn).is_err());
        assert_eq!(global["userInfo"]["login_source"], "qoder");
        let mut legacy = json!({"userInfo":{"id":"u","token":"test"}});
        prepare_import(&mut legacy, QoderRegion::Cn).unwrap();
        assert_eq!(legacy["userInfo"]["login_source"], "qodercn");
    }
}
