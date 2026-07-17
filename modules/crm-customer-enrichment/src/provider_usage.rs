use crate::{EnrichmentRequestId, ProviderProfileVersionId, ProviderResponseReceiptId};
use crm_module_sdk::{ErrorCategory, SdkError};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const PROVIDER_USAGE_ENTRY_STATE_SCHEMA_ID: &str =
    "crm.customer-enrichment.provider_usage_entry.state";
pub const PROVIDER_USAGE_ENTRY_STATE_SCHEMA_VERSION: &str = "1.0.0";
pub const PROVIDER_USAGE_ENTRY_STATE_MAXIMUM_BYTES: u64 = 32 * 1024;
pub const PROVIDER_USAGE_ENTRY_STATE_RETENTION_POLICY_ID: &str =
    "crm.customer_enrichment.provider_usage";

const PROVIDER_USAGE_ENTRY_ID_DOMAIN: &[u8] = b"crm.customer-enrichment.provider-usage-entry/v1";
const PROVIDER_USAGE_ENTRY_STATE_DESCRIPTOR: &[u8] = b"crm.customer-enrichment.provider_usage_entry.state/v1:usage_entry_id,request_id,response_receipt_id,provider_profile_version_id,kind,metered_units,quota_bucket,quota_remaining,provider_observed_at_unix_ms,recorded_at_unix_ms,safe_provider_code";

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ProviderUsageEntryId(String);

impl ProviderUsageEntryId {
    fn from_digest(digest: &[u8]) -> Self {
        Self(format!("enrichment-provider-usage-{}", hex(digest)))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderUsageKind {
    RequestDispatched,
    ResponseReceived,
    BillableUnits,
    QuotaSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderUsageEntryDraft {
    pub request_id: EnrichmentRequestId,
    pub response_receipt_id: Option<ProviderResponseReceiptId>,
    pub provider_profile_version_id: ProviderProfileVersionId,
    pub kind: ProviderUsageKind,
    pub metered_units: u64,
    pub quota_bucket: Option<String>,
    pub quota_remaining: Option<u64>,
    pub provider_observed_at_unix_ms: Option<u64>,
    pub recorded_at_unix_ms: u64,
    pub safe_provider_code: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderUsageEntry {
    usage_entry_id: ProviderUsageEntryId,
    request_id: EnrichmentRequestId,
    response_receipt_id: Option<ProviderResponseReceiptId>,
    provider_profile_version_id: ProviderProfileVersionId,
    kind: ProviderUsageKind,
    metered_units: u64,
    quota_bucket: Option<String>,
    quota_remaining: Option<u64>,
    provider_observed_at_unix_ms: Option<u64>,
    recorded_at_unix_ms: u64,
    safe_provider_code: Option<String>,
}

#[derive(Serialize)]
struct ProviderUsageIdentity<'a> {
    semantic_version: &'static str,
    request_id: &'a EnrichmentRequestId,
    response_receipt_id: &'a Option<ProviderResponseReceiptId>,
    provider_profile_version_id: &'a ProviderProfileVersionId,
    kind: ProviderUsageKind,
    metered_units: u64,
    quota_bucket: &'a Option<String>,
    quota_remaining: Option<u64>,
    provider_observed_at_unix_ms: Option<u64>,
    recorded_at_unix_ms: u64,
    safe_provider_code: &'a Option<String>,
}

impl ProviderUsageEntry {
    pub fn record(draft: ProviderUsageEntryDraft) -> Result<Self, SdkError> {
        let quota_bucket = draft
            .quota_bucket
            .map(|value| canonical_key(value, "provider_usage.quota_bucket"))
            .transpose()?;
        let safe_provider_code = draft
            .safe_provider_code
            .map(|value| canonical_key(value, "provider_usage.safe_provider_code"))
            .transpose()?;
        let mut entry = Self {
            usage_entry_id: ProviderUsageEntryId(String::new()),
            request_id: draft.request_id,
            response_receipt_id: draft.response_receipt_id,
            provider_profile_version_id: draft.provider_profile_version_id,
            kind: draft.kind,
            metered_units: draft.metered_units,
            quota_bucket,
            quota_remaining: draft.quota_remaining,
            provider_observed_at_unix_ms: draft.provider_observed_at_unix_ms,
            recorded_at_unix_ms: draft.recorded_at_unix_ms,
            safe_provider_code,
        };
        entry.validate_semantics()?;
        entry.usage_entry_id = ProviderUsageEntryId::from_digest(&canonical_digest(
            PROVIDER_USAGE_ENTRY_ID_DOMAIN,
            &entry.identity(),
        ));
        Ok(entry)
    }

    fn identity(&self) -> ProviderUsageIdentity<'_> {
        ProviderUsageIdentity {
            semantic_version: "1.0.0",
            request_id: &self.request_id,
            response_receipt_id: &self.response_receipt_id,
            provider_profile_version_id: &self.provider_profile_version_id,
            kind: self.kind,
            metered_units: self.metered_units,
            quota_bucket: &self.quota_bucket,
            quota_remaining: self.quota_remaining,
            provider_observed_at_unix_ms: self.provider_observed_at_unix_ms,
            recorded_at_unix_ms: self.recorded_at_unix_ms,
            safe_provider_code: &self.safe_provider_code,
        }
    }

    fn validate_semantics(&self) -> Result<(), SdkError> {
        if self
            .provider_observed_at_unix_ms
            .is_some_and(|observed| observed > self.recorded_at_unix_ms)
        {
            return Err(invalid(
                "CUSTOMER_ENRICHMENT_PROVIDER_USAGE_TIME_INVALID",
                "provider_usage.provider_observed_at_unix_ms",
                "provider observation must not be later than durable recording",
            ));
        }
        if self.quota_remaining.is_some() && self.quota_bucket.is_none() {
            return Err(invalid(
                "CUSTOMER_ENRICHMENT_PROVIDER_QUOTA_BUCKET_REQUIRED",
                "provider_usage.quota_bucket",
                "quota remaining requires an exact quota bucket",
            ));
        }
        match self.kind {
            ProviderUsageKind::RequestDispatched => {
                if self.response_receipt_id.is_some() {
                    return Err(invalid(
                        "CUSTOMER_ENRICHMENT_PROVIDER_USAGE_RESPONSE_UNEXPECTED",
                        "provider_usage.response_receipt_id",
                        "request-dispatch usage must not reference a response receipt",
                    ));
                }
            }
            ProviderUsageKind::ResponseReceived => {
                if self.response_receipt_id.is_none() {
                    return Err(invalid(
                        "CUSTOMER_ENRICHMENT_PROVIDER_USAGE_RESPONSE_REQUIRED",
                        "provider_usage.response_receipt_id",
                        "response usage requires the exact response receipt",
                    ));
                }
            }
            ProviderUsageKind::BillableUnits => {
                if self.response_receipt_id.is_none() || self.metered_units == 0 {
                    return Err(invalid(
                        "CUSTOMER_ENRICHMENT_PROVIDER_BILLING_EVIDENCE_INVALID",
                        "provider_usage.metered_units",
                        "billable usage requires a response receipt and non-zero metered units",
                    ));
                }
            }
            ProviderUsageKind::QuotaSnapshot => {
                if self.quota_bucket.is_none() || self.quota_remaining.is_none() {
                    return Err(invalid(
                        "CUSTOMER_ENRICHMENT_PROVIDER_QUOTA_EVIDENCE_INVALID",
                        "provider_usage.quota_remaining",
                        "quota snapshots require an exact bucket and remaining value",
                    ));
                }
            }
        }
        Ok(())
    }

    fn validate_persisted(&self) -> Result<(), SdkError> {
        self.validate_semantics().map_err(persisted_domain_error)?;
        if self.quota_bucket.as_ref().is_some_and(|value| {
            canonical_key(value.clone(), "provider_usage.quota_bucket").is_err()
        }) || self.safe_provider_code.as_ref().is_some_and(|value| {
            canonical_key(value.clone(), "provider_usage.safe_provider_code").is_err()
        }) {
            return Err(persisted_error(
                "persisted provider usage contains non-canonical bounded codes",
            ));
        }
        let expected = ProviderUsageEntryId::from_digest(&canonical_digest(
            PROVIDER_USAGE_ENTRY_ID_DOMAIN,
            &self.identity(),
        ));
        if self.usage_entry_id != expected {
            return Err(persisted_error(
                "persisted provider usage identity does not match canonical content",
            ));
        }
        Ok(())
    }

    pub fn usage_entry_id(&self) -> &ProviderUsageEntryId {
        &self.usage_entry_id
    }

    pub fn request_id(&self) -> &EnrichmentRequestId {
        &self.request_id
    }

    pub const fn kind(&self) -> ProviderUsageKind {
        self.kind
    }

    pub const fn metered_units(&self) -> u64 {
        self.metered_units
    }

    pub const fn quota_remaining(&self) -> Option<u64> {
        self.quota_remaining
    }
}

pub fn provider_usage_entry_state_descriptor_hash() -> [u8; 32] {
    Sha256::digest(PROVIDER_USAGE_ENTRY_STATE_DESCRIPTOR).into()
}

pub fn encode_provider_usage_entry_state(value: &ProviderUsageEntry) -> Result<Vec<u8>, SdkError> {
    let bytes = serde_json::to_vec(value).map_err(|error| {
        persisted_error(format!("provider usage serialization failed: {error}"))
    })?;
    validate_size(&bytes)?;
    Ok(bytes)
}

pub fn decode_provider_usage_entry_state(bytes: &[u8]) -> Result<ProviderUsageEntry, SdkError> {
    validate_size(bytes)?;
    let value: ProviderUsageEntry = serde_json::from_slice(bytes)
        .map_err(|error| persisted_error(format!("provider usage JSON is invalid: {error}")))?;
    value.validate_persisted()?;
    if encode_provider_usage_entry_state(&value)? != bytes {
        return Err(persisted_error(
            "persisted provider usage is not the strict canonical v1 encoding",
        ));
    }
    Ok(value)
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
            "CUSTOMER_ENRICHMENT_PROVIDER_USAGE_CODE_INVALID",
            field,
            "provider usage codes must be bounded lowercase ASCII canonical keys",
        ));
    }
    Ok(value)
}

fn canonical_digest<T: Serialize>(domain: &[u8], value: &T) -> Vec<u8> {
    let encoded = serde_json::to_vec(value)
        .expect("canonical customer-enrichment provider usage must serialize");
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
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > PROVIDER_USAGE_ENTRY_STATE_MAXIMUM_BYTES {
        return Err(persisted_error(format!(
            "provider usage state exceeds the maximum of {PROVIDER_USAGE_ENTRY_STATE_MAXIMUM_BYTES} bytes"
        )));
    }
    Ok(())
}

fn invalid(code: &'static str, field: &'static str, safe_message: impl Into<String>) -> SdkError {
    let mut error = SdkError::new(
        code,
        ErrorCategory::InvalidArgument,
        false,
        "The customer-enrichment provider usage evidence is invalid.",
    );
    error.field_violations.push(crm_module_sdk::FieldViolation {
        field: crm_module_sdk::FieldName::try_new(field)
            .expect("static provider usage field path must be valid"),
        code: code.to_owned(),
        safe_message: safe_message.into(),
    });
    error
}

fn persisted_domain_error(error: SdkError) -> SdkError {
    persisted_error(format!(
        "provider usage failed domain validation: {}: {}",
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
    .with_internal_reference(message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        EnrichmentRequestDraft, MappingDraft, MappingNormalization, MappingVersion,
        ProviderProfileDraft, ProviderProfileVersion, ProviderResponseClass,
        ProviderResponseReceipt, ProviderResponseReceiptDraft, RawPayloadPolicy,
        RequestPolicyEvidence, TargetSnapshot,
    };
    use crm_module_sdk::{ActorId, IdempotencyKey, TenantId};
    use serde_json::Value;

    fn provider() -> ProviderProfileVersion {
        ProviderProfileVersion::publish(ProviderProfileDraft {
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
        .unwrap()
    }

    fn request_and_response(
        provider: &ProviderProfileVersion,
    ) -> (crate::EnrichmentRequest, ProviderResponseReceipt) {
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
        let request = crate::EnrichmentRequest::create(EnrichmentRequestDraft {
            tenant_id: TenantId::try_new("tenant-a").unwrap(),
            requested_by: ActorId::try_new("worker-1").unwrap(),
            idempotency_key: IdempotencyKey::try_new("request-usage-1").unwrap(),
            target: TargetSnapshot::try_new("party-123", 7, TargetField::PartyDisplayName).unwrap(),
            provider_profile_version_id: provider.version_id().clone(),
            mapping_version_id: mapping.version_id().clone(),
            requested_fields: vec![TargetField::PartyDisplayName],
            policy_evidence: RequestPolicyEvidence::try_new(
                "customer_profile_enrichment",
                "legitimate_interest",
                None,
                "1.0.0",
            )
            .unwrap(),
            created_at_unix_ms: 100,
            deadline_at_unix_ms: 500,
            expires_at_unix_ms: 1_000,
        })
        .unwrap();
        let response = ProviderResponseReceipt::record(ProviderResponseReceiptDraft {
            request_id: request.request_id().clone(),
            provider_profile_version_id: provider.version_id().clone(),
            mapping_version_id: mapping.version_id().clone(),
            replay_key: "provider-request-usage-1".to_owned(),
            provider_correlation_id: None,
            response_class: ProviderResponseClass::Success,
            canonical_response_digest: [9; 32],
            provider_observed_at_unix_ms: Some(190),
            retrieved_at_unix_ms: 200,
            metered_units: 3,
            protected_evidence_reference: None,
        })
        .unwrap();
        (request, response)
    }

    #[test]
    fn billable_usage_identity_is_deterministic_and_persistence_is_strict() {
        let provider = provider();
        let (request, response) = request_and_response(&provider);
        let draft = || ProviderUsageEntryDraft {
            request_id: request.request_id().clone(),
            response_receipt_id: Some(response.receipt_id().clone()),
            provider_profile_version_id: provider.version_id().clone(),
            kind: ProviderUsageKind::BillableUnits,
            metered_units: 3,
            quota_bucket: Some("daily_company_registry".to_owned()),
            quota_remaining: Some(997),
            provider_observed_at_unix_ms: Some(190),
            recorded_at_unix_ms: 205,
            safe_provider_code: Some("success".to_owned()),
        };
        let first = ProviderUsageEntry::record(draft()).unwrap();
        let second = ProviderUsageEntry::record(draft()).unwrap();
        assert_eq!(first.usage_entry_id(), second.usage_entry_id());
        let bytes = encode_provider_usage_entry_state(&first).unwrap();
        assert_eq!(decode_provider_usage_entry_state(&bytes).unwrap(), first);
    }

    #[test]
    fn invalid_usage_shapes_and_corrupted_identity_are_rejected() {
        let provider = provider();
        let (request, response) = request_and_response(&provider);
        assert!(
            ProviderUsageEntry::record(ProviderUsageEntryDraft {
                request_id: request.request_id().clone(),
                response_receipt_id: None,
                provider_profile_version_id: provider.version_id().clone(),
                kind: ProviderUsageKind::BillableUnits,
                metered_units: 3,
                quota_bucket: None,
                quota_remaining: None,
                provider_observed_at_unix_ms: None,
                recorded_at_unix_ms: 205,
                safe_provider_code: None,
            })
            .is_err()
        );

        let entry = ProviderUsageEntry::record(ProviderUsageEntryDraft {
            request_id: request.request_id().clone(),
            response_receipt_id: Some(response.receipt_id().clone()),
            provider_profile_version_id: provider.version_id().clone(),
            kind: ProviderUsageKind::ResponseReceived,
            metered_units: 0,
            quota_bucket: None,
            quota_remaining: None,
            provider_observed_at_unix_ms: Some(190),
            recorded_at_unix_ms: 205,
            safe_provider_code: None,
        })
        .unwrap();
        let mut json: Value =
            serde_json::from_slice(&encode_provider_usage_entry_state(&entry).unwrap()).unwrap();
        json["usage_entry_id"] =
            Value::String(format!("enrichment-provider-usage-{}", "0".repeat(64)));
        assert!(decode_provider_usage_entry_state(&serde_json::to_vec(&json).unwrap()).is_err());
    }

    #[test]
    fn quota_snapshot_requires_exact_bucket_and_remaining_value() {
        let provider = provider();
        let (request, _) = request_and_response(&provider);
        let entry = ProviderUsageEntry::record(ProviderUsageEntryDraft {
            request_id: request.request_id().clone(),
            response_receipt_id: None,
            provider_profile_version_id: provider.version_id().clone(),
            kind: ProviderUsageKind::QuotaSnapshot,
            metered_units: 0,
            quota_bucket: Some("daily_company_registry".to_owned()),
            quota_remaining: Some(1_000),
            provider_observed_at_unix_ms: Some(180),
            recorded_at_unix_ms: 200,
            safe_provider_code: None,
        })
        .unwrap();
        assert_eq!(entry.kind(), ProviderUsageKind::QuotaSnapshot);
        assert_eq!(entry.quota_remaining(), Some(1_000));
        assert!(
            provider_usage_entry_state_descriptor_hash()
                .iter()
                .any(|byte| *byte != 0)
        );
    }
}
