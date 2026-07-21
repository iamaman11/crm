use crm_module_sdk::{ErrorCategory, EventId, SdkError};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const PROVIDER_PROCESS_PROJECTION_ID: &str = "customer-enrichment-provider-process-v1";
pub const PROVIDER_PROCESS_OUTCOME_RESOURCE_TYPE: &str =
    "customer_enrichment.provider_process_outcome";
pub const PROVIDER_PROCESS_OUTCOME_SCHEMA_VERSION: u32 = 1;

const REQUEST_PREFIX: &str = "enrichment-request-";
const RECEIPT_PREFIX: &str = "enrichment-response-";
const CONFLICT_PREFIX: &str = "enrichment-response-conflict-";
const MAX_RETRY_GENERATION: u32 = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderProcessOutcomeKind {
    ResponseRecorded,
    RetainFirstReceipt,
    RejectRequest,
    Skipped,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderProcessCanonicalOutcome {
    schema_version: u32,
    request_id: String,
    retry_generation: u32,
    kind: ProviderProcessOutcomeKind,
    provider_response_receipt_id: Option<String>,
    provider_response_conflict_id: Option<String>,
    source_created_event_id: String,
}

impl ProviderProcessCanonicalOutcome {
    pub fn response_recorded(
        request_id: impl Into<String>,
        retry_generation: u32,
        provider_response_receipt_id: impl Into<String>,
        source_created_event_id: impl Into<String>,
    ) -> Result<Self, SdkError> {
        Self::new(
            request_id,
            retry_generation,
            ProviderProcessOutcomeKind::ResponseRecorded,
            Some(provider_response_receipt_id.into()),
            None,
            source_created_event_id,
        )
    }

    pub fn retain_first_receipt(
        request_id: impl Into<String>,
        retry_generation: u32,
        provider_response_receipt_id: impl Into<String>,
        provider_response_conflict_id: impl Into<String>,
        source_created_event_id: impl Into<String>,
    ) -> Result<Self, SdkError> {
        Self::new(
            request_id,
            retry_generation,
            ProviderProcessOutcomeKind::RetainFirstReceipt,
            Some(provider_response_receipt_id.into()),
            Some(provider_response_conflict_id.into()),
            source_created_event_id,
        )
    }

    pub fn reject_request(
        request_id: impl Into<String>,
        retry_generation: u32,
        provider_response_receipt_id: impl Into<String>,
        provider_response_conflict_id: impl Into<String>,
        source_created_event_id: impl Into<String>,
    ) -> Result<Self, SdkError> {
        Self::new(
            request_id,
            retry_generation,
            ProviderProcessOutcomeKind::RejectRequest,
            Some(provider_response_receipt_id.into()),
            Some(provider_response_conflict_id.into()),
            source_created_event_id,
        )
    }

    pub fn skipped(
        request_id: impl Into<String>,
        retry_generation: u32,
        source_created_event_id: impl Into<String>,
    ) -> Result<Self, SdkError> {
        Self::new(
            request_id,
            retry_generation,
            ProviderProcessOutcomeKind::Skipped,
            None,
            None,
            source_created_event_id,
        )
    }

    fn new(
        request_id: impl Into<String>,
        retry_generation: u32,
        kind: ProviderProcessOutcomeKind,
        provider_response_receipt_id: Option<String>,
        provider_response_conflict_id: Option<String>,
        source_created_event_id: impl Into<String>,
    ) -> Result<Self, SdkError> {
        let outcome = Self {
            schema_version: PROVIDER_PROCESS_OUTCOME_SCHEMA_VERSION,
            request_id: request_id.into(),
            retry_generation,
            kind,
            provider_response_receipt_id,
            provider_response_conflict_id,
            source_created_event_id: source_created_event_id.into(),
        };
        outcome.validate()?;
        Ok(outcome)
    }

    pub fn request_id(&self) -> &str {
        &self.request_id
    }

    pub fn retry_generation(&self) -> u32 {
        self.retry_generation
    }

    pub fn kind(&self) -> ProviderProcessOutcomeKind {
        self.kind
    }

    pub fn provider_response_receipt_id(&self) -> Option<&str> {
        self.provider_response_receipt_id.as_deref()
    }

    pub fn provider_response_conflict_id(&self) -> Option<&str> {
        self.provider_response_conflict_id.as_deref()
    }

    pub fn source_created_event_id(&self) -> &str {
        &self.source_created_event_id
    }

    pub fn to_projection_document(&self) -> Result<Value, SdkError> {
        self.validate()?;
        serde_json::to_value(self).map_err(|error| outcome_invalid(error.to_string()))
    }

    pub fn from_projection_document(document: Value) -> Result<Self, SdkError> {
        let outcome: Self =
            serde_json::from_value(document).map_err(|error| outcome_invalid(error.to_string()))?;
        outcome.validate()?;
        Ok(outcome)
    }

    fn validate(&self) -> Result<(), SdkError> {
        if self.schema_version != PROVIDER_PROCESS_OUTCOME_SCHEMA_VERSION {
            return Err(outcome_invalid(
                "provider-process outcome schema version is unsupported",
            ));
        }
        validate_digest_id(&self.request_id, REQUEST_PREFIX, "request id")?;
        if self.retry_generation > MAX_RETRY_GENERATION {
            return Err(outcome_invalid(
                "provider-process retry generation exceeds the governed limit",
            ));
        }
        EventId::try_new(self.source_created_event_id.clone())
            .map_err(|error| outcome_invalid(error.to_string()))?;
        match self.kind {
            ProviderProcessOutcomeKind::ResponseRecorded => {
                validate_optional_digest_id(
                    self.provider_response_receipt_id.as_deref(),
                    RECEIPT_PREFIX,
                    true,
                    "provider-response receipt id",
                )?;
                if self.provider_response_conflict_id.is_some() {
                    return Err(outcome_invalid(
                        "ordinary recorded response must not reference a provider-response conflict",
                    ));
                }
            }
            ProviderProcessOutcomeKind::RetainFirstReceipt
            | ProviderProcessOutcomeKind::RejectRequest => {
                validate_optional_digest_id(
                    self.provider_response_receipt_id.as_deref(),
                    RECEIPT_PREFIX,
                    true,
                    "provider-response receipt id",
                )?;
                validate_optional_digest_id(
                    self.provider_response_conflict_id.as_deref(),
                    CONFLICT_PREFIX,
                    true,
                    "provider-response conflict id",
                )?;
            }
            ProviderProcessOutcomeKind::Skipped => {
                if self.provider_response_receipt_id.is_some()
                    || self.provider_response_conflict_id.is_some()
                {
                    return Err(outcome_invalid(
                        "skipped provider-process outcome must not contain response evidence",
                    ));
                }
            }
        }
        Ok(())
    }
}

fn validate_optional_digest_id(
    value: Option<&str>,
    prefix: &str,
    required: bool,
    label: &str,
) -> Result<(), SdkError> {
    match value {
        Some(value) => validate_digest_id(value, prefix, label),
        None if required => Err(outcome_invalid(format!("{label} is required"))),
        None => Ok(()),
    }
}

fn validate_digest_id(value: &str, prefix: &str, label: &str) -> Result<(), SdkError> {
    let suffix = value
        .strip_prefix(prefix)
        .ok_or_else(|| outcome_invalid(format!("{label} has the wrong prefix")))?;
    if suffix.len() != 64
        || !suffix
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return Err(outcome_invalid(format!(
            "{label} must end in 64 lowercase hexadecimal characters"
        )));
    }
    Ok(())
}

fn outcome_invalid(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_PROVIDER_PROCESS_OUTCOME_INVALID",
        ErrorCategory::Internal,
        false,
        "The canonical provider-process outcome is invalid.",
    )
    .with_internal_reference(reference.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest_id(prefix: &str, byte: u8) -> String {
        format!("{prefix}{}", format!("{byte:02x}").repeat(32))
    }

    #[test]
    fn retain_first_projection_document_round_trips_strictly() {
        let outcome = ProviderProcessCanonicalOutcome::retain_first_receipt(
            digest_id(REQUEST_PREFIX, 1),
            2,
            digest_id(RECEIPT_PREFIX, 2),
            digest_id(CONFLICT_PREFIX, 3),
            "provider-created-event-1",
        )
        .unwrap();
        let decoded = ProviderProcessCanonicalOutcome::from_projection_document(
            outcome.to_projection_document().unwrap(),
        )
        .unwrap();
        assert_eq!(decoded, outcome);
    }

    #[test]
    fn unknown_fields_and_inconsistent_choice_fail_closed() {
        let mut document = ProviderProcessCanonicalOutcome::response_recorded(
            digest_id(REQUEST_PREFIX, 1),
            0,
            digest_id(RECEIPT_PREFIX, 2),
            "provider-created-event-1",
        )
        .unwrap()
        .to_projection_document()
        .unwrap();
        document
            .as_object_mut()
            .unwrap()
            .insert("unexpected".to_owned(), Value::Bool(true));
        assert_eq!(
            ProviderProcessCanonicalOutcome::from_projection_document(document)
                .unwrap_err()
                .code,
            "CUSTOMER_ENRICHMENT_PROVIDER_PROCESS_OUTCOME_INVALID"
        );

        let invalid = serde_json::json!({
            "schema_version": PROVIDER_PROCESS_OUTCOME_SCHEMA_VERSION,
            "request_id": digest_id(REQUEST_PREFIX, 1),
            "retry_generation": 0,
            "kind": "reject_request",
            "provider_response_receipt_id": null,
            "provider_response_conflict_id": digest_id(CONFLICT_PREFIX, 3),
            "source_created_event_id": "provider-created-event-1"
        });
        assert!(ProviderProcessCanonicalOutcome::from_projection_document(invalid).is_err());
    }
}
