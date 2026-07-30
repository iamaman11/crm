#![forbid(unsafe_code)]

//! Governed execution composition for validated customer-data imports and exports.
//!
//! The pure customer-data-operations domain owns import/export coordination state while
//! customer-master owners remain authoritative for their records. This composition invokes
//! governed owner boundaries and owns only operation jobs, immutable artifacts, outcomes and
//! reconciliation evidence. No direct cross-owner storage path exists here.

pub use crm_core_files::{
    AppendImmutableFileChunk, CreateImmutableFileArtifact, FileArtifactAppendResult,
    FileArtifactMetadata, FileArtifactStatus, FinalizedFileArtifact, ImmutableFileArtifactStore,
};

pub mod export_execution_reader;
pub use export_execution_reader::*;
pub mod export_execution_sink;
pub use export_execution_sink::*;
pub mod export_execution_source;
pub use export_execution_source::*;
pub mod export_execution_worker;
pub use export_execution_worker::*;
pub mod export_selection_reader;
pub use export_selection_reader::*;
pub mod export_selection_sink;
pub use export_selection_sink::*;
pub mod export_selection_worker;
pub use export_selection_worker::*;
pub mod privacy_export;
pub use privacy_export::*;
pub mod postgres_reader;
pub use postgres_reader::*;
pub mod outcome_plan;
pub use outcome_plan::*;
pub mod postgres_outcome_sink;
pub use postgres_outcome_sink::*;
pub mod worker;
pub use worker::*;

pub const MODULE_ID: &str = "crm.customer-data-operations";
pub const CONTRACT_VERSION: &str = "1.0.0";
