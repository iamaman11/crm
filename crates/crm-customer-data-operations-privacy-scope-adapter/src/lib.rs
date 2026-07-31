#![forbid(unsafe_code)]

//! Contract-only authoritative Customer Data Operations privacy-scope contribution.
//!
//! The adapter contributes subject-level import-row, export-selection, execution-stage and
//! execution-outcome evidence while excluding multi-subject jobs, progress records and complete
//! artifacts. No runtime route, application registration or worker is introduced by this package,
//! and query execution remains read-only.

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
    MAX_PRIVACY_ASSOCIATED_EXPORT_RECORDS_REHYDRATED, MAX_PRIVACY_CANONICAL_PARTY_RESOLUTIONS,
    MAX_PRIVACY_EXPORT_SELECTION_ITEMS_SCANNED, MAX_PRIVACY_IMPORT_ROWS_SCANNED,
    MAX_PRIVACY_OWNER_RECORDS_SCANNED, MAXIMUM_CURSOR_BYTES, MAXIMUM_PAGE_SIZE,
    OUTPUT_MAXIMUM_BYTES, OUTPUT_RETENTION_POLICY_ID, OUTPUT_SCHEMA_ID,
    customer_data_privacy_scope_definition,
};
pub use owner_action::{
    OWNER_ACTION_CAPABILITY_ID, CustomerDataOperationsPrivacyActionPlanner,
    CustomerDataOperationsPrivacyActionPolicy, customer_data_operations_privacy_action_definition,
    customer_data_operations_privacy_action_planner,
};
pub use postgres::CustomerDataOperationsPrivacyScopeQueryAdapter;
