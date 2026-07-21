use crate::{
    PROVIDER_RESPONSE_CONFLICT_RECORDED_EVENT_SCHEMA,
    PROVIDER_RESPONSE_CONFLICT_RECORDED_EVENT_TYPE, provider_response_conflict_persisted_contract,
    provider_response_conflict_persisted_payload, provider_response_conflict_to_wire,
};
use crm_capability_ingress::semantic_input_hash;
use crm_capability_plan_support::{self as support, EventSpec};
use crm_capability_runtime::CapabilityRequest;
use crm_core_data::{
    BatchError, BatchMutationPlan, BatchMutationResult, DataError, PostgresDataStore,
    RecordMutation,
};
use crm_customer_enrichment::{
    PROVIDER_RESPONSE_CONFLICT_RECORD_TYPE, ProviderResponseConflict,
    ProviderResponseConflictDecision, ProviderResponseConflictResolutionDraft,
    ProviderResponseConflictResolutionPolicyDecision, ProviderResponseConflictResolutionPolicyPort,
    ProviderResponseConflictResolutionPolicyRequest, ReplayDisposition,
    decode_provider_response_conflict_state, encode_provider_response_conflict_state,
};
use crm_customer_enrichment_capability_adapter::{
    MODULE_ID, RECORD_PROVIDER_RESPONSE_CAPABILITY, provider_response_capability_definition,
};
use crm_module_sdk::{
    ActorId, BusinessTransactionId, CausationId, CorrelationId, DataClass, ErrorCategory,
    ExecutionContext, IdempotencyKey, ModuleExecutionContext, RecordId, RequestId, SchemaVersion,
    SdkError, TenantId, TraceId,
};
use crm_proto_contracts::crm::customer_enrichment::v1 as wire;
use std::fmt;
use std::sync::Arc;

const CONFLICT_ID_PREFIX: &str = "enrichment-response-conflict-";
const MAX_POLICY_EVIDENCE_BYTES: usize = 80;

/// Exact operator command used only by the internal governed resolution composition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderResponseConflictResolutionCommand {
    pub tenant_id: TenantId,
    pub conflict_id: RecordId,
    pub actor_id: ActorId,
    pub decision: ProviderResponseConflictDecision,
    pub safe_reason_code: String,
    pub approval_evidence_reference: String,
    pub causation_id: CausationId,
    pub correlation_id: CorrelationId,
    pub trace_id: TraceId,
    pub resolved_at_unix_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderResponseConflictResolutionResult {
    pub conflict: ProviderResponseConflict,
    pub replayed: bool,
}

#[derive(Clone)]
pub struct PostgresProviderResponseConflictResolutionExecutor {
    store: PostgresDataStore,
    policy: Arc<dyn ProviderResponseConflictResolutionPolicyPort>,
}

impl fmt::Debug for PostgresProviderResponseConflictResolutionExecutor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PostgresProviderResponseConflictResolutionExecutor")
            .field("store", &self.store)
            .field(
                "policy",
                &"dyn ProviderResponseConflictResolutionPolicyPort",
            )
            .finish()
    }
}

impl PostgresProviderResponseConflictResolutionExecutor {
    pub fn new(
        store: PostgresDataStore,
        policy: Arc<dyn ProviderResponseConflictResolutionPolicyPort>,
    ) -> Self {
        Self { store, policy }
    }

    pub async fn execute(
        &self,
        command: ProviderResponseConflictResolutionCommand,
    ) -> Result<ProviderResponseConflictResolutionResult, SdkError> {
        let suffix = conflict_suffix(&command.conflict_id)?;
        let definition = provider_response_capability_definition()?;
        let context = resolution_context(&definition, &command, suffix)?;
        let record = support::record_ref(
            PROVIDER_RESPONSE_CONFLICT_RECORD_TYPE,
            command.conflict_id.as_str(),
            "customer_enrichment.provider_response_conflict_resolution.provider_response_conflict_id",
        )?;
        let snapshot = self
            .store
            .get_record(&context, &record)
            .await
            .map_err(resolution_read_error)?
            .ok_or_else(conflict_not_found)?;
        if !matches!(snapshot.version, 1 | 2) {
            return Err(resolution_state_invalid(
                "provider-response conflict record version must be 1 or 2",
            ));
        }
        let bytes = support::persisted_json_bytes_with_data_class(
            &snapshot,
            provider_response_conflict_persisted_contract(),
            DataClass::Confidential,
        )?;
        let mut conflict = decode_provider_response_conflict_state(bytes)?;
        if conflict.tenant_id() != &command.tenant_id
            || conflict.conflict_id().as_str() != command.conflict_id.as_str()
        {
            return Err(resolution_state_invalid(
                "loaded provider-response conflict does not match command identity",
            ));
        }

        let policy_decision = self
            .policy
            .evaluate(ProviderResponseConflictResolutionPolicyRequest {
                tenant_id: conflict.tenant_id().clone(),
                actor_id: command.actor_id.clone(),
                conflict_id: conflict.conflict_id().clone(),
                request_id: conflict.request_id().clone(),
                retry_generation: conflict.retry_generation(),
                first_receipt_id: conflict.first_receipt_id().clone(),
                decision: command.decision,
                safe_reason_code: command.safe_reason_code.clone(),
                approval_evidence_reference: command.approval_evidence_reference.clone(),
                evaluated_at_unix_ms: command.resolved_at_unix_ms,
            })
            .await?;
        let policy_version = allowed_policy_version(policy_decision)?;
        let disposition = conflict.resolve(ProviderResponseConflictResolutionDraft {
            decision: command.decision,
            resolved_by: command.actor_id.clone(),
            policy_version,
            safe_reason_code: command.safe_reason_code.clone(),
            approval_evidence_reference: command.approval_evidence_reference.clone(),
            causation_id: command.causation_id.clone(),
            resolved_at_unix_ms: command.resolved_at_unix_ms,
        })?;
        if disposition == ReplayDisposition::Duplicate {
            if snapshot.version != 2 {
                return Err(resolution_state_invalid(
                    "resolved provider-response conflict must be stored at version 2",
                ));
            }
            return Ok(ProviderResponseConflictResolutionResult {
                conflict,
                replayed: true,
            });
        }
        if snapshot.version != 1 {
            return Err(resolution_state_invalid(
                "unresolved provider-response conflict must be stored at version 1",
            ));
        }

        let input = provider_response_conflict_persisted_payload(&conflict)?;
        let request = CapabilityRequest {
            context,
            input_hash: semantic_input_hash(&input),
            input,
            approval: None,
        };
        let state_bytes = encode_provider_response_conflict_state(&conflict)?;
        let event = support::event_evidence_with_data_class(
            &request,
            record.clone(),
            MODULE_ID,
            EventSpec {
                event_type: PROVIDER_RESPONSE_CONFLICT_RECORDED_EVENT_TYPE,
                event_schema_id: PROVIDER_RESPONSE_CONFLICT_RECORDED_EVENT_SCHEMA,
                aggregate_version: 2,
                previous_version: Some(1),
            },
            DataClass::Confidential,
            &wire::ProviderResponseConflictRecordedEvent {
                provider_response_conflict: Some(provider_response_conflict_to_wire(&conflict)?),
            },
        )?;
        let audit = support::audit_intent(
            &request,
            &record,
            2,
            RECORD_PROVIDER_RESPONSE_CAPABILITY,
            &state_bytes,
        )?;
        let batch = BatchMutationPlan {
            context: request.context.clone(),
            records: vec![RecordMutation::Update {
                reference: record.clone(),
                expected_version: 1,
                payload: provider_response_conflict_persisted_payload(&conflict)?,
            }],
            relationships: Vec::new(),
            events: vec![event],
            idempotency: support::capability_idempotency(&definition, &request)?,
            audits: vec![audit],
        };
        batch.validate().map_err(resolution_batch_error)?;
        let result = self
            .store
            .execute_batch(&batch)
            .await
            .map_err(resolution_batch_error)?;
        validate_resolution_result(&record, &conflict, &result)?;
        Ok(ProviderResponseConflictResolutionResult {
            conflict,
            replayed: result.replayed,
        })
    }
}

fn resolution_context(
    definition: &crm_capability_runtime::CapabilityDefinition,
    command: &ProviderResponseConflictResolutionCommand,
    suffix: &str,
) -> Result<ModuleExecutionContext, SdkError> {
    let request_started_at_unix_nanos = command
        .resolved_at_unix_ms
        .checked_mul(1_000_000)
        .and_then(|value| i64::try_from(value).ok())
        .ok_or_else(|| resolution_input_invalid("resolution time exceeds execution range"))?;
    Ok(ModuleExecutionContext {
        module_id: definition.owner_module_id.clone(),
        execution: ExecutionContext {
            tenant_id: command.tenant_id.clone(),
            actor_id: command.actor_id.clone(),
            request_id: configured(RequestId::try_new(format!(
                "enrichment-conflict-resolution-request-{suffix}"
            )))?,
            correlation_id: command.correlation_id.clone(),
            causation_id: command.causation_id.clone(),
            trace_id: command.trace_id.clone(),
            capability_id: definition.capability_id.clone(),
            capability_version: definition.capability_version.clone(),
            idempotency_key: configured(IdempotencyKey::try_new(format!(
                "enrichment-conflict-resolution-{suffix}"
            )))?,
            business_transaction_id: configured(BusinessTransactionId::try_new(format!(
                "enrichment-conflict-resolution-tx-{suffix}"
            )))?,
            schema_version: configured(SchemaVersion::try_new(support::CONTRACT_VERSION))?,
            request_started_at_unix_nanos,
        },
    })
}

fn allowed_policy_version(
    decision: ProviderResponseConflictResolutionPolicyDecision,
) -> Result<String, SdkError> {
    match decision {
        ProviderResponseConflictResolutionPolicyDecision::Allowed { policy_version } => {
            validate_policy_evidence(&policy_version, "policy_version")?;
            Ok(policy_version)
        }
        ProviderResponseConflictResolutionPolicyDecision::Denied {
            policy_version,
            safe_reason_code,
        } => {
            validate_policy_evidence(&policy_version, "policy_version")?;
            validate_policy_evidence(&safe_reason_code, "safe_reason_code")?;
            Err(SdkError::new(
                "CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_CONFLICT_RESOLUTION_DENIED",
                ErrorCategory::Authorization,
                false,
                "The provider-response conflict resolution is not authorized.",
            )
            .with_internal_reference(format!(
                "policy_version={policy_version};reason_code={safe_reason_code}"
            )))
        }
    }
}

fn validate_policy_evidence(value: &str, field: &'static str) -> Result<(), SdkError> {
    if value.is_empty()
        || value.len() > MAX_POLICY_EVIDENCE_BYTES
        || value.trim() != value
        || value.chars().any(char::is_control)
    {
        return Err(resolution_policy_invalid(format!(
            "{field} is not bounded canonical policy evidence"
        )));
    }
    Ok(())
}

fn validate_resolution_result(
    record: &crm_module_sdk::RecordRef,
    expected: &ProviderResponseConflict,
    result: &BatchMutationResult,
) -> Result<(), SdkError> {
    let [snapshot] = result.records.as_slice() else {
        return Err(resolution_state_invalid(
            "conflict resolution returned an unexpected record set",
        ));
    };
    if snapshot.reference != *record || snapshot.version != 2 {
        return Err(resolution_state_invalid(
            "conflict resolution returned the wrong record identity or version",
        ));
    }
    if !result.linked_relationships.is_empty() || !result.unlinked_relationships.is_empty() {
        return Err(resolution_state_invalid(
            "conflict resolution unexpectedly changed relationships",
        ));
    }
    let bytes = support::persisted_json_bytes_with_data_class(
        snapshot,
        provider_response_conflict_persisted_contract(),
        DataClass::Confidential,
    )?;
    if decode_provider_response_conflict_state(bytes)? != *expected {
        return Err(resolution_state_invalid(
            "conflict resolution returned different canonical state",
        ));
    }
    Ok(())
}

fn conflict_suffix(conflict_id: &RecordId) -> Result<&str, SdkError> {
    let suffix = conflict_id
        .as_str()
        .strip_prefix(CONFLICT_ID_PREFIX)
        .ok_or_else(|| resolution_input_invalid("conflict id has the wrong prefix"))?;
    if suffix.len() != 64
        || !suffix
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return Err(resolution_input_invalid(
            "conflict id must end in 64 lowercase hexadecimal characters",
        ));
    }
    Ok(suffix)
}

fn conflict_not_found() -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_CONFLICT_NOT_FOUND",
        ErrorCategory::NotFound,
        false,
        "The provider-response conflict was not found.",
    )
}

fn resolution_input_invalid(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_CONFLICT_RESOLUTION_INPUT_INVALID",
        ErrorCategory::InvalidArgument,
        false,
        "The provider-response conflict resolution input is invalid.",
    )
    .with_internal_reference(reference.into())
}

fn resolution_policy_invalid(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_CONFLICT_RESOLUTION_POLICY_INVALID",
        ErrorCategory::Internal,
        false,
        "The provider-response conflict resolution policy response is invalid.",
    )
    .with_internal_reference(reference.into())
}

fn resolution_state_invalid(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_CONFLICT_RESOLUTION_STATE_INVALID",
        ErrorCategory::Internal,
        false,
        "The stored provider-response conflict resolution state is invalid.",
    )
    .with_internal_reference(reference.into())
}

fn resolution_read_error(error: DataError) -> SdkError {
    let (code, category, retryable) = match &error {
        DataError::Database(_) => (
            "CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_CONFLICT_RESOLUTION_STORE_UNAVAILABLE",
            ErrorCategory::Unavailable,
            true,
        ),
        DataError::Sdk(_) | DataError::InvalidPlan(_) | DataError::InvalidStoredValue(_) => (
            "CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_CONFLICT_RESOLUTION_STATE_INVALID",
            ErrorCategory::Internal,
            false,
        ),
    };
    SdkError::new(
        code,
        category,
        retryable,
        "The provider-response conflict could not be loaded for resolution.",
    )
    .with_internal_reference(error.to_string())
}

fn resolution_batch_error(error: BatchError) -> SdkError {
    let (code, category, retryable) = match error {
        BatchError::Conflict(_) | BatchError::IdempotencyKeyReused => (
            "CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_CONFLICT_RESOLUTION_CONFLICT",
            ErrorCategory::Conflict,
            false,
        ),
        BatchError::IdempotencyInProgress => (
            "CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_CONFLICT_RESOLUTION_IN_PROGRESS",
            ErrorCategory::Unavailable,
            true,
        ),
        BatchError::Database(_) => (
            "CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_CONFLICT_RESOLUTION_STORE_UNAVAILABLE",
            ErrorCategory::Unavailable,
            true,
        ),
        BatchError::Sdk(_) | BatchError::InvalidPlan(_) | BatchError::InvalidStoredValue(_) => (
            "CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_CONFLICT_RESOLUTION_PLAN_INVALID",
            ErrorCategory::Internal,
            false,
        ),
    };
    SdkError::new(
        code,
        category,
        retryable,
        "The provider-response conflict resolution could not be persisted.",
    )
    .with_internal_reference(error.to_string())
}

fn configured<T, E: fmt::Display>(result: Result<T, E>) -> Result<T, SdkError> {
    result.map_err(|error| {
        SdkError::new(
            "CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_CONFLICT_RESOLUTION_CONFIGURATION_INVALID",
            ErrorCategory::Internal,
            false,
            "The provider-response conflict resolution configuration is invalid.",
        )
        .with_internal_reference(error.to_string())
    })
}
