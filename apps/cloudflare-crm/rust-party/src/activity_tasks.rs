//! Native HTTP + D1 boundary for ORIGINAL crm-activities Task aggregate.
//! Domain validity/completion rules remain exclusively in crm-activities.
//! Owner comes from a cryptographically verified Cloudflare Access principal.
use super::{bind, now_nanos};
use crm_activities::{
    CompleteTask, CreateTask, ScheduleReminder, Task, TaskMutation, TaskOwner, TaskPriority,
    TaskStatus, UpdateTask, decode_task_state, encode_task_state,
};
use crm_core_contracts::Patch;
use crm_module_sdk::{ActorId, RecordId, ResourceRef};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use wasm_bindgen::JsValue;
use worker::{D1Database, Result};

#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Priority {
    Low,
    Normal,
    High,
    Urgent,
}
impl From<Priority> for TaskPriority {
    fn from(v: Priority) -> Self {
        match v {
            Priority::Low => Self::Low,
            Priority::Normal => Self::Normal,
            Priority::High => Self::High,
            Priority::Urgent => Self::Urgent,
        }
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CreateInput {
    pub(super) id: String,
    pub(super) subject: String,
    pub(super) description: Option<String>,
    pub(super) account_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) deal_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) assignee_email: Option<String>,
    pub(super) priority: Priority,
    pub(super) due_at_unix_nanos: Option<i64>,
    pub(super) reminder_at_unix_nanos: Option<i64>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CompleteInput {
    pub(super) expected_version: i64,
}
#[derive(Serialize)]
pub(super) struct ReminderInput {
    pub(super) expected_version: i64,
    pub(super) reminder_at_unix_nanos: i64,
}
// Wire intent is serialized only for the existing D1 idempotency digest.
#[derive(Serialize)]
pub(super) enum Change<T> {
    Keep,
    Set(T),
    Clear,
}
impl<T> Change<T> {
    fn into_patch(self) -> Patch<T> {
        match self {
            Self::Keep => Patch::Keep,
            Self::Set(value) => Patch::Set(value),
            Self::Clear => Patch::Clear,
        }
    }
}
#[derive(Serialize)]
pub(super) struct UpdateInput {
    pub(super) expected_version: i64,
    pub(super) subject: Change<String>,
    pub(super) description: Change<String>,
    pub(super) owner: Change<String>,
    pub(super) priority: Change<Priority>,
    pub(super) due_at_unix_nanos: Change<i64>,
}
pub(super) struct MutationOutcome {
    pub(super) task: Task,
    pub(super) replayed: bool,
    pub(super) original_request_id: Option<String>,
    pub(super) changed: bool,
}
pub(super) use super::d1_write::OwnerFailure as MutationFailure;
use connectrpc::ErrorCode;
#[derive(Deserialize)]
struct Row {
    state: String,
}
#[derive(Deserialize)]
struct PagedRow {
    account_name: String,
    id: String,
    state: String,
    account_id: String,
}
use super::d1_write::Receipt;
fn hash<T: Serialize>(operation: &str, id: &str, payload: &T) -> Result<String> {
    let bytes = serde_json::to_vec(&json!([operation, id, payload]))
        .map_err(|e| worker::Error::RustError(e.to_string()))?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}
fn state(item: &Task) -> Result<String> {
    String::from_utf8(encode_task_state(item).map_err(|e| worker::Error::RustError(e.code))?)
        .map_err(|e| worker::Error::RustError(e.to_string()))
}
pub(super) fn decode(input: &str) -> Result<Task> {
    decode_task_state(input.as_bytes()).map_err(|e| worker::Error::RustError(e.code))
}
async fn prior(db: &D1Database, tenant: &str, key: &str) -> Result<Option<Receipt>> {
    bind(
        db,
        "SELECT digest,state,original_request_id FROM crm_activity_task_receipts WHERE tenant = ? AND key = ?",
        &[JsValue::from_str(tenant), JsValue::from_str(key)],
    )?
    .first::<Receipt>(None)
    .await
}
// A missing write acknowledgement is ambiguous. Only a matching atomic receipt
// or an explicit zero-row CAS/insert can settle it.
async fn finish_write(
    lookup: impl Future<Output = Result<Option<Receipt>>>,
    trace: &str,
    digest: &str,
    task: Task,
    write: Result<worker::D1Result>,
    zero_code: &'static str,
) -> Result<std::result::Result<MutationOutcome, MutationFailure>> {
    match super::d1_write::confirm(
        write,
        lookup,
        zero_code,
        "ACTIVITIES_TASK_WRITE_UNCONFIRMED",
    )
    .await
    {
        Ok(super::d1_write::Confirmation::Applied) => Ok(Ok(MutationOutcome {
            task,
            replayed: false,
            original_request_id: Some(trace.to_owned()),
            changed: true,
        })),
        Ok(super::d1_write::Confirmation::Replayed(receipt)) => {
            if receipt.digest != digest {
                return Ok(Err(MutationFailure::domain(
                    "IDEMPOTENCY_CONFLICT",
                    ErrorCode::Aborted,
                )));
            }
            let committed = decode(&receipt.state)?;
            if committed.task_id() != task.task_id() {
                return Ok(Err(MutationFailure::domain(
                    "IDEMPOTENCY_CONFLICT",
                    ErrorCode::Aborted,
                )));
            }
            Ok(Ok(MutationOutcome {
                task: committed,
                replayed: true,
                original_request_id: receipt.original_request_id,
                changed: true,
            }))
        }
        Err(failure) => Ok(Err(MutationFailure::Write(failure))),
    }
}
pub(super) async fn stored(db: &D1Database, tenant: &str, id: &str) -> Result<Option<Task>> {
    bind(
        db,
        "SELECT state FROM crm_activity_task_records WHERE tenant = ? AND id = ?",
        &[JsValue::from_str(tenant), JsValue::from_str(id)],
    )?
    .first::<Row>(None)
    .await?
    .map(|r| decode(&r.state))
    .transpose()
}
async fn account_exists(db: &D1Database, tenant: &str, id: &str) -> Result<bool> {
    Ok(bind(
        db,
        "SELECT id FROM crm_account_records WHERE tenant = ? AND id = ?",
        &[JsValue::from_str(tenant), JsValue::from_str(id)],
    )?
    .first::<Value>(None)
    .await?
    .is_some())
}
// ONE typed persistence boundary for Connect and historical native HTTP.
// All invariant decisions remain in the original crm-activities Task aggregate.
pub(super) async fn create_typed(
    input: CreateInput,
    key: &str,
    db: &D1Database,
    tenant: &str,
    actor: &str,
    trace: &str,
) -> Result<std::result::Result<MutationOutcome, MutationFailure>> {
    if !(8..=120).contains(&key.len()) {
        return Ok(Err(MutationFailure::domain(
            "IDEMPOTENCY_KEY_REQUIRED",
            ErrorCode::InvalidArgument,
        )));
    }
    let digest = hash("create", &input.id, &input)?;
    if let Some(p) = prior(db, tenant, key).await? {
        if p.digest != digest {
            return Ok(Err(MutationFailure::domain(
                "IDEMPOTENCY_CONFLICT",
                ErrorCode::Aborted,
            )));
        }
        let task = decode(&p.state)?;
        if task.task_id().as_str() != input.id {
            return Ok(Err(MutationFailure::domain(
                "IDEMPOTENCY_CONFLICT",
                ErrorCode::Aborted,
            )));
        }
        return Ok(Ok(MutationOutcome {
            task,
            replayed: true,
            original_request_id: p.original_request_id,
            changed: false,
        }));
    }
    let task_id = match RecordId::try_new(&input.id) {
        Ok(x) => x,
        Err(_) => {
            return Ok(Err(MutationFailure::domain(
                "ACTIVITIES_TASK_ID_INVALID",
                ErrorCode::InvalidArgument,
            )));
        }
    };
    let account_id = match RecordId::try_new(&input.account_id) {
        Ok(x) => x,
        Err(_) => {
            return Ok(Err(MutationFailure::domain(
                "ACTIVITIES_ACCOUNT_ID_INVALID",
                ErrorCode::InvalidArgument,
            )));
        }
    };
    if !account_exists(db, tenant, account_id.as_str()).await? {
        return Ok(Err(MutationFailure::domain(
            "ACTIVITIES_ACCOUNT_NOT_FOUND",
            ErrorCode::InvalidArgument,
        )));
    }
    // A manual Deal task must refer to an authorized, existing Deal owned by
    // the same Account. Use the original Sales owner read, not another SQL
    // join or a client-supplied resource type. Keep the exact Deal link in
    // the original Task domain's related_resources.
    let mut related_resources = vec![ResourceRef {
        resource_type: "crm.customer-accounts".into(),
        resource_id: account_id.as_str().to_owned(),
        version: None,
    }];
    if let Some(deal_id) = input.deal_id.as_deref() {
        let deal_id = match RecordId::try_new(deal_id) {
            Ok(value) => value,
            Err(_) => {
                return Ok(Err(MutationFailure::domain(
                    "ACTIVITIES_DEAL_ID_INVALID",
                    ErrorCode::InvalidArgument,
                )));
            }
        };
        let Some(deal) = super::sales_deals::load(db, tenant, deal_id.as_str()).await? else {
            return Ok(Err(MutationFailure::domain(
                "ACTIVITIES_DEAL_NOT_FOUND",
                ErrorCode::InvalidArgument,
            )));
        };
        if !deal.account().is_some_and(|reference| {
            reference.resource_type == "crm.customer-accounts"
                && reference.resource_id == account_id.as_str()
        }) {
            return Ok(Err(MutationFailure::domain(
                "ACTIVITIES_DEAL_ACCOUNT_MISMATCH",
                ErrorCode::InvalidArgument,
            )));
        }
        related_resources.push(ResourceRef {
            resource_type: "sales.deal".into(),
            resource_id: deal_id.as_str().to_owned(),
            version: None,
        });
    }
    // Membership is authoritative in the existing signed Access owner. The
    // lookup occurs AFTER the original receipt, so replay succeeds even if the
    // assignee was later deactivated. The INSERT also checks eligibility atomically.
    let target = if let Some(requested) = input.assignee_email.as_deref() {
        let normalized = match super::access_members::email(requested) {
            Ok(value) => value,
            Err(_) => {
                return Ok(Err(MutationFailure::domain(
                    "ACTIVITIES_ASSIGNEE_INVALID",
                    ErrorCode::InvalidArgument,
                )));
            }
        };
        if !super::access_members::valid_assignee(db, tenant, &normalized).await? {
            return Ok(Err(MutationFailure::domain(
                "ACTIVITIES_ASSIGNEE_NOT_ENROLLED",
                ErrorCode::InvalidArgument,
            )));
        }
        normalized
    } else {
        actor.to_owned()
    };
    let owner = match ActorId::try_new(target.as_str()) {
        Ok(x) => TaskOwner::Actor(x),
        Err(_) => {
            return Ok(Err(MutationFailure::domain(
                "ACTIVITIES_OWNER_INVALID",
                ErrorCode::InvalidArgument,
            )));
        }
    };
    let task = match Task::create(CreateTask {
        task_id,
        subject: input.subject,
        description: input.description,
        owner,
        related_resources,
        priority: input.priority.into(),
        due_at_unix_nanos: input.due_at_unix_nanos,
        reminder_at_unix_nanos: input.reminder_at_unix_nanos,
        occurred_at_unix_nanos: now_nanos(),
    }) {
        Ok(x) => x,
        Err(e) => {
            return Ok(Err(MutationFailure::domain(
                e.code,
                ErrorCode::InvalidArgument,
            )));
        }
    };
    let saved = state(&task)?;
    let r=bind(db,"INSERT INTO crm_activity_task_records(tenant,id,account_id,state,version,idem_key,idem_digest,request_id,actor) SELECT ?,?,?,?,?,?,?,?,? WHERE EXISTS (SELECT 1 FROM crm_users WHERE tenant_id=? AND email=? AND active=1) ON CONFLICT(tenant,id) DO NOTHING",
        &[JsValue::from_str(tenant),JsValue::from_str(task.task_id().as_str()),
          JsValue::from_str(account_id.as_str()),JsValue::from_str(&saved),
          JsValue::from_f64(task.version() as f64),JsValue::from_str(key),
          JsValue::from_str(&digest),JsValue::from_str(trace),JsValue::from_str(actor),
          JsValue::from_str(tenant),JsValue::from_str(&target)])?.run().await;
    finish_write(
        prior(db, tenant, key),
        trace,
        &digest,
        task,
        r,
        "ACTIVITIES_TASK_CREATE_CONFLICT",
    )
    .await
}
pub(super) async fn update_typed(
    input: UpdateInput,
    key: &str,
    db: &D1Database,
    tenant: &str,
    id: &str,
    actor: &str,
    trace: &str,
) -> Result<std::result::Result<MutationOutcome, MutationFailure>> {
    if !(8..=120).contains(&key.len()) {
        return Ok(Err(MutationFailure::domain(
            "IDEMPOTENCY_KEY_REQUIRED",
            ErrorCode::InvalidArgument,
        )));
    }
    if matches!(&input.subject, Change::Keep)
        && matches!(&input.description, Change::Keep)
        && matches!(&input.priority, Change::Keep)
        && matches!(&input.owner, Change::Keep)
        && matches!(&input.due_at_unix_nanos, Change::Keep)
    {
        return Ok(Err(MutationFailure::domain(
            "ACTIVITIES_TASK_NO_CHANGES",
            ErrorCode::InvalidArgument,
        )));
    }
    let digest = hash("update", id, &input)?;
    let current = stored(db, tenant, id).await;
    if let Some(previous) = prior(db, tenant, key).await? {
        if previous.digest != digest {
            return Ok(Err(MutationFailure::domain(
                "IDEMPOTENCY_CONFLICT",
                ErrorCode::Aborted,
            )));
        }
        let task = decode(&previous.state)?;
        if task.task_id().as_str() != id {
            return Ok(Err(MutationFailure::domain(
                "IDEMPOTENCY_CONFLICT",
                ErrorCode::Aborted,
            )));
        }
        return Ok(Ok(MutationOutcome {
            task,
            replayed: true,
            original_request_id: previous.original_request_id,
            changed: false,
        }));
    }
    let Some(mut task) = current? else {
        return Ok(Err(MutationFailure::domain(
            "NOT_FOUND",
            ErrorCode::NotFound,
        )));
    };
    let expected_version = input.expected_version;
    let mut assignee = None;
    let owner = match input.owner {
        Change::Keep => Patch::Keep,
        Change::Clear => {
            return Ok(Err(MutationFailure::domain(
                "ACTIVITIES_TASK_OWNER_REQUIRED",
                ErrorCode::InvalidArgument,
            )));
        }
        Change::Set(value) => {
            let normalized = match super::access_members::email(&value) {
                Ok(value) => value,
                Err(_) => {
                    return Ok(Err(MutationFailure::domain(
                        "ACTIVITIES_ASSIGNEE_INVALID",
                        ErrorCode::InvalidArgument,
                    )));
                }
            };
            if !super::access_members::valid_assignee(db, tenant, &normalized).await? {
                return Ok(Err(MutationFailure::domain(
                    "ACTIVITIES_ASSIGNEE_NOT_ENROLLED",
                    ErrorCode::InvalidArgument,
                )));
            }
            assignee = Some(normalized.clone());
            let actor = match ActorId::try_new(normalized) {
                Ok(value) => value,
                Err(_) => {
                    return Ok(Err(MutationFailure::domain(
                        "ACTIVITIES_ASSIGNEE_INVALID",
                        ErrorCode::InvalidArgument,
                    )));
                }
            };
            Patch::Set(TaskOwner::Actor(actor))
        }
    };
    let priority = match input.priority {
        Change::Keep => Patch::Keep,
        Change::Set(value) => Patch::Set(value.into()),
        Change::Clear => Patch::Clear,
    };
    if let Err(error) = task.apply_update(UpdateTask {
        expected_version,
        subject: input.subject.into_patch(),
        description: input.description.into_patch(),
        owner,
        priority,
        due_at_unix_nanos: input.due_at_unix_nanos.into_patch(),
        occurred_at_unix_nanos: now_nanos(),
    }) {
        return Ok(Err(MutationFailure::domain(
            error.code.clone(),
            if error.code == "ACTIVITIES_TASK_VERSION_CONFLICT" {
                ErrorCode::Aborted
            } else {
                ErrorCode::InvalidArgument
            },
        )));
    }
    let saved = state(&task)?;
    // One CAS write; an explicit new assignee must still be active at commit.
    // The existing D1 trigger atomically stores receipt and audit.
    let changed = bind(db,
        "UPDATE crm_activity_task_records SET state=?,version=?,idem_key=?,idem_digest=?,request_id=?,actor=? WHERE tenant=? AND id=? AND version=? AND (? IS NULL OR EXISTS (SELECT 1 FROM crm_users WHERE tenant_id=? AND email=? AND active=1))",
        &[JsValue::from_str(&saved), JsValue::from_f64(task.version() as f64),
          JsValue::from_str(key), JsValue::from_str(&digest), JsValue::from_str(trace),
          JsValue::from_str(actor),JsValue::from_str(tenant), JsValue::from_str(id),
          JsValue::from_f64(expected_version as f64),
          assignee.as_deref().map(JsValue::from_str).unwrap_or(JsValue::NULL),
          JsValue::from_str(tenant),
          assignee.as_deref().map(JsValue::from_str).unwrap_or(JsValue::NULL)])?.run().await;
    finish_write(
        prior(db, tenant, key),
        trace,
        &digest,
        task,
        changed,
        "ACTIVITIES_TASK_VERSION_CONFLICT",
    )
    .await
}
// The existing Task domain owns all schedule invariants. The SAME D1 UPDATE
// trigger as UpdateTask atomically writes the original receipt and audit.
// A same-time no-op has no receipt, version bump or fabricated audit.
fn may_work_on(task: &Task, principal: &super::access::Actor) -> bool {
    let owns = match task.owner() {
        TaskOwner::Actor(owner) => owner.as_str() == principal.identity.as_str(),
        TaskOwner::Team(_) => false,
    };
    principal.role == "admin"
        || (principal.role == "viewer" && !principal.identity.starts_with("service:") && owns)
}

// Recheck active membership and personal ownership in the same CAS statement.
// Reassignment, deactivation or a role change cannot authorize a later write.
const PERSONAL_TASK_UPDATE: &str = "UPDATE crm_activity_task_records SET state=?,version=?,idem_key=?,idem_digest=?,request_id=?,actor=? WHERE tenant=? AND id=? AND version=? AND EXISTS (SELECT 1 FROM crm_users AS m WHERE m.tenant_id=crm_activity_task_records.tenant AND m.email=? AND m.active=1 AND (m.role='admin' OR (m.role='viewer' AND m.email NOT LIKE 'service:%' AND json_extract(crm_activity_task_records.state,'$.owner.actor_id')=m.email)))";

pub(super) async fn schedule_reminder_typed(
    input: ReminderInput,
    key: &str,
    db: &D1Database,
    id: &str,
    principal: &super::access::Actor,
    trace: &str,
) -> Result<std::result::Result<MutationOutcome, MutationFailure>> {
    let tenant = principal.tenant.as_str();
    let actor = principal.identity.as_str();
    if !(8..=120).contains(&key.len()) {
        return Ok(Err(MutationFailure::domain(
            "IDEMPOTENCY_KEY_REQUIRED",
            ErrorCode::InvalidArgument,
        )));
    }
    let digest = hash("schedule_reminder", id, &input)?;
    let Some(mut task) = stored(db, tenant, id).await? else {
        return Ok(Err(MutationFailure::domain(
            "NOT_FOUND",
            ErrorCode::NotFound,
        )));
    };
    if !may_work_on(&task, principal) {
        return Ok(Err(MutationFailure::domain(
            "ACTIVITIES_TASK_FORBIDDEN",
            ErrorCode::PermissionDenied,
        )));
    }
    if let Some(previous) = prior(db, tenant, key).await? {
        if previous.digest != digest {
            return Ok(Err(MutationFailure::domain(
                "IDEMPOTENCY_CONFLICT",
                ErrorCode::Aborted,
            )));
        }
        let task = decode(&previous.state)?;
        if !may_work_on(&task, principal) {
            return Ok(Err(MutationFailure::domain(
                "ACTIVITIES_TASK_FORBIDDEN",
                ErrorCode::PermissionDenied,
            )));
        }
        if task.task_id().as_str() != id {
            return Ok(Err(MutationFailure::domain(
                "IDEMPOTENCY_CONFLICT",
                ErrorCode::Aborted,
            )));
        }
        return Ok(Ok(MutationOutcome {
            task,
            replayed: true,
            original_request_id: previous.original_request_id,
            changed: true,
        }));
    }
    let changed = match task.schedule_reminder(ScheduleReminder {
        expected_version: input.expected_version,
        reminder_at_unix_nanos: input.reminder_at_unix_nanos,
        occurred_at_unix_nanos: now_nanos(),
    }) {
        Ok(value) => value,
        Err(error) => {
            return Ok(Err(MutationFailure::domain(
                error.code.clone(),
                if error.code == "ACTIVITIES_TASK_VERSION_CONFLICT"
                    || error.code == "ACTIVITIES_TASK_ALREADY_COMPLETED"
                {
                    ErrorCode::Aborted
                } else {
                    ErrorCode::InvalidArgument
                },
            )));
        }
    };
    if changed == TaskMutation::Unchanged {
        return Ok(Ok(MutationOutcome {
            task,
            replayed: false,
            original_request_id: None,
            changed: false,
        }));
    }
    let saved = state(&task)?;
    let result = bind(
        db,
        PERSONAL_TASK_UPDATE,
        &[
            JsValue::from_str(&saved),
            JsValue::from_f64(task.version() as f64),
            JsValue::from_str(key),
            JsValue::from_str(&digest),
            JsValue::from_str(trace),
            JsValue::from_str(actor),
            JsValue::from_str(tenant),
            JsValue::from_str(id),
            JsValue::from_f64(input.expected_version as f64),
            JsValue::from_str(actor),
        ],
    )?
    .run()
    .await;
    finish_write(
        prior(db, tenant, key),
        trace,
        &digest,
        task,
        result,
        "ACTIVITIES_TASK_VERSION_CONFLICT",
    )
    .await
}
pub(super) async fn complete_typed(
    input: CompleteInput,
    key: &str,
    db: &D1Database,
    id: &str,
    principal: &super::access::Actor,
    trace: &str,
) -> Result<std::result::Result<MutationOutcome, MutationFailure>> {
    let tenant = principal.tenant.as_str();
    let actor = principal.identity.as_str();
    if !(8..=120).contains(&key.len()) {
        return Ok(Err(MutationFailure::domain(
            "IDEMPOTENCY_KEY_REQUIRED",
            ErrorCode::InvalidArgument,
        )));
    }
    let digest = hash("complete", id, &input)?;
    let Some(mut task) = stored(db, tenant, id).await? else {
        return Ok(Err(MutationFailure::domain(
            "NOT_FOUND",
            ErrorCode::NotFound,
        )));
    };
    if !may_work_on(&task, principal) {
        return Ok(Err(MutationFailure::domain(
            "ACTIVITIES_TASK_FORBIDDEN",
            ErrorCode::PermissionDenied,
        )));
    }
    if let Some(p) = prior(db, tenant, key).await? {
        if p.digest != digest {
            return Ok(Err(MutationFailure::domain(
                "IDEMPOTENCY_CONFLICT",
                ErrorCode::Aborted,
            )));
        }
        let task = decode(&p.state)?;
        if !may_work_on(&task, principal) {
            return Ok(Err(MutationFailure::domain(
                "ACTIVITIES_TASK_FORBIDDEN",
                ErrorCode::PermissionDenied,
            )));
        }
        if task.task_id().as_str() != id {
            return Ok(Err(MutationFailure::domain(
                "IDEMPOTENCY_CONFLICT",
                ErrorCode::Aborted,
            )));
        }
        return Ok(Ok(MutationOutcome {
            task,
            replayed: true,
            original_request_id: p.original_request_id,
            changed: true,
        }));
    }
    let changed = match task.complete(CompleteTask {
        expected_version: input.expected_version,
        occurred_at_unix_nanos: now_nanos(),
    }) {
        Ok(x) => x,
        Err(e) => {
            return Ok(Err(MutationFailure::domain(
                e.code.clone(),
                if e.code == "ACTIVITIES_TASK_VERSION_CONFLICT" {
                    ErrorCode::Aborted
                } else {
                    ErrorCode::InvalidArgument
                },
            )));
        }
    };
    // Original domain already owns unchanged completion. No D1 receipt/audit
    // is invented for an operation that did not write.
    if changed == TaskMutation::Unchanged {
        return Ok(Ok(MutationOutcome {
            task,
            replayed: false,
            original_request_id: None,
            changed: false,
        }));
    }
    let saved = state(&task)?;
    let r = bind(
        db,
        PERSONAL_TASK_UPDATE,
        &[
            JsValue::from_str(&saved),
            JsValue::from_f64(task.version() as f64),
            JsValue::from_str(key),
            JsValue::from_str(&digest),
            JsValue::from_str(trace),
            JsValue::from_str(actor),
            JsValue::from_str(tenant),
            JsValue::from_str(id),
            JsValue::from_f64(input.expected_version as f64),
            JsValue::from_str(actor),
        ],
    )?
    .run()
    .await;
    finish_write(
        prior(db, tenant, key),
        trace,
        &digest,
        task,
        r,
        "ACTIVITIES_TASK_VERSION_CONFLICT",
    )
    .await
}
// A single typed, owner-local filter value instead of growing positional
// parameters. This is not a generic registry or a second query owner.
pub(super) struct TaskListFilters<'a> {
    pub(super) status: Option<TaskStatus>,
    pub(super) account_id: Option<&'a str>,
    pub(super) priority: Option<TaskPriority>,
    pub(super) deal_id: Option<&'a str>,
    pub(super) search: &'a str,
    pub(super) overdue_only: bool,
    pub(super) due_from: Option<i64>,
    pub(super) due_before: Option<i64>,
    pub(super) sort_by_due: bool,
    pub(super) without_due_only: bool,
    pub(super) owner_identity: Option<&'a str>,
}
pub(super) async fn list_page_typed(
    db: &D1Database,
    tenant: &str,
    after: &str,
    size: usize,
    filters: TaskListFilters<'_>,
) -> Result<(Vec<(Task, String, String)>, Option<String>)> {
    let TaskListFilters {
        status,
        account_id,
        priority,
        deal_id,
        search,
        overdue_only,
        due_from,
        due_before,
        sort_by_due,
        without_due_only,
        owner_identity,
    } = filters;
    // Exactly one Task owner. The order/filter decision is made in SQL BEFORE
    // the keyset LIMIT; a browser cannot find due work in an ID-sorted page.
    let due_sql = "CAST(json_extract(t.state,'$.due_at_unix_nanos') AS INTEGER)";
    let mut query = String::from(
        "SELECT id,state,account_id, (SELECT a.search_name FROM crm_account_records a WHERE a.tenant=t.tenant AND a.id=t.account_id) AS account_name FROM crm_activity_task_records AS t WHERE tenant=?",
    );
    let mut params = vec![JsValue::from_str(tenant)];
    if !sort_by_due {
        query.push_str(" AND t.id>?");
        params.push(JsValue::from_str(after));
    }
    // Filters are checked in native SQL BEFORE LIMIT/cursor calculation.
    if let Some(status) = status {
        query.push_str(" AND json_extract(state,'$.status')=?");
        params.push(JsValue::from_str(match status {
            TaskStatus::Open => "open",
            TaskStatus::Completed => "completed",
        }));
    }
    if let Some(id) = account_id {
        query.push_str(" AND account_id=?");
        params.push(JsValue::from_str(id));
    }
    if let Some(owner) = owner_identity {
        // TaskOwner actor identity is a typed original domain property. Filter
        // before LIMIT and any due cursor, never by a frontend slice.
        query.push_str(" AND json_extract(t.state,'$.owner.actor_id')=?");
        params.push(JsValue::from_str(owner));
    }
    if let Some(priority) = priority {
        query.push_str(" AND json_extract(state,'$.priority')=?");
        params.push(JsValue::from_str(match priority {
            TaskPriority::Low => "low",
            TaskPriority::Normal => "normal",
            TaskPriority::High => "high",
            TaskPriority::Urgent => "urgent",
        }));
    }
    if let Some(deal_id) = deal_id {
        // The original typed Task aggregate owns its related_resources.
        // Filter BEFORE LIMIT/page-token, matching only actual sales.deal IDs
        // in the same tenant. No generic resource syntax, no new registry.
        query.push_str(
            " AND EXISTS (SELECT 1 FROM json_each(t.state,'$.related_resources') AS related WHERE json_extract(related.value,'$.resource_type')='sales.deal' AND json_extract(related.value,'$.resource_id')=?)",
        );
        params.push(JsValue::from_str(deal_id));
    }
    // The signed original Task owner filters before LIMIT and the ID cursor.
    // SQLite instr is case-sensitive and all input remains parameterized.
    if !search.is_empty() {
        query.push_str(" AND (instr(json_extract(state,'$.subject'),?)>0 OR instr(id,?)>0)");
        params.push(JsValue::from_str(search));
        params.push(JsValue::from_str(search));
    }
    // The original signed owner knows the authoritative task status/due time.
    // Use an integer nanosecond comparison against the server clock BEFORE
    // the existing ID-keyset cursor/LIMIT, never page-filter in the browser.
    if overdue_only {
        query.push_str(
            " AND json_extract(state,'$.status')='open' AND json_extract(state,'$.due_at_unix_nanos') IS NOT NULL AND CAST(json_extract(state,'$.due_at_unix_nanos') AS INTEGER) < CAST(? AS INTEGER)",
        );
        params.push(JsValue::from_str(&now_nanos().to_string()));
    }
    if without_due_only {
        query.push_str(" AND json_extract(t.state,'$.due_at_unix_nanos') IS NULL");
    } else {
        if let Some(from) = due_from {
            query.push_str(" AND ");
            query.push_str(due_sql);
            query.push_str(" >= CAST(? AS INTEGER)");
            params.push(JsValue::from_str(&from.to_string()));
        }
        if let Some(before) = due_before {
            query.push_str(" AND ");
            query.push_str(due_sql);
            query.push_str(" < CAST(? AS INTEGER)");
            params.push(JsValue::from_str(&before.to_string()));
        }
    }
    if sort_by_due {
        query.push_str(" AND json_extract(t.state,'$.due_at_unix_nanos') IS NOT NULL");
        if !after.is_empty() {
            // Handler already validates the exact '<nanos>|<Task id>' format.
            let (time, id) = after
                .split_once('|')
                .ok_or_else(|| worker::Error::RustError("INVALID_TASK_DUE_CURSOR".into()))?;
            query.push_str(" AND (");
            query.push_str(due_sql);
            query.push_str(" > CAST(? AS INTEGER) OR (");
            query.push_str(due_sql);
            query.push_str(" = CAST(? AS INTEGER) AND t.id>?))");
            params.push(JsValue::from_str(time));
            params.push(JsValue::from_str(time));
            params.push(JsValue::from_str(id));
        }
        query.push_str(" ORDER BY ");
        query.push_str(due_sql);
        query.push_str(" ASC,t.id ASC LIMIT ?");
    } else {
        query.push_str(" ORDER BY t.id LIMIT ?");
    }
    params.push(JsValue::from_f64((size + 1) as f64));
    let rows = bind(db, &query, &params)?
        .all()
        .await?
        .results::<PagedRow>()?;
    let has_more = rows.len() > size;
    let next = if has_more {
        rows.get(size - 1)
            .map(|row| {
                if sort_by_due {
                    let task = decode(&row.state)?;
                    let due = task.due_at_unix_nanos().ok_or_else(|| {
                        worker::Error::RustError("Task due sort returned undated row".into())
                    })?;
                    Ok::<String, worker::Error>(format!("{}|{}", due, row.id))
                } else {
                    Ok::<String, worker::Error>(row.id.clone())
                }
            })
            .transpose()?
    } else {
        None
    };
    let tasks = rows
        .iter()
        .take(size)
        .map(|row| {
            decode(&row.state).map(|task| (task, row.account_id.clone(), row.account_name.clone()))
        })
        .collect::<Result<Vec<_>>>()?;
    Ok((tasks, next))
}
#[derive(Deserialize)]
struct TaskAccountRow {
    account_id: String,
}
pub(super) async fn account_id(db: &D1Database, tenant: &str, id: &str) -> Result<Option<String>> {
    // Sales-linked tasks refer to the Deal in their original domain state.
    // Their Account association is the tenant-scoped D1 foreign key.
    Ok(bind(
        db,
        "SELECT account_id FROM crm_activity_task_records WHERE tenant=? AND id=?",
        &[JsValue::from_str(tenant), JsValue::from_str(id)],
    )?
    .first::<TaskAccountRow>(None)
    .await?
    .map(|row| row.account_id))
}

#[derive(Deserialize)]
pub(super) struct TaskWorkload {
    pub(super) assignee_identity: String,
    pub(super) owner_team_id: String,
    pub(super) open_count: i64,
    pub(super) completed_count: i64,
    pub(super) overdue_open_count: i64,
    pub(super) due_next_7_days_count: i64,
}
pub(super) async fn workload(db: &D1Database, tenant: &str) -> Result<(Vec<TaskWorkload>, i64)> {
    let now = now_nanos();
    let week = now.saturating_add(7 * 24 * 3600 * 1_000_000_000);
    // This is the ORIGINAL Task owner, aggregating ALL tenant tasks in D1,
    // independently of pagination, while keeping Actor and Team disjoint.
    let sql = "SELECT COALESCE(json_extract(state,'$.owner.actor_id'),'') AS assignee_identity,COALESCE(json_extract(state,'$.owner.team_id'),'') AS owner_team_id,       SUM(CASE WHEN json_extract(state,'$.status')='open' THEN 1 ELSE 0 END) AS open_count,       SUM(CASE WHEN json_extract(state,'$.status')='completed' THEN 1 ELSE 0 END) AS completed_count,       SUM(CASE WHEN json_extract(state,'$.status')='open' AND CAST(json_extract(state,'$.due_at_unix_nanos') AS INTEGER)<CAST(? AS INTEGER) THEN 1 ELSE 0 END) AS overdue_open_count,       SUM(CASE WHEN json_extract(state,'$.status')='open' AND CAST(json_extract(state,'$.due_at_unix_nanos') AS INTEGER)>=CAST(? AS INTEGER) AND CAST(json_extract(state,'$.due_at_unix_nanos') AS INTEGER)<CAST(? AS INTEGER) THEN 1 ELSE 0 END) AS due_next_7_days_count       FROM crm_activity_task_records WHERE tenant=? GROUP BY assignee_identity,owner_team_id ORDER BY assignee_identity,owner_team_id";
    let result = bind(
        db,
        sql,
        &[
            JsValue::from_str(&now.to_string()),
            JsValue::from_str(&now.to_string()),
            JsValue::from_str(&week.to_string()),
            JsValue::from_str(tenant),
        ],
    )?
    .all()
    .await?
    .results::<TaskWorkload>()?;
    Ok((result, now / 1_000_000_000))
}
