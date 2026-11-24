//! Cloudflare native HTTP/D1 boundary for the ORIGINAL crm-sales Deal aggregate.
//! Stage progression, closing rules, money and version invariants belong to
//! crm-sales and crm-core-contracts, never the test pilot's crm_deals model.
use super::{
    bind,
    diagnostics::{MutationFailure, ReadFailureReason, ReceiptUncertainty},
    now_nanos,
};
use crm_core_contracts::{BasisPoints, CalendarDate, CurrencyCode, Money, Patch};
use crm_module_sdk::{ErrorCategory, RecordId, ResourceRef, SdkError};
use crm_sales::{
    AdvanceDealStage, CloseDeal, CreateDeal, Deal, DealOwner, DealStage, DealStatus,
    ForecastCategory, PipelineId, ProbabilityEstimate, ProbabilitySource, ReasonCode, ReopenDeal,
    SalesOwnerScope, StageId, StageTransitionPolicy, UpdateDeal, decode_deal_state,
    encode_deal_state,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use wasm_bindgen::JsValue;
use worker::{D1Database, Result};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CreateInput {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) account_id: String,
    pub(super) pipeline_id: String,
    pub(super) stage_id: String,
    pub(super) probability_bps: Option<u16>,
    pub(super) forecast_category: ForecastCategory,
    // Minor units are exact integers. Floating point money is never accepted.
    pub(super) amount_minor_units: Option<i64>,
    pub(super) currency: Option<String>,
    pub(super) primary_contact_id: Option<String>,
    pub(super) expected_close_date: Option<DateInput>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) owner: Option<SalesOwnerScope>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) automatic_routing: Option<super::sales_routing::AutomaticRouting>,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct DateInput {
    pub(super) year: i32,
    pub(super) month: u32,
    pub(super) day: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum FieldChange<T> {
    Keep,
    Set(T),
    Clear,
}
#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum StatusInput {
    Open,
    Won,
    Lost,
}
impl From<StatusInput> for DealStatus {
    fn from(value: StatusInput) -> Self {
        match value {
            StatusInput::Open => Self::Open,
            StatusInput::Won => Self::Won,
            StatusInput::Lost => Self::Lost,
        }
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct StageInput {
    pub(super) expected_version: i64,
    pub(super) pipeline_id: String,
    pub(super) stage_id: String,
    pub(super) status: StatusInput,
    pub(super) close_reason_code: Option<String>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CloseInput {
    pub(super) expected_version: i64,
    pub(super) outcome: StatusInput,
    pub(super) reason_code: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ReopenInput {
    pub(super) expected_version: i64,
    pub(super) reason_code: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) owner: Option<SalesOwnerScope>,
}

// HTTP only edits the original Deal's business fields, never trusted ownership.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct UpdateInput {
    pub(super) expected_version: i64,
    pub(super) name: Option<String>,
    pub(super) clear_name: Option<bool>,
    pub(super) amount_minor_units: Option<i64>,
    pub(super) currency: Option<String>,
    pub(super) clear_amount: Option<bool>,
    pub(super) probability_bps: Option<u16>,
    pub(super) use_stage_probability: bool,
    pub(super) forecast_category: Option<ForecastCategory>,
    pub(super) primary_contact: FieldChange<String>,
    pub(super) expected_close_date: FieldChange<DateInput>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) owner: Option<SalesOwnerScope>,
}
#[derive(Deserialize)]
struct Row {
    state: String,
}
#[derive(Deserialize)]
struct PagedRow {
    id: String,
    state: String,
}

// Read the canonical serialized Deal through its original owner, rather than
// coercing individual JSON values in SQLite or duplicating persistence rules.
#[derive(Deserialize)]
struct PipelineMoneyRow {
    as_of_nanos: String,
    state: String,
    stage_entered_at_nanos: Option<String>,
    has_overdue_linked_task: i64,
}
#[derive(Deserialize)]
struct Receipt {
    digest: String,
    state: String,
    original_request_id: Option<String>,
}

fn hash<T: Serialize>(operation: &str, id: &str, input: &T) -> Result<String> {
    let bytes = serde_json::to_vec(&json!([operation, id, input]))
        .map_err(|e| worker::Error::RustError(e.to_string()))?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}
fn encode(deal: &Deal) -> Result<String> {
    String::from_utf8(encode_deal_state(deal).map_err(|e| worker::Error::RustError(e.code))?)
        .map_err(|e| worker::Error::RustError(e.to_string()))
}
pub(super) fn decode(data: &str) -> Result<Deal> {
    decode_deal_state(data.as_bytes()).map_err(|e| worker::Error::RustError(e.code))
}
async fn prior(db: &D1Database, tenant: &str, k: &str) -> Result<Option<Receipt>> {
    bind(
        db,
        "SELECT digest,state,original_request_id FROM crm_sales_deal_receipts WHERE tenant=? AND key=?",
        &[JsValue::from_str(tenant), JsValue::from_str(k)],
    )?
    .first::<Receipt>(None)
    .await
}
// Read the existing immutable receipt; never infer a commit from the
// current Deal version and never disclose a receipt for another record.
pub(super) async fn confirmed_receipt(
    db: &D1Database,
    tenant: &str,
    id: &str,
    key: &str,
) -> std::result::Result<Option<(String, i64)>, ReadFailureReason> {
    let Some(receipt) = prior(db, tenant, key).await? else {
        return Ok(None);
    };
    let Some(original) = receipt.original_request_id else {
        return Ok(None);
    };
    let deal = decode(&receipt.state).map_err(|_| ReadFailureReason::SalesStateDecodeFailed)?;
    if deal.deal_id().as_str() != id || original.is_empty() {
        return Ok(None);
    }
    Ok(Some((original, deal.version())))
}
pub(super) async fn load(db: &D1Database, tenant: &str, id: &str) -> Result<Option<Deal>> {
    bind(
        db,
        "SELECT state FROM crm_sales_deal_records WHERE tenant=? AND id=?",
        &[JsValue::from_str(tenant), JsValue::from_str(id)],
    )?
    .first::<Row>(None)
    .await?
    .map(|row| decode(&row.state))
    .transpose()
}
async fn account_exists(db: &D1Database, tenant: &str, id: &str) -> Result<bool> {
    Ok(bind(
        db,
        "SELECT id FROM crm_account_records WHERE tenant=? AND id=?",
        &[JsValue::from_str(tenant), JsValue::from_str(id)],
    )?
    .first::<Value>(None)
    .await?
    .is_some())
}
fn typed_date(date: DateInput) -> std::result::Result<CalendarDate, &'static str> {
    let month = u8::try_from(date.month).map_err(|_| "CONTRACT_DATE_MONTH_INVALID")?;
    let day = u8::try_from(date.day).map_err(|_| "CONTRACT_DATE_DAY_INVALID")?;
    CalendarDate::try_new(date.year, month, day).map_err(|e| e.code)
}

// The contact endpoint is owned by a Party belonging to the selected Account.
// Read the two ORIGINAL aggregates with the same tenant, never trust a browser
// assertion or accept a foreign/inactive contact.
async fn valid_contact(
    db: &D1Database,
    tenant: &str,
    account_id: &str,
    contact_id: &str,
) -> Result<std::result::Result<ResourceRef, &'static str>> {
    let Ok(point_id) = crm_contact_points::ContactPointId::try_new(contact_id) else {
        return Ok(Err("SALES_PRIMARY_CONTACT_ID_INVALID"));
    };
    let Some(contact) = super::contact_points::load(db, tenant, point_id.as_str()).await? else {
        return Ok(Err("SALES_PRIMARY_CONTACT_NOT_FOUND"));
    };
    if contact.status() != crm_contact_points::ContactPointStatus::Active {
        return Ok(Err("SALES_PRIMARY_CONTACT_INACTIVE"));
    }
    let Some(account) = super::accounts::load(db, tenant, account_id).await? else {
        return Ok(Err("SALES_ACCOUNT_NOT_FOUND"));
    };
    if !account
        .party_associations()
        .iter()
        .any(|assoc| assoc.party().as_str() == contact.party_ref().as_str())
    {
        return Ok(Err("SALES_PRIMARY_CONTACT_ACCOUNT_MISMATCH"));
    }
    Ok(Ok(ResourceRef {
        resource_type: "crm.contact-points".into(),
        resource_id: point_id.as_str().into(),
        version: None,
    }))
}

fn stage(
    pipeline_id: String,
    stage_id: String,
    ordinal: u16,
) -> std::result::Result<DealStage, String> {
    let pipeline = PipelineId::try_new(pipeline_id).map_err(|e| e.code)?;
    let id = StageId::try_new(stage_id).map_err(|e| e.code)?;
    DealStage::try_new(pipeline, id, ordinal).map_err(|e| e.code)
}
// One D1 + ORIGINAL crm-sales mutation shared by both native HTTP and Connect.
// The caller handles transport-level decoding and error formatting only.
pub(super) struct SalesSaved {
    pub(super) deal: Deal,
    pub(super) replayed: bool,
    pub(super) original_request_id: Option<String>,
    pub(super) d1_statement_changes: Option<usize>,
}

fn rejected<T>(
    code: impl Into<String>,
    category: ErrorCategory,
) -> std::result::Result<T, MutationFailure> {
    let code = code.into();
    Err(MutationFailure::Rejected(Box::new(SdkError::new(
        code.clone(),
        category,
        false,
        code,
    ))))
}

fn rejected_sdk<T>(error: SdkError) -> std::result::Result<T, MutationFailure> {
    Err(MutationFailure::Rejected(Box::new(error)))
}

// The digest includes operation, record ID and typed input. Older receipts
// deliberately have no original_request_id rather than invented provenance.
fn replay(receipt: Receipt, digest: &str) -> std::result::Result<SalesSaved, MutationFailure> {
    if receipt.digest != digest {
        return rejected("IDEMPOTENCY_CONFLICT", ErrorCategory::Conflict);
    }
    let deal = decode(&receipt.state).map_err(|_| unconfirmed(ReceiptUncertainty::DecodeFailed))?;
    Ok(SalesSaved {
        deal,
        replayed: true,
        original_request_id: receipt.original_request_id,
        d1_statement_changes: None,
    })
}

fn acknowledged(deal: Deal, trace: &str, changes: usize) -> SalesSaved {
    SalesSaved {
        deal,
        replayed: false,
        original_request_id: Some(trace.to_owned()),
        d1_statement_changes: Some(changes),
    }
}

// A successful transport is not sufficient; missing change metadata is not
// proof of a version conflict. This is raw D1 statement metadata, not
// a count of Deal records; trigger-related changes may be included.
fn changes(result: &worker::d1::D1Result) -> Option<usize> {
    if !result.success() {
        return None;
    }
    result.meta().ok().flatten()?.changes
}

fn unconfirmed(reason: ReceiptUncertainty) -> MutationFailure {
    MutationFailure::CommitUnconfirmed {
        code: "SALES_COMMIT_UNCONFIRMED",
        reason,
    }
}

// Read back the exact receipt. Never repeat a mutation here. No receipt after
// an ambiguous SDK error does not establish that the write rolled back.
async fn reconcile(
    db: &D1Database,
    tenant: &str,
    key: &str,
    digest: &str,
    proven_no_change: bool,
) -> std::result::Result<SalesSaved, MutationFailure> {
    // Signed workerd-only fault injection. The production artifact never
    // contains this key or the deliberately missing test relation.
    #[cfg(feature = "native-ci")]
    if key == "r471-ci-receipt-read-fail" {
        let observed = db
            .prepare("SELECT digest,state,original_request_id FROM crm_ci_missing_receipt_table")
            .first::<Receipt>(None)
            .await;
        return classify_receipt(observed, digest, proven_no_change);
    }
    classify_receipt(prior(db, tenant, key).await, digest, proven_no_change)
}

fn classify_receipt(
    observed: Result<Option<Receipt>>,
    digest: &str,
    proven_no_change: bool,
) -> std::result::Result<SalesSaved, MutationFailure> {
    match observed {
        Ok(Some(receipt)) => replay(receipt, digest),
        Ok(None) if proven_no_change => {
            rejected("SALES_DEAL_VERSION_CONFLICT", ErrorCategory::Conflict)
        }
        Ok(None) => Err(unconfirmed(ReceiptUncertainty::NotFound)),
        Err(_) => Err(unconfirmed(ReceiptUncertainty::ReadFailed)),
    }
}

// Existing CI-only signed diagnostic endpoint exercises the exact production
// receipt query/classifier against the local D1 binding. It never writes.
#[cfg(feature = "native-ci")]
pub(super) async fn probe_receipt_read(
    db: &D1Database,
    tenant: &str,
    unused_key: &str,
) -> &'static str {
    match reconcile(db, tenant, unused_key, "", true).await {
        Err(MutationFailure::Rejected(error)) if error.category == ErrorCategory::Conflict => {
            "PROVEN_NO_CHANGE"
        }
        Err(MutationFailure::CommitUnconfirmed {
            reason: ReceiptUncertainty::ReadFailed,
            ..
        }) => "RECEIPT_READ_FAILED",
        _ => "UNEXPECTED",
    }
}

// Owner selection validates against the single Access authority; the same
// predicate is checked again in the original Deal write, at commit time.
pub(super) async fn eligible_owner(
    db: &D1Database,
    tenant: &str,
    owner: &SalesOwnerScope,
) -> Result<bool> {
    match owner {
        SalesOwnerScope::Actor(email) => {
            if super::access_members::email(email).ok().as_deref() != Some(email.as_str()) {
                return Ok(false);
            }
            super::access_members::valid_assignee(db, tenant, email).await
        }
        SalesOwnerScope::Team(id) => super::access_teams::eligible(db, tenant, id).await,
    }
}
pub(super) fn owner_guard(owner: Option<&SalesOwnerScope>) -> String {
    match owner {
        None => String::new(),
        Some(SalesOwnerScope::Actor(_)) => {
            " AND EXISTS (SELECT 1 FROM crm_users WHERE tenant_id=? AND email=? AND active=1)"
                .into()
        }
        Some(SalesOwnerScope::Team(_)) => format!(" AND {}", super::access_teams::ELIGIBLE),
    }
}
pub(super) fn owner_parameters(
    values: &mut Vec<JsValue>,
    tenant: &str,
    owner: Option<&SalesOwnerScope>,
) {
    if let Some(scope) = owner {
        let id = match scope {
            SalesOwnerScope::Actor(id) | SalesOwnerScope::Team(id) => id,
        };
        values.extend([JsValue::from_str(tenant), JsValue::from_str(id)]);
    }
}
// Filter list, stage totals and period analytics by the identical current
// owner dimension, before pagination or monetary aggregation.
fn append_owner_filter(
    query: &mut String,
    values: &mut Vec<JsValue>,
    owner: Option<&SalesOwnerScope>,
) {
    if let Some(owner) = owner {
        let id = match owner {
            SalesOwnerScope::Actor(id) => {
                query.push_str(" AND json_extract(state,'$.owner.actor_id')=?");
                id
            }
            SalesOwnerScope::Team(id) => {
                query.push_str(" AND json_extract(state,'$.owner.team_id')=?");
                id
            }
        };
        values.push(JsValue::from_str(id));
    }
}

// A concurrent identical automatic request can commit between the first
// receipt lookup and route resolution. Its receipt still wins over a new
// capacity/no-match/stale-preview error.
async fn routing_rejection(
    db: &D1Database,
    tenant: &str,
    key: &str,
    digest: &str,
    error: SdkError,
) -> std::result::Result<SalesSaved, MutationFailure> {
    match prior(db, tenant, key).await {
        Ok(Some(receipt)) => replay(receipt, digest),
        Ok(None) => rejected_sdk(error),
        Err(_) => Err(unconfirmed(ReceiptUncertainty::ReadFailed)),
    }
}

pub(super) async fn create_typed(
    input: CreateInput,
    k: &str,
    db: &D1Database,
    tenant: &str,
    actor: &str,
    trace: &str,
) -> Result<std::result::Result<SalesSaved, MutationFailure>> {
    if !(8..=120).contains(&k.len()) {
        return Ok(rejected(
            "IDEMPOTENCY_KEY_REQUIRED",
            ErrorCategory::InvalidArgument,
        ));
    }
    let digest = hash("create", &input.id, &input)?;
    match prior(db, tenant, k).await {
        Ok(Some(p)) => return Ok(replay(p, &digest)),
        Ok(None) => (),
        Err(_) => return Ok(Err(unconfirmed(ReceiptUncertainty::ReadFailed))),
    }
    let id = match RecordId::try_new(&input.id) {
        Ok(x) => x,
        Err(_) => {
            return Ok(rejected(
                "SALES_DEAL_ID_INVALID",
                ErrorCategory::InvalidArgument,
            ));
        }
    };
    let account_id = match RecordId::try_new(&input.account_id) {
        Ok(x) => x,
        Err(_) => {
            return Ok(rejected(
                "SALES_ACCOUNT_ID_INVALID",
                ErrorCategory::InvalidArgument,
            ));
        }
    };
    if !account_exists(db, tenant, account_id.as_str()).await? {
        return Ok(rejected(
            "SALES_ACCOUNT_NOT_FOUND",
            ErrorCategory::InvalidArgument,
        ));
    }
    if input.owner.is_some() && input.automatic_routing.is_some() {
        return Ok(rejected("SALES_ROUTING_MANUAL_AUTO_CONFLICT", ErrorCategory::InvalidArgument));
    }
    let resolution = if let Some(auto) = &input.automatic_routing {
        let facts = crm_sales::SalesRoutingFacts {
            account_id: input.account_id.clone(),
            pipeline_id: input.pipeline_id.clone(),
            stage_id: input.stage_id.clone(),
            territory_id: auto.territory_id.clone(),
            currency_code: input.currency.clone(),
            amount_minor_units: input.amount_minor_units,
        };
        let resolved = match super::sales_routing::resolve(db, tenant, &facts).await? {
            Ok(value) => value,
            Err(error) => return Ok(routing_rejection(db, tenant, k, &digest, error).await),
        };
        if auto
            .expected_decision
            .as_ref()
            .is_some_and(|expected| expected != &resolved.fingerprint)
        {
            let error = SdkError::new(
                "SALES_ROUTING_PREVIEW_STALE",
                ErrorCategory::Conflict,
                false,
                "Routing preview changed.",
            );
            return Ok(routing_rejection(db, tenant, k, &digest, error).await);
        }
        Some(resolved)
    } else {
        None
    };
    let decision = if let Some(resolved) = &resolution {
        resolved.decision.clone()
    } else if let Some(selection) = &input.owner {
        if !eligible_owner(db, tenant, selection).await? {
            return Ok(rejected("SALES_OWNER_NOT_ELIGIBLE", ErrorCategory::InvalidArgument));
        }
        crm_sales::SalesRoutingDecision::explicit(selection.clone(), crm_sales::SalesRoutingSource::Manual)
    } else {
        crm_sales::SalesRoutingDecision::explicit(
            SalesOwnerScope::Actor(actor.to_owned()), crm_sales::SalesRoutingSource::Creator)
    };
    let owner = match decision.selected_owner.domain_owner() {
        Ok(owner) => owner,
        Err(e) => return Ok(rejected_sdk(e)),
    };
    let guarded_owner = resolution.as_ref().map(|r| &r.decision.selected_owner).or(input.owner.as_ref());
    let Some(catalog_stage) =
        super::sales_pipelines::active_stage(db, tenant, &input.pipeline_id, &input.stage_id)
            .await?
    else {
        return Ok(rejected(
            "SALES_STAGE_NOT_IN_CATALOG",
            ErrorCategory::InvalidArgument,
        ));
    };
    let stage = match stage(input.pipeline_id, input.stage_id, catalog_stage.ordinal) {
        Ok(x) => x,
        Err(code) => return Ok(rejected(code, ErrorCategory::InvalidArgument)),
    };
    let probability = match BasisPoints::try_new(
        input
            .probability_bps
            .unwrap_or(catalog_stage.probability_basis_points),
    ) {
        Ok(value) if input.probability_bps.is_some() => ProbabilityEstimate::Manual(value),
        Ok(value) => ProbabilityEstimate::StageDefault(value),
        Err(e) => return Ok(rejected(e.code, ErrorCategory::InvalidArgument)),
    };
    let amount = match (input.amount_minor_units, input.currency) {
        (None, None) => None,
        (Some(units), Some(currency)) => {
            let currency = match CurrencyCode::try_new(currency) {
                Ok(x) => x,
                Err(e) => return Ok(rejected(e.code, ErrorCategory::InvalidArgument)),
            };
            Some(Money::new(i128::from(units), currency))
        }
        _ => {
            return Ok(rejected(
                "SALES_AMOUNT_CURRENCY_PAIR_REQUIRED",
                ErrorCategory::InvalidArgument,
            ));
        }
    };
    let primary_contact = match input.primary_contact_id.as_ref() {
        None => None,
        Some(id) => match valid_contact(db, tenant, account_id.as_str(), id).await? {
            Ok(v) => Some(v),
            Err(code) => return Ok(rejected(code, ErrorCategory::InvalidArgument)),
        },
    };
    let expected_close_date = match input.expected_close_date {
        None => None,
        Some(date) => match typed_date(date) {
            Ok(v) => Some(v),
            Err(code) => return Ok(rejected(code, ErrorCategory::InvalidArgument)),
        },
    };
    let deal = match Deal::create_with_routing(CreateDeal {
        deal_id: id,
        name: input.name,
        owner,
        account: Some(ResourceRef {
            resource_type: "crm.customer-accounts".into(),
            resource_id: account_id.as_str().into(),
            version: None,
        }),
        primary_contact,
        stage,
        amount,
        expected_close_date,
        probability,
        forecast_category: input.forecast_category,
        occurred_at_unix_nanos: now_nanos(),
    }, decision) {
        Ok(x) => x,
        Err(e) => return Ok(rejected_sdk(e)),
    };
    let saved = encode(&deal)?;
    let mut sql = format!(
        "INSERT INTO crm_sales_deal_records(tenant,id,account_id,state,version,idem_key,idem_digest,request_id,actor) SELECT ?,?,?,?,?,?,?,?,? WHERE 1=1{}",
        owner_guard(guarded_owner)
    );
    let mut values = vec![
        JsValue::from_str(tenant),
        JsValue::from_str(deal.deal_id().as_str()),
        JsValue::from_str(account_id.as_str()),
        JsValue::from_str(&saved),
        JsValue::from_f64(deal.version() as f64),
        JsValue::from_str(k),
        JsValue::from_str(&digest),
        JsValue::from_str(trace),
        JsValue::from_str(actor),
    ];
    owner_parameters(&mut values, tenant, guarded_owner);
    if let Some(resolved) = &resolution {
        resolved.append_guard(&mut sql, &mut values, tenant);
    }
    sql.push_str(" ON CONFLICT(tenant,id) DO NOTHING");
    let r = bind(db, &sql, &values)?.run().await;
    if let Some(count) = r.as_ref().ok().and_then(changes).filter(|count| *count > 0) {
        // Discard the genuine D1 write acknowledgement in isolated CI only.
        #[cfg(feature = "native-ci")]
        if matches!(k, "r471-ci-ack-lost" | "r471-ci-receipt-read-fail") {
            return Ok(reconcile(db, tenant, k, &digest, false).await);
        }
        return Ok(Ok(acknowledged(deal, trace, count)));
    }
    if resolution.is_some() && r.as_ref().ok().and_then(changes) == Some(0) {
        return Ok(match prior(db, tenant, k).await {
            Ok(Some(receipt)) => replay(receipt, &digest),
            Ok(None) => rejected("SALES_ROUTING_WRITE_CONFLICT", ErrorCategory::Conflict),
            Err(_) => Err(unconfirmed(ReceiptUncertainty::ReadFailed)),
        });
    }
    Ok(reconcile(
        db,
        tenant,
        k,
        &digest,
        r.as_ref().ok().and_then(changes) == Some(0),
    )
    .await)
}
pub(super) async fn update_typed(
    input: UpdateInput,
    key: &str,
    db: &D1Database,
    tenant: &str,
    id: &str,
    actor: &str,
    trace: &str,
) -> Result<std::result::Result<SalesSaved, MutationFailure>> {
    if !(8..=120).contains(&key.len()) {
        return Ok(rejected(
            "IDEMPOTENCY_KEY_REQUIRED",
            ErrorCategory::InvalidArgument,
        ));
    }
    let digest = hash("update", id, &input)?;
    match prior(db, tenant, key).await {
        Ok(Some(p)) => return Ok(replay(p, &digest)),
        Ok(None) => (),
        Err(_) => return Ok(Err(unconfirmed(ReceiptUncertainty::ReadFailed))),
    }
    let Some(mut deal) = load(db, tenant, id).await? else {
        return Ok(rejected("NOT_FOUND", ErrorCategory::NotFound));
    };
    if input.clear_name == Some(true) && input.name.is_some() {
        return Ok(rejected(
            "SALES_NAME_UPDATE_CONFLICT",
            ErrorCategory::InvalidArgument,
        ));
    }
    let name = if input.clear_name == Some(true) {
        Patch::Clear
    } else {
        input.name.map_or(Patch::Keep, Patch::Set)
    };
    if input.clear_amount == Some(true)
        && (input.amount_minor_units.is_some() || input.currency.is_some())
    {
        return Ok(rejected(
            "SALES_AMOUNT_UPDATE_CONFLICT",
            ErrorCategory::InvalidArgument,
        ));
    }
    let amount = if input.clear_amount == Some(true) {
        Patch::Clear
    } else {
        match (input.amount_minor_units, input.currency) {
            (None, None) => Patch::Keep,
            (Some(units), Some(currency)) => {
                let currency = match CurrencyCode::try_new(currency) {
                    Ok(x) => x,
                    Err(e) => return Ok(rejected(e.code, ErrorCategory::InvalidArgument)),
                };
                Patch::Set(Money::new(i128::from(units), currency))
            }
            _ => {
                return Ok(rejected(
                    "SALES_AMOUNT_CURRENCY_PAIR_REQUIRED",
                    ErrorCategory::InvalidArgument,
                ));
            }
        }
    };
    if input.use_stage_probability && input.probability_bps.is_some() {
        return Ok(rejected(
            "SALES_PROBABILITY_SOURCE_CONFLICT",
            ErrorCategory::InvalidArgument,
        ));
    }
    let probability = if input.use_stage_probability {
        let Some(catalog_stage) = super::sales_pipelines::active_stage(
            db,
            tenant,
            deal.stage().pipeline_id().as_str(),
            deal.stage().stage_id().as_str(),
        )
        .await?
        else {
            return Ok(rejected(
                "SALES_STAGE_NOT_IN_CATALOG",
                ErrorCategory::InvalidArgument,
            ));
        };
        match BasisPoints::try_new(catalog_stage.probability_basis_points) {
            Ok(value) => Patch::Set(ProbabilityEstimate::StageDefault(value)),
            Err(e) => return Ok(rejected(e.code, ErrorCategory::InvalidArgument)),
        }
    } else {
        match input.probability_bps {
            Some(value) => match BasisPoints::try_new(value) {
                Ok(value) => Patch::Set(ProbabilityEstimate::Manual(value)),
                Err(e) => return Ok(rejected(e.code, ErrorCategory::InvalidArgument)),
            },
            None => Patch::Keep,
        }
    };
    let contact_patch = match input.primary_contact {
        FieldChange::Keep => Patch::Keep,
        FieldChange::Clear => Patch::Clear,
        FieldChange::Set(id) => {
            let Some(account) = deal.account() else {
                return Ok(rejected(
                    "SALES_ACCOUNT_NOT_FOUND",
                    ErrorCategory::InvalidArgument,
                ));
            };
            match valid_contact(db, tenant, &account.resource_id, &id).await? {
                Ok(v) => Patch::Set(v),
                Err(code) => return Ok(rejected(code, ErrorCategory::InvalidArgument)),
            }
        }
    };
    let date_patch = match input.expected_close_date {
        FieldChange::Keep => Patch::Keep,
        FieldChange::Clear => Patch::Clear,
        FieldChange::Set(date) => match typed_date(date) {
            Ok(v) => Patch::Set(v),
            Err(code) => return Ok(rejected(code, ErrorCategory::InvalidArgument)),
        },
    };
    let owner = if let Some(selection) = &input.owner {
        if !eligible_owner(db, tenant, selection).await? {
            return Ok(rejected(
                "SALES_OWNER_NOT_ELIGIBLE",
                ErrorCategory::InvalidArgument,
            ));
        }
        match selection.domain_owner() {
            Ok(owner) => Patch::Set(owner),
            Err(e) => return Ok(rejected_sdk(e)),
        }
    } else {
        Patch::Keep
    };
    if matches!(owner, Patch::Keep)
        && matches!(name, Patch::Keep)
        && matches!(amount, Patch::Keep)
        && matches!(probability, Patch::Keep)
        && matches!(contact_patch, Patch::Keep)
        && matches!(date_patch, Patch::Keep)
        && input.forecast_category.is_none()
    {
        return Ok(rejected(
            "SALES_EMPTY_UPDATE",
            ErrorCategory::InvalidArgument,
        ));
    }
    if let Err(e) = deal.apply_update(UpdateDeal {
        expected_version: input.expected_version,
        name,
        owner,
        account: Patch::Keep,
        primary_contact: contact_patch,
        amount,
        expected_close_date: date_patch,
        probability,
        forecast_category: input.forecast_category,
        occurred_at_unix_nanos: now_nanos(),
    }) {
        return Ok(rejected_sdk(e));
    }
    let saved = encode(&deal)?;
    let sql = format!(
        "UPDATE crm_sales_deal_records SET state=?,version=?,idem_key=?,idem_digest=?,request_id=?,actor=?,last_action='sales_deals.update' WHERE tenant=? AND id=? AND version=?{}",
        owner_guard(input.owner.as_ref())
    );
    let mut values = vec![
        JsValue::from_str(&saved),
        JsValue::from_f64(deal.version() as f64),
        JsValue::from_str(key),
        JsValue::from_str(&digest),
        JsValue::from_str(trace),
        JsValue::from_str(actor),
        JsValue::from_str(tenant),
        JsValue::from_str(id),
        JsValue::from_f64(input.expected_version as f64),
    ];
    owner_parameters(&mut values, tenant, input.owner.as_ref());
    let result = bind(db, &sql, &values)?.run().await;
    let count = result.as_ref().ok().and_then(changes);
    if let Some(changed) = count.filter(|value| *value > 0) {
        return Ok(Ok(acknowledged(deal, trace, changed)));
    }
    Ok(reconcile(db, tenant, key, &digest, count == Some(0)).await)
}
pub(super) struct LinkOutcome {
    pub(super) delivery_id: String,
    pub(super) applied: bool,
    pub(super) task_id: Option<String>,
}
pub(super) struct AdvanceSaved {
    pub(super) saved: SalesSaved,
    pub(super) link: Option<LinkOutcome>,
}
/// Atomically persists ONE domain stage advancement, its idempotency receipt
/// and an optional original Activities follow-up in the SAME D1 batch.
enum StageMutation {
    Move(StageInput),
    Close(CloseInput),
}
pub(super) async fn advance_typed(
    input: StageInput,
    key: &str,
    db: &D1Database,
    tenant: &str,
    id: &str,
    actor: &str,
    trace: &str,
) -> Result<std::result::Result<AdvanceSaved, MutationFailure>> {
    stage_mutation(
        StageMutation::Move(input),
        key,
        db,
        tenant,
        id,
        actor,
        trace,
    )
    .await
}
pub(super) async fn close_typed(
    input: CloseInput,
    key: &str,
    db: &D1Database,
    tenant: &str,
    id: &str,
    actor: &str,
    trace: &str,
) -> Result<std::result::Result<AdvanceSaved, MutationFailure>> {
    stage_mutation(
        StageMutation::Close(input),
        key,
        db,
        tenant,
        id,
        actor,
        trace,
    )
    .await
}
async fn stage_mutation(
    input: StageMutation,
    key: &str,
    db: &D1Database,
    tenant: &str,
    id: &str,
    actor: &str,
    trace: &str,
) -> Result<std::result::Result<AdvanceSaved, MutationFailure>> {
    if !(8..=120).contains(&key.len()) {
        return Ok(rejected(
            "IDEMPOTENCY_KEY_REQUIRED",
            ErrorCategory::InvalidArgument,
        ));
    }
    // Only client-owned typed input contributes to the digest. The stage at
    // replay time cannot alter the idempotent identity of a CloseDeal request.
    let digest = match &input {
        StageMutation::Move(value) => hash("advance", id, value)?,
        StageMutation::Close(value) => hash("close", id, value)?,
    };
    match prior(db, tenant, key).await {
        Ok(Some(p)) => {
            return Ok(replay(p, &digest).map(|saved| AdvanceSaved { saved, link: None }));
        }
        Ok(None) => (),
        Err(_) => return Ok(Err(unconfirmed(ReceiptUncertainty::ReadFailed))),
    }
    let Some(mut deal) = load(db, tenant, id).await? else {
        return Ok(rejected("NOT_FOUND", ErrorCategory::NotFound));
    };
    let before = deal.clone();
    let expected_version = match &input {
        StageMutation::Move(value) => value.expected_version,
        StageMutation::Close(value) => value.expected_version,
    };
    let outcome = match input {
        StageMutation::Move(value) => {
            if !matches!(value.status, StatusInput::Open) {
                return Ok(rejected(
                    "SALES_CLOSE_SEPARATE_ACTION_REQUIRED",
                    ErrorCategory::InvalidArgument,
                ));
            }
            let Some(catalog_stage) = super::sales_pipelines::active_stage(
                db,
                tenant,
                &value.pipeline_id,
                &value.stage_id,
            )
            .await?
            else {
                return Ok(rejected(
                    "SALES_STAGE_NOT_IN_CATALOG",
                    ErrorCategory::InvalidArgument,
                ));
            };
            let target = match stage(value.pipeline_id, value.stage_id, catalog_stage.ordinal) {
                Ok(x) => x,
                Err(code) => return Ok(rejected(code, ErrorCategory::InvalidArgument)),
            };
            let reason = match value.close_reason_code {
                Some(code) => match ReasonCode::try_new(code) {
                    Ok(v) => Some(v),
                    Err(e) => return Ok(rejected(e.code, ErrorCategory::InvalidArgument)),
                },
                None => None,
            };
            let default_probability =
                match BasisPoints::try_new(catalog_stage.probability_basis_points) {
                    Ok(value) => value,
                    Err(e) => return Ok(rejected(e.code, ErrorCategory::InvalidArgument)),
                };
            deal.advance_stage(AdvanceDealStage {
                expected_version,
                target_stage: target,
                target_stage_probability: Some(default_probability),
                target_status: DealStatus::Open,
                close_reason_code: reason,
                occurred_at_unix_nanos: now_nanos(),
                policy: StageTransitionPolicy::default(),
            })
        }
        StageMutation::Close(value) => {
            if matches!(value.outcome, StatusInput::Open) {
                return Ok(rejected(
                    "SALES_CLOSE_OUTCOME_REQUIRED",
                    ErrorCategory::InvalidArgument,
                ));
            }
            let reason_code = match ReasonCode::try_new(value.reason_code) {
                Ok(v) => v,
                Err(e) => return Ok(rejected_sdk(e)),
            };
            deal.close(CloseDeal {
                expected_version,
                outcome: value.outcome.into(),
                reason_code,
                occurred_at_unix_nanos: now_nanos(),
            })
        }
    };
    if let Err(e) = outcome {
        return Ok(rejected_sdk(e));
    }
    let saved = encode(&deal)?;
    let prepared = match super::sales_activities_link::prepare(
        &before,
        &deal,
        tenant,
        actor,
        trace,
        now_nanos(),
    )
    .await
    {
        Ok(x) => x,
        Err(e) => {
            return Ok(rejected_sdk(e));
        }
    };
    let update = bind(
        db,
        "UPDATE crm_sales_deal_records SET state=?,version=?,idem_key=?,idem_digest=?,request_id=?,actor=?,last_action=? WHERE tenant=? AND id=? AND version=?",
        &[
            JsValue::from_str(&saved),
            JsValue::from_f64(deal.version() as f64),
            JsValue::from_str(key),
            JsValue::from_str(&digest),
            JsValue::from_str(trace),
            JsValue::from_str(actor),
            JsValue::from_str("sales_deals.advance"),
            JsValue::from_str(tenant),
            JsValue::from_str(id),
            JsValue::from_f64(expected_version as f64),
        ],
    )?;
    let mut writes = vec![update];
    if let Some(task) = &prepared.task {
        let account_id = match deal.account() {
            Some(x) => &x.resource_id,
            None => {
                return Ok(rejected(
                    "SALES_LINK_ACCOUNT_REQUIRED",
                    ErrorCategory::InvalidArgument,
                ));
            }
        };
        let task_state = String::from_utf8(
            crm_activities::encode_task_state(task)
                .map_err(|e| worker::Error::RustError(e.code))?,
        )
        .map_err(|e| worker::Error::RustError(e.to_string()))?;
        let task_stmt = bind(
            db,
            "INSERT INTO crm_activity_task_records(tenant,id,account_id,state,version,idem_key,idem_digest,request_id,actor) VALUES(?,?,?,?,?,?,?,?,?)",
            &[
                JsValue::from_str(tenant),
                JsValue::from_str(task.task_id().as_str()),
                JsValue::from_str(account_id),
                JsValue::from_str(&task_state),
                JsValue::from_f64(task.version() as f64),
                JsValue::from_str(&prepared.delivery_id),
                JsValue::from_str(&prepared.event_id),
                JsValue::from_str(trace),
                JsValue::from_str(actor),
            ],
        )?;
        writes.push(task_stmt);
    }
    let link_stmt = bind(
        db,
        "INSERT INTO crm_sales_link_delivery_records(tenant,delivery_id,event_id,deal_id,deal_version,task_id,disposition,state_json,request_id,actor) VALUES(?,?,?,?,?,?,?,?,?,?)",
        &[
            JsValue::from_str(tenant),
            JsValue::from_str(&prepared.delivery_id),
            JsValue::from_str(&prepared.event_id),
            JsValue::from_str(id),
            JsValue::from_f64(deal.version() as f64),
            prepared.task.as_ref().map_or(JsValue::NULL, |task| {
                JsValue::from_str(task.task_id().as_str())
            }),
            JsValue::from_str(if prepared.task.is_some() {
                "applied"
            } else {
                "ignored"
            }),
            JsValue::from_str(&prepared.state_json),
            JsValue::from_str(trace),
            JsValue::from_str(actor),
        ],
    )?;
    writes.push(link_stmt);
    let count = writes.len();
    let result = db.batch(writes).await;
    let changed = result
        .as_ref()
        .ok()
        .filter(|results| {
            results.len() == count && results.iter().all(worker::d1::D1Result::success)
        })
        .and_then(|results| results.first())
        .and_then(changes)
        .filter(|value| *value > 0);
    let Some(changed) = changed else {
        return Ok(reconcile(db, tenant, key, &digest, false)
            .await
            .map(|saved| AdvanceSaved { saved, link: None }));
    };
    let link = Some(LinkOutcome {
        delivery_id: prepared.delivery_id,
        applied: prepared.task.is_some(),
        task_id: prepared
            .task
            .as_ref()
            .map(|x| x.task_id().as_str().to_owned()),
    });
    Ok(Ok(AdvanceSaved {
        saved: acknowledged(deal, trace, changed),
        link,
    }))
}
/// Explicit reopening is one original Rust CAS, trigger-owned receipt,
/// attributed audit and immutable history event. Never emit a stage-change
/// follow-up on reopening a finished Deal.
pub(super) async fn reopen_typed(
    input: ReopenInput,
    key: &str,
    db: &D1Database,
    tenant: &str,
    id: &str,
    actor: &str,
    trace: &str,
) -> Result<std::result::Result<SalesSaved, MutationFailure>> {
    if !(8..=120).contains(&key.len()) {
        return Ok(rejected(
            "IDEMPOTENCY_KEY_REQUIRED",
            ErrorCategory::InvalidArgument,
        ));
    }
    let digest = hash("reopen", id, &input)?;
    match prior(db, tenant, key).await {
        Ok(Some(p)) => return Ok(replay(p, &digest)),
        Ok(None) => (),
        Err(_) => return Ok(Err(unconfirmed(ReceiptUncertainty::ReadFailed))),
    }
    let Some(mut deal) = load(db, tenant, id).await? else {
        return Ok(rejected("NOT_FOUND", ErrorCategory::NotFound));
    };
    // Reopen only into a live, currently ranked catalog stage.
    // The D1 BEFORE UPDATE guard separately closes the catalog race.
    let Some(ordinal) = super::sales_pipelines::active_stage_ordinal(
        db,
        tenant,
        deal.stage().pipeline_id().as_str(),
        deal.stage().stage_id().as_str(),
    )
    .await?
    else {
        return Ok(rejected(
            "SALES_REOPEN_INACTIVE_STAGE",
            ErrorCategory::Conflict,
        ));
    };
    if ordinal != deal.stage().ordinal() {
        return Ok(rejected(
            "SALES_REOPEN_CATALOG_RANK_CONFLICT",
            ErrorCategory::Conflict,
        ));
    }
    let reason_code = match ReasonCode::try_new(&input.reason_code) {
        Ok(reason) => reason,
        Err(e) => return Ok(rejected(e.code, ErrorCategory::InvalidArgument)),
    };
    // Keep the original owner only if still eligible. The retained
    // signed technical creator is not an explicit human assignment.
    let current = match deal.owner() {
        DealOwner::Actor(id) => SalesOwnerScope::Actor(id.to_string()),
        DealOwner::Team(id) => SalesOwnerScope::Team(id.as_str().to_owned()),
    };
    let target = input.owner.as_ref().unwrap_or(&current);
    let eligible = match target {
        SalesOwnerScope::Actor(id) if input.owner.is_none() && id.starts_with("service:") => {
            super::access_members::valid_assignee(db, tenant, id).await?
        }
        _ => eligible_owner(db, tenant, target).await?,
    };
    if !eligible {
        return Ok(rejected(
            "SALES_REOPEN_OWNER_NOT_ELIGIBLE",
            ErrorCategory::Conflict,
        ));
    }
    let replacement = match input
        .owner
        .as_ref()
        .map(SalesOwnerScope::domain_owner)
        .transpose()
    {
        Ok(owner) => owner,
        Err(e) => return Ok(rejected_sdk(e)),
    };
    if let Err(e) = deal.reopen_with_owner(
        ReopenDeal {
            expected_version: input.expected_version,
            reason_code,
            occurred_at_unix_nanos: now_nanos(),
        },
        replacement,
    ) {
        return Ok(rejected_sdk(e));
    }
    let state = encode(&deal)?;
    let sql = format!(
        "UPDATE crm_sales_deal_records SET state=?,version=?,idem_key=?,idem_digest=?,request_id=?,actor=?,last_action='sales_deals.update',last_reason_code=? WHERE tenant=? AND id=? AND version=?{}",
        owner_guard(Some(target))
    );
    let mut values = vec![
        JsValue::from_str(&state),
        JsValue::from_f64(deal.version() as f64),
        JsValue::from_str(key),
        JsValue::from_str(&digest),
        JsValue::from_str(trace),
        JsValue::from_str(actor),
        JsValue::from_str(&input.reason_code),
        JsValue::from_str(tenant),
        JsValue::from_str(id),
        JsValue::from_f64(input.expected_version as f64),
    ];
    owner_parameters(&mut values, tenant, Some(target));
    let result = bind(db, &sql, &values)?.run().await;
    let count = result.as_ref().ok().and_then(changes);
    if let Some(changed) = count.filter(|value| *value > 0) {
        return Ok(Ok(acknowledged(deal, trace, changed)));
    }
    Ok(reconcile(db, tenant, key, &digest, count == Some(0)).await)
}

#[derive(Deserialize)]
pub(super) struct DealEventRow {
    pub(super) deal_version: i64,
    pub(super) action: String,
    pub(super) pipeline_id: String,
    pub(super) previous_stage_id: Option<String>,
    pub(super) stage_id: String,
    pub(super) previous_status: Option<String>,
    pub(super) status: String,
    pub(super) reason_code: Option<String>,
    pub(super) actor: String,
    pub(super) request_id: String,
    pub(super) occurred_at_unix_nanos: String,
    pub(super) catalog_version: Option<i64>,
    pub(super) linked_task_id: Option<String>,
    pub(super) initial_routing: Option<String>,
}

/// A single tenant-scoped keyset stream backed by the D1 composite PK.
/// Related original Activity Task is joined at read time because link delivery
/// is inserted in the same atomic batch *after* the Deal update trigger.
pub(super) async fn list_events(
    db: &D1Database,
    tenant: &str,
    id: &str,
    before: i64,
    size: usize,
) -> Result<(Vec<DealEventRow>, Option<String>)> {
    let mut rows=bind(db,
        "SELECT e.deal_version,e.action,e.pipeline_id,e.previous_stage_id,e.stage_id,e.previous_status,e.status,e.reason_code,e.actor,e.request_id,CAST(e.occurred_at_unix_nanos AS TEXT) AS occurred_at_unix_nanos,e.catalog_version,l.task_id AS linked_task_id,e.initial_routing FROM crm_sales_deal_events e LEFT JOIN crm_sales_link_delivery_records l ON l.tenant=e.tenant AND l.deal_id=e.deal_id AND l.deal_version=e.deal_version WHERE e.tenant=? AND e.deal_id=? AND e.deal_version<CAST(? AS INTEGER) ORDER BY e.deal_version DESC LIMIT ?",
        &[
            JsValue::from_str(tenant),
            JsValue::from_str(id),
            JsValue::from_str(&before.to_string()),
            JsValue::from_f64((size+1) as f64),
        ],
    )?.all().await?.results::<DealEventRow>()?;
    let more = rows.len() > size;
    rows.truncate(size);
    let next = if more {
        rows.last().map(|e| e.deal_version.to_string())
    } else {
        None
    };
    Ok((rows, next))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct PipelineStatusMetric {
    pub stage_id: String,
    pub status: DealStatus,
    pub deal_count: u64,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct PipelineCurrencyMetric {
    pub stage_id: String,
    pub status: DealStatus,
    pub currency_code: String,
    pub total_minor_units: i128,
    // Not rounded: the numerator / 10_000 is the weighted minor-unit amount.
    pub weighted_minor_units_times_basis_points: i128,
    pub amount_deal_count: u64,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct PipelineOperationalMetric {
    pub stage_id: String,
    pub status: DealStatus,
    pub overdue_expected_close_count: u64,
    pub due_next_30_days_count: u64,
    pub due_later_count: u64,
    pub no_expected_close_count: u64,
    pub known_stage_age_count: u64,
    pub total_stage_age_seconds: i128,
    pub overdue_linked_task_count: u64,
}
#[derive(Default)]
struct OperationalCounts {
    overdue: u64,
    due_soon: u64,
    due_later: u64,
    undated: u64,
    known_age: u64,
    age_seconds: i128,
    overdue_tasks: u64,
}
fn count_one(value: &mut u64) -> std::result::Result<(), ReadFailureReason> {
    *value = value
        .checked_add(1)
        .ok_or(ReadFailureReason::SalesMetricsCountOverflow)?;
    Ok(())
}
fn add_money(
    entry: &mut (i128, i128, u64),
    units: i128,
    weighted: i128,
) -> std::result::Result<(), ReadFailureReason> {
    entry.0 = entry
        .0
        .checked_add(units)
        .ok_or(ReadFailureReason::SalesMetricsMoneyOverflow)?;
    entry.1 = entry
        .1
        .checked_add(weighted)
        .ok_or(ReadFailureReason::SalesMetricsWeightOverflow)?;
    count_one(&mut entry.2)
}
#[derive(Clone, Copy)]
pub(super) struct AnalysisPeriod {
    start: CalendarDate,
    end: CalendarDate,
}

impl AnalysisPeriod {
    pub(super) fn new(start: DateInput, end: DateInput) -> std::result::Result<Self, &'static str> {
        let start = typed_date(start)?;
        let end = typed_date(end)?;
        if start > end {
            return Err("SALES_ANALYSIS_PERIOD_REVERSED");
        }
        Ok(Self { start, end })
    }

    fn includes(self, date: CalendarDate) -> bool {
        self.start <= date && date <= self.end
    }
}

#[derive(Default)]
pub(super) struct ClosedOutcomeMetric {
    pub won_count: u64,
    pub lost_count: u64,
    pub unknown_close_count: u64,
}

impl ClosedOutcomeMetric {
    pub(super) fn denominator(&self) -> u64 {
        self.won_count + self.lost_count
    }

    pub(super) fn win_rate_basis_points(&self) -> Option<u32> {
        let denominator = self.denominator();
        (denominator > 0)
            .then(|| (u128::from(self.won_count) * 10_000 / u128::from(denominator)) as u32)
    }
}

pub(super) struct ForecastCategoryMetric {
    pub category: ForecastCategory,
    pub deal_count: u64,
    pub stage_default_count: u64,
    pub manual_count: u64,
}
pub(super) struct ForecastCurrencyMetric {
    pub category: ForecastCategory,
    pub currency_code: String,
    pub total_minor_units: i128,
    pub weighted_minor_units_times_basis_points: i128,
    pub amount_deal_count: u64,
}
#[derive(Default)]
struct ForecastTotals {
    categories: BTreeMap<ForecastCategory, (u64, u64, u64)>,
    money: BTreeMap<(ForecastCategory, String), (i128, i128, u64)>,
}

impl ForecastTotals {
    fn add(&mut self, deal: &Deal) -> std::result::Result<(), ReadFailureReason> {
        let counts = self.categories.entry(deal.forecast_category()).or_default();
        count_one(&mut counts.0)?;
        match deal.probability_source() {
            ProbabilitySource::StageDefault => count_one(&mut counts.1)?,
            ProbabilitySource::Manual => count_one(&mut counts.2)?,
        }
        if let Some(amount) = deal.amount() {
            let units = amount.minor_units();
            let weighted = units
                .checked_mul(i128::from(deal.probability().get()))
                .ok_or(ReadFailureReason::SalesMetricsWeightOverflow)?;
            let entry = self
                .money
                .entry((deal.forecast_category(), amount.currency().to_string()))
                .or_default();
            add_money(entry, units, weighted)?;
        }
        Ok(())
    }

    fn finish(self) -> (Vec<ForecastCategoryMetric>, Vec<ForecastCurrencyMetric>) {
        let categories = self
            .categories
            .into_iter()
            .map(
                |(category, (deal_count, stage_default_count, manual_count))| {
                    ForecastCategoryMetric {
                        category,
                        deal_count,
                        stage_default_count,
                        manual_count,
                    }
                },
            )
            .collect();
        let currencies = self
            .money
            .into_iter()
            .map(
                |((category, currency_code), (total, weighted, count))| ForecastCurrencyMetric {
                    category,
                    currency_code,
                    total_minor_units: total,
                    weighted_minor_units_times_basis_points: weighted,
                    amount_deal_count: count,
                },
            )
            .collect();
        (categories, currencies)
    }
}

pub(super) struct PipelineMetrics {
    pub total_deal_count: u64,
    pub statuses: Vec<PipelineStatusMetric>,
    pub currencies: Vec<PipelineCurrencyMetric>,
    pub operational: Vec<PipelineOperationalMetric>,
    pub forecast_categories: Vec<ForecastCategoryMetric>,
    pub forecast_currencies: Vec<ForecastCurrencyMetric>,
    pub undated_forecast_categories: Vec<ForecastCategoryMetric>,
    pub undated_forecast_currencies: Vec<ForecastCurrencyMetric>,
    pub closed_outcomes: Option<ClosedOutcomeMetric>,
    pub as_of_unix_seconds: i64,
}

// One indexed SELECT provides a single D1 read snapshot of Deals, Tasks
// and events. Stage/page cursors never participate, and monetary arithmetic
// remains exact in the original Rust owner.
pub(super) async fn pipeline_metrics(
    db: &D1Database,
    tenant: &str,
    pipeline_id: &str,
    status: Option<DealStatus>,
    account: Option<&str>,
    period: Option<AnalysisPeriod>,
    owner_filter: Option<&SalesOwnerScope>,
) -> std::result::Result<PipelineMetrics, ReadFailureReason> {
    let mut condition =
        String::from("d.tenant=? AND json_extract(d.state,'$.stage.pipeline_id')=?");
    let mut arguments = vec![JsValue::from_str(tenant), JsValue::from_str(pipeline_id)];
    if let Some(status) = status {
        condition.push_str(" AND json_extract(d.state,'$.status')=?");
        arguments.push(JsValue::from_str(match status {
            DealStatus::Open => "open",
            DealStatus::Won => "won",
            DealStatus::Lost => "lost",
        }));
    }
    if let Some(account) = account {
        condition.push_str(" AND d.account_id=?");
        arguments.push(JsValue::from_str(account));
    }
    append_owner_filter(&mut condition, &mut arguments, owner_filter);
    // Materialize the database clock once; correlated indexed event/task
    // lookups are evaluated inside the same read statement.
    let sql = format!(
        r#"WITH snapshot_clock AS MATERIALIZED (
           SELECT CAST(strftime('%s','now') AS INTEGER)*1000000000
                + CAST(substr(strftime('%f','now'),4,3) AS INTEGER)*1000000 AS nanos
         )
         SELECT CAST(c.nanos AS TEXT) AS as_of_nanos,
           d.state AS state,
           (SELECT CAST(e.occurred_at_unix_nanos AS TEXT)
              FROM crm_sales_deal_events e
             WHERE e.tenant=d.tenant AND e.deal_id=d.id
               AND e.action IN ('created','moved')
             ORDER BY e.deal_version DESC LIMIT 1) AS stage_entered_at_nanos,
           COALESCE((SELECT MAX(CASE
                       WHEN json_type(t.state,'$.due_at_unix_nanos') IS NULL
                         OR json_type(t.state,'$.due_at_unix_nanos')='null' THEN 0
                       WHEN json_type(t.state,'$.due_at_unix_nanos')<>'integer' THEN 2
                       WHEN json_extract(t.state,'$.due_at_unix_nanos') < c.nanos THEN 1
                       ELSE 0 END)
                     FROM crm_sales_link_delivery_records l
                     JOIN crm_activity_task_records t
                       ON t.tenant=l.tenant AND t.id=l.task_id
                    WHERE l.tenant=d.tenant AND l.deal_id=d.id
                      AND json_extract(t.state,'$.status')='open'),0) AS has_overdue_linked_task
         FROM snapshot_clock c CROSS JOIN crm_sales_deal_records d
        WHERE {condition}"#
    );
    // Resource-limit errors fail the request; never emit a truncated summary.
    // Empty results have no persisted facts to date.
    let rows = bind(db, &sql, &arguments)?
        .all()
        .await?
        .results::<PipelineMoneyRow>()?;
    let as_of_nanos = match rows.first() {
        Some(row) => row
            .as_of_nanos
            .parse::<i64>()
            .map_err(|_| ReadFailureReason::SalesMetricsSnapshotTimeInvalid)?,
        None => now_nanos(),
    };
    let as_of_text = as_of_nanos.to_string();
    let as_of = crm_core_contracts::UtcInstant::from_unix_nanos(as_of_nanos);
    let as_of_unix_seconds = as_of.proto_parts().0;
    let today = as_of.utc_date();
    let mut total_count = 0u64;
    let mut status_groups = BTreeMap::<(String, String), u64>::new();
    let mut money_groups = BTreeMap::<(String, String, String), (i128, i128, u64)>::new();
    let mut operational_groups = BTreeMap::<(String, String), OperationalCounts>::new();
    let mut forecast = ForecastTotals::default();
    let mut undated_forecast = ForecastTotals::default();
    let mut closed_outcomes = status.is_none().then(ClosedOutcomeMetric::default);
    for row in &rows {
        if row.as_of_nanos != as_of_text {
            return Err(ReadFailureReason::SalesMetricsSnapshotTimeInvalid);
        }
        let deal = decode(&row.state).map_err(|_| ReadFailureReason::SalesStateDecodeFailed)?;
        let stage_id = deal.stage().stage_id().as_str();
        let status = deal.status();
        let status_key = match status {
            DealStatus::Open => "open",
            DealStatus::Won => "won",
            DealStatus::Lost => "lost",
        };
        total_count = total_count
            .checked_add(1)
            .ok_or(ReadFailureReason::SalesMetricsCountOverflow)?;
        let status_group = status_groups
            .entry((stage_id.to_owned(), status_key.to_owned()))
            .or_default();
        *status_group = status_group
            .checked_add(1)
            .ok_or(ReadFailureReason::SalesMetricsCountOverflow)?;
        if status == DealStatus::Open {
            match deal.expected_close_date() {
                None => {
                    undated_forecast.add(&deal)?;
                    if period.is_none() {
                        forecast.add(&deal)?;
                    }
                }
                Some(date) => {
                    if period.is_none_or(|period| period.includes(date)) {
                        forecast.add(&deal)?;
                    }
                }
            }
            let group = operational_groups
                .entry((stage_id.to_owned(), status_key.to_owned()))
                .or_default();
            match deal.expected_close_date() {
                None => count_one(&mut group.undated)?,
                Some(date) => {
                    let delta = today.days_until(date);
                    if delta < 0 {
                        count_one(&mut group.overdue)?;
                    } else if delta <= 30 {
                        count_one(&mut group.due_soon)?;
                    } else {
                        count_one(&mut group.due_later)?;
                    }
                }
            }
            // Pre-0013 Deals can have no immutable entry event. Never
            // synthesize a historical timestamp from updated_at.
            if let Some(start) = &row.stage_entered_at_nanos {
                let started = start
                    .parse::<i64>()
                    .map_err(|_| ReadFailureReason::SalesMetricsStageTimeInvalid)?;
                if started <= 0 {
                    return Err(ReadFailureReason::SalesMetricsStageTimeInvalid);
                }
                // A Worker event may be ahead of the D1 clock; do not invent age.
                if started <= as_of_nanos {
                    count_one(&mut group.known_age)?;
                    group.age_seconds = group
                        .age_seconds
                        .checked_add(i128::from((as_of_nanos - started) / 1_000_000_000))
                        .ok_or(ReadFailureReason::SalesMetricsStageTimeOverflow)?;
                }
            }
            match row.has_overdue_linked_task {
                1 => count_one(&mut group.overdue_tasks)?,
                0 => (),
                _ => {
                    return Err(ReadFailureReason::SalesMetricsTaskFlagInvalid);
                }
            }
        } else if let Some(counts) = &mut closed_outcomes {
            match deal.close_outcome() {
                None => count_one(&mut counts.unknown_close_count)?,
                Some(outcome) => {
                    // Domain close timestamp is an instant, never a locally
                    // interpreted date or another Gregorian implementation.
                    let closed_at = outcome.closed_at_unix_nanos();
                    let date =
                        crm_core_contracts::UtcInstant::from_unix_nanos(closed_at).utc_date();
                    if period.is_none_or(|period| period.includes(date)) {
                        match status {
                            DealStatus::Won => count_one(&mut counts.won_count)?,
                            DealStatus::Lost => count_one(&mut counts.lost_count)?,
                            DealStatus::Open => unreachable!("closed outcome branch"),
                        }
                    }
                }
            }
        }
        if let Some(amount) = deal.amount() {
            let units = amount.minor_units();
            let weighted = units
                .checked_mul(i128::from(deal.probability().get()))
                .ok_or(ReadFailureReason::SalesMetricsWeightOverflow)?;
            let entry = money_groups
                .entry((
                    stage_id.to_owned(),
                    status_key.to_owned(),
                    amount.currency().to_string(),
                ))
                .or_default();
            add_money(entry, units, weighted)?;
        }
    }
    fn persisted_status(raw: &str) -> DealStatus {
        match raw {
            "open" => DealStatus::Open,
            "won" => DealStatus::Won,
            "lost" => DealStatus::Lost,
            _ => unreachable!("Validated during the original D1 row scan"),
        }
    }
    let (forecast_categories, forecast_currencies) = forecast.finish();
    let (undated_forecast_categories, undated_forecast_currencies) = undated_forecast.finish();
    Ok(PipelineMetrics {
        total_deal_count: total_count,
        statuses: status_groups
            .into_iter()
            .map(|((stage_id, kind), deal_count)| PipelineStatusMetric {
                stage_id,
                status: persisted_status(&kind),
                deal_count,
            })
            .collect(),
        currencies: money_groups
            .into_iter()
            .map(
                |(
                    (stage_id, kind, currency_code),
                    (total_minor_units, weighted_minor_units_times_basis_points, amount_deal_count),
                )| PipelineCurrencyMetric {
                    stage_id,
                    status: persisted_status(&kind),
                    currency_code,
                    total_minor_units,
                    weighted_minor_units_times_basis_points,
                    amount_deal_count,
                },
            )
            .collect(),
        operational: operational_groups
            .into_iter()
            .map(|((stage_id, kind), c)| PipelineOperationalMetric {
                stage_id,
                status: persisted_status(&kind),
                overdue_expected_close_count: c.overdue,
                due_next_30_days_count: c.due_soon,
                due_later_count: c.due_later,
                no_expected_close_count: c.undated,
                known_stage_age_count: c.known_age,
                total_stage_age_seconds: c.age_seconds,
                overdue_linked_task_count: c.overdue_tasks,
            })
            .collect(),
        forecast_categories,
        forecast_currencies,
        undated_forecast_categories,
        undated_forecast_currencies,
        closed_outcomes,
        as_of_unix_seconds,
    })
}

/// Both count and currency sums refer to the full tenant/pipeline/stage filter,
/// never the current page. Totals accumulate exactly in Rust because SQLite
/// SUM(int64) can overflow and SUM(REAL) silently loses minor-unit precision.
pub(super) struct StageCurrencyTotal {
    pub currency_code: String,
    pub total_minor_units: i128,
    pub amount_deal_count: u64,
}

/// Every column has an independent keyset cursor.
pub(super) struct StageListQuery<'a> {
    pub pipeline_id: &'a str,
    pub stage_id: &'a str,
    pub after: &'a str,
    pub limit: usize,
    pub status: Option<DealStatus>,
    pub account: Option<&'a str>,
    pub owner_filter: Option<&'a SalesOwnerScope>,
}
pub(super) async fn list_stage_page(
    db: &D1Database,
    tenant: &str,
    query: StageListQuery<'_>,
) -> std::result::Result<(Vec<Deal>, Option<String>, u64, Vec<StageCurrencyTotal>), ReadFailureReason>
{
    let StageListQuery {
        pipeline_id,
        stage_id,
        after,
        limit,
        status,
        account,
        owner_filter,
    } = query;
    let mut condition = String::from(
        "tenant=? AND json_extract(state,'$.stage.pipeline_id')=? AND json_extract(state,'$.stage.stage_id')=?",
    );
    let mut arguments = vec![
        JsValue::from_str(tenant),
        JsValue::from_str(pipeline_id),
        JsValue::from_str(stage_id),
    ];
    if let Some(status) = status {
        condition.push_str(" AND json_extract(state,'$.status')=?");
        arguments.push(JsValue::from_str(match status {
            DealStatus::Open => "open",
            DealStatus::Won => "won",
            DealStatus::Lost => "lost",
        }));
    }
    if let Some(account) = account {
        condition.push_str(" AND account_id=?");
        arguments.push(JsValue::from_str(account));
    }
    append_owner_filter(&mut condition, &mut arguments, owner_filter);
    // One indexed snapshot supplies counts, exact totals and the requested
    // keyset page. The cursor limits returned cards, not the summary facts.
    // There is no second read that can observe another committed Deal state.
    let sql = format!("SELECT id,state FROM crm_sales_deal_records WHERE {condition} ORDER BY id");
    let rows = bind(db, &sql, &arguments)?
        .all()
        .await?
        .results::<PagedRow>()?;
    let total = u64::try_from(rows.len()).map_err(|_| ReadFailureReason::SalesBoardCountInvalid)?;
    let mut summary = BTreeMap::<String, (i128, u64)>::new();
    let mut page = Vec::with_capacity(limit + 1);
    for row in &rows {
        let deal = decode(&row.state).map_err(|_| ReadFailureReason::SalesStateDecodeFailed)?;
        if let Some(amount) = deal.amount() {
            let entry = summary
                .entry(amount.currency().to_string())
                .or_insert((0, 0));
            entry.0 = entry
                .0
                .checked_add(amount.minor_units())
                .ok_or(ReadFailureReason::SalesBoardAmountOverflow)?;
            entry.1 = entry
                .1
                .checked_add(1)
                .ok_or(ReadFailureReason::SalesBoardAmountCountOverflow)?;
        }
        if row.id.as_str() > after && page.len() <= limit {
            page.push((row.id.clone(), deal));
        }
    }
    let currency_totals = summary
        .into_iter()
        .map(
            |(currency_code, (total_minor_units, amount_deal_count))| StageCurrencyTotal {
                currency_code,
                total_minor_units,
                amount_deal_count,
            },
        )
        .collect();
    let more = page.len() > limit;
    page.truncate(limit);
    let next = if more {
        page.last().map(|(id, _)| id.clone())
    } else {
        None
    };
    let deals = page.into_iter().map(|(_, deal)| deal).collect();
    Ok((deals, next, total, currency_totals))
}

// One typed owner-local list query, parallel to StageListQuery above.
// Grouping the filters keeps the Rust API readable as Sales grows.
pub(super) struct DealListQuery<'a> {
    pub after: &'a str,
    pub limit: usize,
    pub pipeline: Option<&'a str>,
    pub status: Option<DealStatus>,
    pub account: Option<&'a str>,
    pub owner_filter: Option<&'a SalesOwnerScope>,
    pub search: &'a str,
}
pub(super) async fn list_page_typed(
    db: &D1Database,
    tenant: &str,
    query: DealListQuery<'_>,
) -> Result<(Vec<Deal>, Option<String>)> {
    let DealListQuery {
        after,
        limit,
        pipeline,
        status,
        account,
        search,
        owner_filter,
    } = query;
    let mut args = vec![JsValue::from_str(tenant), JsValue::from_str(after)];
    let mut sql =
        String::from("SELECT id,state FROM crm_sales_deal_records WHERE tenant=? AND id>?");
    if let Some(pipeline) = pipeline {
        sql.push_str(" AND json_extract(state,'$.stage.pipeline_id')=?");
        args.push(JsValue::from_str(pipeline));
    }
    if let Some(status) = status {
        sql.push_str(" AND json_extract(state,'$.status')=?");
        args.push(JsValue::from_str(match status {
            DealStatus::Open => "open",
            DealStatus::Won => "won",
            DealStatus::Lost => "lost",
        }));
    }
    if let Some(account) = account {
        sql.push_str(" AND account_id=?");
        args.push(JsValue::from_str(account));
    }
    append_owner_filter(&mut sql, &mut args, owner_filter);
    // Literal substring matching (including %, _, and quotes) is performed
    // BEFORE keyset pagination. SQL params protect both name and ID search.
    // The original Deal state remains the sole owner of the name.
    if !search.is_empty() {
        sql.push_str(" AND (instr(json_extract(state,'$.name'),?)>0 OR instr(id,?)>0)");
        args.push(JsValue::from_str(search));
        args.push(JsValue::from_str(search));
    }
    sql.push_str(" ORDER BY id LIMIT ?");
    args.push(JsValue::from_f64((limit + 1) as f64));
    let mut rows = bind(db, &sql, &args)?.all().await?.results::<PagedRow>()?;
    let more = rows.len() > limit;
    rows.truncate(limit);
    let next = if more {
        rows.last().map(|x| x.id.clone())
    } else {
        None
    };
    let deals = rows
        .iter()
        .map(|x| decode(&x.state))
        .collect::<Result<Vec<_>>>()?;
    Ok((deals, next))
}
