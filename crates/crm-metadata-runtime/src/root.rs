#![forbid(unsafe_code)]

//! Public composition surface for the Admin Studio metadata runtime.
//!
//! The deterministic single-scope engine lives in `lib.rs` and is intentionally
//! kept private here. Application-facing callers use [`TenantMetadataCatalog`],
//! which binds publication authority, revision lookup, impact analysis,
//! activation and rollback to one explicit [`crm_module_sdk::TenantId`].

#[path = "lib.rs"]
mod scoped;
mod tenant;

pub use scoped::{
    ActivationResult, MAX_METADATA_DOCUMENT_BYTES, MAX_METADATA_ID_BYTES,
    MAX_SCHEMA_VERSION_BYTES, METADATA_REVISION_HASH_PROFILE, MetadataBundleDraft, MetadataChange,
    MetadataChangeType, MetadataDocument, MetadataError, MetadataErrorCode, MetadataId,
    MetadataImpactReport, MetadataImpactSeverity, MetadataKey, MetadataKind, MetadataRevisionId,
    PublishResult, PublishedMetadataRevision, RollbackResult, TenantMetadataSnapshot,
};
pub use tenant::TenantMetadataCatalog;
