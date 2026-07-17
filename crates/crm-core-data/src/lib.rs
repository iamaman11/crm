#![forbid(unsafe_code)]

pub mod aggregate_executor;
mod audit;
mod audited_read;
pub mod capability_executor;
mod metadata_capability_executor;
mod metadata_query_store;
mod metadata_store;
mod module_activation;
pub mod postgres;
pub mod postgres_batch;
mod postgres_event_delivery;
mod postgres_event_delivery_ledger;
mod postgres_file_artifact;
mod postgres_file_artifact_capability;
mod postgres_file_artifact_evidence;
mod postgres_projection;
mod postgres_query;
mod postgres_related_query;
mod projection_store;
mod search_generation_store;
mod search_store;

pub use aggregate_executor::*;
pub use audit::AuditIntent;
pub use audited_read::*;
pub use capability_executor::*;
pub use crm_core_files::{
    AppendImmutableFileChunk, CreateImmutableFileArtifact, FileArtifactMetadata,
    FileArtifactStatus, ImmutableFileArtifactStore,
};
pub use crm_module_sdk::RecordSnapshot;
pub use metadata_capability_executor::*;
pub use metadata_query_store::*;
pub use metadata_store::*;
pub use postgres::*;
pub use postgres_batch::*;
pub use postgres_event_delivery::EventDeliveryQuery;
pub use postgres_event_delivery_ledger::{
    ClaimedEventDelivery, EventDeliveryClaim, EventDeliveryCompletion,
};
pub use postgres_file_artifact::PostgresImmutableFileArtifactStore;
pub use postgres_file_artifact_capability::{
    FileArtifactCapabilityEvidence, FileArtifactCapabilityMutation,
    FileArtifactCapabilityMutationResult,
};
pub use postgres_query::{
    MAXIMUM_RECORD_QUERY_PAGE_SIZE, RecordGetQuery, RecordListQuery, RecordQueryContinuation,
    RecordQueryPage, RecordQuerySort,
};
pub use postgres_related_query::{
    MAXIMUM_RELATED_RECORD_QUERY_PAGE_SIZE, RelatedRecordListQuery, RelatedRecordQueryPage,
};

/// Architecture marker for `crm-core-data`.
pub const CRATE_NAME: &str = "crm-core-data";
