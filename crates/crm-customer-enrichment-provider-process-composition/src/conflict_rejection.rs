use crate::provider_response_conflict_persisted_payload;
use crm_capability_ingress::semantic_input_hash;
use crm_capability_plan_support::{self as support, EventSpec};
use crm_capability_runtime::CapabilityRequest;
use crm_core_data::{
    BatchError, BatchMutationPlan, BatchMutationResult, DataError, PostgresDataStore,
    RecordMutation,
};
use crm_customer_enrichment::{
    ENRICHMENT_REQUEST_RECORD_TYPE, EnrichmentRequest, EnrichmentRequestStatus,
    ProviderResponseConflict, ProviderResponseConflictDecision,
};
use crm_customer_enrichment_capability_adapter::{
    ENRICHMENT_REQUEST_STATUS_CHANGED_EVENT_SCHEMA, ENRICHMENT_REQUEST_STATUS_CHANGED_EVENT_TYPE,
    MODULE_ID, RECORD_PROVIDER_RESPONSE_CAPABILITY, enrichment_request_from_snapshot,
    enrichment_request_persisted_payload, enrichment_request_to_wire,
    provider_response_capability_definition,
};
use crm_module_sdk::{
    ActorId, BusinessTransactionId, CorrelationId, DataClass, ErrorCategory, ExecutionContext,
    IdempotencyKey, ModuleExecutionContext, RequestId, SchemaVersion, SdkError, TraceId,
};
use crm_proto_contracts::crm::customer_enrichment::v1 as wire;
use std::fmt;

const CONFLICT_ID_PREFIX: &str = "enrichment-response-conflict-";

/// Stable process lineage used for the terminal request transition after an approved rejection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderResponseConflictRejectionLineage {
    pub actor_id: ActorId,
    pub correlation_id: CorrelationId,
    pub trace_id: TraceId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderResponseConflictRejectionResult {
    pub request: EnrichmentRequest,
    pub replayed: bool,
}

#[derive(Debug, Clone)]
pub struct PostgresProviderResponseConflictRejectExecutor {
    store: PostgresDataStore,
}

impl PostgresProviderResponseConflictRejectExecutor {
    pub fn new(store: PostgresDataStore) -> Self {
        Self { store }
    }

    pub async fn execute(
        &self,
        conflict: &ProviderResponseConflict,
        lineage: ProviderResponseConflictRejectionLineage,
    ) -> Result<ProviderResponseConflictRejectionResult, SdkError> {
        let resolution = conflict
            .resolution()
            .ok_or_else(rejection_resolution_missing)?;
        if resolution.decision() != ProviderResponseConflictDecision::RejectRequest {
            return Err(rejection_resolution_invalid(
                "provider-response conflict resolution is not reject-request",
            ));
        }
        let suffix = conflict_suffix(conflict.conflict_id().as_str())?;
        let definition = provider_response_capability_definition()?;
        let input = provider_response_conflict_persisted_payload(conflict)?;
        let request = CapabilityRequest {
            context: rejection_context(conflict, &lineage, suffix)?,
            input_hash: semantic_input_hash(&input),
            input,
            approval: None,
        };
        let record = support::record_ref(
            ENRICHMENT_REQUEST_RECORD_TYPE,
            conflict.request_id().as_str(),
            "customer_enrichment.provider_response_conflict.request_id",
        )?;
        let snapshot = self
            .store
            .get_record(&request.context, &record)
            .await
            .map_err(rejection_read_error)?
            .ok_or_else(rejection_request_not_found)?;
        let mut enrichment_request = enrichment_request_from_snapshot(&snapshot)?;
        if enrichment_request.tenant_id() != conflict.tenant_id()
            || enrichment_request.request_id() != conflict.request_id()
            || enrichment_request.retry_generation() != conflict.retry_generation()
        {
            return Err(rejection_state_invalid(
                "request identity or retry generation differs from immutable conflict evidence",
            ));
        }
        if enrichment_request
            .response_receipt_id()
            .is_some_and(|receipt| receipt != conflict.first_receipt_id())
        {
            return Err(rejection_state_invalid(
                "request is bound to a different provider-response receipt",
            ));
        }

        let (expected_version, aggregate_version) = if enrichment_request.status()
            == EnrichmentRequestStatus::FailedTerminal
        {
            if enrichment_request.last_safe_failure_code() != Some(resolution.safe_reason_code())
                || snapshot.version <= 1
            {
                return Err(rejection_state_invalid(
                    "terminal request does not match the approved immutable rejection",
                ));
            }
            (snapshot.version - 1, snapshot.version)
        } else {
            if enrichment_request.status().is_terminal() {
                return Err(rejection_state_invalid(
                    "request already has a different terminal outcome",
                ));
            }
            enrichment_request.fail_terminal(
                resolution.safe_reason_code().to_owned(),
                resolution.resolved_at_unix_ms(),
            )?;
            let aggregate_version = snapshot
                .version
                .checked_add(1)
                .ok_or_else(|| rejection_state_invalid("request version overflow"))?;
            (snapshot.version, aggregate_version)
        };

        let output_request = enrichment_request_to_wire(&enrichment_request)?;
        let persisted = enrichment_request_persisted_payload(&enrichment_request)?;
        let event = support::event_evidence_with_data_class(
            &request,
            record.clone(),
            MODULE_ID,
            EventSpec {
                event_type: ENRICHMENT_REQUEST_STATUS_CHANGED_EVENT_TYPE,
                event_schema_id: ENRICHMENT_REQUEST_STATUS_CHANGED_EVENT_SCHEMA,
                aggregate_version,
                previous_version: Some(expected_version),
            },
            DataClass::Personal,
            &wire::EnrichmentRequestStatusChangedEvent {
                enrichment_request: Some(output_request),
            },
        )?;
        let audit = support::audit_intent(
            &request,
            &record,
            aggregate_version,
            RECORD_PROVIDER_RESPONSE_CAPABILITY,
            &persisted.bytes,
        )?;
        let batch = BatchMutationPlan {
            context: request.context.clone(),
            records: vec![RecordMutation::Update {
                reference: record.clone(),
                expected_version,
                payload: persisted,
            }],
            relationships: Vec::new(),
            events: vec![event],
            idempotency: support::capability_idempotency(&definition, &request)?,
            audits: vec![audit],
        };
        batch.validate().map_err(rejection_batch_error)?;
        let result = self
            .store
            .execute_batch(&batch)
            .await
            .map_err(rejection_batch_error)?;
        validate_rejection_result(&record, aggregate_version, &enrichment_request, &result)?;
        Ok(ProviderResponseConflictRejectionResult {
            request: enrichment_request,
            replayed: result.replayed,
        })
    }
}

fn rejection_context(
    conflict: &ProviderResponseConflict,
    lineage: &ProviderResponseConflictRejectionLineage,
    suffix: &str,
) -> Result<ModuleExecutionContext, SdkError> {
    let resolution = conflict
        .resolution()
        .ok_or_else(rejection_resolution_missing)?;
    let definition = provider_response_capability_definition()?;
    let request_started_at_unix_nanos = resolution
        .resolved_at_unix_ms()
        .checked_mul(1_000_000)
        .and_then(|value| i64::try_from(value).ok())
        .ok_or_else(|| rejection_state_invalid("resolution time exceeds execution range"))?;
    Ok(ModuleExecutionContext {
        module_id: definition.owner_module_id.clone(),
        execution: ExecutionContext {
            tenant_id: conflict.tenant_id().clone(),
            actor_id: lineage.actor_id.clone(),
            request_id: configured(RequestId::try_new(format!(
                "enrichment-conflict-rejection-request-{suffix}"
            )))?,
            correlation_id: lineage.correlation_id.clone(),
            causation_id: resolution.causation_id().clone(),
            trace_id: lineage.trace_id.clone(),
            capability_id: definition.capability_id.clone(),
            capability_version: definition.capability_version.clone(),
            idempotency_key: configured(IdempotencyKey::try_new(format!(
                "enrichment-conflict-rejection-{suffix}"
            )))?,
            business_transaction_id: configured(BusinessTransactionId::try_new(format!(
                "enrichment-conflict-rejection-tx-{suffix}"
            )))?,
            schema_version: configured(SchemaVersion::try_new(support::CONTRACT_VERSION))?,
            request_started_at_unix_nanos,
        },
    })
}

fn validate_rejection_result(
    record: &crm_module_sdk::RecordRef,
    aggregate_version: i64,
    expected: &EnrichmentRequest,
    result: &BatchMutationResult,
) -> Result<(), SdkError> {
    let [snapshot] = result.records.as_slice() else {
        return Err(rejection_state_invalid(
            "terminal rejection returned an unexpected record set",
        ));
    };
    if snapshot.reference != *record || snapshot.version != aggregate_version {
        return Err(rejection_state_invalid(
            "terminal rejection returned the wrong request identity or version",
        ));
    }
    if !result.linked_relationships.is_empty() || !result.unlinked_relationships.is_empty() {
        return Err(rejection_state_invalid(
            "terminal rejection unexpectedly changed relationships",
        ));
    }
    if enrichment_request_from_snapshot(snapshot)? != *expected {
        return Err(rejection_state_invalid(
            "terminal rejection returned different canonical request state",
        ));
    }
    Ok(())
}

fn conflict_suffix(conflict_id: &str) -> Result<&str, SdkError> {
    let suffix = conflict_id
        .strip_prefix(CONFLICT_ID_PREFIX)
        .ok_or_else(|| rejection_state_invalid("conflict id has the wrong prefix"))?;
    if suffix.len() != 64
        || !suffix
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return Err(rejection_state_invalid(
            "conflict id must end in 64 lowercase hexadecimal characters",
        ));
    }
    Ok(suffix)
}

fn rejection_resolution_missing() -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_CONFLICT_UNRESOLVED",
        ErrorCategory::Conflict,
        true,
        "Provider-response conflict resolution is required before request rejection.",
    )
}

fn rejection_resolution_invalid(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_CONFLICT_REJECTION_DECISION_INVALID",
        ErrorCategory::Conflict,
        false,
        "The provider-response conflict resolution cannot reject the request.",
    )
    .with_internal_reference(reference.into())
}

fn rejection_request_not_found() -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_REQUEST_NOT_FOUND",
        ErrorCategory::NotFound,
        false,
        "The enrichment request selected for conflict rejection was not found.",
    )
}

fn rejection_state_invalid(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_CONFLICT_REJECTION_STATE_INVALID",
        ErrorCategory::Conflict,
        false,
        "The provider-response conflict rejection state is inconsistent.",
    )
    .with_internal_reference(reference.into())
}

fn rejection_read_error(error: DataError) -> SdkError {
    let (code, category, retryable) = match &error {
        DataError::Database(_) => (
            "CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_CONFLICT_REJECTION_STORE_UNAVAILABLE",
            ErrorCategory::Unavailable,
            true,
        ),
        DataError::Sdk(_) | DataError::InvalidPlan(_) | DataError::InvalidStoredValue(_) => (
            "CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_CONFLICT_REJECTION_STATE_INVALID",
            ErrorCategory::Internal,
            false,
        ),
    };
    SdkError::new(
        code,
        category,
        retryable,
        "The enrichment request could not be loaded for conflict rejection.",
    )
    .with_internal_reference(error.to_string())
}

fn rejection_batch_error(error: BatchError) -> SdkError {
    let (code, category, retryable) = match error {
        BatchError::Conflict(_) | BatchError::IdempotencyKeyReused => (
            "CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_CONFLICT_REJECTION_CONFLICT",
            ErrorCategory::Conflict,
            false,
        ),
        BatchError::IdempotencyInProgress => (
            "CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_CONFLICT_REJECTION_IN_PROGRESS",
            ErrorCategory::Unavailable,
            true,
        ),
        BatchError::Database(_) => (
            "CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_CONFLICT_REJECTION_STORE_UNAVAILABLE",
            ErrorCategory::Unavailable,
            true,
        ),
        BatchError::Sdk(_) | BatchError::InvalidPlan(_) | BatchError::InvalidStoredValue(_) => (
            "CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_CONFLICT_REJECTION_PLAN_INVALID",
            ErrorCategory::Internal,
            false,
        ),
    };
    SdkError::new(
        code,
        category,
        retryable,
        "The terminal provider-response conflict rejection could not be persisted.",
    )
    .with_internal_reference(error.to_string())
}

fn configured<T, E: fmt::Display>(result: Result<T, E>) -> Result<T, SdkError> {
    result.map_err(|error| {
        SdkError::new(
            "CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_CONFLICT_REJECTION_CONFIGURATION_INVALID",
            ErrorCategory::Internal,
            false,
            "The provider-response conflict rejection configuration is invalid.",
        )
        .with_internal_reference(error.to_string())
    })
}
