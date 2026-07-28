#![forbid(unsafe_code)]

//! Stable PostgreSQL boundary for Customer Privacy.
//!
//! The existing persistence and transaction-guard crates remain transitional
//! implementation details. New owner persistence enters through this package.

mod discovery;
mod planning;
mod reads;

pub use discovery::*;
pub use planning::*;
pub use reads::*;

pub use crm_customer_privacy_capability_composition::{
    PostgresCustomerPrivacyApprovalGuard, PostgresCustomerPrivacyCancellationGuard,
    PostgresCustomerPrivacyPreviousCaseGuard, PostgresCustomerPrivacySubjectVerificationGuard,
    postgres_case_approval_executor, postgres_case_cancel_executor, postgres_case_create_executor,
    postgres_case_subject_verify_executor, postgres_case_submit_executor,
};
pub use crm_customer_privacy_persistence_adapter::{
    legal_hold_from_snapshot, legal_hold_persisted_contract, legal_hold_persisted_payload,
    legal_hold_record_ref, privacy_case_from_snapshot, privacy_case_persisted_contract,
    privacy_case_persisted_payload, privacy_case_record_ref, processing_restriction_from_snapshot,
    processing_restriction_persisted_contract, processing_restriction_persisted_payload,
    processing_restriction_record_ref,
};
