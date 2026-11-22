#![forbid(unsafe_code)]

pub const MODULE_ID: &str = "crm.activities";

pub mod domain;
pub mod persistence;
pub mod recurrence;

pub use domain::*;
pub use persistence::*;
pub use recurrence::*;

/// Architecture marker for `crm-activities`.
pub const CRATE_NAME: &str = "crm-activities";
