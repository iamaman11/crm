use crate::definitions::{
    ACTIVITIES_MODULE_ID, ACTIVITIES_RECORD_TYPE, SALES_MODULE_ID, SALES_RECORD_TYPE,
};
use crm_activities::{
    TASK_STATE_MAXIMUM_BYTES, TASK_STATE_RETENTION_POLICY_ID, TASK_STATE_SCHEMA_ID,
    TASK_STATE_SCHEMA_VERSION, Task, TaskOwner, TaskPriority, TaskStatus, decode_task_state,
    task_state_descriptor_hash,
};
use crm_capability_plan_support::{self as support, PersistedPayloadContract};
use crm_core_data::RecordSnapshot;
use crm_module_sdk::{RecordType, ResourceRef, SdkError, TenantId};
use crm_proto_contracts::crm::{
    activities::v1 as activities, core::v1 as core, sales::v1 as sales,
};
use crm_query_runtime::QueryVisibilityDecision;
use crm_sales::{
    DEAL_STATE_MAXIMUM_BYTES, DEAL_STATE_RETENTION_POLICY_ID, DEAL_STATE_SCHEMA_ID,
    DEAL_STATE_SCHEMA_VERSION, Deal, DealCloseOutcome, DealOwner, DealStage, DealStatus,
    deal_state_descriptor_hash, decode_deal_state,
};

pub(crate) fn sales_record_type() -> RecordType {
    RecordType::try_new(SALES_RECORD_TYPE).expect("configured Sales record type must be valid")
}

pub(crate) fn activities_record_type() -> RecordType {
    RecordType::try_new(ACTIVITIES_RECORD_TYPE)
        .expect("configured Activities record type must be valid")
}

pub(crate) fn deal_from_snapshot(snapshot: &RecordSnapshot) -> Result<Deal, SdkError> {
    let deal = decode_deal_state(support::persisted_json_bytes(
        snapshot,
        PersistedPayloadContract {
            owner: SALES_MODULE_ID,
            schema_id: DEAL_STATE_SCHEMA_ID,
            schema_version: DEAL_STATE_SCHEMA_VERSION,
            descriptor_hash: deal_state_descriptor_hash(),
            maximum_size_bytes: DEAL_STATE_MAXIMUM_BYTES,
            retention_policy_id: DEAL_STATE_RETENTION_POLICY_ID,
        },
    )?)?;
    if deal.deal_id() != &snapshot.reference.record_id || deal.version() != snapshot.version {
        return Err(support::stored_data_error(
            "SALES_QUERY_PERSISTED_DEAL_IDENTITY_INVALID",
        ));
    }
    Ok(deal)
}

pub(crate) fn task_from_snapshot(snapshot: &RecordSnapshot) -> Result<Task, SdkError> {
    let task = decode_task_state(support::persisted_json_bytes(
        snapshot,
        PersistedPayloadContract {
            owner: ACTIVITIES_MODULE_ID,
            schema_id: TASK_STATE_SCHEMA_ID,
            schema_version: TASK_STATE_SCHEMA_VERSION,
            descriptor_hash: task_state_descriptor_hash(),
            maximum_size_bytes: TASK_STATE_MAXIMUM_BYTES,
            retention_policy_id: TASK_STATE_RETENTION_POLICY_ID,
        },
    )?)?;
    if task.task_id() != &snapshot.reference.record_id || task.version() != snapshot.version {
        return Err(support::stored_data_error(
            "ACTIVITIES_QUERY_PERSISTED_TASK_IDENTITY_INVALID",
        ));
    }
    Ok(task)
}

#[expect(
    deprecated,
    reason = "Published Sales v1 compatibility fields remain populated when their governed field is visible."
)]
pub(crate) fn deal_to_wire(
    deal: &Deal,
    tenant: &TenantId,
    visibility: &QueryVisibilityDecision,
) -> sales::Deal {
    let show_name = visibility.allows_field("name");
    let show_stage = visibility.allows_field("stage");
    let show_amount = visibility.allows_field("amount");
    let show_owner = visibility.allows_field("owner");
    let show_account = visibility.allows_field("account");
    let show_primary_contact = visibility.allows_field("primary_contact");
    let show_expected_close = visibility.allows_field("expected_close_date");
    let show_probability = visibility.allows_field("probability_basis_points");
    let show_status = visibility.allows_field("status");
    let show_close_outcome = visibility.allows_field("close_outcome");
    let show_created_at = visibility.allows_field("created_at");
    let show_updated_at = visibility.allows_field("updated_at");

    let legacy_amount = show_amount
        .then(|| deal.amount())
        .flatten()
        .and_then(|value| i64::try_from(value.minor_units()).ok());
    let legacy_currency = legacy_amount
        .and_then(|_| deal.amount())
        .map(|value| value.currency().as_str().to_owned())
        .unwrap_or_default();
    let legacy_owner = if show_owner {
        match deal.owner() {
            DealOwner::Actor(actor) => actor.as_str().to_owned(),
            DealOwner::Team(team) => team.as_str().to_owned(),
        }
    } else {
        String::new()
    };

    sales::Deal {
        deal_id: deal.deal_id().as_str().to_owned(),
        tenant_id: tenant.as_str().to_owned(),
        name: show_name
            .then(|| deal.name().to_owned())
            .unwrap_or_default(),
        stage: show_stage
            .then(|| deal.stage().stage_id().as_str().to_owned())
            .unwrap_or_default(),
        amount_minor: legacy_amount.unwrap_or_default(),
        currency: legacy_currency,
        owner_id: legacy_owner,
        version: deal.version(),
        stage_details: show_stage.then(|| stage_to_wire(deal.stage())),
        amount: show_amount
            .then(|| deal.amount().map(support::domain_money_to_wire))
            .flatten(),
        owner: show_owner.then(|| owner_to_wire(deal.owner())),
        account: show_account
            .then(|| {
                deal.account()
                    .map(|value| support::domain_resource_to_wire(value, tenant))
            })
            .flatten(),
        primary_contact: show_primary_contact
            .then(|| {
                deal.primary_contact()
                    .map(|value| support::domain_resource_to_wire(value, tenant))
            })
            .flatten(),
        expected_close_date: show_expected_close
            .then(|| deal.expected_close_date().map(support::domain_date_to_wire))
            .flatten(),
        probability_basis_points: show_probability
            .then(|| u32::from(deal.probability().get()))
            .unwrap_or_default(),
        status: show_status
            .then(|| deal_status_to_wire(deal.status()))
            .unwrap_or_default(),
        close_outcome: show_close_outcome
            .then(|| deal.close_outcome().map(close_outcome_to_wire))
            .flatten(),
        created_at: show_created_at
            .then(|| support::nanos_to_wire_time(deal.created_at_unix_nanos())),
        updated_at: show_updated_at
            .then(|| support::nanos_to_wire_time(deal.updated_at_unix_nanos())),
    }
}

pub(crate) fn task_to_wire(
    task: &Task,
    tenant: &TenantId,
    visibility: &QueryVisibilityDecision,
) -> activities::Task {
    activities::Task {
        task_id: task.task_id().as_str().to_owned(),
        tenant_id: tenant.as_str().to_owned(),
        subject: visibility
            .allows_field("subject")
            .then(|| task.subject().to_owned())
            .unwrap_or_default(),
        description: visibility
            .allows_field("description")
            .then(|| task.description().map(str::to_owned))
            .flatten(),
        owner: visibility
            .allows_field("owner")
            .then(|| task_owner_to_wire(task.owner())),
        related_resources: if visibility.allows_field("related_resources") {
            task.related_resources()
                .iter()
                .map(|value| support::domain_resource_to_wire(value, tenant))
                .collect()
        } else {
            Vec::new()
        },
        priority: visibility
            .allows_field("priority")
            .then(|| task_priority_to_wire(task.priority()))
            .unwrap_or_default(),
        status: visibility
            .allows_field("status")
            .then(|| task_status_to_wire(task.status()))
            .unwrap_or_default(),
        due_at: visibility
            .allows_field("due_at")
            .then(|| task.due_at_unix_nanos().map(support::nanos_to_wire_time))
            .flatten(),
        reminder_at: visibility
            .allows_field("reminder_at")
            .then(|| {
                task.reminder_at_unix_nanos()
                    .map(support::nanos_to_wire_time)
            })
            .flatten(),
        completed_at: visibility
            .allows_field("completed_at")
            .then(|| {
                task.completed_at_unix_nanos()
                    .map(support::nanos_to_wire_time)
            })
            .flatten(),
        created_at: visibility
            .allows_field("created_at")
            .then(|| support::nanos_to_wire_time(task.created_at_unix_nanos())),
        updated_at: visibility
            .allows_field("updated_at")
            .then(|| support::nanos_to_wire_time(task.updated_at_unix_nanos())),
        version: task.version(),
    }
}

pub(crate) fn deal_matches(
    deal: &Deal,
    owner: Option<&core::ActorOrTeamOwner>,
    pipeline_id: Option<&str>,
    status: Option<i32>,
) -> bool {
    owner.is_none_or(|expected| deal_owner_matches(deal.owner(), expected))
        && pipeline_id.is_none_or(|expected| deal.stage().pipeline_id().as_str() == expected)
        && status.is_none_or(|expected| deal_status_to_wire(deal.status()) == expected)
}

pub(crate) fn task_matches(
    task: &Task,
    owner: Option<&core::ActorOrTeamOwner>,
    status: Option<i32>,
    related_resource: Option<&core::ResourceRef>,
) -> bool {
    owner.is_none_or(|expected| task_owner_matches(task.owner(), expected))
        && status.is_none_or(|expected| task_status_to_wire(task.status()) == expected)
        && related_resource.is_none_or(|expected| {
            task.related_resources().iter().any(|actual| {
                actual.resource_type == expected.resource_type
                    && actual.resource_id == expected.resource_id
                    && expected
                        .version
                        .is_none_or(|version| actual.version == Some(version))
            })
        })
}

fn deal_owner_matches(actual: &DealOwner, expected: &core::ActorOrTeamOwner) -> bool {
    use core::actor_or_team_owner::Owner;
    matches!(
        (actual, expected.owner.as_ref()),
        (DealOwner::Actor(actual), Some(Owner::ActorId(expected))) if actual.as_str() == expected
            | (DealOwner::Team(actual), Some(Owner::TeamId(expected))) if actual.as_str() == expected
    )
}

fn task_owner_matches(actual: &TaskOwner, expected: &core::ActorOrTeamOwner) -> bool {
    use core::actor_or_team_owner::Owner;
    matches!(
        (actual, expected.owner.as_ref()),
        (TaskOwner::Actor(actual), Some(Owner::ActorId(expected))) if actual.as_str() == expected
            | (TaskOwner::Team(actual), Some(Owner::TeamId(expected))) if actual.as_str() == expected
    )
}

fn owner_to_wire(value: &DealOwner) -> core::ActorOrTeamOwner {
    use core::actor_or_team_owner::Owner;
    core::ActorOrTeamOwner {
        owner: Some(match value {
            DealOwner::Actor(actor) => Owner::ActorId(actor.as_str().to_owned()),
            DealOwner::Team(team) => Owner::TeamId(team.as_str().to_owned()),
        }),
    }
}

fn task_owner_to_wire(value: &TaskOwner) -> core::ActorOrTeamOwner {
    use core::actor_or_team_owner::Owner;
    core::ActorOrTeamOwner {
        owner: Some(match value {
            TaskOwner::Actor(actor) => Owner::ActorId(actor.as_str().to_owned()),
            TaskOwner::Team(team) => Owner::TeamId(team.as_str().to_owned()),
        }),
    }
}

fn stage_to_wire(value: &DealStage) -> sales::DealStage {
    sales::DealStage {
        pipeline_id: value.pipeline_id().as_str().to_owned(),
        stage_id: value.stage_id().as_str().to_owned(),
        ordinal: u32::from(value.ordinal()),
    }
}

fn deal_status_to_wire(value: DealStatus) -> i32 {
    match value {
        DealStatus::Open => sales::DealStatus::Open as i32,
        DealStatus::Won => sales::DealStatus::Won as i32,
        DealStatus::Lost => sales::DealStatus::Lost as i32,
    }
}

fn close_outcome_to_wire(value: &DealCloseOutcome) -> sales::DealCloseOutcome {
    sales::DealCloseOutcome {
        status: deal_status_to_wire(value.status()),
        reason_code: value.reason_code().as_str().to_owned(),
        closed_at: Some(support::nanos_to_wire_time(value.closed_at_unix_nanos())),
    }
}

fn task_priority_to_wire(value: TaskPriority) -> i32 {
    match value {
        TaskPriority::Low => activities::TaskPriority::Low as i32,
        TaskPriority::Normal => activities::TaskPriority::Normal as i32,
        TaskPriority::High => activities::TaskPriority::High as i32,
        TaskPriority::Urgent => activities::TaskPriority::Urgent as i32,
    }
}

fn task_status_to_wire(value: TaskStatus) -> i32 {
    match value {
        TaskStatus::Open => activities::TaskStatus::Open as i32,
        TaskStatus::Completed => activities::TaskStatus::Completed as i32,
    }
}

pub(crate) fn validate_related_resource_tenant(
    value: Option<&core::ResourceRef>,
    tenant: &TenantId,
) -> Result<(), SdkError> {
    if let Some(value) = value {
        if value.tenant_id != tenant.as_str() {
            return Err(SdkError::invalid_argument(
                "task.related_resource.tenant_id",
                "resource tenant must match the authenticated tenant",
            ));
        }
        if value.resource_type.is_empty() || value.resource_id.is_empty() {
            return Err(SdkError::invalid_argument(
                "task.related_resource",
                "resource type and ID are required",
            ));
        }
    }
    Ok(())
}

pub(crate) fn resource_ref_for_visibility(snapshot: &RecordSnapshot) -> &crm_module_sdk::RecordRef {
    &snapshot.reference
}

pub(crate) fn domain_resource_matches(actual: &ResourceRef, expected: &core::ResourceRef) -> bool {
    actual.resource_type == expected.resource_type
        && actual.resource_id == expected.resource_id
        && expected
            .version
            .is_none_or(|version| actual.version == Some(version))
}
