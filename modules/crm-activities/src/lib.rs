#![forbid(unsafe_code)]

pub mod domain;

pub use domain::*;

/// Architecture marker for `crm-activities`.
pub const CRATE_NAME: &str = "crm-activities";
