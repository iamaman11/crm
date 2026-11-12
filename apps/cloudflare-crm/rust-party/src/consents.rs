//! Native original crm-consents authority, NOT a self-attested consent API.
//! Only authenticated *operator suppression* (Deny) is currently accepted.
//! No user-controlled evidence_ref, grant or source is ever trusted. Operator
//! action evidence proves only who recorded the block, NOT subject consent.
//! Future Grant ingress requires a separately verified authoritative source.
use super::{bind, now_nanos};
use crm_consents::{
    CommunicationAuthorizationReason, CommunicationChannel, ConsentAuthorization,
    ConsentAuthorizationId, ConsentAuthorizationStatus, ConsentEffect, ContactPointReference,
    CreateConsentAuthorization, EvaluateCommunicationAuthorization, EvidenceReference,
    JurisdictionCode, LegalBasisCode, PartyReference, PurposeCode, SourceCode,
    decode_consent_authorization_state, encode_consent_authorization_state,
    evaluate_communication_authorization,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use wasm_bindgen::JsValue;
use worker::{D1Database, Result};

#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum ChannelInput {
    Email,
    Phone,
    Sms,
    Postal,
    Messaging,
    Push,
}
impl From<ChannelInput> for CommunicationChannel {
    fn from(x: ChannelInput) -> Self {
        match x {
            ChannelInput::Email => Self::Email,
            ChannelInput::Phone => Self::Phone,
            ChannelInput::Sms => Self::Sms,
            ChannelInput::Postal => Self::Postal,
            ChannelInput::Messaging => Self::Messaging,
            ChannelInput::Push => Self::Push,
        }
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ScopeInput {
    pub(super) party_id: String,
    pub(super) contact_point_id: Option<String>,
    pub(super) purpose: String,
    pub(super) channel: ChannelInput,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SuppressionInput {
    pub(super) id: String,
    pub(super) party_id: String,
    pub(super) contact_point_id: Option<String>,
    pub(super) purpose: String,
    pub(super) channel: ChannelInput,
    pub(super) jurisdiction: String,
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
pub(super) struct SuppressionOutcome {
    pub(super) item: ConsentAuthorization,
    pub(super) replayed: bool,
}
pub(super) struct ConsentDecision {
    pub(super) allowed: bool,
    pub(super) reason: CommunicationAuthorizationReason,
    pub(super) determining_ids: Vec<String>,
}
pub(super) struct OperationFailure {
    pub(super) code: String,
    pub(super) status: u16,
}
fn fail(code: impl Into<String>, status: u16) -> OperationFailure {
    OperationFailure {
        code: code.into(),
        status,
    }
}
#[derive(Deserialize)]
struct Receipt {
    digest: String,
    state: String,
}
fn hash<T: Serialize>(op: &str, id: &str, payload: &T) -> Result<String> {
    let bytes = serde_json::to_vec(&json!([op, id, payload]))
        .map_err(|e| worker::Error::RustError(e.to_string()))?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}
fn state(item: &ConsentAuthorization) -> Result<String> {
    String::from_utf8(
        encode_consent_authorization_state(item).map_err(|e| worker::Error::RustError(e.code))?,
    )
    .map_err(|e| worker::Error::RustError(e.to_string()))
}
fn decode(value: &str) -> Result<ConsentAuthorization> {
    decode_consent_authorization_state(value.as_bytes())
        .map_err(|e| worker::Error::RustError(e.code))
}
async fn receipt(db: &D1Database, tenant: &str, key: &str) -> Result<Option<Receipt>> {
    bind(
        db,
        "SELECT digest,state FROM crm_consent_receipts WHERE tenant=? AND key=?",
        &[JsValue::from_str(tenant), JsValue::from_str(key)],
    )?
    .first::<Receipt>(None)
    .await
}
async fn party_exists(db: &D1Database, tenant: &str, id: &str) -> Result<bool> {
    Ok(bind(
        db,
        "SELECT id FROM crm_party_records WHERE tenant=? AND id=?",
        &[JsValue::from_str(tenant), JsValue::from_str(id)],
    )?
    .first::<Value>(None)
    .await?
    .is_some())
}
async fn contact_belongs(
    db: &D1Database,
    tenant: &str,
    party: &str,
    contact: &str,
) -> Result<bool> {
    Ok(bind(
        db,
        "SELECT id FROM crm_contact_point_records WHERE tenant=? AND party_id=? AND id=?",
        &[
            JsValue::from_str(tenant),
            JsValue::from_str(party),
            JsValue::from_str(contact),
        ],
    )?
    .first::<Value>(None)
    .await?
    .is_some())
}
async fn all(db: &D1Database, tenant: &str) -> Result<Vec<ConsentAuthorization>> {
    // Evaluator never silently drops records: an incomplete view must not grant.
    let rows = bind(
        db,
        "SELECT state FROM crm_consent_records WHERE tenant=? ORDER BY id LIMIT 501",
        &[JsValue::from_str(tenant)],
    )?
    .all()
    .await?
    .results::<Row>()?;
    if rows.len() > 500 {
        return Err(worker::Error::RustError(
            "CONSENTS_EVALUATION_CAP_REACHED".into(),
        ));
    }
    rows.iter().map(|row| decode(&row.state)).collect()
}
/// Single consent mutation owner shared by legacy isolated native tests and
/// generated Connect. Operator may record only DENY, never unverified GRANT.
pub(super) async fn suppress_typed(
    input: SuppressionInput,
    k: &str,
    db: &D1Database,
    tenant: &str,
    actor: &str,
    trace: &str,
) -> Result<std::result::Result<SuppressionOutcome, OperationFailure>> {
    if !(8..=120).contains(&k.len()) {
        return Ok(Err(fail("IDEMPOTENCY_KEY_REQUIRED", 422)));
    }
    let digest = hash("operator-suppress", &input.id, &input)?;
    if let Some(p) = receipt(db, tenant, k).await? {
        if p.digest != digest {
            return Ok(Err(fail("IDEMPOTENCY_CONFLICT", 409)));
        }
        return Ok(Ok(SuppressionOutcome {
            item: decode(&p.state)?,
            replayed: true,
        }));
    }
    let id = match ConsentAuthorizationId::try_new(&input.id) {
        Ok(x) => x,
        Err(e) => return Ok(Err(fail(e.code, 422))),
    };
    let party = match PartyReference::try_new(&input.party_id) {
        Ok(x) => x,
        Err(e) => return Ok(Err(fail(e.code, 422))),
    };
    if !party_exists(db, tenant, party.as_str()).await? {
        return Ok(Err(fail("CONSENTS_PARTY_NOT_FOUND", 422)));
    }
    let contact = match input.contact_point_id {
        Some(id) => {
            if !contact_belongs(db, tenant, party.as_str(), &id).await? {
                return Ok(Err(fail("CONSENTS_CONTACT_POINT_PARTY_MISMATCH", 422)));
            }
            match ContactPointReference::try_new(id) {
                Ok(x) => Some(x),
                Err(e) => return Ok(Err(fail(e.code, 422))),
            }
        }
        None => None,
    };
    let purpose = match PurposeCode::try_new(input.purpose) {
        Ok(x) => x,
        Err(e) => return Ok(Err(fail(e.code, 422))),
    };
    let jurisdiction = match JurisdictionCode::try_new(input.jurisdiction) {
        Ok(x) => x,
        Err(e) => return Ok(Err(fail(e.code, 422))),
    };
    let legal_basis = LegalBasisCode::try_new("operational.suppression")
        .map_err(|e| worker::Error::RustError(e.code))?;
    let source =
        SourceCode::try_new("crm.access.operator").map_err(|e| worker::Error::RustError(e.code))?;
    let evidence_ref = EvidenceReference::try_new(format!("operator-action:{trace}"))
        .map_err(|e| worker::Error::RustError(e.code))?;
    let now = now_nanos();
    let item = match ConsentAuthorization::create(CreateConsentAuthorization {
        authorization_id: id,
        party_ref: party.clone(),
        contact_point_ref: contact,
        purpose,
        channel: input.channel.into(),
        effect: ConsentEffect::Deny,
        legal_basis,
        jurisdiction,
        source,
        evidence_ref,
        effective_from_unix_nanos: now,
        expires_at_unix_nanos: None,
        occurred_at_unix_nanos: now,
    }) {
        Ok(x) => x,
        Err(e) => return Ok(Err(fail(e.code, 422))),
    };
    let saved = state(&item)?;
    let r=bind(db,"INSERT INTO crm_consent_records(tenant,id,party_id,state,version,idem_key,idem_digest,request_id,operator_identity) VALUES(?,?,?,?,?,?,?,?,?)",
        &[JsValue::from_str(tenant),JsValue::from_str(item.authorization_id().as_str()),
          JsValue::from_str(party.as_str()),JsValue::from_str(&saved),
          JsValue::from_f64(item.version() as f64),JsValue::from_str(k),
          JsValue::from_str(&digest),JsValue::from_str(trace),
          JsValue::from_str(actor)])?.run().await;
    if r.is_err() {
        return Ok(Err(fail("CONSENTS_SUPPRESSION_CREATE_CONFLICT", 409)));
    }
    Ok(Ok(SuppressionOutcome {
        item,
        replayed: false,
    }))
}
pub(super) async fn evaluate_typed(
    input: ScopeInput,
    db: &D1Database,
    tenant: &str,
) -> Result<std::result::Result<ConsentDecision, OperationFailure>> {
    let party = match PartyReference::try_new(&input.party_id) {
        Ok(x) => x,
        Err(e) => return Ok(Err(fail(e.code, 422))),
    };
    if !party_exists(db, tenant, party.as_str()).await? {
        return Ok(Err(fail("CONSENTS_PARTY_NOT_FOUND", 422)));
    }
    let contact = match input.contact_point_id {
        Some(id) => {
            if !contact_belongs(db, tenant, party.as_str(), &id).await? {
                return Ok(Err(fail("CONSENTS_CONTACT_POINT_PARTY_MISMATCH", 422)));
            }
            match ContactPointReference::try_new(id) {
                Ok(x) => Some(x),
                Err(e) => return Ok(Err(fail(e.code, 422))),
            }
        }
        None => None,
    };
    let purpose = match PurposeCode::try_new(input.purpose) {
        Ok(x) => x,
        Err(e) => return Ok(Err(fail(e.code, 422))),
    };
    let scope = EvaluateCommunicationAuthorization {
        party_ref: party,
        contact_point_ref: contact,
        purpose,
        channel: input.channel.into(),
        evaluation_time_unix_nanos: now_nanos(),
    };
    let rows = all(db, tenant).await?;
    let decision = evaluate_communication_authorization(&scope, rows.iter())
        .map_err(|e| worker::Error::RustError(e.code))?;
    Ok(Ok(ConsentDecision {
        allowed: decision.allowed,
        reason: decision.reason,
        determining_ids: decision
            .determining_authorization_ids
            .iter()
            .map(|id| id.as_str().to_owned())
            .collect(),
    }))
}
pub(super) async fn list_page_typed(
    db: &D1Database,
    tenant: &str,
    after: &str,
    size: usize,
    party_filter: Option<&str>,
) -> Result<(Vec<ConsentAuthorization>, Option<String>)> {
    let limit = JsValue::from_f64((size + 1) as f64);
    let mut bindings = vec![JsValue::from_str(tenant), JsValue::from_str(after)];
    let sql = if let Some(party) = party_filter {
        bindings.push(JsValue::from_str(party));
        "SELECT id,state FROM crm_consent_records WHERE tenant=? AND id>? AND party_id=? ORDER BY id LIMIT ?"
    } else {
        "SELECT id,state FROM crm_consent_records WHERE tenant=? AND id>? ORDER BY id LIMIT ?"
    };
    bindings.push(limit);
    let mut rows = bind(db, sql, &bindings)?
        .all()
        .await?
        .results::<PagedRow>()?;
    let more = rows.len() > size;
    rows.truncate(size);
    let next = if more {
        rows.last().map(|x| x.id.clone())
    } else {
        None
    };
    let decoded = rows
        .iter()
        .map(|r| decode(&r.state))
        .collect::<Result<Vec<_>>>()?;
    Ok((decoded, next))
}
