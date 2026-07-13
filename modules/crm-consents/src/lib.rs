#![forbid(unsafe_code)]

//! Authoritative Consent and Communication Authorization owner domain.
//!
//! This pure owner crate contains no SQL, transport/provider types or direct
//! cross-owner storage access. Party and optional Contact Point integrity are
//! validated by application composition before governed owner mutations.

pub mod domain;
pub mod persistence;

pub use domain::*;
pub use persistence::*;

pub const MODULE_ID: &str = "crm.consents";
pub const RECORD_TYPE: &str = "consents.authorization";
