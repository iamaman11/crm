#![forbid(unsafe_code)]

mod definitions;
mod executor;
mod wire;

pub use definitions::*;
pub use executor::*;

/// Architecture marker for the production Sales and Activities query adapter.
pub const CRATE_NAME: &str = "crm-sales-activities-query-adapter";
