use crate::{MappingVersionId, ProviderProfileVersionId, TargetField};
use crm_module_sdk::{
    ActorId, BusinessTransactionId, ErrorCategory, FieldName, FieldViolation, IdempotencyKey,
    SdkError, TenantId,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const REQUEST_ID_DOMAIN: &[u8] = b"crm.customer-enrichment.request/v1";
const RESPONSE_RECEIPT_ID_DOMAIN: &[u8] = b"crm.customer-enrichment.response-receipt/v1";
const SUGGESTION_ID_DOMAIN: &[u8] = b"crm.customer-enrichment.suggestion/v1";
const REVIEW_DECISION_ID_DOMAIN: &[u8] = b"crm.customer-enrichment.review-decision/v1";
const APPLICATION_ATTEMPT_ID_DOMAIN: &[u8] = b"crm.customer-enrichment.application-attempt/v1";
const TARGET_IDEMPOTENCY_DOMAIN: &[u8] = b"crm.customer-enrichment.target-idempotency/v1";

const MAX_RESOURCE_ID_BYTES: usize = 180;
const MAX_EVIDENCE_REFERENCE_BYTES: usize = 240;
const MAX_PROVIDER_CORRELATION_BYTES: usize = 180;
const MAX_REPLAY_KEY_BYTES: usize = 180;
const MAX_SAFE_CODE_BYTES: usize = 80;
const MAX_PROPOSED_VALUE_BYTES: usize = 320;
const MAX_EVIDENCE_REFERENCES: usize = 16;
const MAX_REQUEST_RETRY_GENERATION: u32 = 100;
const MAX_APPLICATION_GENERATION: u32 = 100;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct EnrichmentRequestId(String);

impl EnrichmentRequestId {
    fn from_digest(digest: &[u8]) -> Self {
        Self(format!("enrichment-request-{}", hex(digest)))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ProviderResponseReceiptId(String);

impl ProviderResponseReceiptId {
    fn from_digest(digest: &[u8]) -> Self {
        Self(format!("enrichment-response-{}", hex(digest)))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SuggestionId(String);

impl SuggestionId {
    fn from_digest(digest: &[u8]) -> Self {
        Self(format!("enrichment-suggestion-{}", hex(digest)))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ReviewDecisionId(String);

impl ReviewDecisionId {
    fn from_digest(digest: &[u8]) -> Self {
        Self(format!("enrichment-review-{}", hex(digest)))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ApplicationAttemptId(String);

impl ApplicationAttemptId {
    fn from_digest(digest: &[u8]) -> Self {
        Self(format!("enrichment-application-{}", hex(digest)))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TargetSnapshot {
    pub resource_id: String,
    pub resource_version: u64,
    pub target_field: TargetField,
}

impl TargetSnapshot {
    pub fn try_new(
        resource_id: impl Into<String>,
        resource_version: u64,
        target_field: TargetField,
    ) -> Result<Self, SdkError> {
        let resource_id = bounded_identifier(
            resource_id.into(),
            MAX_RESOURCE_ID_BYTES,
            "target.resource_id",
            "CUSTOMER_ENRICHMENT_TARGET_RESOURCE_ID_INVALID",
        )?;
        if resource_version == 0 {
            return Err(invalid(
                "CUSTOMER_ENRICHMENT_TARGET_VERSION_INVALID",
                "target.resource_version",
                "target resource version must be greater than zero",
            ));
        }
        Ok(Self {
            resource_id,
            resource_version,
            target_field,
        })
    }

    pub fn owner_module_id(&self) -> &'static str {
        self.target_field.owner_module_id()
    }

    pub fn resource_type(&self) -> &'static str {
        self.target_field.resource_type()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequestPolicyEvidence {
    pub purpose_code: String,
    pub legal_basis_code: String,
    pub consent_evidence_reference: Option<String>,
    pub policy_version: String,
}

impl RequestPolicyEvidence {
    pub fn try_new(
        purpose_code: impl Into<String>,
        legal_basis_code: impl Into<String>,
        consent_evidence_reference: Option<String>,
        policy_version: impl Into<String>,
    ) -> Result<Self, SdkError> {
        Ok(Self {
            purpose_code: canonical_key(purpose_code.into(), "request_policy.purpose_code")?,
            legal_basis_code: canonical_key(
                legal_basis_code.into(),
                "request_policy.legal_basis_code",
            )?,
            consent_evidence_reference: consent_evidence_reference
                .map(|value| {
                    bounded_identifier(
                        value,
                        MAX_EVIDENCE_REFERENCE_BYTES,
                        "request_policy.consent_evidence_reference",
                        "CUSTOMER_ENRICHMENT_CONSENT_REFERENCE_INVALID",
                    )
                })
                .transpose()?,
            policy_version: canonical_version(
                policy_version.into(),
                "request_policy.policy_version",
            )?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnrichmentRequestDraft {
    pub tenant_id: TenantId,
    pub requested_by: ActorId,
    pub idempotency_key: IdempotencyKey,
    pub target: TargetSnapshot,
    pub provider_profile_version_id: ProviderProfileVersionId,
    pub mapping_version_id: MappingVersionId,
    pub requested_fields: Vec<TargetField>,
    pub policy_evidence: RequestPolicyEvidence,
    pub created_at_unix_ms: u64,
    pub deadline_at_unix_ms: u64,
    pub expires_at_unix_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnrichmentRequestStatus {
    Created,
    Queued,
    Dispatched,
    ResponseRecorded,
    SuggestionsMaterialized,
    Completed,
    FailedRetryable,
    FailedTerminal,
    Cancelled,
    Expired,
}

impl EnrichmentRequestStatus {
    pub const fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Completed | Self::FailedTerminal | Self::Cancelled | Self::Expired
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnrichmentRequest {
    request_id: EnrichmentRequestId,
    tenant_id: TenantId,
    requested_by: ActorId,
    idempotency_key: IdempotencyKey,
    target: TargetSnapshot,
    provider_profile_version_id: ProviderProfileVersionId,
    mapping_version_id: MappingVersionId,
    requested_fields: Vec<TargetField>,
    policy_evidence: RequestPolicyEvidence,
    created_at_unix_ms: u64,
    deadline_at_unix_ms: u64,
    expires_at_unix_ms: u64,
    status: EnrichmentRequestStatus,
    retry_generation: u32,
    response_receipt_id: Option<ProviderResponseReceiptId>,
    last_safe_failure_code: Option<String>,
    updated_at_unix_ms: u64,
}

#[derive(Serialize)]
struct RequestIdentity<'a> {
    semantic_version: &'static str,
    tenant_id: &'a TenantId,
    idempotency_key: &'a IdempotencyKey,
    target: &'a TargetSnapshot,
    provider_profile_version_id: &'a ProviderProfileVersionId,
    mapping_version_id: &'a MappingVersionId,
    requested_fields: &'a [TargetField],
    policy_evidence: &'a RequestPolicyEvidence,
}

impl EnrichmentRequest {
    pub fn create(draft: EnrichmentRequestDraft) -> Result<Self, SdkError> {
        if draft.created_at_unix_ms >= draft.deadline_at_unix_ms {
            return Err(invalid(
                "CUSTOMER_ENRICHMENT_REQUEST_DEADLINE_INVALID",
                "request.deadline_at_unix_ms",
                "request deadline must be later than request creation",
            ));
        }
        if draft.deadline_at_unix_ms > draft.expires_at_unix_ms {
            return Err(invalid(
                "CUSTOMER_ENRICHMENT_REQUEST_EXPIRY_INVALID",
                "request.expires_at_unix_ms",
                "request expiry must not be earlier than its deadline",
            ));
        }
        let requested_fields = canonical_target_fields(draft.requested_fields)?;
        if !requested_fields.contains(&draft.target.target_field) {
            return Err(invalid(
                "CUSTOMER_ENRICHMENT_TARGET_FIELD_NOT_REQUESTED",
                "request.requested_fields",
                "the exact target field must be included in requested fields",
            ));
        }

        let mut request = Self {
            request_id: EnrichmentRequestId(String::new()),
            tenant_id: draft.tenant_id,
            requested_by: draft.requested_by,
            idempotency_key: draft.idempotency_key,
            target: draft.target,
            provider_profile_version_id: draft.provider_profile_version_id,
            mapping_version_id: draft.mapping_version_id,
            requested_fields,
            policy_evidence: draft.policy_evidence,
            created_at_unix_ms: draft.created_at_unix_ms,
            deadline_at_unix_ms: draft.deadline_at_unix_ms,
            expires_at_unix_ms: draft.expires_at_unix_ms,
            status: EnrichmentRequestStatus::Created,
            retry_generation: 0,
            response_receipt_id: None,
            last_safe_failure_code: None,
            updated_at_unix_ms: draft.created_at_unix_ms,
        };
        request.request_id = EnrichmentRequestId::from_digest(&canonical_digest(
            REQUEST_ID_DOMAIN,
            &request.identity(),
        ));
        Ok(request)
    }

    fn identity(&self) -> RequestIdentity<'_> {
        RequestIdentity {
            semantic_version: "1.0.0",
            tenant_id: &self.tenant_id,
            idempotency_key: &self.idempotency_key,
            target: &self.target,
            provider_profile_version_id: &self.provider_profile_version_id,
            mapping_version_id: &self.mapping_version_id,
            requested_fields: &self.requested_fields,
            policy_evidence: &self.policy_evidence,
        }
    }

    pub fn queue(&mut self, at_unix_ms: u64) -> Result<(), SdkError> {
        self.require_time(at_unix_ms)?;
        match self.status {
            EnrichmentRequestStatus::Created => {
                self.status = EnrichmentRequestStatus::Queued;
                self.updated_at_unix_ms = at_unix_ms;
                Ok(())
            }
            EnrichmentRequestStatus::FailedRetryable => {
                if self.retry_generation >= MAX_REQUEST_RETRY_GENERATION {
                    return Err(conflict(
                        "CUSTOMER_ENRICHMENT_RETRY_LIMIT_REACHED",
                        "request retry generation limit has been reached",
                    ));
                }
                self.retry_generation += 1;
                self.status = EnrichmentRequestStatus::Queued;
                self.last_safe_failure_code = None;
                self.updated_at_unix_ms = at_unix_ms;
                Ok(())
            }
            _ => Err(invalid_transition(self.status, "queued")),
        }
    }

    pub fn mark_dispatched(&mut self, at_unix_ms: u64) -> Result<(), SdkError> {
        self.transition(
            EnrichmentRequestStatus::Queued,
            EnrichmentRequestStatus::Dispatched,
            at_unix_ms,
        )
    }

    pub fn record_response(
        &mut self,
        receipt_id: ProviderResponseReceiptId,
        at_unix_ms: u64,
    ) -> Result<bool, SdkError> {
        self.require_time(at_unix_ms)?;
        if let Some(existing) = &self.response_receipt_id {
            if existing == &receipt_id {
                return Ok(false);
            }
            return Err(conflict(
                "CUSTOMER_ENRICHMENT_RESPONSE_RECEIPT_CONFLICT",
                "the request is already bound to a different response receipt",
            ));
        }
        if self.status != EnrichmentRequestStatus::Dispatched {
            return Err(invalid_transition(self.status, "response_recorded"));
        }
        self.response_receipt_id = Some(receipt_id);
        self.status = EnrichmentRequestStatus::ResponseRecorded;
        self.updated_at_unix_ms = at_unix_ms;
        Ok(true)
    }

    pub fn mark_suggestions_materialized(&mut self, at_unix_ms: u64) -> Result<(), SdkError> {
        self.transition(
            EnrichmentRequestStatus::ResponseRecorded,
            EnrichmentRequestStatus::SuggestionsMaterialized,
            at_unix_ms,
        )
    }

    pub fn complete(&mut self, at_unix_ms: u64) -> Result<(), SdkError> {
        self.transition(
            EnrichmentRequestStatus::SuggestionsMaterialized,
            EnrichmentRequestStatus::Completed,
            at_unix_ms,
        )
    }

    pub fn fail_retryable(
        &mut self,
        safe_code: impl Into<String>,
        at_unix_ms: u64,
    ) -> Result<(), SdkError> {
        self.fail(
            EnrichmentRequestStatus::FailedRetryable,
            safe_code.into(),
            at_unix_ms,
        )
    }

    pub fn fail_terminal(
        &mut self,
        safe_code: impl Into<String>,
        at_unix_ms: u64,
    ) -> Result<(), SdkError> {
        self.fail(
            EnrichmentRequestStatus::FailedTerminal,
            safe_code.into(),
            at_unix_ms,
        )
    }

    pub fn cancel(&mut self, at_unix_ms: u64) -> Result<(), SdkError> {
        self.require_time(at_unix_ms)?;
        if self.status.is_terminal() {
            return Err(invalid_transition(self.status, "cancelled"));
        }
        self.status = EnrichmentRequestStatus::Cancelled;
        self.updated_at_unix_ms = at_unix_ms;
        Ok(())
    }

    pub fn expire(&mut self, at_unix_ms: u64) -> Result<(), SdkError> {
        self.require_time(at_unix_ms)?;
        if at_unix_ms < self.expires_at_unix_ms {
            return Err(invalid(
                "CUSTOMER_ENRICHMENT_REQUEST_NOT_EXPIRED",
                "request.expires_at_unix_ms",
                "request cannot expire before its configured expiry timestamp",
            ));
        }
        if self.status.is_terminal() {
            return Err(invalid_transition(self.status, "expired"));
        }
        self.status = EnrichmentRequestStatus::Expired;
        self.updated_at_unix_ms = at_unix_ms;
        Ok(())
    }

    fn fail(
        &mut self,
        target_status: EnrichmentRequestStatus,
        safe_code: String,
        at_unix_ms: u64,
    ) -> Result<(), SdkError> {
        self.require_time(at_unix_ms)?;
        if self.status.is_terminal() {
            return Err(invalid_transition(self.status, "failed"));
        }
        let safe_code = bounded_safe_code(safe_code, "request.last_safe_failure_code")?;
        self.status = target_status;
        self.last_safe_failure_code = Some(safe_code);
        self.updated_at_unix_ms = at_unix_ms;
        Ok(())
    }

    fn transition(
        &mut self,
        expected: EnrichmentRequestStatus,
        next: EnrichmentRequestStatus,
        at_unix_ms: u64,
    ) -> Result<(), SdkError> {
        self.require_time(at_unix_ms)?;
        if self.status != expected {
            return Err(invalid_transition(self.status, status_name(next)));
        }
        self.status = next;
        self.updated_at_unix_ms = at_unix_ms;
        Ok(())
    }

    fn require_time(&self, at_unix_ms: u64) -> Result<(), SdkError> {
        if at_unix_ms < self.updated_at_unix_ms {
            return Err(invalid(
                "CUSTOMER_ENRICHMENT_TIME_REGRESSION",
                "request.updated_at_unix_ms",
                "request lifecycle timestamps must be monotonic",
            ));
        }
        Ok(())
    }

    pub fn request_id(&self) -> &EnrichmentRequestId {
        &self.request_id
    }

    pub fn tenant_id(&self) -> &TenantId {
        &self.tenant_id
    }

    pub fn target(&self) -> &TargetSnapshot {
        &self.target
    }

    pub fn provider_profile_version_id(&self) -> &ProviderProfileVersionId {
        &self.provider_profile_version_id
    }

    pub fn mapping_version_id(&self) -> &MappingVersionId {
        &self.mapping_version_id
    }

    pub const fn status(&self) -> EnrichmentRequestStatus {
        self.status
    }

    pub const fn retry_generation(&self) -> u32 {
        self.retry_generation
    }

    pub fn response_receipt_id(&self) -> Option<&ProviderResponseReceiptId> {
        self.response_receipt_id.as_ref()
    }

    pub fn last_safe_failure_code(&self) -> Option<&str> {
        self.last_safe_failure_code.as_deref()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderResponseClass {
    Success,
    NoMatch,
    RetryableFailure,
    TerminalFailure,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderResponseReceiptDraft {
    pub request_id: EnrichmentRequestId,
    pub provider_profile_version_id: ProviderProfileVersionId,
    pub mapping_version_id: MappingVersionId,
    pub replay_key: String,
    pub provider_correlation_id: Option<String>,
    pub response_class: ProviderResponseClass,
    pub canonical_response_digest: [u8; 32],
    pub provider_observed_at_unix_ms: Option<u64>,
    pub retrieved_at_unix_ms: u64,
    pub metered_units: u64,
    pub protected_evidence_reference: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderResponseReceipt {
    receipt_id: ProviderResponseReceiptId,
    request_id: EnrichmentRequestId,
    provider_profile_version_id: ProviderProfileVersionId,
    mapping_version_id: MappingVersionId,
    replay_key: String,
    provider_correlation_id: Option<String>,
    response_class: ProviderResponseClass,
    canonical_response_digest: [u8; 32],
    provider_observed_at_unix_ms: Option<u64>,
    retrieved_at_unix_ms: u64,
    metered_units: u64,
    protected_evidence_reference: Option<String>,
}

#[derive(Serialize)]
struct ResponseReceiptIdentity<'a> {
    semantic_version: &'static str,
    request_id: &'a EnrichmentRequestId,
    replay_key: &'a str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplayDisposition {
    New,
    Duplicate,
}

impl ProviderResponseReceipt {
    pub fn record(draft: ProviderResponseReceiptDraft) -> Result<Self, SdkError> {
        if draft
            .canonical_response_digest
            .iter()
            .all(|byte| *byte == 0)
        {
            return Err(invalid(
                "CUSTOMER_ENRICHMENT_RESPONSE_DIGEST_INVALID",
                "response.canonical_response_digest",
                "canonical response digest must not be all zeroes",
            ));
        }
        if draft
            .provider_observed_at_unix_ms
            .is_some_and(|observed| observed > draft.retrieved_at_unix_ms)
        {
            return Err(invalid(
                "CUSTOMER_ENRICHMENT_RESPONSE_TIME_INVALID",
                "response.provider_observed_at_unix_ms",
                "provider observed timestamp must not be later than retrieval",
            ));
        }
        let replay_key = bounded_identifier(
            draft.replay_key,
            MAX_REPLAY_KEY_BYTES,
            "response.replay_key",
            "CUSTOMER_ENRICHMENT_REPLAY_KEY_INVALID",
        )?;
        let provider_correlation_id = draft
            .provider_correlation_id
            .map(|value| {
                bounded_identifier(
                    value,
                    MAX_PROVIDER_CORRELATION_BYTES,
                    "response.provider_correlation_id",
                    "CUSTOMER_ENRICHMENT_PROVIDER_CORRELATION_INVALID",
                )
            })
            .transpose()?;
        let protected_evidence_reference = draft
            .protected_evidence_reference
            .map(|value| {
                bounded_identifier(
                    value,
                    MAX_EVIDENCE_REFERENCE_BYTES,
                    "response.protected_evidence_reference",
                    "CUSTOMER_ENRICHMENT_EVIDENCE_REFERENCE_INVALID",
                )
            })
            .transpose()?;

        let identity = ResponseReceiptIdentity {
            semantic_version: "1.0.0",
            request_id: &draft.request_id,
            replay_key: &replay_key,
        };
        Ok(Self {
            receipt_id: ProviderResponseReceiptId::from_digest(&canonical_digest(
                RESPONSE_RECEIPT_ID_DOMAIN,
                &identity,
            )),
            request_id: draft.request_id,
            provider_profile_version_id: draft.provider_profile_version_id,
            mapping_version_id: draft.mapping_version_id,
            replay_key,
            provider_correlation_id,
            response_class: draft.response_class,
            canonical_response_digest: draft.canonical_response_digest,
            provider_observed_at_unix_ms: draft.provider_observed_at_unix_ms,
            retrieved_at_unix_ms: draft.retrieved_at_unix_ms,
            metered_units: draft.metered_units,
            protected_evidence_reference,
        })
    }

    pub fn reconcile(&self, candidate: &Self) -> Result<ReplayDisposition, SdkError> {
        if self.receipt_id != candidate.receipt_id {
            return Ok(ReplayDisposition::New);
        }
        if self == candidate {
            return Ok(ReplayDisposition::Duplicate);
        }
        Err(conflict(
            "CUSTOMER_ENRICHMENT_CONFLICTING_PROVIDER_REPLAY",
            "the same provider replay identity produced different canonical evidence",
        ))
    }

    pub fn receipt_id(&self) -> &ProviderResponseReceiptId {
        &self.receipt_id
    }

    pub fn request_id(&self) -> &EnrichmentRequestId {
        &self.request_id
    }

    pub fn canonical_response_digest(&self) -> &[u8; 32] {
        &self.canonical_response_digest
    }

    pub const fn response_class(&self) -> ProviderResponseClass {
        self.response_class
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuggestionDraft {
    pub request_id: EnrichmentRequestId,
    pub response_receipt_id: ProviderResponseReceiptId,
    pub provider_profile_version_id: ProviderProfileVersionId,
    pub mapping_version_id: MappingVersionId,
    pub target: TargetSnapshot,
    pub proposed_value: String,
    pub observed_at_unix_ms: Option<u64>,
    pub retrieved_at_unix_ms: u64,
    pub effective_at_unix_ms: u64,
    pub fresh_until_unix_ms: u64,
    pub expires_at_unix_ms: u64,
    pub confidence_basis_points: Option<u16>,
    pub purpose_code: String,
    pub legal_basis_code: String,
    pub license_id: String,
    pub permitted_use_class: String,
    pub residency_region: String,
    pub retention_days: u32,
    pub consent_evidence_reference: Option<String>,
    pub evidence_references: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Suggestion {
    suggestion_id: SuggestionId,
    request_id: EnrichmentRequestId,
    response_receipt_id: ProviderResponseReceiptId,
    provider_profile_version_id: ProviderProfileVersionId,
    mapping_version_id: MappingVersionId,
    target: TargetSnapshot,
    proposed_value: String,
    proposed_value_digest: [u8; 32],
    observed_at_unix_ms: Option<u64>,
    retrieved_at_unix_ms: u64,
    effective_at_unix_ms: u64,
    fresh_until_unix_ms: u64,
    expires_at_unix_ms: u64,
    confidence_basis_points: Option<u16>,
    purpose_code: String,
    legal_basis_code: String,
    license_id: String,
    permitted_use_class: String,
    residency_region: String,
    retention_days: u32,
    consent_evidence_reference: Option<String>,
    evidence_references: Vec<String>,
}

#[derive(Serialize)]
struct SuggestionIdentity<'a> {
    semantic_version: &'static str,
    request_id: &'a EnrichmentRequestId,
    response_receipt_id: &'a ProviderResponseReceiptId,
    mapping_version_id: &'a MappingVersionId,
    target: &'a TargetSnapshot,
    proposed_value_digest: &'a [u8; 32],
}

impl Suggestion {
    pub fn materialize(draft: SuggestionDraft) -> Result<Self, SdkError> {
        if draft.proposed_value.chars().any(char::is_control) {
            return Err(invalid(
                "CUSTOMER_ENRICHMENT_PROPOSED_VALUE_INVALID",
                "suggestion.proposed_value",
                "proposed value must not contain control characters",
            ));
        }
        let proposed_value = draft
            .proposed_value
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        if proposed_value.is_empty() || proposed_value.len() > MAX_PROPOSED_VALUE_BYTES {
            return Err(invalid(
                "CUSTOMER_ENRICHMENT_PROPOSED_VALUE_INVALID",
                "suggestion.proposed_value",
                format!(
                    "proposed value must contain 1..={MAX_PROPOSED_VALUE_BYTES} UTF-8 bytes after normalization"
                ),
            ));
        }
        if draft
            .observed_at_unix_ms
            .is_some_and(|observed| observed > draft.retrieved_at_unix_ms)
        {
            return Err(invalid(
                "CUSTOMER_ENRICHMENT_SUGGESTION_TIME_INVALID",
                "suggestion.observed_at_unix_ms",
                "observed timestamp must not be later than retrieval",
            ));
        }
        if draft.effective_at_unix_ms > draft.fresh_until_unix_ms
            || draft.fresh_until_unix_ms > draft.expires_at_unix_ms
        {
            return Err(invalid(
                "CUSTOMER_ENRICHMENT_SUGGESTION_WINDOW_INVALID",
                "suggestion.fresh_until_unix_ms",
                "suggestion time window must satisfy effective <= fresh_until <= expiry",
            ));
        }
        if draft
            .confidence_basis_points
            .is_some_and(|confidence| confidence > 10_000)
        {
            return Err(invalid(
                "CUSTOMER_ENRICHMENT_CONFIDENCE_INVALID",
                "suggestion.confidence_basis_points",
                "confidence must be in the inclusive range 0..=10000 basis points",
            ));
        }
        let evidence_references = canonical_references(draft.evidence_references)?;
        let proposed_value_digest = fixed_digest(
            b"crm.customer-enrichment.proposed-value/v1",
            proposed_value.as_bytes(),
        );
        let purpose_code = canonical_key(draft.purpose_code, "suggestion.purpose_code")?;
        let legal_basis_code =
            canonical_key(draft.legal_basis_code, "suggestion.legal_basis_code")?;
        let permitted_use_class =
            canonical_key(draft.permitted_use_class, "suggestion.permitted_use_class")?;
        let residency_region =
            canonical_key(draft.residency_region, "suggestion.residency_region")?;
        let license_id = bounded_identifier(
            draft.license_id,
            MAX_EVIDENCE_REFERENCE_BYTES,
            "suggestion.license_id",
            "CUSTOMER_ENRICHMENT_LICENSE_ID_INVALID",
        )?;
        let consent_evidence_reference = draft
            .consent_evidence_reference
            .map(|value| {
                bounded_identifier(
                    value,
                    MAX_EVIDENCE_REFERENCE_BYTES,
                    "suggestion.consent_evidence_reference",
                    "CUSTOMER_ENRICHMENT_CONSENT_REFERENCE_INVALID",
                )
            })
            .transpose()?;

        let identity = SuggestionIdentity {
            semantic_version: "1.0.0",
            request_id: &draft.request_id,
            response_receipt_id: &draft.response_receipt_id,
            mapping_version_id: &draft.mapping_version_id,
            target: &draft.target,
            proposed_value_digest: &proposed_value_digest,
        };
        Ok(Self {
            suggestion_id: SuggestionId::from_digest(&canonical_digest(
                SUGGESTION_ID_DOMAIN,
                &identity,
            )),
            request_id: draft.request_id,
            response_receipt_id: draft.response_receipt_id,
            provider_profile_version_id: draft.provider_profile_version_id,
            mapping_version_id: draft.mapping_version_id,
            target: draft.target,
            proposed_value,
            proposed_value_digest,
            observed_at_unix_ms: draft.observed_at_unix_ms,
            retrieved_at_unix_ms: draft.retrieved_at_unix_ms,
            effective_at_unix_ms: draft.effective_at_unix_ms,
            fresh_until_unix_ms: draft.fresh_until_unix_ms,
            expires_at_unix_ms: draft.expires_at_unix_ms,
            confidence_basis_points: draft.confidence_basis_points,
            purpose_code,
            legal_basis_code,
            license_id,
            permitted_use_class,
            residency_region,
            retention_days: draft.retention_days,
            consent_evidence_reference,
            evidence_references,
        })
    }

    pub fn is_fresh_at(&self, at_unix_ms: u64) -> bool {
        at_unix_ms >= self.effective_at_unix_ms && at_unix_ms < self.fresh_until_unix_ms
    }

    pub fn is_expired_at(&self, at_unix_ms: u64) -> bool {
        at_unix_ms >= self.expires_at_unix_ms
    }

    pub fn suggestion_id(&self) -> &SuggestionId {
        &self.suggestion_id
    }

    pub fn target(&self) -> &TargetSnapshot {
        &self.target
    }

    pub fn proposed_value(&self) -> &str {
        &self.proposed_value
    }

    pub fn proposed_value_digest(&self) -> &[u8; 32] {
        &self.proposed_value_digest
    }

    pub const fn expires_at_unix_ms(&self) -> u64 {
        self.expires_at_unix_ms
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewDecisionKind {
    Accepted,
    Rejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalRequirement {
    NotRequired,
    Required,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewDecision {
    decision_id: ReviewDecisionId,
    suggestion_id: SuggestionId,
    target_resource_version: u64,
    proposed_value_digest: [u8; 32],
    reviewed_by: ActorId,
    kind: ReviewDecisionKind,
    policy_version: String,
    safe_reason_code: String,
    approval_evidence_reference: Option<String>,
    decided_at_unix_ms: u64,
    expires_at_unix_ms: Option<u64>,
}

#[derive(Serialize)]
struct ReviewDecisionIdentity<'a> {
    semantic_version: &'static str,
    suggestion_id: &'a SuggestionId,
    target_resource_version: u64,
    proposed_value_digest: &'a [u8; 32],
    reviewed_by: &'a ActorId,
    kind: ReviewDecisionKind,
    policy_version: &'a str,
    safe_reason_code: &'a str,
    approval_evidence_reference: &'a Option<String>,
    decided_at_unix_ms: u64,
    expires_at_unix_ms: Option<u64>,
}

impl ReviewDecision {
    #[allow(clippy::too_many_arguments)]
    pub fn decide(
        suggestion: &Suggestion,
        reviewed_by: ActorId,
        kind: ReviewDecisionKind,
        policy_version: impl Into<String>,
        safe_reason_code: impl Into<String>,
        approval_requirement: ApprovalRequirement,
        approval_evidence_reference: Option<String>,
        decided_at_unix_ms: u64,
        expires_at_unix_ms: Option<u64>,
    ) -> Result<Self, SdkError> {
        if suggestion.is_expired_at(decided_at_unix_ms) {
            return Err(conflict(
                "CUSTOMER_ENRICHMENT_SUGGESTION_EXPIRED",
                "an expired suggestion cannot be reviewed",
            ));
        }
        if expires_at_unix_ms.is_some_and(|expiry| expiry <= decided_at_unix_ms) {
            return Err(invalid(
                "CUSTOMER_ENRICHMENT_REVIEW_EXPIRY_INVALID",
                "review.expires_at_unix_ms",
                "review expiry must be later than its decision timestamp",
            ));
        }
        if matches!(kind, ReviewDecisionKind::Accepted)
            && matches!(approval_requirement, ApprovalRequirement::Required)
            && approval_evidence_reference.is_none()
        {
            return Err(conflict(
                "CUSTOMER_ENRICHMENT_APPROVAL_REQUIRED",
                "approval evidence is required for this accepted suggestion",
            ));
        }
        let policy_version = canonical_version(policy_version.into(), "review.policy_version")?;
        let safe_reason_code =
            bounded_safe_code(safe_reason_code.into(), "review.safe_reason_code")?;
        let approval_evidence_reference = approval_evidence_reference
            .map(|value| {
                bounded_identifier(
                    value,
                    MAX_EVIDENCE_REFERENCE_BYTES,
                    "review.approval_evidence_reference",
                    "CUSTOMER_ENRICHMENT_APPROVAL_REFERENCE_INVALID",
                )
            })
            .transpose()?;
        let identity = ReviewDecisionIdentity {
            semantic_version: "1.0.0",
            suggestion_id: suggestion.suggestion_id(),
            target_resource_version: suggestion.target().resource_version,
            proposed_value_digest: suggestion.proposed_value_digest(),
            reviewed_by: &reviewed_by,
            kind,
            policy_version: &policy_version,
            safe_reason_code: &safe_reason_code,
            approval_evidence_reference: &approval_evidence_reference,
            decided_at_unix_ms,
            expires_at_unix_ms,
        };
        Ok(Self {
            decision_id: ReviewDecisionId::from_digest(&canonical_digest(
                REVIEW_DECISION_ID_DOMAIN,
                &identity,
            )),
            suggestion_id: suggestion.suggestion_id().clone(),
            target_resource_version: suggestion.target().resource_version,
            proposed_value_digest: *suggestion.proposed_value_digest(),
            reviewed_by,
            kind,
            policy_version,
            safe_reason_code,
            approval_evidence_reference,
            decided_at_unix_ms,
            expires_at_unix_ms,
        })
    }

    pub fn is_effective_at(&self, at_unix_ms: u64) -> bool {
        at_unix_ms >= self.decided_at_unix_ms
            && self
                .expires_at_unix_ms
                .is_none_or(|expiry| at_unix_ms < expiry)
    }

    pub fn decision_id(&self) -> &ReviewDecisionId {
        &self.decision_id
    }

    pub fn suggestion_id(&self) -> &SuggestionId {
        &self.suggestion_id
    }

    pub const fn kind(&self) -> ReviewDecisionKind {
        self.kind
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum ApplicationOutcome {
    Succeeded {
        business_transaction_id: BusinessTransactionId,
        resulting_target_version: u64,
    },
    RetryableFailure {
        safe_code: String,
    },
    TerminalFailure {
        safe_code: String,
    },
    StaleTarget {
        actual_target_version: u64,
    },
    AuthorizationDenied,
    PolicyDenied,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordedApplicationOutcome {
    pub outcome: ApplicationOutcome,
    pub recorded_at_unix_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApplicationAttempt {
    attempt_id: ApplicationAttemptId,
    tenant_id: TenantId,
    suggestion_id: SuggestionId,
    review_decision_id: ReviewDecisionId,
    target: TargetSnapshot,
    proposed_value_digest: [u8; 32],
    application_generation: u32,
    owner_capability_id: String,
    owner_capability_version: String,
    target_idempotency_key: IdempotencyKey,
    planned_at_unix_ms: u64,
    recorded_outcome: Option<RecordedApplicationOutcome>,
}

#[derive(Serialize)]
struct ApplicationAttemptIdentity<'a> {
    semantic_version: &'static str,
    tenant_id: &'a TenantId,
    suggestion_id: &'a SuggestionId,
    application_generation: u32,
    owner_capability_id: &'a str,
    owner_capability_version: &'a str,
}

impl ApplicationAttempt {
    pub fn plan(
        tenant_id: TenantId,
        suggestion: &Suggestion,
        decision: &ReviewDecision,
        application_generation: u32,
        planned_at_unix_ms: u64,
    ) -> Result<Self, SdkError> {
        if application_generation > MAX_APPLICATION_GENERATION {
            return Err(invalid(
                "CUSTOMER_ENRICHMENT_APPLICATION_GENERATION_INVALID",
                "application.application_generation",
                format!(
                    "application generation must be in the inclusive range 0..={MAX_APPLICATION_GENERATION}"
                ),
            ));
        }
        if decision.kind() != ReviewDecisionKind::Accepted {
            return Err(conflict(
                "CUSTOMER_ENRICHMENT_SUGGESTION_NOT_ACCEPTED",
                "only an accepted suggestion can be applied",
            ));
        }
        if decision.suggestion_id() != suggestion.suggestion_id()
            || decision.target_resource_version != suggestion.target().resource_version
            || decision.proposed_value_digest != *suggestion.proposed_value_digest()
        {
            return Err(conflict(
                "CUSTOMER_ENRICHMENT_REVIEW_BINDING_MISMATCH",
                "review decision is not bound to the exact suggestion value and target version",
            ));
        }
        if !decision.is_effective_at(planned_at_unix_ms) {
            return Err(conflict(
                "CUSTOMER_ENRICHMENT_REVIEW_EXPIRED",
                "review decision is not effective at application time",
            ));
        }
        if !suggestion.is_fresh_at(planned_at_unix_ms) {
            return Err(conflict(
                "CUSTOMER_ENRICHMENT_SUGGESTION_STALE",
                "suggestion is not fresh at application time",
            ));
        }
        let owner_capability_id = suggestion
            .target()
            .target_field
            .owner_capability_id()
            .to_owned();
        let owner_capability_version = suggestion
            .target()
            .target_field
            .owner_capability_version()
            .to_owned();
        let identity = ApplicationAttemptIdentity {
            semantic_version: "1.0.0",
            tenant_id: &tenant_id,
            suggestion_id: suggestion.suggestion_id(),
            application_generation,
            owner_capability_id: &owner_capability_id,
            owner_capability_version: &owner_capability_version,
        };
        let attempt_digest = canonical_digest(APPLICATION_ATTEMPT_ID_DOMAIN, &identity);
        let target_digest = canonical_digest(TARGET_IDEMPOTENCY_DOMAIN, &identity);
        let target_idempotency_key =
            IdempotencyKey::try_new(format!("customer-enrichment-apply-{}", hex(&target_digest)))
                .map_err(|_| {
                internal(
                    "CUSTOMER_ENRICHMENT_TARGET_IDEMPOTENCY_INTERNAL",
                    "could not construct deterministic target idempotency key",
                )
            })?;
        Ok(Self {
            attempt_id: ApplicationAttemptId::from_digest(&attempt_digest),
            tenant_id,
            suggestion_id: suggestion.suggestion_id().clone(),
            review_decision_id: decision.decision_id().clone(),
            target: suggestion.target().clone(),
            proposed_value_digest: *suggestion.proposed_value_digest(),
            application_generation,
            owner_capability_id,
            owner_capability_version,
            target_idempotency_key,
            planned_at_unix_ms,
            recorded_outcome: None,
        })
    }

    pub fn record_outcome(
        &mut self,
        outcome: ApplicationOutcome,
        recorded_at_unix_ms: u64,
    ) -> Result<ReplayDisposition, SdkError> {
        if recorded_at_unix_ms < self.planned_at_unix_ms {
            return Err(invalid(
                "CUSTOMER_ENRICHMENT_APPLICATION_TIME_INVALID",
                "application.recorded_at_unix_ms",
                "application outcome cannot be recorded before planning",
            ));
        }
        validate_application_outcome(&outcome)?;
        let candidate = RecordedApplicationOutcome {
            outcome,
            recorded_at_unix_ms,
        };
        match &self.recorded_outcome {
            None => {
                self.recorded_outcome = Some(candidate);
                Ok(ReplayDisposition::New)
            }
            Some(existing) if existing == &candidate => Ok(ReplayDisposition::Duplicate),
            Some(_) => Err(conflict(
                "CUSTOMER_ENRICHMENT_APPLICATION_OUTCOME_CONFLICT",
                "the same deterministic application attempt produced conflicting outcome evidence",
            )),
        }
    }

    pub fn attempt_id(&self) -> &ApplicationAttemptId {
        &self.attempt_id
    }

    pub fn target_idempotency_key(&self) -> &IdempotencyKey {
        &self.target_idempotency_key
    }

    pub fn recorded_outcome(&self) -> Option<&RecordedApplicationOutcome> {
        self.recorded_outcome.as_ref()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuggestionLifecycleStatus {
    Proposed,
    Accepted,
    Rejected,
    Expired,
    Superseded,
    Applied,
    ApplicationFailedRetryable,
    ApplicationFailedTerminal,
}

pub fn derive_suggestion_status(
    suggestion: &Suggestion,
    latest_decision: Option<&ReviewDecision>,
    latest_attempt: Option<&ApplicationAttempt>,
    superseded_by: Option<&SuggestionId>,
    at_unix_ms: u64,
) -> SuggestionLifecycleStatus {
    if superseded_by.is_some() {
        return SuggestionLifecycleStatus::Superseded;
    }
    if suggestion.is_expired_at(at_unix_ms) {
        return SuggestionLifecycleStatus::Expired;
    }
    if let Some(outcome) = latest_attempt.and_then(ApplicationAttempt::recorded_outcome) {
        return match outcome.outcome {
            ApplicationOutcome::Succeeded { .. } => SuggestionLifecycleStatus::Applied,
            ApplicationOutcome::RetryableFailure { .. } => {
                SuggestionLifecycleStatus::ApplicationFailedRetryable
            }
            ApplicationOutcome::TerminalFailure { .. }
            | ApplicationOutcome::StaleTarget { .. }
            | ApplicationOutcome::AuthorizationDenied
            | ApplicationOutcome::PolicyDenied => {
                SuggestionLifecycleStatus::ApplicationFailedTerminal
            }
        };
    }
    match latest_decision {
        Some(decision)
            if decision.is_effective_at(at_unix_ms)
                && decision.kind() == ReviewDecisionKind::Accepted =>
        {
            SuggestionLifecycleStatus::Accepted
        }
        Some(decision)
            if decision.is_effective_at(at_unix_ms)
                && decision.kind() == ReviewDecisionKind::Rejected =>
        {
            SuggestionLifecycleStatus::Rejected
        }
        _ => SuggestionLifecycleStatus::Proposed,
    }
}

fn validate_application_outcome(outcome: &ApplicationOutcome) -> Result<(), SdkError> {
    match outcome {
        ApplicationOutcome::Succeeded {
            resulting_target_version,
            ..
        } if *resulting_target_version == 0 => Err(invalid(
            "CUSTOMER_ENRICHMENT_RESULTING_VERSION_INVALID",
            "application.resulting_target_version",
            "resulting target version must be greater than zero",
        )),
        ApplicationOutcome::RetryableFailure { safe_code }
        | ApplicationOutcome::TerminalFailure { safe_code } => {
            bounded_safe_code(safe_code.clone(), "application.safe_code").map(|_| ())
        }
        ApplicationOutcome::StaleTarget {
            actual_target_version,
        } if *actual_target_version == 0 => Err(invalid(
            "CUSTOMER_ENRICHMENT_ACTUAL_VERSION_INVALID",
            "application.actual_target_version",
            "actual target version must be greater than zero",
        )),
        _ => Ok(()),
    }
}

fn canonical_target_fields(mut values: Vec<TargetField>) -> Result<Vec<TargetField>, SdkError> {
    if values.is_empty() || values.len() > 8 {
        return Err(invalid(
            "CUSTOMER_ENRICHMENT_REQUESTED_FIELDS_INVALID",
            "request.requested_fields",
            "requested fields must contain 1..=8 entries",
        ));
    }
    values.sort();
    if values.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(invalid(
            "CUSTOMER_ENRICHMENT_REQUESTED_FIELDS_DUPLICATE",
            "request.requested_fields",
            "requested fields must be unique",
        ));
    }
    Ok(values)
}

fn canonical_references(mut values: Vec<String>) -> Result<Vec<String>, SdkError> {
    if values.len() > MAX_EVIDENCE_REFERENCES {
        return Err(invalid(
            "CUSTOMER_ENRICHMENT_EVIDENCE_REFERENCES_INVALID",
            "suggestion.evidence_references",
            format!("evidence references must contain at most {MAX_EVIDENCE_REFERENCES} entries"),
        ));
    }
    let mut canonical = Vec::with_capacity(values.len());
    for value in values.drain(..) {
        canonical.push(bounded_identifier(
            value,
            MAX_EVIDENCE_REFERENCE_BYTES,
            "suggestion.evidence_references",
            "CUSTOMER_ENRICHMENT_EVIDENCE_REFERENCE_INVALID",
        )?);
    }
    canonical.sort();
    if canonical.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(invalid(
            "CUSTOMER_ENRICHMENT_EVIDENCE_REFERENCE_DUPLICATE",
            "suggestion.evidence_references",
            "evidence references must be unique",
        ));
    }
    Ok(canonical)
}

fn canonical_key(value: String, field: &'static str) -> Result<String, SdkError> {
    let valid = !value.is_empty()
        && value.len() <= 80
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
            "CUSTOMER_ENRICHMENT_CANONICAL_KEY_INVALID",
            field,
            "canonical key must be 1..=80 lowercase ASCII bytes using letters, digits, dot, underscore or hyphen and start/end alphanumeric",
        ));
    }
    Ok(value)
}

fn canonical_version(value: String, field: &'static str) -> Result<String, SdkError> {
    let valid = !value.is_empty()
        && value.len() <= 48
        && value.is_ascii()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'+'));
    if !valid {
        return Err(invalid(
            "CUSTOMER_ENRICHMENT_VERSION_INVALID",
            field,
            "version must be 1..=48 ASCII alphanumeric/dot/hyphen/plus bytes",
        ));
    }
    Ok(value)
}

fn bounded_identifier(
    value: String,
    maximum_bytes: usize,
    field: &'static str,
    code: &'static str,
) -> Result<String, SdkError> {
    if value.is_empty() || value.len() > maximum_bytes || value.chars().any(char::is_control) {
        return Err(invalid(
            code,
            field,
            format!("value must contain 1..={maximum_bytes} bytes and no control characters"),
        ));
    }
    Ok(value)
}

fn bounded_safe_code(value: String, field: &'static str) -> Result<String, SdkError> {
    canonical_key(value, field).and_then(|value| {
        if value.len() > MAX_SAFE_CODE_BYTES {
            Err(invalid(
                "CUSTOMER_ENRICHMENT_SAFE_CODE_INVALID",
                field,
                format!("safe code must not exceed {MAX_SAFE_CODE_BYTES} bytes"),
            ))
        } else {
            Ok(value)
        }
    })
}

fn canonical_digest<T: Serialize>(domain: &[u8], value: &T) -> Vec<u8> {
    let encoded = serde_json::to_vec(value)
        .expect("canonical customer-enrichment lifecycle evidence must serialize");
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update((encoded.len() as u64).to_be_bytes());
    hasher.update(encoded);
    hasher.finalize().to_vec()
}

fn fixed_digest(domain: &[u8], value: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update((value.len() as u64).to_be_bytes());
    hasher.update(value);
    hasher.finalize().into()
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

fn status_name(status: EnrichmentRequestStatus) -> &'static str {
    match status {
        EnrichmentRequestStatus::Created => "created",
        EnrichmentRequestStatus::Queued => "queued",
        EnrichmentRequestStatus::Dispatched => "dispatched",
        EnrichmentRequestStatus::ResponseRecorded => "response_recorded",
        EnrichmentRequestStatus::SuggestionsMaterialized => "suggestions_materialized",
        EnrichmentRequestStatus::Completed => "completed",
        EnrichmentRequestStatus::FailedRetryable => "failed_retryable",
        EnrichmentRequestStatus::FailedTerminal => "failed_terminal",
        EnrichmentRequestStatus::Cancelled => "cancelled",
        EnrichmentRequestStatus::Expired => "expired",
    }
}

fn invalid_transition(current: EnrichmentRequestStatus, attempted: &'static str) -> SdkError {
    conflict(
        "CUSTOMER_ENRICHMENT_REQUEST_TRANSITION_INVALID",
        format!(
            "cannot transition enrichment request from {} to {attempted}",
            status_name(current)
        ),
    )
}

fn invalid(code: &'static str, field: &'static str, safe_message: impl Into<String>) -> SdkError {
    let mut error = SdkError::new(
        code,
        ErrorCategory::InvalidArgument,
        false,
        "The customer-enrichment evidence is invalid.",
    );
    error.field_violations.push(FieldViolation {
        field: FieldName::try_new(field).expect("static field path must be valid"),
        code: code.to_owned(),
        safe_message: safe_message.into(),
    });
    error
}

fn conflict(code: &'static str, safe_message: impl Into<String>) -> SdkError {
    SdkError::new(code, ErrorCategory::Conflict, false, safe_message)
}

fn internal(code: &'static str, safe_message: impl Into<String>) -> SdkError {
    SdkError::new(code, ErrorCategory::Internal, false, safe_message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        MappingDraft, MappingNormalization, MappingVersion, ProviderProfileDraft,
        ProviderProfileVersion, RawPayloadPolicy,
    };

    fn tenant() -> TenantId {
        TenantId::try_new("tenant-a").unwrap()
    }

    fn actor(value: &str) -> ActorId {
        ActorId::try_new(value).unwrap()
    }

    fn definitions() -> (ProviderProfileVersion, MappingVersion) {
        let provider = ProviderProfileVersion::publish(ProviderProfileDraft {
            provider_key: "company_registry".to_owned(),
            adapter_kind: "registry_http_v1".to_owned(),
            adapter_contract_version: "1.0.0".to_owned(),
            supported_target_fields: vec![TargetField::PartyDisplayName],
            purpose_codes: vec!["customer_profile_enrichment".to_owned()],
            license_id: "Registry licence v3".to_owned(),
            permitted_use_class: "customer_master_review".to_owned(),
            residency_region: "eu".to_owned(),
            retention_days: 30,
            raw_payload_policy: RawPayloadPolicy::DigestOnly,
            credential_handle_aliases: vec!["registry_primary".to_owned()],
            effective_at_unix_ms: 1,
            expires_at_unix_ms: None,
        })
        .unwrap();
        let mapping = MappingVersion::publish(MappingDraft {
            mapping_key: "party_display_name".to_owned(),
            provider_profile_version_id: provider.version_id().clone(),
            provider_response_field_path: "organization.legal_name".to_owned(),
            target_field: TargetField::PartyDisplayName,
            normalization: MappingNormalization::CanonicalPartyDisplayNameV1,
            maximum_suggestions_per_response: 1,
            confidence_required: true,
        })
        .unwrap();
        (provider, mapping)
    }

    fn target() -> TargetSnapshot {
        TargetSnapshot::try_new("party-123", 7, TargetField::PartyDisplayName).unwrap()
    }

    fn request() -> EnrichmentRequest {
        let (provider, mapping) = definitions();
        EnrichmentRequest::create(EnrichmentRequestDraft {
            tenant_id: tenant(),
            requested_by: actor("reviewer-1"),
            idempotency_key: IdempotencyKey::try_new("request-key-1").unwrap(),
            target: target(),
            provider_profile_version_id: provider.version_id().clone(),
            mapping_version_id: mapping.version_id().clone(),
            requested_fields: vec![TargetField::PartyDisplayName],
            policy_evidence: RequestPolicyEvidence::try_new(
                "customer_profile_enrichment",
                "legitimate_interest",
                Some("consent-proof-42".to_owned()),
                "1.0.0",
            )
            .unwrap(),
            created_at_unix_ms: 100,
            deadline_at_unix_ms: 500,
            expires_at_unix_ms: 1_000,
        })
        .unwrap()
    }

    fn receipt(request: &EnrichmentRequest, digest: u8) -> ProviderResponseReceipt {
        ProviderResponseReceipt::record(ProviderResponseReceiptDraft {
            request_id: request.request_id().clone(),
            provider_profile_version_id: request.provider_profile_version_id().clone(),
            mapping_version_id: request.mapping_version_id().clone(),
            replay_key: "provider-request-42".to_owned(),
            provider_correlation_id: Some("provider-correlation-42".to_owned()),
            response_class: ProviderResponseClass::Success,
            canonical_response_digest: [digest; 32],
            provider_observed_at_unix_ms: Some(190),
            retrieved_at_unix_ms: 200,
            metered_units: 1,
            protected_evidence_reference: None,
        })
        .unwrap()
    }

    fn suggestion(request: &EnrichmentRequest, receipt: &ProviderResponseReceipt) -> Suggestion {
        Suggestion::materialize(SuggestionDraft {
            request_id: request.request_id().clone(),
            response_receipt_id: receipt.receipt_id().clone(),
            provider_profile_version_id: request.provider_profile_version_id().clone(),
            mapping_version_id: request.mapping_version_id().clone(),
            target: request.target().clone(),
            proposed_value: "  Acme   Corporation  ".to_owned(),
            observed_at_unix_ms: Some(190),
            retrieved_at_unix_ms: 200,
            effective_at_unix_ms: 200,
            fresh_until_unix_ms: 800,
            expires_at_unix_ms: 1_000,
            confidence_basis_points: Some(9_500),
            purpose_code: "customer_profile_enrichment".to_owned(),
            legal_basis_code: "legitimate_interest".to_owned(),
            license_id: "Registry licence v3".to_owned(),
            permitted_use_class: "customer_master_review".to_owned(),
            residency_region: "eu".to_owned(),
            retention_days: 30,
            consent_evidence_reference: Some("consent-proof-42".to_owned()),
            evidence_references: vec!["provider-row-7".to_owned()],
        })
        .unwrap()
    }

    #[test]
    fn request_identity_is_deterministic_and_lifecycle_is_strict() {
        let first = request();
        let second = request();
        assert_eq!(first.request_id(), second.request_id());

        let mut request = first;
        assert!(request.mark_dispatched(110).is_err());
        request.queue(110).unwrap();
        request.mark_dispatched(120).unwrap();
        assert_eq!(request.status(), EnrichmentRequestStatus::Dispatched);
        request.fail_retryable("provider_unavailable", 130).unwrap();
        request.queue(140).unwrap();
        assert_eq!(request.retry_generation(), 1);
        request.mark_dispatched(150).unwrap();
        assert!(request.complete(160).is_err());
    }

    #[test]
    fn response_replay_is_idempotent_and_conflicting_content_is_rejected() {
        let request = request();
        let first = receipt(&request, 7);
        let duplicate = receipt(&request, 7);
        let conflict = receipt(&request, 8);
        assert_eq!(
            first.reconcile(&duplicate).unwrap(),
            ReplayDisposition::Duplicate
        );
        assert_eq!(first.receipt_id(), conflict.receipt_id());
        assert!(first.reconcile(&conflict).is_err());
    }

    #[test]
    fn cancellation_retains_already_recorded_response_identity() {
        let mut request = request();
        request.queue(110).unwrap();
        request.mark_dispatched(120).unwrap();
        let receipt = receipt(&request, 7);
        request
            .record_response(receipt.receipt_id().clone(), 200)
            .unwrap();
        request.cancel(210).unwrap();
        assert_eq!(request.status(), EnrichmentRequestStatus::Cancelled);
        assert_eq!(request.response_receipt_id(), Some(receipt.receipt_id()));
    }

    #[test]
    fn suggestion_normalization_and_freshness_are_explicit() {
        let request = request();
        let receipt = receipt(&request, 7);
        let suggestion = suggestion(&request, &receipt);
        assert_eq!(suggestion.proposed_value(), "Acme Corporation");
        assert!(suggestion.is_fresh_at(200));
        assert!(!suggestion.is_fresh_at(800));
        assert!(suggestion.is_expired_at(1_000));
    }

    #[test]
    fn accepted_review_requires_approval_when_policy_demands_it() {
        let request = request();
        let receipt = receipt(&request, 7);
        let suggestion = suggestion(&request, &receipt);
        let denied = ReviewDecision::decide(
            &suggestion,
            actor("reviewer-2"),
            ReviewDecisionKind::Accepted,
            "1.0.0",
            "accepted",
            ApprovalRequirement::Required,
            None,
            300,
            Some(700),
        );
        assert!(denied.is_err());

        let accepted = ReviewDecision::decide(
            &suggestion,
            actor("reviewer-2"),
            ReviewDecisionKind::Accepted,
            "1.0.0",
            "accepted",
            ApprovalRequirement::Required,
            Some("approval-42".to_owned()),
            300,
            Some(700),
        )
        .unwrap();
        assert_eq!(accepted.kind(), ReviewDecisionKind::Accepted);
    }

    #[test]
    fn application_attempt_has_stable_target_idempotency_and_append_only_outcome() {
        let request = request();
        let receipt = receipt(&request, 7);
        let suggestion = suggestion(&request, &receipt);
        let decision = ReviewDecision::decide(
            &suggestion,
            actor("reviewer-2"),
            ReviewDecisionKind::Accepted,
            "1.0.0",
            "accepted",
            ApprovalRequirement::NotRequired,
            None,
            300,
            Some(700),
        )
        .unwrap();
        let mut first = ApplicationAttempt::plan(tenant(), &suggestion, &decision, 0, 400).unwrap();
        let second = ApplicationAttempt::plan(tenant(), &suggestion, &decision, 0, 400).unwrap();
        assert_eq!(first.attempt_id(), second.attempt_id());
        assert_eq!(
            first.target_idempotency_key(),
            second.target_idempotency_key()
        );

        let success = ApplicationOutcome::Succeeded {
            business_transaction_id: BusinessTransactionId::try_new("party-update-tx-42").unwrap(),
            resulting_target_version: 8,
        };
        assert_eq!(
            first.record_outcome(success.clone(), 450).unwrap(),
            ReplayDisposition::New
        );
        assert_eq!(
            first.record_outcome(success, 450).unwrap(),
            ReplayDisposition::Duplicate
        );
        assert!(
            first
                .record_outcome(ApplicationOutcome::AuthorizationDenied, 450)
                .is_err()
        );
        assert_eq!(
            derive_suggestion_status(&suggestion, Some(&decision), Some(&first), None, 500),
            SuggestionLifecycleStatus::Applied
        );
    }

    #[test]
    fn stale_review_or_stale_suggestion_cannot_be_applied() {
        let request = request();
        let receipt = receipt(&request, 7);
        let suggestion = suggestion(&request, &receipt);
        let decision = ReviewDecision::decide(
            &suggestion,
            actor("reviewer-2"),
            ReviewDecisionKind::Accepted,
            "1.0.0",
            "accepted",
            ApprovalRequirement::NotRequired,
            None,
            300,
            Some(700),
        )
        .unwrap();
        assert!(ApplicationAttempt::plan(tenant(), &suggestion, &decision, 0, 700).is_err());
        assert!(ApplicationAttempt::plan(tenant(), &suggestion, &decision, 0, 800).is_err());
    }
}
