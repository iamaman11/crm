#![forbid(unsafe_code)]

pub mod domain;
pub mod persistence;

pub use domain::*;
pub use persistence::*;

/// Architecture marker for `crm-activities`.
pub const CRATE_NAME: &str = "crm-activities";
