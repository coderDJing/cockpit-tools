// Shared implementation; each platform has an independent typed index and lock.
use crate::models::qoder::QoderCnAccountIndex as QoderAccountIndex;
const ACCOUNTS_INDEX_FILE: &str = "qoder_cn_accounts.json";
const ACCOUNTS_DIR: &str = "qoder_cn_accounts";
const STORE_KIND: &str = "qoder_cn";
const STORE_REGION: crate::modules::qoder_region::QoderRegion =
    crate::modules::qoder_region::QoderRegion::Cn;
include!("qoder_account_impl.rs");
