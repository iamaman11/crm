#![forbid(unsafe_code)]

use crm_module_sdk::{
    ActorId, CapabilityClient, CapabilityId, CapabilityInvocation, CapabilityVersion, DataClass,
    ErrorCategory, EventDelivery, ModuleExecutionContext, ModuleId, ModuleStateEntry,
    ModuleStateStore, PayloadEncoding, PutModuleStateRequest, RecordId, ResourceRef,
    RetentionPolicyId, SchemaId, SchemaVersion, SdkError, StateKey, TypedPayload,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const MODULE_ID: &str = "crm.sales-activities-link";
pub const SOURCE_MODULE_ID: &str = "crm.sales";
pub const SOURCE_EVENT_TYPE: &str = "sales.deal.stage_changed";
pub const SOURCE_EVENT_VERSION: &str = "1.0.0";
pub const SOURCE_RECORD_TYPE: &str = "sales.deal";
pub const TARGET_MODULE_ID: &str = "crm.activities";
pub const TARGET_CAPABILITY_ID: &str = "activities.task.create";
pub const TARGET_CAPABILITY_VERSION: &str = "1.0.0";
pub const TARGET_REQUEST_SCHEMA_ID: &str = "crm.activities.v1.CreateTaskRequest";

const DELIVERY_STATE_SCHEMA_ID: &str = "crm.sales-activities-link.delivery-state";
const DELIVERY_STATE_SCHEMA_VERSION: &str = "1.0.0";
const DELIVERY_STATE_RETENTION_POLICY_ID: &str = "standard";
const DELIVERY_STATE_MAXIMUM_BYTES: u64 = 16 * 1024;
const FOLLOW_UP_SUBJECT: &str = "Follow up deal after stage change";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DealLifecycleStatus {
    Open,
    Won,
    Lost,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SalesDealStageChanged {
    pub deal_id: RecordId,
    pub version: i64,
    pub status: DealLifecycleStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateTaskIntent {
    pub task_id: RecordId,
    pub tenant_id: crm_module_sdk::TenantId,
    pub subject: String,
    pub owner_actor_id: ActorId,
    pub related_deal: ResourceRef,
}

pub trait ActivitiesTaskCommandEncoder: Send + Sync {
    fn encode_create_task(&self, intent: &CreateTaskIntent) -> Result<TypedPayload, SdkError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LinkDisposition {
    Applied,
    Ignored,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkProcessResult {
    pub disposition: LinkDisposition,
    pub replayed: bool,
    pub affected_resources: Vec<ResourceRef>,
}

#[derive(Debug)]
pub struct SalesActivitiesLink<E> {
    encoder: E,
}

impl<E> SalesActivitiesLink<E>
where
    E: ActivitiesTaskCommandEncoder,
{
    pub fn new(encoder: E) -> Self {
        Self { encoder }
    }

    /// Processes one immutable Sales event delivery.
    ///
    /// Exactly-once business effect is anchored in the target capability idempotency key
    /// (`delivery_id`). The durable link receipt is written only after the target capability
    /// succeeds. Therefore a crash after target commit but before receipt persistence is safe:
    /// retry re-enters the target gateway with the same idempotency key, receives the original
    /// replayed result and then repairs the missing receipt.
    pub async fn handle(
        &self,
        context: &ModuleExecutionContext,
        delivery: &EventDelivery,
        event: &SalesDealStageChanged,
        capabilities: &dyn CapabilityClient,
        state: &dyn ModuleStateStore,
    ) -> Result<LinkProcessResult, SdkError> {
        validate_source_delivery(context, delivery, event)?;

        let state_key = delivery_state_key(delivery)?;
        let existing = state.get(context, state_key.clone()).await?;
        if let Some(entry) = existing.as_ref() {
            let receipt = decode_receipt(entry)?;
            receipt.validate_binding(delivery, event)?;
            match receipt.status {
                DeliveryStatus::Applied => {
                    return Ok(LinkProcessResult {
                        disposition: LinkDisposition::Applied,
                        replayed: true,
                        affected_resources: receipt.affected_resources,
                    });
                }
                DeliveryStatus::Ignored => {
                    return Ok(LinkProcessResult {
                        disposition: LinkDisposition::Ignored,
                        replayed: true,
                        affected_resources: Vec::new(),
                    });
                }
                // Compatibility with the pre-production 0.1 receipt shape. A pending receipt
                // is never treated as proof of a completed target business effect.
                DeliveryStatus::Pending => {}
            }
        }

        if event.status != DealLifecycleStatus::Open {
            return Ok(LinkProcessResult {
                disposition: LinkDisposition::Ignored,
                replayed: false,
                affected_resources: Vec::new(),
            });
        }

        let intent = follow_up_intent(delivery, event)?;
        let input = self.encoder.encode_create_task(&intent)?;
        validate_target_payload(&input)?;
        let outcome = capabilities
            .invoke(
                context,
                CapabilityInvocation {
                    capability_id: configured_capability_id(TARGET_CAPABILITY_ID)?,
                    capability_version: configured_capability_version(TARGET_CAPABILITY_VERSION)?,
                    input,
                },
            )
            .await?;

        let applied = DeliveryReceipt::new(
            delivery,
            event,
            DeliveryStatus::Applied,
            outcome.affected_resources.clone(),
        );
        let expected_version = existing.as_ref().map(|entry| entry.version);
        match state
            .put(
                context,
                PutModuleStateRequest {
                    key: state_key.clone(),
                    expected_version,
                    value: encode_receipt(&applied)?,
                },
            )
            .await
        {
            Ok(_) => Ok(LinkProcessResult {
                disposition: LinkDisposition::Applied,
                replayed: false,
                affected_resources: outcome.affected_resources,
            }),
            Err(error) if error.code == "SDK_VERSION_CONFLICT" => {
                let current = state
                    .get(context, state_key)
                    .await?
                    .ok_or_else(delivery_state_race)?;
                let current = decode_receipt(&current)?;
                current.validate_binding(delivery, event)?;
                if current.status == DeliveryStatus::Applied {
                    Ok(LinkProcessResult {
                        disposition: LinkDisposition::Applied,
                        replayed: true,
                        affected_resources: current.affected_resources,
                    })
                } else {
                    Err(delivery_state_race())
                }
            }
            Err(error) => Err(error),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum DeliveryStatus {
    Pending,
    Applied,
    Ignored,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DeliveryReceipt {
    event_id: String,
    aggregate_id: String,
    aggregate_version: i64,
    source_status: DealLifecycleStatus,
    status: DeliveryStatus,
    affected_resources: Vec<ResourceRef>,
}

impl DeliveryReceipt {
    fn new(
        delivery: &EventDelivery,
        event: &SalesDealStageChanged,
        status: DeliveryStatus,
        affected_resources: Vec<ResourceRef>,
    ) -> Self {
        Self {
            event_id: delivery.event_id.as_str().to_owned(),
            aggregate_id: event.deal_id.as_str().to_owned(),
            aggregate_version: event.version,
            source_status: event.status,
            status,
            affected_resources,
        }
    }

    fn validate_binding(
        &self,
        delivery: &EventDelivery,
        event: &SalesDealStageChanged,
    ) -> Result<(), SdkError> {
        if self.event_id != delivery.event_id.as_str()
            || self.aggregate_id != event.deal_id.as_str()
            || self.aggregate_version != event.version
            || self.source_status != event.status
        {
            return Err(SdkError::new(
                "LINK_DELIVERY_ID_REUSED",
                ErrorCategory::Conflict,
                false,
                "The delivery identity is already bound to different source evidence.",
            ));
        }
        Ok(())
    }
}

fn validate_source_delivery(
    context: &ModuleExecutionContext,
    delivery: &EventDelivery,
    event: &SalesDealStageChanged,
) -> Result<(), SdkError> {
    delivery.validate_for_consumer(context)?;

    if context.module_id.as_str() != MODULE_ID
        || delivery.source_module_id.as_str() != SOURCE_MODULE_ID
        || delivery.consumer_module_id.as_str() != MODULE_ID
        || delivery.event_type.as_str() != SOURCE_EVENT_TYPE
        || delivery.event_version.as_str() != SOURCE_EVENT_VERSION
        || delivery.aggregate.record_type.as_str() != SOURCE_RECORD_TYPE
    {
        return Err(SdkError::new(
            "LINK_SOURCE_EVENT_UNSUPPORTED",
            ErrorCategory::InvalidArgument,
            false,
            "The source event is not supported by this link module.",
        ));
    }

    if event.version <= 0
        || event.version != delivery.aggregate_version
        || event.deal_id.as_str() != delivery.aggregate.record_id.as_str()
    {
        return Err(SdkError::new(
            "LINK_SOURCE_EVENT_BINDING_INVALID",
            ErrorCategory::InvalidArgument,
            false,
            "The decoded source event does not match its immutable delivery envelope.",
        ));
    }

    Ok(())
}

fn follow_up_intent(
    delivery: &EventDelivery,
    event: &SalesDealStageChanged,
) -> Result<CreateTaskIntent, SdkError> {
    if event.status != DealLifecycleStatus::Open {
        return Err(delivery_state_invalid());
    }

    let task_id = RecordId::try_new(delivery.delivery_id.as_str().to_owned())
        .map_err(|_| delivery_state_invalid())?;

    Ok(CreateTaskIntent {
        task_id,
        tenant_id: delivery.tenant_id.clone(),
        subject: FOLLOW_UP_SUBJECT.to_owned(),
        owner_actor_id: delivery.source_actor_id.clone(),
        related_deal: ResourceRef {
            resource_type: SOURCE_RECORD_TYPE.to_owned(),
            resource_id: event.deal_id.as_str().to_owned(),
            version: Some(event.version),
        },
    })
}

fn delivery_state_key(delivery: &EventDelivery) -> Result<StateKey, SdkError> {
    StateKey::try_new(delivery.delivery_id.as_str().to_owned()).map_err(|_| delivery_state_invalid())
}

fn encode_receipt(receipt: &DeliveryReceipt) -> Result<TypedPayload, SdkError> {
    let bytes = serde_json::to_vec(receipt).map_err(|_| delivery_state_invalid())?;
    if bytes.len() as u64 > DELIVERY_STATE_MAXIMUM_BYTES {
        return Err(delivery_state_invalid());
    }

    let payload = TypedPayload {
        owner: configured_module_id(MODULE_ID)?,
        schema_id: configured_schema_id(DELIVERY_STATE_SCHEMA_ID)?,
        schema_version: configured_schema_version(DELIVERY_STATE_SCHEMA_VERSION)?,
        descriptor_hash: delivery_state_descriptor_hash(),
        data_class: DataClass::Internal,
        encoding: PayloadEncoding::Json,
        maximum_size_bytes: DELIVERY_STATE_MAXIMUM_BYTES,
        retention_policy_id: configured_retention_policy(DELIVERY_STATE_RETENTION_POLICY_ID)?,
        bytes,
    };
    payload.validate()?;
    Ok(payload)
}

fn decode_receipt(entry: &ModuleStateEntry) -> Result<DeliveryReceipt, SdkError> {
    let payload = &entry.value;
    if payload.owner.as_str() != MODULE_ID
        || payload.schema_id.as_str() != DELIVERY_STATE_SCHEMA_ID
        || payload.schema_version.as_str() != DELIVERY_STATE_SCHEMA_VERSION
        || payload.descriptor_hash != delivery_state_descriptor_hash()
        || payload.data_class != DataClass::Internal
        || payload.encoding != PayloadEncoding::Json
        || payload.maximum_size_bytes != DELIVERY_STATE_MAXIMUM_BYTES
        || payload.retention_policy_id.as_str() != DELIVERY_STATE_RETENTION_POLICY_ID
        || payload.validate().is_err()
    {
        return Err(delivery_state_invalid());
    }

    serde_json::from_slice(&payload.bytes).map_err(|_| delivery_state_invalid())
}

fn validate_target_payload(payload: &TypedPayload) -> Result<(), SdkError> {
    payload.validate()?;
    if payload.owner.as_str() != TARGET_MODULE_ID
        || payload.schema_id.as_str() != TARGET_REQUEST_SCHEMA_ID
        || payload.schema_version.as_str() != TARGET_CAPABILITY_VERSION
        || payload.encoding != PayloadEncoding::Protobuf
    {
        return Err(SdkError::new(
            "LINK_TARGET_CONTRACT_INVALID",
            ErrorCategory::Internal,
            false,
            "The link target contract adapter is invalid.",
        ));
    }
    Ok(())
}

fn delivery_state_descriptor_hash() -> [u8; 32] {
    Sha256::digest(b"crm.sales-activities-link.delivery-state/json/v1").into()
}

fn configured_module_id(value: &str) -> Result<ModuleId, SdkError> {
    ModuleId::try_new(value).map_err(|_| configuration_invalid())
}

fn configured_schema_id(value: &str) -> Result<SchemaId, SdkError> {
    SchemaId::try_new(value).map_err(|_| configuration_invalid())
}

fn configured_schema_version(value: &str) -> Result<SchemaVersion, SdkError> {
    SchemaVersion::try_new(value).map_err(|_| configuration_invalid())
}

fn configured_retention_policy(value: &str) -> Result<RetentionPolicyId, SdkError> {
    RetentionPolicyId::try_new(value).map_err(|_| configuration_invalid())
}

fn configured_capability_id(value: &str) -> Result<CapabilityId, SdkError> {
    CapabilityId::try_new(value).map_err(|_| configuration_invalid())
}

fn configured_capability_version(value: &str) -> Result<CapabilityVersion, SdkError> {
    CapabilityVersion::try_new(value).map_err(|_| configuration_invalid())
}

fn configuration_invalid() -> SdkError {
    SdkError::new(
        "LINK_CONFIGURATION_INVALID",
        ErrorCategory::Internal,
        false,
        "The link module configuration is invalid.",
    )
}

fn delivery_state_invalid() -> SdkError {
    SdkError::new(
        "LINK_DELIVERY_STATE_INVALID",
        ErrorCategory::Internal,
        false,
        "The link delivery state is invalid.",
    )
}

fn delivery_state_race() -> SdkError {
    SdkError::new(
        "LINK_DELIVERY_STATE_RETRY_REQUIRED",
        ErrorCategory::Conflict,
        true,
        "The delivery state changed concurrently; retry the delivery.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crm_module_sdk::testing::{InMemoryModuleStateStore, RecordingCapabilityClient};
    use crm_module_sdk::{
        BusinessTransactionId, CapabilityOutcome, CausationId, CorrelationId, DeliveryId, EventId,
        EventType, EventVersion, ExecutionContext, IdempotencyKey, RecordRef, RecordType, RequestId,
        TraceId,
    };
    use std::future::Future;
    use std::task::{Context, Poll, Waker};

    #[derive(Debug, Default)]
    struct TestEncoder;

    impl ActivitiesTaskCommandEncoder for TestEncoder {
        fn encode_create_task(&self, _intent: &CreateTaskIntent) -> Result<TypedPayload, SdkError> {
            Ok(TypedPayload {
                owner: ModuleId::try_new(TARGET_MODULE_ID).unwrap(),
                schema_id: SchemaId::try_new(TARGET_REQUEST_SCHEMA_ID).unwrap(),
                schema_version: SchemaVersion::try_new(TARGET_CAPABILITY_VERSION).unwrap(),
                descriptor_hash: [2; 32],
                data_class: DataClass::Confidential,
                encoding: PayloadEncoding::Protobuf,
                maximum_size_bytes: 1024,
                retention_policy_id: RetentionPolicyId::try_new("standard").unwrap(),
                bytes: vec![1],
            })
        }
    }

    fn run_ready<F: Future>(future: F) -> F::Output {
        let mut context = Context::from_waker(Waker::noop());
        let mut future = Box::pin(future);
        match future.as_mut().poll(&mut context) {
            Poll::Ready(output) => output,
            Poll::Pending => panic!("test double future unexpectedly returned Pending"),
        }
    }

    #[test]
    fn open_stage_change_invokes_activities_once_and_duplicate_uses_receipt() {
        let link = SalesActivitiesLink::new(TestEncoder);
        let capabilities = RecordingCapabilityClient::default();
        capabilities.push_response(Ok(CapabilityOutcome {
            output: None,
            affected_resources: vec![task_resource()],
        }));
        let state = InMemoryModuleStateStore::default();
        let (delivery, event) = delivery(DealLifecycleStatus::Open);
        let context = context(&delivery);

        let first = run_ready(link.handle(&context, &delivery, &event, &capabilities, &state)).unwrap();
        let second = run_ready(link.handle(&context, &delivery, &event, &capabilities, &state)).unwrap();

        assert_eq!(first.disposition, LinkDisposition::Applied);
        assert!(!first.replayed);
        assert_eq!(second.disposition, LinkDisposition::Applied);
        assert!(second.replayed);
        assert_eq!(capabilities.calls().len(), 1);
        assert_eq!(state.entry_count(), 1);
    }

    #[test]
    fn target_failure_creates_no_receipt_and_retry_can_reenter_target() {
        let link = SalesActivitiesLink::new(TestEncoder);
        let capabilities = RecordingCapabilityClient::default();
        capabilities.push_response(Err(SdkError::new(
            "TEST_TARGET_UNAVAILABLE",
            ErrorCategory::Unavailable,
            true,
            "The target is temporarily unavailable.",
        )));
        capabilities.push_response(Ok(CapabilityOutcome {
            output: None,
            affected_resources: vec![task_resource()],
        }));
        let state = InMemoryModuleStateStore::default();
        let (delivery, event) = delivery(DealLifecycleStatus::Open);
        let context = context(&delivery);

        run_ready(link.handle(&context, &delivery, &event, &capabilities, &state))
            .expect_err("first target call must fail");
        assert_eq!(state.entry_count(), 0);

        let retry = run_ready(link.handle(&context, &delivery, &event, &capabilities, &state)).unwrap();
        assert_eq!(retry.disposition, LinkDisposition::Applied);
        assert_eq!(capabilities.calls().len(), 2);
        assert_eq!(state.entry_count(), 1);
    }

    #[test]
    fn closed_deal_is_ignored_without_target_or_state_mutation() {
        let link = SalesActivitiesLink::new(TestEncoder);
        let capabilities = RecordingCapabilityClient::default();
        let state = InMemoryModuleStateStore::default();
        let (delivery, event) = delivery(DealLifecycleStatus::Won);
        let context = context(&delivery);

        let result = run_ready(link.handle(&context, &delivery, &event, &capabilities, &state)).unwrap();

        assert_eq!(result.disposition, LinkDisposition::Ignored);
        assert!(!result.replayed);
        assert!(capabilities.calls().is_empty());
        assert_eq!(state.entry_count(), 0);
    }

    #[test]
    fn cross_tenant_delivery_is_denied_before_target_call() {
        let link = SalesActivitiesLink::new(TestEncoder);
        let capabilities = RecordingCapabilityClient::default();
        let state = InMemoryModuleStateStore::default();
        let (delivery, event) = delivery(DealLifecycleStatus::Open);
        let mut context = context(&delivery);
        context.execution.tenant_id = crm_module_sdk::TenantId::try_new("tenant-2").unwrap();

        run_ready(link.handle(&context, &delivery, &event, &capabilities, &state))
            .expect_err("cross-tenant delivery must fail");

        assert!(capabilities.calls().is_empty());
        assert_eq!(state.entry_count(), 0);
    }

    fn task_resource() -> ResourceRef {
        ResourceRef {
            resource_type: "activities.task".to_owned(),
            resource_id: "delivery-1".to_owned(),
            version: Some(1),
        }
    }

    fn delivery(status: DealLifecycleStatus) -> (EventDelivery, SalesDealStageChanged) {
        let event = SalesDealStageChanged {
            deal_id: RecordId::try_new("deal-1").unwrap(),
            version: 2,
            status,
        };
        let delivery = EventDelivery {
            delivery_id: DeliveryId::try_new("delivery-1").unwrap(),
            event_id: EventId::try_new("event-1").unwrap(),
            tenant_id: crm_module_sdk::TenantId::try_new("tenant-1").unwrap(),
            source_module_id: ModuleId::try_new(SOURCE_MODULE_ID).unwrap(),
            consumer_module_id: ModuleId::try_new(MODULE_ID).unwrap(),
            source_actor_id: ActorId::try_new("sales-user").unwrap(),
            event_type: EventType::try_new(SOURCE_EVENT_TYPE).unwrap(),
            event_version: EventVersion::try_new(SOURCE_EVENT_VERSION).unwrap(),
            aggregate: RecordRef {
                record_type: RecordType::try_new(SOURCE_RECORD_TYPE).unwrap(),
                record_id: RecordId::try_new("deal-1").unwrap(),
            },
            aggregate_version: 2,
            occurred_at_unix_nanos: 100,
            correlation_id: CorrelationId::try_new("correlation-1").unwrap(),
            trace_id: TraceId::try_new("trace-1").unwrap(),
            payload: TypedPayload {
                owner: ModuleId::try_new(SOURCE_MODULE_ID).unwrap(),
                schema_id: SchemaId::try_new("crm.sales.v1.DealStageChangedEvent").unwrap(),
                schema_version: SchemaVersion::try_new(SOURCE_EVENT_VERSION).unwrap(),
                descriptor_hash: [1; 32],
                data_class: DataClass::Confidential,
                encoding: PayloadEncoding::Protobuf,
                maximum_size_bytes: 1024,
                retention_policy_id: RetentionPolicyId::try_new("standard").unwrap(),
                bytes: vec![1],
            },
        };
        (delivery, event)
    }

    fn context(delivery: &EventDelivery) -> ModuleExecutionContext {
        ModuleExecutionContext {
            module_id: ModuleId::try_new(MODULE_ID).unwrap(),
            execution: ExecutionContext {
                tenant_id: delivery.tenant_id.clone(),
                actor_id: ActorId::try_new("link-service").unwrap(),
                request_id: RequestId::try_new("request-1").unwrap(),
                correlation_id: delivery.correlation_id.clone(),
                causation_id: CausationId::try_new(delivery.event_id.as_str()).unwrap(),
                trace_id: delivery.trace_id.clone(),
                capability_id: CapabilityId::try_new("link.sales.stage-changed.process").unwrap(),
                capability_version: CapabilityVersion::try_new("1.0.0").unwrap(),
                idempotency_key: IdempotencyKey::try_new(delivery.delivery_id.as_str()).unwrap(),
                business_transaction_id: BusinessTransactionId::try_new("transaction-1").unwrap(),
                schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
                request_started_at_unix_nanos: 101,
            },
        }
    }
}
