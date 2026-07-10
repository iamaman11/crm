#![forbid(unsafe_code)]

pub mod postgres;

pub use postgres::*;

/// Architecture marker for `crm-core-data`.
pub const CRATE_NAME: &str = "crm-core-data";
