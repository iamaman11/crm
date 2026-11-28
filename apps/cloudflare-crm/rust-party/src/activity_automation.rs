//! Original Activities automation rule and manual execution D1 boundary.
//! No new authorization plane: Connect's compiled policy requires a signed
//! enrolled administrator. The single Task owner writes the actual Task,
//! original receipt and audit. This module only owns rule/run provenance.
use super::d1_write::{self, Confirmation};
use super::{access, activity_interactions, activity_tasks, bind, now_nanos};
use connectrpc::{ConnectError, ErrorCode};
use crm_activities::automation::{AutomationRule, AutomationSource, AutomationTrigger};
use crm_activities::{TaskOwner, TaskPriority};
use crm_module_sdk::{ActorId, RecordId};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use wasm_bindgen::JsValue;
use worker::D1Database;

pub(super) const MAX_ATTEMPTS: i64 = 5;
const MAX_SCHEDULED_BATCH: u32 = 50;
pub(super) const AUTOMATION_ACTOR: &str = "service:crm-automation";

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(super) struct RuleRow {
    pub rule_id: String,
    pub version: i64,
    pub trigger: String,
    pub enabled: i64,
    pub task_subject: String,
    pub assignee_email: String,
    pub due_after_seconds: i64,
    #[serde(default)]
    pub published_at_unix_nanos: i64,
}
#[derive(Clone, Serialize)]
pub(super) struct RuleInput {
    pub rule_id: String,
    pub trigger: String,
    pub enabled: bool,
    pub task_subject: String,
    pub assignee_email: String,
    pub due_after_seconds: i64,
}
pub(super) struct RuleSaved {
    pub rule: RuleRow,
    pub replayed: bool,
    pub original_request_id: Option<String>,
}
#[derive(Deserialize)]
struct SourceRow {
    event_id: String,
    #[serde(default)]
    source_version: i64,
}
#[derive(Clone, Deserialize)]
pub(super) struct RunRow {
    pub seq: i64,
    pub rule_id: String,
    pub rule_version: i64,
    pub source_event_id: String,
    pub source_type: String,
    pub source_id: String,
    pub source_version: i64,
    pub interaction_id: String,
    pub source_stale: i64,
    pub task_id: String,
    pub task_intent: String,
    pub status: String,
    pub requested_by: String,
    pub original_request_id: String,
    pub task_request_id: Option<String>,
    pub created_at_unix_millis: i64,
    pub completed_at_unix_millis: Option<i64>,
    receipt_exists: i64,
    rule_current: i64,
    assignee_active: i64,
    pub attempt_count: i64,
    pub next_attempt_at_unix_millis: i64,
    pub lease_until_unix_millis: i64,
    lease_request_id: Option<String>,
    lease_active: i64,
    backoff_active: i64,
    task_exists: i64,
    cancel_key: Option<String>,
    pub cancel_request_id: Option<String>,
    pub cancelled_by: Option<String>,
    pub cancelled_at_unix_millis: Option<i64>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RunDisposition {
    Committed,
    RetryReady,
    WaitingAssignee,
    RuleSuperseded,
    ReconcileReceipt,
    Executing,
    Backoff,
    AttemptsExhausted,
    Cancelled,
    SourceStale,
}
impl RunRow {
    pub(super) fn can_cancel(&self) -> bool {
        self.status == "pending"
            && self.cancel_request_id.is_none()
            && self.receipt_exists == 0
            && self.task_exists == 0
            && self.source_stale == 0
    }

    pub(super) fn disposition(&self) -> Result<RunDisposition, ConnectError> {
        match self.status.as_str() {
            "committed" => Ok(RunDisposition::Committed),
            "pending" if self.cancel_request_id.is_some() => Ok(RunDisposition::Cancelled),
            "pending" if self.receipt_exists == 1 => Ok(RunDisposition::ReconcileReceipt),
            "pending" if self.source_stale == 1 => Ok(RunDisposition::SourceStale),
            "pending" if self.rule_current != 1 => Ok(RunDisposition::RuleSuperseded),
            "pending" if self.assignee_active != 1 => Ok(RunDisposition::WaitingAssignee),
            "pending" if self.lease_active == 1 => Ok(RunDisposition::Executing),
            "pending" if self.attempt_count >= MAX_ATTEMPTS => {
                Ok(RunDisposition::AttemptsExhausted)
            }
            "pending" if self.backoff_active == 1 => Ok(RunDisposition::Backoff),
            "pending" => Ok(RunDisposition::RetryReady),
            _ => Err(ConnectError::internal(
                "ACTIVITIES_AUTOMATION_RUN_STATUS_INVALID",
            )),
        }
    }
}
pub(super) struct RunSaved {
    pub run: RunRow,
    pub replayed: bool,
    pub original_request_id: Option<String>,
}

fn storage() -> ConnectError {
    ConnectError::unavailable("ACTIVITIES_AUTOMATION_D1_UNAVAILABLE")
}
fn invalid(code: &'static str) -> ConnectError {
    ConnectError::invalid_argument(code)
}

pub(super) fn rule_id(value: &str) -> Result<RecordId, ConnectError> {
    if value != value.trim() || value.contains('/') {
        return Err(invalid("ACTIVITIES_AUTOMATION_RULE_ID_INVALID"));
    }
    RecordId::try_new(value.to_owned())
        .map_err(|_| invalid("ACTIVITIES_AUTOMATION_RULE_ID_INVALID"))
}

pub(super) fn check(input: &RuleInput) -> Result<AutomationRule, ConnectError> {
    let trigger = match input.trigger.as_str() {
        "interaction_created" => AutomationTrigger::InteractionCreated,
        "deal_created" => AutomationTrigger::DealCreated,
        "task_overdue" => AutomationTrigger::TaskOverdue,
        _ => return Err(invalid("ACTIVITIES_AUTOMATION_TRIGGER_NOT_ENABLED")),
    };
    if input.due_after_seconds < 0
        || input.due_after_seconds > 31_536_000
        || input.assignee_email.starts_with("service:")
        || input.assignee_email.trim() != input.assignee_email
    {
        return Err(invalid("ACTIVITIES_AUTOMATION_RULE_INVALID"));
    }
    let owner = TaskOwner::Actor(
        ActorId::try_new(input.assignee_email.clone())
            .map_err(|_| invalid("ACTIVITIES_AUTOMATION_ASSIGNEE_INVALID"))?,
    );
    let value = AutomationRule {
        rule_id: rule_id(&input.rule_id)?,
        version: 1,
        trigger,
        task_subject: input.task_subject.clone(),
        target: owner,
        priority: TaskPriority::High,
        due_after_seconds: (input.due_after_seconds > 0).then_some(input.due_after_seconds),
        enabled: input.enabled,
    };
    value.validate().map_err(|e| invalid_code(e.code))?;
    Ok(value)
}
fn invalid_code(code: String) -> ConnectError {
    ConnectError::invalid_argument(code)
}

pub(super) fn wire_input(row: &RuleRow) -> RuleInput {
    RuleInput {
        rule_id: row.rule_id.clone(),
        trigger: row.trigger.clone(),
        enabled: row.enabled == 1,
        task_subject: row.task_subject.clone(),
        assignee_email: row.assignee_email.clone(),
        due_after_seconds: row.due_after_seconds,
    }
}
fn digest(input: &RuleInput, version: Option<i64>) -> Result<String, ConnectError> {
    let bytes = serde_json::to_vec(&json!(["put_automation_rule", input, version]))
        .map_err(|_| storage())?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}
async fn receipt(
    db: &D1Database,
    tenant: &str,
    key: &str,
) -> worker::Result<Option<d1_write::Receipt>> {
    bind(db,
        "SELECT digest,state,original_request_id FROM crm_activity_automation_rule_receipts WHERE tenant=? AND key=?",
        &[JsValue::from_str(tenant), JsValue::from_str(key)])?
        .first::<d1_write::Receipt>(None).await
}
fn saved_receipt(
    value: d1_write::Receipt,
    expected_digest: &str,
    rule: &str,
    replayed: bool,
) -> Result<RuleSaved, ConnectError> {
    if value.digest != expected_digest {
        return Err(ConnectError::aborted("IDEMPOTENCY_CONFLICT"));
    }
    let stored: RuleRow = serde_json::from_str(&value.state).map_err(|_| storage())?;
    if stored.rule_id != rule {
        return Err(ConnectError::aborted("IDEMPOTENCY_CONFLICT"));
    }
    Ok(RuleSaved {
        rule: stored,
        replayed,
        original_request_id: value.original_request_id,
    })
}
pub(super) async fn get(
    db: &D1Database,
    tenant: &str,
    id: &str,
) -> Result<Option<RuleRow>, ConnectError> {
    bind(
        db,
        concat!(
            "SELECT id AS rule_id,version,trigger_kind AS trigger,enabled,task_subject,",
            "assignee_email,due_after_seconds,published_at_unix_nanos FROM crm_activity_automation_rules",
            " WHERE tenant=? AND id=?"
        ),
        &[JsValue::from_str(tenant), JsValue::from_str(id)],
    )
    .map_err(|_| storage())?
    .first::<RuleRow>(None)
    .await
    .map_err(|_| storage())
}
pub(super) async fn get_receipt(
    db: &D1Database,
    tenant: &str,
    key: &str,
) -> Result<Option<d1_write::Receipt>, ConnectError> {
    receipt(db, tenant, key).await.map_err(|_| storage())
}
pub(super) async fn put(
    db: &D1Database,
    principal: &access::Actor,
    input: RuleInput,
    key: &str,
    expected: Option<i64>,
    request_id: &str,
) -> Result<RuleSaved, ConnectError> {
    let tenant = &principal.tenant;
    let _checked = check(&input)?;
    let hash = digest(&input, expected)?;
    if let Some(prior) = receipt(db, tenant, key).await.map_err(|_| storage())? {
        return saved_receipt(prior, &hash, &input.rule_id, true);
    }
    if input.enabled
        && !super::access_members::valid_assignee(db, tenant, &input.assignee_email)
            .await
            .map_err(|_| storage())?
    {
        return Err(invalid("ACTIVITIES_AUTOMATION_ASSIGNEE_NOT_ACTIVE"));
    }
    let insert = expected.is_none();
    let sql = if insert {
        concat!(
            "INSERT INTO crm_activity_automation_rules",
            "(tenant,id,version,trigger,trigger_kind,enabled,task_subject,assignee_email,",
            "due_after_seconds,published_at_unix_nanos,idem_key,idem_digest,request_id,actor)",
            " VALUES(?,?,1,'interaction_created',?,?,?,?,?,",
            " CAST(strftime('%s','now') AS INTEGER)*1000000000+",
            " CAST(substr(strftime('%f','now'),4,3) AS INTEGER)*1000000,?,?,?,?)",
            " ON CONFLICT(tenant,id) DO NOTHING"
        )
    } else {
        concat!(
            "UPDATE crm_activity_automation_rules SET version=version+1,",
            "trigger='interaction_created',trigger_kind=?,enabled=?,task_subject=?,assignee_email=?,",
            "due_after_seconds=?,published_at_unix_nanos=",
            "CAST(strftime('%s','now') AS INTEGER)*1000000000+",
            "CAST(substr(strftime('%f','now'),4,3) AS INTEGER)*1000000,",
            "idem_key=?,idem_digest=?,request_id=?,actor=?",
            " WHERE tenant=? AND id=? AND version=?"
        )
    };
    let fields = if insert {
        vec![
            JsValue::from_str(tenant),
            JsValue::from_str(&input.rule_id),
            JsValue::from_str(&input.trigger),
            JsValue::from_f64(if input.enabled { 1.0 } else { 0.0 }),
            JsValue::from_str(&input.task_subject),
            JsValue::from_str(&input.assignee_email),
            JsValue::from_f64(input.due_after_seconds as f64),
            JsValue::from_str(key),
            JsValue::from_str(&hash),
            JsValue::from_str(request_id),
            JsValue::from_str(&principal.identity),
        ]
    } else {
        vec![
            JsValue::from_str(&input.trigger),
            JsValue::from_f64(if input.enabled { 1.0 } else { 0.0 }),
            JsValue::from_str(&input.task_subject),
            JsValue::from_str(&input.assignee_email),
            JsValue::from_f64(input.due_after_seconds as f64),
            JsValue::from_str(key),
            JsValue::from_str(&hash),
            JsValue::from_str(request_id),
            JsValue::from_str(&principal.identity),
            JsValue::from_str(tenant),
            JsValue::from_str(&input.rule_id),
            JsValue::from_f64(expected.unwrap_or_default() as f64),
        ]
    };
    let write = bind(db, sql, &fields).map_err(|_| storage())?.run().await;
    match d1_write::confirm(
        write,
        receipt(db, tenant, key),
        if insert {
            "ACTIVITIES_AUTOMATION_RULE_EXISTS"
        } else {
            "ACTIVITIES_AUTOMATION_RULE_VERSION_CONFLICT"
        },
        "ACTIVITIES_AUTOMATION_RULE_COMMIT_UNCONFIRMED",
    )
    .await
    .map_err(|error| super::diagnostics::mutation_error("PutAutomationRule", request_id, error))?
    {
        Confirmation::Applied => {
            let committed = receipt(db, tenant, key)
                .await
                .map_err(|_| storage())?
                .ok_or_else(|| ConnectError::unavailable("AUTOMATION_RULE_RECEIPT_MISSING"))?;
            saved_receipt(committed, &hash, &input.rule_id, false)
        }
        Confirmation::Replayed(prior) => saved_receipt(prior, &hash, &input.rule_id, true),
    }
}
pub(super) async fn list_rules(
    db: &D1Database,
    tenant: &str,
    after: &str,
    limit: u32,
) -> Result<Vec<RuleRow>, ConnectError> {
    bind(
        db,
        concat!(
            "SELECT id AS rule_id,version,trigger_kind AS trigger,enabled,task_subject,",
            "assignee_email,due_after_seconds,published_at_unix_nanos FROM crm_activity_automation_rules",
            " WHERE tenant=? AND id>? ORDER BY id LIMIT ?"
        ),
        &[
            JsValue::from_str(tenant),
            JsValue::from_str(after),
            JsValue::from_f64(limit as f64),
        ],
    )
    .map_err(|_| storage())?
    .all()
    .await
    .map_err(|_| storage())?
    .results::<RuleRow>()
    .map_err(|_| storage())
}
async fn source(
    db: &D1Database,
    tenant: &str,
    interaction_id: &str,
) -> Result<SourceRow, ConnectError> {
    bind(
        db,
        concat!(
            "SELECT event_id,source_version FROM crm_activity_automation_source_events",
            " WHERE tenant=? AND source_id=?",
            " AND event_type='crm.activities.interaction_created.v1'"
        ),
        &[JsValue::from_str(tenant), JsValue::from_str(interaction_id)],
    )
    .map_err(|_| storage())?
    .first::<SourceRow>(None)
    .await
    .map_err(|_| storage())?
    .ok_or_else(|| ConnectError::not_found("ACTIVITIES_AUTOMATION_SOURCE_NOT_FOUND"))
}
// One indexed D1 statement projects current facts for each claimed run.
// A receipt candidate takes precedence over revoked authority; only the
// original Task owner may validate its digest and reconcile the actual commit.
const RUN_SELECT: &str = concat!(
    "SELECT a.seq,a.rule_id,a.rule_version,a.source_event_id,a.source_type,a.source_id,",
    "a.source_version,COALESCE(a.interaction_id,'') AS interaction_id,a.source_stale,a.task_id,",
    "a.task_intent,a.status,a.requested_by,a.original_request_id,a.task_request_id,",
    "a.created_at_unix_millis,a.completed_at_unix_millis,",
    "EXISTS(SELECT 1 FROM crm_activity_task_receipts t WHERE t.tenant=a.tenant",
    " AND t.key=a.task_id) AS receipt_exists,",
    "EXISTS(SELECT 1 FROM crm_activity_automation_rules r WHERE r.tenant=a.tenant",
    " AND r.id=a.rule_id AND r.version=a.rule_version AND r.enabled=1) AS rule_current,",
    "EXISTS(SELECT 1 FROM crm_users u WHERE u.tenant_id=a.tenant",
    " AND u.email=json_extract(a.task_intent,'$.assignee_email') AND u.active=1)",
    " AS assignee_active,a.attempt_count,a.next_attempt_at_unix_millis,",
    "a.lease_until_unix_millis,a.lease_request_id,",
    "(a.lease_until_unix_millis>unixepoch('now')*1000) AS lease_active,",
    "(a.next_attempt_at_unix_millis>unixepoch('now')*1000) AS backoff_active,",
    "EXISTS(SELECT 1 FROM crm_activity_task_records t WHERE t.tenant=a.tenant",
    " AND t.id=a.task_id) AS task_exists,",
    "a.cancel_key,a.cancel_request_id,a.cancelled_by,a.cancelled_at_unix_millis",
    " FROM crm_activity_automation_runs a"
);

async fn read_run(
    db: &D1Database,
    tenant: &str,
    rule_id: &str,
    event_id: &str,
) -> Result<Option<RunRow>, ConnectError> {
    bind(
        db,
        &format!("{RUN_SELECT} WHERE a.tenant=? AND a.rule_id=? AND a.source_event_id=?"),
        &[
            JsValue::from_str(tenant),
            JsValue::from_str(rule_id),
            JsValue::from_str(event_id),
        ],
    )
    .map_err(|_| storage())?
    .first::<RunRow>(None)
    .await
    .map_err(|_| storage())
}
pub(super) async fn list_runs(
    db: &D1Database,
    tenant: &str,
    rule_id: &str,
    after: i64,
    limit: u32,
) -> Result<Vec<RunRow>, ConnectError> {
    bind(
        db,
        &format!(
            "{RUN_SELECT} WHERE a.tenant=? AND (?='' OR a.rule_id=?) AND a.seq>? ORDER BY a.seq LIMIT ?"
        ),
        &[
            JsValue::from_str(tenant),
            JsValue::from_str(rule_id),
            JsValue::from_str(rule_id),
            JsValue::from_f64(after as f64),
            JsValue::from_f64(limit as f64),
        ],
    )
    .map_err(|_| storage())?
    .all()
    .await
    .map_err(|_| storage())?
    .results::<RunRow>()
    .map_err(|_| storage())
}
#[derive(Deserialize)]
struct RunRequest {
    rule_id: String,
    source_event_id: String,
    digest: String,
}

async fn run_request(
    db: &D1Database,
    tenant: &str,
    key: &str,
) -> Result<Option<RunRequest>, ConnectError> {
    bind(
        db,
        "SELECT rule_id,source_event_id,digest FROM crm_activity_automation_run_requests WHERE tenant=? AND key=?",
        &[JsValue::from_str(tenant), JsValue::from_str(key)],
    )
    .map_err(|_| storage())?
    .first::<RunRequest>(None)
    .await
    .map_err(|_| storage())
}

async fn verify_run_request(
    db: &D1Database,
    tenant: &str,
    key: &str,
    rule: &str,
    event: &str,
    digest: &str,
) -> Result<bool, ConnectError> {
    match run_request(db, tenant, key).await? {
        None => Ok(false),
        Some(saved) => {
            if saved.rule_id == rule && saved.source_event_id == event && saved.digest == digest {
                Ok(true)
            } else {
                Err(ConnectError::aborted("IDEMPOTENCY_CONFLICT"))
            }
        }
    }
}

pub(super) async fn execute_interaction(
    db: &D1Database,
    principal: &access::Actor,
    rule_name: &str,
    interaction_id: &str,
    expected_version: i64,
    key: &str,
    request_id: &str,
) -> Result<RunSaved, ConnectError> {
    let tenant = &principal.tenant;
    let rule_id = rule_id(rule_name)?;
    crm_activities::interactions::validate_identifier(interaction_id)
        .map_err(|_| invalid("ACTIVITIES_AUTOMATION_SOURCE_ID_INVALID"))?;
    let source = source(db, tenant, interaction_id).await?;
    let request_digest = format!(
        "{:x}",
        Sha256::digest(
            serde_json::to_vec(&json!([
                "run_interaction_automation",
                rule_name,
                interaction_id,
                expected_version
            ]))
            .map_err(|_| storage())?
        )
    );
    verify_run_request(
        db,
        tenant,
        key,
        rule_id.as_str(),
        &source.event_id,
        &request_digest,
    )
    .await?;
    let mut existing = read_run(db, tenant, rule_id.as_str(), &source.event_id).await?;
    if existing.is_none() {
        let rule = get(db, tenant, rule_id.as_str())
            .await?
            .ok_or_else(|| ConnectError::not_found("ACTIVITIES_AUTOMATION_RULE_NOT_FOUND"))?;
        if rule.version != expected_version || rule.enabled != 1 {
            return Err(ConnectError::failed_precondition(
                "AUTOMATION_RULE_NOT_ENABLED_OR_STALE",
            ));
        }
        let domain = check(&wire_input(&rule))?;
        let original = activity_interactions::get(db, tenant, interaction_id)
            .await
            .map_err(|_| storage())?
            .ok_or_else(|| ConnectError::not_found("ACTIVITIES_AUTOMATION_SOURCE_NOT_FOUND"))?;
        let plan = domain
            .plan(AutomationSource::InteractionCreated(&original), now_nanos())
            .map_err(|error| ConnectError::invalid_argument(error.code))?
            .ok_or_else(|| {
                ConnectError::failed_precondition("ACTIVITIES_AUTOMATION_NOT_ELIGIBLE")
            })?;
        let input = activity_tasks::CreateInput {
            id: plan.task_id.as_str().to_owned(),
            subject: plan.subject,
            description: Some(format!(
                "Автоматически создано из взаимодействия {}",
                interaction_id
            )),
            account_id: plan.account_id,
            deal_id: plan.deal_id,
            assignee_email: Some(rule.assignee_email.clone()),
            owner_team_id: None,
            priority: activity_tasks::Priority::High,
            due_at_unix_nanos: plan.due_at_unix_nanos,
            reminder_at_unix_nanos: None,
        };
        let snapshot = serde_json::to_string(&input).map_err(|_| storage())?;
        let write = bind(
            db,
            concat!(
                "INSERT INTO crm_activity_automation_runs",
                "(tenant,rule_id,rule_version,source_event_id,source_type,source_id,source_version,interaction_id,task_id,",
                "task_intent,run_request_key,run_request_digest,status,",
                "requested_by,original_request_id,created_at_unix_millis)",
                " SELECT ?,r.id,r.version,s.event_id,'interaction_created',s.source_id,s.source_version,s.source_id,?,?,?,?,'pending',?,?,?",
                " FROM crm_activity_automation_rules r",
                " JOIN crm_activity_automation_source_events s ON s.tenant=r.tenant",
                " WHERE r.tenant=? AND r.id=? AND r.version=? AND r.enabled=1",
                " AND s.source_id=? AND s.event_type='crm.activities.interaction_created.v1'",
                " ON CONFLICT DO NOTHING"
            ),
            &[
                JsValue::from_str(tenant),
                JsValue::from_str(&input.id),
                JsValue::from_str(&snapshot),
                JsValue::from_str(key),
                JsValue::from_str(&request_digest),
                JsValue::from_str(&principal.identity),
                JsValue::from_str(request_id),
                JsValue::from_f64(js_sys::Date::now()),
                JsValue::from_str(tenant),
                JsValue::from_str(rule_id.as_str()),
                JsValue::from_f64(expected_version as f64),
                JsValue::from_str(interaction_id),
            ],
        )
        .map_err(|_| storage())?
        .run()
        .await;
        // The result of an unacknowledged claim is always resolved by the
        // tenant+rule+event UNIQUE row; no blind duplicate Task attempt.
        existing = read_run(db, tenant, rule_id.as_str(), &source.event_id).await?;
        if existing.is_none() {
            verify_run_request(
                db,
                tenant,
                key,
                rule_id.as_str(),
                &source.event_id,
                &request_digest,
            )
            .await?;
            return Err(match write {
                Ok(result) if result.success() => {
                    ConnectError::aborted("ACTIVITIES_AUTOMATION_RULE_STALE")
                }
                _ => ConnectError::unavailable("ACTIVITIES_AUTOMATION_CLAIM_UNCONFIRMED"),
            });
        }
    }
    let row = existing.ok_or_else(storage)?;
    if row.status == "pending" && expected_version != row.rule_version {
        return Err(ConnectError::failed_precondition(
            "ACTIVITIES_AUTOMATION_PINNED_RULE_VERSION_REQUIRED",
        ));
    }
    // A new key may replay the logical run, but must durably bind to
    // this exact request before any Task execution or success response.
    if !verify_run_request(
        db,
        tenant,
        key,
        &row.rule_id,
        &row.source_event_id,
        &request_digest,
    )
    .await?
    {
        let _write = bind(
            db,
            "INSERT INTO crm_activity_automation_run_requests(tenant,key,rule_id,source_event_id,digest) VALUES(?,?,?,?,?) ON CONFLICT(tenant,key) DO NOTHING",
            &[
                JsValue::from_str(tenant),
                JsValue::from_str(key),
                JsValue::from_str(&row.rule_id),
                JsValue::from_str(&row.source_event_id),
                JsValue::from_str(&request_digest),
            ],
        )
        .map_err(|_| storage())?
        .run()
        .await;
        if !verify_run_request(
            db,
            tenant,
            key,
            &row.rule_id,
            &row.source_event_id,
            &request_digest,
        )
        .await?
        {
            return Err(ConnectError::unavailable(
                "AUTOMATION_RUN_REQUEST_UNCONFIRMED",
            ));
        }
    }
    execute_claimed(
        db, tenant, row, &principal.identity, request_id, false, "RunInteractionAutomation"
    ).await
}


// One execution path owns both manual Interaction and autonomous sources.
// The only difference is the trusted actor supplied to the original Task owner.
async fn execute_claimed(
    db: &D1Database,
    tenant: &str,
    row: RunRow,
    actor: &str,
    request_id: &str,
    system_actor: bool,
    rpc_method: &str,
) -> Result<RunSaved, ConnectError> {
    match row.disposition()? {
        RunDisposition::Committed => {
            return Ok(RunSaved {
                original_request_id: row.task_request_id.clone(),
                run: row,
                replayed: true,
            });
        }
        RunDisposition::RuleSuperseded => {
            return Err(ConnectError::failed_precondition("AUTOMATION_RULE_DISABLED"));
        }
        RunDisposition::WaitingAssignee => {
            return Err(ConnectError::invalid_argument("ACTIVITIES_ASSIGNEE_NOT_ENROLLED"));
        }
        RunDisposition::Executing => {
            return Err(ConnectError::aborted("AUTOMATION_RUN_EXECUTING"));
        }
        RunDisposition::Backoff => {
            return Err(ConnectError::failed_precondition("AUTOMATION_RUN_BACKOFF"));
        }
        RunDisposition::AttemptsExhausted => {
            return Err(ConnectError::failed_precondition("AUTOMATION_RUN_ATTEMPTS_EXHAUSTED"));
        }
        RunDisposition::Cancelled => {
            return Err(ConnectError::failed_precondition("AUTOMATION_RUN_CANCELLED"));
        }
        RunDisposition::SourceStale => {
            return Err(ConnectError::failed_precondition("AUTOMATION_SOURCE_STALE"));
        }
        RunDisposition::RetryReady | RunDisposition::ReconcileReceipt => {}
    }
    let input: activity_tasks::CreateInput =
        serde_json::from_str(&row.task_intent).map_err(|_| storage())?;
    if input.id != row.task_id {
        return Err(ConnectError::unavailable("ACTIVITIES_AUTOMATION_INTENT_MISMATCH"));
    }
    let has_receipt = bind(
        db,
        "SELECT key FROM crm_activity_task_receipts WHERE tenant=? AND key=?",
        &[JsValue::from_str(tenant), JsValue::from_str(&row.task_id)],
    )
    .map_err(|_| storage())?
    .first::<serde_json::Value>(None)
    .await
    .map_err(|_| storage())?
    .is_some();
    if !has_receipt {
        if row.source_type == "task_overdue" {
            let current = bind(
                db,
                concat!(
                    "SELECT id FROM crm_activity_task_records WHERE tenant=? AND id=? AND version=?",
                    " AND json_extract(state,'$.status')='open'",
                    " AND json_type(state,'$.due_at_unix_nanos')='integer'",
                    " AND CAST(json_extract(state,'$.due_at_unix_nanos') AS INTEGER)<=?"
                ),
                &[
                    JsValue::from_str(tenant),
                    JsValue::from_str(&row.source_id),
                    JsValue::from_f64(row.source_version as f64),
                    JsValue::from_f64(now_nanos() as f64),
                ],
            )
            .map_err(|_| storage())?
            .first::<serde_json::Value>(None)
            .await
            .map_err(|_| storage())?
            .is_some();
            if !current {
                let _write = bind(
                    db,
                    concat!(
                        "UPDATE crm_activity_automation_runs SET source_stale=1",
                        " WHERE tenant=? AND rule_id=? AND source_event_id=?",
                        " AND status='pending' AND source_type='task_overdue'",
                        " AND source_stale=0 AND NOT EXISTS(",
                        " SELECT 1 FROM crm_activity_task_records t WHERE t.tenant=?",
                        " AND t.id=? AND t.version=? AND json_extract(t.state,'$.status')='open'",
                        " AND json_type(t.state,'$.due_at_unix_nanos')='integer'",
                        " AND CAST(json_extract(t.state,'$.due_at_unix_nanos') AS INTEGER)<=?)"
                    ),
                    &[
                        JsValue::from_str(tenant),
                        JsValue::from_str(&row.rule_id),
                        JsValue::from_str(&row.source_event_id),
                        JsValue::from_str(tenant),
                        JsValue::from_str(&row.source_id),
                        JsValue::from_f64(row.source_version as f64),
                        JsValue::from_f64(now_nanos() as f64),
                    ],
                )
                .map_err(|_| storage())?
                .run()
                .await;
                let stale = read_run(db, tenant, &row.rule_id, &row.source_event_id)
                    .await?.ok_or_else(storage)?;
                return Ok(RunSaved { run: stale, replayed: false, original_request_id: None });
            }
        }
        let current = get(db, tenant, &row.rule_id)
            .await?
            .ok_or_else(|| ConnectError::failed_precondition("AUTOMATION_RULE_DISABLED"))?;
        if current.enabled != 1 || current.version != row.rule_version {
            return Err(ConnectError::failed_precondition("AUTOMATION_RULE_DISABLED"));
        }
        acquire_attempt(db, tenant, &row, actor, request_id).await?;
    }
    let result = if system_actor {
        activity_tasks::create_automation_typed(
            input, &row.task_id, db, tenant, actor, request_id
        ).await
    } else {
        activity_tasks::create_typed(
            input, &row.task_id, db, tenant, actor, request_id
        ).await
    };
    let outcome = match result {
        Ok(Ok(outcome)) => outcome,
        other => {
            if !has_receipt {
                release_attempt(db, tenant, &row, request_id).await?;
            }
            return Err(match other {
                Ok(Err(error)) => error.connect(rpc_method, request_id),
                _ => storage(),
            });
        }
    };
    if outcome.task.task_id().as_str() != row.task_id {
        return Err(ConnectError::unavailable("AUTOMATION_TASK_RECEIPT_MISMATCH"));
    }
    let task_trace = outcome.original_request_id.clone();
    let done = bind(
        db,
        concat!(
            "UPDATE crm_activity_automation_runs SET status='committed',",
            "task_request_id=?,completed_at_unix_millis=?,lease_until_unix_millis=0,",
            "next_attempt_at_unix_millis=0 WHERE tenant=?",
            " AND rule_id=? AND source_event_id=? AND status='pending'",
            " AND cancel_request_id IS NULL AND source_stale=0"
        ),
        &[
            task_trace.as_deref().map(JsValue::from_str).unwrap_or(JsValue::NULL),
            JsValue::from_f64(js_sys::Date::now()),
            JsValue::from_str(tenant),
            JsValue::from_str(&row.rule_id),
            JsValue::from_str(&row.source_event_id),
        ],
    )
    .map_err(|_| storage())?
    .run()
    .await;
    let completed = read_run(db, tenant, &row.rule_id, &row.source_event_id)
        .await?.ok_or_else(storage)?;
    if completed.status != "committed" {
        return Err(match done {
            Ok(_) => ConnectError::unavailable("AUTOMATION_RUN_RECEIPT_NOT_CONFIRMED"),
            Err(_) => ConnectError::unavailable("AUTOMATION_RUN_WRITE_UNCONFIRMED"),
        });
    }
    Ok(RunSaved { run: completed, replayed: outcome.replayed, original_request_id: task_trace })
}


#[derive(Deserialize)]
struct TenantRef {
    id: String,
}
#[derive(Deserialize)]
struct ScheduledRunRef {
    rule_id: String,
    source_event_id: String,
}
#[derive(Deserialize)]
struct ScheduledDealSource {
    rule_id: String,
    rule_version: i64,
    source_id: String,
    source_version: i64,
    account_id: String,
}
#[derive(Deserialize)]
struct ScheduledTaskSource {
    rule_id: String,
    rule_version: i64,
    source_id: String,
    source_version: i64,
    state: String,
}
#[derive(Default, Debug)]
pub(super) struct ScheduledSummary {
    pub recovered: u32,
    pub deals_created: u32,
    pub tasks_overdue: u32,
    pub failed: u32,
}

// A scheduled invocation has no caller JWT. Instead it first reads only the
// tenant directory, then creates one private system principal per tenant.
// Every business-data query below is bound to that single tenant. This is
// deliberately distinct from a user/Access Actor and cannot be supplied by RPC.
struct AutomationSystemActor {
    tenant: String,
}
impl AutomationSystemActor {
    fn for_tenant(tenant: String) -> Self {
        Self { tenant }
    }
    fn tenant(&self) -> &str {
        &self.tenant
    }
    fn identity(&self) -> &'static str {
        AUTOMATION_ACTOR
    }
}

fn scheduled_event_id(
    tenant: &str,
    rule_id: &str,
    rule_version: i64,
    source_type: &str,
    source_id: &str,
) -> Result<String, ConnectError> {
    let identity = serde_json::to_vec(&(tenant, rule_id, rule_version, source_type, source_id))
        .map_err(|_| storage())?;
    Ok(format!("crm.activities.a1:{:x}", Sha256::digest(identity)))
}

async fn claim_scheduled_run(
    db: &D1Database,
    principal: &AutomationSystemActor,
    rule: &RuleRow,
    source_type: &str,
    source_id: &str,
    source_version: i64,
    input: activity_tasks::CreateInput,
) -> Result<Option<RunRow>, ConnectError> {
    let tenant = principal.tenant();
    let event_id = scheduled_event_id(
        tenant, &rule.rule_id, rule.version, source_type, source_id
    )?;
    let snapshot = serde_json::to_string(&input).map_err(|_| storage())?;
    let request_key_digest = format!("{:x}", Sha256::digest(event_id.as_bytes()));
    let request_key = format!("automation-scheduled-{}", &request_key_digest[..48]);
    let request_digest = format!("{:x}", Sha256::digest(
        serde_json::to_vec(&(rule.rule_id.as_str(), rule.version, source_type,
            source_id, source_version, snapshot.as_str())).map_err(|_| storage())?
    ));
    let original_request_id = super::native_request_id().map_err(|_| storage())?;
    let write = bind(
        db,
        concat!(
            "INSERT INTO crm_activity_automation_runs",
            "(tenant,rule_id,rule_version,source_event_id,source_type,source_id,source_version,",
            "interaction_id,task_id,task_intent,run_request_key,run_request_digest,status,",
            "requested_by,original_request_id,created_at_unix_millis)",
            " SELECT ?,r.id,r.version,?,?,?,?,NULL,?,?,?,?,'pending',?,?,?",
            " FROM crm_activity_automation_rules r WHERE r.tenant=? AND r.id=?",
            " AND r.version=? AND r.trigger_kind=? AND r.enabled=1 ON CONFLICT DO NOTHING"
        ),
        &[
            JsValue::from_str(tenant),
            JsValue::from_str(&event_id),
            JsValue::from_str(source_type),
            JsValue::from_str(source_id),
            JsValue::from_f64(source_version as f64),
            JsValue::from_str(&input.id),
            JsValue::from_str(&snapshot),
            JsValue::from_str(&request_key),
            JsValue::from_str(&request_digest),
            JsValue::from_str(principal.identity()),
            JsValue::from_str(&original_request_id),
            JsValue::from_f64(js_sys::Date::now()),
            JsValue::from_str(tenant),
            JsValue::from_str(&rule.rule_id),
            JsValue::from_f64(rule.version as f64),
            JsValue::from_str(source_type),
        ],
    )
    .map_err(|_| storage())?
    .run()
    .await;
    let row = read_run(db, tenant, &rule.rule_id, &event_id).await?;
    if row.is_none() {
        return match write {
            Ok(result) if result.success() => Ok(None),
            _ => Err(ConnectError::unavailable("AUTOMATION_SCHEDULED_CLAIM_UNCONFIRMED")),
        };
    }
    let row = row.ok_or_else(storage)?;
    if row.source_type != source_type || row.source_id != source_id {
        return Err(ConnectError::unavailable("AUTOMATION_SCHEDULED_SOURCE_IDENTITY_MISMATCH"));
    }
    Ok(Some(row))
}

pub(super) async fn dispatch_scheduled(
    db: &D1Database,
    now: i64,
) -> Result<ScheduledSummary, ConnectError> {
    let tenants = bind(
        db,
        "SELECT id FROM crm_tenants ORDER BY id",
        &[],
    )
    .map_err(|_| storage())?
    .all().await.map_err(|_| storage())?
    .results::<TenantRef>().map_err(|_| storage())?;
    let mut total = ScheduledSummary::default();
    for tenant in tenants {
        let principal = AutomationSystemActor::for_tenant(tenant.id);
        match dispatch_tenant(db, &principal, now).await {
            Ok(summary) => {
                total.recovered += summary.recovered;
                total.deals_created += summary.deals_created;
                total.tasks_overdue += summary.tasks_overdue;
                total.failed += summary.failed;
            }
            Err(_) => total.failed += 1,
        }
    }
    Ok(total)
}

async fn dispatch_tenant(
    db: &D1Database,
    principal: &AutomationSystemActor,
    now: i64,
) -> Result<ScheduledSummary, ConnectError> {
    let tenant = principal.tenant();
    let mut summary = ScheduledSummary::default();
    // Recover durable pending runs before discovering new sources. A Task
    // receipt takes priority over lease expiry so a lost acknowledgement is
    // reconciled immediately without incrementing attempts or making a Task.
    let pending = bind(
        db,
        concat!(
            "SELECT rule_id,source_event_id FROM crm_activity_automation_runs",
            " WHERE tenant=? AND source_type IN ('deal_created','task_overdue')",
            " AND status='pending' AND source_stale=0 AND cancel_request_id IS NULL AND (",
            " EXISTS(SELECT 1 FROM crm_activity_task_receipts t",
            "  WHERE t.tenant=crm_activity_automation_runs.tenant",
            "   AND t.key=crm_activity_automation_runs.task_id)",
            " OR (lease_until_unix_millis<=unixepoch('now')*1000",
            " AND next_attempt_at_unix_millis<=unixepoch('now')*1000))",
            " ORDER BY seq LIMIT ?"
        ),
        &[JsValue::from_str(tenant),JsValue::from_f64(MAX_SCHEDULED_BATCH as f64)],
    )
    .map_err(|_| storage())?
    .all().await.map_err(|_| storage())?
    .results::<ScheduledRunRef>().map_err(|_| storage())?;
    for pending_run in pending {
        let Some(row) = read_run(db, tenant, &pending_run.rule_id,
            &pending_run.source_event_id).await? else { continue; };
        let request_id = super::native_request_id().map_err(|_| storage())?;
        match execute_claimed(db, tenant, row, principal.identity(),
            &request_id, true, "ScheduledAutomation").await {
            Ok(_) => summary.recovered += 1,
            Err(_) => summary.failed += 1,
        }
    }

    // Deal lifecycle events are immutable and transactionally coupled to the
    // original Deal create. The rule publication boundary prevents backfill.
    let deals = bind(
        db,
        concat!(
            "SELECT r.id AS rule_id,r.version AS rule_version,",
            " e.deal_id AS source_id,e.deal_version AS source_version,d.account_id",
            " FROM crm_activity_automation_rules r",
            " JOIN crm_sales_deal_events e ON e.tenant=r.tenant",
            " JOIN crm_sales_deal_records d ON d.tenant=e.tenant AND d.id=e.deal_id",
            " WHERE r.tenant=? AND r.trigger_kind='deal_created' AND r.enabled=1",
            " AND e.action='created' AND e.occurred_at_unix_nanos>=r.published_at_unix_nanos",
            " AND NOT EXISTS(SELECT 1 FROM crm_activity_automation_runs a",
            "  WHERE a.tenant=r.tenant AND a.rule_id=r.id AND a.source_type='deal_created'",
            "   AND a.source_id=e.deal_id AND a.rule_version=r.version)",
            " ORDER BY e.occurred_at_unix_nanos,e.deal_id LIMIT ?"
        ),
        &[JsValue::from_str(tenant),JsValue::from_f64(MAX_SCHEDULED_BATCH as f64)],
    )
    .map_err(|_| storage())?
    .all().await.map_err(|_| storage())?
    .results::<ScheduledDealSource>().map_err(|_| storage())?;
    for candidate in deals {
        let Some(rule) = get(db, tenant, &candidate.rule_id).await? else { continue; };
        if rule.version != candidate.rule_version || rule.trigger != "deal_created" || rule.enabled != 1 {
            continue;
        }
        let domain = check(&wire_input(&rule))?;
        let plan = domain.plan(AutomationSource::DealCreated {
            deal_id: &candidate.source_id,
            account_id: &candidate.account_id,
            source_version: candidate.source_version,
        }, now).map_err(|_| invalid("ACTIVITIES_AUTOMATION_SOURCE_INVALID"))?;
        let Some(plan) = plan else { continue; };
        let input = activity_tasks::CreateInput {
            id: plan.task_id.as_str().to_owned(),
            subject: plan.subject,
            description: Some(format!("Автоматически создано из сделки {}", candidate.source_id)),
            account_id: plan.account_id,
            deal_id: plan.deal_id,
            assignee_email: Some(rule.assignee_email.clone()),
            owner_team_id: None,
            priority: activity_tasks::Priority::High,
            due_at_unix_nanos: plan.due_at_unix_nanos,
            reminder_at_unix_nanos: None,
        };
        if let Some(row) = claim_scheduled_run(db, principal, &rule, "deal_created",
            &candidate.source_id, candidate.source_version, input).await? {
            let request_id = super::native_request_id().map_err(|_| storage())?;
            match execute_claimed(db, tenant, row, principal.identity(),
                &request_id, true, "ScheduledAutomation").await {
                Ok(_) => summary.deals_created += 1,
                Err(_) => summary.failed += 1,
            }
        }
    }

    // A Task is a due timer only while the exact current version remains open
    // and overdue. Generated Tasks are excluded by their original run receipt.
    let overdue = bind(
        db,
        concat!(
            "SELECT r.id AS rule_id,r.version AS rule_version,",
            " t.id AS source_id,t.version AS source_version,t.state",
            " FROM crm_activity_automation_rules r",
            " JOIN crm_activity_task_records t ON t.tenant=r.tenant",
            " WHERE r.tenant=? AND r.trigger_kind='task_overdue' AND r.enabled=1",
            " AND json_extract(t.state,'$.status')='open'",
            " AND json_type(t.state,'$.due_at_unix_nanos')='integer'",
            " AND CAST(json_extract(t.state,'$.due_at_unix_nanos') AS INTEGER)<=?",
            " AND NOT EXISTS(SELECT 1 FROM crm_activity_automation_runs made",
            "  WHERE made.tenant=t.tenant AND made.task_id=t.id)",
            " AND NOT EXISTS(SELECT 1 FROM crm_activity_automation_runs a",
            "  WHERE a.tenant=r.tenant AND a.rule_id=r.id AND a.source_type='task_overdue'",
            "   AND a.source_id=t.id AND a.rule_version=r.version)",
            " ORDER BY CAST(json_extract(t.state,'$.due_at_unix_nanos') AS INTEGER),t.id LIMIT ?"
        ),
        &[
            JsValue::from_str(tenant),
            JsValue::from_f64(now as f64),
            JsValue::from_f64(MAX_SCHEDULED_BATCH as f64),
        ],
    )
    .map_err(|_| storage())?
    .all().await.map_err(|_| storage())?
    .results::<ScheduledTaskSource>().map_err(|_| storage())?;
    for candidate in overdue {
        let Some(rule) = get(db, tenant, &candidate.rule_id).await? else { continue; };
        if rule.version != candidate.rule_version || rule.trigger != "task_overdue" || rule.enabled != 1 {
            continue;
        }
        let task = activity_tasks::decode(&candidate.state).map_err(|_| storage())?;
        let domain = check(&wire_input(&rule))?;
        let plan = domain.plan(AutomationSource::TaskOverdue {
            task: &task,
            generated_by_automation: false,
        }, now).map_err(|_| invalid("ACTIVITIES_AUTOMATION_SOURCE_INVALID"))?;
        let Some(plan) = plan else { continue; };
        let input = activity_tasks::CreateInput {
            id: plan.task_id.as_str().to_owned(),
            subject: plan.subject,
            description: Some(format!("Автоматически создано из просроченной задачи {}", candidate.source_id)),
            account_id: plan.account_id,
            deal_id: plan.deal_id,
            assignee_email: Some(rule.assignee_email.clone()),
            owner_team_id: None,
            priority: activity_tasks::Priority::High,
            due_at_unix_nanos: plan.due_at_unix_nanos,
            reminder_at_unix_nanos: None,
        };
        if let Some(row) = claim_scheduled_run(db, principal, &rule, "task_overdue",
            &candidate.source_id, candidate.source_version, input).await? {
            let request_id = super::native_request_id().map_err(|_| storage())?;
            match execute_claimed(db, tenant, row, principal.identity(),
                &request_id, true, "ScheduledAutomation").await {
                Ok(_) => summary.tasks_overdue += 1,
                Err(_) => summary.failed += 1,
            }
        }
    }
    Ok(summary)
}

// The D1 clock and exact request id fence the original Task INSERT.// The D1 clock and exact request id fence the original Task INSERT. A
// crashed worker leaves a bounded lease; a later attempt cannot be written
// by the expired worker even if its asynchronous D1 call resumes.
async fn acquire_attempt(
    db: &D1Database,
    tenant: &str,
    row: &RunRow,
    actor: &str,
    request_id: &str,
) -> Result<(), ConnectError> {
    let _write = bind(
        db,
        concat!(
            "UPDATE crm_activity_automation_runs SET attempt_count=attempt_count+1,",
            "lease_request_id=?,lease_actor=?,lease_until_unix_millis=unixepoch('now')*1000+60000,",
            "next_attempt_at_unix_millis=unixepoch('now')*1000+60000+(5000<<attempt_count)",
            " WHERE tenant=? AND rule_id=? AND source_event_id=? AND status='pending'",
            " AND cancel_request_id IS NULL AND attempt_count<?",
            " AND lease_until_unix_millis<=unixepoch('now')*1000",
            " AND next_attempt_at_unix_millis<=unixepoch('now')*1000 AND source_stale=0",
            " AND NOT EXISTS(SELECT 1 FROM crm_activity_task_receipts t",
            " WHERE t.tenant=crm_activity_automation_runs.tenant",
            " AND t.key=crm_activity_automation_runs.task_id)"
        ),
        &[
            JsValue::from_str(request_id),
            JsValue::from_str(actor),
            JsValue::from_str(tenant),
            JsValue::from_str(&row.rule_id),
            JsValue::from_str(&row.source_event_id),
            JsValue::from_f64(MAX_ATTEMPTS as f64),
        ],
    )
    .map_err(|_| storage())?
    .run()
    .await;
    let fresh = read_run(db, tenant, &row.rule_id, &row.source_event_id)
        .await?
        .ok_or_else(storage)?;
    if fresh.lease_request_id.as_deref() == Some(request_id)
        && fresh.lease_active == 1
        && fresh.cancel_request_id.is_none()
    {
        Ok(())
    } else {
        Err(ConnectError::aborted("AUTOMATION_ATTEMPT_NOT_ACQUIRED"))
    }
}

async fn release_attempt(
    db: &D1Database,
    tenant: &str,
    row: &RunRow,
    request_id: &str,
) -> Result<(), ConnectError> {
    // A failed release leaves the original expiration + backoff intact.
    // Never undo another attempt or mark a receipt-bearing run as failed.
    let _write = bind(
        db,
        concat!(
            "UPDATE crm_activity_automation_runs SET lease_until_unix_millis=0,",
            "next_attempt_at_unix_millis=unixepoch('now')*1000+(5000<<(attempt_count-1))",
            " WHERE tenant=? AND rule_id=? AND source_event_id=? AND status='pending'",
            " AND lease_request_id=? AND cancel_request_id IS NULL",
            " AND NOT EXISTS(SELECT 1 FROM crm_activity_task_receipts t",
            " WHERE t.tenant=crm_activity_automation_runs.tenant",
            " AND t.key=crm_activity_automation_runs.task_id)"
        ),
        &[
            JsValue::from_str(tenant),
            JsValue::from_str(&row.rule_id),
            JsValue::from_str(&row.source_event_id),
            JsValue::from_str(request_id),
        ],
    )
    .map_err(|_| storage())?
    .run()
    .await;
    Ok(())
}

pub(super) async fn cancel(
    db: &D1Database,
    principal: &access::Actor,
    rule: &str,
    source_type: &str,
    source_id: &str,
    expected_version: i64,
    key: &str,
    request_id: &str,
) -> Result<RunSaved, ConnectError> {
    let tenant = &principal.tenant;
    let row = get_run(db, tenant, rule, source_type, source_id, expected_version)
        .await?
        .ok_or_else(|| ConnectError::not_found("ACTIVITIES_AUTOMATION_RUN_NOT_FOUND"))?;
    // Preserve the historical Interaction idempotency digest exactly;
    // new autonomous source kinds bind both their type and source id.
    let identity = if source_type == "interaction_created" {
        json!(["cancel_automation_run", rule, source_id, expected_version])
    } else {
        json!(["cancel_automation_run", rule, source_type, source_id, expected_version])
    };
    let digest = format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&identity).map_err(|_| storage())?)
    );
    let replayed = verify_run_request(db, tenant, key, rule, &row.source_event_id, &digest).await?;
    if replayed && row.cancel_key.as_deref() == Some(key) {
        return Ok(RunSaved {
            original_request_id: row.cancel_request_id.clone(),
            run: row,
            replayed: true,
        });
    }
    if row.rule_version != expected_version || !row.can_cancel() {
        return Err(ConnectError::failed_precondition(
            "AUTOMATION_CANCEL_NOT_ALLOWED",
        ));
    }
    let _write = bind(
        db,
        concat!(
            "UPDATE crm_activity_automation_runs SET cancel_key=?,cancel_digest=?,",
            "cancel_request_id=?,cancelled_by=?,cancelled_at_unix_millis=unixepoch('now')*1000,",
            "lease_until_unix_millis=0,next_attempt_at_unix_millis=0",
            " WHERE tenant=? AND rule_id=? AND source_event_id=? AND status='pending'",
            " AND rule_version=? AND cancel_request_id IS NULL",
            " AND NOT EXISTS(SELECT 1 FROM crm_activity_task_records t",
            " WHERE t.tenant=crm_activity_automation_runs.tenant",
            " AND t.id=crm_activity_automation_runs.task_id)",
            " AND NOT EXISTS(SELECT 1 FROM crm_activity_task_receipts t",
            " WHERE t.tenant=crm_activity_automation_runs.tenant",
            " AND t.key=crm_activity_automation_runs.task_id)"
        ),
        &[
            JsValue::from_str(key),
            JsValue::from_str(&digest),
            JsValue::from_str(request_id),
            JsValue::from_str(&principal.identity),
            JsValue::from_str(tenant),
            JsValue::from_str(rule),
            JsValue::from_str(&row.source_event_id),
            JsValue::from_f64(expected_version as f64),
        ],
    )
    .map_err(|_| storage())?
    .run()
    .await;
    let fresh = read_run(db, tenant, rule, &row.source_event_id)
        .await?
        .ok_or_else(storage)?;
    let bound = verify_run_request(db, tenant, key, rule, &row.source_event_id, &digest).await?;
    if bound && fresh.cancel_key.as_deref() == Some(key) {
        let original = fresh.cancel_request_id.clone().ok_or_else(storage)?;
        return Ok(RunSaved {
            replayed: original != request_id,
            original_request_id: Some(original),
            run: fresh,
        });
    }
    if !fresh.can_cancel() {
        return Err(ConnectError::failed_precondition(
            "AUTOMATION_CANCEL_NOT_ALLOWED",
        ));
    }
    Err(ConnectError::unavailable("AUTOMATION_CANCEL_UNCONFIRMED"))
}

// Read-only recovery: a matching caller key and the original, atomic Task
// receipt are both required. A pending run is not a committed action.
pub(super) async fn get_run(
    db: &D1Database,
    tenant: &str,
    rule: &str,
    source_type: &str,
    source_id: &str,
    expected_version: i64,
) -> Result<Option<RunRow>, ConnectError> {
    rule_id(rule)?;
    if !matches!(source_type, "interaction_created" | "deal_created" | "task_overdue") {
        return Err(invalid("ACTIVITIES_AUTOMATION_SOURCE_TYPE_INVALID"));
    }
    RecordId::try_new(source_id.to_owned())
        .map_err(|_| invalid("ACTIVITIES_AUTOMATION_SOURCE_ID_INVALID"))?;
    let Some(event) = bind(
        db,
        "SELECT source_event_id AS event_id FROM crm_activity_automation_runs WHERE tenant=? AND rule_id=? AND source_type=? AND source_id=? AND (?=0 OR rule_version=?) ORDER BY rule_version DESC LIMIT 1",
        &[JsValue::from_str(tenant), JsValue::from_str(rule),
          JsValue::from_str(source_type), JsValue::from_str(source_id),
          JsValue::from_f64(expected_version as f64),JsValue::from_f64(expected_version as f64)],
    )
    .map_err(|_| storage())?
    .first::<SourceRow>(None)
    .await
    .map_err(|_| storage())?
    else {
        return Ok(None);
    };
    read_run(db, tenant, rule, &event.event_id).await
}
pub(super) async fn confirmed_request(
    db: &D1Database,
    tenant: &str,
    run: &RunRow,
    key: &str,
) -> Result<Option<String>, ConnectError> {
    if run.cancel_key.as_deref() == Some(key) && run.cancel_request_id.is_some() {
        let binding = run_request(db, tenant, key).await?;
        return Ok(binding
            .filter(|b| b.rule_id == run.rule_id && b.source_event_id == run.source_event_id)
            .and(run.cancel_request_id.clone()));
    }
    if run.status != "committed" {
        return Ok(None);
    }
    let Some(binding) = run_request(db, tenant, key).await? else {
        return Ok(None);
    };
    if binding.rule_id != run.rule_id || binding.source_event_id != run.source_event_id {
        return Ok(None);
    }
    let original = bind(
        db,
        concat!(
            "SELECT original_request_id FROM crm_activity_task_receipts",
            " WHERE tenant=? AND key=?"
        ),
        &[JsValue::from_str(tenant), JsValue::from_str(&run.task_id)],
    )
    .map_err(|_| storage())?
    .first::<serde_json::Value>(None)
    .await
    .map_err(|_| storage())?;
    Ok(run.task_request_id.clone().filter(|request| {
        original
            .as_ref()
            .and_then(|v| v.get("original_request_id"))
            .and_then(|v| v.as_str())
            == Some(request.as_str())
    }))
}
