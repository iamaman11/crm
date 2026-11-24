//! Sales coverage persistence: one original aggregate, CAS and atomic receipt/audit.
use super::{bind, d1_write, sales_deals};
use connectrpc::ErrorCode;
use crm_sales::{SalesAccountAssignment, SalesOwnerScope, SalesRoutingRule, SalesTerritory};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use wasm_bindgen::JsValue;
use worker::{D1Database, Result};

pub(super) type Failure = d1_write::OwnerFailure;

#[derive(Clone, Copy)]
pub(super) enum Kind {
    Territory,
    Assignment,
    RoutingRule,
    Quota,
}

impl Kind {
    fn table(self) -> &'static str {
        match self {
            Self::Territory => "crm_sales_territory_records",
            Self::Assignment => "crm_sales_account_assignment_records",
            Self::RoutingRule => "crm_sales_routing_rule_records",
            Self::Quota => "crm_sales_quota_records",
        }
    }

    fn receipts(self) -> &'static str {
        match self {
            Self::Territory => "crm_sales_territory_receipts",
            Self::Assignment => "crm_sales_account_assignment_receipts",
            Self::RoutingRule => "crm_sales_routing_rule_receipts",
            Self::Quota => "crm_sales_quota_receipts",
        }
    }

    pub(super) fn decode(self, state: &str) -> Result<Record> {
        match self {
            Self::Territory => SalesTerritory::decode(state).map(Record::Territory),
            Self::Assignment => SalesAccountAssignment::decode(state).map(Record::Assignment),
            Self::RoutingRule => SalesRoutingRule::decode(state).map(Record::RoutingRule),
            Self::Quota => crm_sales::SalesQuota::decode(state).map(Record::Quota),
        }
        .map_err(|e| worker::Error::RustError(e.code))
    }
}

pub(super) enum Record {
    Territory(SalesTerritory),
    Assignment(SalesAccountAssignment),
    RoutingRule(SalesRoutingRule),
    Quota(crm_sales::SalesQuota),
}

impl Record {
    fn kind(&self) -> Kind {
        match self {
            Self::Territory(_) => Kind::Territory,
            Self::Assignment(_) => Kind::Assignment,
            Self::RoutingRule(_) => Kind::RoutingRule,
            Self::Quota(_) => Kind::Quota,
        }
    }

    pub(super) fn id(&self) -> &str {
        match self {
            Self::Territory(value) => &value.territory_id,
            Self::Assignment(value) => &value.account_id,
            Self::RoutingRule(value) => &value.rule_id,
            Self::Quota(value) => &value.quota_id,
        }
    }

    pub(super) fn version(&self) -> i64 {
        match self {
            Self::Territory(value) => value.version,
            Self::Assignment(value) => value.version,
            Self::RoutingRule(value) => value.version,
            Self::Quota(value) => value.version,
        }
    }

    fn archived(&self) -> bool {
        match self {
            Self::Territory(value) => value.archived,
            Self::Assignment(value) => value.archived,
            Self::RoutingRule(value) => value.archived,
            Self::Quota(value) => value.archived,
        }
    }

    fn owners(&self) -> Vec<&SalesOwnerScope> {
        match self {
            Self::Territory(value) => vec![&value.default_owner],
            Self::Assignment(value) => std::iter::once(&value.primary_owner)
                .chain(value.overlays.iter().map(|overlay| &overlay.owner))
                .collect(),
            Self::RoutingRule(value) => vec![&value.target],
            Self::Quota(value) => match &value.scope {
                crm_sales::SalesQuotaScope::Owner(owner) => vec![owner],
                _ => Vec::new(),
            },
        }
    }

    fn encode(&self) -> Result<String> {
        match self {
            Self::Territory(value) => serde_json::to_string(value),
            Self::Assignment(value) => serde_json::to_string(value),
            Self::RoutingRule(value) => serde_json::to_string(value),
            Self::Quota(value) => serde_json::to_string(value),
        }
        .map_err(|e| worker::Error::RustError(e.to_string()))
    }

    fn revise(
        self,
        prior: Option<&Self>,
        expected: Option<i64>,
    ) -> std::result::Result<Self, Failure> {
        let result = match (self, prior) {
            (Self::Territory(value), None) => value.revise(None, expected).map(Self::Territory),
            (Self::Territory(value), Some(Self::Territory(old))) => {
                value.revise(Some(old), expected).map(Self::Territory)
            }
            (Self::Assignment(value), None) => value.revise(None, expected).map(Self::Assignment),
            (Self::Assignment(value), Some(Self::Assignment(old))) => {
                value.revise(Some(old), expected).map(Self::Assignment)
            }
            (Self::Quota(value), None) => value.revise(None, expected, super::now_nanos()).map(Self::Quota),
            (Self::Quota(value), Some(Self::Quota(old))) => value.revise(Some(old), expected, super::now_nanos()).map(Self::Quota),
            (Self::RoutingRule(value), None) => value.revise(None, expected).map(Self::RoutingRule),
            (Self::RoutingRule(value), Some(Self::RoutingRule(old))) => {
                value.revise(Some(old), expected).map(Self::RoutingRule)
            }
            _ => {
                return Err(Failure::domain(
                    "SALES_COVERAGE_KIND_INVALID",
                    ErrorCode::Internal,
                ));
            }
        };
        result.map_err(|e| {
            let code = if e.category == crm_module_sdk::ErrorCategory::Conflict {
                ErrorCode::Aborted
            } else {
                ErrorCode::InvalidArgument
            };
            Failure::domain(e.code, code)
        })
    }
}

#[derive(Deserialize)]
struct Row {
    id: String,
    state: String,
}

pub(super) struct Saved {
    pub record: Record,
    pub replayed: bool,
    pub original_request_id: Option<String>,
}

pub(super) struct Context<'a> {
    pub db: &'a D1Database,
    pub tenant: &'a str,
    pub actor: &'a str,
    pub request_id: &'a str,
}

pub(super) async fn get(
    db: &D1Database,
    tenant: &str,
    kind: Kind,
    id: &str,
) -> Result<Option<Record>> {
    bind(
        db,
        &format!(
            "SELECT id,state FROM {} WHERE tenant=? AND id=?",
            kind.table()
        ),
        &[JsValue::from_str(tenant), JsValue::from_str(id)],
    )?
    .first::<Row>(None)
    .await?
    .map(|row| kind.decode(&row.state))
    .transpose()
}

async fn receipt(ctx: &Context<'_>, kind: Kind, key: &str) -> Result<Option<d1_write::Receipt>> {
    bind(
        ctx.db,
        &format!(
            "SELECT digest,state,original_request_id FROM {} WHERE tenant=? AND key=?",
            kind.receipts()
        ),
        &[JsValue::from_str(ctx.tenant), JsValue::from_str(key)],
    )?
    .first::<d1_write::Receipt>(None)
    .await
}

// Guard the graph in the actual CAS. UNION terminates even if a corrupted
// persisted graph already contains a cycle.
const TERRITORY_GUARD: &str = " AND (json_extract(?3,'$.parent_id') IS NULL OR EXISTS (SELECT 1 FROM crm_sales_territory_records p WHERE p.tenant=?1 AND p.id=json_extract(?3,'$.parent_id') AND (json_extract(?3,'$.archived')=1 OR json_extract(p.state,'$.archived')=0))) AND NOT EXISTS (WITH RECURSIVE ancestors(id) AS (SELECT json_extract(?3,'$.parent_id') UNION SELECT json_extract(p.state,'$.parent_id') FROM crm_sales_territory_records p JOIN ancestors a ON p.id=a.id WHERE p.tenant=?1) SELECT 1 FROM ancestors WHERE id=?2) AND (json_extract(?3,'$.archived')=0 OR (NOT EXISTS (SELECT 1 FROM crm_sales_territory_records c WHERE c.tenant=?1 AND json_extract(c.state,'$.parent_id')=?2 AND json_extract(c.state,'$.archived')=0) AND NOT EXISTS (SELECT 1 FROM crm_sales_account_assignment_records a WHERE a.tenant=?1 AND json_extract(a.state,'$.territory_id')=?2 AND json_extract(a.state,'$.archived')=0)))";
const ASSIGNMENT_GUARD: &str = " AND EXISTS (SELECT 1 FROM crm_account_records a WHERE a.tenant=?1 AND a.id=?2) AND (json_extract(?3,'$.territory_id') IS NULL OR EXISTS (SELECT 1 FROM crm_sales_territory_records t WHERE t.tenant=?1 AND t.id=json_extract(?3,'$.territory_id') AND (json_extract(?3,'$.archived')=1 OR json_extract(t.state,'$.archived')=0)))";

const ROUTING_GUARD: &str = " AND (json_extract(?3,'$.archived')=1 OR (NOT EXISTS (SELECT 1 FROM crm_sales_routing_rule_records r WHERE r.tenant=?1 AND r.id<>?2 AND json_extract(r.state,'$.archived')=0 AND json_extract(r.state,'$.priority')=json_extract(?3,'$.priority')) AND (json_extract(?3,'$.criteria.territory_id') IS NULL OR EXISTS (SELECT 1 FROM crm_sales_territory_records t WHERE t.tenant=?1 AND t.id=json_extract(?3,'$.criteria.territory_id') AND json_extract(t.state,'$.archived')=0)) AND (json_extract(?3,'$.criteria.pipeline_id') IS NULL OR EXISTS (SELECT 1 FROM crm_sales_pipeline_records p WHERE p.tenant=?1 AND p.id=json_extract(?3,'$.criteria.pipeline_id') AND json_extract(p.state,'$.archived')=0 AND (json_extract(?3,'$.criteria.stage_id') IS NULL OR EXISTS (SELECT 1 FROM json_each(p.state,'$.stages') s WHERE json_extract(s.value,'$.stage_id')=json_extract(?3,'$.criteria.stage_id') AND json_extract(s.value,'$.archived')=0))))))";

// Published series remain reserved even when archived; a revised target uses
// the same quota ID. The CAS serializes overlapping publication attempts.
const QUOTA_GUARD: &str = " AND (json_extract(?3,'$.archived')=1 OR ((json_extract(?3,'$.scope.territory') IS NULL OR EXISTS (SELECT 1 FROM crm_sales_territory_records t WHERE t.tenant=?1 AND t.id=json_extract(?3,'$.scope.territory') AND json_extract(t.state,'$.archived')=0)) AND (json_extract(?3,'$.pipeline_id') IS NULL OR EXISTS (SELECT 1 FROM crm_sales_pipeline_records p WHERE p.tenant=?1 AND p.id=json_extract(?3,'$.pipeline_id') AND json_extract(p.state,'$.archived')=0)))) AND (json_extract(?3,'$.published')=0 OR NOT EXISTS (SELECT 1 FROM crm_sales_quota_records q WHERE q.tenant=?1 AND q.id<>?2 AND json_extract(q.state,'$.published')=1 AND json_extract(q.state,'$.scope')=json_extract(?3,'$.scope') AND json_extract(q.state,'$.pipeline_id') IS json_extract(?3,'$.pipeline_id') AND json_extract(q.state,'$.currency_code')=json_extract(?3,'$.currency_code') AND json_extract(q.state,'$.period.time_zone')=json_extract(?3,'$.period.time_zone') AND json_extract(q.state,'$.period.cadence')=json_extract(?3,'$.period.cadence') AND CAST(json_extract(q.state,'$.period.start_unix_nanos') AS INTEGER)<CAST(json_extract(?3,'$.period.end_exclusive_unix_nanos') AS INTEGER) AND CAST(json_extract(q.state,'$.period.end_exclusive_unix_nanos') AS INTEGER)>CAST(json_extract(?3,'$.period.start_unix_nanos') AS INTEGER)))";

pub(super) async fn quota_revision(
    db: &D1Database,
    tenant: &str,
    id: &str,
    version: i64,
) -> Result<Option<Record>> {
    bind(
        db,
        "SELECT id,state FROM crm_sales_quota_revisions WHERE tenant=? AND id=? AND version=CAST(? AS INTEGER)",
        &[
            JsValue::from_str(tenant),
            JsValue::from_str(id),
            JsValue::from_str(&version.to_string()),
        ],
    )?
    .first::<Row>(None)
    .await?
    .map(|row| Kind::Quota.decode(&row.state))
    .transpose()
}

pub(super) async fn mutate(
    ctx: &Context<'_>,
    draft: Record,
    key: &str,
    expected: Option<i64>,
) -> Result<std::result::Result<Saved, Failure>> {
    let kind = draft.kind();
    let hash = format!(
        "{:x}",
        Sha256::digest(
            serde_json::to_vec(&(expected, draft.encode()?))
                .map_err(|e| worker::Error::RustError(e.to_string()))?
        )
    );
    let replay = |prior: d1_write::Receipt| {
        if prior.digest != hash {
            return Ok(Err(Failure::domain(
                "IDEMPOTENCY_CONFLICT",
                ErrorCode::Aborted,
            )));
        }
        Ok(Ok(Saved {
            record: kind.decode(&prior.state)?,
            replayed: true,
            original_request_id: prior.original_request_id,
        }))
    };
    if let Some(prior) = receipt(ctx, kind, key).await? {
        return replay(prior);
    }
    let prior = get(ctx.db, ctx.tenant, kind, draft.id()).await?;
    let record = match draft.revise(prior.as_ref(), expected) {
        Ok(record) => record,
        Err(error) => {
            // A simultaneous identical write may have committed after the
            // initial receipt lookup. Reconcile before reporting conflict.
            if let Some(prior) = receipt(ctx, kind, key).await? {
                return replay(prior);
            }
            return Ok(Err(error));
        }
    };
    let mut guard = match kind {
        Kind::Territory => TERRITORY_GUARD.to_owned(),
        Kind::Assignment => ASSIGNMENT_GUARD.to_owned(),
        Kind::RoutingRule => ROUTING_GUARD.to_owned(),
        Kind::Quota => QUOTA_GUARD.to_owned(),
    };
    let state = record.encode()?;
    let mut values = vec![
        JsValue::from_str(ctx.tenant),
        JsValue::from_str(record.id()),
        JsValue::from_str(&state),
        JsValue::from_f64(record.version() as f64),
        JsValue::from_str(key),
        JsValue::from_str(&hash),
        JsValue::from_str(ctx.request_id),
        JsValue::from_str(ctx.actor),
        JsValue::from_f64(expected.unwrap_or(0) as f64),
    ];
    if matches!(kind, Kind::Territory) && record.archived() {
        guard.push_str(" AND NOT EXISTS (SELECT 1 FROM crm_sales_routing_rule_records r WHERE r.tenant=?1 AND json_extract(r.state,'$.archived')=0 AND json_extract(r.state,'$.criteria.territory_id')=?2)");
    }
    if !record.archived() {
        for owner in record.owners() {
            if !sales_deals::eligible_owner(ctx.db, ctx.tenant, owner).await? {
                return Ok(Err(Failure::domain(
                    "SALES_COVERAGE_OWNER_NOT_ELIGIBLE",
                    ErrorCode::Aborted,
                )));
            }
            guard.push_str(&sales_deals::owner_guard(Some(owner)));
            sales_deals::owner_parameters(&mut values, ctx.tenant, Some(owner));
        }
    }
    let table = kind.table();
    let query = if expected.is_none() {
        format!(
            "INSERT INTO {table}(tenant,id,state,version,idem_key,idem_digest,request_id,actor) SELECT ?1,?2,?3,?4,?5,?6,?7,?8 WHERE ?9=0{guard} ON CONFLICT(tenant,id) DO NOTHING"
        )
    } else {
        format!(
            "UPDATE {table} SET state=?3,version=?4,idem_key=?5,idem_digest=?6,request_id=?7,actor=?8 WHERE tenant=?1 AND id=?2 AND version=?9{guard}"
        )
    };
    let write = bind(ctx.db, &query, &values)?.run().await;
    match d1_write::confirm(
        write,
        receipt(ctx, kind, key),
        if matches!(kind, Kind::Quota) {
            "SALES_QUOTA_CONFLICT"
        } else if matches!(kind, Kind::RoutingRule) {
            "SALES_ROUTING_RULE_CONFLICT"
        } else {
            "SALES_COVERAGE_CONFLICT"
        },
        if matches!(kind, Kind::Quota) {
            "SALES_QUOTA_WRITE_UNCONFIRMED"
        } else if matches!(kind, Kind::RoutingRule) {
            "SALES_ROUTING_RULE_WRITE_UNCONFIRMED"
        } else {
            "SALES_COVERAGE_WRITE_UNCONFIRMED"
        },
    )
    .await
    {
        Ok(d1_write::Confirmation::Applied) => Ok(Ok(Saved {
            record,
            replayed: false,
            original_request_id: Some(ctx.request_id.to_owned()),
        })),
        Ok(d1_write::Confirmation::Replayed(prior)) => replay(prior),
        Err(error) => Ok(Err(Failure::Write(error))),
    }
}

pub(super) struct Filter<'a> {
    pub after: &'a str,
    pub limit: usize,
    pub include_archived: bool,
    pub territory_id: Option<&'a str>,
    pub primary_owner: Option<&'a SalesOwnerScope>,
}

pub(super) async fn list(
    db: &D1Database,
    tenant: &str,
    kind: Kind,
    filter: Filter<'_>,
) -> Result<(Vec<Record>, Option<String>)> {
    let mut query = format!(
        "SELECT id,state FROM {} WHERE tenant=? AND id>?",
        kind.table()
    );
    let mut values = vec![JsValue::from_str(tenant), JsValue::from_str(filter.after)];
    if !filter.include_archived {
        query.push_str(" AND json_extract(state,'$.archived')=0");
    }
    if let Some(id) = filter.territory_id {
        query.push_str(" AND json_extract(state,'$.territory_id')=?");
        values.push(JsValue::from_str(id));
    }
    if let Some(owner) = filter.primary_owner {
        query.push_str(" AND json_extract(state,'$.primary_owner')=?");
        let encoded =
            serde_json::to_string(owner).map_err(|e| worker::Error::RustError(e.to_string()))?;
        values.push(JsValue::from_str(&encoded));
    }
    query.push_str(" ORDER BY id LIMIT ?");
    values.push(JsValue::from_f64((filter.limit + 1) as f64));
    let mut rows = bind(db, &query, &values)?.all().await?.results::<Row>()?;
    let more = rows.len() > filter.limit;
    rows.truncate(filter.limit);
    let next = if more {
        rows.last().map(|row| row.id.clone())
    } else {
        None
    };
    let result = rows
        .iter()
        .map(|row| kind.decode(&row.state))
        .collect::<Result<Vec<_>>>()?;
    Ok((result, next))
}

// Only static SQL expressions from the Access CAS statements are accepted.
pub(super) fn owner_in_use(tenant: &str, id: &str, team: bool) -> String {
    let field = if team { "team" } else { "actor" };
    format!(
        "(EXISTS (SELECT 1 FROM crm_sales_territory_records t WHERE t.tenant={tenant} AND json_extract(t.state,'$.archived')=0 AND json_extract(t.state,'$.default_owner.{field}')={id}) OR EXISTS (SELECT 1 FROM crm_sales_routing_rule_records r WHERE r.tenant={tenant} AND json_extract(r.state,'$.archived')=0 AND json_extract(r.state,'$.target.{field}')={id}) OR EXISTS (SELECT 1 FROM crm_sales_account_assignment_records a WHERE a.tenant={tenant} AND json_extract(a.state,'$.archived')=0 AND (json_extract(a.state,'$.primary_owner.{field}')={id} OR EXISTS (SELECT 1 FROM json_each(a.state,'$.overlays') o WHERE json_extract(o.value,'$.owner.{field}')={id}))))"
    )
}
