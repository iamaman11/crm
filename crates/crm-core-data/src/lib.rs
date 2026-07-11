#![forbid(unsafe_code)]

pub mod aggregate_executor;
mod audit;
pub mod capability_executor;
pub mod postgres;
pub mod postgres_batch;

pub use aggregate_executor::*;
pub use audit::AuditIntent;
pub use capability_executor::*;
pub use postgres::*;
pub use postgres_batch::*;

/// Architecture marker for `crm-core-data`.
pub const CRATE_NAME: &str = "crm-core-data";
