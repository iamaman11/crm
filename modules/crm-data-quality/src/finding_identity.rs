use crate::{RuleKey, derived_identity::derived_id};
use crm_module_sdk::{RecordId, TenantId};

const FINDING_ID_DOMAIN: &[u8] = b"crm.data-quality.finding/v1";
const TARGET_OWNER_MODULE_ID: &[u8] = b"crm.parties";
const TARGET_RESOURCE_TYPE: &[u8] = b"parties.party";

pub fn party_finding_id(
    tenant_id: &TenantId,
    party_id: &RecordId,
    rule_set_version_id: &str,
    rule_key: &RuleKey,
) -> String {
    derived_id(
        "dq-finding",
        FINDING_ID_DOMAIN,
        &[
            tenant_id.as_str().as_bytes(),
            TARGET_OWNER_MODULE_ID,
            TARGET_RESOURCE_TYPE,
            party_id.as_str().as_bytes(),
            rule_set_version_id.as_bytes(),
            rule_key.as_str().as_bytes(),
        ],
    )
}
