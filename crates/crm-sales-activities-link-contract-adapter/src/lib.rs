#![forbid(unsafe_code)]

use crm_module_sdk::{
    DataClass, ErrorCategory, EventDelivery, ModuleId, PayloadEncoding, RetentionPolicyId,
    SchemaId, SchemaVersion, SdkError, TypedPayload,
};
use crm_proto_contracts::{
    MAX_PROTOBUF_BYTES,
    crm::{activities::v1 as activities, core::v1 as core, sales::v1 as sales},
    message_descriptor_hash,
};
use crm_sales_activities_link::{
    ActivitiesTaskCommandEncoder, CreateTaskIntent, DealLifecycleStatus, SOURCE_EVENT_TYPE,
    SOURCE_EVENT_VERSION, SOURCE_MODULE_ID, SalesDealStageChanged, TARGET_CAPABILITY_VERSION,
    TARGET_MODULE_ID, TARGET_REQUEST_SCHEMA_ID,
};
use prost::Message;

pub const SOURCE_EVENT_SCHEMA_ID: &str = "crm.sales.v1.DealStageChangedEvent";
pub const DEFAULT_RETENTION_POLICY_ID: &str = "standard";

#[derive(Debug, Default, Clone, Copy)]
pub struct ProtobufSalesActivitiesLinkContractAdapter;

impl ProtobufSalesActivitiesLinkContractAdapter {
    pub fn decode_sales_deal_stage_changed(
        &self,
        delivery: &EventDelivery,
    ) -> Result<SalesDealStageChanged, SdkError> {
        validate_source_payload(delivery)?;
        let event = sales::DealStageChangedEvent::decode(delivery.payload.bytes.as_slice())
            .map_err(|_| source_protobuf_invalid())?;
        let status = deal_status(event.status)?;
        validate_close_outcome(status, event.close_outcome.as_ref())?;
        let deal_id = crm_module_sdk::RecordId::try_new(event.deal_id)
            .map_err(|_| source_semantics_invalid())?;
        if event.version <= 0 {
            return Err(source_semantics_invalid());
        }

        Ok(SalesDealStageChanged {
            deal_id,
            version: event.version,
            status,
        })
    }
}

impl ActivitiesTaskCommandEncoder for ProtobufSalesActivitiesLinkContractAdapter {
    fn encode_create_task(&self, intent: &CreateTaskIntent) -> Result<TypedPayload, SdkError> {
        let message = activities::CreateTaskRequest {
            task_id: intent.task_id.as_str().to_owned(),
            subject: intent.subject.clone(),
            description: None,
            owner: Some(core::ActorOrTeamOwner {
                owner: Some(core::actor_or_team_owner::Owner::ActorId(
                    intent.owner_actor_id.as_str().to_owned(),
                )),
            }),
            related_resources: vec![core::ResourceRef {
                tenant_id: intent.tenant_id.as_str().to_owned(),
                resource_type: intent.related_deal.resource_type.clone(),
                resource_id: intent.related_deal.resource_id.clone(),
                version: intent.related_deal.version,
            }],
            priority: activities::TaskPriority::Normal as i32,
            due_at: None,
            reminder_at: None,
        };

        protobuf_payload(
            TARGET_MODULE_ID,
            TARGET_REQUEST_SCHEMA_ID,
            TARGET_CAPABILITY_VERSION,
            &message,
        )
    }
}

fn validate_source_payload(delivery: &EventDelivery) -> Result<(), SdkError> {
    delivery.validate()?;
    let payload = &delivery.payload;
    if delivery.source_module_id.as_str() != SOURCE_MODULE_ID
        || delivery.event_type.as_str() != SOURCE_EVENT_TYPE
        || delivery.event_version.as_str() != SOURCE_EVENT_VERSION
        || payload.owner.as_str() != SOURCE_MODULE_ID
        || payload.schema_id.as_str() != SOURCE_EVENT_SCHEMA_ID
        || payload.schema_version.as_str() != SOURCE_EVENT_VERSION
        || payload.descriptor_hash != message_descriptor_hash(SOURCE_EVENT_SCHEMA_ID)
        || payload.data_class != DataClass::Confidential
        || payload.encoding != PayloadEncoding::Protobuf
        || payload.maximum_size_bytes != MAX_PROTOBUF_BYTES
        || payload.retention_policy_id.as_str() != DEFAULT_RETENTION_POLICY_ID
    {
        return Err(SdkError::new(
            "LINK_SOURCE_EVENT_CONTRACT_MISMATCH",
            ErrorCategory::InvalidArgument,
            false,
            "The source event payload does not match the required published contract.",
        ));
    }
    Ok(())
}

fn deal_status(value: i32) -> Result<DealLifecycleStatus, SdkError> {
    match sales::DealStatus::try_from(value).map_err(|_| source_semantics_invalid())? {
        sales::DealStatus::Open => Ok(DealLifecycleStatus::Open),
        sales::DealStatus::Won => Ok(DealLifecycleStatus::Won),
        sales::DealStatus::Lost => Ok(DealLifecycleStatus::Lost),
        sales::DealStatus::Unspecified => Err(source_semantics_invalid()),
    }
}

fn validate_close_outcome(
    status: DealLifecycleStatus,
    outcome: Option<&sales::DealCloseOutcome>,
) -> Result<(), SdkError> {
    match (status, outcome) {
        (DealLifecycleStatus::Open, None) => Ok(()),
        (DealLifecycleStatus::Open, Some(_)) | (_, None) => Err(source_semantics_invalid()),
        (DealLifecycleStatus::Won | DealLifecycleStatus::Lost, Some(outcome)) => {
            if deal_status(outcome.status)? != status
                || outcome.reason_code.is_empty()
                || outcome.closed_at.is_none()
            {
                return Err(source_semantics_invalid());
            }
            Ok(())
        }
    }
}

fn protobuf_payload<M: Message>(
    owner: &str,
    schema_id: &str,
    schema_version: &str,
    message: &M,
) -> Result<TypedPayload, SdkError> {
    let bytes = message.encode_to_vec();
    if bytes.len() as u64 > MAX_PROTOBUF_BYTES {
        return Err(SdkError::new(
            "LINK_TARGET_PAYLOAD_TOO_LARGE",
            ErrorCategory::InvalidArgument,
            false,
            "The encoded target command exceeds the permitted size.",
        ));
    }

    let payload = TypedPayload {
        owner: ModuleId::try_new(owner).map_err(|_| configuration_invalid())?,
        schema_id: SchemaId::try_new(schema_id).map_err(|_| configuration_invalid())?,
        schema_version: SchemaVersion::try_new(schema_version)
            .map_err(|_| configuration_invalid())?,
        descriptor_hash: message_descriptor_hash(schema_id),
        data_class: DataClass::Confidential,
        encoding: PayloadEncoding::Protobuf,
        maximum_size_bytes: MAX_PROTOBUF_BYTES,
        retention_policy_id: RetentionPolicyId::try_new(DEFAULT_RETENTION_POLICY_ID)
            .map_err(|_| configuration_invalid())?,
        bytes,
    };
    payload.validate()?;
    Ok(payload)
}

fn source_protobuf_invalid() -> SdkError {
    SdkError::new(
        "LINK_SOURCE_EVENT_PROTOBUF_INVALID",
        ErrorCategory::InvalidArgument,
        false,
        "The source event is not valid Protobuf for the published contract.",
    )
}

fn source_semantics_invalid() -> SdkError {
    SdkError::new(
        "LINK_SOURCE_EVENT_SEMANTICS_INVALID",
        ErrorCategory::InvalidArgument,
        false,
        "The source event violates the published Sales lifecycle semantics.",
    )
}

fn configuration_invalid() -> SdkError {
    SdkError::new(
        "LINK_CONTRACT_ADAPTER_CONFIGURATION_INVALID",
        ErrorCategory::Internal,
        false,
        "The link contract adapter configuration is invalid.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crm_module_sdk::{
        ActorId, CorrelationId, DeliveryId, EventId, EventType, EventVersion, RecordId, RecordRef,
        RecordType, ResourceRef, TenantId, TraceId,
    };

    fn delivery(message: sales::DealStageChangedEvent) -> EventDelivery {
        let bytes = message.encode_to_vec();
        EventDelivery {
            delivery_id: DeliveryId::try_new("delivery-1").unwrap(),
            event_id: EventId::try_new("event-1").unwrap(),
            tenant_id: TenantId::try_new("tenant-1").unwrap(),
            source_module_id: ModuleId::try_new(SOURCE_MODULE_ID).unwrap(),
            consumer_module_id: ModuleId::try_new("crm.sales-activities-link").unwrap(),
            source_actor_id: ActorId::try_new("sales-user").unwrap(),
            event_type: EventType::try_new(SOURCE_EVENT_TYPE).unwrap(),
            event_version: EventVersion::try_new(SOURCE_EVENT_VERSION).unwrap(),
            aggregate: RecordRef {
                record_type: RecordType::try_new("sales.deal").unwrap(),
                record_id: RecordId::try_new("deal-1").unwrap(),
            },
            aggregate_version: message.version,
            occurred_at_unix_nanos: 100,
            correlation_id: CorrelationId::try_new("correlation-1").unwrap(),
            trace_id: TraceId::try_new("trace-1").unwrap(),
            payload: TypedPayload {
                owner: ModuleId::try_new(SOURCE_MODULE_ID).unwrap(),
                schema_id: SchemaId::try_new(SOURCE_EVENT_SCHEMA_ID).unwrap(),
                schema_version: SchemaVersion::try_new(SOURCE_EVENT_VERSION).unwrap(),
                descriptor_hash: message_descriptor_hash(SOURCE_EVENT_SCHEMA_ID),
                data_class: DataClass::Confidential,
                encoding: PayloadEncoding::Protobuf,
                maximum_size_bytes: MAX_PROTOBUF_BYTES,
                retention_policy_id: RetentionPolicyId::try_new(DEFAULT_RETENTION_POLICY_ID)
                    .unwrap(),
                bytes,
            },
        }
    }

    #[test]
    fn decodes_open_sales_event_into_link_domain_input() {
        let adapter = ProtobufSalesActivitiesLinkContractAdapter;
        let delivery = delivery(sales::DealStageChangedEvent {
            deal_id: "deal-1".to_owned(),
            status: sales::DealStatus::Open as i32,
            version: 2,
            ..Default::default()
        });

        let decoded = adapter.decode_sales_deal_stage_changed(&delivery).unwrap();

        assert_eq!(decoded.deal_id.as_str(), "deal-1");
        assert_eq!(decoded.version, 2);
        assert_eq!(decoded.status, DealLifecycleStatus::Open);
    }

    #[test]
    fn rejects_descriptor_rebinding_before_protobuf_decode() {
        let adapter = ProtobufSalesActivitiesLinkContractAdapter;
        let mut delivery = delivery(sales::DealStageChangedEvent {
            deal_id: "deal-1".to_owned(),
            status: sales::DealStatus::Open as i32,
            version: 2,
            ..Default::default()
        });
        delivery.payload.descriptor_hash = [9; 32];

        let error = adapter
            .decode_sales_deal_stage_changed(&delivery)
            .expect_err("descriptor rebinding must fail");

        assert_eq!(error.code, "LINK_SOURCE_EVENT_CONTRACT_MISMATCH");
    }

    #[test]
    fn rejects_closed_status_without_matching_close_outcome() {
        let adapter = ProtobufSalesActivitiesLinkContractAdapter;
        let delivery = delivery(sales::DealStageChangedEvent {
            deal_id: "deal-1".to_owned(),
            status: sales::DealStatus::Won as i32,
            version: 2,
            ..Default::default()
        });

        let error = adapter
            .decode_sales_deal_stage_changed(&delivery)
            .expect_err("closed lifecycle evidence is incomplete");

        assert_eq!(error.code, "LINK_SOURCE_EVENT_SEMANTICS_INVALID");
    }

    #[test]
    fn encodes_create_task_intent_as_exact_published_contract() {
        let adapter = ProtobufSalesActivitiesLinkContractAdapter;
        let payload = adapter
            .encode_create_task(&CreateTaskIntent {
                task_id: RecordId::try_new("task-1").unwrap(),
                tenant_id: TenantId::try_new("tenant-1").unwrap(),
                subject: "Follow up deal after stage change".to_owned(),
                owner_actor_id: ActorId::try_new("sales-user").unwrap(),
                related_deal: ResourceRef {
                    resource_type: "sales.deal".to_owned(),
                    resource_id: "deal-1".to_owned(),
                    version: Some(2),
                },
            })
            .unwrap();

        assert_eq!(payload.owner.as_str(), TARGET_MODULE_ID);
        assert_eq!(payload.schema_id.as_str(), TARGET_REQUEST_SCHEMA_ID);
        assert_eq!(
            payload.descriptor_hash,
            message_descriptor_hash(TARGET_REQUEST_SCHEMA_ID)
        );

        let command = activities::CreateTaskRequest::decode(payload.bytes.as_slice()).unwrap();
        assert_eq!(command.task_id, "task-1");
        assert_eq!(command.subject, "Follow up deal after stage change");
        assert_eq!(command.priority, activities::TaskPriority::Normal as i32);
        assert_eq!(command.related_resources.len(), 1);
        let related = &command.related_resources[0];
        assert_eq!(related.tenant_id, "tenant-1");
        assert_eq!(related.resource_type, "sales.deal");
        assert_eq!(related.resource_id, "deal-1");
        assert_eq!(related.version, Some(2));
        let owner = command.owner.unwrap().owner.unwrap();
        assert_eq!(
            owner,
            core::actor_or_team_owner::Owner::ActorId("sales-user".to_owned())
        );
    }
}
