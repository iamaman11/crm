//! Activities-owned typed planning for the first bounded automation use cases.
//! This pure domain module never executes a Task or grants authority: the
//! existing Access policy, Task owner and D1 receipt must authorize/commit it.
//! No UI/client rule interpreter, generic JSON workflow or second Task owner.
use crate::domain::{Task, TaskOwner, TaskPriority, TaskStatus};
use crate::interactions::Interaction;
use crm_module_sdk::{ErrorCategory, RecordId, SdkError};
use sha2::{Digest, Sha256};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AutomationTrigger {
    InteractionCreated,
    TaskOverdue,
}

impl AutomationTrigger {
    const fn code(self) -> &'static str {
        match self {
            Self::InteractionCreated => "interaction_created",
            Self::TaskOverdue => "task_overdue",
        }
    }
}

/// Rule revisions are immutable once published. Identity excludes revision,
/// preventing a new revision from repeating the same source occurrence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AutomationRule {
    pub rule_id: RecordId,
    pub version: i64,
    pub trigger: AutomationTrigger,
    pub task_subject: String,
    pub target: TaskOwner,
    pub priority: TaskPriority,
    pub due_after_seconds: Option<i64>,
    pub enabled: bool,
}

pub enum AutomationSource<'a> {
    InteractionCreated(&'a Interaction),
    TaskOverdue {
        task: &'a Task,
        generated_by_automation: bool,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlannedAutomationTask {
    pub task_id: RecordId,
    pub idempotency_key: String,
    pub source_id: String,
    pub source_version: i64,
    pub account_id: String,
    pub deal_id: Option<String>,
    pub subject: String,
    pub target: TaskOwner,
    pub priority: TaskPriority,
    pub due_at_unix_nanos: Option<i64>,
}

fn invalid(code: &'static str) -> SdkError {
    SdkError::new(
        code,
        ErrorCategory::InvalidArgument,
        false,
        "Invalid Activities automation",
    )
}

impl AutomationRule {
    pub fn validate(&self) -> Result<(), SdkError> {
        if self.version < 1
            || self.task_subject.trim().is_empty()
            || self.task_subject.len() > 240
            || self.due_after_seconds.is_some_and(|v| !(0..=31_536_000).contains(&v))
        {
            return Err(invalid("ACTIVITIES_AUTOMATION_RULE_INVALID"));
        }
        Ok(())
    }

    /// Pure planning, not permission evaluation or execution.
    /// A Task is never eligible when its trusted persisted run provenance
    /// says it was created by automation, so timer rules cannot self-loop.
    pub fn plan(
        &self,
        source: AutomationSource<'_>,
        now_nanos: i64,
    ) -> Result<Option<PlannedAutomationTask>, SdkError> {
        self.validate()?;
        if now_nanos < 0 {
            return Err(invalid("ACTIVITIES_AUTOMATION_TIME_INVALID"));
        }
        if !self.enabled {
            return Ok(None);
        }
        let (kind, source_id, source_version, account_id, deal_id) = match source {
            AutomationSource::InteractionCreated(interaction) => {
                if self.trigger != AutomationTrigger::InteractionCreated {
                    return Ok(None);
                }
                interaction.verify_stored()?;
                (
                    AutomationTrigger::InteractionCreated,
                    interaction.interaction_id.as_str(),
                    interaction.version,
                    interaction.account_id.clone(),
                    interaction.deal_id.clone(),
                )
            }
            AutomationSource::TaskOverdue {
                task,
                generated_by_automation,
            } => {
                if self.trigger != AutomationTrigger::TaskOverdue
                    || generated_by_automation
                    || task.status() != TaskStatus::Open
                    || !task.due_at_unix_nanos().is_some_and(|due| due <= now_nanos)
                {
                    return Ok(None);
                }
                let account = task
                    .related_resources()
                    .iter()
                    .find(|reference| reference.resource_type == "crm.customer-accounts")
                    .ok_or_else(|| invalid("ACTIVITIES_AUTOMATION_SOURCE_ACCOUNT_REQUIRED"))?;
                let deal_id = task
                    .related_resources()
                    .iter()
                    .find(|reference| reference.resource_type == "sales.deal")
                    .map(|reference| reference.resource_id.clone());
                (
                    AutomationTrigger::TaskOverdue,
                    task.task_id().as_str(),
                    task.version(),
                    account.resource_id.clone(),
                    deal_id,
                )
            }
        };
        // Length-delimited serialization prevents ambiguous concatenated IDs.
        // A revision or source Task CAS version cannot change the identity.
        let identity = serde_json::to_vec(&(self.rule_id.as_str(), kind.code(), source_id))
            .map_err(|_| invalid("ACTIVITIES_AUTOMATION_IDENTITY_INVALID"))?;
        let digest = format!("{:x}", Sha256::digest(identity));
        let due_at_unix_nanos = self
            .due_after_seconds
            .map(|seconds| {
                seconds
                    .checked_mul(1_000_000_000)
                    .and_then(|delta| now_nanos.checked_add(delta))
                    .ok_or_else(|| invalid("ACTIVITIES_AUTOMATION_TIME_OVERFLOW"))
            })
            .transpose()?;
        Ok(Some(PlannedAutomationTask {
            task_id: RecordId::try_new(format!("automation-{digest}"))
                .map_err(|_| invalid("ACTIVITIES_AUTOMATION_IDENTITY_INVALID"))?,
            idempotency_key: format!("automation-{digest}"),
            source_id: source_id.to_owned(),
            source_version,
            account_id,
            deal_id,
            subject: self.task_subject.clone(),
            target: self.target.clone(),
            priority: self.priority,
            due_at_unix_nanos,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{CompleteTask, CreateTask, TaskMutation, TeamId};
    use crate::interactions::{CreateInteraction, InteractionKind};
    use crm_module_sdk::{ActorId, ResourceRef};

    fn rule(trigger: AutomationTrigger) -> AutomationRule {
        AutomationRule {
            rule_id: RecordId::try_new("follow-up-rule").unwrap(),
            version: 1,
            trigger,
            task_subject: "Follow up with customer".into(),
            target: TaskOwner::Team(TeamId::try_new("team-one").unwrap()),
            priority: TaskPriority::High,
            due_after_seconds: Some(3_600),
            enabled: true,
        }
    }

    fn interaction() -> Interaction {
        Interaction::create(
            CreateInteraction {
                interaction_id: "interaction-1".into(),
                account_id: "account-1".into(),
                deal_id: Some("deal-1".into()),
                kind: InteractionKind::Call,
                text: "Call completed".into(),
                occurred_at_nanos: 10,
            },
            "alice",
            20,
        )
        .unwrap()
    }

    fn task() -> Task {
        Task::create(CreateTask {
            task_id: RecordId::try_new("task-one").unwrap(),
            subject: "Existing task".into(),
            description: None,
            owner: TaskOwner::Actor(ActorId::try_new("alice").unwrap()),
            related_resources: vec![ResourceRef {
                resource_type: "crm.customer-accounts".into(),
                resource_id: "account-1".into(),
                version: None,
            }],
            priority: TaskPriority::Normal,
            due_at_unix_nanos: Some(10),
            reminder_at_unix_nanos: None,
            occurred_at_unix_nanos: 1,
        })
        .unwrap()
    }

    #[test]
    fn immutable_interaction_creates_stable_authority_free_intent() {
        let rule = rule(AutomationTrigger::InteractionCreated);
        let source = interaction();
        let plan = rule
            .plan(AutomationSource::InteractionCreated(&source), 20)
            .unwrap()
            .unwrap();
        assert_eq!(plan.account_id, "account-1");
        assert_eq!(plan.deal_id.as_deref(), Some("deal-1"));
        assert_eq!(plan.source_version, 1);
        assert_eq!(plan.due_at_unix_nanos, Some(3_600_000_000_020));
        assert!(plan.idempotency_key.len() <= 120);
        let mut revised = rule.clone();
        revised.version = 2;
        revised.task_subject = "New published revision".into();
        let next = revised
            .plan(AutomationSource::InteractionCreated(&source), 25)
            .unwrap()
            .unwrap();
        assert_eq!(plan.task_id, next.task_id);
        assert_eq!(plan.idempotency_key, next.idempotency_key);
        assert_ne!(plan.subject, next.subject);
    }

    #[test]
    fn overdue_requires_open_due_original_task_and_never_self_retriggers() {
        let rule = rule(AutomationTrigger::TaskOverdue);
        let mut source = task();
        assert!(rule
            .plan(
                AutomationSource::TaskOverdue {
                    task: &source,
                    generated_by_automation: false,
                },
                9,
            )
            .unwrap()
            .is_none());
        let first = rule
            .plan(
                AutomationSource::TaskOverdue {
                    task: &source,
                    generated_by_automation: false,
                },
                10,
            )
            .unwrap()
            .unwrap();
        assert!(rule
            .plan(
                AutomationSource::TaskOverdue {
                    task: &source,
                    generated_by_automation: true,
                },
                11,
            )
            .unwrap()
            .is_none());
        assert_eq!(
            source
                .complete(CompleteTask {
                    expected_version: 1,
                    occurred_at_unix_nanos: 12,
                })
                .unwrap(),
            TaskMutation::Changed
        );
        assert!(rule
            .plan(
                AutomationSource::TaskOverdue {
                    task: &source,
                    generated_by_automation: false,
                },
                20,
            )
            .unwrap()
            .is_none());
        assert_eq!(first.source_id, "task-one");
        let mut revised = rule.clone();
        revised.version = 3;
        assert_eq!(first.idempotency_key, revised.plan(AutomationSource::TaskOverdue {
            task: &task(),
            generated_by_automation: false,
        }, 15).unwrap().unwrap().idempotency_key);
    }

    #[test]
    fn disabled_wrong_trigger_and_invalid_rule_fail_closed() {
        let mut rule = rule(AutomationTrigger::InteractionCreated);
        let source = interaction();
        rule.enabled = false;
        assert!(rule.plan(AutomationSource::InteractionCreated(&source), 20).unwrap().is_none());
        rule.enabled = true;
        rule.version = 0;
        assert_eq!(
            rule.plan(AutomationSource::InteractionCreated(&source), 20)
                .unwrap_err().code,
            "ACTIVITIES_AUTOMATION_RULE_INVALID"
        );
        rule.version = 1;
        rule.due_after_seconds = Some(-1);
        assert!(rule.validate().is_err());
        rule.due_after_seconds = Some(0);
        assert!(rule.plan(AutomationSource::TaskOverdue {
            task: &task(), generated_by_automation: false,
        }, 20).unwrap().is_none());
    }

    #[test]
    fn deadline_overflow_is_rejected_without_partial_intent() {
        let rule = rule(AutomationTrigger::InteractionCreated);
        assert_eq!(
            rule.plan(AutomationSource::InteractionCreated(&interaction()), i64::MAX)
                .unwrap_err().code,
            "ACTIVITIES_AUTOMATION_TIME_OVERFLOW"
        );
    }
}
