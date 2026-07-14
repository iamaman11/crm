#![forbid(unsafe_code)]

//! Authoritative identity-resolution candidate-case owner domain.
//!
//! This pure owner crate contains no SQL, transport types or direct cross-owner
//! storage access. Party existence and exact source-version integrity are
//! validated by application composition before governed owner mutations.

pub mod domain;
pub mod persistence;

pub use domain::*;
pub use persistence::*;

pub const MODULE_ID: &str = "crm.identity-resolution";
pub const RECORD_TYPE: &str = "identity_resolution.candidate_case";
