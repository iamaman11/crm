#![forbid(unsafe_code)]

//! Contract-only authoritative Data Quality privacy-scope contribution.
//!
//! The adapter contributes reference-only Party evaluation, finding, completeness and
//! remediation evidence. Shared rule-set and completeness-profile definitions are strict
//! validation dependencies and are never emitted as subject resources. No runtime route,
//! application registration or worker is introduced by this package, and query execution
//! remains read-only.

mod contract;
mod digest;
mod errors;
mod owner_action;
mod postgres;
mod request;
mod response;

#[cfg(test)]
mod tests;

pub use contract::{
    CAPABILITY_ID, CAPABILITY_VERSION, CONTRACT_SCHEMA_VERSION, DEFAULT_PAGE_SIZE,
    INPUT_MAXIMUM_BYTES, INPUT_RETENTION_POLICY_ID, INPUT_SCHEMA_ID,
    MAX_PRIVACY_ASSOCIATION_RECORDS_REHYDRATED, MAX_PRIVACY_CANONICAL_PARTY_RESOLUTIONS,
    MAX_PRIVACY_COMPLETENESS_RESULTS_SCANNED, MAX_PRIVACY_DEFINITION_RECORDS_REHYDRATED,
    MAX_PRIVACY_EVALUATION_INPUTS_SCANNED, MAX_PRIVACY_EVALUATION_JOBS_SCANNED,
    MAX_PRIVACY_FINDING_OBSERVATIONS_SCANNED, MAX_PRIVACY_FINDINGS_SCANNED,
    MAX_PRIVACY_OWNER_RECORDS_SCANNED, MAX_PRIVACY_REMEDIATION_ATTEMPTS_SCANNED,
    MAX_PRIVACY_RULE_OUTCOMES_SCANNED, MAXIMUM_CURSOR_BYTES, MAXIMUM_PAGE_SIZE,
    OUTPUT_MAXIMUM_BYTES, OUTPUT_RETENTION_POLICY_ID, OUTPUT_SCHEMA_ID,
    PRIVACY_OWNER_SCAN_BATCH_SIZE, data_quality_privacy_scope_definition,
};
pub use owner_action::{
    DataQualityPrivacyActionPlanner, DataQualityPrivacyActionPolicy, OWNER_ACTION_CAPABILITY_ID,
    data_quality_privacy_action_definition, data_quality_privacy_action_planner,
};
pub use postgres::DataQualityPrivacyScopeQueryAdapter;
