// Shared implementation; each platform has an independent typed index and lock.
use crate::models::qoder::QoderAccountIndex;
const ACCOUNTS_INDEX_FILE: &str = "qoder_accounts.json";
const ACCOUNTS_DIR: &str = "qoder_accounts";
const STORE_KIND: &str = "qoder";
const STORE_REGION: crate::modules::qoder_region::QoderRegion =
    crate::modules::qoder_region::QoderRegion::Global;
include!("qoder_account_impl.rs");
