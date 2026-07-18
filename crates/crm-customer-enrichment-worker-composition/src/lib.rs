#![forbid(unsafe_code)]

//! Durable non-runtime provider-dispatch worker composition.
//!
//! The worker commits the exact pre-I/O dispatch batch, invokes one exact infrastructure adapter,
//! and commits the sanitized response batch. It is intentionally not registered in the public
//! capability inventory; production activation still requires real provider process acceptance.

use crm_capability_ingress::semantic_input_hash;
use crm_capability_plan_support::{self as support, message_descriptor_hash};
use crm_capability_runtime::{
    CapabilityDefinition, CapabilityExecutionResult, CapabilityRequest,
    TransactionalCapabilityExecutor,
};
use crm_core_data::{PostgresDataStore, PostgresTransactionalAggregateExecutor};
use crm_customer_enrichment::{
    ProviderAdapterRegistryPort, ProviderDispatchRequest, ProviderResponseClass,
    SanitizedProviderResponse,
};
use crm_customer_enrichment_capability_adapter::{
    CustomerEnrichmentRequestDispatchPlanner, CustomerEnrichmentRequestReferencePlanner,
    DISPATCH_ENRICHMENT_REQUEST_CAPABILITY, DISPATCH_ENRICHMENT_REQUEST_REQUEST_SCHEMA,
    DISPATCH_ENRICHMENT_REQUEST_RESPONSE_SCHEMA, MODULE_ID,
    RECORD_PROVIDER_RESPONSE_REQUEST_SCHEMA, RECORD_PROVIDER_RESPONSE_RESPONSE_SCHEMA,
    provider_response_capability_definition, request_dispatch_capability_definition,
};
use crm_module_sdk::{
    BusinessTransactionId, CapabilityId, CapabilityVersion, CausationId, DataClass, ErrorCategory,
    IdempotencyKey, ModuleExecutionContext, PayloadEncoding, RequestId, SchemaVersion, SdkError,
    TypedPayload,
};
use crm_proto_contracts::crm::customer_enrichment::v1 as wire;
use prost::Message;
use sha2::{Digest, Sha256};
use std::fmt;
use std::sync::Arc;

const RESPONSE_IDENTITY_DOMAIN: &[u8] = b"crm.customer-enrichment.response-worker/v1";
const MAX_INTERNAL_KEY_BYTES: usize = 180;

/// Stable crate identity for architecture tooling.
pub const CRATE_NAME: &str = "crm-customer-enrichment-worker-composition";

/// One validated dispatch transaction plus the exact provider envelope prepared from governed
/// profile and Party snapshots.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderDispatchWorkItem {
    pub dispatch_request: CapabilityRequest,
    pub provider_request: ProviderDispatchRequest,
}

/// Durable worker outcome after the provider response has been atomically recorded.
#[derive(Debug, Clone, PartialEq)]
pub struct ProviderDispatchWorkerResult {
    pub dispatch_replayed: bool,
    pub response_replayed: bool,
    pub response: wire::RecordProviderResponseResponse,
}

/// Infrastructure coordinator for commit-before-I/O and atomic response recording.
#[derive(Clone)]
pub struct CustomerEnrichmentProviderWorker {
    dispatch_executor: Arc<dyn TransactionalCapabilityExecutor>,
    response_executor: Arc<dyn TransactionalCapabilityExecutor>,
    registry: Arc<dyn ProviderAdapterRegistryPort>,
    dispatch_definition: CapabilityDefinition,
    response_definition: CapabilityDefinition,
}

impl fmt::Debug for CustomerEnrichmentProviderWorker {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CustomerEnrichmentProviderWorker")
            .field("dispatch_executor", &"dyn TransactionalCapabilityExecutor")
            .field("response_executor", &"dyn TransactionalCapabilityExecutor")
            .field("registry", &"dyn ProviderAdapterRegistryPort")
            .field(
                "dispatch_capability",
                &self.dispatch_definition.capability_id,
            )
            .field(
                "response_capability",
                &self.response_definition.capability_id,
            )
            .finish()
    }
}

impl CustomerEnrichmentProviderWorker {
    pub fn try_new(
        dispatch_executor: Arc<dyn TransactionalCapabilityExecutor>,
        response_executor: Arc<dyn TransactionalCapabilityExecutor>,
        registry: Arc<dyn ProviderAdapterRegistryPort>,
    ) -> Result<Self, SdkError> {
        Ok(Self {
            dispatch_executor,
            response_executor,
            registry,
            dispatch_definition: request_dispatch_capability_definition()?,
            response_definition: provider_response_capability_definition()?,
        })
    }

    /// Creates the non-runtime worker with two independent transactional executors over the same
    /// PostgreSQL store. No capability is added to the public runtime registry by this factory.
    pub fn postgres(
        store: PostgresDataStore,
        registry: Arc<dyn ProviderAdapterRegistryPort>,
    ) -> Result<Self, SdkError> {
        let dispatch_executor = Arc::new(PostgresTransactionalAggregateExecutor::new(
            store.clone(),
            Arc::new(CustomerEnrichmentRequestDispatchPlanner),
        ));
        let response_executor = Arc::new(PostgresTransactionalAggregateExecutor::new(
            store,
            Arc::new(CustomerEnrichmentRequestReferencePlanner),
        ));
        Self::try_new(dispatch_executor, response_executor, registry)
    }

    /// Executes one replay-safe provider attempt.
    ///
    /// 1. The dispatch capability transaction commits the final `Dispatched` state and immutable
    ///    RequestDispatched evidence.
    /// 2. The exact registry invokes the adapter with the generation-bound provider key.
    /// 3. The response capability transaction atomically records request, receipt and usage state.
    ///
    /// Re-running the same item after either crash window reuses capability idempotency and the
    /// exact provider key. Provider I/O is never attempted when the dispatch commit fails.
    pub async fn execute(
        &self,
        item: ProviderDispatchWorkItem,
    ) -> Result<ProviderDispatchWorkerResult, SdkError> {
        let expectation = validate_work_item(&self.dispatch_definition, &item)?;

        let dispatch_result = self
            .dispatch_executor
            .execute(&self.dispatch_definition, item.dispatch_request.clone())
            .await?;
        validate_dispatch_output(&dispatch_result, &item.provider_request, expectation)?;

        let sanitized = self
            .registry
            .dispatch_exact(item.provider_request.clone())
            .await?;
        validate_sanitized_response(&item.provider_request, &sanitized)?;

        let response_request = build_response_request(
            &self.response_definition,
            &item.dispatch_request,
            &item.provider_request,
            &sanitized,
        )?;
        let response_result = self
            .response_executor
            .execute(&self.response_definition, response_request)
            .await?;
        let response: wire::RecordProviderResponseResponse = decode_execution_output(
            &response_result,
            RECORD_PROVIDER_RESPONSE_RESPONSE_SCHEMA,
            DataClass::Personal,
        )?;
        validate_response_output(&response, &item.provider_request, &sanitized)?;

        Ok(ProviderDispatchWorkerResult {
            dispatch_replayed: dispatch_result.replayed,
            response_replayed: response_result.replayed,
            response,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DispatchExpectation {
    final_retry_generation: u32,
}

fn validate_work_item(
    definition: &CapabilityDefinition,
    item: &ProviderDispatchWorkItem,
) -> Result<DispatchExpectation, SdkError> {
    let request = &item.dispatch_request;
    if definition.capability_id.as_str() != DISPATCH_ENRICHMENT_REQUEST_CAPABILITY
        || request.context.module_id.as_str() != MODULE_ID
        || request.context.execution.capability_id != definition.capability_id
        || request.context.execution.capability_version != definition.capability_version
        || request.context.execution.tenant_id != item.provider_request.tenant_id
        || request.context.execution.actor_id != item.provider_request.actor_id
    {
        return Err(worker_input_invalid(
            "dispatch request context does not match the exact provider envelope",
        ));
    }

    let command: wire::DispatchEnrichmentRequestRequest = support::decode_request_with_data_class(
        request,
        MODULE_ID,
        DISPATCH_ENRICHMENT_REQUEST_REQUEST_SCHEMA,
        DataClass::Personal,
    )?;
    let request_ref = command.enrichment_request_ref.ok_or_else(|| {
        worker_input_invalid("dispatch command is missing the enrichment-request reference")
    })?;
    if request_ref.enrichment_request_id != item.provider_request.enrichment_request_id.as_str() {
        return Err(worker_input_invalid(
            "dispatch command and provider envelope target different enrichment requests",
        ));
    }

    let final_retry_generation =
        match wire::EnrichmentRequestStatus::try_from(command.expected_status) {
            Ok(wire::EnrichmentRequestStatus::Created)
            | Ok(wire::EnrichmentRequestStatus::Queued) => command.expected_retry_generation,
            Ok(wire::EnrichmentRequestStatus::FailedRetryable) => command
                .expected_retry_generation
                .checked_add(1)
                .ok_or_else(|| worker_input_invalid("dispatch retry generation overflow"))?,
            _ => {
                return Err(worker_input_invalid(
                    "dispatch expectation must be Created, Queued or FailedRetryable",
                ));
            }
        };
    if final_retry_generation != item.provider_request.retry_generation
        || item.provider_request.party_resource_version <= 0
        || item.provider_request.deadline_at_unix_ms <= 0
    {
        return Err(worker_input_invalid(
            "provider envelope does not match the final dispatch generation or target bounds",
        ));
    }
    validate_internal_key(
        &item.provider_request.provider_idempotency_key,
        "provider idempotency key",
    )?;
    Ok(DispatchExpectation {
        final_retry_generation,
    })
}

fn validate_dispatch_output(
    result: &CapabilityExecutionResult,
    provider_request: &ProviderDispatchRequest,
    expectation: DispatchExpectation,
) -> Result<(), SdkError> {
    let output: wire::DispatchEnrichmentRequestResponse = decode_execution_output(
        result,
        DISPATCH_ENRICHMENT_REQUEST_RESPONSE_SCHEMA,
        DataClass::Personal,
    )?;
    let request = output
        .enrichment_request
        .ok_or_else(|| dispatch_output_invalid("dispatch output is missing request state"))?;
    let request_ref = request
        .enrichment_request_ref
        .as_ref()
        .ok_or_else(|| dispatch_output_invalid("dispatch output is missing request identity"))?;
    let profile_ref = request
        .provider_profile_version_ref
        .as_ref()
        .ok_or_else(|| {
            dispatch_output_invalid("dispatch output is missing provider-profile identity")
        })?;
    let mapping_ref = request
        .mapping_version_ref
        .as_ref()
        .ok_or_else(|| dispatch_output_invalid("dispatch output is missing mapping identity"))?;
    let target = request
        .target
        .as_ref()
        .ok_or_else(|| dispatch_output_invalid("dispatch output is missing Party target"))?;
    let party_ref = target
        .party_ref
        .as_ref()
        .ok_or_else(|| dispatch_output_invalid("dispatch output is missing Party identity"))?;

    if request_ref.enrichment_request_id != provider_request.enrichment_request_id.as_str()
        || request.status != wire::EnrichmentRequestStatus::Dispatched as i32
        || request.retry_generation != expectation.final_retry_generation
        || profile_ref.provider_profile_version_id
            != provider_request.provider_profile_version_id.as_str()
        || mapping_ref.mapping_version_id != provider_request.mapping_version_id.as_str()
        || party_ref.party_id != provider_request.party_id.as_str()
        || target.party_resource_version != provider_request.party_resource_version
        || request.deadline_at_unix_ms != provider_request.deadline_at_unix_ms
    {
        return Err(dispatch_output_invalid(
            "committed dispatch state does not match the exact provider envelope",
        ));
    }
    Ok(())
}

fn validate_sanitized_response(
    provider_request: &ProviderDispatchRequest,
    response: &SanitizedProviderResponse,
) -> Result<(), SdkError> {
    if response.replay_key != provider_request.provider_idempotency_key {
        return Err(SdkError::new(
            "CUSTOMER_ENRICHMENT_PROVIDER_REPLAY_KEY_MISMATCH",
            ErrorCategory::Dependency,
            false,
            "The provider response could not be bound to the dispatch attempt.",
        ));
    }
    if response
        .canonical_response_digest
        .iter()
        .all(|byte| *byte == 0)
        || response.retrieved_at_unix_ms < 0
        || response
            .provider_observed_at_unix_ms
            .is_some_and(|value| value < 0 || value > response.retrieved_at_unix_ms)
    {
        return Err(SdkError::new(
            "CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_INVALID",
            ErrorCategory::Dependency,
            false,
            "The provider returned invalid sanitized evidence.",
        ));
    }
    if let Some(code) = response.safe_provider_code.as_deref() {
        validate_internal_key(code, "safe provider code")?;
    }
    Ok(())
}

fn build_response_request(
    definition: &CapabilityDefinition,
    dispatch_request: &CapabilityRequest,
    provider_request: &ProviderDispatchRequest,
    response: &SanitizedProviderResponse,
) -> Result<CapabilityRequest, SdkError> {
    let command = wire::RecordProviderResponseRequest {
        enrichment_request_ref: Some(wire::EnrichmentRequestRef {
            enrichment_request_id: provider_request.enrichment_request_id.as_str().to_owned(),
        }),
        replay_key: response.replay_key.clone(),
        provider_correlation_id: response.provider_correlation_id.clone(),
        response_class: provider_response_class_to_wire(response.response_class),
        canonical_response_digest: response.canonical_response_digest.to_vec(),
        provider_observed_at_unix_ms: response.provider_observed_at_unix_ms,
        retrieved_at_unix_ms: response.retrieved_at_unix_ms,
        metered_units: response.metered_units,
        protected_evidence_reference: response.protected_evidence_reference.clone(),
        safe_provider_code: response.safe_provider_code.clone(),
        expected_retry_generation: provider_request.retry_generation,
    };
    let input = support::protobuf_payload(
        MODULE_ID,
        RECORD_PROVIDER_RESPONSE_REQUEST_SCHEMA,
        DataClass::Personal,
        &command,
    )?;
    let digest = response_identity(provider_request);
    let suffix = hex(&digest);
    let started_at = response
        .retrieved_at_unix_ms
        .checked_mul(1_000_000)
        .ok_or_else(|| {
            worker_input_invalid("provider retrieval timestamp exceeds nanosecond range")
        })?;
    let source = &dispatch_request.context.execution;
    let context = ModuleExecutionContext {
        module_id: definition.owner_module_id.clone(),
        execution: crm_module_sdk::ExecutionContext {
            tenant_id: source.tenant_id.clone(),
            actor_id: source.actor_id.clone(),
            request_id: configured(RequestId::try_new(format!(
                "enrichment-response-request-{suffix}"
            )))?,
            correlation_id: source.correlation_id.clone(),
            causation_id: configured(CausationId::try_new(source.request_id.as_str()))?,
            trace_id: source.trace_id.clone(),
            capability_id: configured(CapabilityId::try_new(definition.capability_id.as_str()))?,
            capability_version: configured(CapabilityVersion::try_new(
                definition.capability_version.as_str(),
            ))?,
            idempotency_key: configured(IdempotencyKey::try_new(format!(
                "enrichment-response-{suffix}"
            )))?,
            business_transaction_id: configured(BusinessTransactionId::try_new(format!(
                "enrichment-response-tx-{suffix}"
            )))?,
            schema_version: configured(SchemaVersion::try_new(support::CONTRACT_VERSION))?,
            request_started_at_unix_nanos: started_at,
        },
    };
    let input_hash = semantic_input_hash(&input);
    Ok(CapabilityRequest {
        context,
        input,
        input_hash,
        approval: None,
    })
}

fn validate_response_output(
    response: &wire::RecordProviderResponseResponse,
    provider_request: &ProviderDispatchRequest,
    sanitized: &SanitizedProviderResponse,
) -> Result<(), SdkError> {
    let request = response
        .enrichment_request
        .as_ref()
        .ok_or_else(|| response_output_invalid("response output is missing request state"))?;
    let request_ref = request
        .enrichment_request_ref
        .as_ref()
        .ok_or_else(|| response_output_invalid("response output is missing request identity"))?;
    let receipt = response.provider_response_receipt.as_ref().ok_or_else(|| {
        response_output_invalid("response output is missing immutable receipt evidence")
    })?;
    let receipt_request_ref = receipt.enrichment_request_ref.as_ref().ok_or_else(|| {
        response_output_invalid("response receipt is missing enrichment-request identity")
    })?;
    if request_ref.enrichment_request_id != provider_request.enrichment_request_id.as_str()
        || request.status != wire::EnrichmentRequestStatus::ResponseRecorded as i32
        || request.retry_generation != provider_request.retry_generation
        || receipt_request_ref.enrichment_request_id
            != provider_request.enrichment_request_id.as_str()
        || receipt.replay_key != provider_request.provider_idempotency_key
        || receipt.response_class != provider_response_class_to_wire(sanitized.response_class)
        || receipt.canonical_response_digest != sanitized.canonical_response_digest
    {
        return Err(response_output_invalid(
            "recorded response does not match the exact provider attempt",
        ));
    }
    Ok(())
}

fn decode_execution_output<M: Message + Default>(
    result: &CapabilityExecutionResult,
    schema_id: &'static str,
    data_class: DataClass,
) -> Result<M, SdkError> {
    let payload = result
        .output
        .as_ref()
        .ok_or_else(|| worker_output_invalid("capability execution returned no output"))?;
    validate_output_payload(payload, schema_id, data_class)?;
    M::decode(payload.bytes.as_slice())
        .map_err(|_| worker_output_invalid("capability output is not valid Protobuf"))
}

fn validate_output_payload(
    payload: &TypedPayload,
    schema_id: &'static str,
    data_class: DataClass,
) -> Result<(), SdkError> {
    if payload.owner.as_str() != MODULE_ID
        || payload.schema_id.as_str() != schema_id
        || payload.schema_version.as_str() != support::CONTRACT_VERSION
        || payload.descriptor_hash != message_descriptor_hash(schema_id)
        || payload.data_class != data_class
        || payload.encoding != PayloadEncoding::Protobuf
        || payload.validate().is_err()
    {
        return Err(worker_output_invalid(
            "capability output contract does not match the worker expectation",
        ));
    }
    Ok(())
}

fn response_identity(request: &ProviderDispatchRequest) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hash_frame(&mut hasher, RESPONSE_IDENTITY_DOMAIN);
    hash_frame(
        &mut hasher,
        request.enrichment_request_id.as_str().as_bytes(),
    );
    hash_frame(&mut hasher, &request.retry_generation.to_be_bytes());
    hash_frame(&mut hasher, request.provider_idempotency_key.as_bytes());
    hasher.finalize().into()
}

fn provider_response_class_to_wire(value: ProviderResponseClass) -> i32 {
    match value {
        ProviderResponseClass::Success => wire::ProviderResponseClass::Success as i32,
        ProviderResponseClass::NoMatch => wire::ProviderResponseClass::NoMatch as i32,
        ProviderResponseClass::RetryableFailure => {
            wire::ProviderResponseClass::RetryableFailure as i32
        }
        ProviderResponseClass::TerminalFailure => {
            wire::ProviderResponseClass::TerminalFailure as i32
        }
    }
}

fn validate_internal_key(value: &str, label: &'static str) -> Result<(), SdkError> {
    if value.is_empty()
        || value.len() > MAX_INTERNAL_KEY_BYTES
        || value.trim() != value
        || value.chars().any(char::is_control)
    {
        return Err(worker_input_invalid(format!(
            "{label} is empty, non-canonical or oversized"
        )));
    }
    Ok(())
}

fn hash_frame(hasher: &mut Sha256, value: &[u8]) {
    hasher.update((value.len() as u64).to_be_bytes());
    hasher.update(value);
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(DIGITS[(byte >> 4) as usize] as char);
        output.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    output
}

fn configured<T>(value: Result<T, crm_module_sdk::IdentifierError>) -> Result<T, SdkError> {
    value.map_err(|error| worker_input_invalid(error.to_string()))
}

fn worker_input_invalid(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_PROVIDER_WORK_ITEM_INVALID",
        ErrorCategory::Internal,
        false,
        "The provider work item could not be executed safely.",
    )
    .with_internal_reference(reference.into())
}

fn dispatch_output_invalid(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_DISPATCH_OUTPUT_INVALID",
        ErrorCategory::Internal,
        false,
        "The committed dispatch state could not be verified.",
    )
    .with_internal_reference(reference.into())
}

fn response_output_invalid(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_RESPONSE_OUTPUT_INVALID",
        ErrorCategory::Internal,
        false,
        "The recorded provider response could not be verified.",
    )
    .with_internal_reference(reference.into())
}

fn worker_output_invalid(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_WORKER_OUTPUT_INVALID",
        ErrorCategory::Internal,
        false,
        "The provider worker received invalid capability output.",
    )
    .with_internal_reference(reference.into())
}

#[cfg(test)]
mod tests;
