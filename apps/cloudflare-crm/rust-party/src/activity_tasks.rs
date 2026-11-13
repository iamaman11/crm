//! Native HTTP + D1 boundary for ORIGINAL crm-activities Task aggregate.
//! Domain validity/completion rules remain exclusively in crm-activities.
//! Owner comes from a cryptographically verified Cloudflare Access principal.
use super::{bind, now_nanos};
use crm_activities::{
    CompleteTask, CreateTask, Task, TaskMutation, TaskOwner, TaskPriority, TaskStatus, UpdateTask,
    decode_task_state, encode_task_state,
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
    pub(super) priority: Priority,
    pub(super) due_at_unix_nanos: Option<i64>,
    pub(super) reminder_at_unix_nanos: Option<i64>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CompleteInput {
    pub(super) expected_version: i64,
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
    pub(super) priority: Change<Priority>,
    pub(super) due_at_unix_nanos: Change<i64>,
}
pub(super) struct MutationOutcome {
    pub(super) task: Task,
    pub(super) replayed: bool,
    pub(super) changed: bool,
}
pub(super) struct MutationFailure {
    pub(super) code: String,
    pub(super) status: u16,
}
fn fail(code: impl Into<String>, status: u16) -> MutationFailure {
    MutationFailure {
        code: code.into(),
        status,
    }
}
#[derive(Deserialize)]
struct Row {
    state: String,
}
#[derive(Deserialize)]
struct PagedRow {
    id: String,
    state: String,
    account_id: String,
}
#[derive(Deserialize)]
struct Receipt {
    digest: String,
    state: String,
}
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
        "SELECT digest,state FROM crm_activity_task_receipts WHERE tenant = ? AND key = ?",
        &[JsValue::from_str(tenant), JsValue::from_str(key)],
    )?
    .first::<Receipt>(None)
    .await
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
        return Ok(Err(fail("IDEMPOTENCY_KEY_REQUIRED", 422)));
    }
    let digest = hash("create", &input.id, &input)?;
    if let Some(p) = prior(db, tenant, key).await? {
        if p.digest != digest {
            return Ok(Err(fail("IDEMPOTENCY_CONFLICT", 409)));
        }
        let task = decode(&p.state)?;
        if task.task_id().as_str() != input.id {
            return Ok(Err(fail("IDEMPOTENCY_CONFLICT", 409)));
        }
        return Ok(Ok(MutationOutcome {
            task,
            replayed: true,
            changed: false,
        }));
    }
    let task_id = match RecordId::try_new(&input.id) {
        Ok(x) => x,
        Err(_) => return Ok(Err(fail("ACTIVITIES_TASK_ID_INVALID", 422))),
    };
    let account_id = match RecordId::try_new(&input.account_id) {
        Ok(x) => x,
        Err(_) => return Ok(Err(fail("ACTIVITIES_ACCOUNT_ID_INVALID", 422))),
    };
    if !account_exists(db, tenant, account_id.as_str()).await? {
        return Ok(Err(fail("ACTIVITIES_ACCOUNT_NOT_FOUND", 422)));
    }
    let owner = match ActorId::try_new(actor) {
        Ok(x) => TaskOwner::Actor(x),
        Err(_) => return Ok(Err(fail("ACTIVITIES_OWNER_INVALID", 422))),
    };
    let task = match Task::create(CreateTask {
        task_id,
        subject: input.subject,
        description: input.description,
        owner,
        related_resources: vec![ResourceRef {
            resource_type: "crm.customer-accounts".into(),
            resource_id: account_id.as_str().to_string(),
            version: None,
        }],
        priority: input.priority.into(),
        due_at_unix_nanos: input.due_at_unix_nanos,
        reminder_at_unix_nanos: input.reminder_at_unix_nanos,
        occurred_at_unix_nanos: now_nanos(),
    }) {
        Ok(x) => x,
        Err(e) => return Ok(Err(fail(e.code, 422))),
    };
    let saved = state(&task)?;
    let r=bind(db,"INSERT INTO crm_activity_task_records(tenant,id,account_id,state,version,idem_key,idem_digest,request_id) VALUES(?,?,?,?,?,?,?,?)",
        &[JsValue::from_str(tenant),JsValue::from_str(task.task_id().as_str()),
          JsValue::from_str(account_id.as_str()),JsValue::from_str(&saved),
          JsValue::from_f64(task.version() as f64),JsValue::from_str(key),
          JsValue::from_str(&digest),JsValue::from_str(trace)])?.run().await;
    if r.is_err() {
        return Ok(Err(fail("ACTIVITIES_TASK_CREATE_CONFLICT", 409)));
    }
    Ok(Ok(MutationOutcome {
        task,
        replayed: false,
        changed: true,
    }))
}
pub(super) async fn update_typed(
    input: UpdateInput,
    key: &str,
    db: &D1Database,
    tenant: &str,
    id: &str,
    trace: &str,
) -> Result<std::result::Result<MutationOutcome, MutationFailure>> {
    if !(8..=120).contains(&key.len()) {
        return Ok(Err(fail("IDEMPOTENCY_KEY_REQUIRED", 422)));
    }
    if matches!(&input.subject, Change::Keep)
        && matches!(&input.description, Change::Keep)
        && matches!(&input.priority, Change::Keep)
        && matches!(&input.due_at_unix_nanos, Change::Keep)
    {
        return Ok(Err(fail("ACTIVITIES_TASK_NO_CHANGES", 422)));
    }
    let digest = hash("update", id, &input)?;
    if let Some(previous) = prior(db, tenant, key).await? {
        if previous.digest != digest {
            return Ok(Err(fail("IDEMPOTENCY_CONFLICT", 409)));
        }
        let task = decode(&previous.state)?;
        if task.task_id().as_str() != id {
            return Ok(Err(fail("IDEMPOTENCY_CONFLICT", 409)));
        }
        return Ok(Ok(MutationOutcome {
            task,
            replayed: true,
            changed: false,
        }));
    }
    let Some(mut task) = stored(db, tenant, id).await? else {
        return Ok(Err(fail("NOT_FOUND", 404)));
    };
    let expected_version = input.expected_version;
    let priority = match input.priority {
        Change::Keep => Patch::Keep,
        Change::Set(value) => Patch::Set(value.into()),
        Change::Clear => Patch::Clear,
    };
    if let Err(error) = task.apply_update(UpdateTask {
        expected_version,
        subject: input.subject.into_patch(),
        description: input.description.into_patch(),
        owner: Patch::Keep,
        priority,
        due_at_unix_nanos: input.due_at_unix_nanos.into_patch(),
        occurred_at_unix_nanos: now_nanos(),
    }) {
        return Ok(Err(fail(
            error.code.clone(),
            if error.code == "ACTIVITIES_TASK_VERSION_CONFLICT" {
                409
            } else {
                422
            },
        )));
    }
    let saved = state(&task)?;
    // One CAS write; the existing D1 trigger atomically stores receipt and audit.
    let changed = bind(db,
        "UPDATE crm_activity_task_records SET state=?,version=?,idem_key=?,idem_digest=?,request_id=? WHERE tenant=? AND id=? AND version=?",
        &[JsValue::from_str(&saved), JsValue::from_f64(task.version() as f64),
          JsValue::from_str(key), JsValue::from_str(&digest), JsValue::from_str(trace),
          JsValue::from_str(tenant), JsValue::from_str(id),
          JsValue::from_f64(expected_version as f64)])?.run().await;
    let Ok(result) = changed else {
        return Ok(Err(fail("ACTIVITIES_TASK_UPDATE_CONFLICT", 409)));
    };
    if result.meta()?.and_then(|m| m.changes).unwrap_or(0) == 0 {
        return Ok(Err(fail("ACTIVITIES_TASK_VERSION_CONFLICT", 409)));
    }
    Ok(Ok(MutationOutcome {
        task,
        replayed: false,
        changed: true,
    }))
}
pub(super) async fn complete_typed(
    input: CompleteInput,
    key: &str,
    db: &D1Database,
    tenant: &str,
    id: &str,
    trace: &str,
) -> Result<std::result::Result<MutationOutcome, MutationFailure>> {
    if !(8..=120).contains(&key.len()) {
        return Ok(Err(fail("IDEMPOTENCY_KEY_REQUIRED", 422)));
    }
    let digest = hash("complete", id, &input)?;
    if let Some(p) = prior(db, tenant, key).await? {
        if p.digest != digest {
            return Ok(Err(fail("IDEMPOTENCY_CONFLICT", 409)));
        }
        let task = decode(&p.state)?;
        if task.task_id().as_str() != id {
            return Ok(Err(fail("IDEMPOTENCY_CONFLICT", 409)));
        }
        return Ok(Ok(MutationOutcome {
            task,
            replayed: true,
            changed: true,
        }));
    }
    let Some(mut task) = stored(db, tenant, id).await? else {
        return Ok(Err(fail("NOT_FOUND", 404)));
    };
    let changed = match task.complete(CompleteTask {
        expected_version: input.expected_version,
        occurred_at_unix_nanos: now_nanos(),
    }) {
        Ok(x) => x,
        Err(e) => {
            return Ok(Err(fail(
                e.code.clone(),
                if e.code == "ACTIVITIES_TASK_VERSION_CONFLICT" {
                    409
                } else {
                    422
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
            changed: false,
        }));
    }
    let saved = state(&task)?;
    let r=bind(db,"UPDATE crm_activity_task_records SET state=?,version=?,idem_key=?,idem_digest=?,request_id=? WHERE tenant=? AND id=? AND version=?",
        &[JsValue::from_str(&saved),JsValue::from_f64(task.version() as f64),
          JsValue::from_str(key),JsValue::from_str(&digest),JsValue::from_str(trace),
          JsValue::from_str(tenant),JsValue::from_str(id),
          JsValue::from_f64(input.expected_version as f64)])?.run().await;
    let Ok(result) = r else {
        return Ok(Err(fail("ACTIVITIES_TASK_COMPLETE_CONFLICT", 409)));
    };
    if result.meta()?.and_then(|m| m.changes).unwrap_or(0) == 0 {
        return Ok(Err(fail("ACTIVITIES_TASK_VERSION_CONFLICT", 409)));
    }
    Ok(Ok(MutationOutcome {
        task,
        replayed: false,
        changed: true,
    }))
}
pub(super) async fn list_page_typed(
    db: &D1Database,
    tenant: &str,
    after: &str,
    size: usize,
    status: Option<TaskStatus>,
    account_id: Option<&str>,
    priority: Option<TaskPriority>,
) -> Result<(Vec<(Task, String)>, Option<String>)> {
    let mut query = String::from(
        "SELECT id,state,account_id FROM crm_activity_task_records WHERE tenant=? AND id>?",
    );
    let mut params = vec![JsValue::from_str(tenant), JsValue::from_str(after)];
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
    if let Some(priority) = priority {
        query.push_str(" AND json_extract(state,'$.priority')=?");
        params.push(JsValue::from_str(match priority {
            TaskPriority::Low => "low",
            TaskPriority::Normal => "normal",
            TaskPriority::High => "high",
            TaskPriority::Urgent => "urgent",
        }));
    }
    query.push_str(" ORDER BY id LIMIT ?");
    params.push(JsValue::from_f64((size + 1) as f64));
    let rows = bind(db, &query, &params)?
        .all()
        .await?
        .results::<PagedRow>()?;
    let has_more = rows.len() > size;
    let next = if has_more {
        rows.get(size - 1).map(|x| x.id.clone())
    } else {
        None
    };
    let tasks = rows
        .iter()
        .take(size)
        .map(|row| decode(&row.state).map(|task| (task, row.account_id.clone())))
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
