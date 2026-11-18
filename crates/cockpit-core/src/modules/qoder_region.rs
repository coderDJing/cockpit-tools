//! Explicit Qoder realms. A credential is never probed against another realm.
use crate::models::qoder::QoderAccount;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum QoderRegion {
    Global,
    Cn,
}

impl QoderRegion {
    pub fn parse(value: Option<&str>) -> Result<Self, String> {
        match value.unwrap_or("global") {
            "global" => Ok(Self::Global),
            "cn" => Ok(Self::Cn),
            _ => Err("QODER_REGION_INVALID".into()),
        }
    }
    pub fn source(self) -> &'static str {
        match self {
            Self::Global => "qoder",
            Self::Cn => "qodercn",
        }
    }
    pub fn api(self) -> &'static str {
        match self {
            Self::Global => "https://openapi.qoder.sh",
            Self::Cn => "https://openapi.qoder.com.cn",
        }
    }
    pub fn website(self) -> &'static str {
        match self {
            Self::Global => "https://qoder.com",
            Self::Cn => "https://qoder.cn",
        }
    }
    pub fn desktop_dir(self) -> &'static str {
        match self {
            Self::Global => "com.qoder.app.stable",
            Self::Cn => "com.qodercn.app.stable",
        }
    }
    pub fn cli_dir(self) -> &'static str {
        match self {
            Self::Global => ".qoder",
            Self::Cn => ".qoder-cn",
        }
    }
    pub fn device_login_url(
        self,
        nonce: &str,
        challenge: &str,
        machine_id: &str,
    ) -> Result<String, String> {
        let mut callback = url::Url::parse(&format!("{}/device/selectAccounts", self.website()))
            .map_err(|e| e.to_string())?;
        callback
            .query_pairs_mut()
            .append_pair("nonce", nonce)
            .append_pair("challenge", challenge)
            .append_pair("challenge_method", "S256")
            .append_pair("client_id", "732aef47-9cf2-46a2-95fe-4cebb5d0d1fa")
            .append_pair("machine_id", machine_id);
        let mut signin = url::Url::parse(&format!("{}/users/sign-in", self.website()))
            .map_err(|e| e.to_string())?;
        signin
            .query_pairs_mut()
            .append_pair("biz_variant", "qoder")
            .append_pair("oauth_callback", callback.as_str());
        Ok(signin.to_string())
    }
    pub fn from_account(account: &QoderAccount) -> Result<Self, String> {
        QoderLoginSource::for_account(account).map(QoderLoginSource::region)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum QoderLoginSource {
    Public(QoderRegion),
    Dedicated,
    LegacyLocal,
}

impl QoderLoginSource {
    pub(crate) fn for_account(account: &QoderAccount) -> Result<Self, String> {
        Ok(Self::recorded_for_account(account)?.unwrap_or(Self::Public(QoderRegion::Global)))
    }

    pub(crate) fn recorded_for_account(account: &QoderAccount) -> Result<Option<Self>, String> {
        Self::resolve(
            account.auth_user_info_raw.as_ref(),
            account.auth_user_plan_raw.as_ref(),
        )
    }

    pub(crate) fn resolve(info: Option<&Value>, plan: Option<&Value>) -> Result<Option<Self>, String> {
        let info_source = Self::read(info)?;
        let plan_source = Self::read(plan)?;
        if matches!((info_source, plan_source), (Some(info), Some(plan)) if info != plan) {
            return Err("QODER_PLATFORM_MISMATCH".into());
        }
        Ok(info_source.or(plan_source))
    }

    fn read(metadata: Option<&Value>) -> Result<Option<Self>, String> {
        let Some(value) = metadata.and_then(|value| value.get("login_source")) else {
            return Ok(None);
        };
        if value.is_null() {
            return Ok(None);
        }
        match value.as_str().map(str::trim) {
            Some("") => Ok(None),
            Some("qoder") => Ok(Some(Self::Public(QoderRegion::Global))),
            Some("qodercn") => Ok(Some(Self::Public(QoderRegion::Cn))),
            Some("dedicated_qodercn") => Ok(Some(Self::Dedicated)),
            Some("qodercn_legacy_local") => Ok(Some(Self::LegacyLocal)),
            _ => Err("QODER_REGION_UNSUPPORTED".into()),
        }
    }

    pub(crate) fn region(self) -> QoderRegion {
        match self {
            Self::Public(region) => region,
            // These clients use the existing Qoder IDE store and require local endpoint verification.
            Self::Dedicated | Self::LegacyLocal => QoderRegion::Global,
        }
    }

    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Public(region) => region.source(),
            Self::Dedicated => "dedicated_qodercn",
            Self::LegacyLocal => "qodercn_legacy_local",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn realms_are_explicit_and_isolated() {
        assert_ne!(QoderRegion::Cn.api(), QoderRegion::Global.api());
        assert_ne!(
            QoderRegion::Cn.desktop_dir(),
            QoderRegion::Global.desktop_dir()
        );
        assert_ne!(QoderRegion::Cn.cli_dir(), QoderRegion::Global.cli_dir());
        assert!(QoderRegion::parse(Some("https://other.invalid")).is_err());
        let cn = serde_json::json!({ "login_source": "qodercn" });
        assert_eq!(
            QoderLoginSource::resolve(Some(&cn), None),
            Ok(Some(QoderLoginSource::Public(QoderRegion::Cn)))
        );
        let unknown = serde_json::json!({ "login_source": "unknown" });
        assert_eq!(
            QoderLoginSource::resolve(Some(&unknown), None),
            Err("QODER_REGION_UNSUPPORTED".into())
        );
    }
    #[test]
    fn existing_ide_sources_keep_their_local_endpoint_policies() {
        for source in [QoderLoginSource::Dedicated, QoderLoginSource::LegacyLocal] {
            let plan = serde_json::json!({ "login_source": source.name() });
            assert_eq!(QoderLoginSource::resolve(None, Some(&plan)), Ok(Some(source)));
            assert_eq!(source.region(), QoderRegion::Global);
        }
    }

    #[test]
    fn cn_login_and_callback_never_point_at_global_service() {
        let login = QoderRegion::Cn
            .device_login_url("nonce-test", "challenge-test", "machine-test")
            .unwrap();
        let login = url::Url::parse(&login).unwrap();
        assert_eq!(login.host_str(), Some("qoder.cn"));
        let callback = login
            .query_pairs()
            .find(|(k, _)| k == "oauth_callback")
            .unwrap()
            .1
            .into_owned();
        let callback = url::Url::parse(&callback).unwrap();
        assert_eq!(callback.host_str(), Some("qoder.cn"));
        assert!(callback
            .query_pairs()
            .any(|(k, v)| k == "challenge" && v == "challenge-test"));
        assert!(!callback.query_pairs().any(|(k, _)| k == "redirect_uri"));
    }
}
