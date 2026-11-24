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

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(super) struct RuleRow {
    pub rule_id: String,
    pub version: i64,
    pub trigger: String,
    pub enabled: i64,
    pub task_subject: String,
    pub assignee_email: String,
    pub due_after_seconds: i64,
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
}
#[derive(Clone, Deserialize)]
pub(super) struct RunRow {
    pub seq: i64,
    pub rule_id: String,
    pub rule_version: i64,
    pub source_event_id: String,
    pub interaction_id: String,
    pub task_id: String,
    pub task_intent: String,
    pub run_request_key: String,
    pub run_request_digest: String,
    pub status: String,
    pub requested_by: String,
    pub original_request_id: String,
    pub task_request_id: Option<String>,
    pub created_at_unix_millis: i64,
    pub completed_at_unix_millis: Option<i64>,
}
pub(super) struct RunSaved {
    pub run: RunRow,
    pub replayed: bool,
    pub task_original_request_id: Option<String>,
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
    if input.trigger != "interaction_created"
        || input.due_after_seconds < 0
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
        trigger: AutomationTrigger::InteractionCreated,
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
            "SELECT id AS rule_id,version,trigger,enabled,task_subject,",
            "assignee_email,due_after_seconds FROM crm_activity_automation_rules",
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
            "(tenant,id,version,trigger,enabled,task_subject,assignee_email,",
            "due_after_seconds,idem_key,idem_digest,request_id,actor)",
            " VALUES(?,?,1,?,?,?,?,?,?,?,?,?) ON CONFLICT(tenant,id) DO NOTHING"
        )
    } else {
        concat!(
            "UPDATE crm_activity_automation_rules SET version=version+1,",
            "trigger=?,enabled=?,task_subject=?,assignee_email=?,",
            "due_after_seconds=?,idem_key=?,idem_digest=?,request_id=?,actor=?",
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
            "SELECT id AS rule_id,version,trigger,enabled,task_subject,",
            "assignee_email,due_after_seconds FROM crm_activity_automation_rules",
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
            "SELECT event_id FROM crm_activity_automation_source_events",
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
async fn read_run(
    db: &D1Database,
    tenant: &str,
    rule_id: &str,
    event_id: &str,
) -> Result<Option<RunRow>, ConnectError> {
    bind(db,
      concat!("SELECT seq,rule_id,rule_version,source_event_id,interaction_id,task_id,",
             "task_intent,run_request_key,run_request_digest,status,requested_by,original_request_id,task_request_id,",
             "created_at_unix_millis,completed_at_unix_millis FROM crm_activity_automation_runs",
             " WHERE tenant=? AND rule_id=? AND source_event_id=?"),
      &[JsValue::from_str(tenant),JsValue::from_str(rule_id),JsValue::from_str(event_id)]
    ).map_err(|_|storage())?.first::<RunRow>(None).await.map_err(|_|storage())
}
pub(super) async fn list_runs(
    db: &D1Database,
    tenant: &str,
    rule_id: &str,
    after: i64,
    limit: u32,
) -> Result<Vec<RunRow>, ConnectError> {
    bind(db,
      concat!("SELECT seq,rule_id,rule_version,source_event_id,interaction_id,task_id,",
             "task_intent,run_request_key,run_request_digest,status,requested_by,original_request_id,task_request_id,",
             "created_at_unix_millis,completed_at_unix_millis FROM crm_activity_automation_runs",
             " WHERE tenant=? AND (?='' OR rule_id=?) AND seq>? ORDER BY seq LIMIT ?"),
      &[JsValue::from_str(tenant),JsValue::from_str(rule_id),JsValue::from_str(rule_id),
        JsValue::from_f64(after as f64),JsValue::from_f64(limit as f64)]
    ).map_err(|_|storage())?.all().await.map_err(|_|storage())?
       .results::<RunRow>().map_err(|_|storage())
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
                "(tenant,rule_id,rule_version,source_event_id,interaction_id,task_id,",
                "task_intent,run_request_key,run_request_digest,status,",
                "requested_by,original_request_id,created_at_unix_millis)",
                " SELECT ?,r.id,r.version,s.event_id,s.source_id,?,?,?,'pending',?,?,?",
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
            return Err(match write {
                Ok(result) if result.success() => {
                    ConnectError::aborted("ACTIVITIES_AUTOMATION_RULE_STALE")
                }
                _ => ConnectError::unavailable("ACTIVITIES_AUTOMATION_CLAIM_UNCONFIRMED"),
            });
        }
    }
    let row = existing.ok_or_else(storage)?;
    if row.run_request_key == key && row.run_request_digest != request_digest {
        return Err(ConnectError::aborted("IDEMPOTENCY_CONFLICT"));
    }
    if row.status == "committed" {
        return Ok(RunSaved {
            task_original_request_id: row.task_request_id.clone(),
            run: row,
            replayed: true,
        });
    }
    if row.status != "pending" {
        return Err(storage());
    }
    let input: activity_tasks::CreateInput =
        serde_json::from_str(&row.task_intent).map_err(|_| storage())?;
    if input.id != row.task_id {
        return Err(ConnectError::unavailable(
            "ACTIVITIES_AUTOMATION_INTENT_MISMATCH",
        ));
    }
    // A disabled/revoked rule cannot trigger a NEW task. A proven original
    // receipt may still be reconciled after disable/revocation.
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
        let current = get(db, tenant, rule_id.as_str())
            .await?
            .ok_or_else(|| ConnectError::failed_precondition("AUTOMATION_RULE_DISABLED"))?;
        if current.enabled != 1 || current.version != row.rule_version {
            return Err(ConnectError::failed_precondition(
                "AUTOMATION_RULE_DISABLED",
            ));
        }
    }
    let outcome = activity_tasks::create_typed(
        input,
        &row.task_id,
        db,
        tenant,
        &principal.identity,
        request_id,
    )
    .await
    .map_err(|_| storage())?
    .map_err(|error| error.connect("RunInteractionAutomation", request_id))?;
    if outcome.task.task_id().as_str() != row.task_id {
        return Err(ConnectError::unavailable(
            "AUTOMATION_TASK_RECEIPT_MISMATCH",
        ));
    }
    let task_trace = outcome.original_request_id.clone();
    let done = bind(
        db,
        concat!(
            "UPDATE crm_activity_automation_runs SET status='committed',",
            "task_request_id=?,completed_at_unix_millis=? WHERE tenant=?",
            " AND rule_id=? AND source_event_id=? AND status='pending'"
        ),
        &[
            task_trace
                .as_deref()
                .map(JsValue::from_str)
                .unwrap_or(JsValue::NULL),
            JsValue::from_f64(js_sys::Date::now()),
            JsValue::from_str(tenant),
            JsValue::from_str(rule_id.as_str()),
            JsValue::from_str(&source.event_id),
        ],
    )
    .map_err(|_| storage())?
    .run()
    .await;
    let completed = read_run(db, tenant, rule_id.as_str(), &source.event_id)
        .await?
        .ok_or_else(storage)?;
    if completed.status != "committed" {
        return Err(match done {
            Ok(_) => ConnectError::unavailable("AUTOMATION_RUN_RECEIPT_NOT_CONFIRMED"),
            Err(_) => ConnectError::unavailable("AUTOMATION_RUN_WRITE_UNCONFIRMED"),
        });
    }
    Ok(RunSaved {
        run: completed,
        replayed: outcome.replayed,
        task_original_request_id: task_trace,
    })
}

// Read-only recovery: a matching caller key and the original, atomic Task
// receipt are both required. A pending run is not a committed action.
pub(super) async fn get_run(
    db: &D1Database,
    tenant: &str,
    rule: &str,
    interaction: &str,
) -> Result<Option<RunRow>, ConnectError> {
    rule_id(rule)?;
    crm_activities::interactions::validate_identifier(interaction)
        .map_err(|_| invalid("ACTIVITIES_AUTOMATION_SOURCE_ID_INVALID"))?;
    let Some(event) = bind(
        db,
        "SELECT event_id FROM crm_activity_automation_source_events WHERE tenant=? AND source_id=?",
        &[JsValue::from_str(tenant), JsValue::from_str(interaction)],
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
pub(super) async fn confirmed(
    db: &D1Database,
    tenant: &str,
    run: &RunRow,
    key: &str,
) -> Result<bool, ConnectError> {
    if run.status != "committed" || run.run_request_key != key {
        return Ok(false);
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
    Ok(original
        .as_ref()
        .and_then(|v| v.get("original_request_id"))
        .and_then(|v| v.as_str())
        == run.task_request_id.as_deref()
        && run.task_request_id.is_some())
}
