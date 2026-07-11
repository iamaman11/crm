#![forbid(unsafe_code)]

pub mod aggregate_executor;
mod audit;
pub mod capability_executor;
pub mod postgres;
pub mod postgres_batch;
mod postgres_query;

pub use aggregate_executor::*;
pub use audit::AuditIntent;
pub use capability_executor::*;
pub use crm_module_sdk::RecordSnapshot;
pub use postgres::*;
pub use postgres_batch::*;
pub use postgres_query::{
    MAXIMUM_RECORD_QUERY_PAGE_SIZE, RecordGetQuery, RecordListQuery, RecordQueryContinuation,
    RecordQueryPage, RecordQuerySort,
};

/// Architecture marker for `crm-core-data`.
pub const CRATE_NAME: &str = "crm-core-data";
