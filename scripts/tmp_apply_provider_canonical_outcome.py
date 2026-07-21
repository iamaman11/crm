from pathlib import Path


def replace_once(path: Path, old: str, new: str) -> None:
    text = path.read_text()
    if old not in text:
        raise SystemExit(f"marker not found in {path}: {old[:200]!r}")
    path.write_text(text.replace(old, new, 1))


outcome_source = r'''use crm_module_sdk::{ErrorCategory, EventId, SdkError};
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
            return Err(outcome_invalid("provider-process outcome schema version is unsupported"));
        }
        validate_digest_id(&self.request_id, REQUEST_PREFIX, "request id")?;
        if self.retry_generation > MAX_RETRY_GENERATION {
            return Err(outcome_invalid("provider-process retry generation exceeds the governed limit"));
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
'''
Path("modules/crm-customer-enrichment/src/provider_process_outcome.rs").write_text(outcome_source)

module_lib = Path("modules/crm-customer-enrichment/src/lib.rs")
replace_once(
    module_lib,
    '''/// Pure-core governed reads, policy, provider dispatch and owner-application boundaries.
pub mod ports;
''',
    '''/// Pure-core governed reads, policy, provider dispatch and owner-application boundaries.
pub mod ports;
/// Canonical cross-process outcome written atomically with the provider checkpoint.
pub mod provider_process_outcome;
''',
)
replace_once(
    module_lib,
    '''pub use ports::*;
pub use provider_usage::*;
''',
    '''pub use ports::*;
pub use provider_process_outcome::*;
pub use provider_usage::*;
''',
)

projection = Path("crates/crm-core-data/src/postgres_projection.rs")
replace_once(
    projection,
    '''    pub async fn projection_documents(
        &self,
        tenant_id: &TenantId,
''',
    '''    pub async fn projection_document(
        &self,
        tenant_id: &TenantId,
        projection_id: &str,
        resource_type: &str,
        resource_id: &str,
    ) -> Result<Option<serde_json::Value>, SdkError> {
        validate_projection_id(projection_id)?;
        if resource_type.is_empty()
            || resource_type.len() > 180
            || resource_id.is_empty()
            || resource_id.len() > 360
        {
            return Err(projection_request_invalid(
                "projection resource identity is invalid",
            ));
        }
        let mut transaction = self
            .pool()
            .begin()
            .await
            .map_err(projection_database_error)?;
        bind_projection_tenant(&mut transaction, tenant_id).await?;
        sqlx::query("SET TRANSACTION READ ONLY")
            .execute(&mut *transaction)
            .await
            .map_err(projection_database_error)?;
        let document = sqlx::query_scalar::<_, serde_json::Value>(
            r#"
            SELECT document
            FROM crm.projection_documents
            WHERE tenant_id = $1
              AND projection_id = $2
              AND resource_type = $3
              AND resource_id = $4
            "#,
        )
        .bind(tenant_id.as_str())
        .bind(projection_id)
        .bind(resource_type)
        .bind(resource_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(projection_database_error)?;
        transaction
            .commit()
            .await
            .map_err(projection_database_error)?;
        Ok(document)
    }

    pub async fn projection_documents(
        &self,
        tenant_id: &TenantId,
''',
)

provider_worker = Path(
    "crates/crm-customer-enrichment-provider-process-composition/src/worker.rs"
)
replace_once(
    provider_worker,
    '''use crm_core_events::{
    EventHistoryRequest, ProjectionEventApplication, ProjectionFailure, ProjectionStore,
};
''',
    '''use crm_core_events::{
    EventHistoryRequest, ProjectionDocumentWrite, ProjectionEventApplication, ProjectionFailure,
    ProjectionStore,
};
''',
)
replace_once(
    provider_worker,
    '''    ENRICHMENT_REQUEST_RECORD_TYPE, EnrichmentRequest, PartySnapshot, ProviderProfileVersion,
    ProviderResponseConflictDecision,
};
''',
    '''    ENRICHMENT_REQUEST_RECORD_TYPE, EnrichmentRequest, PROVIDER_PROCESS_OUTCOME_RESOURCE_TYPE,
    PartySnapshot, ProviderProcessCanonicalOutcome, ProviderProfileVersion,
    ProviderResponseConflictDecision,
};
''',
)
replace_once(
    provider_worker,
    '''pub const PROVIDER_PROCESS_WORKER_ID: &str = "customer-enrichment-provider-process";
pub const PROVIDER_PROCESS_PROJECTION_ID: &str = "customer-enrichment-provider-process-v1";
''',
    '''pub const PROVIDER_PROCESS_WORKER_ID: &str = "customer-enrichment-provider-process";
pub use crm_customer_enrichment::PROVIDER_PROCESS_PROJECTION_ID;
''',
)
old_loop = '''                for delivery in page.deliveries {
                    cycle.created_events = cycle.created_events.saturating_add(1);
                    match self
                        .process_delivery(&tenant_id, now_unix_ms, &delivery)
                        .await
                    {
                        Ok(DeliveryDisposition::Executed(result)) => {
                            cycle.dispatched = cycle.dispatched.saturating_add(1);
                            if result.dispatch_replayed {
                                cycle.dispatch_replays = cycle.dispatch_replays.saturating_add(1);
                            }
                            if result.response_replayed {
                                cycle.response_replays = cycle.response_replays.saturating_add(1);
                            }
                        }
                        Ok(DeliveryDisposition::Skipped) => {
                            cycle.skipped = cycle.skipped.saturating_add(1);
                        }
                        Ok(DeliveryDisposition::RetainedFirstReceipt) => {
                            cycle.retained_first_receipts =
                                cycle.retained_first_receipts.saturating_add(1);
                        }
                        Ok(DeliveryDisposition::RejectedRequest { replayed }) => {
                            cycle.rejected_requests = cycle.rejected_requests.saturating_add(1);
                            if replayed {
                                cycle.rejection_replays =
                                    cycle.rejection_replays.saturating_add(1);
                            }
                        }
                        Err(error) => {
                            if !error.retryable {
                                let _ = ProjectionStore::mark_projection_failed(
                                    &self.store,
                                    ProjectionFailure {
                                        tenant_id: tenant_id.clone(),
                                        projection_id: PROVIDER_PROCESS_PROJECTION_ID.to_owned(),
                                        event_id: delivery.event_id.clone(),
                                        occurred_at_unix_nanos: delivery.occurred_at_unix_nanos,
                                        failure_code: error.code.clone(),
                                    },
                                )
                                .await;
                            }
                            return Err(error);
                        }
                    }
                    ProjectionStore::apply_projection_event(
                        &self.store,
                        ProjectionEventApplication {
                            projection_id: PROVIDER_PROCESS_PROJECTION_ID.to_owned(),
                            delivery,
                            writes: Vec::new(),
                        },
                    )
                    .await?;
                }
'''
new_loop = '''                for delivery in page.deliveries {
                    cycle.created_events = cycle.created_events.saturating_add(1);
                    let processed = match self
                        .process_delivery(&tenant_id, now_unix_ms, &delivery)
                        .await
                    {
                        Ok(processed) => processed,
                        Err(error) => {
                            if !error.retryable {
                                let _ = ProjectionStore::mark_projection_failed(
                                    &self.store,
                                    ProjectionFailure {
                                        tenant_id: tenant_id.clone(),
                                        projection_id: PROVIDER_PROCESS_PROJECTION_ID.to_owned(),
                                        event_id: delivery.event_id.clone(),
                                        occurred_at_unix_nanos: delivery.occurred_at_unix_nanos,
                                        failure_code: error.code.clone(),
                                    },
                                )
                                .await;
                            }
                            return Err(error);
                        }
                    };
                    match processed.disposition {
                        DeliveryDisposition::Executed(result) => {
                            cycle.dispatched = cycle.dispatched.saturating_add(1);
                            if result.dispatch_replayed {
                                cycle.dispatch_replays = cycle.dispatch_replays.saturating_add(1);
                            }
                            if result.response_replayed {
                                cycle.response_replays = cycle.response_replays.saturating_add(1);
                            }
                        }
                        DeliveryDisposition::Skipped => {
                            cycle.skipped = cycle.skipped.saturating_add(1);
                        }
                        DeliveryDisposition::RetainedFirstReceipt => {
                            cycle.retained_first_receipts =
                                cycle.retained_first_receipts.saturating_add(1);
                        }
                        DeliveryDisposition::RejectedRequest { replayed } => {
                            cycle.rejected_requests = cycle.rejected_requests.saturating_add(1);
                            if replayed {
                                cycle.rejection_replays =
                                    cycle.rejection_replays.saturating_add(1);
                            }
                        }
                    }
                    ProjectionStore::apply_projection_event(
                        &self.store,
                        ProjectionEventApplication {
                            projection_id: PROVIDER_PROCESS_PROJECTION_ID.to_owned(),
                            delivery,
                            writes: vec![processed.outcome_write],
                        },
                    )
                    .await?;
                }
'''
replace_once(provider_worker, old_loop, new_loop)
replace_once(
    provider_worker,
    '''    ) -> Result<DeliveryDisposition, SdkError> {
''',
    '''    ) -> Result<ProcessedDelivery, SdkError> {
''',
)
replace_once(
    provider_worker,
    '''                ProviderResponseConflictDecision::RetainFirstReceipt => {
                    Ok(DeliveryDisposition::RetainedFirstReceipt)
                }
''',
    '''                ProviderResponseConflictDecision::RetainFirstReceipt => processed_delivery(
                    DeliveryDisposition::RetainedFirstReceipt,
                    ProviderProcessCanonicalOutcome::retain_first_receipt(
                        conflict.request_id().as_str().to_owned(),
                        conflict.retry_generation(),
                        conflict.first_receipt_id().as_str().to_owned(),
                        conflict.conflict_id().as_str().to_owned(),
                        delivery.event_id.as_str().to_owned(),
                    )?,
                    delivery,
                ),
''',
)
replace_once(
    provider_worker,
    '''                    Ok(DeliveryDisposition::RejectedRequest {
                        replayed: result.replayed,
                    })
''',
    '''                    processed_delivery(
                        DeliveryDisposition::RejectedRequest {
                            replayed: result.replayed,
                        },
                        ProviderProcessCanonicalOutcome::reject_request(
                            conflict.request_id().as_str().to_owned(),
                            conflict.retry_generation(),
                            conflict.first_receipt_id().as_str().to_owned(),
                            conflict.conflict_id().as_str().to_owned(),
                            delivery.event_id.as_str().to_owned(),
                        )?,
                        delivery,
                    )
''',
)
replace_once(
    provider_worker,
    '''        let ProviderDispatchSourceDisposition::Ready(source) = source else {
            return Ok(DeliveryDisposition::Skipped);
        };
''',
    '''        let ProviderDispatchSourceDisposition::Ready(source) = source else {
            return processed_delivery(
                DeliveryDisposition::Skipped,
                ProviderProcessCanonicalOutcome::skipped(
                    request_ref.enrichment_request_id.clone(),
                    created.retry_generation,
                    delivery.event_id.as_str().to_owned(),
                )?,
                delivery,
            );
        };
''',
)
replace_once(
    provider_worker,
    '''            ProviderDispatchExecution::Recorded(result) => {
                Ok(DeliveryDisposition::Executed(result))
            }
''',
    '''            ProviderDispatchExecution::Recorded(result) => {
                let outcome = recorded_outcome(result.as_ref(), delivery)?;
                processed_delivery(DeliveryDisposition::Executed(result), outcome, delivery)
            }
''',
)
replace_once(
    provider_worker,
    '''#[derive(Debug)]
enum DeliveryDisposition {
''',
    '''#[derive(Debug)]
struct ProcessedDelivery {
    disposition: DeliveryDisposition,
    outcome_write: ProjectionDocumentWrite,
}

#[derive(Debug)]
enum DeliveryDisposition {
''',
)
helper_marker = '''fn decode_created_event(
'''
helpers = r'''fn processed_delivery(
    disposition: DeliveryDisposition,
    outcome: ProviderProcessCanonicalOutcome,
    delivery: &EventDelivery,
) -> Result<ProcessedDelivery, SdkError> {
    Ok(ProcessedDelivery {
        disposition,
        outcome_write: ProjectionDocumentWrite {
            resource_type: PROVIDER_PROCESS_OUTCOME_RESOURCE_TYPE.to_owned(),
            resource_id: outcome.request_id().to_owned(),
            source_version: delivery.aggregate_version,
            document: outcome.to_projection_document()?,
        },
    })
}

fn recorded_outcome(
    result: &ProviderDispatchWorkerResult,
    delivery: &EventDelivery,
) -> Result<ProviderProcessCanonicalOutcome, SdkError> {
    let request = result
        .response
        .enrichment_request
        .as_ref()
        .ok_or_else(provider_outcome_invalid)?;
    let request_id = request
        .enrichment_request_ref
        .as_ref()
        .map(|reference| reference.enrichment_request_id.clone())
        .ok_or_else(provider_outcome_invalid)?;
    let receipt_id = result
        .response
        .provider_response_receipt
        .as_ref()
        .and_then(|receipt| receipt.provider_response_receipt_ref.as_ref())
        .map(|reference| reference.provider_response_receipt_id.clone())
        .ok_or_else(provider_outcome_invalid)?;
    ProviderProcessCanonicalOutcome::response_recorded(
        request_id,
        request.retry_generation,
        receipt_id,
        delivery.event_id.as_str().to_owned(),
    )
}

fn provider_outcome_invalid() -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_PROVIDER_PROCESS_OUTCOME_INVALID",
        crm_module_sdk::ErrorCategory::Internal,
        false,
        "The canonical provider-process outcome could not be derived.",
    )
}

'''
replace_once(provider_worker, helper_marker, helpers + helper_marker)

materialization = Path(
    "crates/crm-customer-enrichment-materialization-composition/src/process.rs"
)
replace_once(
    materialization,
    '''use crm_customer_enrichment::PROVIDER_RESPONSE_RECEIPT_RECORD_TYPE;
''',
    '''use crm_customer_enrichment::{
    PROVIDER_PROCESS_OUTCOME_RESOURCE_TYPE, PROVIDER_PROCESS_PROJECTION_ID,
    PROVIDER_RESPONSE_RECEIPT_RECORD_TYPE, ProviderProcessCanonicalOutcome,
    ProviderProcessOutcomeKind,
};
''',
)
replace_once(
    materialization,
    '''    pub skipped_failed_responses: u32,
    pub replays: u32,
''',
    '''    pub skipped_failed_responses: u32,
    pub skipped_rejected_requests: u32,
    pub replays: u32,
''',
)
replace_once(
    materialization,
    '''                        Ok(MaterializationDisposition::SkippedFailedResponse) => {
                            cycle.skipped_failed_responses =
                                cycle.skipped_failed_responses.saturating_add(1);
                        }
''',
    '''                        Ok(MaterializationDisposition::SkippedFailedResponse) => {
                            cycle.skipped_failed_responses =
                                cycle.skipped_failed_responses.saturating_add(1);
                        }
                        Ok(MaterializationDisposition::SkippedRejectedRequest) => {
                            cycle.skipped_rejected_requests =
                                cycle.skipped_rejected_requests.saturating_add(1);
                        }
''',
)
choice_gate = r'''        let canonical_document = self
            .store
            .projection_document(
                tenant_id,
                PROVIDER_PROCESS_PROJECTION_ID,
                PROVIDER_PROCESS_OUTCOME_RESOURCE_TYPE,
                &request_id,
            )
            .await?
            .ok_or_else(canonical_choice_pending)?;
        let canonical =
            ProviderProcessCanonicalOutcome::from_projection_document(canonical_document)?;
        if canonical.request_id() != request_id
            || canonical.provider_response_receipt_id() != Some(receipt_id.as_str())
        {
            return Err(canonical_choice_mismatch());
        }
        match canonical.kind() {
            ProviderProcessOutcomeKind::ResponseRecorded
            | ProviderProcessOutcomeKind::RetainFirstReceipt => {}
            ProviderProcessOutcomeKind::RejectRequest => {
                return Ok(MaterializationDisposition::SkippedRejectedRequest);
            }
            ProviderProcessOutcomeKind::Skipped => return Err(canonical_choice_mismatch()),
        }

'''
replace_once(
    materialization,
    '''        let definition = suggestion_materialization_capability_definition()?;
''',
    choice_gate + '''        let definition = suggestion_materialization_capability_definition()?;
''',
)
replace_once(
    materialization,
    '''enum MaterializationDisposition {
    Executed(CapabilityExecutionResult),
    SkippedFailedResponse,
}
''',
    '''enum MaterializationDisposition {
    Executed(CapabilityExecutionResult),
    SkippedFailedResponse,
    SkippedRejectedRequest,
}
''',
)
replace_once(
    materialization,
    '''fn success_evidence_missing() -> SdkError {
''',
    '''fn canonical_choice_pending() -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_PROVIDER_CANONICAL_CHOICE_PENDING",
        crm_module_sdk::ErrorCategory::Conflict,
        true,
        "The governed provider canonical choice is not available yet.",
    )
}

fn canonical_choice_mismatch() -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_PROVIDER_CANONICAL_CHOICE_MISMATCH",
        crm_module_sdk::ErrorCategory::Internal,
        false,
        "The provider response does not match the governed canonical choice.",
    )
}

fn success_evidence_missing() -> SdkError {
''',
)

materialization_test = Path(
    "crates/crm-customer-enrichment-materialization-composition/tests/postgres_materialization_event_process.rs"
)
replace_once(
    materialization_test,
    '''use crm_core_events::ProjectionStore;
''',
    '''use crm_core_events::{
    EventHistoryRequest, ProjectionDocumentWrite, ProjectionEventApplication, ProjectionStore,
};
''',
)
replace_once(
    materialization_test,
    '''    ProviderProfileDraft, ProviderProfileVersion, ProviderResponseClass, ProviderResponseReceipt,
''',
    '''    PROVIDER_PROCESS_OUTCOME_RESOURCE_TYPE, PROVIDER_PROCESS_PROJECTION_ID,
    ProviderProcessCanonicalOutcome, ProviderProfileDraft, ProviderProfileVersion,
    ProviderResponseClass, ProviderResponseReceipt,
''',
)
old_missing = '''    let missing = process
        .run_cycle(tenant_id.clone(), 50_000_000)
        .await
        .unwrap_err();
    assert_eq!(
        missing.code,
        "CUSTOMER_ENRICHMENT_SUGGESTION_EVIDENCE_UNAVAILABLE"
    );
'''
new_missing = '''    let pending_choice = process
        .run_cycle(tenant_id.clone(), 45_000_000)
        .await
        .expect_err("response event must wait for the provider canonical choice");
    assert_eq!(
        pending_choice.code,
        "CUSTOMER_ENRICHMENT_PROVIDER_CANONICAL_CHOICE_PENDING"
    );
    assert!(pending_choice.retryable);
    assert!(
        ProjectionStore::projection_checkpoint(
            &store,
            tenant_id.clone(),
            MATERIALIZATION_PROCESS_PROJECTION_ID.to_owned(),
        )
        .await
        .unwrap()
        .is_none()
    );
    assert_eq!(suggestion_count(&admin).await, 0);

    apply_recorded_provider_outcome(&store, &fixture)
        .await
        .expect("apply canonical provider outcome with provider checkpoint");

    let missing = process
        .run_cycle(tenant_id.clone(), 50_000_000)
        .await
        .unwrap_err();
    assert_eq!(
        missing.code,
        "CUSTOMER_ENRICHMENT_SUGGESTION_EVIDENCE_UNAVAILABLE"
    );
'''
replace_once(materialization_test, old_missing, new_missing)
helper_insert = r'''async fn apply_recorded_provider_outcome(
    store: &PostgresDataStore,
    fixture: &Fixture,
) -> Result<(), SdkError> {
    let tenant_id = TenantId::try_new(TENANT_ID).unwrap();
    let page = ProjectionStore::list_event_history(
        store,
        EventHistoryRequest {
            tenant_id: tenant_id.clone(),
            consumer_module_id: ModuleId::try_new(MODULE_ID).unwrap(),
            event_types: vec![EventType::try_new(ENRICHMENT_REQUEST_CREATED_EVENT_TYPE).unwrap()],
            after: None,
            page_size: 100,
        },
    )
    .await?;
    let delivery = page
        .deliveries
        .into_iter()
        .find(|delivery| {
            delivery.aggregate.record_id.as_str() == fixture.request.request_id().as_str()
        })
        .expect("request-created delivery exists");
    let outcome = ProviderProcessCanonicalOutcome::response_recorded(
        fixture.request.request_id().as_str().to_owned(),
        fixture.request.retry_generation(),
        fixture.receipt.receipt_id().as_str().to_owned(),
        delivery.event_id.as_str().to_owned(),
    )?;
    ProjectionStore::apply_projection_event(
        store,
        ProjectionEventApplication {
            projection_id: PROVIDER_PROCESS_PROJECTION_ID.to_owned(),
            writes: vec![ProjectionDocumentWrite {
                resource_type: PROVIDER_PROCESS_OUTCOME_RESOURCE_TYPE.to_owned(),
                resource_id: fixture.request.request_id().as_str().to_owned(),
                source_version: delivery.aggregate_version,
                document: outcome.to_projection_document()?,
            }],
            delivery,
        },
    )
    .await?;
    Ok(())
}

'''
replace_once(materialization_test, '''fn materialization_process(
''', helper_insert + '''fn materialization_process(
''')

hold_test = Path(
    "crates/crm-customer-enrichment-provider-process-composition/tests/postgres_conflict_process_hold.rs"
)
replace_once(
    hold_test,
    '''    ProviderResponseReceiptId, RawPayloadPolicy, RequestPolicyEvidence, TargetField,
    TargetSnapshot,
''',
    '''    PROVIDER_PROCESS_OUTCOME_RESOURCE_TYPE, ProviderProcessCanonicalOutcome,
    ProviderProcessOutcomeKind, ProviderResponseReceiptId, RawPayloadPolicy, RequestPolicyEvidence,
    TargetField, TargetSnapshot,
''',
)
replace_once(
    hold_test,
    '''    assert_eq!(evidence_counts(&admin).await, resolved_baseline);

    let resolved = conflict_store
''',
    '''    assert_eq!(evidence_counts(&admin).await, resolved_baseline);
    let outcome = provider_outcome(&store, &fixture.request)
        .await
        .expect("retain-first provider outcome exists");
    assert_eq!(outcome.kind(), ProviderProcessOutcomeKind::RetainFirstReceipt);
    assert_eq!(
        outcome.provider_response_receipt_id(),
        Some(first_receipt_id.as_str())
    );
    assert_eq!(
        outcome.provider_response_conflict_id(),
        Some(conflict.conflict_id().as_str())
    );

    let resolved = conflict_store
''',
)
hold_helper = r'''async fn provider_outcome(
    store: &PostgresDataStore,
    request: &EnrichmentRequest,
) -> Option<ProviderProcessCanonicalOutcome> {
    store
        .projection_document(
            &TenantId::try_new(TENANT_ID).unwrap(),
            PROVIDER_PROCESS_PROJECTION_ID,
            PROVIDER_PROCESS_OUTCOME_RESOURCE_TYPE,
            request.request_id().as_str(),
        )
        .await
        .expect("read provider canonical outcome")
        .map(|document| {
            ProviderProcessCanonicalOutcome::from_projection_document(document)
                .expect("decode provider canonical outcome")
        })
}

'''
replace_once(hold_test, '''#[derive(Clone)]
struct AllowRetainFirstPolicy;
''', hold_helper + '''#[derive(Clone)]
struct AllowRetainFirstPolicy;
''')

reject_test = Path(
    "crates/crm-customer-enrichment-provider-process-composition/tests/postgres_conflict_reject_process.rs"
)
replace_once(
    reject_test,
    '''    ProviderResponseConflictResolutionPolicyRequest, ProviderResponseReceiptId, RawPayloadPolicy,
    RequestPolicyEvidence, TargetField, TargetSnapshot,
''',
    '''    PROVIDER_PROCESS_OUTCOME_RESOURCE_TYPE, ProviderProcessCanonicalOutcome,
    ProviderProcessOutcomeKind, ProviderResponseConflictResolutionPolicyRequest,
    ProviderResponseReceiptId, RawPayloadPolicy, RequestPolicyEvidence, TargetField, TargetSnapshot,
''',
)
replace_once(
    reject_test,
    '''    assert_eq!(evidence_counts(&admin).await, after_terminal);

    let snapshot = store
''',
    '''    assert_eq!(evidence_counts(&admin).await, after_terminal);
    let outcome = provider_outcome(&store, &request)
        .await
        .expect("reject provider outcome exists");
    assert_eq!(outcome.kind(), ProviderProcessOutcomeKind::RejectRequest);
    assert_eq!(
        outcome.provider_response_receipt_id(),
        Some(resolved.conflict.first_receipt_id().as_str())
    );
    assert_eq!(
        outcome.provider_response_conflict_id(),
        Some(resolved.conflict.conflict_id().as_str())
    );

    let snapshot = store
''',
)
reject_helper = r'''async fn provider_outcome(
    store: &PostgresDataStore,
    request: &EnrichmentRequest,
) -> Option<ProviderProcessCanonicalOutcome> {
    store
        .projection_document(
            &TenantId::try_new(TENANT_ID).unwrap(),
            PROVIDER_PROCESS_PROJECTION_ID,
            PROVIDER_PROCESS_OUTCOME_RESOURCE_TYPE,
            request.request_id().as_str(),
        )
        .await
        .expect("read reject provider outcome")
        .map(|document| {
            ProviderProcessCanonicalOutcome::from_projection_document(document)
                .expect("decode reject provider outcome")
        })
}

'''
replace_once(reject_test, '''#[derive(Clone)]
struct AllowRejectPolicy;
''', reject_helper + '''#[derive(Clone)]
struct AllowRejectPolicy;
''')
