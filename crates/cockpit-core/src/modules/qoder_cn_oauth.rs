use crate::modules::qoder_cn_account as qoder_account;
const DEFAULT_REGION: crate::modules::qoder_region::QoderRegion =
    crate::modules::qoder_region::QoderRegion::Cn;
include!("qoder_oauth_impl.rs");
