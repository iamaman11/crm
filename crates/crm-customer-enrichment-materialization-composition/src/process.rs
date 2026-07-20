use crate::{
    PostgresCustomerEnrichmentSuggestionMaterializationWorker,
    ProviderSuggestionCandidateEvidenceRequest, ProviderSuggestionCandidateEvidenceSourcePort,
};
use crm_application_composition::TenantBackgroundWorker;
use crm_capability_ingress::semantic_input_hash;
use crm_capability_plan_support as support;
use crm_capability_runtime::{CapabilityExecutionResult, CapabilityRequest};
use crm_core_data::PostgresDataStore;
use crm_core_events::{
    EventHistoryRequest, ProjectionEventApplication, ProjectionFailure, ProjectionStore,
};
use crm_customer_enrichment::PROVIDER_RESPONSE_RECEIPT_RECORD_TYPE;
use crm_customer_enrichment_capability_adapter::MODULE_ID;
use crm_customer_enrichment_materialization_adapter::{
    MATERIALIZE_SUGGESTIONS_REQUEST_SCHEMA, suggestion_materialization_capability_definition,
};
use crm_module_sdk::{
    ActorId, BusinessTransactionId, CausationId, CorrelationId, DataClass, EventDelivery, EventType,
    ExecutionContext, FileId, IdempotencyKey, ModuleExecutionContext, ModuleId, PayloadEncoding,
    PortFuture, RequestId, SchemaVersion, SdkError, TenantId, TraceId,
};
use crm_proto_contracts::crm::customer_enrichment::v1 as wire;
use prost::Message;
use sha2::{Digest, Sha256};
use std::fmt;
use std::sync::Arc;

pub const MATERIALIZATION_PROCESS_WORKER_ID: &str = "customer-enrichment-materialization-process";
pub const MATERIALIZATION_PROCESS_PROJECTION_ID: &str =
    "customer-enrichment-materialization-process-v1";
pub const MATERIALIZATION_PROCESS_WORKER_ACTOR_ID: &str =
    "customer-enrichment-materialization-worker";
pub const PROVIDER_RESPONSE_RECORDED_EVENT_TYPE: &str =
    "customer_enrichment.provider_response.recorded";
pub const PROVIDER_RESPONSE_RECORDED_EVENT_SCHEMA: &str =
    "crm.customer_enrichment.v1.ProviderResponseRecordedEvent";

const MATERIALIZATION_REQUEST_IDENTITY_DOMAIN: &[u8] =
    b"crm.customer-enrichment.materialization-process-request/v1";
const DEFAULT_PAGE_SIZE: u32 = 100;

pub trait SuggestionMaterializationExecutorPort: Send + Sync {
    fn execute<'a>(
        &'a self,
        request: CapabilityRequest,
    ) -> PortFuture<'a, Result<CapabilityExecutionResult, SdkError>>;
}

impl SuggestionMaterializationExecutorPort
    for PostgresCustomerEnrichmentSuggestionMaterializationWorker
{
    fn execute<'a>(
        &'a self,
        request: CapabilityRequest,
    ) -> PortFuture<'a, Result<CapabilityExecutionResult, SdkError>> {
        Box::pin(async move {
            PostgresCustomerEnrichmentSuggestionMaterializationWorker::execute(self, request).await
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MaterializationProcessCycle {
    pub response_events: u32,
    pub materialized: u32,
    pub skipped_failed_responses: u32,
    pub replays: u32,
}

#[derive(Clone)]
pub struct CustomerEnrichmentMaterializationProcessWorker {
    store: PostgresDataStore,
    evidence: Arc<dyn ProviderSuggestionCandidateEvidenceSourcePort>,
    executor: Arc<dyn SuggestionMaterializationExecutorPort>,
    actor_id: ActorId,
    page_size: u32,
}

impl fmt::Debug for CustomerEnrichmentMaterializationProcessWorker {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CustomerEnrichmentMaterializationProcessWorker")
            .field("store", &self.store)
            .field("evidence", &"dyn ProviderSuggestionCandidateEvidenceSourcePort")
            .field("executor", &"dyn SuggestionMaterializationExecutorPort")
            .field("actor_id", &self.actor_id)
            .field("page_size", &self.page_size)
            .finish()
    }
}

impl CustomerEnrichmentMaterializationProcessWorker {
    pub fn new(
        store: PostgresDataStore,
        evidence: Arc<dyn ProviderSuggestionCandidateEvidenceSourcePort>,
        executor: Arc<dyn SuggestionMaterializationExecutorPort>,
        actor_id: ActorId,
    ) -> Result<Self, SdkError> {
        Self::with_page_size(store, evidence, executor, actor_id, DEFAULT_PAGE_SIZE)
    }

    pub fn with_page_size(
        store: PostgresDataStore,
        evidence: Arc<dyn ProviderSuggestionCandidateEvidenceSourcePort>,
        executor: Arc<dyn SuggestionMaterializationExecutorPort>,
        actor_id: ActorId,
        page_size: u32,
    ) -> Result<Self, SdkError> {
        if page_size == 0 || page_size > crm_core_events::MAX_EVENT_HISTORY_PAGE_SIZE {
            return Err(configuration_invalid(
                "materialization-process page size is outside the governed limit",
            ));
        }
        Ok(Self {
            store,
            evidence,
            executor,
            actor_id,
            page_size,
        })
    }

    pub fn run_cycle<'a>(
        &'a self,
        tenant_id: TenantId,
        _now_unix_nanos: i64,
    ) -> PortFuture<'a, Result<MaterializationProcessCycle, SdkError>> {
        Box::pin(async move {
            let module_id = module_id()?;
            let event_type = event_type()?;
            let checkpoint = ProjectionStore::projection_checkpoint(
                &self.store,
                tenant_id.clone(),
                MATERIALIZATION_PROCESS_PROJECTION_ID.to_owned(),
            )
            .await?;
            let mut after = checkpoint.map(|value| value.cursor);
            let mut cycle = MaterializationProcessCycle::default();

            loop {
                let page = ProjectionStore::list_event_history(
                    &self.store,
                    EventHistoryRequest {
                        tenant_id: tenant_id.clone(),
                        consumer_module_id: module_id.clone(),
                        event_types: vec![event_type.clone()],
                        after: after.clone(),
                        page_size: self.page_size,
                    },
                )
                .await?;
                if page.deliveries.is_empty() {
                    return Ok(cycle);
                }
                let next_cursor = page.next_cursor.clone();
                for delivery in page.deliveries {
                    cycle.response_events = cycle.response_events.saturating_add(1);
                    match self.process_delivery(&tenant_id, &delivery).await {
                        Ok(MaterializationDisposition::Executed(result)) => {
                            cycle.materialized = cycle.materialized.saturating_add(1);
                            if result.replayed {
                                cycle.replays = cycle.replays.saturating_add(1);
                            }
                        }
                        Ok(MaterializationDisposition::SkippedFailedResponse) => {
                            cycle.skipped_failed_responses =
                                cycle.skipped_failed_responses.saturating_add(1);
                        }
                        Err(error) => {
                            if !error.retryable {
                                let _ = ProjectionStore::mark_projection_failed(
                                    &self.store,
                                    ProjectionFailure {
                                        tenant_id: tenant_id.clone(),
                                        projection_id: MATERIALIZATION_PROCESS_PROJECTION_ID
                                            .to_owned(),
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
                            projection_id: MATERIALIZATION_PROCESS_PROJECTION_ID.to_owned(),
                            delivery,
                            writes: Vec::new(),
                        },
                    )
                    .await?;
                }
                let Some(next) = next_cursor else {
                    return Ok(cycle);
                };
                after = Some(next);
            }
        })
    }

    async fn process_delivery(
        &self,
        tenant_id: &TenantId,
        delivery: &EventDelivery,
    ) -> Result<MaterializationDisposition, SdkError> {
        let event = decode_response_recorded_event(delivery)?;
        let receipt = event
            .provider_response_receipt
            .ok_or_else(response_event_invalid)?;
        let request_id = receipt
            .enrichment_request_ref
            .as_ref()
            .map(|reference| reference.enrichment_request_id.clone())
            .ok_or_else(response_event_invalid)?;
        let receipt_id = receipt
            .provider_response_receipt_ref
            .as_ref()
            .map(|reference| reference.provider_response_receipt_id.clone())
            .ok_or_else(response_event_invalid)?;
        if tenant_id != &delivery.tenant_id
            || delivery.aggregate.record_type.as_str() != PROVIDER_RESPONSE_RECEIPT_RECORD_TYPE
            || delivery.aggregate.record_id.as_str() != receipt_id
        {
            return Err(response_event_invalid());
        }

        let definition = suggestion_materialization_capability_definition()?;
        let context = materialization_context(delivery, &definition.owner_module_id, &self.actor_id)?;
        let command = match wire::ProviderResponseClass::try_from(receipt.response_class) {
            Ok(wire::ProviderResponseClass::Success) => {
                let file_id = receipt
                    .protected_evidence_reference
                    .as_deref()
                    .ok_or_else(success_evidence_missing)
                    .and_then(|reference| {
                        FileId::try_new(reference.to_owned()).map_err(configuration_identifier)
                    })?;
                self.evidence
                    .load(ProviderSuggestionCandidateEvidenceRequest {
                        context: context.clone(),
                        file_id,
                        expected_enrichment_request_id: request_id.clone(),
                        expected_provider_response_receipt_id: receipt_id.clone(),
                    })
                    .await?
            }
            Ok(wire::ProviderResponseClass::NoMatch) => wire::MaterializeSuggestionsRequest {
                enrichment_request_ref: Some(wire::EnrichmentRequestRef {
                    enrichment_request_id: request_id,
                }),
                provider_response_receipt_ref: Some(wire::ProviderResponseReceiptRef {
                    provider_response_receipt_id: receipt_id,
                }),
                candidates: Vec::new(),
            },
            Ok(wire::ProviderResponseClass::RetryableFailure)
            | Ok(wire::ProviderResponseClass::TerminalFailure) => {
                return Ok(MaterializationDisposition::SkippedFailedResponse);
            }
            _ => return Err(response_event_invalid()),
        };

        let input = support::protobuf_payload(
            MODULE_ID,
            MATERIALIZE_SUGGESTIONS_REQUEST_SCHEMA,
            DataClass::Personal,
            &command,
        )?;
        let request = CapabilityRequest {
            context,
            input_hash: semantic_input_hash(&input),
            input,
            approval: None,
        };
        let result = self.executor.execute(request).await?;
        Ok(MaterializationDisposition::Executed(result))
    }
}

impl TenantBackgroundWorker for CustomerEnrichmentMaterializationProcessWorker {
    fn run_tenant_cycle<'a>(
        &'a self,
        tenant_id: TenantId,
        now_unix_nanos: i64,
    ) -> PortFuture<'a, Result<(), SdkError>> {
        Box::pin(async move {
            self.run_cycle(tenant_id, now_unix_nanos).await?;
            Ok(())
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
enum MaterializationDisposition {
    Executed(CapabilityExecutionResult),
    SkippedFailedResponse,
}

fn decode_response_recorded_event(
    delivery: &EventDelivery,
) -> Result<wire::ProviderResponseRecordedEvent, SdkError> {
    delivery.validate()?;
    let module_id = module_id()?;
    let contract = support::protobuf_contract(
        MODULE_ID,
        PROVIDER_RESPONSE_RECORDED_EVENT_SCHEMA,
        vec![DataClass::Personal],
    )?;
    if delivery.source_module_id != module_id
        || delivery.consumer_module_id != module_id
        || delivery.event_type.as_str() != PROVIDER_RESPONSE_RECORDED_EVENT_TYPE
        || delivery.event_version.as_str() != support::CONTRACT_VERSION
        || !contract.matches(&delivery.payload)
        || delivery.payload.encoding != PayloadEncoding::Protobuf
    {
        return Err(response_event_invalid());
    }
    wire::ProviderResponseRecordedEvent::decode(delivery.payload.bytes.as_slice()).map_err(|error| {
        response_event_invalid().with_internal_reference(format!("response event decode: {error}"))
    })
}

fn materialization_context(
    delivery: &EventDelivery,
    module_id: &ModuleId,
    actor_id: &ActorId,
) -> Result<ModuleExecutionContext, SdkError> {
    let definition = suggestion_materialization_capability_definition()?;
    if module_id != &definition.owner_module_id {
        return Err(configuration_invalid(
            "materialization definition owner differs from the process module",
        ));
    }
    let suffix = hex(&materialization_request_identity(delivery));
    let context = ModuleExecutionContext {
        module_id: module_id.clone(),
        execution: ExecutionContext {
            tenant_id: delivery.tenant_id.clone(),
            actor_id: actor_id.clone(),
            request_id: configured(RequestId::try_new(format!(
                "enrichment-materialization-request-{suffix}"
            )))?,
            correlation_id: delivery.correlation_id.clone(),
            causation_id: configured(CausationId::try_new(delivery.event_id.as_str()))?,
            trace_id: delivery.trace_id.clone(),
            capability_id: definition.capability_id,
            capability_version: definition.capability_version,
            idempotency_key: configured(IdempotencyKey::try_new(delivery.delivery_id.as_str()))?,
            business_transaction_id: configured(BusinessTransactionId::try_new(format!(
                "enrichment-materialization-tx-{suffix}"
            )))?,
            schema_version: configured(SchemaVersion::try_new(support::CONTRACT_VERSION))?,
            request_started_at_unix_nanos: delivery.occurred_at_unix_nanos,
        },
    };
    delivery.validate_for_consumer(&context)?;
    Ok(context)
}

fn materialization_request_identity(delivery: &EventDelivery) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hash_frame(&mut hasher, MATERIALIZATION_REQUEST_IDENTITY_DOMAIN);
    hash_frame(&mut hasher, delivery.tenant_id.as_str().as_bytes());
    hash_frame(&mut hasher, delivery.event_id.as_str().as_bytes());
    hash_frame(&mut hasher, delivery.delivery_id.as_str().as_bytes());
    hasher.finalize().into()
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

fn module_id() -> Result<ModuleId, SdkError> {
    ModuleId::try_new(MODULE_ID).map_err(configuration_identifier)
}

fn event_type() -> Result<EventType, SdkError> {
    EventType::try_new(PROVIDER_RESPONSE_RECORDED_EVENT_TYPE).map_err(configuration_identifier)
}

fn configured<T>(value: Result<T, crm_module_sdk::IdentifierError>) -> Result<T, SdkError> {
    value.map_err(configuration_identifier)
}

fn configuration_identifier(error: crm_module_sdk::IdentifierError) -> SdkError {
    configuration_invalid(error.to_string())
}

fn configuration_invalid(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_MATERIALIZATION_PROCESS_CONFIGURATION_INVALID",
        crm_module_sdk::ErrorCategory::Internal,
        false,
        "The Customer Enrichment materialization process is not configured safely.",
    )
    .with_internal_reference(reference.into())
}

fn response_event_invalid() -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_EVENT_INVALID",
        crm_module_sdk::ErrorCategory::Internal,
        false,
        "Customer Enrichment provider-response evidence is invalid.",
    )
}

fn success_evidence_missing() -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_SUCCESS_EVIDENCE_MISSING",
        crm_module_sdk::ErrorCategory::Conflict,
        false,
        "A successful provider response is missing governed canonical candidate evidence.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_response_event_coordinate_is_internal_and_versioned() {
        assert_eq!(
            PROVIDER_RESPONSE_RECORDED_EVENT_TYPE,
            "customer_enrichment.provider_response.recorded"
        );
        assert_eq!(
            PROVIDER_RESPONSE_RECORDED_EVENT_SCHEMA,
            "crm.customer_enrichment.v1.ProviderResponseRecordedEvent"
        );
    }

    #[test]
    fn worker_identity_and_projection_are_stable() {
        assert_eq!(
            MATERIALIZATION_PROCESS_WORKER_ACTOR_ID,
            "customer-enrichment-materialization-worker"
        );
        assert_eq!(
            MATERIALIZATION_PROCESS_PROJECTION_ID,
            "customer-enrichment-materialization-process-v1"
        );
    }
}
