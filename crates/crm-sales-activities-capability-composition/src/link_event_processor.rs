use crm_capability_adapters::GatewayCapabilityClient;
use crm_capability_runtime::CapabilityGateway;
use crm_core_data::{
    EventDeliveryClaim, EventDeliveryCompletion, EventDeliveryQuery, PostgresDataStore,
};
use crm_module_sdk::{
    ActorId, BusinessTransactionId, CapabilityId, CapabilityVersion, CausationId, ErrorCategory,
    EventDelivery, EventId, ExecutionContext, IdempotencyKey, ModuleExecutionContext, ModuleId,
    ModuleStateEntry, ModuleStateStore, PortFuture, PortResult, PutModuleStateRequest, RequestId,
    ResourceRef, SchemaVersion, SdkError, StateKey, TenantId,
};
use crm_sales_activities_link::{
    LinkDisposition, MODULE_ID as LINK_MODULE_ID, SalesActivitiesLink,
};
use crm_sales_activities_link_contract_adapter::ProtobufSalesActivitiesLinkContractAdapter;
use std::sync::{Arc, Mutex};

const LINK_HANDLER_CAPABILITY_ID: &str = "link.sales.stage_changed.process";
const LINK_HANDLER_CAPABILITY_VERSION: &str = "1.0.0";
const CONSUMER_INACTIVE_ERROR_CODE: &str = "EVENT_DELIVERY_CONSUMER_INACTIVE";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SalesActivitiesLinkDeliveryOutcome {
    InactiveConsumer,
    MissingSourceEvent,
    NotReady,
    Applied {
        affected_resources: Vec<ResourceRef>,
    },
    Ignored,
    RetryScheduled {
        error_code: String,
    },
    DeadLettered {
        error_code: String,
    },
}

#[derive(Debug, Clone)]
pub struct SalesActivitiesLinkEventProcessorConfig {
    pub worker_id: String,
    pub worker_actor_id: ActorId,
    pub lease_duration_nanos: i64,
    pub retry_delay_nanos: i64,
}

impl SalesActivitiesLinkEventProcessorConfig {
    fn validate(&self) -> Result<(), SdkError> {
        if self.worker_id.is_empty()
            || self.worker_id.len() > 180
            || self.lease_duration_nanos <= 0
            || self.retry_delay_nanos <= 0
        {
            return Err(configuration_invalid());
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct SalesActivitiesLinkEventProcessor {
    store: PostgresDataStore,
    contract_adapter: ProtobufSalesActivitiesLinkContractAdapter,
    link: SalesActivitiesLink<ProtobufSalesActivitiesLinkContractAdapter>,
    capabilities: GatewayCapabilityClient,
    config: SalesActivitiesLinkEventProcessorConfig,
}

impl SalesActivitiesLinkEventProcessor {
    pub fn new(
        store: PostgresDataStore,
        gateway: Arc<CapabilityGateway>,
        config: SalesActivitiesLinkEventProcessorConfig,
    ) -> Result<Self, SdkError> {
        config.validate()?;
        Ok(Self {
            store,
            contract_adapter: ProtobufSalesActivitiesLinkContractAdapter,
            link: SalesActivitiesLink::new(ProtobufSalesActivitiesLinkContractAdapter),
            capabilities: GatewayCapabilityClient::new(gateway),
            config,
        })
    }

    pub async fn process(
        &self,
        tenant_id: TenantId,
        event_id: EventId,
        now_unix_nanos: i64,
    ) -> Result<SalesActivitiesLinkDeliveryOutcome, SdkError> {
        if now_unix_nanos < 0 {
            return Err(configuration_invalid());
        }
        let lease_expires_at_unix_nanos = now_unix_nanos
            .checked_add(self.config.lease_duration_nanos)
            .ok_or_else(configuration_invalid)?;
        let query = EventDeliveryQuery {
            tenant_id,
            event_id,
            consumer_module_id: configured_module_id(LINK_MODULE_ID)?,
        };
        let claim = self
            .store
            .claim_event_delivery(
                &query,
                &self.config.worker_id,
                now_unix_nanos,
                lease_expires_at_unix_nanos,
            )
            .await?;
        let claimed = match claim {
            EventDeliveryClaim::InactiveConsumer => {
                return Ok(SalesActivitiesLinkDeliveryOutcome::InactiveConsumer);
            }
            EventDeliveryClaim::MissingSourceEvent => {
                return Ok(SalesActivitiesLinkDeliveryOutcome::MissingSourceEvent);
            }
            EventDeliveryClaim::NotReady => {
                return Ok(SalesActivitiesLinkDeliveryOutcome::NotReady);
            }
            EventDeliveryClaim::Claimed(claimed) => claimed,
        };

        if !self
            .store
            .is_module_active(
                &claimed.delivery.tenant_id,
                &claimed.delivery.consumer_module_id,
            )
            .await?
        {
            let retry_at = self.retry_at(now_unix_nanos)?;
            self.store
                .retry_event_delivery(
                    &claimed.delivery.tenant_id,
                    claimed.delivery.delivery_id.as_str(),
                    &self.config.worker_id,
                    CONSUMER_INACTIVE_ERROR_CODE,
                    retry_at,
                )
                .await?;
            return Ok(SalesActivitiesLinkDeliveryOutcome::InactiveConsumer);
        }

        let context = match delivery_context(
            &claimed.delivery,
            claimed.attempt_count,
            &self.config.worker_actor_id,
            now_unix_nanos,
        ) {
            Ok(context) => context,
            Err(error) => {
                return self
                    .finish_error(&claimed.delivery, error, now_unix_nanos)
                    .await;
            }
        };
        let event = match self
            .contract_adapter
            .decode_sales_deal_stage_changed(&claimed.delivery)
        {
            Ok(event) => event,
            Err(error) => {
                return self
                    .finish_error(&claimed.delivery, error, now_unix_nanos)
                    .await;
            }
        };
        let state = ClaimScopedModuleStateStore::default();
        let result = match self
            .link
            .handle(
                &context,
                &claimed.delivery,
                &event,
                &self.capabilities,
                &state,
            )
            .await
        {
            Ok(result) => result,
            Err(error) => {
                return self
                    .finish_error(&claimed.delivery, error, now_unix_nanos)
                    .await;
            }
        };

        let (completion, outcome) = match result.disposition {
            LinkDisposition::Applied => (
                EventDeliveryCompletion::Applied,
                SalesActivitiesLinkDeliveryOutcome::Applied {
                    affected_resources: result.affected_resources,
                },
            ),
            LinkDisposition::Ignored => (
                EventDeliveryCompletion::Ignored,
                SalesActivitiesLinkDeliveryOutcome::Ignored,
            ),
        };
        self.store
            .complete_event_delivery(
                &claimed.delivery.tenant_id,
                claimed.delivery.delivery_id.as_str(),
                &self.config.worker_id,
                completion,
            )
            .await?;
        Ok(outcome)
    }

    async fn finish_error(
        &self,
        delivery: &EventDelivery,
        error: SdkError,
        now_unix_nanos: i64,
    ) -> Result<SalesActivitiesLinkDeliveryOutcome, SdkError> {
        let error_code = error.code.clone();
        if is_terminal_delivery_error(&error) {
            self.store
                .dead_letter_event_delivery(
                    &delivery.tenant_id,
                    delivery.delivery_id.as_str(),
                    &self.config.worker_id,
                    &error_code,
                )
                .await?;
            Ok(SalesActivitiesLinkDeliveryOutcome::DeadLettered { error_code })
        } else {
            let retry_at = self.retry_at(now_unix_nanos)?;
            self.store
                .retry_event_delivery(
                    &delivery.tenant_id,
                    delivery.delivery_id.as_str(),
                    &self.config.worker_id,
                    &error_code,
                    retry_at,
                )
                .await?;
            Ok(SalesActivitiesLinkDeliveryOutcome::RetryScheduled { error_code })
        }
    }

    fn retry_at(&self, now_unix_nanos: i64) -> Result<i64, SdkError> {
        now_unix_nanos
            .checked_add(self.config.retry_delay_nanos)
            .ok_or_else(configuration_invalid)
    }
}

fn delivery_context(
    delivery: &EventDelivery,
    attempt_count: u32,
    worker_actor_id: &ActorId,
    now_unix_nanos: i64,
) -> Result<ModuleExecutionContext, SdkError> {
    let request_id = RequestId::try_new(format!(
        "{}-attempt-{attempt_count}",
        delivery.delivery_id.as_str()
    ))
    .map_err(|_| configuration_invalid())?;
    let business_transaction_id =
        BusinessTransactionId::try_new(format!("link-{}", delivery.delivery_id.as_str()))
            .map_err(|_| configuration_invalid())?;
    let context = ModuleExecutionContext {
        module_id: delivery.consumer_module_id.clone(),
        execution: ExecutionContext {
            tenant_id: delivery.tenant_id.clone(),
            actor_id: worker_actor_id.clone(),
            request_id,
            correlation_id: delivery.correlation_id.clone(),
            causation_id: CausationId::try_new(delivery.event_id.as_str().to_owned())
                .map_err(|_| configuration_invalid())?,
            trace_id: delivery.trace_id.clone(),
            capability_id: configured_capability_id(LINK_HANDLER_CAPABILITY_ID)?,
            capability_version: configured_capability_version(LINK_HANDLER_CAPABILITY_VERSION)?,
            idempotency_key: IdempotencyKey::try_new(delivery.delivery_id.as_str().to_owned())
                .map_err(|_| configuration_invalid())?,
            business_transaction_id,
            schema_version: SchemaVersion::try_new(delivery.event_version.as_str().to_owned())
                .map_err(|_| configuration_invalid())?,
            request_started_at_unix_nanos: now_unix_nanos,
        },
    };
    delivery.validate_for_consumer(&context)?;
    Ok(context)
}

fn is_terminal_delivery_error(error: &SdkError) -> bool {
    !error.retryable
        && matches!(
            error.category,
            ErrorCategory::InvalidArgument | ErrorCategory::Conflict
        )
}

fn configured_module_id(value: &str) -> Result<ModuleId, SdkError> {
    ModuleId::try_new(value).map_err(|_| configuration_invalid())
}

fn configured_capability_id(value: &str) -> Result<CapabilityId, SdkError> {
    CapabilityId::try_new(value).map_err(|_| configuration_invalid())
}

fn configured_capability_version(value: &str) -> Result<CapabilityVersion, SdkError> {
    CapabilityVersion::try_new(value).map_err(|_| configuration_invalid())
}

fn configuration_invalid() -> SdkError {
    SdkError::new(
        "LINK_EVENT_PROCESSOR_CONFIGURATION_INVALID",
        ErrorCategory::Internal,
        false,
        "The Sales and Activities link event processor is misconfigured.",
    )
}

#[derive(Debug, Default)]
struct ClaimScopedModuleStateStore {
    entry: Mutex<Option<ModuleStateEntry>>,
}

impl ModuleStateStore for ClaimScopedModuleStateStore {
    fn get<'a>(
        &'a self,
        context: &'a ModuleExecutionContext,
        key: StateKey,
    ) -> PortFuture<'a, PortResult<Option<ModuleStateEntry>>> {
        Box::pin(async move {
            context.validate()?;
            let entry = self
                .entry
                .lock()
                .map_err(|_| state_store_unavailable())?
                .clone();
            Ok(entry.filter(|entry| entry.key == key))
        })
    }

    fn put<'a>(
        &'a self,
        context: &'a ModuleExecutionContext,
        request: PutModuleStateRequest,
    ) -> PortFuture<'a, PortResult<ModuleStateEntry>> {
        Box::pin(async move {
            context.validate()?;
            request.value.validate()?;
            let mut entry = self.entry.lock().map_err(|_| state_store_unavailable())?;
            let next_version = match entry.as_ref() {
                None if request.expected_version.is_none() => 1,
                Some(current)
                    if current.key == request.key
                        && request.expected_version == Some(current.version) =>
                {
                    current
                        .version
                        .checked_add(1)
                        .ok_or_else(state_store_unavailable)?
                }
                _ => return Err(state_version_conflict()),
            };
            let next = ModuleStateEntry {
                key: request.key,
                version: next_version,
                value: request.value,
            };
            *entry = Some(next.clone());
            Ok(next)
        })
    }

    fn delete<'a>(
        &'a self,
        context: &'a ModuleExecutionContext,
        key: StateKey,
        expected_version: Option<i64>,
    ) -> PortFuture<'a, PortResult<()>> {
        Box::pin(async move {
            context.validate()?;
            let mut entry = self.entry.lock().map_err(|_| state_store_unavailable())?;
            match entry.as_ref() {
                None if expected_version.is_none() => Ok(()),
                Some(current)
                    if current.key == key && expected_version == Some(current.version) =>
                {
                    *entry = None;
                    Ok(())
                }
                _ => Err(state_version_conflict()),
            }
        })
    }
}

fn state_version_conflict() -> SdkError {
    SdkError::new(
        "SDK_VERSION_CONFLICT",
        ErrorCategory::Conflict,
        true,
        "The module state changed concurrently.",
    )
}

fn state_store_unavailable() -> SdkError {
    SdkError::new(
        "LINK_EVENT_PROCESSOR_STATE_UNAVAILABLE",
        ErrorCategory::Unavailable,
        true,
        "The link event processor state is temporarily unavailable.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crm_module_sdk::{
        CorrelationId, DeliveryId, EventVersion, RecordId, RecordRef, RecordType, TraceId,
        TypedPayload,
    };

    #[test]
    fn target_context_is_stable_across_delivery_retries_except_request_attempt_identity() {
        let delivery = delivery();
        let actor = ActorId::try_new("link-worker").unwrap();
        let first = delivery_context(&delivery, 1, &actor, 100).unwrap();
        let second = delivery_context(&delivery, 2, &actor, 200).unwrap();

        assert_ne!(first.execution.request_id, second.execution.request_id);
        assert_eq!(first.execution.tenant_id, second.execution.tenant_id);
        assert_eq!(first.execution.actor_id, second.execution.actor_id);
        assert_eq!(
            first.execution.correlation_id,
            second.execution.correlation_id
        );
        assert_eq!(first.execution.causation_id, second.execution.causation_id);
        assert_eq!(first.execution.trace_id, second.execution.trace_id);
        assert_eq!(
            first.execution.idempotency_key,
            second.execution.idempotency_key
        );
        assert_eq!(
            first.execution.business_transaction_id,
            second.execution.business_transaction_id
        );
    }

    #[test]
    fn only_non_retryable_invalid_or_conflicting_evidence_is_terminal() {
        assert!(is_terminal_delivery_error(&SdkError::new(
            "INVALID",
            ErrorCategory::InvalidArgument,
            false,
            "invalid"
        )));
        assert!(is_terminal_delivery_error(&SdkError::new(
            "CONFLICT",
            ErrorCategory::Conflict,
            false,
            "conflict"
        )));
        assert!(!is_terminal_delivery_error(&SdkError::new(
            "AUTH",
            ErrorCategory::Authorization,
            false,
            "auth"
        )));
        assert!(!is_terminal_delivery_error(&SdkError::new(
            "TEMP",
            ErrorCategory::Unavailable,
            true,
            "temp"
        )));
    }

    fn delivery() -> EventDelivery {
        EventDelivery {
            delivery_id: DeliveryId::try_new(
                "delivery-0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            )
            .unwrap(),
            event_id: EventId::try_new("event-1").unwrap(),
            tenant_id: TenantId::try_new("tenant-a").unwrap(),
            source_module_id: ModuleId::try_new("crm.sales").unwrap(),
            consumer_module_id: ModuleId::try_new(LINK_MODULE_ID).unwrap(),
            source_actor_id: ActorId::try_new("sales-user").unwrap(),
            event_type: crm_module_sdk::EventType::try_new("sales.deal.stage_changed").unwrap(),
            event_version: EventVersion::try_new("1.0.0").unwrap(),
            aggregate: RecordRef {
                record_type: RecordType::try_new("sales.deal").unwrap(),
                record_id: RecordId::try_new("deal-1").unwrap(),
            },
            aggregate_version: 2,
            occurred_at_unix_nanos: 99,
            correlation_id: CorrelationId::try_new("correlation-1").unwrap(),
            trace_id: TraceId::try_new("trace-1").unwrap(),
            payload: TypedPayload {
                owner: ModuleId::try_new("crm.sales").unwrap(),
                schema_id: crm_module_sdk::SchemaId::try_new("crm.sales.v1.DealStageChangedEvent")
                    .unwrap(),
                schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
                descriptor_hash: [1; 32],
                data_class: crm_module_sdk::DataClass::Confidential,
                encoding: crm_module_sdk::PayloadEncoding::Protobuf,
                maximum_size_bytes: 1024,
                retention_policy_id: crm_module_sdk::RetentionPolicyId::try_new("standard")
                    .unwrap(),
                bytes: vec![1],
            },
        }
    }
}
