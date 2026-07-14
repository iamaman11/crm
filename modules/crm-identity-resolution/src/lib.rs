#![forbid(unsafe_code)]

//! Authoritative identity-resolution candidate-case and merge-lineage owner domain.
//!
//! This pure owner crate contains no SQL, transport types or direct cross-owner
//! storage access. Party existence, lifecycle and exact source-version integrity
//! are validated by application composition before governed owner mutations.
//! Only Party-owned domain/planner code may construct authoritative Party state
//! transitions; this crate owns duplicate evidence, merge lineage, survivorship
//! decisions and provenance.

pub mod domain;
pub mod merge;
pub mod persistence;

pub use domain::*;
pub use merge::*;
pub use persistence::*;

pub const MODULE_ID: &str = "crm.identity-resolution";
pub const CANDIDATE_CASE_RECORD_TYPE: &str = "identity_resolution.candidate_case";
pub const MERGE_LINEAGE_RECORD_TYPE: &str = "identity_resolution.merge_lineage";

/// Backward-compatible alias retained for the Phase 8A.5 candidate-case adapters.
pub const RECORD_TYPE: &str = CANDIDATE_CASE_RECORD_TYPE;
