use crm_core_contracts::{PageRequest, Patch};
use crm_module_sdk::{
    ActorId, ErrorCategory, FieldName, FieldViolation, RecordId, ResourceRef, SdkError,
};
use std::collections::BTreeSet;

const MAX_SUBJECT_BYTES: usize = 240;
const MAX_DESCRIPTION_BYTES: usize = 16_384;
const MAX_IDENTIFIER_BYTES: usize = 180;
const MAX_RELATED_RESOURCES: usize = 50;

macro_rules! domain_identifier {
    ($name:ident, $field:literal) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(String);

        impl $name {
            pub fn try_new(value: impl Into<String>) -> Result<Self, SdkError> {
                let value = value.into();
                validate_identifier($field, &value)?;
                Ok(Self(value))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
    };
}

domain_identifier!(TeamId, "task.owner.team_id");

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskOwner {
    Actor(ActorId),
    Team(TeamId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskPriority {
    Low,
    Normal,
    High,
    Urgent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskStatus {
    Open,
    Completed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Task {
    pub task_id: RecordId,
    pub subject: String,
    pub description: Option<String>,
    pub owner: TaskOwner,
    pub related_resources: Vec<ResourceRef>,
    pub priority: TaskPriority,
    pub status: TaskStatus,
    pub due_at_unix_nanos: Option<i64>,
    pub reminder_at_unix_nanos: Option<i64>,
    pub completed_at_unix_nanos: Option<i64>,
    pub created_at_unix_nanos: i64,
    pub updated_at_unix_nanos: i64,
    pub version: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateTask {
    pub task_id: RecordId,
    pub subject: String,
    pub description: Option<String>,
    pub owner: TaskOwner,
    pub related_resources: Vec<ResourceRef>,
    pub priority: TaskPriority,
    pub due_at_unix_nanos: Option<i64>,
    pub reminder_at_unix_nanos: Option<i64>,
    pub occurred_at_unix_nanos: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateTask {
    pub expected_version: i64,
    pub subject: Patch<String>,
    pub description: Patch<String>,
    pub owner: Patch<TaskOwner>,
    pub priority: Patch<TaskPriority>,
    pub due_at_unix_nanos: Patch<i64>,
    pub occurred_at_unix_nanos: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompleteTask {
    pub expected_version: i64,
    pub occurred_at_unix_nanos: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScheduleReminder {
    pub expected_version: i64,
    pub reminder_at_unix_nanos: i64,
    pub occurred_at_unix_nanos: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskMutation {
    Changed,
    Unchanged,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskSort {
    DueAtAscending,
    UpdatedAtDescending,
    PriorityDescending,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskListQuery {
    pub page: PageRequest,
    pub owner: Option<TaskOwner>,
    pub status: Option<TaskStatus>,
    pub related_resource: Option<ResourceRef>,
    pub sort: TaskSort,
}

impl Task {
    pub fn create(command: CreateTask) -> Result<Self, SdkError> {
        validate_subject(&command.subject)?;
        validate_optional_description(command.description.as_deref())?;
        validate_related_resources(&command.related_resources)?;
        validate_timestamp(
            "task.occurred_at_unix_nanos",
            command.occurred_at_unix_nanos,
        )?;
        validate_optional_timestamp("task.due_at_unix_nanos", command.due_at_unix_nanos)?;
        validate_optional_timestamp(
            "task.reminder_at_unix_nanos",
            command.reminder_at_unix_nanos,
        )?;
        validate_reminder_window(
            command.reminder_at_unix_nanos,
            command.due_at_unix_nanos,
            command.occurred_at_unix_nanos,
        )?;

        Ok(Self {
            task_id: command.task_id,
            subject: command.subject,
            description: command.description,
            owner: command.owner,
            related_resources: command.related_resources,
            priority: command.priority,
            status: TaskStatus::Open,
            due_at_unix_nanos: command.due_at_unix_nanos,
            reminder_at_unix_nanos: command.reminder_at_unix_nanos,
            completed_at_unix_nanos: None,
            created_at_unix_nanos: command.occurred_at_unix_nanos,
            updated_at_unix_nanos: command.occurred_at_unix_nanos,
            version: 1,
        })
    }

    pub fn apply_update(&mut self, command: UpdateTask) -> Result<(), SdkError> {
        self.require_version(command.expected_version)?;
        self.require_monotonic_time(command.occurred_at_unix_nanos)?;

        match command.subject {
            Patch::Keep => {}
            Patch::Set(subject) => {
                validate_subject(&subject)?;
                self.subject = subject;
            }
            Patch::Clear => {
                return Err(invalid(
                    "ACTIVITIES_TASK_SUBJECT_REQUIRED",
                    "task.subject",
                    "task subject cannot be cleared",
                ));
            }
        }

        match command.description {
            Patch::Keep => {}
            Patch::Set(description) => {
                validate_optional_description(Some(&description))?;
                self.description = Some(description);
            }
            Patch::Clear => self.description = None,
        }

        match command.owner {
            Patch::Keep => {}
            Patch::Set(owner) => self.owner = owner,
            Patch::Clear => {
                return Err(invalid(
                    "ACTIVITIES_TASK_OWNER_REQUIRED",
                    "task.owner",
                    "task owner cannot be cleared",
                ));
            }
        }

        match command.priority {
            Patch::Keep => {}
            Patch::Set(priority) => self.priority = priority,
            Patch::Clear => {
                return Err(invalid(
                    "ACTIVITIES_TASK_PRIORITY_REQUIRED",
                    "task.priority",
                    "task priority cannot be cleared",
                ));
            }
        }

        match command.due_at_unix_nanos {
            Patch::Keep => {}
            Patch::Set(due_at) => {
                validate_timestamp("task.due_at_unix_nanos", due_at)?;
                if self
                    .reminder_at_unix_nanos
                    .is_some_and(|reminder_at| reminder_at > due_at)
                {
                    return Err(invalid(
                        "ACTIVITIES_REMINDER_AFTER_DUE",
                        "task.due_at_unix_nanos",
                        "due time cannot precede the existing reminder",
                    ));
                }
                self.due_at_unix_nanos = Some(due_at);
            }
            Patch::Clear => self.due_at_unix_nanos = None,
        }

        self.updated_at_unix_nanos = command.occurred_at_unix_nanos;
        self.version += 1;
        Ok(())
    }

    pub fn complete(&mut self, command: CompleteTask) -> Result<TaskMutation, SdkError> {
        self.require_version(command.expected_version)?;
        self.require_monotonic_time(command.occurred_at_unix_nanos)?;

        if self.status == TaskStatus::Completed {
            return Ok(TaskMutation::Unchanged);
        }

        self.status = TaskStatus::Completed;
        self.completed_at_unix_nanos = Some(command.occurred_at_unix_nanos);
        self.reminder_at_unix_nanos = None;
        self.updated_at_unix_nanos = command.occurred_at_unix_nanos;
        self.version += 1;
        Ok(TaskMutation::Changed)
    }

    pub fn schedule_reminder(
        &mut self,
        command: ScheduleReminder,
    ) -> Result<TaskMutation, SdkError> {
        self.require_version(command.expected_version)?;
        self.require_monotonic_time(command.occurred_at_unix_nanos)?;

        if self.status == TaskStatus::Completed {
            return Err(conflict(
                "ACTIVITIES_TASK_ALREADY_COMPLETED",
                "a completed task cannot receive a reminder",
            ));
        }
        validate_timestamp(
            "task.reminder_at_unix_nanos",
            command.reminder_at_unix_nanos,
        )?;
        validate_reminder_window(
            Some(command.reminder_at_unix_nanos),
            self.due_at_unix_nanos,
            command.occurred_at_unix_nanos,
        )?;

        if self.reminder_at_unix_nanos == Some(command.reminder_at_unix_nanos) {
            return Ok(TaskMutation::Unchanged);
        }

        self.reminder_at_unix_nanos = Some(command.reminder_at_unix_nanos);
        self.updated_at_unix_nanos = command.occurred_at_unix_nanos;
        self.version += 1;
        Ok(TaskMutation::Changed)
    }

    fn require_version(&self, expected_version: i64) -> Result<(), SdkError> {
        if expected_version != self.version {
            return Err(conflict(
                "ACTIVITIES_TASK_VERSION_CONFLICT",
                format!(
                    "expected task version {expected_version}, found {}",
                    self.version
                ),
            ));
        }
        Ok(())
    }

    fn require_monotonic_time(&self, occurred_at_unix_nanos: i64) -> Result<(), SdkError> {
        validate_timestamp("task.occurred_at_unix_nanos", occurred_at_unix_nanos)?;
        if occurred_at_unix_nanos < self.updated_at_unix_nanos {
            return Err(invalid(
                "ACTIVITIES_TASK_TIME_REGRESSION",
                "task.occurred_at_unix_nanos",
                "task mutation time cannot precede the previous mutation",
            ));
        }
        Ok(())
    }
}

fn validate_subject(subject: &str) -> Result<(), SdkError> {
    if subject.trim().is_empty()
        || subject.len() > MAX_SUBJECT_BYTES
        || subject.chars().any(char::is_control)
    {
        return Err(invalid(
            "ACTIVITIES_TASK_SUBJECT_INVALID",
            "task.subject",
            format!(
                "task subject must be non-empty, contain no control characters and not exceed {MAX_SUBJECT_BYTES} bytes"
            ),
        ));
    }
    Ok(())
}

fn validate_optional_description(description: Option<&str>) -> Result<(), SdkError> {
    if description.is_some_and(|description| {
        description.len() > MAX_DESCRIPTION_BYTES || description.contains('\0')
    }) {
        return Err(invalid(
            "ACTIVITIES_TASK_DESCRIPTION_INVALID",
            "task.description",
            format!(
                "task description must not contain NUL and not exceed {MAX_DESCRIPTION_BYTES} bytes"
            ),
        ));
    }
    Ok(())
}

fn validate_related_resources(resources: &[ResourceRef]) -> Result<(), SdkError> {
    if resources.len() > MAX_RELATED_RESOURCES {
        return Err(invalid(
            "ACTIVITIES_RELATED_RESOURCES_LIMIT",
            "task.related_resources",
            format!("a task may reference at most {MAX_RELATED_RESOURCES} resources"),
        ));
    }

    let mut unique = BTreeSet::new();
    for resource in resources {
        if resource.resource_type.is_empty()
            || resource.resource_id.is_empty()
            || resource.resource_type.chars().any(char::is_control)
            || resource.resource_id.chars().any(char::is_control)
        {
            return Err(invalid(
                "ACTIVITIES_RESOURCE_REFERENCE_INVALID",
                "task.related_resources",
                "resource reference type and id must be non-empty and contain no control characters",
            ));
        }
        if !unique.insert((
            resource.resource_type.as_str(),
            resource.resource_id.as_str(),
        )) {
            return Err(invalid(
                "ACTIVITIES_RELATED_RESOURCE_DUPLICATE",
                "task.related_resources",
                "related resources must not contain duplicates",
            ));
        }
    }
    Ok(())
}

fn validate_reminder_window(
    reminder_at: Option<i64>,
    due_at: Option<i64>,
    command_time: i64,
) -> Result<(), SdkError> {
    let Some(reminder_at) = reminder_at else {
        return Ok(());
    };
    if reminder_at <= command_time {
        return Err(invalid(
            "ACTIVITIES_REMINDER_NOT_FUTURE",
            "task.reminder_at_unix_nanos",
            "reminder must be scheduled after the command time",
        ));
    }
    if due_at.is_some_and(|due_at| reminder_at > due_at) {
        return Err(invalid(
            "ACTIVITIES_REMINDER_AFTER_DUE",
            "task.reminder_at_unix_nanos",
            "reminder must not be scheduled after the task due time",
        ));
    }
    Ok(())
}

fn validate_optional_timestamp(field: &'static str, value: Option<i64>) -> Result<(), SdkError> {
    if let Some(value) = value {
        validate_timestamp(field, value)?;
    }
    Ok(())
}

fn validate_timestamp(field: &'static str, value: i64) -> Result<(), SdkError> {
    if value < 0 {
        return Err(invalid(
            "ACTIVITIES_TIMESTAMP_INVALID",
            field,
            "timestamp must not be negative",
        ));
    }
    Ok(())
}

fn validate_identifier(field: &'static str, value: &str) -> Result<(), SdkError> {
    if value.is_empty()
        || value.len() > MAX_IDENTIFIER_BYTES
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b':'))
    {
        return Err(invalid(
            "ACTIVITIES_IDENTIFIER_INVALID",
            field,
            format!(
                "identifier must use ASCII letters, digits, '.', '_', '-' or ':' and not exceed {MAX_IDENTIFIER_BYTES} bytes"
            ),
        ));
    }
    Ok(())
}

fn invalid(code: &'static str, field: &'static str, safe_message: impl Into<String>) -> SdkError {
    let safe_message = safe_message.into();
    let mut error = SdkError::new(
        code,
        ErrorCategory::InvalidArgument,
        false,
        "The task request contains invalid data.",
    );
    error.field_violations.push(FieldViolation {
        field: FieldName::try_new(field).expect("static field path must be valid"),
        code: code.to_owned(),
        safe_message,
    });
    error
}

fn conflict(code: &'static str, safe_message: impl Into<String>) -> SdkError {
    SdkError::new(code, ErrorCategory::Conflict, false, safe_message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crm_core_contracts::PageSize;

    fn open_task() -> Task {
        Task::create(CreateTask {
            task_id: RecordId::try_new("task-1").unwrap(),
            subject: "Prepare proposal".to_owned(),
            description: Some("Draft the commercial proposal and legal review notes.".to_owned()),
            owner: TaskOwner::Actor(ActorId::try_new("actor-1").unwrap()),
            related_resources: vec![ResourceRef {
                resource_type: "sales.deal".to_owned(),
                resource_id: "deal-1".to_owned(),
                version: Some(1),
            }],
            priority: TaskPriority::High,
            due_at_unix_nanos: Some(100),
            reminder_at_unix_nanos: Some(80),
            occurred_at_unix_nanos: 10,
        })
        .unwrap()
    }

    #[test]
    fn creates_typed_open_task() {
        let task = open_task();
        assert_eq!(task.status, TaskStatus::Open);
        assert_eq!(task.version, 1);
        assert_eq!(task.related_resources.len(), 1);
    }

    #[test]
    fn duplicate_related_resources_are_rejected() {
        let resource = ResourceRef {
            resource_type: "sales.deal".to_owned(),
            resource_id: "deal-1".to_owned(),
            version: None,
        };
        let error = Task::create(CreateTask {
            task_id: RecordId::try_new("task-2").unwrap(),
            subject: "Follow up".to_owned(),
            description: None,
            owner: TaskOwner::Actor(ActorId::try_new("actor-1").unwrap()),
            related_resources: vec![resource.clone(), resource],
            priority: TaskPriority::Normal,
            due_at_unix_nanos: None,
            reminder_at_unix_nanos: None,
            occurred_at_unix_nanos: 10,
        })
        .unwrap_err();
        assert_eq!(error.code, "ACTIVITIES_RELATED_RESOURCE_DUPLICATE");
    }

    #[test]
    fn completion_is_idempotent_at_current_version_and_clears_reminder() {
        let mut task = open_task();
        assert_eq!(
            task.complete(CompleteTask {
                expected_version: 1,
                occurred_at_unix_nanos: 90,
            })
            .unwrap(),
            TaskMutation::Changed
        );
        assert_eq!(task.status, TaskStatus::Completed);
        assert!(task.reminder_at_unix_nanos.is_none());
        assert_eq!(task.version, 2);

        assert_eq!(
            task.complete(CompleteTask {
                expected_version: 2,
                occurred_at_unix_nanos: 95,
            })
            .unwrap(),
            TaskMutation::Unchanged
        );
        assert_eq!(task.version, 2);
    }

    #[test]
    fn completed_task_rejects_new_reminder() {
        let mut task = open_task();
        task.complete(CompleteTask {
            expected_version: 1,
            occurred_at_unix_nanos: 90,
        })
        .unwrap();
        let error = task
            .schedule_reminder(ScheduleReminder {
                expected_version: 2,
                reminder_at_unix_nanos: 99,
                occurred_at_unix_nanos: 91,
            })
            .unwrap_err();
        assert_eq!(error.code, "ACTIVITIES_TASK_ALREADY_COMPLETED");
    }

    #[test]
    fn identical_reminder_is_idempotent_without_version_bump() {
        let mut task = open_task();
        assert_eq!(
            task.schedule_reminder(ScheduleReminder {
                expected_version: 1,
                reminder_at_unix_nanos: 80,
                occurred_at_unix_nanos: 20,
            })
            .unwrap(),
            TaskMutation::Unchanged
        );
        assert_eq!(task.version, 1);
    }

    #[test]
    fn update_uses_explicit_clear_and_version_check() {
        let mut task = open_task();
        task.apply_update(UpdateTask {
            expected_version: 1,
            subject: Patch::Keep,
            description: Patch::Clear,
            owner: Patch::Set(TaskOwner::Team(TeamId::try_new("team.enterprise").unwrap())),
            priority: Patch::Set(TaskPriority::Urgent),
            due_at_unix_nanos: Patch::Clear,
            occurred_at_unix_nanos: 20,
        })
        .unwrap();
        assert!(task.description.is_none());
        assert!(task.due_at_unix_nanos.is_none());
        assert_eq!(task.priority, TaskPriority::Urgent);
        assert_eq!(task.version, 2);
    }

    #[test]
    fn list_query_uses_bounded_shared_pagination() {
        let query = TaskListQuery {
            page: PageRequest {
                cursor: None,
                page_size: PageSize::try_new(75).unwrap(),
            },
            owner: None,
            status: Some(TaskStatus::Open),
            related_resource: None,
            sort: TaskSort::DueAtAscending,
        };
        assert_eq!(query.page.page_size.get(), 75);
    }
}
