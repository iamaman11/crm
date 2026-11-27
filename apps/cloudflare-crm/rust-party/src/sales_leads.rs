//! D1 persistence for the crm-sales Lead lifecycle.
use super::{
    accounts, bind,
    d1_write::{self, Confirmation, WriteResult, rejected},
    diagnostics::{MutationFailure, ReceiptUncertainty},
    identity_resolution, parties,
};
use crm_customer_accounts::{AccountPartyRole, AccountStatus};
use crm_identity_resolution::CanonicalResolution;
use crm_module_sdk::{ErrorCategory, RecordId, SdkError};
use crm_sales::lead::{Lead, LeadDecision, LeadStatus};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use wasm_bindgen::JsValue;
use worker::{D1Database, Result};

pub(super) struct Context<'a> {
    pub db: &'a D1Database,
    pub tenant: &'a str,
    pub actor: &'a str,
    pub request_id: &'a str,
    pub key: &'a str,
}

#[derive(Deserialize)]
struct Snapshot {
    id: String,
    original_party_id: String,
    name: String,
    status: LeadStatus,
    reason: String,
    deal_id: Option<String>,
    version: i64,
}

fn storage_error(error: impl std::fmt::Display) -> worker::Error {
    worker::Error::RustError(error.to_string())
}

impl Snapshot {
    fn into_lead(self) -> Result<Lead> {
        Ok(Lead {
            id: RecordId::try_new(self.id).map_err(storage_error)?,
            original_party_id: RecordId::try_new(self.original_party_id).map_err(storage_error)?,
            name: self.name,
            status: self.status,
            reason: self.reason,
            deal_id: self
                .deal_id
                .map(RecordId::try_new)
                .transpose()
                .map_err(storage_error)?,
            version: self.version,
        })
    }
}

#[derive(Deserialize)]
struct Event {
    #[serde(flatten)]
    snapshot: Snapshot,
    idem_digest: String,
    request_id: String,
    occurred_at: i64,
}

const FIELDS: &str = "id,original_party_id,name,status,reason,deal_id,version";

pub(super) struct Saved {
    pub lead: Lead,
    pub original_request_id: String,
    pub replayed: bool,
    pub d1_statement_changes: Option<usize>,
}

fn domain(error: SdkError) -> MutationFailure {
    MutationFailure::Rejected(Box::new(error))
}

fn receipt_failure(reason: ReceiptUncertainty) -> MutationFailure {
    MutationFailure::CommitUnconfirmed {
        code: "SALES_LEAD_COMMIT_UNCONFIRMED",
        reason,
    }
}

fn digest(input: impl Serialize) -> Result<String> {
    let data = serde_json::to_vec(&input).map_err(|e| worker::Error::RustError(e.to_string()))?;
    Ok(format!("{:x}", Sha256::digest(data)))
}

fn text(value: &str) -> JsValue {
    JsValue::from_str(value)
}

fn number(value: i64) -> JsValue {
    JsValue::from_f64(value as f64)
}

async fn prior(ctx: &Context<'_>) -> Result<Option<Event>> {
    bind(
        ctx.db,
        &format!("SELECT {FIELDS},idem_digest,request_id,occurred_at FROM crm_sales_lead_events WHERE tenant=? AND idem_key=?"),
        &[text(ctx.tenant), text(ctx.key)],
    )?
    .first(None)
    .await
}

fn saved(event: Event, expected_digest: &str, replayed: bool) -> Result<WriteResult<Saved>> {
    if event.idem_digest != expected_digest {
        return Ok(Err(rejected(
            "SALES_LEAD_IDEMPOTENCY_CONFLICT",
            ErrorCategory::Conflict,
        )));
    }
    Ok(Ok(Saved {
        lead: event.snapshot.into_lead()?,
        original_request_id: event.request_id,
        replayed,
        d1_statement_changes: None,
    }))
}

pub(super) async fn confirmed(ctx: &Context<'_>, id: &str) -> Result<Option<(i64, String)>> {
    Ok(prior(ctx)
        .await?
        .filter(|event| event.snapshot.id == id)
        .map(|event| (event.snapshot.version, event.request_id)))
}

pub(super) async fn converted(
    ctx: &Context<'_>,
    expected_digest: &str,
    replayed: bool,
) -> Result<WriteResult<Saved>> {
    match prior(ctx).await {
        Ok(Some(event)) => saved(event, expected_digest, replayed),
        Ok(None) => Ok(Err(receipt_failure(ReceiptUncertainty::NotFound))),
        Err(_) => Ok(Err(receipt_failure(ReceiptUncertainty::ReadFailed))),
    }
}

pub(super) async fn load(db: &D1Database, tenant: &str, id: &str) -> Result<Option<Lead>> {
    bind(
        db,
        &format!("SELECT {FIELDS} FROM crm_sales_lead_records WHERE tenant=? AND id=?"),
        &[text(tenant), text(id)],
    )?
    .first::<Snapshot>(None)
    .await?
    .map(Snapshot::into_lead)
    .transpose()
}

pub(super) async fn list(
    db: &D1Database,
    tenant: &str,
    cursor: &str,
    size: usize,
) -> Result<(Vec<Lead>, Option<String>)> {
    let mut entries = bind(
        db,
        &format!("SELECT {FIELDS} FROM crm_sales_lead_records WHERE tenant=? AND id>? ORDER BY id LIMIT ?"),
        &[text(tenant), text(cursor), number((size + 1) as i64)],
    )?
    .all()
    .await?
    .results::<Snapshot>()?;
    let next = if entries.len() > size {
        entries.pop();
        entries.last().map(|entry| entry.id.clone())
    } else {
        None
    };
    let leads = entries
        .into_iter()
        .map(Snapshot::into_lead)
        .collect::<Result<_>>()?;
    Ok((leads, next))
}

pub(super) async fn history(
    db: &D1Database,
    tenant: &str,
    id: &str,
) -> Result<Vec<(Lead, String, i64)>> {
    bind(
        db,
        &format!("SELECT {FIELDS},idem_digest,request_id,occurred_at FROM crm_sales_lead_events WHERE tenant=? AND id=? ORDER BY version"),
        &[text(tenant), text(id)],
    )?
    .all()
    .await?
    .results::<Event>()?
    .into_iter()
    .map(|event| Ok((event.snapshot.into_lead()?, event.request_id, event.occurred_at)))
    .collect()
}

pub(super) async fn create(
    ctx: &Context<'_>,
    id: RecordId,
    party_id: RecordId,
    name: &str,
) -> Result<WriteResult<Saved>> {
    let lead = match Lead::create(id, party_id, name) {
        Ok(value) => value,
        Err(error) => return Ok(Err(domain(error))),
    };
    let id = lead.id.as_str();
    let party = lead.original_party_id.as_str();
    let fingerprint = digest(("create", id, party, &lead.name))?;
    match prior(ctx).await {
        Ok(Some(event)) => return saved(event, &fingerprint, true),
        Ok(None) => (),
        Err(_) => return Ok(Err(receipt_failure(ReceiptUncertainty::ReadFailed))),
    }
    if parties::stored(ctx.db, ctx.tenant, party).await?.is_none() {
        return Ok(Err(rejected(
            "SALES_LEAD_PARTY_NOT_FOUND",
            ErrorCategory::InvalidArgument,
        )));
    }
    let write = bind(
        ctx.db,
        "INSERT INTO crm_sales_lead_records(tenant,id,original_party_id,name,status,version,idem_key,idem_digest,request_id,actor)
         VALUES(?,?,?,?,'new',1,?,?,?,?) ON CONFLICT(tenant,id) DO NOTHING",
        &[text(ctx.tenant), text(id), text(party), text(&lead.name), text(ctx.key),
          text(&fingerprint), text(ctx.request_id), text(ctx.actor)],
    )?.run().await;
    let d1_statement_changes = write
        .as_ref()
        .ok()
        .filter(|result| result.success())
        .and_then(|result| result.meta().ok().flatten())
        .and_then(|meta| meta.changes);
    match d1_write::confirm(
        write,
        prior(ctx),
        "SALES_LEAD_ALREADY_EXISTS",
        "SALES_LEAD_COMMIT_UNCONFIRMED",
    )
    .await
    {
        Ok(Confirmation::Applied) => Ok(Ok(Saved {
            lead,
            original_request_id: ctx.request_id.to_owned(),
            replayed: false,
            d1_statement_changes,
        })),
        Ok(Confirmation::Replayed(event)) => saved(event, &fingerprint, true),
        Err(error) => Ok(Err(error)),
    }
}

pub(super) async fn decide(
    ctx: &Context<'_>,
    id: &str,
    version: i64,
    decision: LeadDecision,
    reason: &str,
) -> Result<WriteResult<Saved>> {
    let fingerprint = digest((decision, id, version, reason.trim()))?;
    match prior(ctx).await {
        Ok(Some(event)) => return saved(event, &fingerprint, true),
        Ok(None) => (),
        Err(_) => return Ok(Err(receipt_failure(ReceiptUncertainty::ReadFailed))),
    }
    let Some(current) = load(ctx.db, ctx.tenant, id).await? else {
        return Ok(Err(rejected(
            "SALES_LEAD_NOT_FOUND",
            ErrorCategory::NotFound,
        )));
    };
    let lead = match current.decide(version, decision, reason) {
        Ok(value) => value,
        Err(error) => {
            return match prior(ctx).await {
                Ok(Some(event)) => saved(event, &fingerprint, true),
                Ok(None) => Ok(Err(domain(error))),
                Err(_) => Ok(Err(receipt_failure(ReceiptUncertainty::ReadFailed))),
            };
        }
    };
    let write = bind(
        ctx.db,
        "UPDATE crm_sales_lead_records SET status=?,reason=?,version=version+1,idem_key=?,idem_digest=?,request_id=?,actor=?
         WHERE tenant=? AND id=? AND version=? AND status='new'",
        &[text(decision.wire_name()), text(&lead.reason), text(ctx.key), text(&fingerprint),
          text(ctx.request_id), text(ctx.actor), text(ctx.tenant), text(id), number(version)],
    )?.run().await;
    let d1_statement_changes = write
        .as_ref()
        .ok()
        .filter(|result| result.success())
        .and_then(|result| result.meta().ok().flatten())
        .and_then(|meta| meta.changes);
    match d1_write::confirm(
        write,
        prior(ctx),
        "SALES_LEAD_VERSION_CONFLICT",
        "SALES_LEAD_COMMIT_UNCONFIRMED",
    )
    .await
    {
        Ok(Confirmation::Applied) => Ok(Ok(Saved {
            lead,
            original_request_id: ctx.request_id.to_owned(),
            replayed: false,
            d1_statement_changes,
        })),
        Ok(Confirmation::Replayed(event)) => saved(event, &fingerprint, true),
        Err(error) => Ok(Err(error)),
    }
}

pub(super) struct ConversionInput<'a> {
    pub id: &'a str,
    pub version: i64,
}

pub(super) struct ConversionGuard {
    account_version: i64,
    lead_party: CanonicalResolution,
    account_party: CanonicalResolution,
}

pub(super) async fn prepare_conversion(
    ctx: &Context<'_>,
    input: &ConversionInput<'_>,
    account_id: &str,
) -> Result<WriteResult<ConversionGuard>> {
    let Some(lead) = load(ctx.db, ctx.tenant, input.id).await? else {
        return Ok(Err(rejected(
            "SALES_LEAD_NOT_FOUND",
            ErrorCategory::NotFound,
        )));
    };
    if let Err(error) = lead.check_conversion(input.version) {
        return Ok(Err(domain(error)));
    }
    let Some(account) = accounts::load(ctx.db, ctx.tenant, account_id).await? else {
        return Ok(Err(rejected(
            "SALES_ACCOUNT_NOT_FOUND",
            ErrorCategory::InvalidArgument,
        )));
    };
    let primary = account
        .party_associations()
        .iter()
        .find(|association| association.role() == AccountPartyRole::Primary);
    let Some(primary) = primary.filter(|_| account.status() == AccountStatus::Active) else {
        return Ok(Err(rejected(
            "SALES_LEAD_ACCOUNT_NOT_ELIGIBLE",
            ErrorCategory::InvalidArgument,
        )));
    };
    let lead_party =
        identity_resolution::resolve_canonical(ctx.db, ctx.tenant, lead.original_party_id.as_str())
            .await?;
    let account_party =
        identity_resolution::resolve_canonical(ctx.db, ctx.tenant, primary.party().as_str())
            .await?;
    match (lead_party, account_party) {
        (Some(lead_party), Some(account_party))
            if lead_party.canonical_party_ref() == account_party.canonical_party_ref() =>
        {
            Ok(Ok(ConversionGuard {
                lead_party,
                account_party,
                account_version: account.version(),
            }))
        }
        _ => Ok(Err(rejected(
            "SALES_LEAD_ACCOUNT_PARTY_MISMATCH",
            ErrorCategory::InvalidArgument,
        ))),
    }
}

impl ConversionGuard {
    pub(super) fn append(
        &self,
        sql: &mut String,
        values: &mut Vec<JsValue>,
        ctx: &Context<'_>,
        input: &ConversionInput<'_>,
        account_id: &str,
    ) -> Result<()> {
        sql.push_str(" AND EXISTS(SELECT 1 FROM crm_sales_lead_records WHERE tenant=? AND id=? AND version=? AND status='qualified')
            AND EXISTS(SELECT 1 FROM crm_account_records WHERE tenant=? AND id=? AND version=?)
            AND NOT EXISTS(SELECT 1 FROM crm_sales_lead_events WHERE tenant=? AND idem_key=?)
            AND NOT EXISTS(SELECT 1 FROM crm_identity_merge_records WHERE tenant=? AND source_party=? AND status='active')");
        values.extend([
            text(ctx.tenant),
            text(input.id),
            number(input.version),
            text(ctx.tenant),
            text(account_id),
            number(self.account_version),
            text(ctx.tenant),
            text(ctx.key),
            text(ctx.tenant),
            text(self.lead_party.canonical_party_ref().as_str()),
        ]);
        // The existing identity owner resolved both paths. Ensure its proofs
        // still hold in the same statement that writes the Deal.
        for resolution in [&self.lead_party, &self.account_party] {
            let path = resolution
                .merge_operation_path()
                .iter()
                .map(|id| id.as_str())
                .collect::<Vec<_>>();
            let path = serde_json::to_string(&path)
                .map_err(|e| worker::Error::RustError(e.to_string()))?;
            sql.push_str(" AND NOT EXISTS(SELECT 1 FROM json_each(?) edge WHERE NOT EXISTS(
                SELECT 1 FROM crm_identity_merge_records WHERE tenant=? AND id=edge.value AND status='active'))");
            values.extend([text(&path), text(ctx.tenant)]);
        }
        Ok(())
    }
}
