use crate::canonicalization::persisted_state_json as access_export_state_json;
use crm_module_sdk::{FileId as ExportFileId, IdempotencyKey as ExportIdempotencyKey};

pub const ACCESS_EXPORT_REQUEST_COORDINATE: &str =
    "customer_privacy.access_export.request@1.0.0";
pub const CUSTOMER_DATA_PRIVACY_EXPORT_COORDINATE: &str =
    "customer_data.export.privacy.request@1.0.0";
pub const ACCESS_EXPORT_STATE_SCHEMA_ID: &str =
    "crm.customer-privacy.access_export_reference.state";
pub const ACCESS_EXPORT_STATE_SCHEMA_VERSION: &str = "1.0.0";
pub const ACCESS_EXPORT_STATE_MAXIMUM_BYTES: u64 = 2 * 1024 * 1024;
pub const ACCESS_EXPORT_STATE_RETENTION_POLICY_ID: &str =
    "crm.customer_privacy.access_export_reference";
pub const ACCESS_EXPORT_MANIFEST_MEDIA_TYPE: &str =
    "application/vnd.crm.customer-privacy-access-export+json;version=1";

const MANIFEST_DESCRIPTOR: &[u8] = b"crm.customer-privacy.access_export_manifest/v1:manifest_id,tenant_id,privacy_case_id,scope_snapshot_id,action_plan_id,action_plan_digest,canonical_party_id,identity_resolution_generation,case_kind,items,manifest_digest";
const REFERENCE_DESCRIPTOR: &[u8] = b"crm.customer-privacy.access_export_reference.state/v1:reference_id,manifest,customer_data_coordinate,export_job_id,target_idempotency_key,status,artifact,prepared_at_unix_nanos,completed_at_unix_nanos,reference_digest";
const MANIFEST_ID_PREFIX: &str = "privacy-access-export-manifest-";
const REFERENCE_ID_PREFIX: &str = "privacy-access-export-reference-";
const EXPORT_JOB_ID_PREFIX: &str = "privacy-export-job-";
const TARGET_IDEMPOTENCY_PREFIX: &str = "privacy-export-target-";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrivacyAccessExportManifestItem {
    sequence: u32,
    owner_module_id: ModuleId,
    resource_type: String,
    resource_id: RecordId,
    resource_version: u64,
    data_class: DataClass,
    evidence_class: EvidenceClass,
    retention_policy_id: RetentionPolicyId,
    plan_item_digest: [u8; 32],
}

impl PrivacyAccessExportManifestItem {
    pub const fn sequence(&self) -> u32 { self.sequence }
    pub fn owner_module_id(&self) -> &ModuleId { &self.owner_module_id }
    pub fn resource_type(&self) -> &str { &self.resource_type }
    pub fn resource_id(&self) -> &RecordId { &self.resource_id }
    pub const fn resource_version(&self) -> u64 { self.resource_version }
    pub const fn data_class(&self) -> DataClass { self.data_class }
    pub const fn evidence_class(&self) -> EvidenceClass { self.evidence_class }
    pub fn retention_policy_id(&self) -> &RetentionPolicyId { &self.retention_policy_id }
    pub const fn plan_item_digest(&self) -> &[u8; 32] { &self.plan_item_digest }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrivacyAccessExportManifest {
    manifest_id: RecordId,
    tenant_id: TenantId,
    privacy_case_id: RecordId,
    scope_snapshot_id: RecordId,
    action_plan_id: RecordId,
    action_plan_digest: [u8; 32],
    canonical_party_id: RecordId,
    identity_resolution_generation: u64,
    case_kind: PrivacyCaseKind,
    items: Vec<PrivacyAccessExportManifestItem>,
    digest: [u8; 32],
}

impl PrivacyAccessExportManifest {
    pub fn build(plan: &PrivacyActionPlan) -> Result<Self, SdkError> {
        let lineage = plan.lineage();
        if !matches!(lineage.case_kind(), PrivacyCaseKind::Access | PrivacyCaseKind::PortabilityExport) {
            return Err(access_export_conflict("only Access or PortabilityExport plans can be assembled"));
        }
        let expected_reason = match lineage.case_kind() {
            PrivacyCaseKind::Access => PrivacyPlanReason::AccessDisclosureOnly,
            PrivacyCaseKind::PortabilityExport => PrivacyPlanReason::PortabilityDisclosureOnly,
            _ => unreachable!(),
        };
        if plan.items().len() > ACTION_PLAN_MAXIMUM_ITEMS {
            return Err(access_export_invalid("access export manifest exceeds the bounded item count"));
        }
        let mut items = Vec::with_capacity(plan.items().len());
        for item in plan.items() {
            if item.action() != PlannedPrivacyAction::Retain || item.reason() != expected_reason {
                return Err(access_export_conflict("access export plan contains a non-disclosure item"));
            }
            items.push(PrivacyAccessExportManifestItem {
                sequence: item.sequence(),
                owner_module_id: item.owner_module_id().clone(),
                resource_type: item.resource_type().to_owned(),
                resource_id: item.resource_id().clone(),
                resource_version: item.resource_version(),
                data_class: item.data_class(),
                evidence_class: item.evidence_class(),
                retention_policy_id: item.retention_policy_id().clone(),
                plan_item_digest: *item.digest(),
            });
        }
        let digest = manifest_digest(
            lineage.tenant_id(),
            lineage.privacy_case_id(),
            lineage.scope_snapshot_id(),
            plan.plan_id(),
            plan.digest(),
            lineage.canonical_party_id(),
            lineage.identity_resolution_generation(),
            lineage.case_kind(),
            &items,
        );
        let manifest_id = RecordId::try_new(format!("{MANIFEST_ID_PREFIX}{}", hex(&digest)))
            .map_err(access_export_invalid)?;
        let value = Self {
            manifest_id,
            tenant_id: lineage.tenant_id().clone(),
            privacy_case_id: lineage.privacy_case_id().clone(),
            scope_snapshot_id: lineage.scope_snapshot_id().clone(),
            action_plan_id: plan.plan_id().clone(),
            action_plan_digest: *plan.digest(),
            canonical_party_id: lineage.canonical_party_id().clone(),
            identity_resolution_generation: lineage.identity_resolution_generation(),
            case_kind: lineage.case_kind(),
            items,
            digest,
        };
        value.validate()?;
        Ok(value)
    }

    pub fn validate(&self) -> Result<(), SdkError> {
        if self.identity_resolution_generation == 0 || self.items.len() > ACTION_PLAN_MAXIMUM_ITEMS {
            return Err(access_export_invalid("access export manifest lineage is invalid"));
        }
        if !matches!(self.case_kind, PrivacyCaseKind::Access | PrivacyCaseKind::PortabilityExport) {
            return Err(access_export_conflict("access export manifest case kind is unsupported"));
        }
        for (index, item) in self.items.iter().enumerate() {
            let expected = u32::try_from(index + 1).map_err(access_export_invalid)?;
            if item.sequence != expected || item.resource_version == 0 || item.resource_type.is_empty()
                || item.plan_item_digest.iter().all(|byte| *byte == 0)
            {
                return Err(access_export_invalid("access export manifest item is invalid"));
            }
        }
        let expected = manifest_digest(
            &self.tenant_id,
            &self.privacy_case_id,
            &self.scope_snapshot_id,
            &self.action_plan_id,
            &self.action_plan_digest,
            &self.canonical_party_id,
            self.identity_resolution_generation,
            self.case_kind,
            &self.items,
        );
        if self.digest != expected
            || self.manifest_id.as_str() != format!("{MANIFEST_ID_PREFIX}{}", hex(&expected))
        {
            return Err(access_export_invalid("access export manifest digest is invalid"));
        }
        Ok(())
    }

    pub fn manifest_id(&self) -> &RecordId { &self.manifest_id }
    pub fn tenant_id(&self) -> &TenantId { &self.tenant_id }
    pub fn privacy_case_id(&self) -> &RecordId { &self.privacy_case_id }
    pub fn scope_snapshot_id(&self) -> &RecordId { &self.scope_snapshot_id }
    pub fn action_plan_id(&self) -> &RecordId { &self.action_plan_id }
    pub const fn action_plan_digest(&self) -> &[u8; 32] { &self.action_plan_digest }
    pub fn canonical_party_id(&self) -> &RecordId { &self.canonical_party_id }
    pub const fn identity_resolution_generation(&self) -> u64 { self.identity_resolution_generation }
    pub const fn case_kind(&self) -> PrivacyCaseKind { self.case_kind }
    pub fn items(&self) -> &[PrivacyAccessExportManifestItem] { &self.items }
    pub const fn digest(&self) -> &[u8; 32] { &self.digest }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrivacyAccessExportStatus {
    Prepared,
    Completed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrivacyAccessExportArtifact {
    file_id: ExportFileId,
    media_type: String,
    content_sha256: [u8; 32],
    size_bytes: u64,
    retention_policy_id: RetentionPolicyId,
}

impl PrivacyAccessExportArtifact {
    pub fn file_id(&self) -> &ExportFileId { &self.file_id }
    pub fn media_type(&self) -> &str { &self.media_type }
    pub const fn content_sha256(&self) -> &[u8; 32] { &self.content_sha256 }
    pub const fn size_bytes(&self) -> u64 { self.size_bytes }
    pub fn retention_policy_id(&self) -> &RetentionPolicyId { &self.retention_policy_id }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrivacyAccessExportReference {
    reference_id: RecordId,
    manifest: PrivacyAccessExportManifest,
    customer_data_coordinate: String,
    export_job_id: RecordId,
    target_idempotency_key: ExportIdempotencyKey,
    status: PrivacyAccessExportStatus,
    artifact: Option<PrivacyAccessExportArtifact>,
    prepared_at_unix_nanos: i64,
    completed_at_unix_nanos: Option<i64>,
    digest: [u8; 32],
}

impl PrivacyAccessExportReference {
    pub fn prepare(
        manifest: PrivacyAccessExportManifest,
        prepared_at_unix_nanos: i64,
    ) -> Result<Self, SdkError> {
        manifest.validate()?;
        if prepared_at_unix_nanos <= 0 {
            return Err(access_export_invalid("access export prepare time must be positive"));
        }
        let reference_id = RecordId::try_new(format!(
            "{REFERENCE_ID_PREFIX}{}",
            hex(manifest.digest())
        ))
        .map_err(access_export_invalid)?;
        let export_job_id = RecordId::try_new(format!(
            "{EXPORT_JOB_ID_PREFIX}{}",
            hex(manifest.digest())
        ))
        .map_err(access_export_invalid)?;
        let target_idempotency_key = ExportIdempotencyKey::try_new(format!(
            "{TARGET_IDEMPOTENCY_PREFIX}{}",
            hex(manifest.digest())
        ))
        .map_err(access_export_invalid)?;
        let mut value = Self {
            reference_id,
            manifest,
            customer_data_coordinate: CUSTOMER_DATA_PRIVACY_EXPORT_COORDINATE.to_owned(),
            export_job_id,
            target_idempotency_key,
            status: PrivacyAccessExportStatus::Prepared,
            artifact: None,
            prepared_at_unix_nanos,
            completed_at_unix_nanos: None,
            digest: [0; 32],
        };
        value.digest = reference_digest(&value);
        value.validate()?;
        Ok(value)
    }

    pub fn complete(
        &mut self,
        export_job_id: &RecordId,
        file_id: ExportFileId,
        media_type: String,
        content_sha256: [u8; 32],
        size_bytes: u64,
        retention_policy_id: RetentionPolicyId,
        completed_at_unix_nanos: i64,
    ) -> Result<(), SdkError> {
        if export_job_id != &self.export_job_id {
            return Err(access_export_conflict("Customer Data Operations export job identity changed"));
        }
        if completed_at_unix_nanos < self.prepared_at_unix_nanos
            || media_type != ACCESS_EXPORT_MANIFEST_MEDIA_TYPE
            || content_sha256.iter().all(|byte| *byte == 0)
        {
            return Err(access_export_invalid("Customer Data Operations artifact evidence is invalid"));
        }
        let artifact = PrivacyAccessExportArtifact {
            file_id,
            media_type,
            content_sha256,
            size_bytes,
            retention_policy_id,
        };
        if let Some(existing) = &self.artifact {
            if existing != &artifact || self.completed_at_unix_nanos != Some(completed_at_unix_nanos) {
                return Err(access_export_conflict("completed access export replay conflicts with immutable evidence"));
            }
            return Ok(());
        }
        self.status = PrivacyAccessExportStatus::Completed;
        self.artifact = Some(artifact);
        self.completed_at_unix_nanos = Some(completed_at_unix_nanos);
        self.digest = reference_digest(self);
        self.validate()
    }

    pub fn validate(&self) -> Result<(), SdkError> {
        self.manifest.validate()?;
        if self.customer_data_coordinate != CUSTOMER_DATA_PRIVACY_EXPORT_COORDINATE
            || self.prepared_at_unix_nanos <= 0
            || self.reference_id.as_str()
                != format!("{REFERENCE_ID_PREFIX}{}", hex(self.manifest.digest()))
            || self.export_job_id.as_str()
                != format!("{EXPORT_JOB_ID_PREFIX}{}", hex(self.manifest.digest()))
            || self.target_idempotency_key.as_str()
                != format!("{TARGET_IDEMPOTENCY_PREFIX}{}", hex(self.manifest.digest()))
        {
            return Err(access_export_invalid("access export reference lineage is invalid"));
        }
        match (self.status, &self.artifact, self.completed_at_unix_nanos) {
            (PrivacyAccessExportStatus::Prepared, None, None) => {}
            (PrivacyAccessExportStatus::Completed, Some(artifact), Some(completed_at))
                if completed_at >= self.prepared_at_unix_nanos
                    && artifact.media_type == ACCESS_EXPORT_MANIFEST_MEDIA_TYPE
                    && !artifact.content_sha256.iter().all(|byte| *byte == 0) => {}
            _ => return Err(access_export_invalid("access export reference state is inconsistent")),
        }
        if self.digest != reference_digest(self) {
            return Err(access_export_invalid("access export reference digest is invalid"));
        }
        Ok(())
    }

    pub fn reference_id(&self) -> &RecordId { &self.reference_id }
    pub fn manifest(&self) -> &PrivacyAccessExportManifest { &self.manifest }
    pub fn customer_data_coordinate(&self) -> &str { &self.customer_data_coordinate }
    pub fn export_job_id(&self) -> &RecordId { &self.export_job_id }
    pub fn target_idempotency_key(&self) -> &ExportIdempotencyKey { &self.target_idempotency_key }
    pub const fn status(&self) -> PrivacyAccessExportStatus { self.status }
    pub fn artifact(&self) -> Option<&PrivacyAccessExportArtifact> { self.artifact.as_ref() }
    pub const fn prepared_at_unix_nanos(&self) -> i64 { self.prepared_at_unix_nanos }
    pub const fn completed_at_unix_nanos(&self) -> Option<i64> { self.completed_at_unix_nanos }
    pub const fn digest(&self) -> &[u8; 32] { &self.digest }
}

pub fn access_export_state_descriptor_hash() -> [u8; 32] {
    sha2::Sha256::digest(REFERENCE_DESCRIPTOR).into()
}

pub fn access_export_manifest_descriptor_hash() -> [u8; 32] {
    sha2::Sha256::digest(MANIFEST_DESCRIPTOR).into()
}

pub fn encode_access_export_manifest(
    manifest: &PrivacyAccessExportManifest,
) -> Result<Vec<u8>, SdkError> {
    manifest.validate()?;
    let bytes = access_export_state_json::to_vec(manifest).map_err(access_export_invalid)?;
    validate_size(&bytes)?;
    Ok(bytes)
}

pub fn decode_access_export_manifest(bytes: &[u8]) -> Result<PrivacyAccessExportManifest, SdkError> {
    validate_size(bytes)?;
    let manifest: PrivacyAccessExportManifest =
        access_export_state_json::from_slice(bytes).map_err(access_export_invalid)?;
    manifest.validate()?;
    if encode_access_export_manifest(&manifest)? != bytes {
        return Err(access_export_invalid("access export manifest is not strict canonical v1"));
    }
    Ok(manifest)
}

pub fn encode_access_export_reference(
    reference: &PrivacyAccessExportReference,
) -> Result<Vec<u8>, SdkError> {
    reference.validate()?;
    let bytes = access_export_state_json::to_vec(reference).map_err(access_export_invalid)?;
    validate_size(&bytes)?;
    Ok(bytes)
}

pub fn decode_access_export_reference(bytes: &[u8]) -> Result<PrivacyAccessExportReference, SdkError> {
    validate_size(bytes)?;
    let reference: PrivacyAccessExportReference =
        access_export_state_json::from_slice(bytes).map_err(access_export_invalid)?;
    reference.validate()?;
    if encode_access_export_reference(&reference)? != bytes {
        return Err(access_export_invalid("access export reference is not strict canonical v1"));
    }
    Ok(reference)
}

fn manifest_digest(
    tenant_id: &TenantId,
    privacy_case_id: &RecordId,
    scope_snapshot_id: &RecordId,
    action_plan_id: &RecordId,
    action_plan_digest: &[u8; 32],
    canonical_party_id: &RecordId,
    identity_resolution_generation: u64,
    case_kind: PrivacyCaseKind,
    items: &[PrivacyAccessExportManifestItem],
) -> [u8; 32] {
    let mut hasher = sha2::Sha256::new();
    hash_field(&mut hasher, MANIFEST_DESCRIPTOR);
    for value in [
        tenant_id.as_str(),
        privacy_case_id.as_str(),
        scope_snapshot_id.as_str(),
        action_plan_id.as_str(),
        canonical_party_id.as_str(),
    ] {
        hash_field(&mut hasher, value.as_bytes());
    }
    hash_field(&mut hasher, action_plan_digest);
    hash_field(&mut hasher, &identity_resolution_generation.to_be_bytes());
    hash_field(&mut hasher, case_kind_label(case_kind).as_bytes());
    hash_field(&mut hasher, &(items.len() as u64).to_be_bytes());
    for item in items {
        hash_field(&mut hasher, &item.sequence.to_be_bytes());
        hash_field(&mut hasher, item.owner_module_id.as_str().as_bytes());
        hash_field(&mut hasher, item.resource_type.as_bytes());
        hash_field(&mut hasher, item.resource_id.as_str().as_bytes());
        hash_field(&mut hasher, &item.resource_version.to_be_bytes());
        hash_field(&mut hasher, item.plan_item_digest.as_slice());
    }
    hasher.finalize().into()
}

fn reference_digest(reference: &PrivacyAccessExportReference) -> [u8; 32] {
    let mut hasher = sha2::Sha256::new();
    hash_field(&mut hasher, REFERENCE_DESCRIPTOR);
    hash_field(&mut hasher, reference.reference_id.as_str().as_bytes());
    hash_field(&mut hasher, reference.manifest.digest());
    hash_field(&mut hasher, reference.customer_data_coordinate.as_bytes());
    hash_field(&mut hasher, reference.export_job_id.as_str().as_bytes());
    hash_field(&mut hasher, reference.target_idempotency_key.as_str().as_bytes());
    hash_field(&mut hasher, status_label(reference.status).as_bytes());
    hash_field(&mut hasher, &reference.prepared_at_unix_nanos.to_be_bytes());
    if let Some(completed) = reference.completed_at_unix_nanos {
        hash_field(&mut hasher, &completed.to_be_bytes());
    }
    if let Some(artifact) = &reference.artifact {
        hash_field(&mut hasher, artifact.file_id.as_str().as_bytes());
        hash_field(&mut hasher, artifact.media_type.as_bytes());
        hash_field(&mut hasher, &artifact.content_sha256);
        hash_field(&mut hasher, &artifact.size_bytes.to_be_bytes());
        hash_field(&mut hasher, artifact.retention_policy_id.as_str().as_bytes());
    }
    hasher.finalize().into()
}

fn case_kind_label(kind: PrivacyCaseKind) -> &'static str {
    match kind {
        PrivacyCaseKind::Access => "access",
        PrivacyCaseKind::PortabilityExport => "portability_export",
        PrivacyCaseKind::RestrictProcessing => "restrict_processing",
        PrivacyCaseKind::Erasure => "erasure",
    }
}

fn status_label(status: PrivacyAccessExportStatus) -> &'static str {
    match status {
        PrivacyAccessExportStatus::Prepared => "prepared",
        PrivacyAccessExportStatus::Completed => "completed",
    }
}

fn hash_field(hasher: &mut sha2::Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_be_bytes());
    hasher.update(bytes);
}

fn hex(bytes: &[u8; 32]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn validate_size(bytes: &[u8]) -> Result<(), SdkError> {
    if bytes.is_empty() || bytes.len() as u64 > ACCESS_EXPORT_STATE_MAXIMUM_BYTES {
        return Err(access_export_invalid("access export state exceeds the bounded size"));
    }
    Ok(())
}

fn access_export_invalid(error: impl std::fmt::Display) -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_ACCESS_EXPORT_EVIDENCE_INVALID",
        ErrorCategory::Internal,
        false,
        "Customer Privacy access export evidence is invalid.",
    )
    .with_internal_reference(error.to_string())
}

fn access_export_conflict(message: &'static str) -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_ACCESS_EXPORT_CONFLICT",
        ErrorCategory::Conflict,
        false,
        "Customer Privacy access export evidence conflicts with immutable lineage.",
    )
    .with_internal_reference(message)
}

#[cfg(test)]
mod access_export_tests {
    use super::*;

    #[test]
    fn frozen_coordinates_and_descriptor_are_non_empty() {
        assert_eq!(ACCESS_EXPORT_REQUEST_COORDINATE, "customer_privacy.access_export.request@1.0.0");
        assert_eq!(CUSTOMER_DATA_PRIVACY_EXPORT_COORDINATE, "customer_data.export.privacy.request@1.0.0");
        assert_ne!(access_export_state_descriptor_hash(), [0; 32]);
        assert_ne!(access_export_manifest_descriptor_hash(), [0; 32]);
    }
}
