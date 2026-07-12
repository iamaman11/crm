#![forbid(unsafe_code)]

//! Pure lifecycle rules for immutable Admin Studio metadata publications.
//!
//! The runtime owns generic publication mechanics only: validated metadata
//! coordinates, deterministic bundle identity, structural impact analysis,
//! tenant-scoped optimistic activation and rollback. Kind-specific metadata
//! schemas, durable persistence, transport and product UI remain outside this
//! crate.

use crm_module_sdk::TenantId;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt::{self, Write as _};

pub const METADATA_REVISION_HASH_PROFILE: &str = "crm.metadata.bundle.sha256/v1";
pub const MAX_METADATA_ID_BYTES: usize = 180;
pub const MAX_SCHEMA_VERSION_BYTES: usize = 80;
pub const MAX_METADATA_DOCUMENT_BYTES: usize = 4 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MetadataKind {
    Object,
    Field,
    Relationship,
    Layout,
    View,
    Pipeline,
    Permission,
    Workflow,
}

impl MetadataKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Object => "object",
            Self::Field => "field",
            Self::Relationship => "relationship",
            Self::Layout => "layout",
            Self::View => "view",
            Self::Pipeline => "pipeline",
            Self::Permission => "permission",
            Self::Workflow => "workflow",
        }
    }

    const fn canonical_tag(self) -> u8 {
        match self {
            Self::Object => 1,
            Self::Field => 2,
            Self::Relationship => 3,
            Self::Layout => 4,
            Self::View => 5,
            Self::Pipeline => 6,
            Self::Permission => 7,
            Self::Workflow => 8,
        }
    }
}

impl fmt::Display for MetadataKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MetadataId(String);

impl MetadataId {
    pub fn try_new(value: impl Into<String>) -> Result<Self, MetadataError> {
        let value = value.into();
        if value.is_empty()
            || value.len() > MAX_METADATA_ID_BYTES
            || !value.contains('.')
            || !value
                .bytes()
                .next()
                .is_some_and(|byte| byte.is_ascii_lowercase())
            || !value.bytes().all(|byte| {
                byte.is_ascii_lowercase()
                    || byte.is_ascii_digit()
                    || matches!(byte, b'.' | b'_' | b'-')
            })
        {
            return Err(MetadataError::new(
                MetadataErrorCode::InvalidIdentifier,
                "The metadata identifier is invalid.",
                value,
            ));
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for MetadataId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MetadataKey {
    kind: MetadataKind,
    id: MetadataId,
}

impl MetadataKey {
    pub fn new(kind: MetadataKind, id: MetadataId) -> Self {
        Self { kind, id }
    }

    pub const fn kind(&self) -> MetadataKind {
        self.kind
    }

    pub fn id(&self) -> &MetadataId {
        &self.id
    }
}

impl fmt::Display for MetadataKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}:{}", self.kind, self.id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetadataDocument {
    key: MetadataKey,
    schema_version: String,
    canonical_content: Vec<u8>,
    dependencies: BTreeSet<MetadataKey>,
}

impl MetadataDocument {
    pub fn new(
        key: MetadataKey,
        schema_version: impl Into<String>,
        canonical_content: Vec<u8>,
        dependencies: impl IntoIterator<Item = MetadataKey>,
    ) -> Result<Self, MetadataError> {
        let schema_version = schema_version.into();
        if schema_version.is_empty()
            || schema_version.len() > MAX_SCHEMA_VERSION_BYTES
            || schema_version.chars().any(char::is_control)
        {
            return Err(MetadataError::new(
                MetadataErrorCode::InvalidSchemaVersion,
                "The metadata schema version is invalid.",
                schema_version,
            ));
        }
        if canonical_content.is_empty() {
            return Err(MetadataError::new(
                MetadataErrorCode::EmptyContent,
                "Metadata content must not be empty.",
                key.to_string(),
            ));
        }
        if canonical_content.len() > MAX_METADATA_DOCUMENT_BYTES {
            return Err(MetadataError::new(
                MetadataErrorCode::ContentTooLarge,
                "Metadata content exceeds the maximum supported size.",
                key.to_string(),
            ));
        }
        let dependencies = dependencies.into_iter().collect::<BTreeSet<_>>();
        if dependencies.contains(&key) {
            return Err(MetadataError::new(
                MetadataErrorCode::SelfDependency,
                "Metadata definitions cannot depend on themselves.",
                key.to_string(),
            ));
        }
        Ok(Self {
            key,
            schema_version,
            canonical_content,
            dependencies,
        })
    }

    pub fn key(&self) -> &MetadataKey {
        &self.key
    }

    pub fn schema_version(&self) -> &str {
        &self.schema_version
    }

    pub fn canonical_content(&self) -> &[u8] {
        &self.canonical_content
    }

    pub fn dependencies(&self) -> &BTreeSet<MetadataKey> {
        &self.dependencies
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetadataBundleDraft {
    documents: BTreeMap<MetadataKey, MetadataDocument>,
}

impl MetadataBundleDraft {
    pub fn new(
        documents: impl IntoIterator<Item = MetadataDocument>,
    ) -> Result<Self, MetadataError> {
        let mut indexed = BTreeMap::new();
        for document in documents {
            let key = document.key.clone();
            if indexed.insert(key.clone(), document).is_some() {
                return Err(MetadataError::new(
                    MetadataErrorCode::DuplicateDocument,
                    "The metadata bundle contains a duplicate definition.",
                    key.to_string(),
                ));
            }
        }
        if indexed.is_empty() {
            return Err(MetadataError::new(
                MetadataErrorCode::EmptyBundle,
                "A metadata publication must contain at least one definition.",
                "empty metadata bundle",
            ));
        }
        for document in indexed.values() {
            for dependency in document.dependencies() {
                if !indexed.contains_key(dependency) {
                    return Err(MetadataError::new(
                        MetadataErrorCode::MissingDependency,
                        "A metadata dependency is missing from the complete bundle snapshot.",
                        format!("{} -> {dependency}", document.key()),
                    ));
                }
            }
        }
        Ok(Self { documents: indexed })
    }

    pub fn documents(&self) -> &BTreeMap<MetadataKey, MetadataDocument> {
        &self.documents
    }

    pub fn revision_id(&self) -> MetadataRevisionId {
        metadata_revision_id(&self.documents)
    }

    fn into_documents(self) -> BTreeMap<MetadataKey, MetadataDocument> {
        self.documents
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MetadataRevisionId([u8; 32]);

impl MetadataRevisionId {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn to_hex(&self) -> String {
        let mut output = String::with_capacity(64);
        for byte in self.0 {
            write!(&mut output, "{byte:02x}").expect("writing to a String cannot fail");
        }
        output
    }
}

impl fmt::Display for MetadataRevisionId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.to_hex())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublishedMetadataRevision {
    revision_id: MetadataRevisionId,
    documents: BTreeMap<MetadataKey, MetadataDocument>,
    published_at_unix_millis: u64,
}

impl PublishedMetadataRevision {
    pub fn revision_id(&self) -> &MetadataRevisionId {
        &self.revision_id
    }

    pub fn documents(&self) -> &BTreeMap<MetadataKey, MetadataDocument> {
        &self.documents
    }

    pub const fn published_at_unix_millis(&self) -> u64 {
        self.published_at_unix_millis
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublishResult {
    pub revision_id: MetadataRevisionId,
    pub already_published: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetadataChangeType {
    Added,
    Modified,
    Removed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MetadataImpactSeverity {
    Informational,
    ReviewRequired,
    Breaking,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetadataChange {
    pub key: MetadataKey,
    pub change_type: MetadataChangeType,
    pub severity: MetadataImpactSeverity,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetadataImpactReport {
    pub current_revision: Option<MetadataRevisionId>,
    pub candidate_revision: MetadataRevisionId,
    pub changes: Vec<MetadataChange>,
}

impl MetadataImpactReport {
    pub fn has_breaking_changes(&self) -> bool {
        self.changes
            .iter()
            .any(|change| change.severity == MetadataImpactSeverity::Breaking)
    }

    pub fn requires_review(&self) -> bool {
        self.changes
            .iter()
            .any(|change| change.severity >= MetadataImpactSeverity::ReviewRequired)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TenantMetadataSnapshot {
    pub generation: u64,
    pub active_revision: Option<MetadataRevisionId>,
    pub rollback_depth: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivationResult {
    pub generation: u64,
    pub active_revision: MetadataRevisionId,
    pub previous_revision: Option<MetadataRevisionId>,
    pub already_active: bool,
    pub impact: MetadataImpactReport,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RollbackResult {
    pub generation: u64,
    pub active_revision: MetadataRevisionId,
    pub replaced_revision: MetadataRevisionId,
}

#[derive(Debug, Clone, Default)]
struct TenantActivationState {
    generation: u64,
    active_revision: Option<MetadataRevisionId>,
    history: Vec<MetadataRevisionId>,
}

#[derive(Debug, Clone, Default)]
pub struct MetadataCatalog {
    revisions: BTreeMap<MetadataRevisionId, PublishedMetadataRevision>,
    activations: BTreeMap<TenantId, TenantActivationState>,
}

impl MetadataCatalog {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn publish(
        &mut self,
        draft: MetadataBundleDraft,
        published_at_unix_millis: u64,
    ) -> PublishResult {
        let revision_id = draft.revision_id();
        if self.revisions.contains_key(&revision_id) {
            return PublishResult {
                revision_id,
                already_published: true,
            };
        }
        self.revisions.insert(
            revision_id.clone(),
            PublishedMetadataRevision {
                revision_id: revision_id.clone(),
                documents: draft.into_documents(),
                published_at_unix_millis,
            },
        );
        PublishResult {
            revision_id,
            already_published: false,
        }
    }

    pub fn revision(
        &self,
        revision_id: &MetadataRevisionId,
    ) -> Result<&PublishedMetadataRevision, MetadataError> {
        self.revisions.get(revision_id).ok_or_else(|| {
            MetadataError::new(
                MetadataErrorCode::RevisionNotFound,
                "The requested metadata revision does not exist.",
                revision_id.to_string(),
            )
        })
    }

    pub fn tenant_state(&self, tenant_id: &TenantId) -> TenantMetadataSnapshot {
        let state = self.activations.get(tenant_id).cloned().unwrap_or_default();
        TenantMetadataSnapshot {
            generation: state.generation,
            active_revision: state.active_revision,
            rollback_depth: state.history.len(),
        }
    }

    pub fn impact_for(
        &self,
        tenant_id: &TenantId,
        candidate_revision: &MetadataRevisionId,
    ) -> Result<MetadataImpactReport, MetadataError> {
        let candidate = self.revision(candidate_revision)?;
        let current_revision = self
            .activations
            .get(tenant_id)
            .and_then(|state| state.active_revision.clone());
        let current_documents = current_revision
            .as_ref()
            .and_then(|revision_id| self.revisions.get(revision_id))
            .map(PublishedMetadataRevision::documents);
        let mut keys = BTreeSet::new();
        if let Some(documents) = current_documents {
            keys.extend(documents.keys().cloned());
        }
        keys.extend(candidate.documents().keys().cloned());

        let mut changes = Vec::new();
        for key in keys {
            let current = current_documents.and_then(|documents| documents.get(&key));
            let next = candidate.documents().get(&key);
            let change = match (current, next) {
                (None, Some(_)) => Some(MetadataChange {
                    key,
                    change_type: MetadataChangeType::Added,
                    severity: MetadataImpactSeverity::Informational,
                }),
                (Some(before), Some(after)) if before != after => Some(MetadataChange {
                    key,
                    change_type: MetadataChangeType::Modified,
                    severity: MetadataImpactSeverity::ReviewRequired,
                }),
                (Some(_), None) => Some(MetadataChange {
                    key,
                    change_type: MetadataChangeType::Removed,
                    severity: MetadataImpactSeverity::Breaking,
                }),
                (Some(_), Some(_)) | (None, None) => None,
            };
            if let Some(change) = change {
                changes.push(change);
            }
        }
        Ok(MetadataImpactReport {
            current_revision,
            candidate_revision: candidate_revision.clone(),
            changes,
        })
    }

    pub fn activate(
        &mut self,
        tenant_id: TenantId,
        candidate_revision: &MetadataRevisionId,
        expected_generation: u64,
        allow_breaking_changes: bool,
    ) -> Result<ActivationResult, MetadataError> {
        self.revision(candidate_revision)?;
        let current_generation = self
            .activations
            .get(&tenant_id)
            .map_or(0, |state| state.generation);
        if current_generation != expected_generation {
            return Err(MetadataError::new(
                MetadataErrorCode::GenerationConflict,
                "Metadata activation state changed concurrently.",
                format!(
                    "tenant={tenant_id}, expected={expected_generation}, actual={current_generation}"
                ),
            ));
        }
        let impact = self.impact_for(&tenant_id, candidate_revision)?;
        let previous_revision = self
            .activations
            .get(&tenant_id)
            .and_then(|state| state.active_revision.clone());
        if previous_revision.as_ref() == Some(candidate_revision) {
            return Ok(ActivationResult {
                generation: current_generation,
                active_revision: candidate_revision.clone(),
                previous_revision,
                already_active: true,
                impact,
            });
        }
        if impact.has_breaking_changes() && !allow_breaking_changes {
            return Err(MetadataError::new(
                MetadataErrorCode::BreakingChangeConfirmationRequired,
                "Breaking metadata changes require explicit confirmation before activation.",
                candidate_revision.to_string(),
            ));
        }
        let next_generation = current_generation.checked_add(1).ok_or_else(|| {
            MetadataError::new(
                MetadataErrorCode::GenerationOverflow,
                "Metadata activation generation cannot advance further.",
                tenant_id.to_string(),
            )
        })?;
        let state = self.activations.entry(tenant_id).or_default();
        if let Some(previous) = previous_revision.clone() {
            state.history.push(previous);
        }
        state.active_revision = Some(candidate_revision.clone());
        state.generation = next_generation;
        Ok(ActivationResult {
            generation: next_generation,
            active_revision: candidate_revision.clone(),
            previous_revision,
            already_active: false,
            impact,
        })
    }

    pub fn rollback(
        &mut self,
        tenant_id: &TenantId,
        expected_generation: u64,
    ) -> Result<RollbackResult, MetadataError> {
        let current_generation = self
            .activations
            .get(tenant_id)
            .map_or(0, |state| state.generation);
        if current_generation != expected_generation {
            return Err(MetadataError::new(
                MetadataErrorCode::GenerationConflict,
                "Metadata activation state changed concurrently.",
                format!(
                    "tenant={tenant_id}, expected={expected_generation}, actual={current_generation}"
                ),
            ));
        }
        let next_generation = current_generation.checked_add(1).ok_or_else(|| {
            MetadataError::new(
                MetadataErrorCode::GenerationOverflow,
                "Metadata activation generation cannot advance further.",
                tenant_id.to_string(),
            )
        })?;
        let state = self.activations.get_mut(tenant_id).ok_or_else(|| {
            MetadataError::new(
                MetadataErrorCode::RollbackUnavailable,
                "No previous metadata revision is available for rollback.",
                tenant_id.to_string(),
            )
        })?;
        let previous = state.history.pop().ok_or_else(|| {
            MetadataError::new(
                MetadataErrorCode::RollbackUnavailable,
                "No previous metadata revision is available for rollback.",
                tenant_id.to_string(),
            )
        })?;
        let replaced = state
            .active_revision
            .replace(previous.clone())
            .ok_or_else(|| {
                MetadataError::new(
                    MetadataErrorCode::RollbackUnavailable,
                    "No active metadata revision is available for rollback.",
                    tenant_id.to_string(),
                )
            })?;
        state.generation = next_generation;
        Ok(RollbackResult {
            generation: next_generation,
            active_revision: previous,
            replaced_revision: replaced,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetadataErrorCode {
    InvalidIdentifier,
    InvalidSchemaVersion,
    EmptyContent,
    ContentTooLarge,
    EmptyBundle,
    DuplicateDocument,
    SelfDependency,
    MissingDependency,
    RevisionNotFound,
    GenerationConflict,
    GenerationOverflow,
    BreakingChangeConfirmationRequired,
    RollbackUnavailable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetadataError {
    pub code: MetadataErrorCode,
    pub safe_message: &'static str,
    pub detail: String,
}

impl MetadataError {
    fn new(code: MetadataErrorCode, safe_message: &'static str, detail: impl Into<String>) -> Self {
        Self {
            code,
            safe_message,
            detail: detail.into(),
        }
    }
}

impl fmt::Display for MetadataError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} ({:?}): {}",
            self.safe_message, self.code, self.detail
        )
    }
}

impl Error for MetadataError {}

fn metadata_revision_id(documents: &BTreeMap<MetadataKey, MetadataDocument>) -> MetadataRevisionId {
    let mut hasher = Sha256::new();
    hash_bytes(&mut hasher, METADATA_REVISION_HASH_PROFILE.as_bytes());
    hash_usize(&mut hasher, documents.len());
    for (key, document) in documents {
        hash_key(&mut hasher, key);
        hash_bytes(&mut hasher, document.schema_version().as_bytes());
        hash_bytes(&mut hasher, document.canonical_content());
        hash_usize(&mut hasher, document.dependencies().len());
        for dependency in document.dependencies() {
            hash_key(&mut hasher, dependency);
        }
    }
    let digest = hasher.finalize();
    let mut bytes = [0_u8; 32];
    bytes.copy_from_slice(&digest);
    MetadataRevisionId(bytes)
}

fn hash_key(hasher: &mut Sha256, key: &MetadataKey) {
    hasher.update([key.kind().canonical_tag()]);
    hash_bytes(hasher, key.id().as_str().as_bytes());
}

fn hash_bytes(hasher: &mut Sha256, bytes: &[u8]) {
    hash_usize(hasher, bytes.len());
    hasher.update(bytes);
}

fn hash_usize(hasher: &mut Sha256, value: usize) {
    let value = u64::try_from(value).expect("metadata lengths must fit into u64");
    hasher.update(value.to_be_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(kind: MetadataKind, id: &str) -> MetadataKey {
        MetadataKey::new(kind, MetadataId::try_new(id).unwrap())
    }

    fn document(
        kind: MetadataKind,
        id: &str,
        content: &str,
        dependencies: Vec<MetadataKey>,
    ) -> MetadataDocument {
        MetadataDocument::new(
            key(kind, id),
            "1.0.0",
            content.as_bytes().to_vec(),
            dependencies,
        )
        .unwrap()
    }

    fn bundle(documents: Vec<MetadataDocument>) -> MetadataBundleDraft {
        MetadataBundleDraft::new(documents).unwrap()
    }

    #[test]
    fn revision_identity_is_deterministic_and_order_independent() {
        let object_key = key(MetadataKind::Object, "crm.sales.deal");
        let object = document(MetadataKind::Object, "crm.sales.deal", "object-v1", vec![]);
        let field = document(
            MetadataKind::Field,
            "crm.sales.deal.name",
            "field-v1",
            vec![object_key],
        );
        let first = bundle(vec![object.clone(), field.clone()]);
        let second = bundle(vec![field, object]);
        assert_eq!(first.revision_id(), second.revision_id());
        assert_eq!(first.revision_id().to_hex().len(), 64);
    }

    #[test]
    fn publication_is_content_addressed_immutable_and_idempotent() {
        let mut catalog = MetadataCatalog::new();
        let first = catalog.publish(
            bundle(vec![document(
                MetadataKind::Object,
                "crm.sales.deal",
                "object-v1",
                vec![],
            )]),
            100,
        );
        let duplicate = catalog.publish(
            bundle(vec![document(
                MetadataKind::Object,
                "crm.sales.deal",
                "object-v1",
                vec![],
            )]),
            200,
        );
        let changed = catalog.publish(
            bundle(vec![document(
                MetadataKind::Object,
                "crm.sales.deal",
                "object-v2",
                vec![],
            )]),
            300,
        );

        assert!(!first.already_published);
        assert!(duplicate.already_published);
        assert_eq!(first.revision_id, duplicate.revision_id);
        assert_ne!(first.revision_id, changed.revision_id);
        assert_eq!(
            catalog
                .revision(&first.revision_id)
                .unwrap()
                .published_at_unix_millis(),
            100
        );
    }

    #[test]
    fn invalid_bundle_dependencies_are_rejected() {
        let object_key = key(MetadataKind::Object, "crm.sales.deal");
        let duplicate = MetadataBundleDraft::new(vec![
            document(MetadataKind::Object, "crm.sales.deal", "one", vec![]),
            document(MetadataKind::Object, "crm.sales.deal", "two", vec![]),
        ])
        .unwrap_err();
        assert_eq!(duplicate.code, MetadataErrorCode::DuplicateDocument);

        let self_dependency = MetadataDocument::new(
            object_key.clone(),
            "1.0.0",
            b"object".to_vec(),
            vec![object_key.clone()],
        )
        .unwrap_err();
        assert_eq!(self_dependency.code, MetadataErrorCode::SelfDependency);

        let dangling = MetadataBundleDraft::new(vec![document(
            MetadataKind::Field,
            "crm.sales.deal.name",
            "field",
            vec![object_key],
        )])
        .unwrap_err();
        assert_eq!(dangling.code, MetadataErrorCode::MissingDependency);
    }

    #[test]
    fn impact_analysis_and_activation_require_explicit_breaking_confirmation() {
        let tenant = TenantId::try_new("tenant-a").unwrap();
        let mut catalog = MetadataCatalog::new();
        let original = catalog.publish(
            bundle(vec![
                document(MetadataKind::Object, "crm.sales.deal", "object-v1", vec![]),
                document(
                    MetadataKind::Field,
                    "crm.sales.deal.name",
                    "field-v1",
                    vec![],
                ),
            ]),
            100,
        );
        let candidate = catalog.publish(
            bundle(vec![
                document(MetadataKind::Object, "crm.sales.deal", "object-v2", vec![]),
                document(
                    MetadataKind::Layout,
                    "crm.sales.deal.default",
                    "layout-v1",
                    vec![],
                ),
            ]),
            200,
        );

        let first_activation = catalog
            .activate(tenant.clone(), &original.revision_id, 0, false)
            .unwrap();
        assert_eq!(first_activation.generation, 1);

        let report = catalog.impact_for(&tenant, &candidate.revision_id).unwrap();
        assert!(report.has_breaking_changes());
        assert!(report.requires_review());
        assert!(report.changes.iter().any(|change| {
            change.key == key(MetadataKind::Object, "crm.sales.deal")
                && change.change_type == MetadataChangeType::Modified
        }));
        assert!(report.changes.iter().any(|change| {
            change.key == key(MetadataKind::Layout, "crm.sales.deal.default")
                && change.change_type == MetadataChangeType::Added
        }));
        assert!(report.changes.iter().any(|change| {
            change.key == key(MetadataKind::Field, "crm.sales.deal.name")
                && change.change_type == MetadataChangeType::Removed
        }));

        let confirmation_error = catalog
            .activate(tenant.clone(), &candidate.revision_id, 1, false)
            .unwrap_err();
        assert_eq!(
            confirmation_error.code,
            MetadataErrorCode::BreakingChangeConfirmationRequired
        );
        let stale_error = catalog
            .activate(tenant.clone(), &candidate.revision_id, 0, true)
            .unwrap_err();
        assert_eq!(stale_error.code, MetadataErrorCode::GenerationConflict);

        let activation = catalog
            .activate(tenant, &candidate.revision_id, 1, true)
            .unwrap();
        assert_eq!(activation.generation, 2);
    }

    #[test]
    fn rollback_restores_previous_revision_without_mutating_publications() {
        let tenant = TenantId::try_new("tenant-a").unwrap();
        let mut catalog = MetadataCatalog::new();
        let first = catalog.publish(
            bundle(vec![document(
                MetadataKind::Object,
                "crm.sales.deal",
                "object-v1",
                vec![],
            )]),
            100,
        );
        let second = catalog.publish(
            bundle(vec![document(
                MetadataKind::Object,
                "crm.sales.deal",
                "object-v2",
                vec![],
            )]),
            200,
        );
        catalog
            .activate(tenant.clone(), &first.revision_id, 0, false)
            .unwrap();
        catalog
            .activate(tenant.clone(), &second.revision_id, 1, false)
            .unwrap();

        let rollback = catalog.rollback(&tenant, 2).unwrap();
        assert_eq!(rollback.generation, 3);
        assert_eq!(rollback.active_revision, first.revision_id);
        assert_eq!(rollback.replaced_revision, second.revision_id);
        assert_eq!(
            catalog.tenant_state(&tenant).active_revision,
            Some(first.revision_id)
        );
        assert!(catalog.revision(&second.revision_id).is_ok());
    }

    #[test]
    fn tenant_activation_state_is_isolated() {
        let tenant_a = TenantId::try_new("tenant-a").unwrap();
        let tenant_b = TenantId::try_new("tenant-b").unwrap();
        let mut catalog = MetadataCatalog::new();
        let revision = catalog.publish(
            bundle(vec![document(
                MetadataKind::Object,
                "crm.sales.deal",
                "object-v1",
                vec![],
            )]),
            100,
        );

        catalog
            .activate(tenant_a.clone(), &revision.revision_id, 0, false)
            .unwrap();
        assert_eq!(catalog.tenant_state(&tenant_a).generation, 1);
        assert_eq!(catalog.tenant_state(&tenant_b).generation, 0);
        assert_eq!(catalog.tenant_state(&tenant_b).active_revision, None);
    }
}
