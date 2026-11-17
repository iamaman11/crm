//! Cloudflare native HTTP/D1 boundary for the ORIGINAL crm-sales Deal aggregate.
//! Stage progression, closing rules, money and version invariants belong to
//! crm-sales and crm-core-contracts, never the test pilot's crm_deals model.
use super::{bind, now_nanos};
use crm_core_contracts::{BasisPoints, CalendarDate, CurrencyCode, Money, Patch};
use crm_module_sdk::{ActorId, ErrorCategory, RecordId, ResourceRef};
use crm_sales::{
    AdvanceDealStage, CloseDeal, CreateDeal, Deal, DealOwner, DealStage, DealStatus, PipelineId,
    ReasonCode, ReopenDeal, StageId, StageTransitionPolicy, UpdateDeal, decode_deal_state,
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
    pub(super) probability_bps: u16,
    // Minor units are exact integers. Floating point money is never accepted.
    pub(super) amount_minor_units: Option<i64>,
    pub(super) currency: Option<String>,
    pub(super) primary_contact_id: Option<String>,
    pub(super) expected_close_date: Option<DateInput>,
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
    pub(super) primary_contact: FieldChange<String>,
    pub(super) expected_close_date: FieldChange<DateInput>,
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

// Only this indexed money projection is needed for a full-stage summary.
// Do not deserialize an entire original Deal for every one of 1000+ rows.
#[derive(Deserialize)]
struct StageMoneyRow {
    id: String,
    currency_code: String,
    minor_units: String,
}

// A compact D1 projection of the original Deal state. No duplicate metrics
// store or parallel Deal owner: summary is derived from the canonical records.
#[derive(Deserialize)]
struct PipelineMoneyRow {
    id: String,
    stage_id: String,
    status: String,
    currency_code: Option<String>,
    minor_units: Option<String>,
    probability_basis_points: i64,
}
#[derive(Deserialize)]
struct Receipt {
    digest: String,
    state: String,
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
        "SELECT digest,state FROM crm_sales_deal_receipts WHERE tenant=? AND key=?",
        &[JsValue::from_str(tenant), JsValue::from_str(k)],
    )?
    .first::<Receipt>(None)
    .await
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
}
pub(super) struct SalesFailure {
    pub(super) code: String,
    pub(super) status: u16,
}
fn rejected(code: impl Into<String>, status: u16) -> std::result::Result<SalesSaved, SalesFailure> {
    Err(SalesFailure {
        code: code.into(),
        status,
    })
}
pub(super) async fn create_typed(
    input: CreateInput,
    k: &str,
    db: &D1Database,
    tenant: &str,
    actor: &str,
    trace: &str,
) -> Result<std::result::Result<SalesSaved, SalesFailure>> {
    if !(8..=120).contains(&k.len()) {
        return Ok(rejected("IDEMPOTENCY_KEY_REQUIRED", 422));
    }
    let digest = hash("create", &input.id, &input)?;
    if let Some(p) = prior(db, tenant, k).await? {
        if p.digest != digest {
            return Ok(rejected("IDEMPOTENCY_CONFLICT", 409));
        }
        return Ok(Ok(SalesSaved {
            deal: decode(&p.state)?,
            replayed: true,
        }));
    }
    let id = match RecordId::try_new(&input.id) {
        Ok(x) => x,
        Err(_) => return Ok(rejected("SALES_DEAL_ID_INVALID", 422)),
    };
    let account_id = match RecordId::try_new(&input.account_id) {
        Ok(x) => x,
        Err(_) => return Ok(rejected("SALES_ACCOUNT_ID_INVALID", 422)),
    };
    if !account_exists(db, tenant, account_id.as_str()).await? {
        return Ok(rejected("SALES_ACCOUNT_NOT_FOUND", 422));
    }
    let owner = match ActorId::try_new(actor) {
        Ok(x) => DealOwner::Actor(x),
        Err(_) => return Ok(rejected("SALES_OWNER_INVALID", 422)),
    };
    let Some(ordinal) = super::sales_pipelines::active_stage_ordinal(
        db,
        tenant,
        &input.pipeline_id,
        &input.stage_id,
    )
    .await?
    else {
        return Ok(rejected("SALES_STAGE_NOT_IN_CATALOG", 422));
    };
    let stage = match stage(input.pipeline_id, input.stage_id, ordinal) {
        Ok(x) => x,
        Err(code) => return Ok(rejected(code, 422)),
    };
    let probability = match BasisPoints::try_new(input.probability_bps) {
        Ok(x) => x,
        Err(e) => return Ok(rejected(e.code, 422)),
    };
    let amount = match (input.amount_minor_units, input.currency) {
        (None, None) => None,
        (Some(units), Some(currency)) => {
            let currency = match CurrencyCode::try_new(currency) {
                Ok(x) => x,
                Err(e) => return Ok(rejected(e.code, 422)),
            };
            Some(Money::new(i128::from(units), currency))
        }
        _ => return Ok(rejected("SALES_AMOUNT_CURRENCY_PAIR_REQUIRED", 422)),
    };
    let primary_contact = match input.primary_contact_id.as_ref() {
        None => None,
        Some(id) => match valid_contact(db, tenant, account_id.as_str(), id).await? {
            Ok(v) => Some(v),
            Err(code) => return Ok(rejected(code, 422)),
        },
    };
    let expected_close_date = match input.expected_close_date {
        None => None,
        Some(date) => match typed_date(date) {
            Ok(v) => Some(v),
            Err(code) => return Ok(rejected(code, 422)),
        },
    };
    let deal = match Deal::create(CreateDeal {
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
        occurred_at_unix_nanos: now_nanos(),
    }) {
        Ok(x) => x,
        Err(e) => return Ok(rejected(e.code, 422)),
    };
    let saved = encode(&deal)?;
    let r=bind(db,"INSERT INTO crm_sales_deal_records(tenant,id,account_id,state,version,idem_key,idem_digest,request_id,actor) VALUES(?,?,?,?,?,?,?,?,?)",
        &[JsValue::from_str(tenant),JsValue::from_str(deal.deal_id().as_str()),
          JsValue::from_str(account_id.as_str()),JsValue::from_str(&saved),
          JsValue::from_f64(deal.version() as f64),JsValue::from_str(k),
          JsValue::from_str(&digest),JsValue::from_str(trace),JsValue::from_str(actor)])?.run().await;
    if r.is_err() {
        return Ok(rejected("SALES_DEAL_CREATE_CONFLICT", 409));
    }
    Ok(Ok(SalesSaved {
        deal,
        replayed: false,
    }))
}
pub(super) async fn update_typed(
    input: UpdateInput,
    key: &str,
    db: &D1Database,
    tenant: &str,
    id: &str,
    actor: &str,
    trace: &str,
) -> Result<std::result::Result<SalesSaved, SalesFailure>> {
    if !(8..=120).contains(&key.len()) {
        return Ok(rejected("IDEMPOTENCY_KEY_REQUIRED", 422));
    }
    let digest = hash("update", id, &input)?;
    if let Some(p) = prior(db, tenant, key).await? {
        if p.digest != digest {
            return Ok(rejected("IDEMPOTENCY_CONFLICT", 409));
        }
        return Ok(Ok(SalesSaved {
            deal: decode(&p.state)?,
            replayed: true,
        }));
    }
    let Some(mut deal) = load(db, tenant, id).await? else {
        return Ok(rejected("NOT_FOUND", 404));
    };
    if input.clear_name == Some(true) && input.name.is_some() {
        return Ok(rejected("SALES_NAME_UPDATE_CONFLICT", 422));
    }
    let name = if input.clear_name == Some(true) {
        Patch::Clear
    } else {
        input.name.map_or(Patch::Keep, Patch::Set)
    };
    if input.clear_amount == Some(true)
        && (input.amount_minor_units.is_some() || input.currency.is_some())
    {
        return Ok(rejected("SALES_AMOUNT_UPDATE_CONFLICT", 422));
    }
    let amount = if input.clear_amount == Some(true) {
        Patch::Clear
    } else {
        match (input.amount_minor_units, input.currency) {
            (None, None) => Patch::Keep,
            (Some(units), Some(currency)) => {
                let currency = match CurrencyCode::try_new(currency) {
                    Ok(x) => x,
                    Err(e) => return Ok(rejected(e.code, 422)),
                };
                Patch::Set(Money::new(i128::from(units), currency))
            }
            _ => return Ok(rejected("SALES_AMOUNT_CURRENCY_PAIR_REQUIRED", 422)),
        }
    };
    let probability = match input.probability_bps {
        Some(v) => match BasisPoints::try_new(v) {
            Ok(x) => Patch::Set(x),
            Err(e) => return Ok(rejected(e.code, 422)),
        },
        None => Patch::Keep,
    };
    let contact_patch = match input.primary_contact {
        FieldChange::Keep => Patch::Keep,
        FieldChange::Clear => Patch::Clear,
        FieldChange::Set(id) => {
            let Some(account) = deal.account() else {
                return Ok(rejected("SALES_ACCOUNT_NOT_FOUND", 422));
            };
            match valid_contact(db, tenant, &account.resource_id, &id).await? {
                Ok(v) => Patch::Set(v),
                Err(code) => return Ok(rejected(code, 422)),
            }
        }
    };
    let date_patch = match input.expected_close_date {
        FieldChange::Keep => Patch::Keep,
        FieldChange::Clear => Patch::Clear,
        FieldChange::Set(date) => match typed_date(date) {
            Ok(v) => Patch::Set(v),
            Err(code) => return Ok(rejected(code, 422)),
        },
    };
    if matches!(name, Patch::Keep)
        && matches!(amount, Patch::Keep)
        && matches!(probability, Patch::Keep)
        && matches!(contact_patch, Patch::Keep)
        && matches!(date_patch, Patch::Keep)
    {
        return Ok(rejected("SALES_EMPTY_UPDATE", 422));
    }
    if let Err(e) = deal.apply_update(UpdateDeal {
        expected_version: input.expected_version,
        name,
        owner: Patch::Keep,
        account: Patch::Keep,
        primary_contact: contact_patch,
        amount,
        expected_close_date: date_patch,
        probability,
        occurred_at_unix_nanos: now_nanos(),
    }) {
        return Ok(rejected(
            e.code,
            if e.category == ErrorCategory::Conflict {
                409
            } else {
                422
            },
        ));
    }
    let saved = encode(&deal)?;
    let result=bind(db,"UPDATE crm_sales_deal_records SET state=?,version=?,idem_key=?,idem_digest=?,request_id=?,actor=?,last_action='sales_deals.update' WHERE tenant=? AND id=? AND version=?",
        &[JsValue::from_str(&saved),JsValue::from_f64(deal.version() as f64),
          JsValue::from_str(key),JsValue::from_str(&digest),JsValue::from_str(trace),
          JsValue::from_str(actor),JsValue::from_str(tenant),JsValue::from_str(id),
          JsValue::from_f64(input.expected_version as f64)])?.run().await;
    let Ok(result) = result else {
        return Ok(rejected("SALES_DEAL_UPDATE_CONFLICT", 409));
    };
    if result.meta()?.and_then(|m| m.changes).unwrap_or(0) == 0 {
        return Ok(rejected("SALES_DEAL_VERSION_CONFLICT", 409));
    }
    Ok(Ok(SalesSaved {
        deal,
        replayed: false,
    }))
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
fn advance_rejected(
    code: impl Into<String>,
    status: u16,
) -> std::result::Result<AdvanceSaved, SalesFailure> {
    Err(SalesFailure {
        code: code.into(),
        status,
    })
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
) -> Result<std::result::Result<AdvanceSaved, SalesFailure>> {
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
) -> Result<std::result::Result<AdvanceSaved, SalesFailure>> {
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
) -> Result<std::result::Result<AdvanceSaved, SalesFailure>> {
    if !(8..=120).contains(&key.len()) {
        return Ok(advance_rejected("IDEMPOTENCY_KEY_REQUIRED", 422));
    }
    // Only client-owned typed input contributes to the digest. The stage at
    // replay time cannot alter the idempotent identity of a CloseDeal request.
    let digest = match &input {
        StageMutation::Move(value) => hash("advance", id, value)?,
        StageMutation::Close(value) => hash("close", id, value)?,
    };
    if let Some(p) = prior(db, tenant, key).await? {
        if p.digest != digest {
            return Ok(advance_rejected("IDEMPOTENCY_CONFLICT", 409));
        }
        return Ok(Ok(AdvanceSaved {
            saved: SalesSaved {
                deal: decode(&p.state)?,
                replayed: true,
            },
            link: None,
        }));
    }
    let Some(mut deal) = load(db, tenant, id).await? else {
        return Ok(advance_rejected("NOT_FOUND", 404));
    };
    let before = deal.clone();
    let expected_version = match &input {
        StageMutation::Move(value) => value.expected_version,
        StageMutation::Close(value) => value.expected_version,
    };
    let outcome = match input {
        StageMutation::Move(value) => {
            if !matches!(value.status, StatusInput::Open) {
                return Ok(advance_rejected(
                    "SALES_CLOSE_SEPARATE_ACTION_REQUIRED",
                    422,
                ));
            }
            let Some(ordinal) = super::sales_pipelines::active_stage_ordinal(
                db,
                tenant,
                &value.pipeline_id,
                &value.stage_id,
            )
            .await?
            else {
                return Ok(advance_rejected("SALES_STAGE_NOT_IN_CATALOG", 422));
            };
            let target = match stage(value.pipeline_id, value.stage_id, ordinal) {
                Ok(x) => x,
                Err(code) => return Ok(advance_rejected(code, 422)),
            };
            let reason = match value.close_reason_code {
                Some(code) => match ReasonCode::try_new(code) {
                    Ok(v) => Some(v),
                    Err(e) => return Ok(advance_rejected(e.code, 422)),
                },
                None => None,
            };
            deal.advance_stage(AdvanceDealStage {
                expected_version,
                target_stage: target,
                target_status: DealStatus::Open,
                close_reason_code: reason,
                occurred_at_unix_nanos: now_nanos(),
                policy: StageTransitionPolicy::default(),
            })
        }
        StageMutation::Close(value) => {
            if matches!(value.outcome, StatusInput::Open) {
                return Ok(advance_rejected("SALES_CLOSE_OUTCOME_REQUIRED", 422));
            }
            let reason_code = match ReasonCode::try_new(value.reason_code) {
                Ok(v) => v,
                Err(e) => return Ok(advance_rejected(e.code, 422)),
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
        return Ok(advance_rejected(
            e.code,
            if e.category == ErrorCategory::Conflict {
                409
            } else {
                422
            },
        ));
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
            return Ok(advance_rejected(
                e.code,
                if e.category == ErrorCategory::Conflict {
                    409
                } else {
                    422
                },
            ));
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
            None => return Ok(advance_rejected("SALES_LINK_ACCOUNT_REQUIRED", 422)),
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
    if db.batch(writes).await.is_err() {
        return Ok(advance_rejected("SALES_LINK_ATOMIC_COMMIT_CONFLICT", 409));
    }
    let link = Some(LinkOutcome {
        delivery_id: prepared.delivery_id,
        applied: prepared.task.is_some(),
        task_id: prepared
            .task
            .as_ref()
            .map(|x| x.task_id().as_str().to_owned()),
    });
    Ok(Ok(AdvanceSaved {
        saved: SalesSaved {
            deal,
            replayed: false,
        },
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
) -> Result<std::result::Result<SalesSaved, SalesFailure>> {
    if !(8..=120).contains(&key.len()) {
        return Ok(rejected("IDEMPOTENCY_KEY_REQUIRED", 422));
    }
    let digest = hash("reopen", id, &input)?;
    if let Some(p) = prior(db, tenant, key).await? {
        if p.digest != digest {
            return Ok(rejected("IDEMPOTENCY_CONFLICT", 409));
        }
        return Ok(Ok(SalesSaved {
            deal: decode(&p.state)?,
            replayed: true,
        }));
    }
    let Some(mut deal) = load(db, tenant, id).await? else {
        return Ok(rejected("NOT_FOUND", 404));
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
        return Ok(rejected("SALES_REOPEN_INACTIVE_STAGE", 409));
    };
    if ordinal != deal.stage().ordinal() {
        return Ok(rejected("SALES_REOPEN_CATALOG_RANK_CONFLICT", 409));
    }
    let reason_code = match ReasonCode::try_new(&input.reason_code) {
        Ok(reason) => reason,
        Err(e) => return Ok(rejected(e.code, 422)),
    };
    if let Err(e) = deal.reopen(ReopenDeal {
        expected_version: input.expected_version,
        reason_code,
        occurred_at_unix_nanos: now_nanos(),
    }) {
        return Ok(rejected(
            e.code,
            if e.category == ErrorCategory::Conflict {
                409
            } else {
                422
            },
        ));
    }
    let state = encode(&deal)?;
    let result = bind(db,
        "UPDATE crm_sales_deal_records SET state=?,version=?,idem_key=?,idem_digest=?,request_id=?,actor=?,last_action='sales_deals.update',last_reason_code=? WHERE tenant=? AND id=? AND version=?",
        &[
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
        ],
    )?.run().await;
    let Ok(result) = result else {
        return Ok(rejected("SALES_DEAL_REOPEN_CONFLICT", 409));
    };
    if result.meta()?.and_then(|m| m.changes).unwrap_or(0) == 0 {
        return Ok(rejected("SALES_DEAL_VERSION_CONFLICT", 409));
    }
    Ok(Ok(SalesSaved {
        deal,
        replayed: false,
    }))
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
        "SELECT e.deal_version,e.action,e.pipeline_id,e.previous_stage_id,e.stage_id,e.previous_status,e.status,e.reason_code,e.actor,e.request_id,CAST(e.occurred_at_unix_nanos AS TEXT) AS occurred_at_unix_nanos,e.catalog_version,l.task_id AS linked_task_id FROM crm_sales_deal_events e LEFT JOIN crm_sales_link_delivery_records l ON l.tenant=e.tenant AND l.deal_id=e.deal_id AND l.deal_version=e.deal_version WHERE e.tenant=? AND e.deal_id=? AND e.deal_version<CAST(? AS INTEGER) ORDER BY e.deal_version DESC LIMIT ?",
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

#[derive(Deserialize)]
struct CountRow {
    total: i64,
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
pub(super) struct PipelineMetrics {
    pub total_deal_count: u64,
    pub statuses: Vec<PipelineStatusMetric>,
    pub currencies: Vec<PipelineCurrencyMetric>,
}

// One indexed keyset scan over the full filtered tenant pipeline. Stage
// pagination cursors never participate. All monetary arithmetic stays integer
// in original Rust, including sums above SQLite signed int64 SUM limits.
pub(super) async fn pipeline_metrics(
    db: &D1Database,
    tenant: &str,
    pipeline_id: &str,
    status: Option<DealStatus>,
    account: Option<&str>,
) -> Result<PipelineMetrics> {
    let mut condition =
        String::from("tenant=? AND json_extract(state,'$.stage.pipeline_id')=? AND id>?");
    let mut optional = String::new();
    let mut filters = Vec::<JsValue>::new();
    if let Some(status) = status {
        optional.push_str(" AND json_extract(state,'$.status')=?");
        filters.push(JsValue::from_str(match status {
            DealStatus::Open => "open",
            DealStatus::Won => "won",
            DealStatus::Lost => "lost",
        }));
    }
    if let Some(account) = account {
        optional.push_str(" AND account_id=?");
        filters.push(JsValue::from_str(account));
    }
    condition.push_str(&optional);
    let sql = format!(
        "SELECT id,json_extract(state,'$.stage.stage_id') AS stage_id,\
         json_extract(state,'$.status') AS status,\
         json_extract(state,'$.amount.currency') AS currency_code,\
         CAST(json_extract(state,'$.amount.minor_units') AS TEXT) AS minor_units,\
         CAST(json_extract(state,'$.probability_basis_points') AS INTEGER) AS probability_basis_points \
         FROM crm_sales_deal_records WHERE {condition} ORDER BY id LIMIT ?"
    );
    const METRICS_BATCH: usize = 256;
    let mut after = String::new();
    let mut total_count = 0u64;
    let mut status_groups = BTreeMap::<(String, String), u64>::new();
    let mut money_groups = BTreeMap::<(String, String, String), (i128, i128, u64)>::new();
    loop {
        let mut arguments = vec![
            JsValue::from_str(tenant),
            JsValue::from_str(pipeline_id),
            JsValue::from_str(&after),
        ];
        arguments.extend(filters.iter().cloned());
        arguments.push(JsValue::from_f64(METRICS_BATCH as f64));
        let rows = bind(db, &sql, &arguments)?
            .all()
            .await?
            .results::<PipelineMoneyRow>()?;
        if rows.is_empty() {
            break;
        }
        for row in &rows {
            let status = match row.status.as_str() {
                "open" => DealStatus::Open,
                "won" => DealStatus::Won,
                "lost" => DealStatus::Lost,
                _ => {
                    return Err(worker::Error::RustError(
                        "SALES_METRICS_STATUS_INVALID".into(),
                    ));
                }
            };
            StageId::try_new(row.stage_id.clone())
                .map_err(|e| worker::Error::RustError(e.code.to_owned()))?;
            total_count = total_count
                .checked_add(1)
                .ok_or_else(|| worker::Error::RustError("SALES_METRICS_COUNT_OVERFLOW".into()))?;
            let status_group = status_groups
                .entry((row.stage_id.clone(), row.status.clone()))
                .or_default();
            *status_group = status_group
                .checked_add(1)
                .ok_or_else(|| worker::Error::RustError("SALES_METRICS_COUNT_OVERFLOW".into()))?;
            let bps = u16::try_from(row.probability_basis_points).map_err(|_| {
                worker::Error::RustError("SALES_METRICS_PROBABILITY_INVALID".into())
            })?;
            if bps > 10_000 {
                return Err(worker::Error::RustError(
                    "SALES_METRICS_PROBABILITY_INVALID".into(),
                ));
            }
            match (&row.currency_code, &row.minor_units) {
                (None, None) => {}
                (Some(code), Some(raw_units)) => {
                    let currency = CurrencyCode::try_new(code.clone())
                        .map_err(|e| worker::Error::RustError(e.code.to_owned()))?;
                    let units = raw_units.parse::<i64>().map_err(|_| {
                        worker::Error::RustError("SALES_METRICS_MONEY_INVALID".into())
                    })?;
                    if units < 0 {
                        return Err(worker::Error::RustError(
                            "SALES_METRICS_MONEY_INVALID".into(),
                        ));
                    }
                    let weighted =
                        i128::from(units)
                            .checked_mul(i128::from(bps))
                            .ok_or_else(|| {
                                worker::Error::RustError("SALES_METRICS_WEIGHT_OVERFLOW".into())
                            })?;
                    let entry = money_groups
                        .entry((
                            row.stage_id.clone(),
                            row.status.clone(),
                            currency.to_string(),
                        ))
                        .or_default();
                    entry.0 = entry.0.checked_add(i128::from(units)).ok_or_else(|| {
                        worker::Error::RustError("SALES_METRICS_MONEY_OVERFLOW".into())
                    })?;
                    entry.1 = entry.1.checked_add(weighted).ok_or_else(|| {
                        worker::Error::RustError("SALES_METRICS_WEIGHT_OVERFLOW".into())
                    })?;
                    entry.2 = entry.2.checked_add(1).ok_or_else(|| {
                        worker::Error::RustError("SALES_METRICS_COUNT_OVERFLOW".into())
                    })?;
                }
                _ => {
                    return Err(worker::Error::RustError(
                        "SALES_METRICS_MONEY_INVALID".into(),
                    ));
                }
            }
            // The grouping key is the persisted status; no implicit stage->Won
            // conversion and no cross-currency totals.
            let _ = status;
        }
        let last = rows
            .last()
            .ok_or_else(|| worker::Error::RustError("SALES_METRICS_CURSOR_MISSING".into()))?
            .id
            .clone();
        if last <= after {
            return Err(worker::Error::RustError(
                "SALES_METRICS_CURSOR_INVALID".into(),
            ));
        }
        after = last;
        if rows.len() < METRICS_BATCH {
            break;
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
}
pub(super) async fn list_stage_page(
    db: &D1Database,
    tenant: &str,
    query: StageListQuery<'_>,
) -> Result<(Vec<Deal>, Option<String>, u64, Vec<StageCurrencyTotal>)> {
    let StageListQuery {
        pipeline_id,
        stage_id,
        after,
        limit,
        status,
        account,
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
    let count_sql =
        format!("SELECT COUNT(*) AS total FROM crm_sales_deal_records WHERE {condition}");
    let counted = bind(db, &count_sql, &arguments)?
        .first::<CountRow>(None)
        .await?
        .ok_or_else(|| worker::Error::RustError("SALES_BOARD_COUNT_MISSING".into()))?;
    let total = u64::try_from(counted.total)
        .map_err(|_| worker::Error::RustError("SALES_BOARD_COUNT_INVALID".into()))?;

    // One bounded D1 keyset batch at a time, with the SAME tenant, pipeline,
    // stage, status and account predicates as total_count. Pagination cursor
    // is intentionally excluded so every column page reports the same totals.
    // The existing stage index supplies the filter/id scan; no summary table
    // or independently mutated aggregate state is introduced.
    const SUMMARY_BATCH: usize = 256;
    let totals_sql = format!(
        "SELECT id, json_extract(state,'$.amount.currency') AS currency_code, \
         CAST(json_extract(state,'$.amount.minor_units') AS TEXT) AS minor_units \
         FROM crm_sales_deal_records WHERE {condition} \
         AND json_type(state,'$.amount')='object' AND id>? ORDER BY id LIMIT ?"
    );
    let mut summary_after = String::new();
    let mut summary = BTreeMap::<String, (i128, u64)>::new();
    loop {
        let mut summary_args = arguments.clone();
        summary_args.push(JsValue::from_str(&summary_after));
        summary_args.push(JsValue::from_f64(SUMMARY_BATCH as f64));
        let rows = bind(db, &totals_sql, &summary_args)?
            .all()
            .await?
            .results::<StageMoneyRow>()?;
        if rows.is_empty() {
            break;
        }
        for row in &rows {
            // Decode exact integer text, never SQLite SUM/REAL; reject invalid
            // stored Money rather than silently returning misleading totals.
            let units = row
                .minor_units
                .parse::<i128>()
                .map_err(|_| worker::Error::RustError("SALES_BOARD_AMOUNT_INVALID".into()))?;
            if units < 0 {
                return Err(worker::Error::RustError(
                    "SALES_BOARD_AMOUNT_INVALID".into(),
                ));
            }
            let currency = CurrencyCode::try_new(row.currency_code.clone())
                .map_err(|e| worker::Error::RustError(e.code.to_owned()))?;
            let total = summary.entry(currency.to_string()).or_insert((0, 0));
            total.0 = total
                .0
                .checked_add(units)
                .ok_or_else(|| worker::Error::RustError("SALES_BOARD_AMOUNT_OVERFLOW".into()))?;
            total.1 = total.1.checked_add(1).ok_or_else(|| {
                worker::Error::RustError("SALES_BOARD_AMOUNT_COUNT_OVERFLOW".into())
            })?;
        }
        let next_after = rows
            .last()
            .ok_or_else(|| worker::Error::RustError("SALES_BOARD_SUMMARY_CURSOR_MISSING".into()))?
            .id
            .clone();
        if next_after <= summary_after {
            return Err(worker::Error::RustError(
                "SALES_BOARD_SUMMARY_CURSOR_INVALID".into(),
            ));
        }
        summary_after = next_after;
        if rows.len() < SUMMARY_BATCH {
            break;
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
    condition.push_str(" AND id>?");
    arguments.push(JsValue::from_str(after));
    arguments.push(JsValue::from_f64((limit + 1) as f64));
    let list_sql = format!(
        "SELECT id,state FROM crm_sales_deal_records WHERE {condition} ORDER BY id LIMIT ?"
    );
    let mut rows = bind(db, &list_sql, &arguments)?
        .all()
        .await?
        .results::<PagedRow>()?;
    let more = rows.len() > limit;
    rows.truncate(limit);
    let next = if more {
        rows.last().map(|item| item.id.clone())
    } else {
        None
    };
    let deals = rows
        .iter()
        .map(|item| decode(&item.state))
        .collect::<Result<Vec<_>>>()?;
    Ok((deals, next, total, currency_totals))
}

pub(super) async fn list_page_typed(
    db: &D1Database,
    tenant: &str,
    after: &str,
    size: usize,
    pipeline: Option<&str>,
    status: Option<DealStatus>,
    account: Option<&str>,
) -> Result<(Vec<Deal>, Option<String>)> {
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
    sql.push_str(" ORDER BY id LIMIT ?");
    args.push(JsValue::from_f64((size + 1) as f64));
    let mut rows = bind(db, &sql, &args)?.all().await?.results::<PagedRow>()?;
    let more = rows.len() > size;
    rows.truncate(size);
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
