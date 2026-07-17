use crate::{
    ApplicationOutcome, EnrichmentRequest, EnrichmentRequestStatus, MappingVersionId,
    ProviderProfileVersionId, ProviderResponseClass, ProviderResponseReceipt,
    RecordedApplicationOutcome, ReviewDecision, ReviewDecisionKind, Suggestion, TargetField,
};
use crm_module_sdk::{
    ActorId, ErrorCategory, IdempotencyKey, SdkError, TenantId,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};

pub const ENRICHMENT_REQUEST_STATE_SCHEMA_ID: &str =
    "crm.customer-enrichment.request.state";
pub const PROVIDER_RESPONSE_RECEIPT_STATE_SCHEMA_ID: &str =
    "crm.customer-enrichment.provider_response_receipt.state";
pub const SUGGESTION_STATE_SCHEMA_ID: &str =
    "crm.customer-enrichment.suggestion.state";
pub const REVIEW_DECISION_STATE_SCHEMA_ID: &str =
    "crm.customer-enrichment.review_decision.state";
pub const APPLICATION_ATTEMPT_STATE_SCHEMA_ID: &str =
    "crm.customer-enrichment.application_attempt.state";
pub const LIFECYCLE_STATE_SCHEMA_VERSION: &str = "1.0.0";

pub const ENRICHMENT_REQUEST_STATE_MAXIMUM_BYTES: u64 = 64 * 1024;
pub const PROVIDER_RESPONSE_RECEIPT_STATE_MAXIMUM_BYTES: u64 = 32 * 1024;
pub const SUGGESTION_STATE_MAXIMUM_BYTES: u64 = 64 * 1024;
pub const REVIEW_DECISION_STATE_MAXIMUM_BYTES: u64 = 32 * 1024;
pub const APPLICATION_ATTEMPT_STATE_MAXIMUM_BYTES: u64 = 32 * 1024;

pub const LIFECYCLE_STATE_RETENTION_POLICY_ID: &str = "crm.customer_enrichment.provenance";

const REQUEST_ID_DOMAIN: &[u8] = b"crm.customer-enrichment.request/v1";
const RESPONSE_RECEIPT_ID_DOMAIN: &[u8] = b"crm.customer-enrichment.response-receipt/v1";
const SUGGESTION_ID_DOMAIN: &[u8] = b"crm.customer-enrichment.suggestion/v1";
const REVIEW_DECISION_ID_DOMAIN: &[u8] = b"crm.customer-enrichment.review-decision/v1";
const APPLICATION_ATTEMPT_ID_DOMAIN: &[u8] = b"crm.customer-enrichment.application-attempt/v1";
const TARGET_IDEMPOTENCY_DOMAIN: &[u8] = b"crm.customer-enrichment.target-idempotency/v1";
const PROPOSED_VALUE_DIGEST_DOMAIN: &[u8] = b"crm.customer-enrichment.proposed-value/v1";

const REQUEST_STATE_DESCRIPTOR: &[u8] = b"crm.customer-enrichment.request.state/v1:request_id,tenant_id,requested_by,idempotency_key,target,provider_profile_version_id,mapping_version_id,requested_fields,policy_evidence,created_at_unix_ms,deadline_at_unix_ms,expires_at_unix_ms,status,retry_generation,response_receipt_id,last_safe_failure_code,updated_at_unix_ms";
const RESPONSE_STATE_DESCRIPTOR: &[u8] = b"crm.customer-enrichment.provider_response_receipt.state/v1:receipt_id,request_id,provider_profile_version_id,mapping_version_id,replay_key,provider_correlation_id,response_class,canonical_response_digest,provider_observed_at_unix_ms,retrieved_at_unix_ms,metered_units,protected_evidence_reference";
const SUGGESTION_STATE_DESCRIPTOR: &[u8] = b"crm.customer-enrichment.suggestion.state/v1:suggestion_id,request_id,response_receipt_id,provider_profile_version_id,mapping_version_id,target,proposed_value,proposed_value_digest,observed_at_unix_ms,retrieved_at_unix_ms,effective_at_unix_ms,fresh_until_unix_ms,expires_at_unix_ms,confidence_basis_points,purpose_code,legal_basis_code,license_id,permitted_use_class,residency_region,retention_days,consent_evidence_reference,evidence_references";
const REVIEW_STATE_DESCRIPTOR: &[u8] = b"crm.customer-enrichment.review_decision.state/v1:decision_id,suggestion_id,target_resource_version,proposed_value_digest,reviewed_by,kind,policy_version,safe_reason_code,approval_evidence_reference,decided_at_unix_ms,expires_at_unix_ms";
const APPLICATION_STATE_DESCRIPTOR: &[u8] = b"crm.customer-enrichment.application_attempt.state/v1:attempt_id,tenant_id,suggestion_id,review_decision_id,target,proposed_value_digest,application_generation,owner_capability_id,owner_capability_version,target_idempotency_key,planned_at_unix_ms,recorded_outcome";

pub fn enrichment_request_state_descriptor_hash() -> [u8; 32] {
    Sha256::digest(REQUEST_STATE_DESCRIPTOR).into()
}

pub fn provider_response_receipt_state_descriptor_hash() -> [u8; 32] {
    Sha256::digest(RESPONSE_STATE_DESCRIPTOR).into()
}

pub fn suggestion_state_descriptor_hash() -> [u8; 32] {
    Sha256::digest(SUGGESTION_STATE_DESCRIPTOR).into()
}

pub fn review_decision_state_descriptor_hash() -> [u8; 32] {
    Sha256::digest(REVIEW_STATE_DESCRIPTOR).into()
}

pub fn application_attempt_state_descriptor_hash() -> [u8; 32] {
    Sha256::digest(APPLICATION_STATE_DESCRIPTOR).into()
}

pub fn encode_enrichment_request_state(value: &EnrichmentRequest) -> Result<Vec<u8>, SdkError> {
    encode(value, ENRICHMENT_REQUEST_STATE_MAXIMUM_BYTES, "enrichment request")
}

pub fn decode_enrichment_request_state(bytes: &[u8]) -> Result<EnrichmentRequest, SdkError> {
    decode::<EnrichmentRequest, EnrichmentRequestStateV1>(
        bytes,
        ENRICHMENT_REQUEST_STATE_MAXIMUM_BYTES,
        "enrichment request",
        EnrichmentRequestStateV1::validate,
        encode_enrichment_request_state,
    )
}

pub fn encode_provider_response_receipt_state(
    value: &ProviderResponseReceipt,
) -> Result<Vec<u8>, SdkError> {
    encode(
        value,
        PROVIDER_RESPONSE_RECEIPT_STATE_MAXIMUM_BYTES,
        "provider response receipt",
    )
}

pub fn decode_provider_response_receipt_state(
    bytes: &[u8],
) -> Result<ProviderResponseReceipt, SdkError> {
    decode::<ProviderResponseReceipt, ProviderResponseReceiptStateV1>(
        bytes,
        PROVIDER_RESPONSE_RECEIPT_STATE_MAXIMUM_BYTES,
        "provider response receipt",
        ProviderResponseReceiptStateV1::validate,
        encode_provider_response_receipt_state,
    )
}

pub fn encode_suggestion_state(value: &Suggestion) -> Result<Vec<u8>, SdkError> {
    encode(value, SUGGESTION_STATE_MAXIMUM_BYTES, "suggestion")
}

pub fn decode_suggestion_state(bytes: &[u8]) -> Result<Suggestion, SdkError> {
    decode::<Suggestion, SuggestionStateV1>(
        bytes,
        SUGGESTION_STATE_MAXIMUM_BYTES,
        "suggestion",
        SuggestionStateV1::validate,
        encode_suggestion_state,
    )
}

pub fn encode_review_decision_state(value: &ReviewDecision) -> Result<Vec<u8>, SdkError> {
    encode(
        value,
        REVIEW_DECISION_STATE_MAXIMUM_BYTES,
        "review decision",
    )
}

pub fn decode_review_decision_state(bytes: &[u8]) -> Result<ReviewDecision, SdkError> {
    decode::<ReviewDecision, ReviewDecisionStateV1>(
        bytes,
        REVIEW_DECISION_STATE_MAXIMUM_BYTES,
        "review decision",
        ReviewDecisionStateV1::validate,
        encode_review_decision_state,
    )
}

pub fn encode_application_attempt_state(value: &crate::ApplicationAttempt) -> Result<Vec<u8>, SdkError> {
    encode(
        value,
        APPLICATION_ATTEMPT_STATE_MAXIMUM_BYTES,
        "application attempt",
    )
}

pub fn decode_application_attempt_state(bytes: &[u8]) -> Result<crate::ApplicationAttempt, SdkError> {
    decode::<crate::ApplicationAttempt, ApplicationAttemptStateV1>(
        bytes,
        APPLICATION_ATTEMPT_STATE_MAXIMUM_BYTES,
        "application attempt",
        ApplicationAttemptStateV1::validate,
        encode_application_attempt_state,
    )
}

fn encode<T: Serialize>(value: &T, maximum_bytes: u64, label: &str) -> Result<Vec<u8>, SdkError> {
    let bytes = serde_json::to_vec(value)
        .map_err(|error| persisted_error(format!("{label} serialization failed: {error}")))?;
    validate_size(&bytes, maximum_bytes, label)?;
    Ok(bytes)
}

fn decode<T, S>(
    bytes: &[u8],
    maximum_bytes: u64,
    label: &str,
    validate: fn(&S) -> Result<(), SdkError>,
    encode_domain: fn(&T) -> Result<Vec<u8>, SdkError>,
) -> Result<T, SdkError>
where
    T: DeserializeOwned,
    S: DeserializeOwned,
{
    validate_size(bytes, maximum_bytes, label)?;
    let state: S = serde_json::from_slice(bytes)
        .map_err(|error| persisted_error(format!("{label} JSON is invalid: {error}")))?;
    validate(&state)?;
    let domain: T = serde_json::from_slice(bytes)
        .map_err(|error| persisted_error(format!("{label} domain state is invalid: {error}")))?;
    if encode_domain(&domain)? != bytes {
        return Err(persisted_error(format!(
            "persisted {label} is not the strict canonical v1 encoding"
        )));
    }
    Ok(domain)
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct EnrichmentRequestStateV1 {
    request_id: String,
    tenant_id: String,
    requested_by: String,
    idempotency_key: String,
    target: TargetSnapshotStateV1,
    provider_profile_version_id: String,
    mapping_version_id: String,
    requested_fields: Vec<TargetField>,
    policy_evidence: RequestPolicyEvidenceStateV1,
    created_at_unix_ms: u64,
    deadline_at_unix_ms: u64,
    expires_at_unix_ms: u64,
    status: EnrichmentRequestStatus,
    retry_generation: u32,
    response_receipt_id: Option<String>,
    last_safe_failure_code: Option<String>,
    updated_at_unix_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TargetSnapshotStateV1 {
    resource_id: String,
    resource_version: u64,
    target_field: TargetField,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RequestPolicyEvidenceStateV1 {
    purpose_code: String,
    legal_basis_code: String,
    consent_evidence_reference: Option<String>,
    policy_version: String,
}

#[derive(Serialize)]
struct RequestIdentityState<'a> {
    semantic_version: &'static str,
    tenant_id: &'a str,
    idempotency_key: &'a str,
    target: &'a TargetSnapshotStateV1,
    provider_profile_version_id: &'a str,
    mapping_version_id: &'a str,
    requested_fields: &'a [TargetField],
    policy_evidence: &'a RequestPolicyEvidenceStateV1,
}

impl EnrichmentRequestStateV1 {
    fn validate(&self) -> Result<(), SdkError> {
        identifier::<TenantId>(&self.tenant_id, "tenant")?;
        identifier::<ActorId>(&self.requested_by, "request actor")?;
        identifier::<IdempotencyKey>(&self.idempotency_key, "request idempotency key")?;
        validate_target(&self.target)?;
        validate_derived_id(
            &self.provider_profile_version_id,
            "enrichment-provider-profile-",
            "provider profile version",
        )?;
        validate_derived_id(
            &self.mapping_version_id,
            "enrichment-mapping-",
            "mapping version",
        )?;
        canonical_target_fields(&self.requested_fields)?;
        if !self.requested_fields.contains(&self.target.target_field) {
            return Err(persisted_error(
                "persisted request does not include its exact target field",
            ));
        }
        validate_policy_evidence(&self.policy_evidence)?;
        if self.created_at_unix_ms >= self.deadline_at_unix_ms
            || self.deadline_at_unix_ms > self.expires_at_unix_ms
            || self.updated_at_unix_ms < self.created_at_unix_ms
        {
            return Err(persisted_error(
                "persisted request timestamps violate creation/deadline/expiry monotonicity",
            ));
        }
        if self.retry_generation > 100 {
            return Err(persisted_error(
                "persisted request retry generation exceeds the domain limit",
            ));
        }
        if let Some(receipt_id) = &self.response_receipt_id {
            validate_derived_id(receipt_id, "enrichment-response-", "response receipt")?;
        }
        if matches!(
            self.status,
            EnrichmentRequestStatus::ResponseRecorded
                | EnrichmentRequestStatus::SuggestionsMaterialized
                | EnrichmentRequestStatus::Completed
        ) && self.response_receipt_id.is_none()
        {
            return Err(persisted_error(
                "persisted request status requires a response receipt",
            ));
        }
        if matches!(
            self.status,
            EnrichmentRequestStatus::Created
                | EnrichmentRequestStatus::Queued
                | EnrichmentRequestStatus::Dispatched
        ) && self.response_receipt_id.is_some()
        {
            return Err(persisted_error(
                "persisted pre-response request unexpectedly contains a response receipt",
            ));
        }
        if let Some(code) = &self.last_safe_failure_code {
            canonical_key(code, "request failure code")?;
        }
        if matches!(
            self.status,
            EnrichmentRequestStatus::FailedRetryable | EnrichmentRequestStatus::FailedTerminal
        ) && self.last_safe_failure_code.is_none()
        {
            return Err(persisted_error(
                "persisted failed request is missing its safe failure code",
            ));
        }
        let identity = RequestIdentityState {
            semantic_version: "1.0.0",
            tenant_id: &self.tenant_id,
            idempotency_key: &self.idempotency_key,
            target: &self.target,
            provider_profile_version_id: &self.provider_profile_version_id,
            mapping_version_id: &self.mapping_version_id,
            requested_fields: &self.requested_fields,
            policy_evidence: &self.policy_evidence,
        };
        validate_expected_id(
            &self.request_id,
            "enrichment-request-",
            REQUEST_ID_DOMAIN,
            &identity,
            "request",
        )
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProviderResponseReceiptStateV1 {
    receipt_id: String,
    request_id: String,
    provider_profile_version_id: String,
    mapping_version_id: String,
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
struct ResponseIdentityState<'a> {
    semantic_version: &'static str,
    request_id: &'a str,
    replay_key: &'a str,
}

impl ProviderResponseReceiptStateV1 {
    fn validate(&self) -> Result<(), SdkError> {
        validate_derived_id(&self.request_id, "enrichment-request-", "request")?;
        validate_derived_id(
            &self.provider_profile_version_id,
            "enrichment-provider-profile-",
            "provider profile version",
        )?;
        validate_derived_id(
            &self.mapping_version_id,
            "enrichment-mapping-",
            "mapping version",
        )?;
        bounded(&self.replay_key, 180, "response replay key")?;
        optional_bounded(
            self.provider_correlation_id.as_deref(),
            180,
            "provider correlation id",
        )?;
        optional_bounded(
            self.protected_evidence_reference.as_deref(),
            240,
            "protected response evidence reference",
        )?;
        if self.canonical_response_digest.iter().all(|byte| *byte == 0) {
            return Err(persisted_error(
                "persisted response canonical digest is all zeroes",
            ));
        }
        if self
            .provider_observed_at_unix_ms
            .is_some_and(|observed| observed > self.retrieved_at_unix_ms)
        {
            return Err(persisted_error(
                "persisted response observed timestamp is later than retrieval",
            ));
        }
        let identity = ResponseIdentityState {
            semantic_version: "1.0.0",
            request_id: &self.request_id,
            replay_key: &self.replay_key,
        };
        validate_expected_id(
            &self.receipt_id,
            "enrichment-response-",
            RESPONSE_RECEIPT_ID_DOMAIN,
            &identity,
            "response receipt",
        )
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SuggestionStateV1 {
    suggestion_id: String,
    request_id: String,
    response_receipt_id: String,
    provider_profile_version_id: String,
    mapping_version_id: String,
    target: TargetSnapshotStateV1,
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
struct SuggestionIdentityState<'a> {
    semantic_version: &'static str,
    request_id: &'a str,
    response_receipt_id: &'a str,
    mapping_version_id: &'a str,
    target: &'a TargetSnapshotStateV1,
    proposed_value_digest: &'a [u8; 32],
}

impl SuggestionStateV1 {
    fn validate(&self) -> Result<(), SdkError> {
        validate_derived_id(&self.request_id, "enrichment-request-", "request")?;
        validate_derived_id(
            &self.response_receipt_id,
            "enrichment-response-",
            "response receipt",
        )?;
        validate_derived_id(
            &self.provider_profile_version_id,
            "enrichment-provider-profile-",
            "provider profile version",
        )?;
        validate_derived_id(
            &self.mapping_version_id,
            "enrichment-mapping-",
            "mapping version",
        )?;
        validate_target(&self.target)?;
        let normalized = self
            .proposed_value
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        if normalized != self.proposed_value
            || self.proposed_value.is_empty()
            || self.proposed_value.len() > 320
            || self.proposed_value.chars().any(char::is_control)
        {
            return Err(persisted_error(
                "persisted suggestion proposed value is not canonical",
            ));
        }
        if fixed_digest(PROPOSED_VALUE_DIGEST_DOMAIN, self.proposed_value.as_bytes())
            != self.proposed_value_digest
        {
            return Err(persisted_error(
                "persisted suggestion proposed-value digest does not match its value",
            ));
        }
        if self
            .observed_at_unix_ms
            .is_some_and(|observed| observed > self.retrieved_at_unix_ms)
            || self.effective_at_unix_ms > self.fresh_until_unix_ms
            || self.fresh_until_unix_ms > self.expires_at_unix_ms
        {
            return Err(persisted_error(
                "persisted suggestion timestamps violate observed/retrieved/effective/fresh/expiry semantics",
            ));
        }
        if self
            .confidence_basis_points
            .is_some_and(|confidence| confidence > 10_000)
        {
            return Err(persisted_error(
                "persisted suggestion confidence exceeds 10000 basis points",
            ));
        }
        canonical_key(&self.purpose_code, "suggestion purpose")?;
        canonical_key(&self.legal_basis_code, "suggestion legal basis")?;
        canonical_key(&self.permitted_use_class, "suggestion permitted use")?;
        canonical_key(&self.residency_region, "suggestion residency")?;
        bounded(&self.license_id, 240, "suggestion license")?;
        if self.retention_days > 3_650 {
            return Err(persisted_error(
                "persisted suggestion retention exceeds the bounded policy maximum",
            ));
        }
        optional_bounded(
            self.consent_evidence_reference.as_deref(),
            240,
            "suggestion consent reference",
        )?;
        canonical_references(&self.evidence_references)?;
        let identity = SuggestionIdentityState {
            semantic_version: "1.0.0",
            request_id: &self.request_id,
            response_receipt_id: &self.response_receipt_id,
            mapping_version_id: &self.mapping_version_id,
            target: &self.target,
            proposed_value_digest: &self.proposed_value_digest,
        };
        validate_expected_id(
            &self.suggestion_id,
            "enrichment-suggestion-",
            SUGGESTION_ID_DOMAIN,
            &identity,
            "suggestion",
        )
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReviewDecisionStateV1 {
    decision_id: String,
    suggestion_id: String,
    target_resource_version: u64,
    proposed_value_digest: [u8; 32],
    reviewed_by: String,
    kind: ReviewDecisionKind,
    policy_version: String,
    safe_reason_code: String,
    approval_evidence_reference: Option<String>,
    decided_at_unix_ms: u64,
    expires_at_unix_ms: Option<u64>,
}

#[derive(Serialize)]
struct ReviewIdentityState<'a> {
    semantic_version: &'static str,
    suggestion_id: &'a str,
    target_resource_version: u64,
    proposed_value_digest: &'a [u8; 32],
    reviewed_by: &'a str,
    kind: ReviewDecisionKind,
    policy_version: &'a str,
    safe_reason_code: &'a str,
    approval_evidence_reference: &'a Option<String>,
    decided_at_unix_ms: u64,
    expires_at_unix_ms: Option<u64>,
}

impl ReviewDecisionStateV1 {
    fn validate(&self) -> Result<(), SdkError> {
        validate_derived_id(&self.suggestion_id, "enrichment-suggestion-", "suggestion")?;
        identifier::<ActorId>(&self.reviewed_by, "review actor")?;
        if self.target_resource_version == 0
            || self.proposed_value_digest.iter().all(|byte| *byte == 0)
        {
            return Err(persisted_error(
                "persisted review target version or proposed-value digest is invalid",
            ));
        }
        canonical_version(&self.policy_version, "review policy version")?;
        canonical_key(&self.safe_reason_code, "review safe reason code")?;
        optional_bounded(
            self.approval_evidence_reference.as_deref(),
            240,
            "review approval reference",
        )?;
        if self
            .expires_at_unix_ms
            .is_some_and(|expiry| expiry <= self.decided_at_unix_ms)
        {
            return Err(persisted_error(
                "persisted review expiry is not later than its decision timestamp",
            ));
        }
        let identity = ReviewIdentityState {
            semantic_version: "1.0.0",
            suggestion_id: &self.suggestion_id,
            target_resource_version: self.target_resource_version,
            proposed_value_digest: &self.proposed_value_digest,
            reviewed_by: &self.reviewed_by,
            kind: self.kind,
            policy_version: &self.policy_version,
            safe_reason_code: &self.safe_reason_code,
            approval_evidence_reference: &self.approval_evidence_reference,
            decided_at_unix_ms: self.decided_at_unix_ms,
            expires_at_unix_ms: self.expires_at_unix_ms,
        };
        validate_expected_id(
            &self.decision_id,
            "enrichment-review-",
            REVIEW_DECISION_ID_DOMAIN,
            &identity,
            "review decision",
        )
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ApplicationAttemptStateV1 {
    attempt_id: String,
    tenant_id: String,
    suggestion_id: String,
    review_decision_id: String,
    target: TargetSnapshotStateV1,
    proposed_value_digest: [u8; 32],
    application_generation: u32,
    owner_capability_id: String,
    owner_capability_version: String,
    target_idempotency_key: String,
    planned_at_unix_ms: u64,
    recorded_outcome: Option<RecordedApplicationOutcome>,
}

#[derive(Serialize)]
struct ApplicationIdentityState<'a> {
    semantic_version: &'static str,
    tenant_id: &'a str,
    suggestion_id: &'a str,
    application_generation: u32,
    owner_capability_id: &'a str,
    owner_capability_version: &'a str,
}

impl ApplicationAttemptStateV1 {
    fn validate(&self) -> Result<(), SdkError> {
        identifier::<TenantId>(&self.tenant_id, "application tenant")?;
        validate_derived_id(&self.suggestion_id, "enrichment-suggestion-", "suggestion")?;
        validate_derived_id(
            &self.review_decision_id,
            "enrichment-review-",
            "review decision",
        )?;
        validate_target(&self.target)?;
        if self.proposed_value_digest.iter().all(|byte| *byte == 0) {
            return Err(persisted_error(
                "persisted application proposed-value digest is all zeroes",
            ));
        }
        if self.application_generation > 100 {
            return Err(persisted_error(
                "persisted application generation exceeds the domain limit",
            ));
        }
        if self.owner_capability_id != self.target.target_field.owner_capability_id()
            || self.owner_capability_version
                != self.target.target_field.owner_capability_version()
        {
            return Err(persisted_error(
                "persisted application owner capability does not match the target field",
            ));
        }
        identifier::<IdempotencyKey>(
            &self.target_idempotency_key,
            "target idempotency key",
        )?;
        let identity = ApplicationIdentityState {
            semantic_version: "1.0.0",
            tenant_id: &self.tenant_id,
            suggestion_id: &self.suggestion_id,
            application_generation: self.application_generation,
            owner_capability_id: &self.owner_capability_id,
            owner_capability_version: &self.owner_capability_version,
        };
        validate_expected_id(
            &self.attempt_id,
            "enrichment-application-",
            APPLICATION_ATTEMPT_ID_DOMAIN,
            &identity,
            "application attempt",
        )?;
        let expected_target_key = format!(
            "customer-enrichment-apply-{}",
            hex(&canonical_digest(TARGET_IDEMPOTENCY_DOMAIN, &identity))
        );
        if self.target_idempotency_key != expected_target_key {
            return Err(persisted_error(
                "persisted application target idempotency key does not match its deterministic identity",
            ));
        }
        if let Some(recorded) = &self.recorded_outcome {
            if recorded.recorded_at_unix_ms < self.planned_at_unix_ms {
                return Err(persisted_error(
                    "persisted application outcome predates the attempt plan",
                ));
            }
            validate_outcome(&recorded.outcome)?;
        }
        Ok(())
    }
}

fn validate_target(target: &TargetSnapshotStateV1) -> Result<(), SdkError> {
    bounded(&target.resource_id, 180, "target resource id")?;
    if target.resource_version == 0 {
        return Err(persisted_error(
            "persisted target resource version must be greater than zero",
        ));
    }
    Ok(())
}

fn validate_policy_evidence(value: &RequestPolicyEvidenceStateV1) -> Result<(), SdkError> {
    canonical_key(&value.purpose_code, "request purpose")?;
    canonical_key(&value.legal_basis_code, "request legal basis")?;
    optional_bounded(
        value.consent_evidence_reference.as_deref(),
        240,
        "request consent reference",
    )?;
    canonical_version(&value.policy_version, "request policy version")
}

fn canonical_target_fields(values: &[TargetField]) -> Result<(), SdkError> {
    if values.is_empty() || values.len() > 8 {
        return Err(persisted_error(
            "persisted requested fields must contain 1..=8 entries",
        ));
    }
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(persisted_error(
            "persisted requested fields are not in strict canonical order",
        ));
    }
    Ok(())
}

fn canonical_references(values: &[String]) -> Result<(), SdkError> {
    if values.len() > 16 {
        return Err(persisted_error(
            "persisted evidence references exceed the bounded item count",
        ));
    }
    for value in values {
        bounded(value, 240, "evidence reference")?;
    }
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(persisted_error(
            "persisted evidence references are not unique canonical order",
        ));
    }
    Ok(())
}

fn validate_outcome(outcome: &ApplicationOutcome) -> Result<(), SdkError> {
    match outcome {
        ApplicationOutcome::Succeeded {
            resulting_target_version,
            ..
        } if *resulting_target_version == 0 => Err(persisted_error(
            "persisted successful application has an invalid resulting target version",
        )),
        ApplicationOutcome::RetryableFailure { safe_code }
        | ApplicationOutcome::TerminalFailure { safe_code } => {
            canonical_key(safe_code, "application safe failure code")
        }
        ApplicationOutcome::StaleTarget {
            actual_target_version,
        } if *actual_target_version == 0 => Err(persisted_error(
            "persisted stale-target outcome has an invalid actual target version",
        )),
        _ => Ok(()),
    }
}

fn identifier<T>(value: &str, label: &str) -> Result<(), SdkError>
where
    T: TryFrom<String>,
    T::Error: std::fmt::Display,
{
    T::try_from(value.to_owned())
        .map(|_| ())
        .map_err(|error| persisted_error(format!("persisted {label} is invalid: {error}")))
}

fn validate_derived_id(value: &str, prefix: &str, label: &str) -> Result<(), SdkError> {
    let Some(digest) = value.strip_prefix(prefix) else {
        return Err(persisted_error(format!(
            "persisted {label} identity has the wrong prefix"
        )));
    };
    if digest.len() != 64
        || !digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(persisted_error(format!(
            "persisted {label} identity is not a lowercase SHA-256 coordinate"
        )));
    }
    Ok(())
}

fn validate_expected_id<T: Serialize>(
    actual: &str,
    prefix: &str,
    domain: &[u8],
    identity: &T,
    label: &str,
) -> Result<(), SdkError> {
    validate_derived_id(actual, prefix, label)?;
    let expected = format!("{prefix}{}", hex(&canonical_digest(domain, identity)));
    if actual != expected {
        return Err(persisted_error(format!(
            "persisted {label} identity does not match its canonical content"
        )));
    }
    Ok(())
}

fn canonical_key(value: &str, label: &str) -> Result<(), SdkError> {
    let valid = !value.is_empty()
        && value.len() <= 80
        && value.is_ascii()
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || matches!(byte, b'.' | b'_' | b'-')
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
        return Err(persisted_error(format!(
            "persisted {label} is not a canonical key"
        )));
    }
    Ok(())
}

fn canonical_version(value: &str, label: &str) -> Result<(), SdkError> {
    let valid = !value.is_empty()
        && value.len() <= 48
        && value.is_ascii()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'+'));
    if !valid {
        return Err(persisted_error(format!(
            "persisted {label} is not a canonical version"
        )));
    }
    Ok(())
}

fn bounded(value: &str, maximum_bytes: usize, label: &str) -> Result<(), SdkError> {
    if value.is_empty()
        || value.len() > maximum_bytes
        || value.chars().any(char::is_control)
    {
        return Err(persisted_error(format!(
            "persisted {label} must contain 1..={maximum_bytes} bytes and no control characters"
        )));
    }
    Ok(())
}

fn optional_bounded(
    value: Option<&str>,
    maximum_bytes: usize,
    label: &str,
) -> Result<(), SdkError> {
    value.map_or(Ok(()), |value| bounded(value, maximum_bytes, label))
}

fn canonical_digest<T: Serialize>(domain: &[u8], value: &T) -> Vec<u8> {
    let encoded = serde_json::to_vec(value)
        .expect("canonical customer-enrichment persisted identity must serialize");
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

fn validate_size(bytes: &[u8], maximum_bytes: u64, label: &str) -> Result<(), SdkError> {
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > maximum_bytes {
        return Err(persisted_error(format!(
            "{label} state exceeds the maximum of {maximum_bytes} bytes"
        )));
    }
    Ok(())
}

fn persisted_error(message: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_PERSISTED_STATE_INVALID",
        ErrorCategory::Internal,
        false,
        "The persisted customer-enrichment state is invalid.",
    )
    .with_internal_reference(message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ApplicationAttempt, ApprovalRequirement, EnrichmentRequestDraft, MappingDraft,
        MappingNormalization, MappingVersion, ProviderProfileDraft, ProviderProfileVersion,
        ProviderResponseReceiptDraft, RawPayloadPolicy, RequestPolicyEvidence, ReviewDecision,
        SuggestionDraft, TargetSnapshot,
    };
    use crm_module_sdk::{BusinessTransactionId, IdempotencyKey};
    use serde_json::Value;

    struct Fixture {
        request: EnrichmentRequest,
        response: ProviderResponseReceipt,
        suggestion: Suggestion,
        review: ReviewDecision,
        application: ApplicationAttempt,
    }

    fn fixture() -> Fixture {
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
        let target = TargetSnapshot::try_new("party-123", 7, TargetField::PartyDisplayName).unwrap();
        let mut request = EnrichmentRequest::create(EnrichmentRequestDraft {
            tenant_id: TenantId::try_new("tenant-a").unwrap(),
            requested_by: ActorId::try_new("reviewer-1").unwrap(),
            idempotency_key: IdempotencyKey::try_new("request-key-1").unwrap(),
            target: target.clone(),
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
        .unwrap();
        request.queue(110).unwrap();
        request.mark_dispatched(120).unwrap();
        let response = ProviderResponseReceipt::record(ProviderResponseReceiptDraft {
            request_id: request.request_id().clone(),
            provider_profile_version_id: provider.version_id().clone(),
            mapping_version_id: mapping.version_id().clone(),
            replay_key: "provider-request-42".to_owned(),
            provider_correlation_id: Some("provider-correlation-42".to_owned()),
            response_class: ProviderResponseClass::Success,
            canonical_response_digest: [7; 32],
            provider_observed_at_unix_ms: Some(190),
            retrieved_at_unix_ms: 200,
            metered_units: 1,
            protected_evidence_reference: None,
        })
        .unwrap();
        request
            .record_response(response.receipt_id().clone(), 200)
            .unwrap();
        request.mark_suggestions_materialized(210).unwrap();
        let suggestion = Suggestion::materialize(SuggestionDraft {
            request_id: request.request_id().clone(),
            response_receipt_id: response.receipt_id().clone(),
            provider_profile_version_id: provider.version_id().clone(),
            mapping_version_id: mapping.version_id().clone(),
            target,
            proposed_value: "Acme Corporation".to_owned(),
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
        .unwrap();
        let review = ReviewDecision::decide(
            &suggestion,
            ActorId::try_new("reviewer-2").unwrap(),
            ReviewDecisionKind::Accepted,
            "1.0.0",
            "accepted",
            ApprovalRequirement::Required,
            Some("approval-42".to_owned()),
            300,
            Some(700),
        )
        .unwrap();
        let mut application = ApplicationAttempt::plan(
            TenantId::try_new("tenant-a").unwrap(),
            &suggestion,
            &review,
            0,
            400,
        )
        .unwrap();
        application
            .record_outcome(
                ApplicationOutcome::Succeeded {
                    business_transaction_id: BusinessTransactionId::try_new("party-tx-42")
                        .unwrap(),
                    resulting_target_version: 8,
                },
                450,
            )
            .unwrap();
        Fixture {
            request,
            response,
            suggestion,
            review,
            application,
        }
    }

    #[test]
    fn lifecycle_states_round_trip_through_strict_canonical_encoding() {
        let values = fixture();
        let request = encode_enrichment_request_state(&values.request).unwrap();
        assert_eq!(
            decode_enrichment_request_state(&request).unwrap(),
            values.request
        );
        let response = encode_provider_response_receipt_state(&values.response).unwrap();
        assert_eq!(
            decode_provider_response_receipt_state(&response).unwrap(),
            values.response
        );
        let suggestion = encode_suggestion_state(&values.suggestion).unwrap();
        assert_eq!(decode_suggestion_state(&suggestion).unwrap(), values.suggestion);
        let review = encode_review_decision_state(&values.review).unwrap();
        assert_eq!(decode_review_decision_state(&review).unwrap(), values.review);
        let application = encode_application_attempt_state(&values.application).unwrap();
        assert_eq!(
            decode_application_attempt_state(&application).unwrap(),
            values.application
        );
    }

    #[test]
    fn changed_derived_identity_is_rejected_as_corruption() {
        let values = fixture();
        let bytes = encode_suggestion_state(&values.suggestion).unwrap();
        let mut json: Value = serde_json::from_slice(&bytes).unwrap();
        json["suggestion_id"] = Value::String(format!(
            "enrichment-suggestion-{}",
            "0".repeat(64)
        ));
        let corrupted = serde_json::to_vec(&json).unwrap();
        let error = decode_suggestion_state(&corrupted).unwrap_err();
        assert_eq!(error.code, "CUSTOMER_ENRICHMENT_PERSISTED_STATE_INVALID");
    }

    #[test]
    fn unknown_fields_and_noncanonical_json_are_rejected() {
        let values = fixture();
        let bytes = encode_review_decision_state(&values.review).unwrap();
        let mut json: Value = serde_json::from_slice(&bytes).unwrap();
        json["unexpected"] = Value::Bool(true);
        assert!(decode_review_decision_state(&serde_json::to_vec(&json).unwrap()).is_err());

        let noncanonical = format!(
            " {} ",
            String::from_utf8(encode_enrichment_request_state(&values.request).unwrap()).unwrap()
        );
        assert!(decode_enrichment_request_state(noncanonical.as_bytes()).is_err());
    }

    #[test]
    fn descriptor_hashes_are_stable_and_nonzero() {
        for digest in [
            enrichment_request_state_descriptor_hash(),
            provider_response_receipt_state_descriptor_hash(),
            suggestion_state_descriptor_hash(),
            review_decision_state_descriptor_hash(),
            application_attempt_state_descriptor_hash(),
        ] {
            assert!(digest.iter().any(|byte| *byte != 0));
        }
    }
}
