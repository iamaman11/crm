use crate::domain::{Task, TaskOwner, TaskPriority, TaskSnapshot, TaskStatus, TeamId};
use crm_module_sdk::{ActorId, ErrorCategory, RecordId, ResourceRef, SdkError};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const TASK_STATE_SCHEMA_ID: &str = "crm.activities.task.state";
pub const TASK_STATE_SCHEMA_VERSION: &str = "1.0.0";
pub const TASK_STATE_MAXIMUM_BYTES: u64 = 256 * 1024;
pub const TASK_STATE_RETENTION_POLICY_ID: &str = "crm.activities.business_record";
const TASK_STATE_DESCRIPTOR: &[u8] = b"crm.activities.task.state/v1:task_id,subject,description,owner,related_resources,priority,status,due_at_unix_nanos,reminder_at_unix_nanos,completed_at_unix_nanos,created_at_unix_nanos,updated_at_unix_nanos,version";

pub fn task_state_descriptor_hash() -> [u8; 32] {
    Sha256::digest(TASK_STATE_DESCRIPTOR).into()
}

pub fn encode_task_state(task: &Task) -> Result<Vec<u8>, SdkError> {
    let bytes = serde_json::to_vec(&TaskStateV1::from(task.snapshot()))
        .map_err(|error| persisted_error(format!("task state serialization failed: {error}")))?;
    validate_size(&bytes)?;
    Ok(bytes)
}

pub fn decode_task_state(bytes: &[u8]) -> Result<Task, SdkError> {
    validate_size(bytes)?;
    let state: TaskStateV1 = serde_json::from_slice(bytes)
        .map_err(|error| persisted_error(format!("task state JSON is invalid: {error}")))?;
    Task::rehydrate(state.try_into()?)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TaskStateV1 {
    task_id: String,
    subject: String,
    description: Option<String>,
    owner: OwnerState,
    related_resources: Vec<ResourceState>,
    priority: PriorityState,
    status: StatusState,
    due_at_unix_nanos: Option<i64>,
    reminder_at_unix_nanos: Option<i64>,
    completed_at_unix_nanos: Option<i64>,
    created_at_unix_nanos: i64,
    updated_at_unix_nanos: i64,
    version: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum OwnerState {
    Actor { actor_id: String },
    Team { team_id: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ResourceState {
    resource_type: String,
    resource_id: String,
    version: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum PriorityState {
    Low,
    Normal,
    High,
    Urgent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum StatusState {
    Open,
    Completed,
}

impl From<TaskSnapshot> for TaskStateV1 {
    fn from(value: TaskSnapshot) -> Self {
        Self {
            task_id: value.task_id.to_string(),
            subject: value.subject,
            description: value.description,
            owner: value.owner.into(),
            related_resources: value
                .related_resources
                .into_iter()
                .map(Into::into)
                .collect(),
            priority: value.priority.into(),
            status: value.status.into(),
            due_at_unix_nanos: value.due_at_unix_nanos,
            reminder_at_unix_nanos: value.reminder_at_unix_nanos,
            completed_at_unix_nanos: value.completed_at_unix_nanos,
            created_at_unix_nanos: value.created_at_unix_nanos,
            updated_at_unix_nanos: value.updated_at_unix_nanos,
            version: value.version,
        }
    }
}

impl TryFrom<TaskStateV1> for TaskSnapshot {
    type Error = SdkError;

    fn try_from(value: TaskStateV1) -> Result<Self, Self::Error> {
        Ok(Self {
            task_id: RecordId::try_new(value.task_id).map_err(identifier_error)?,
            subject: value.subject,
            description: value.description,
            owner: value.owner.try_into()?,
            related_resources: value
                .related_resources
                .into_iter()
                .map(Into::into)
                .collect(),
            priority: value.priority.into(),
            status: value.status.into(),
            due_at_unix_nanos: value.due_at_unix_nanos,
            reminder_at_unix_nanos: value.reminder_at_unix_nanos,
            completed_at_unix_nanos: value.completed_at_unix_nanos,
            created_at_unix_nanos: value.created_at_unix_nanos,
            updated_at_unix_nanos: value.updated_at_unix_nanos,
            version: value.version,
        })
    }
}

impl From<TaskOwner> for OwnerState {
    fn from(value: TaskOwner) -> Self {
        match value {
            TaskOwner::Actor(actor_id) => Self::Actor {
                actor_id: actor_id.to_string(),
            },
            TaskOwner::Team(team_id) => Self::Team {
                team_id: team_id.as_str().to_owned(),
            },
        }
    }
}

impl TryFrom<OwnerState> for TaskOwner {
    type Error = SdkError;

    fn try_from(value: OwnerState) -> Result<Self, Self::Error> {
        match value {
            OwnerState::Actor { actor_id } => Ok(Self::Actor(
                ActorId::try_new(actor_id).map_err(identifier_error)?,
            )),
            OwnerState::Team { team_id } => Ok(Self::Team(TeamId::try_new(team_id)?)),
        }
    }
}

impl From<ResourceRef> for ResourceState {
    fn from(value: ResourceRef) -> Self {
        Self {
            resource_type: value.resource_type,
            resource_id: value.resource_id,
            version: value.version,
        }
    }
}

impl From<ResourceState> for ResourceRef {
    fn from(value: ResourceState) -> Self {
        Self {
            resource_type: value.resource_type,
            resource_id: value.resource_id,
            version: value.version,
        }
    }
}

impl From<TaskPriority> for PriorityState {
    fn from(value: TaskPriority) -> Self {
        match value {
            TaskPriority::Low => Self::Low,
            TaskPriority::Normal => Self::Normal,
            TaskPriority::High => Self::High,
            TaskPriority::Urgent => Self::Urgent,
        }
    }
}

impl From<PriorityState> for TaskPriority {
    fn from(value: PriorityState) -> Self {
        match value {
            PriorityState::Low => Self::Low,
            PriorityState::Normal => Self::Normal,
            PriorityState::High => Self::High,
            PriorityState::Urgent => Self::Urgent,
        }
    }
}

impl From<TaskStatus> for StatusState {
    fn from(value: TaskStatus) -> Self {
        match value {
            TaskStatus::Open => Self::Open,
            TaskStatus::Completed => Self::Completed,
        }
    }
}

impl From<StatusState> for TaskStatus {
    fn from(value: StatusState) -> Self {
        match value {
            StatusState::Open => Self::Open,
            StatusState::Completed => Self::Completed,
        }
    }
}

fn identifier_error(error: crm_module_sdk::IdentifierError) -> SdkError {
    persisted_error(error.to_string())
}

fn validate_size(bytes: &[u8]) -> Result<(), SdkError> {
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > TASK_STATE_MAXIMUM_BYTES {
        return Err(persisted_error(format!(
            "task state exceeds the maximum of {} bytes",
            TASK_STATE_MAXIMUM_BYTES
        )));
    }
    Ok(())
}

fn persisted_error(message: impl Into<String>) -> SdkError {
    SdkError::new(
        "ACTIVITIES_TASK_PERSISTED_STATE_INVALID",
        ErrorCategory::InvalidArgument,
        false,
        message,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{CompleteTask, CreateTask, TaskMutation};

    fn task() -> Task {
        Task::create(CreateTask {
            task_id: RecordId::try_new("task-persisted-1").unwrap(),
            subject: "Prepare implementation plan".to_owned(),
            description: Some("Include rollback and acceptance evidence.".to_owned()),
            owner: TaskOwner::Team(TeamId::try_new("team.delivery").unwrap()),
            related_resources: vec![ResourceRef {
                resource_type: "sales.deal".to_owned(),
                resource_id: "deal-1".to_owned(),
                version: Some(3),
            }],
            priority: TaskPriority::Urgent,
            due_at_unix_nanos: Some(100),
            reminder_at_unix_nanos: Some(80),
            occurred_at_unix_nanos: 10,
        })
        .unwrap()
    }

    #[test]
    fn round_trip_preserves_completed_task_and_schema_hash() {
        let mut value = task();
        assert_eq!(
            value
                .complete(CompleteTask {
                    expected_version: 1,
                    occurred_at_unix_nanos: 90,
                })
                .unwrap(),
            TaskMutation::Changed
        );

        let bytes = encode_task_state(&value).unwrap();
        let decoded = decode_task_state(&bytes).unwrap();

        assert_eq!(decoded, value);
        assert_ne!(task_state_descriptor_hash(), [0; 32]);
    }

    #[test]
    fn rejects_unknown_fields_and_inconsistent_completion_state() {
        let mut json: serde_json::Value =
            serde_json::from_slice(&encode_task_state(&task()).unwrap()).unwrap();
        json["unknown"] = serde_json::json!(true);
        assert_eq!(
            decode_task_state(&serde_json::to_vec(&json).unwrap())
                .unwrap_err()
                .code,
            "ACTIVITIES_TASK_PERSISTED_STATE_INVALID"
        );

        let mut invalid: serde_json::Value =
            serde_json::from_slice(&encode_task_state(&task()).unwrap()).unwrap();
        invalid["status"] = serde_json::json!("completed");
        assert_eq!(
            decode_task_state(&serde_json::to_vec(&invalid).unwrap())
                .unwrap_err()
                .code,
            "ACTIVITIES_COMPLETED_TASK_TIME_REQUIRED"
        );
    }
}
