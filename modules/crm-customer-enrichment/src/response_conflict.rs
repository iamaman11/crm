use crate::{EnrichmentRequestId, ProviderResponseReceiptId, ReplayDisposition};
use crm_module_sdk::{
    ActorId, CausationId, ErrorCategory, FieldName, FieldViolation, PortFuture, SdkError, TenantId,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const PROVIDER_RESPONSE_CONFLICT_STATE_SCHEMA_ID: &str =
    "crm.customer-enrichment.provider_response_conflict.state";
pub const PROVIDER_RESPONSE_CONFLICT_STATE_SCHEMA_VERSION: &str = "1.0.0";
pub const PROVIDER_RESPONSE_CONFLICT_STATE_MAXIMUM_BYTES: u64 = 32 * 1024;
pub const PROVIDER_RESPONSE_CONFLICT_STATE_RETENTION_POLICY_ID: &str =
    "crm.customer_enrichment.provenance";

const PROVIDER_RESPONSE_CONFLICT_ID_DOMAIN: &[u8] =
    b"crm.customer-enrichment.provider-response-conflict/v1";
const PROVIDER_RESPONSE_CONFLICT_STATE_DESCRIPTOR: &[u8] = b"crm.customer-enrichment.provider_response_conflict.state/v1:conflict_id,tenant_id,request_id,retry_generation,first_receipt_id,conflicting_semantic_fingerprint,detected_at_unix_ms,resolution";
const MAX_RETRY_GENERATION: u32 = 100;
const MAX_POLICY_VERSION_BYTES: usize = 80;
const MAX_SAFE_REASON_CODE_BYTES: usize = 80;
const MAX_EVIDENCE_REFERENCE_BYTES: usize = 240;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ProviderResponseConflictId(String);

impl ProviderResponseConflictId {
    fn from_digest(digest: &[u8]) -> Self {
        Self(format!("enrichment-response-conflict-{}", hex(digest)))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderResponseConflictDecision {
    RetainFirstReceipt,
    RejectRequest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderResponseConflictDraft {
    pub tenant_id: TenantId,
    pub request_id: EnrichmentRequestId,
    pub retry_generation: u32,
    pub first_receipt_id: ProviderResponseReceiptId,
    pub conflicting_semantic_fingerprint: [u8; 32],
    pub detected_at_unix_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderResponseConflictResolutionDraft {
    pub decision: ProviderResponseConflictDecision,
    pub resolved_by: ActorId,
    pub policy_version: String,
    pub safe_reason_code: String,
    pub approval_evidence_reference: String,
    pub causation_id: CausationId,
    pub resolved_at_unix_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderResponseConflictResolution {
    decision: ProviderResponseConflictDecision,
    resolved_by: ActorId,
    policy_version: String,
    safe_reason_code: String,
    approval_evidence_reference: String,
    causation_id: CausationId,
    resolved_at_unix_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderResponseConflict {
    conflict_id: ProviderResponseConflictId,
    tenant_id: TenantId,
    request_id: EnrichmentRequestId,
    retry_generation: u32,
    first_receipt_id: ProviderResponseReceiptId,
    conflicting_semantic_fingerprint: [u8; 32],
    detected_at_unix_ms: u64,
    resolution: Option<ProviderResponseConflictResolution>,
}

#[derive(Serialize)]
struct ProviderResponseConflictIdentity<'a> {
    semantic_version: &'static str,
    tenant_id: &'a TenantId,
    request_id: &'a EnrichmentRequestId,
    retry_generation: u32,
    first_receipt_id: &'a ProviderResponseReceiptId,
    conflicting_semantic_fingerprint: &'a [u8; 32],
}

impl ProviderResponseConflict {
    pub fn record(draft: ProviderResponseConflictDraft) -> Result<Self, SdkError> {
        validate_tenant(&draft.tenant_id)?;
        validate_derived_id(
            draft.request_id.as_str(),
            "enrichment-request-",
            "conflict.request_id",
        )?;
        validate_derived_id(
            draft.first_receipt_id.as_str(),
            "enrichment-response-",
            "conflict.first_receipt_id",
        )?;
        validate_retry_generation(draft.retry_generation)?;
        validate_fingerprint(&draft.conflicting_semantic_fingerprint)?;
        if draft.detected_at_unix_ms == 0 {
            return Err(invalid(
                "CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_CONFLICT_TIME_INVALID",
                "conflict.detected_at_unix_ms",
                "conflict detection time must be after the Unix epoch",
            ));
        }

        let identity = ProviderResponseConflictIdentity {
            semantic_version: "1.0.0",
            tenant_id: &draft.tenant_id,
            request_id: &draft.request_id,
            retry_generation: draft.retry_generation,
            first_receipt_id: &draft.first_receipt_id,
            conflicting_semantic_fingerprint: &draft.conflicting_semantic_fingerprint,
        };
        Ok(Self {
            conflict_id: ProviderResponseConflictId::from_digest(&canonical_digest(
                PROVIDER_RESPONSE_CONFLICT_ID_DOMAIN,
                &identity,
            )),
            tenant_id: draft.tenant_id,
            request_id: draft.request_id,
            retry_generation: draft.retry_generation,
            first_receipt_id: draft.first_receipt_id,
            conflicting_semantic_fingerprint: draft.conflicting_semantic_fingerprint,
            detected_at_unix_ms: draft.detected_at_unix_ms,
            resolution: None,
        })
    }

    pub fn resolve(
        &mut self,
        draft: ProviderResponseConflictResolutionDraft,
    ) -> Result<ReplayDisposition, SdkError> {
        let resolution =
            ProviderResponseConflictResolution::try_from_draft(draft, self.detected_at_unix_ms)?;
        match &self.resolution {
            Some(existing) if existing == &resolution => Ok(ReplayDisposition::Duplicate),
            Some(_) => Err(conflict(
                "CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_CONFLICT_ALREADY_RESOLVED",
                "the provider-response conflict already has a different immutable resolution",
            )),
            None => {
                self.resolution = Some(resolution);
                Ok(ReplayDisposition::New)
            }
        }
    }

    fn identity(&self) -> ProviderResponseConflictIdentity<'_> {
        ProviderResponseConflictIdentity {
            semantic_version: "1.0.0",
            tenant_id: &self.tenant_id,
            request_id: &self.request_id,
            retry_generation: self.retry_generation,
            first_receipt_id: &self.first_receipt_id,
            conflicting_semantic_fingerprint: &self.conflicting_semantic_fingerprint,
        }
    }

    fn validate_persisted(&self) -> Result<(), SdkError> {
        validate_tenant(&self.tenant_id).map_err(persisted_domain_error)?;
        validate_derived_id(
            self.request_id.as_str(),
            "enrichment-request-",
            "conflict.request_id",
        )
        .map_err(persisted_domain_error)?;
        validate_derived_id(
            self.first_receipt_id.as_str(),
            "enrichment-response-",
            "conflict.first_receipt_id",
        )
        .map_err(persisted_domain_error)?;
        validate_retry_generation(self.retry_generation).map_err(persisted_domain_error)?;
        validate_fingerprint(&self.conflicting_semantic_fingerprint)
            .map_err(persisted_domain_error)?;
        if self.detected_at_unix_ms == 0 {
            return Err(persisted_error(
                "persisted provider-response conflict has an invalid detection time",
            ));
        }
        if let Some(resolution) = &self.resolution {
            resolution
                .validate(self.detected_at_unix_ms)
                .map_err(persisted_domain_error)?;
        }
        let expected = ProviderResponseConflictId::from_digest(&canonical_digest(
            PROVIDER_RESPONSE_CONFLICT_ID_DOMAIN,
            &self.identity(),
        ));
        if self.conflict_id != expected {
            return Err(persisted_error(
                "persisted provider-response conflict identity does not match canonical content",
            ));
        }
        Ok(())
    }

    pub fn conflict_id(&self) -> &ProviderResponseConflictId {
        &self.conflict_id
    }

    pub fn tenant_id(&self) -> &TenantId {
        &self.tenant_id
    }

    pub fn request_id(&self) -> &EnrichmentRequestId {
        &self.request_id
    }

    pub const fn retry_generation(&self) -> u32 {
        self.retry_generation
    }

    pub fn first_receipt_id(&self) -> &ProviderResponseReceiptId {
        &self.first_receipt_id
    }

    pub fn conflicting_semantic_fingerprint(&self) -> &[u8; 32] {
        &self.conflicting_semantic_fingerprint
    }

    pub const fn detected_at_unix_ms(&self) -> u64 {
        self.detected_at_unix_ms
    }

    pub fn resolution(&self) -> Option<&ProviderResponseConflictResolution> {
        self.resolution.as_ref()
    }
}

impl ProviderResponseConflictResolution {
    fn try_from_draft(
        draft: ProviderResponseConflictResolutionDraft,
        detected_at_unix_ms: u64,
    ) -> Result<Self, SdkError> {
        let resolution = Self {
            decision: draft.decision,
            resolved_by: draft.resolved_by,
            policy_version: canonical_key(
                draft.policy_version,
                MAX_POLICY_VERSION_BYTES,
                "conflict_resolution.policy_version",
            )?,
            safe_reason_code: canonical_key(
                draft.safe_reason_code,
                MAX_SAFE_REASON_CODE_BYTES,
                "conflict_resolution.safe_reason_code",
            )?,
            approval_evidence_reference: bounded_identifier(
                draft.approval_evidence_reference,
                MAX_EVIDENCE_REFERENCE_BYTES,
                "conflict_resolution.approval_evidence_reference",
            )?,
            causation_id: draft.causation_id,
            resolved_at_unix_ms: draft.resolved_at_unix_ms,
        };
        resolution.validate(detected_at_unix_ms)?;
        Ok(resolution)
    }

    fn validate(&self, detected_at_unix_ms: u64) -> Result<(), SdkError> {
        validate_actor(&self.resolved_by)?;
        validate_causation(&self.causation_id)?;
        canonical_key(
            self.policy_version.clone(),
            MAX_POLICY_VERSION_BYTES,
            "conflict_resolution.policy_version",
        )?;
        canonical_key(
            self.safe_reason_code.clone(),
            MAX_SAFE_REASON_CODE_BYTES,
            "conflict_resolution.safe_reason_code",
        )?;
        bounded_identifier(
            self.approval_evidence_reference.clone(),
            MAX_EVIDENCE_REFERENCE_BYTES,
            "conflict_resolution.approval_evidence_reference",
        )?;
        if self.resolved_at_unix_ms < detected_at_unix_ms {
            return Err(invalid(
                "CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_CONFLICT_TIME_INVALID",
                "conflict_resolution.resolved_at_unix_ms",
                "conflict resolution time must not precede conflict detection",
            ));
        }
        Ok(())
    }

    pub const fn decision(&self) -> ProviderResponseConflictDecision {
        self.decision
    }

    pub fn resolved_by(&self) -> &ActorId {
        &self.resolved_by
    }

    pub fn policy_version(&self) -> &str {
        &self.policy_version
    }

    pub fn safe_reason_code(&self) -> &str {
        &self.safe_reason_code
    }

    pub fn approval_evidence_reference(&self) -> &str {
        &self.approval_evidence_reference
    }

    pub fn causation_id(&self) -> &CausationId {
        &self.causation_id
    }

    pub const fn resolved_at_unix_ms(&self) -> u64 {
        self.resolved_at_unix_ms
    }
}

pub fn provider_response_conflict_state_descriptor_hash() -> [u8; 32] {
    Sha256::digest(PROVIDER_RESPONSE_CONFLICT_STATE_DESCRIPTOR).into()
}

pub fn encode_provider_response_conflict_state(
    value: &ProviderResponseConflict,
) -> Result<Vec<u8>, SdkError> {
    let bytes = serde_json::to_vec(value).map_err(|error| {
        persisted_error(format!(
            "provider-response conflict serialization failed: {error}"
        ))
    })?;
    validate_size(&bytes)?;
    Ok(bytes)
}

pub fn decode_provider_response_conflict_state(
    bytes: &[u8],
) -> Result<ProviderResponseConflict, SdkError> {
    validate_size(bytes)?;
    let value: ProviderResponseConflict = serde_json::from_slice(bytes).map_err(|error| {
        persisted_error(format!(
            "provider-response conflict JSON is invalid: {error}"
        ))
    })?;
    value.validate_persisted()?;
    if encode_provider_response_conflict_state(&value)? != bytes {
        return Err(persisted_error(
            "persisted provider-response conflict is not the strict canonical v1 encoding",
        ));
    }
    Ok(value)
}

fn validate_tenant(value: &TenantId) -> Result<(), SdkError> {
    TenantId::try_new(value.as_str().to_owned())
        .map(|_| ())
        .map_err(|error| invalid_identifier("conflict.tenant_id", error.to_string()))
}

fn validate_actor(value: &ActorId) -> Result<(), SdkError> {
    ActorId::try_new(value.as_str().to_owned())
        .map(|_| ())
        .map_err(|error| invalid_identifier("conflict_resolution.resolved_by", error.to_string()))
}

fn validate_causation(value: &CausationId) -> Result<(), SdkError> {
    CausationId::try_new(value.as_str().to_owned())
        .map(|_| ())
        .map_err(|error| invalid_identifier("conflict_resolution.causation_id", error.to_string()))
}

fn validate_retry_generation(value: u32) -> Result<(), SdkError> {
    if value > MAX_RETRY_GENERATION {
        return Err(invalid(
            "CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_CONFLICT_GENERATION_INVALID",
            "conflict.retry_generation",
            "conflict retry generation exceeds the governed request limit",
        ));
    }
    Ok(())
}

fn validate_fingerprint(value: &[u8; 32]) -> Result<(), SdkError> {
    if value.iter().all(|byte| *byte == 0) {
        return Err(invalid(
            "CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_CONFLICT_FINGERPRINT_INVALID",
            "conflict.conflicting_semantic_fingerprint",
            "conflicting semantic fingerprint must not be all zeroes",
        ));
    }
    Ok(())
}

fn validate_derived_id(
    value: &str,
    prefix: &'static str,
    field: &'static str,
) -> Result<(), SdkError> {
    let suffix = value
        .strip_prefix(prefix)
        .ok_or_else(|| invalid_identifier(field, format!("identifier must start with {prefix}")))?;
    if suffix.len() != 64
        || !suffix
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return Err(invalid_identifier(
            field,
            "identifier must end with exactly 64 lowercase hexadecimal characters",
        ));
    }
    Ok(())
}

fn canonical_key(
    value: String,
    maximum_bytes: usize,
    field: &'static str,
) -> Result<String, SdkError> {
    let valid = !value.is_empty()
        && value.len() <= maximum_bytes
        && value.is_ascii()
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-')
        })
        && value
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphanumeric)
        && value
            .as_bytes()
            .last()
            .is_some_and(u8::is_ascii_alphanumeric);
    if !valid {
        return Err(invalid(
            "CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_CONFLICT_CODE_INVALID",
            field,
            "conflict policy and reason codes must be bounded lowercase ASCII canonical keys",
        ));
    }
    Ok(value)
}

fn bounded_identifier(
    value: String,
    maximum_bytes: usize,
    field: &'static str,
) -> Result<String, SdkError> {
    if value.is_empty()
        || value.len() > maximum_bytes
        || value.trim() != value
        || value.chars().any(char::is_control)
    {
        return Err(invalid(
            "CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_CONFLICT_EVIDENCE_INVALID",
            field,
            "conflict evidence references must be bounded canonical text",
        ));
    }
    Ok(value)
}

fn canonical_digest<T: Serialize>(domain: &[u8], value: &T) -> Vec<u8> {
    let encoded = serde_json::to_vec(value)
        .expect("canonical provider-response conflict identity must serialize");
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update((encoded.len() as u64).to_be_bytes());
    hasher.update(encoded);
    hasher.finalize().to_vec()
}

fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 0x0f) as usize] as char);
    }
    encoded
}

fn validate_size(bytes: &[u8]) -> Result<(), SdkError> {
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX)
        > PROVIDER_RESPONSE_CONFLICT_STATE_MAXIMUM_BYTES
    {
        return Err(persisted_error(format!(
            "provider-response conflict state exceeds the maximum of {PROVIDER_RESPONSE_CONFLICT_STATE_MAXIMUM_BYTES} bytes"
        )));
    }
    Ok(())
}

fn invalid_identifier(field: &'static str, safe_message: impl Into<String>) -> SdkError {
    invalid(
        "CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_CONFLICT_IDENTIFIER_INVALID",
        field,
        safe_message,
    )
}

fn invalid(code: &'static str, field: &'static str, safe_message: impl Into<String>) -> SdkError {
    let mut error = SdkError::new(
        code,
        ErrorCategory::InvalidArgument,
        false,
        "The provider-response conflict evidence is invalid.",
    );
    error.field_violations.push(FieldViolation {
        field: FieldName::try_new(field)
            .expect("static provider-response conflict field path must be valid"),
        code: code.to_owned(),
        safe_message: safe_message.into(),
    });
    error
}

fn conflict(code: &'static str, reference: impl Into<String>) -> SdkError {
    SdkError::new(
        code,
        ErrorCategory::Conflict,
        false,
        "The provider-response conflict resolution conflicts with immutable evidence.",
    )
    .with_internal_reference(reference.into())
}

fn persisted_domain_error(error: SdkError) -> SdkError {
    persisted_error(format!(
        "provider-response conflict failed domain validation: {}: {}",
        error.code, error.safe_message
    ))
}

fn persisted_error(message: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_PERSISTED_STATE_INVALID",
        ErrorCategory::Internal,
        false,
        "The persisted customer-enrichment state is invalid.",
    )
    .with_internal_reference(message.into())
}

/// Exact immutable conflict binding evaluated immediately before an operator resolution write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderResponseConflictResolutionPolicyRequest {
    pub tenant_id: TenantId,
    pub actor_id: ActorId,
    pub conflict_id: ProviderResponseConflictId,
    pub request_id: EnrichmentRequestId,
    pub retry_generation: u32,
    pub first_receipt_id: ProviderResponseReceiptId,
    pub decision: ProviderResponseConflictDecision,
    pub safe_reason_code: String,
    pub approval_evidence_reference: String,
    pub evaluated_at_unix_ms: u64,
}

/// Closed, versioned live authorization outcome for one exact conflict resolution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderResponseConflictResolutionPolicyDecision {
    Allowed {
        policy_version: String,
    },
    Denied {
        policy_version: String,
        safe_reason_code: String,
    },
}

/// Infrastructure-owned final authorization boundary for provider-response conflict resolution.
///
/// Implementations must evaluate the exact immutable conflict, operator, decision and approval
/// evidence. The caller must invoke this port after loading current state and immediately before
/// the atomic resolution write.
pub trait ProviderResponseConflictResolutionPolicyPort: Send + Sync {
    fn evaluate<'a>(
        &'a self,
        request: ProviderResponseConflictResolutionPolicyRequest,
    ) -> PortFuture<'a, Result<ProviderResponseConflictResolutionPolicyDecision, SdkError>>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn request_id(byte: u8) -> EnrichmentRequestId {
        serde_json::from_str(&format!(
            "\"enrichment-request-{}\"",
            format!("{byte:02x}").repeat(32)
        ))
        .unwrap()
    }

    fn receipt_id(byte: u8) -> ProviderResponseReceiptId {
        serde_json::from_str(&format!(
            "\"enrichment-response-{}\"",
            format!("{byte:02x}").repeat(32)
        ))
        .unwrap()
    }

    fn draft() -> ProviderResponseConflictDraft {
        ProviderResponseConflictDraft {
            tenant_id: TenantId::try_new("tenant-a").unwrap(),
            request_id: request_id(1),
            retry_generation: 2,
            first_receipt_id: receipt_id(2),
            conflicting_semantic_fingerprint: [3; 32],
            detected_at_unix_ms: 50,
        }
    }

    fn resolution(
        decision: ProviderResponseConflictDecision,
    ) -> ProviderResponseConflictResolutionDraft {
        ProviderResponseConflictResolutionDraft {
            decision,
            resolved_by: ActorId::try_new("operator-a").unwrap(),
            policy_version: "provider-conflict-policy-v1".to_owned(),
            safe_reason_code: "retain-first-receipt".to_owned(),
            approval_evidence_reference: "approval/provider-conflict/1".to_owned(),
            causation_id: CausationId::try_new("operator-command-1").unwrap(),
            resolved_at_unix_ms: 60,
        }
    }

    #[test]
    fn deterministic_identity_binds_generation_receipt_and_conflicting_fingerprint() {
        let first = ProviderResponseConflict::record(draft()).unwrap();
        let replay = ProviderResponseConflict::record(draft()).unwrap();
        assert_eq!(first, replay);

        let mut changed_generation = draft();
        changed_generation.retry_generation += 1;
        let changed_generation = ProviderResponseConflict::record(changed_generation).unwrap();
        assert_ne!(first.conflict_id(), changed_generation.conflict_id());

        let mut changed_receipt = draft();
        changed_receipt.first_receipt_id = receipt_id(4);
        let changed_receipt = ProviderResponseConflict::record(changed_receipt).unwrap();
        assert_ne!(first.conflict_id(), changed_receipt.conflict_id());

        let mut changed_fingerprint = draft();
        changed_fingerprint.conflicting_semantic_fingerprint = [5; 32];
        let changed_fingerprint = ProviderResponseConflict::record(changed_fingerprint).unwrap();
        assert_ne!(first.conflict_id(), changed_fingerprint.conflict_id());
    }

    #[test]
    fn exact_resolution_replay_is_auditable_noop_and_conflicting_choice_fails_closed() {
        let mut conflict = ProviderResponseConflict::record(draft()).unwrap();
        assert_eq!(
            conflict
                .resolve(resolution(
                    ProviderResponseConflictDecision::RetainFirstReceipt
                ))
                .unwrap(),
            ReplayDisposition::New
        );
        let first_receipt = conflict.first_receipt_id().clone();
        assert_eq!(
            conflict
                .resolve(resolution(
                    ProviderResponseConflictDecision::RetainFirstReceipt
                ))
                .unwrap(),
            ReplayDisposition::Duplicate
        );

        let error = conflict
            .resolve(resolution(ProviderResponseConflictDecision::RejectRequest))
            .unwrap_err();
        assert_eq!(
            error.code,
            "CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_CONFLICT_ALREADY_RESOLVED"
        );
        assert_eq!(conflict.first_receipt_id(), &first_receipt);
        assert_eq!(
            conflict.resolution().unwrap().decision(),
            ProviderResponseConflictDecision::RetainFirstReceipt
        );
    }

    #[test]
    fn reject_resolution_preserves_exact_operator_policy_approval_and_causation_lineage() {
        let mut conflict = ProviderResponseConflict::record(draft()).unwrap();
        conflict
            .resolve(resolution(ProviderResponseConflictDecision::RejectRequest))
            .unwrap();
        let resolved = conflict.resolution().unwrap();
        assert_eq!(
            resolved.decision(),
            ProviderResponseConflictDecision::RejectRequest
        );
        assert_eq!(resolved.resolved_by().as_str(), "operator-a");
        assert_eq!(resolved.policy_version(), "provider-conflict-policy-v1");
        assert_eq!(resolved.safe_reason_code(), "retain-first-receipt");
        assert_eq!(
            resolved.approval_evidence_reference(),
            "approval/provider-conflict/1"
        );
        assert_eq!(resolved.causation_id().as_str(), "operator-command-1");
        assert_eq!(resolved.resolved_at_unix_ms(), 60);
        assert_eq!(conflict.first_receipt_id(), &receipt_id(2));
        assert_eq!(conflict.conflicting_semantic_fingerprint(), &[3; 32]);
    }

    #[test]
    fn invalid_fingerprint_and_time_regression_are_rejected() {
        let mut invalid_fingerprint = draft();
        invalid_fingerprint.conflicting_semantic_fingerprint = [0; 32];
        assert_eq!(
            ProviderResponseConflict::record(invalid_fingerprint)
                .unwrap_err()
                .code,
            "CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_CONFLICT_FINGERPRINT_INVALID"
        );

        let mut conflict = ProviderResponseConflict::record(draft()).unwrap();
        let mut regressed = resolution(ProviderResponseConflictDecision::RetainFirstReceipt);
        regressed.resolved_at_unix_ms = 49;
        assert_eq!(
            conflict.resolve(regressed).unwrap_err().code,
            "CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_CONFLICT_TIME_INVALID"
        );
    }

    #[test]
    fn strict_persistence_round_trips_and_rejects_unknown_fields() {
        let mut conflict = ProviderResponseConflict::record(draft()).unwrap();
        conflict
            .resolve(resolution(
                ProviderResponseConflictDecision::RetainFirstReceipt,
            ))
            .unwrap();
        let bytes = encode_provider_response_conflict_state(&conflict).unwrap();
        assert_eq!(
            decode_provider_response_conflict_state(&bytes).unwrap(),
            conflict
        );

        let mut value: Value = serde_json::from_slice(&bytes).unwrap();
        value
            .as_object_mut()
            .unwrap()
            .insert("unexpected".to_owned(), Value::Bool(true));
        let error = decode_provider_response_conflict_state(&serde_json::to_vec(&value).unwrap())
            .unwrap_err();
        assert_eq!(error.code, "CUSTOMER_ENRICHMENT_PERSISTED_STATE_INVALID");
    }

    #[test]
    fn canonical_resolution_evidence_is_required() {
        let mut conflict = ProviderResponseConflict::record(draft()).unwrap();
        let mut invalid = resolution(ProviderResponseConflictDecision::RetainFirstReceipt);
        invalid.policy_version = "Policy V1".to_owned();
        assert_eq!(
            conflict.resolve(invalid).unwrap_err().code,
            "CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_CONFLICT_CODE_INVALID"
        );

        let mut invalid = resolution(ProviderResponseConflictDecision::RetainFirstReceipt);
        invalid.approval_evidence_reference = "  ".to_owned();
        assert_eq!(
            conflict.resolve(invalid).unwrap_err().code,
            "CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_CONFLICT_EVIDENCE_INVALID"
        );
    }
}
