//! Teams belong to the same Access authority as crm_users.
use super::{access, access_members, bind, d1_write};
use connectrpc::ErrorCode;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use wasm_bindgen::JsValue;
use worker::{D1Database, Result};

type Failure = d1_write::OwnerFailure;
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct Team {
    pub team_id: String,
    pub name: String,
    pub description: String,
    pub active: bool,
    pub member_emails: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub queue_coordinators: Vec<QueueCoordinator>,
    pub version: i64,
}
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct QueueCoordinator {
    pub email: String,
    pub member_version: i64,
}

#[derive(Serialize)]
pub(super) struct QueueCoordinatorChange {
    pub email: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct DistributionScope {
    pub team_id: String,
    pub team_version: i64,
}

pub(super) struct DistributionOptions {
    pub scope: DistributionScope,
    pub eligible_member_emails: Vec<String>,
}

// Shared Access predicate for both original owners; no second ACL store.
const COORDINATOR_JOIN: &str = " FROM crm_access_teams tm JOIN json_each(tm.state,'$.queue_coordinators') g JOIN crm_users u ON u.tenant_id=tm.tenant AND u.email=json_extract(g.value,'$.email') AND u.active=1 AND u.version=CAST(json_extract(g.value,'$.member_version') AS INTEGER) JOIN json_each(tm.state,'$.member_emails') r JOIN crm_users target ON target.tenant_id=tm.tenant AND target.email=r.value AND target.active=1 WHERE tm.tenant=? AND tm.id=? AND json_extract(tm.state,'$.active')=1 AND u.email=? AND u.email NOT LIKE 'service:%' AND EXISTS (SELECT 1 FROM json_each(tm.state,'$.member_emails') leader WHERE leader.value=u.email) AND target.email NOT LIKE 'service:%'";

#[derive(Deserialize)]
struct DistributionRow {
    version: String,
    email: String,
}

pub(super) async fn distribution_options(
    db: &D1Database,
    tenant: &str,
    team: &str,
    actor: &str,
) -> Result<Option<DistributionOptions>> {
    let sql = format!(
        "SELECT CAST(tm.version AS TEXT) version,target.email{COORDINATOR_JOIN} ORDER BY target.email"
    );
    let rows = bind(
        db,
        &sql,
        &[
            JsValue::from_str(tenant),
            JsValue::from_str(team),
            JsValue::from_str(actor),
        ],
    )?
    .all()
    .await?
    .results::<DistributionRow>()?;
    let Some(first) = rows.first() else {
        return Ok(None);
    };
    let team_version = first
        .version
        .parse::<i64>()
        .map_err(|_| worker::Error::RustError("invalid Team version".into()))?;
    Ok(Some(DistributionOptions {
        scope: DistributionScope {
            team_id: team.to_owned(),
            team_version,
        },
        eligible_member_emails: rows.into_iter().map(|row| row.email).collect(),
    }))
}

pub(super) async fn may_distribute(
    db: &D1Database,
    tenant: &str,
    actor: &str,
    scope: &DistributionScope,
    target: &str,
) -> Result<bool> {
    let sql =
        format!("SELECT 1{COORDINATOR_JOIN} AND tm.version=CAST(? AS INTEGER) AND target.email=?");
    Ok(bind(
        db,
        &sql,
        &[
            JsValue::from_str(tenant),
            JsValue::from_str(&scope.team_id),
            JsValue::from_str(actor),
            JsValue::from_str(&scope.team_version.to_string()),
            JsValue::from_str(target),
        ],
    )?
    .first::<serde_json::Value>(None)
    .await?
    .is_some())
}

pub(super) enum DistributionOwner {
    Task,
    Deal,
}

pub(super) fn append_distribution_guard(
    sql: &mut String,
    values: &mut Vec<JsValue>,
    owner: DistributionOwner,
    tenant: &str,
    actor: &str,
    scope: &DistributionScope,
    target: &str,
) {
    let table = match owner {
        DistributionOwner::Task => "crm_activity_task_records",
        DistributionOwner::Deal => "crm_sales_deal_records",
    };
    sql.push_str(&format!(" AND json_extract({table}.state,'$.owner.team_id')=? AND json_extract({table}.state,'$.status')='open' AND EXISTS (SELECT 1{COORDINATOR_JOIN} AND tm.version=CAST(? AS INTEGER) AND target.email=?)"));
    values.extend([
        JsValue::from_str(&scope.team_id),
        JsValue::from_str(tenant),
        JsValue::from_str(&scope.team_id),
        JsValue::from_str(actor),
        JsValue::from_str(&scope.team_version.to_string()),
        JsValue::from_str(target),
    ]);
}

#[derive(Serialize)]
pub(super) struct Intent {
    pub name: Option<String>,
    pub description: Option<String>,
    pub active: Option<bool>,
    pub roster: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub queue_coordinator: Option<QueueCoordinatorChange>,
    pub expected: Option<i64>,
}
pub(super) struct Saved {
    pub team: Team,
    pub replayed: bool,
    pub original_request_id: Option<String>,
}
#[derive(Deserialize)]
struct Row {
    state: String,
}
fn reject(code: &str, kind: ErrorCode) -> Failure {
    Failure::domain(code, kind)
}
pub(super) fn id(raw: &str) -> std::result::Result<String, Failure> {
    if raw.is_empty()
        || raw.len() > 128
        || raw.trim() != raw
        || raw.contains(['/', '\\', '%'])
        || raw == "."
        || raw == ".."
    {
        return Err(reject("ACCESS_TEAM_ID_INVALID", ErrorCode::InvalidArgument));
    }
    crm_module_sdk::RecordId::try_new(raw)
        .map(|v| v.as_str().to_owned())
        .map_err(|_| reject("ACCESS_TEAM_ID_INVALID", ErrorCode::InvalidArgument))
}
pub(super) fn decode(raw: &str) -> Result<Team> {
    serde_json::from_str(raw).map_err(|e| worker::Error::RustError(e.to_string()))
}
pub(super) async fn get(db: &D1Database, tenant: &str, id: &str) -> Result<Option<Team>> {
    bind(
        db,
        "SELECT state FROM crm_access_teams WHERE tenant=? AND id=?",
        &[JsValue::from_str(tenant), JsValue::from_str(id)],
    )?
    .first::<Row>(None)
    .await?
    .map(|row| decode(&row.state))
    .transpose()
}
pub(super) async fn list(
    db: &D1Database,
    tenant: &str,
    after: &str,
    size: usize,
    inactive: bool,
) -> Result<(Vec<Team>, Option<String>)> {
    let rows = bind(
        db,
        "SELECT state FROM crm_access_teams WHERE tenant=? AND id>? AND (?=1 OR json_extract(state,'$.active')=1) ORDER BY id LIMIT ?",
        &[
            JsValue::from_str(tenant),
            JsValue::from_str(after),
            JsValue::from_f64(if inactive { 1.0 } else { 0.0 }),
            JsValue::from_f64((size + 1) as f64),
        ],
    )?
    .all()
    .await?
    .results::<Row>()?;
    let teams = rows
        .iter()
        .map(|row| decode(&row.state))
        .collect::<Result<Vec<_>>>()?;
    let next = if teams.len() > size {
        teams.get(size - 1).map(|v| v.team_id.clone())
    } else {
        None
    };
    Ok((teams.into_iter().take(size).collect(), next))
}
async fn prior(db: &D1Database, tenant: &str, key: &str) -> Result<Option<d1_write::Receipt>> {
    bind(
        db,
        "SELECT state,digest,original_request_id FROM crm_team_receipts WHERE tenant=? AND key=?",
        &[JsValue::from_str(tenant), JsValue::from_str(key)],
    )?
    .first::<d1_write::Receipt>(None)
    .await
}
fn replay(
    receipt: d1_write::Receipt,
    id: &str,
    hash: &str,
) -> Result<std::result::Result<Saved, Failure>> {
    let team = decode(&receipt.state)?;
    if receipt.digest != hash || team.team_id != id {
        return Ok(Err(reject("IDEMPOTENCY_CONFLICT", ErrorCode::Aborted)));
    }
    Ok(Ok(Saved {
        team,
        replayed: true,
        original_request_id: receipt.original_request_id,
    }))
}
// Reused in the same D1 statement as any new owner assignment.
pub(super) const ELIGIBLE: &str = "EXISTS (SELECT 1 FROM crm_access_teams AS tm JOIN json_each(tm.state,'$.member_emails') AS m JOIN crm_users AS u ON u.email=m.value AND u.tenant_id=tm.tenant AND u.active=1 WHERE tm.tenant=? AND tm.id=? AND json_extract(tm.state,'$.active')=1 AND u.email NOT LIKE 'service:%')";
// Same Access roster authority for ClaimTask and every explicit team assignment.
pub(super) const MEMBER_ELIGIBLE: &str = "EXISTS (SELECT 1 FROM crm_access_teams AS tm JOIN json_each(tm.state,'$.member_emails') AS m JOIN crm_users AS u ON u.email=m.value AND u.tenant_id=tm.tenant AND u.active=1 WHERE tm.tenant=? AND tm.id=? AND json_extract(tm.state,'$.active')=1 AND u.email=? AND u.email NOT LIKE 'service:%')";
// Only trusted SQL expressions from the two Access mutation statements are
// passed here. Closed work never prevents retaining historical ownership.
pub(super) fn open_work(tenant: &str, team: &str) -> String {
    let coverage = super::sales_territories::owner_in_use(tenant, team, true);
    format!(
        "(EXISTS (SELECT 1 FROM crm_activity_task_records WHERE tenant={tenant} AND json_extract(state,'$.owner.team_id')={team} AND json_extract(state,'$.status')='open') OR EXISTS (SELECT 1 FROM crm_sales_deal_records WHERE tenant={tenant} AND json_extract(state,'$.owner.team_id')={team} AND json_extract(state,'$.status')='open') OR {coverage})"
    )
}
pub(super) async fn eligible(db: &D1Database, tenant: &str, id: &str) -> Result<bool> {
    let query = format!("SELECT 1 WHERE {ELIGIBLE}");
    Ok(bind(
        db,
        &query,
        &[JsValue::from_str(tenant), JsValue::from_str(id)],
    )?
    .first::<serde_json::Value>(None)
    .await?
    .is_some())
}
pub(super) async fn mutate(
    db: &D1Database,
    principal: &access::Actor,
    id: &str,
    intent: Intent,
    key: &str,
    trace: &str,
) -> Result<std::result::Result<Saved, Failure>> {
    let tenant = principal.tenant.as_str();
    let bytes =
        serde_json::to_vec(&(id, &intent)).map_err(|e| worker::Error::RustError(e.to_string()))?;
    let hash = format!("{:x}", Sha256::digest(bytes));
    if let Some(receipt) = prior(db, tenant, key).await? {
        return replay(receipt, id, &hash);
    }
    let create = intent.expected.is_none();
    let old = get(db, tenant, id).await?;
    let mut team = if create {
        if old.is_some() {
            return Ok(Err(reject("ACCESS_TEAM_EXISTS", ErrorCode::Aborted)));
        }
        Team {
            team_id: id.to_owned(),
            name: String::new(),
            description: String::new(),
            active: true,
            member_emails: Vec::new(),
            queue_coordinators: Vec::new(),
            version: 0,
        }
    } else {
        let Some(team) = old else {
            return Ok(Err(reject("ACCESS_TEAM_NOT_FOUND", ErrorCode::NotFound)));
        };
        if Some(team.version) != intent.expected {
            return Ok(Err(reject(
                "ACCESS_TEAM_VERSION_CONFLICT",
                ErrorCode::Aborted,
            )));
        }
        team
    };
    let previous = team.version;
    let original = team.clone();
    let changed_roster = intent.roster.is_some();
    if let Some(name) = intent.name {
        team.name = name.trim().to_owned();
    }
    if let Some(description) = intent.description {
        team.description = description;
    }
    if let Some(active) = intent.active {
        team.active = active;
    }
    if let Some(roster) = intent.roster {
        let mut normalized = Vec::with_capacity(roster.len());
        for member in roster {
            let email = match access_members::email(&member) {
                Ok(v) => v,
                Err(e) => return Ok(Err(e)),
            };
            normalized.push(email);
        }
        normalized.sort();
        if normalized.windows(2).any(|pair| pair[0] == pair[1]) {
            return Ok(Err(reject(
                "ACCESS_TEAM_DUPLICATE_MEMBER",
                ErrorCode::InvalidArgument,
            )));
        }
        team.member_emails = normalized;
    }
    // Removing a roster member or archiving the Team revokes the template.
    team.queue_coordinators
        .retain(|grant| team.active && team.member_emails.contains(&grant.email));
    let mut grant_evidence = None;
    if let Some(change) = intent.queue_coordinator {
        let email = match access_members::email(&change.email) {
            Ok(email) if email == change.email => email,
            _ => {
                return Ok(Err(reject(
                    "ACCESS_TEAM_COORDINATOR_INVALID",
                    ErrorCode::InvalidArgument,
                )));
            }
        };
        let existing = team
            .queue_coordinators
            .iter()
            .position(|grant| grant.email == email);
        if change.enabled {
            if !team.active || !team.member_emails.contains(&email) {
                return Ok(Err(reject(
                    "ACCESS_TEAM_COORDINATOR_NOT_IN_ROSTER",
                    ErrorCode::InvalidArgument,
                )));
            }
            let Some(member_version) = access_members::member_version(db, tenant, &email).await?
            else {
                return Ok(Err(reject(
                    "ACCESS_TEAM_COORDINATOR_INACTIVE",
                    ErrorCode::InvalidArgument,
                )));
            };
            let grant = QueueCoordinator {
                email,
                member_version,
            };
            if let Some(index) = existing {
                team.queue_coordinators[index] = grant.clone();
            } else {
                team.queue_coordinators.push(grant.clone());
            }
            grant_evidence = Some(grant);
        } else if let Some(index) = existing {
            team.queue_coordinators.remove(index);
        }
    }
    team.queue_coordinators
        .sort_by(|a, b| a.email.cmp(&b.email));
    if team.name.is_empty()
        || team.name.chars().count() > 160
        || team.description.chars().count() > 4000
        || team.member_emails.len() > 1000
        || team.queue_coordinators.len() > 32
    {
        return Ok(Err(reject(
            "ACCESS_TEAM_INVALID",
            ErrorCode::InvalidArgument,
        )));
    }
    if !create && original == team {
        return Ok(Err(reject(
            "ACCESS_TEAM_NO_CHANGES",
            ErrorCode::InvalidArgument,
        )));
    }
    let Some(version) = team.version.checked_add(1) else {
        return Ok(Err(reject(
            "ACCESS_TEAM_VERSION_OVERFLOW",
            ErrorCode::Aborted,
        )));
    };
    team.version = version;
    let state =
        serde_json::to_string(&team).map_err(|e| worker::Error::RustError(e.to_string()))?;
    // One statement checks actor rights, roster, duplicate name and open work.
    // Existing inactive members can remain in an unchanged historical roster.
    let work = open_work("?1", "?2");
    let guard = format!(
        " EXISTS (SELECT 1 FROM crm_users WHERE tenant_id=?1 AND email=?7 AND active=1 AND role='admin') AND NOT EXISTS (SELECT 1 FROM crm_access_teams AS t WHERE t.tenant=?1 AND t.id<>?2 AND lower(json_extract(t.state,'$.name'))=lower(json_extract(?3,'$.name'))) AND (?9=0 OR NOT EXISTS (SELECT 1 FROM json_each(?3,'$.member_emails') AS m WHERE NOT EXISTS (SELECT 1 FROM crm_users AS u WHERE u.tenant_id=?1 AND u.email=m.value AND u.active=1 AND u.email NOT LIKE 'service:%') AND NOT EXISTS (SELECT 1 FROM crm_access_teams AS old JOIN json_each(old.state,'$.member_emails') AS previous_member WHERE old.tenant=?1 AND old.id=?2 AND previous_member.value=m.value))) AND ((json_extract(?3,'$.active')=1 AND EXISTS (SELECT 1 FROM json_each(?3,'$.member_emails') AS m JOIN crm_users AS u ON u.tenant_id=?1 AND u.email=m.value AND u.active=1 WHERE u.email NOT LIKE 'service:%')) OR NOT {work}) AND (?11 IS NULL OR EXISTS (SELECT 1 FROM crm_users u WHERE u.tenant_id=?1 AND u.email=?11 AND u.active=1 AND u.version=CAST(?12 AS INTEGER)))"
    );
    let sql = if create {
        format!(
            "INSERT INTO crm_access_teams(tenant,id,state,version,idem_key,idem_digest,actor,request_id) SELECT ?1,?2,?3,?4,?5,?6,?7,?8 WHERE ?10=0 AND {guard} ON CONFLICT(tenant,id) DO NOTHING"
        )
    } else {
        format!(
            "UPDATE crm_access_teams SET state=?3,version=?4,idem_key=?5,idem_digest=?6,actor=?7,request_id=?8 WHERE tenant=?1 AND id=?2 AND version=?10 AND {guard}"
        )
    };
    let write = bind(
        db,
        &sql,
        &[
            JsValue::from_str(tenant),
            JsValue::from_str(id),
            JsValue::from_str(&state),
            JsValue::from_f64(team.version as f64),
            JsValue::from_str(key),
            JsValue::from_str(&hash),
            JsValue::from_str(&principal.identity),
            JsValue::from_str(trace),
            JsValue::from_f64(if changed_roster { 1.0 } else { 0.0 }),
            JsValue::from_f64(previous as f64),
            grant_evidence
                .as_ref()
                .map(|grant| JsValue::from_str(&grant.email))
                .unwrap_or(JsValue::NULL),
            grant_evidence
                .as_ref()
                .map(|grant| JsValue::from_str(&grant.member_version.to_string()))
                .unwrap_or(JsValue::NULL),
        ],
    )?
    .run()
    .await;
    match d1_write::confirm(
        write,
        prior(db, tenant, key),
        "ACCESS_TEAM_CONFLICT",
        "ACCESS_TEAM_WRITE_UNCONFIRMED",
    )
    .await
    {
        Ok(d1_write::Confirmation::Applied) => Ok(Ok(Saved {
            team,
            replayed: false,
            original_request_id: Some(trace.to_owned()),
        })),
        Ok(d1_write::Confirmation::Replayed(receipt)) => replay(receipt, id, &hash),
        Err(e) => Ok(Err(Failure::Write(e))),
    }
}
