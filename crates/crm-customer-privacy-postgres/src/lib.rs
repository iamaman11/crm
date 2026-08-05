#![forbid(unsafe_code)]

//! Stable PostgreSQL boundary for Customer Privacy.
//!
//! The existing persistence and transaction-guard crates remain transitional
//! implementation details. New owner persistence enters through this package.

mod access_export;
mod discovery;
mod execution;
mod planning;
mod reads;
mod ready;
mod restrictions;
mod retention;

pub use access_export::*;
pub use discovery::*;
pub use execution::*;
pub use planning::*;
pub use reads::*;
pub use restrictions::*;
pub use retention::*;

pub use crm_customer_privacy_capability_composition::{
    PostgresCustomerPrivacyApprovalGuard, PostgresCustomerPrivacyCancellationGuard,
    PostgresCustomerPrivacyLegalHoldPlacementGuard,
    PostgresCustomerPrivacyLegalHoldReleaseGuard, PostgresCustomerPrivacyPreviousCaseGuard,
    PostgresCustomerPrivacyRestrictionPlacementGuard,
    PostgresCustomerPrivacyRestrictionReleaseGuard,
    PostgresCustomerPrivacySubjectVerificationGuard, postgres_case_approval_executor,
    postgres_case_cancel_executor, postgres_case_create_executor,
    postgres_case_subject_verify_executor, postgres_case_submit_executor,
    postgres_legal_hold_place_executor, postgres_legal_hold_release_executor,
    postgres_restriction_place_executor, postgres_restriction_release_executor,
};
pub use crm_customer_privacy_persistence_adapter::{
    legal_hold_from_snapshot, legal_hold_persisted_contract, legal_hold_persisted_payload,
    legal_hold_record_ref, privacy_case_from_snapshot, privacy_case_persisted_contract,
    privacy_case_persisted_payload, privacy_case_record_ref, processing_restriction_from_snapshot,
    processing_restriction_persisted_contract, processing_restriction_persisted_payload,
    processing_restriction_record_ref, retention_decision_from_snapshot,
    retention_decision_persisted_contract, retention_decision_persisted_payload,
    retention_decision_record_ref,
};
