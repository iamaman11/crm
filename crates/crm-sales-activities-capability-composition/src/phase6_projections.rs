use crm_core_events::ProjectionDocumentWrite;
use crm_module_sdk::{
    DataClass, ErrorCategory, EventDelivery, EventType, ModuleId, PayloadEncoding, SdkError,
};
use crm_projection_runtime::{
    ProjectionDefinition, ProjectionHandler, ProjectionId, ProjectionRegistry,
};
use crm_proto_contracts::{crm::activities::v1 as activities, message_descriptor_hash};
use prost::Message;
use serde_json::json;
use std::sync::Arc;

pub const DEAL_TIMELINE_PROJECTION_ID: &str = "phase6.deal-timeline.v1";
pub const TASK_STATUS_PROJECTION_ID: &str = "phase6.task-status.v1";
pub const PROJECTION_CONSUMER_MODULE_ID: &str = "crm.phase6-projections";
pub const DEAL_TIMELINE_RESOURCE_TYPE: &str = "sales.deal.timeline-entry";
pub const TASK_STATUS_RESOURCE_TYPE: &str = "activities.task.status";

const SALES_MODULE_ID: &str = "crm.sales";
const ACTIVITIES_MODULE_ID: &str = "crm.activities";
const CONTRACT_VERSION: &str = "1.0.0";

const SALES_CREATED: &str = "sales.deal.created";
const SALES_UPDATED: &str = "sales.deal.updated";
const SALES_STAGE_CHANGED: &str = "sales.deal.stage_changed";
const SALES_CREATED_SCHEMA: &str = "crm.sales.v1.DealCreatedEvent";
const SALES_UPDATED_SCHEMA: &str = "crm.sales.v1.DealUpdatedEvent";
const SALES_STAGE_CHANGED_SCHEMA: &str = "crm.sales.v1.DealStageChangedEvent";

const TASK_CREATED: &str = "activities.task.created";
const TASK_UPDATED: &str = "activities.task.updated";
const TASK_COMPLETED: &str = "activities.task.completed";
const TASK_REMINDER_SCHEDULED: &str = "activities.task.reminder_scheduled";
const TASK_CREATED_SCHEMA: &str = "crm.activities.v1.TaskCreatedEvent";
const TASK_UPDATED_SCHEMA: &str = "crm.activities.v1.TaskUpdatedEvent";
const TASK_COMPLETED_SCHEMA: &str = "crm.activities.v1.TaskCompletedEvent";
const TASK_REMINDER_SCHEMA: &str = "crm.activities.v1.TaskReminderScheduledEvent";

#[derive(Debug, Clone, Copy)]
struct DealTimelineProjectionHandler;

impl ProjectionHandler for DealTimelineProjectionHandler {
    fn project(&self, delivery: &EventDelivery) -> Result<Vec<ProjectionDocumentWrite>, SdkError> {
        deal_timeline_writes(delivery)
    }
}

#[derive(Debug, Clone, Copy)]
struct TaskStatusProjectionHandler;

impl ProjectionHandler for TaskStatusProjectionHandler {
    fn project(&self, delivery: &EventDelivery) -> Result<Vec<ProjectionDocumentWrite>, SdkError> {
        task_status_writes(delivery)
    }
}

/// Registers the concrete Phase 6 projections with the generic platform runtime.
///
/// Business-contract decoding remains in this composition crate; paging,
/// checkpointing, poison handling and rebuild orchestration live in
/// `crm-projection-runtime`.
pub fn phase6_projection_registry() -> Result<ProjectionRegistry, SdkError> {
    ProjectionRegistry::new(vec![
        ProjectionDefinition::new(
            configured_projection_id(DEAL_TIMELINE_PROJECTION_ID)?,
            configured_module_id(PROJECTION_CONSUMER_MODULE_ID)?,
            configured_event_types(&[SALES_CREATED, SALES_UPDATED, SALES_STAGE_CHANGED])?,
            Arc::new(DealTimelineProjectionHandler),
        )?,
        ProjectionDefinition::new(
            configured_projection_id(TASK_STATUS_PROJECTION_ID)?,
            configured_module_id(PROJECTION_CONSUMER_MODULE_ID)?,
            configured_event_types(&[
                TASK_CREATED,
                TASK_UPDATED,
                TASK_COMPLETED,
                TASK_REMINDER_SCHEDULED,
            ])?,
            Arc::new(TaskStatusProjectionHandler),
        )?,
    ])
}

fn configured_event_types(values: &[&str]) -> Result<Vec<EventType>, SdkError> {
    values
        .iter()
        .map(|value| {
            EventType::try_new(*value)
                .map_err(|error| projection_configuration_invalid(error.to_string()))
        })
        .collect()
}

fn deal_timeline_writes(
    delivery: &EventDelivery,
) -> Result<Vec<ProjectionDocumentWrite>, SdkError> {
    if delivery.source_module_id.as_str() != SALES_MODULE_ID
        || delivery.aggregate.record_type.as_str() != "sales.deal"
    {
        return Err(projection_event_invalid(
            "Sales timeline event ownership is invalid",
        ));
    }
    let expected_schema = match delivery.event_type.as_str() {
        SALES_CREATED => SALES_CREATED_SCHEMA,
        SALES_UPDATED => SALES_UPDATED_SCHEMA,
        SALES_STAGE_CHANGED => SALES_STAGE_CHANGED_SCHEMA,
        _ => {
            return Err(projection_event_invalid(
                "Sales timeline event type is unsupported",
            ));
        }
    };
    validate_contract(delivery, SALES_MODULE_ID, expected_schema)?;

    let deal_id = delivery.aggregate.record_id.as_str();
    Ok(vec![ProjectionDocumentWrite {
        resource_type: DEAL_TIMELINE_RESOURCE_TYPE.to_owned(),
        resource_id: format!("{deal_id}:{}", delivery.event_id.as_str()),
        source_version: delivery.aggregate_version,
        document: json!({
            "event_id": delivery.event_id.as_str(),
            "deal_id": deal_id,
            "event_type": delivery.event_type.as_str(),
            "aggregate_version": delivery.aggregate_version,
            "occurred_at_unix_nanos": delivery.occurred_at_unix_nanos,
            "schema_id": delivery.payload.schema_id.as_str(),
            "schema_version": delivery.payload.schema_version.as_str(),
        }),
    }])
}

fn task_status_writes(delivery: &EventDelivery) -> Result<Vec<ProjectionDocumentWrite>, SdkError> {
    if delivery.source_module_id.as_str() != ACTIVITIES_MODULE_ID
        || delivery.aggregate.record_type.as_str() != "activities.task"
    {
        return Err(projection_event_invalid(
            "Task status event ownership is invalid",
        ));
    }
    let task_id = delivery.aggregate.record_id.as_str();
    let status = match delivery.event_type.as_str() {
        TASK_CREATED => {
            validate_contract(delivery, ACTIVITIES_MODULE_ID, TASK_CREATED_SCHEMA)?;
            let event = decode::<activities::TaskCreatedEvent>(delivery)?;
            let task = event
                .task
                .ok_or_else(|| projection_event_invalid("Task created event is missing task"))?;
            validate_task_snapshot(task_id, delivery.aggregate_version, &task)?;
            task_status_name(task.status)?
        }
        TASK_UPDATED => {
            validate_contract(delivery, ACTIVITIES_MODULE_ID, TASK_UPDATED_SCHEMA)?;
            let event = decode::<activities::TaskUpdatedEvent>(delivery)?;
            let task = event
                .task
                .ok_or_else(|| projection_event_invalid("Task updated event is missing task"))?;
            validate_task_snapshot(task_id, delivery.aggregate_version, &task)?;
            task_status_name(task.status)?
        }
        TASK_COMPLETED => {
            validate_contract(delivery, ACTIVITIES_MODULE_ID, TASK_COMPLETED_SCHEMA)?;
            let event = decode::<activities::TaskCompletedEvent>(delivery)?;
            if event.task_id != task_id || event.version != delivery.aggregate_version {
                return Err(projection_event_invalid(
                    "Task completed event identity is inconsistent",
                ));
            }
            "completed"
        }
        TASK_REMINDER_SCHEDULED => {
            validate_contract(delivery, ACTIVITIES_MODULE_ID, TASK_REMINDER_SCHEMA)?;
            let event = decode::<activities::TaskReminderScheduledEvent>(delivery)?;
            if event.task_id != task_id || event.version != delivery.aggregate_version {
                return Err(projection_event_invalid(
                    "Task reminder event identity is inconsistent",
                ));
            }
            return Ok(Vec::new());
        }
        _ => {
            return Err(projection_event_invalid(
                "Task status event type is unsupported",
            ));
        }
    };

    Ok(vec![ProjectionDocumentWrite {
        resource_type: TASK_STATUS_RESOURCE_TYPE.to_owned(),
        resource_id: task_id.to_owned(),
        source_version: delivery.aggregate_version,
        document: json!({
            "event_id": delivery.event_id.as_str(),
            "task_id": task_id,
            "status": status,
            "version": delivery.aggregate_version,
            "occurred_at_unix_nanos": delivery.occurred_at_unix_nanos,
        }),
    }])
}

fn validate_task_snapshot(
    task_id: &str,
    aggregate_version: i64,
    task: &activities::Task,
) -> Result<(), SdkError> {
    if task.task_id != task_id || task.version != aggregate_version {
        return Err(projection_event_invalid(
            "Task event snapshot identity is inconsistent",
        ));
    }
    Ok(())
}

fn task_status_name(value: i32) -> Result<&'static str, SdkError> {
    match activities::TaskStatus::try_from(value).ok() {
        Some(activities::TaskStatus::Open) => Ok("open"),
        Some(activities::TaskStatus::Completed) => Ok("completed"),
        _ => Err(projection_event_invalid("Task status is invalid")),
    }
}

fn validate_contract(
    delivery: &EventDelivery,
    owner_module_id: &str,
    schema_id: &str,
) -> Result<(), SdkError> {
    if delivery.payload.owner.as_str() != owner_module_id
        || delivery.event_version.as_str() != CONTRACT_VERSION
        || delivery.payload.schema_id.as_str() != schema_id
        || delivery.payload.schema_version.as_str() != CONTRACT_VERSION
        || delivery.payload.descriptor_hash != message_descriptor_hash(schema_id)
        || delivery.payload.data_class != DataClass::Confidential
        || delivery.payload.encoding != PayloadEncoding::Protobuf
    {
        return Err(projection_event_invalid(
            "Projection event contract identity is invalid",
        ));
    }
    Ok(())
}

fn decode<M>(delivery: &EventDelivery) -> Result<M, SdkError>
where
    M: Message + Default,
{
    M::decode(delivery.payload.bytes.as_slice())
        .map_err(|error| projection_event_invalid(error.to_string()))
}

fn configured_projection_id(value: &str) -> Result<ProjectionId, SdkError> {
    ProjectionId::try_new(value)
        .map_err(|error| projection_configuration_invalid(error.to_string()))
}

fn configured_module_id(value: &str) -> Result<ModuleId, SdkError> {
    ModuleId::try_new(value).map_err(|error| projection_configuration_invalid(error.to_string()))
}

fn projection_configuration_invalid(internal: impl Into<String>) -> SdkError {
    SdkError::new(
        "PHASE6_PROJECTION_CONFIGURATION_INVALID",
        ErrorCategory::Internal,
        false,
        "The Phase 6 projection registry is misconfigured.",
    )
    .with_internal_reference(internal)
}

fn projection_event_invalid(internal: impl Into<String>) -> SdkError {
    SdkError::new(
        "PHASE6_PROJECTION_EVENT_INVALID",
        ErrorCategory::InvalidArgument,
        false,
        "The projection source event is invalid.",
    )
    .with_internal_reference(internal)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projection_coordinates_are_stable_and_non_overlapping() {
        assert_ne!(DEAL_TIMELINE_PROJECTION_ID, TASK_STATUS_PROJECTION_ID);
        let registry = phase6_projection_registry().expect("valid Phase 6 projection registry");
        assert_eq!(registry.len(), 2);
        assert_eq!(
            registry
                .get(DEAL_TIMELINE_PROJECTION_ID)
                .unwrap()
                .event_types()
                .len(),
            3
        );
        assert_eq!(
            registry
                .get(TASK_STATUS_PROJECTION_ID)
                .unwrap()
                .event_types()
                .len(),
            4
        );
    }
}
