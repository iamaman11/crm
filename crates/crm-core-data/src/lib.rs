#![forbid(unsafe_code)]

pub mod postgres;
pub mod postgres_batch;

pub use postgres::*;
pub use postgres_batch::*;

/// Architecture marker for `crm-core-data`.
pub const CRATE_NAME: &str = "crm-core-data";
