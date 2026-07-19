use crate::CustomerEnrichmentPartyApplicationOrchestrator;
use crm_capability_ingress::semantic_input_hash;
use crm_capability_plan_support as support;
use crm_capability_runtime::CapabilityRequest;
use crm_core_data::PostgresDataStore;
use crm_core_events::{
    EventHistoryRequest, ProjectionEventApplication, ProjectionFailure, ProjectionStore,
};
use crm_customer_enrichment_application_adapter::{
    APPLY_PARTY_DISPLAY_NAME_CAPABILITY, APPLY_PARTY_DISPLAY_NAME_REQUEST_SCHEMA,
    apply_party_display_name_capability_definition,
};
use crm_customer_enrichment_capability_adapter::MODULE_ID;
use crm_customer_enrichment_review_adapter::{
    SUGGESTION_REVIEWED_EVENT_SCHEMA, SUGGESTION_REVIEWED_EVENT_TYPE,
};
use crm_module_sdk::{
    ActorId, BusinessTransactionId, CapabilityVersion, CausationId, CorrelationId, DataClass,
    EventDelivery, EventType, ExecutionContext, IdempotencyKey, ModuleExecutionContext, ModuleId,
    PayloadEncoding, PortFuture, RequestId, SchemaVersion, SdkError, TenantId, TraceId,
};
use crm_proto_contracts::crm::customer_enrichment::v1 as wire;
use prost::Message;
use std::fmt;
use std::sync::Arc;

pub const PARTY_DISPLAY_NAME_APPLICATION_WORKER_ID: &str =
    "customer-enrichment-party-display-name-application";
pub const PARTY_DISPLAY_NAME_APPLICATION_PROJECTION_ID: &str =
    "customer-enrichment-party-display-name-application-v1";
pub const PARTY_DISPLAY_NAME_APPLICATION_WORKER_ACTOR_ID: &str =
    "customer-enrichment-application-worker";
const DEFAULT_PAGE_SIZE: u32 = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PartyDisplayNameApplicationCycle {
    pub reviewed_events: u32,
    pub accepted_events: u32,
    pub skipped_events: u32,
    pub replayed_attempts: u32,
}

#[derive(Clone)]
pub struct CustomerEnrichmentPartyApplicationWorker {
    store: PostgresDataStore,
    orchestrator: Arc<CustomerEnrichmentPartyApplicationOrchestrator>,
    actor_id: ActorId,
    page_size: u32,
}

impl fmt::Debug for CustomerEnrichmentPartyApplicationWorker {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CustomerEnrichmentPartyApplicationWorker")
            .field("store", &self.store)
            .field("orchestrator", &"CustomerEnrichmentPartyApplicationOrchestrator")
            .field("actor_id", &self.actor_id)
            .field("page_size", &self.page_size)
            .finish()
    }
}

impl CustomerEnrichmentPartyApplicationWorker {
    pub fn new(
        store: PostgresDataStore,
        orchestrator: Arc<CustomerEnrichmentPartyApplicationOrchestrator>,
        actor_id: ActorId,
    ) -> Result<Self, SdkError> {
        Self::with_page_size(store, orchestrator, actor_id, DEFAULT_PAGE_SIZE)
    }

    pub fn with_page_size(
        store: PostgresDataStore,
        orchestrator: Arc<CustomerEnrichmentPartyApplicationOrchestrator>,
        actor_id: ActorId,
        page_size: u32,
    ) -> Result<Self, SdkError> {
        if page_size == 0 || page_size > crm_core_events::MAX_EVENT_HISTORY_PAGE_SIZE {
            return Err(worker_configuration_invalid(
                "application worker page size is outside the governed limit",
            ));
        }
        Ok(Self {
            store,
            orchestrator,
            actor_id,
            page_size,
        })
    }

    pub fn run_tenant_cycle<'a>(
        &'a self,
        tenant_id: TenantId,
        now_unix_nanos: i64,
    ) -> PortFuture<'a, Result<PartyDisplayNameApplicationCycle, SdkError>> {
        Box::pin(async move {
            if now_unix_nanos <= 0 {
                return Err(worker_configuration_invalid(
                    "application worker clock is invalid",
                ));
            }
            let module_id = module_id()?;
            let event_type = event_type()?;
            let checkpoint = ProjectionStore::projection_checkpoint(
                &self.store,
                tenant_id.clone(),
                PARTY_DISPLAY_NAME_APPLICATION_PROJECTION_ID.to_owned(),
            )
            .await?;
            let mut after = checkpoint.map(|value| value.cursor);
            let mut cycle = PartyDisplayNameApplicationCycle::default();

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

                for delivery in page.deliveries {
                    cycle.reviewed_events = cycle.reviewed_events.saturating_add(1);
                    let process = self
                        .process_delivery(&tenant_id, &delivery, now_unix_nanos)
                        .await;
                    match process {
                        Ok(DeliveryDisposition::Accepted { attempt_replayed }) => {
                            cycle.accepted_events = cycle.accepted_events.saturating_add(1);
                            if attempt_replayed {
                                cycle.replayed_attempts = cycle.replayed_attempts.saturating_add(1);
                            }
                        }
                        Ok(DeliveryDisposition::Skipped) => {
                            cycle.skipped_events = cycle.skipped_events.saturating_add(1);
                        }
                        Err(error) => {
                            if !error.retryable {
                                let _ = ProjectionStore::mark_projection_failed(
                                    &self.store,
                                    ProjectionFailure {
                                        tenant_id: tenant_id.clone(),
                                        projection_id:
                                            PARTY_DISPLAY_NAME_APPLICATION_PROJECTION_ID.to_owned(),
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
                            projection_id: PARTY_DISPLAY_NAME_APPLICATION_PROJECTION_ID.to_owned(),
                            delivery,
                            writes: Vec::new(),
                        },
                    )
                    .await?;
                }

                let Some(next) = page.next_cursor else {
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
        _now_unix_nanos: i64,
    ) -> Result<DeliveryDisposition, SdkError> {
        let event = decode_reviewed_event(delivery)?;
        let suggestion = event.suggestion.ok_or_else(review_event_invalid)?;
        let review = event.review_decision.ok_or_else(review_event_invalid)?;
        if wire::SuggestionReviewDecisionKind::try_from(review.kind).ok()
            != Some(wire::SuggestionReviewDecisionKind::Accepted)
        {
            return Ok(DeliveryDisposition::Skipped);
        }
        let suggestion_ref = suggestion
            .suggestion_ref
            .as_ref()
            .ok_or_else(review_event_invalid)?;
        let review_ref = review
            .review_decision_ref
            .as_ref()
            .ok_or_else(review_event_invalid)?;
        if review
            .suggestion_ref
            .as_ref()
            .is_none_or(|value| value.suggestion_id != suggestion_ref.suggestion_id)
        {
            return Err(review_event_invalid());
        }
        let target = suggestion.target.as_ref().ok_or_else(review_event_invalid)?;
        if wire::EnrichmentTargetField::try_from(target.target_field).ok()
            != Some(wire::EnrichmentTargetField::PartyDisplayName)
            || target.party_resource_version <= 0
            || target.party_resource_version != review.target_party_resource_version
            || review
                .approval_evidence_reference
                .as_deref()
                .is_none_or(str::is_empty)
        {
            return Err(review_event_invalid());
        }
        if tenant_id != &delivery.tenant_id {
            return Err(review_event_invalid());
        }

        let definition = apply_party_display_name_capability_definition()?;
        let input = support::protobuf_payload(
            MODULE_ID,
            APPLY_PARTY_DISPLAY_NAME_REQUEST_SCHEMA,
            DataClass::Personal,
            &wire::ApplyPartyDisplayNameSuggestionRequest {
                suggestion_ref: Some(suggestion_ref.clone()),
                review_decision_ref: Some(review_ref.clone()),
                expected_party_resource_version: target.party_resource_version,
                application_generation: 0,
            },
        )?;
        let request_identity = delivery.delivery_id.as_str().to_owned();
        let request = CapabilityRequest {
            context: ModuleExecutionContext {
                module_id: module_id()?,
                execution: ExecutionContext {
                    tenant_id: tenant_id.clone(),
                    actor_id: self.actor_id.clone(),
                    request_id: RequestId::try_new(request_identity.clone())
                        .map_err(worker_identifier_invalid)?,
                    correlation_id: delivery.correlation_id.clone(),
                    causation_id: CausationId::try_new(delivery.event_id.as_str())
                        .map_err(worker_identifier_invalid)?,
                    trace_id: delivery.trace_id.clone(),
                    capability_id: definition.capability_id.clone(),
                    capability_version: definition.capability_version.clone(),
                    idempotency_key: IdempotencyKey::try_new(request_identity)
                        .map_err(worker_identifier_invalid)?,
                    business_transaction_id: BusinessTransactionId::try_new(
                        review_ref.review_decision_id.clone(),
                    )
                    .map_err(worker_identifier_invalid)?,
                    schema_version: SchemaVersion::try_new("1.0.0")
                        .map_err(worker_identifier_invalid)?,
                    request_started_at_unix_nanos: delivery.occurred_at_unix_nanos,
                },
            },
            input_hash: semantic_input_hash(&input),
            input,
            approval: None,
        };
        let result = self.orchestrator.execute(request).await?;
        Ok(DeliveryDisposition::Accepted {
            attempt_replayed: result.attempt_replayed,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DeliveryDisposition {
    Accepted { attempt_replayed: bool },
    Skipped,
}

fn decode_reviewed_event(delivery: &EventDelivery) -> Result<wire::SuggestionReviewedEvent, SdkError> {
    delivery.validate()?;
    let module_id = module_id()?;
    let contract = support::protobuf_contract(
        MODULE_ID,
        SUGGESTION_REVIEWED_EVENT_SCHEMA,
        vec![DataClass::Personal],
    )?;
    if delivery.source_module_id != module_id
        || delivery.consumer_module_id != module_id
        || delivery.event_type.as_str() != SUGGESTION_REVIEWED_EVENT_TYPE
        || delivery.event_version.as_str() != "1.0.0"
        || !contract.matches(&delivery.payload)
        || delivery.payload.encoding != PayloadEncoding::Protobuf
    {
        return Err(review_event_invalid());
    }
    wire::SuggestionReviewedEvent::decode(delivery.payload.bytes.as_slice()).map_err(|error| {
        review_event_invalid().with_internal_reference(format!("review event decode: {error}"))
    })
}

fn module_id() -> Result<ModuleId, SdkError> {
    ModuleId::try_new(MODULE_ID).map_err(worker_identifier_invalid)
}

fn event_type() -> Result<EventType, SdkError> {
    EventType::try_new(SUGGESTION_REVIEWED_EVENT_TYPE).map_err(worker_identifier_invalid)
}

fn worker_identifier_invalid(error: crm_module_sdk::IdentifierError) -> SdkError {
    worker_configuration_invalid(error.to_string())
}

fn worker_configuration_invalid(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_APPLICATION_WORKER_CONFIGURATION_INVALID",
        crm_module_sdk::ErrorCategory::Internal,
        false,
        "The Customer Enrichment application worker is not configured safely.",
    )
    .with_internal_reference(reference.into())
}

fn review_event_invalid() -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_APPLICATION_REVIEW_EVENT_INVALID",
        crm_module_sdk::ErrorCategory::Internal,
        false,
        "Accepted suggestion review evidence is invalid.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worker_identity_and_phase_input_are_canonical() {
        assert!(ActorId::try_new(PARTY_DISPLAY_NAME_APPLICATION_WORKER_ACTOR_ID).is_ok());
        assert!(EventType::try_new(SUGGESTION_REVIEWED_EVENT_TYPE).is_ok());
        assert!(CapabilityVersion::try_new("1.0.0").is_ok());
        assert_eq!(APPLY_PARTY_DISPLAY_NAME_CAPABILITY, "customer_enrichment.party.display_name.apply");
    }
}
