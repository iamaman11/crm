#![forbid(unsafe_code)]

use crm_activities::{
    CompleteTask, CreateTask, ScheduleReminder, TASK_STATE_MAXIMUM_BYTES,
    TASK_STATE_RETENTION_POLICY_ID, TASK_STATE_SCHEMA_ID, TASK_STATE_SCHEMA_VERSION, Task,
    TaskMutation, TaskOwner, TaskPriority, TaskStatus, TeamId, UpdateTask, decode_task_state,
    encode_task_state, task_state_descriptor_hash,
};
use crm_capability_plan_support::{self as support, EventSpec, PersistedPayloadContract};
use crm_capability_runtime::{CapabilityDefinition, CapabilityRequest, CapabilityRisk};
use crm_core_contracts::Patch;
use crm_core_data::{
    AggregatePresence, AggregateTarget, BatchMutationPlan, CapabilityBatchExecutionPlan,
    EventEvidence, RecordMutation, TransactionalAggregatePlanner,
};
use crm_module_sdk::{
    ActorId, CapabilityId, CapabilityVersion, DataClass, ErrorCategory, ModuleId, RecordId,
    RecordRef, RecordSnapshot, ResourceRef, SdkError, TenantId, TypedPayload,
};
use crm_proto_contracts::crm::{activities::v1 as wire, core::v1 as core};

pub const MODULE_ID: &str = "crm.activities";
pub const RECORD_TYPE: &str = "activities.task";

pub const CREATE_CAPABILITY: &str = "activities.task.create";
pub const UPDATE_CAPABILITY: &str = "activities.task.update";
pub const COMPLETE_CAPABILITY: &str = "activities.task.complete";
pub const REMINDER_CAPABILITY: &str = "activities.task.schedule_reminder";

const CREATE_REQUEST_SCHEMA: &str = "crm.activities.v1.CreateTaskRequest";
const CREATE_RESPONSE_SCHEMA: &str = "crm.activities.v1.CreateTaskResponse";
const UPDATE_REQUEST_SCHEMA: &str = "crm.activities.v1.UpdateTaskRequest";
const UPDATE_RESPONSE_SCHEMA: &str = "crm.activities.v1.UpdateTaskResponse";
const COMPLETE_REQUEST_SCHEMA: &str = "crm.activities.v1.CompleteTaskRequest";
const COMPLETE_RESPONSE_SCHEMA: &str = "crm.activities.v1.CompleteTaskResponse";
const REMINDER_REQUEST_SCHEMA: &str = "crm.activities.v1.ScheduleReminderRequest";
const REMINDER_RESPONSE_SCHEMA: &str = "crm.activities.v1.ScheduleReminderResponse";
const CREATED_EVENT_SCHEMA: &str = "crm.activities.v1.TaskCreatedEvent";
const UPDATED_EVENT_SCHEMA: &str = "crm.activities.v1.TaskUpdatedEvent";
const COMPLETED_EVENT_SCHEMA: &str = "crm.activities.v1.TaskCompletedEvent";
const REMINDER_EVENT_SCHEMA: &str = "crm.activities.v1.TaskReminderScheduledEvent";

#[derive(Debug, Default, Clone, Copy)]
pub struct ActivitiesTaskCapabilityPlanner;

pub fn capability_definition(capability_id: &str) -> Result<CapabilityDefinition, SdkError> {
    let (input_schema, output_schema) = match capability_id {
        CREATE_CAPABILITY => (CREATE_REQUEST_SCHEMA, CREATE_RESPONSE_SCHEMA),
        UPDATE_CAPABILITY => (UPDATE_REQUEST_SCHEMA, UPDATE_RESPONSE_SCHEMA),
        COMPLETE_CAPABILITY => (COMPLETE_REQUEST_SCHEMA, COMPLETE_RESPONSE_SCHEMA),
        REMINDER_CAPABILITY => (REMINDER_REQUEST_SCHEMA, REMINDER_RESPONSE_SCHEMA),
        _ => return Err(unsupported_capability()),
    };

    Ok(CapabilityDefinition {
        capability_id: support::configured_identifier(CapabilityId::try_new(capability_id))?,
        capability_version: support::configured_identifier(CapabilityVersion::try_new(
            support::CONTRACT_VERSION,
        ))?,
        owner_module_id: support::configured_identifier(ModuleId::try_new(MODULE_ID))?,
        input_contract: support::protobuf_contract(
            MODULE_ID,
            input_schema,
            vec![DataClass::Confidential],
        )?,
        output_contract: Some(support::protobuf_contract(
            MODULE_ID,
            output_schema,
            vec![DataClass::Confidential],
        )?),
        risk: CapabilityRisk::Low,
        mutation: true,
        requires_idempotency: true,
        requires_approval: false,
        authorization_policy_id: capability_id.to_owned(),
        rate_limit_policy_id: None,
    })
}

impl TransactionalAggregatePlanner for ActivitiesTaskCapabilityPlanner {
    fn target(
        &self,
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
    ) -> Result<AggregateTarget, SdkError> {
        ensure_definition(definition, request)?;
        let (task_id, presence) = match definition.capability_id.as_str() {
            CREATE_CAPABILITY => {
                let command: wire::CreateTaskRequest =
                    support::decode_request(request, MODULE_ID, CREATE_REQUEST_SCHEMA)?;
                (command.task_id, AggregatePresence::MustBeAbsent)
            }
            UPDATE_CAPABILITY => {
                let command: wire::UpdateTaskRequest =
                    support::decode_request(request, MODULE_ID, UPDATE_REQUEST_SCHEMA)?;
                (command.task_id, AggregatePresence::MustExist)
            }
            COMPLETE_CAPABILITY => {
                let command: wire::CompleteTaskRequest =
                    support::decode_request(request, MODULE_ID, COMPLETE_REQUEST_SCHEMA)?;
                (command.task_id, AggregatePresence::MustExist)
            }
            REMINDER_CAPABILITY => {
                let command: wire::ScheduleReminderRequest =
                    support::decode_request(request, MODULE_ID, REMINDER_REQUEST_SCHEMA)?;
                (command.task_id, AggregatePresence::MustExist)
            }
            _ => return Err(unsupported_capability()),
        };

        Ok(AggregateTarget {
            reference: support::record_ref(RECORD_TYPE, &task_id, "task.task_id")?,
            presence,
        })
    }

    fn plan(
        &self,
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
        current: Option<&RecordSnapshot>,
    ) -> Result<CapabilityBatchExecutionPlan, SdkError> {
        ensure_definition(definition, request)?;
        match definition.capability_id.as_str() {
            CREATE_CAPABILITY => plan_create(definition, request, current),
            UPDATE_CAPABILITY => plan_update(definition, request, current),
            COMPLETE_CAPABILITY => plan_complete(definition, request, current),
            REMINDER_CAPABILITY => plan_reminder(definition, request, current),
            _ => Err(unsupported_capability()),
        }
    }
}

fn plan_create(
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
    current: Option<&RecordSnapshot>,
) -> Result<CapabilityBatchExecutionPlan, SdkError> {
    if current.is_some() {
        return Err(invalid_plan());
    }
    let command: wire::CreateTaskRequest =
        support::decode_request(request, MODULE_ID, CREATE_REQUEST_SCHEMA)?;
    let tenant = &request.context.execution.tenant_id;
    let task = Task::create(CreateTask {
        task_id: support::input_identifier(RecordId::try_new(command.task_id), "task.task_id")?,
        subject: command.subject,
        description: command.description,
        owner: owner_from_wire(required(command.owner, "task.owner")?, "task.owner")?,
        related_resources: command
            .related_resources
            .into_iter()
            .map(|value| support::wire_resource_to_domain(value, tenant, "task.related_resources"))
            .collect::<Result<Vec<_>, _>>()?,
        priority: priority_from_wire(command.priority, "task.priority")?,
        due_at_unix_nanos: optional_time(command.due_at, "task.due_at")?,
        reminder_at_unix_nanos: optional_time(command.reminder_at, "task.reminder_at")?,
        occurred_at_unix_nanos: request.context.execution.request_started_at_unix_nanos,
    })?;

    let aggregate = support::record_ref(RECORD_TYPE, task.task_id().as_str(), "task.task_id")?;
    let public_task = task_to_wire(&task, tenant);
    let output = support::protobuf_payload(
        MODULE_ID,
        CREATE_RESPONSE_SCHEMA,
        DataClass::Confidential,
        &wire::CreateTaskResponse {
            task: Some(public_task.clone()),
        },
    )?;
    let event = support::event_evidence(
        request,
        aggregate.clone(),
        MODULE_ID,
        EventSpec {
            event_type: "activities.task.created",
            event_schema_id: CREATED_EVENT_SCHEMA,
            aggregate_version: task.version(),
            previous_version: None,
        },
        &wire::TaskCreatedEvent {
            task: Some(public_task),
        },
    )?;

    execution_plan(
        definition,
        request,
        aggregate.clone(),
        Some(RecordMutation::Create {
            reference: aggregate,
            payload: persisted_payload(&task)?,
        }),
        Some(event),
        task.version(),
        output,
    )
}

fn plan_update(
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
    current: Option<&RecordSnapshot>,
) -> Result<CapabilityBatchExecutionPlan, SdkError> {
    let current = current.ok_or_else(invalid_plan)?;
    let command: wire::UpdateTaskRequest =
        support::decode_request(request, MODULE_ID, UPDATE_REQUEST_SCHEMA)?;
    ensure_target(&command.task_id, current)?;
    let tenant = &request.context.execution.tenant_id;
    let mut task = task_from_snapshot(current)?;
    let before = task.clone();
    task.apply_update(UpdateTask {
        expected_version: command.expected_version,
        subject: string_patch(command.subject, "task.subject")?,
        description: string_patch(command.description, "task.description")?,
        owner: owner_patch(command.owner, "task.owner")?,
        priority: priority_patch(command.priority, "task.priority")?,
        due_at_unix_nanos: time_patch(command.due_at, "task.due_at")?,
        occurred_at_unix_nanos: request.context.execution.request_started_at_unix_nanos,
    })?;

    let aggregate = current.reference.clone();
    let public_task = task_to_wire(&task, tenant);
    let output = support::protobuf_payload(
        MODULE_ID,
        UPDATE_RESPONSE_SCHEMA,
        DataClass::Confidential,
        &wire::UpdateTaskResponse {
            task: Some(public_task.clone()),
        },
    )?;
    let event = support::event_evidence(
        request,
        aggregate.clone(),
        MODULE_ID,
        EventSpec {
            event_type: "activities.task.updated",
            event_schema_id: UPDATED_EVENT_SCHEMA,
            aggregate_version: task.version(),
            previous_version: Some(current.version),
        },
        &wire::TaskUpdatedEvent {
            task: Some(public_task),
            changed_fields: changed_fields(&before, &task),
        },
    )?;

    execution_plan(
        definition,
        request,
        aggregate.clone(),
        Some(RecordMutation::Update {
            reference: aggregate,
            expected_version: current.version,
            payload: persisted_payload(&task)?,
        }),
        Some(event),
        task.version(),
        output,
    )
}

fn plan_complete(
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
    current: Option<&RecordSnapshot>,
) -> Result<CapabilityBatchExecutionPlan, SdkError> {
    let current = current.ok_or_else(invalid_plan)?;
    let command: wire::CompleteTaskRequest =
        support::decode_request(request, MODULE_ID, COMPLETE_REQUEST_SCHEMA)?;
    ensure_target(&command.task_id, current)?;
    let tenant = &request.context.execution.tenant_id;
    let mut task = task_from_snapshot(current)?;
    let mutation = task.complete(CompleteTask {
        expected_version: command.expected_version,
        occurred_at_unix_nanos: request.context.execution.request_started_at_unix_nanos,
    })?;
    let changed = mutation == TaskMutation::Changed;
    let aggregate = current.reference.clone();
    let public_task = task_to_wire(&task, tenant);
    let output = support::protobuf_payload(
        MODULE_ID,
        COMPLETE_RESPONSE_SCHEMA,
        DataClass::Confidential,
        &wire::CompleteTaskResponse {
            task: Some(public_task),
            changed,
        },
    )?;

    let (record, event) = if changed {
        let event = support::event_evidence(
            request,
            aggregate.clone(),
            MODULE_ID,
            EventSpec {
                event_type: "activities.task.completed",
                event_schema_id: COMPLETED_EVENT_SCHEMA,
                aggregate_version: task.version(),
                previous_version: Some(current.version),
            },
            &wire::TaskCompletedEvent {
                task_id: task.task_id().as_str().to_owned(),
                completed_at: task
                    .completed_at_unix_nanos()
                    .map(support::nanos_to_wire_time),
                version: task.version(),
            },
        )?;
        (
            Some(RecordMutation::Update {
                reference: aggregate.clone(),
                expected_version: current.version,
                payload: persisted_payload(&task)?,
            }),
            Some(event),
        )
    } else {
        (None, None)
    };

    execution_plan(
        definition,
        request,
        aggregate,
        record,
        event,
        task.version(),
        output,
    )
}

fn plan_reminder(
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
    current: Option<&RecordSnapshot>,
) -> Result<CapabilityBatchExecutionPlan, SdkError> {
    let current = current.ok_or_else(invalid_plan)?;
    let command: wire::ScheduleReminderRequest =
        support::decode_request(request, MODULE_ID, REMINDER_REQUEST_SCHEMA)?;
    ensure_target(&command.task_id, current)?;
    let tenant = &request.context.execution.tenant_id;
    let reminder_at_unix_nanos = support::wire_time_to_nanos(
        required(command.reminder_at, "task.reminder_at")?,
        "task.reminder_at",
    )?;
    let mut task = task_from_snapshot(current)?;
    let mutation = task.schedule_reminder(ScheduleReminder {
        expected_version: command.expected_version,
        reminder_at_unix_nanos,
        occurred_at_unix_nanos: request.context.execution.request_started_at_unix_nanos,
    })?;
    let changed = mutation == TaskMutation::Changed;
    let aggregate = current.reference.clone();
    let public_task = task_to_wire(&task, tenant);
    let output = support::protobuf_payload(
        MODULE_ID,
        REMINDER_RESPONSE_SCHEMA,
        DataClass::Confidential,
        &wire::ScheduleReminderResponse {
            task: Some(public_task),
            changed,
        },
    )?;

    let (record, event) = if changed {
        let event = support::event_evidence(
            request,
            aggregate.clone(),
            MODULE_ID,
            EventSpec {
                event_type: "activities.task.reminder_scheduled",
                event_schema_id: REMINDER_EVENT_SCHEMA,
                aggregate_version: task.version(),
                previous_version: Some(current.version),
            },
            &wire::TaskReminderScheduledEvent {
                task_id: task.task_id().as_str().to_owned(),
                reminder_at: task
                    .reminder_at_unix_nanos()
                    .map(support::nanos_to_wire_time),
                version: task.version(),
            },
        )?;
        (
            Some(RecordMutation::Update {
                reference: aggregate.clone(),
                expected_version: current.version,
                payload: persisted_payload(&task)?,
            }),
            Some(event),
        )
    } else {
        (None, None)
    };

    execution_plan(
        definition,
        request,
        aggregate,
        record,
        event,
        task.version(),
        output,
    )
}

fn execution_plan(
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
    aggregate: RecordRef,
    record: Option<RecordMutation>,
    event: Option<EventEvidence>,
    aggregate_version: i64,
    output: TypedPayload,
) -> Result<CapabilityBatchExecutionPlan, SdkError> {
    let audit = support::audit_intent(
        request,
        &aggregate,
        aggregate_version,
        definition.capability_id.as_str(),
        &output.bytes,
    )?;
    Ok(CapabilityBatchExecutionPlan {
        batch: BatchMutationPlan {
            context: request.context.clone(),
            records: record.into_iter().collect(),
            relationships: Vec::new(),
            events: event.into_iter().collect(),
            idempotency: support::capability_idempotency(definition, request)?,
            audits: vec![audit],
        },
        output: Some(output),
    })
}

fn persisted_contract() -> PersistedPayloadContract<'static> {
    PersistedPayloadContract {
        owner: MODULE_ID,
        schema_id: TASK_STATE_SCHEMA_ID,
        schema_version: TASK_STATE_SCHEMA_VERSION,
        descriptor_hash: task_state_descriptor_hash(),
        maximum_size_bytes: TASK_STATE_MAXIMUM_BYTES,
        retention_policy_id: TASK_STATE_RETENTION_POLICY_ID,
    }
}

fn persisted_payload(task: &Task) -> Result<TypedPayload, SdkError> {
    support::persisted_json_payload(persisted_contract(), encode_task_state(task)?)
}

fn task_from_snapshot(snapshot: &RecordSnapshot) -> Result<Task, SdkError> {
    let task = decode_task_state(support::persisted_json_bytes(
        snapshot,
        persisted_contract(),
    )?)?;
    if task.task_id() != &snapshot.reference.record_id || task.version() != snapshot.version {
        return Err(support::stored_data_error(
            "ACTIVITIES_PERSISTED_TASK_IDENTITY_INVALID",
        ));
    }
    Ok(task)
}

fn task_to_wire(task: &Task, tenant: &TenantId) -> wire::Task {
    wire::Task {
        task_id: task.task_id().as_str().to_owned(),
        tenant_id: tenant.as_str().to_owned(),
        subject: task.subject().to_owned(),
        description: task.description().map(str::to_owned),
        owner: Some(owner_to_wire(task.owner())),
        related_resources: task
            .related_resources()
            .iter()
            .map(|value| support::domain_resource_to_wire(value, tenant))
            .collect(),
        priority: priority_to_wire(task.priority()),
        status: status_to_wire(task.status()),
        due_at: task.due_at_unix_nanos().map(support::nanos_to_wire_time),
        reminder_at: task
            .reminder_at_unix_nanos()
            .map(support::nanos_to_wire_time),
        completed_at: task
            .completed_at_unix_nanos()
            .map(support::nanos_to_wire_time),
        created_at: Some(support::nanos_to_wire_time(task.created_at_unix_nanos())),
        updated_at: Some(support::nanos_to_wire_time(task.updated_at_unix_nanos())),
        version: task.version(),
    }
}

fn owner_from_wire(
    value: core::ActorOrTeamOwner,
    field: &'static str,
) -> Result<TaskOwner, SdkError> {
    use core::actor_or_team_owner::Owner;
    match value.owner {
        Some(Owner::ActorId(value)) => Ok(TaskOwner::Actor(support::input_identifier(
            ActorId::try_new(value),
            field,
        )?)),
        Some(Owner::TeamId(value)) => Ok(TaskOwner::Team(TeamId::try_new(value)?)),
        None => Err(SdkError::invalid_argument(field, "owner is required")),
    }
}

fn owner_to_wire(value: &TaskOwner) -> core::ActorOrTeamOwner {
    use core::actor_or_team_owner::Owner;
    core::ActorOrTeamOwner {
        owner: Some(match value {
            TaskOwner::Actor(actor) => Owner::ActorId(actor.as_str().to_owned()),
            TaskOwner::Team(team) => Owner::TeamId(team.as_str().to_owned()),
        }),
    }
}

fn priority_from_wire(value: i32, field: &'static str) -> Result<TaskPriority, SdkError> {
    match wire::TaskPriority::try_from(value).ok() {
        Some(wire::TaskPriority::Low) => Ok(TaskPriority::Low),
        Some(wire::TaskPriority::Normal) => Ok(TaskPriority::Normal),
        Some(wire::TaskPriority::High) => Ok(TaskPriority::High),
        Some(wire::TaskPriority::Urgent) => Ok(TaskPriority::Urgent),
        _ => Err(SdkError::invalid_argument(
            field,
            "task priority is required",
        )),
    }
}

fn priority_to_wire(value: TaskPriority) -> i32 {
    match value {
        TaskPriority::Low => wire::TaskPriority::Low as i32,
        TaskPriority::Normal => wire::TaskPriority::Normal as i32,
        TaskPriority::High => wire::TaskPriority::High as i32,
        TaskPriority::Urgent => wire::TaskPriority::Urgent as i32,
    }
}

fn status_to_wire(value: TaskStatus) -> i32 {
    match value {
        TaskStatus::Open => wire::TaskStatus::Open as i32,
        TaskStatus::Completed => wire::TaskStatus::Completed as i32,
    }
}

fn optional_time(
    value: Option<core::UnixTime>,
    field: &'static str,
) -> Result<Option<i64>, SdkError> {
    value
        .map(|value| support::wire_time_to_nanos(value, field))
        .transpose()
}

fn string_patch(
    value: Option<core::StringPatch>,
    field: &'static str,
) -> Result<Patch<String>, SdkError> {
    use core::string_patch::Operation;
    let Some(patch) = value else {
        return Ok(Patch::Keep);
    };
    match patch.operation {
        Some(Operation::Set(value)) => Ok(Patch::Set(value)),
        Some(Operation::Clear(true)) => Ok(Patch::Clear),
        Some(Operation::Clear(false)) => {
            Err(SdkError::invalid_argument(field, "clear must be true"))
        }
        None => Err(SdkError::invalid_argument(
            field,
            "a present patch must select exactly one operation",
        )),
    }
}

fn owner_patch(
    value: Option<core::ActorOrTeamOwnerPatch>,
    field: &'static str,
) -> Result<Patch<TaskOwner>, SdkError> {
    use core::actor_or_team_owner_patch::Operation;
    let Some(patch) = value else {
        return Ok(Patch::Keep);
    };
    match patch.operation {
        Some(Operation::Set(value)) => Ok(Patch::Set(owner_from_wire(value, field)?)),
        Some(Operation::Clear(true)) => Ok(Patch::Clear),
        Some(Operation::Clear(false)) => {
            Err(SdkError::invalid_argument(field, "clear must be true"))
        }
        None => Err(SdkError::invalid_argument(
            field,
            "a present patch must select exactly one operation",
        )),
    }
}

fn priority_patch(
    value: Option<wire::TaskPriorityPatch>,
    field: &'static str,
) -> Result<Patch<TaskPriority>, SdkError> {
    use wire::task_priority_patch::Operation;
    let Some(patch) = value else {
        return Ok(Patch::Keep);
    };
    match patch.operation {
        Some(Operation::Set(value)) => Ok(Patch::Set(priority_from_wire(value, field)?)),
        Some(Operation::Clear(true)) => Ok(Patch::Clear),
        Some(Operation::Clear(false)) => {
            Err(SdkError::invalid_argument(field, "clear must be true"))
        }
        None => Err(SdkError::invalid_argument(
            field,
            "a present patch must select exactly one operation",
        )),
    }
}

fn time_patch(
    value: Option<core::UnixTimePatch>,
    field: &'static str,
) -> Result<Patch<i64>, SdkError> {
    use core::unix_time_patch::Operation;
    let Some(patch) = value else {
        return Ok(Patch::Keep);
    };
    match patch.operation {
        Some(Operation::Set(value)) => Ok(Patch::Set(support::wire_time_to_nanos(value, field)?)),
        Some(Operation::Clear(true)) => Ok(Patch::Clear),
        Some(Operation::Clear(false)) => {
            Err(SdkError::invalid_argument(field, "clear must be true"))
        }
        None => Err(SdkError::invalid_argument(
            field,
            "a present patch must select exactly one operation",
        )),
    }
}

fn changed_fields(before: &Task, after: &Task) -> Vec<String> {
    let mut fields = Vec::new();
    if before.subject() != after.subject() {
        fields.push("subject".to_owned());
    }
    if before.description() != after.description() {
        fields.push("description".to_owned());
    }
    if before.owner() != after.owner() {
        fields.push("owner".to_owned());
    }
    if before.priority() != after.priority() {
        fields.push("priority".to_owned());
    }
    if before.due_at_unix_nanos() != after.due_at_unix_nanos() {
        fields.push("due_at".to_owned());
    }
    fields
}

fn ensure_target(task_id: &str, current: &RecordSnapshot) -> Result<(), SdkError> {
    if task_id != current.reference.record_id.as_str() {
        return Err(invalid_plan());
    }
    Ok(())
}

fn required<T>(value: Option<T>, field: &'static str) -> Result<T, SdkError> {
    value.ok_or_else(|| SdkError::invalid_argument(field, "field is required"))
}

fn ensure_definition(
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
) -> Result<(), SdkError> {
    if definition.owner_module_id.as_str() != MODULE_ID
        || request.context.module_id.as_str() != MODULE_ID
        || definition.capability_version.as_str() != support::CONTRACT_VERSION
        || request.context.execution.capability_id != definition.capability_id
        || request.context.execution.capability_version != definition.capability_version
    {
        return Err(invalid_plan());
    }
    Ok(())
}

fn unsupported_capability() -> SdkError {
    SdkError::new(
        "ACTIVITIES_CAPABILITY_UNSUPPORTED",
        ErrorCategory::Internal,
        false,
        "The Activities capability is not configured.",
    )
}

fn invalid_plan() -> SdkError {
    SdkError::new(
        "ACTIVITIES_MUTATION_PLAN_INVALID",
        ErrorCategory::Internal,
        false,
        "The Activities mutation could not be planned safely.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crm_module_sdk::{
        BusinessTransactionId, CausationId, CorrelationId, ExecutionContext, IdempotencyKey,
        ModuleExecutionContext, PayloadEncoding, RequestId, RetentionPolicyId, SchemaId,
        SchemaVersion, TraceId,
    };
    use prost::Message;

    #[test]
    fn create_plan_is_deterministic_and_uses_internal_persisted_schema() {
        let definition = capability_definition(CREATE_CAPABILITY).unwrap();
        let request = create_request("tenant-a");
        let planner = ActivitiesTaskCapabilityPlanner;
        let first = planner.plan(&definition, &request, None).unwrap();
        let second = planner.plan(&definition, &request, None).unwrap();
        assert_eq!(first, second);
        let RecordMutation::Create { payload, .. } = &first.batch.records[0] else {
            panic!("expected create mutation");
        };
        assert_eq!(payload.schema_id.as_str(), TASK_STATE_SCHEMA_ID);
        assert_eq!(payload.encoding, PayloadEncoding::Json);
        assert_eq!(decode_task_state(&payload.bytes).unwrap().version(), 1);
        assert_eq!(first.batch.events.len(), 1);
        assert_eq!(first.batch.audits.len(), 1);
    }

    #[test]
    fn cross_tenant_related_resource_is_rejected() {
        let definition = capability_definition(CREATE_CAPABILITY).unwrap();
        let request = create_request("tenant-b");
        let error = ActivitiesTaskCapabilityPlanner
            .plan(&definition, &request, None)
            .unwrap_err();
        assert_eq!(error.category, ErrorCategory::InvalidArgument);
    }

    #[test]
    fn completing_an_already_completed_task_is_an_audited_noop() {
        let definition = capability_definition(COMPLETE_CAPABILITY).unwrap();
        let request = complete_request();
        let snapshot = completed_snapshot();
        let plan = ActivitiesTaskCapabilityPlanner
            .plan(&definition, &request, Some(&snapshot))
            .unwrap();
        assert!(plan.batch.records.is_empty());
        assert!(plan.batch.events.is_empty());
        assert_eq!(plan.batch.audits.len(), 1);
        let output =
            wire::CompleteTaskResponse::decode(plan.output.as_ref().unwrap().bytes.as_slice())
                .unwrap();
        assert!(!output.changed);
        assert_eq!(output.task.unwrap().version, snapshot.version);
    }

    fn create_request(resource_tenant: &str) -> CapabilityRequest {
        let command = wire::CreateTaskRequest {
            task_id: "task-1".to_owned(),
            subject: "Prepare renewal".to_owned(),
            description: Some("Confirm stakeholders".to_owned()),
            owner: Some(owner_wire()),
            related_resources: vec![core::ResourceRef {
                tenant_id: resource_tenant.to_owned(),
                resource_type: "sales.deal".to_owned(),
                resource_id: "deal-1".to_owned(),
                version: Some(1),
            }],
            priority: wire::TaskPriority::High as i32,
            due_at: Some(support::nanos_to_wire_time(300)),
            reminder_at: Some(support::nanos_to_wire_time(200)),
        };
        request(
            CREATE_CAPABILITY,
            CREATE_REQUEST_SCHEMA,
            command.encode_to_vec(),
            100,
        )
    }

    fn complete_request() -> CapabilityRequest {
        let command = wire::CompleteTaskRequest {
            task_id: "task-completed".to_owned(),
            expected_version: 2,
        };
        request(
            COMPLETE_CAPABILITY,
            COMPLETE_REQUEST_SCHEMA,
            command.encode_to_vec(),
            300,
        )
    }

    fn completed_snapshot() -> RecordSnapshot {
        let mut task = Task::create(CreateTask {
            task_id: RecordId::try_new("task-completed").unwrap(),
            subject: "Completed task".to_owned(),
            description: None,
            owner: TaskOwner::Actor(ActorId::try_new("actor-a").unwrap()),
            related_resources: Vec::new(),
            priority: TaskPriority::Normal,
            due_at_unix_nanos: None,
            reminder_at_unix_nanos: None,
            occurred_at_unix_nanos: 100,
        })
        .unwrap();
        assert_eq!(
            task.complete(CompleteTask {
                expected_version: 1,
                occurred_at_unix_nanos: 200,
            })
            .unwrap(),
            TaskMutation::Changed
        );
        RecordSnapshot {
            reference: support::record_ref(RECORD_TYPE, "task-completed", "task.task_id").unwrap(),
            version: task.version(),
            payload: persisted_payload(&task).unwrap(),
        }
    }

    fn owner_wire() -> core::ActorOrTeamOwner {
        core::ActorOrTeamOwner {
            owner: Some(core::actor_or_team_owner::Owner::ActorId(
                "actor-a".to_owned(),
            )),
        }
    }

    fn request(
        capability_id: &str,
        schema_id: &str,
        bytes: Vec<u8>,
        started_at: i64,
    ) -> CapabilityRequest {
        let definition = capability_definition(capability_id).unwrap();
        CapabilityRequest {
            context: crm_module_sdk::ModuleExecutionContext {
                module_id: ModuleId::try_new(MODULE_ID).unwrap(),
                execution: crm_module_sdk::ExecutionContext {
                    tenant_id: TenantId::try_new("tenant-a").unwrap(),
                    actor_id: ActorId::try_new("actor-a").unwrap(),
                    request_id: RequestId::try_new(format!("request-{capability_id}")).unwrap(),
                    correlation_id: CorrelationId::try_new("correlation-a").unwrap(),
                    causation_id: CausationId::try_new("causation-a").unwrap(),
                    trace_id: TraceId::try_new("trace-a").unwrap(),
                    capability_id: definition.capability_id.clone(),
                    capability_version: definition.capability_version.clone(),
                    idempotency_key: IdempotencyKey::try_new(format!("idem-{capability_id}"))
                        .unwrap(),
                    business_transaction_id: BusinessTransactionId::try_new(format!(
                        "tx-{capability_id}"
                    ))
                    .unwrap(),
                    schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
                    request_started_at_unix_nanos: started_at,
                },
            },
            input: TypedPayload {
                owner: ModuleId::try_new(MODULE_ID).unwrap(),
                schema_id: SchemaId::try_new(schema_id).unwrap(),
                schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
                descriptor_hash: support::message_descriptor_hash(schema_id),
                data_class: DataClass::Confidential,
                encoding: PayloadEncoding::Protobuf,
                maximum_size_bytes: support::MAX_PROTOBUF_BYTES,
                retention_policy_id: RetentionPolicyId::try_new("standard").unwrap(),
                bytes,
            },
            input_hash: [9; 32],
            approval: None,
        }
    }
}
