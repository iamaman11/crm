#![forbid(unsafe_code)]

//! Authoritative identity-resolution owner domain.
//!
//! This pure owner crate contains no SQL, transport types or direct cross-owner
//! storage access. Party existence and exact source-version integrity are
//! validated by application composition before governed owner mutations.
//!
//! Duplicate-candidate review state and reversible merge lineage are separate
//! authoritative records. Merge lineage never deletes Party records or rewrites
//! downstream references destructively. Canonical Party resolution is derived
//! only from the bounded current set of active authoritative merge-operation edges.

pub mod domain;
pub mod merge_lineage;
pub mod merge_lineage_persistence;
pub mod persistence;

pub use domain::*;
pub use merge_lineage::*;
pub use merge_lineage_persistence::*;
pub use persistence::*;

pub const MODULE_ID: &str = "crm.identity-resolution";
pub const RECORD_TYPE: &str = "identity_resolution.candidate_case";
pub const CANDIDATE_CASE_RECORD_TYPE: &str = RECORD_TYPE;
pub const MERGE_OPERATION_RECORD_TYPE: &str = "identity_resolution.merge_operation";
