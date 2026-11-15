//! Native original crm-consents authority, NOT a self-attested consent API.
//! Operator suppression and verified email consent share this original D1
//! owner. Human email proof is signed by Access; enrolled dev service-admin
//! test actions carry separate provenance and never impersonate a human.
use super::{access, bind, contact_points, now_nanos};
use crm_consents::{
    CommunicationAuthorizationReason, CommunicationChannel, ConsentAuthorization,
    ConsentAuthorizationId, ConsentAuthorizationStatus, ConsentEffect, ContactPointReference,
    CreateConsentAuthorization, EvaluateCommunicationAuthorization, EvidenceReference,
    JurisdictionCode, LegalBasisCode, PartyReference, PurposeCode, SourceCode,
    WithdrawConsentAuthorization, decode_consent_authorization_state,
    encode_consent_authorization_state, evaluate_communication_authorization,
};
use crm_contact_points::{ContactPointKind, ContactPointStatus};
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
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct GrantInput {
    pub(super) id: String,
    pub(super) party_id: String,
    pub(super) contact_point_id: String,
    pub(super) purpose: String,
    pub(super) jurisdiction: String,
}
#[derive(Deserialize)]
struct ExistingConsent {
    state: String,
    operator_identity: String,
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
fn contact_proof(contact: &crm_contact_points::ContactPoint) -> Option<String> {
    let verification = contact.verification().evidence()?;
    Some(format!(
        "verified-contact:{:x}",
        Sha256::digest(verification.evidence_ref().as_bytes())
    ))
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
// Human consent requires a matching verified Access email. Dev service-admin
// may grant on a contact it verified as TEST data; audit records the real
// service identity and the source cannot be mistaken for human consent.
pub(super) async fn grant_verified_email_typed(
    input: GrantInput,
    key: &str,
    db: &D1Database,
    actor: &access::Actor,
    trace: &str,
) -> Result<std::result::Result<SuppressionOutcome, OperationFailure>> {
    let human_email = contact_points::signed_human_email(&actor.identity);
    let test_agent = actor.is_dev_test_agent();
    if human_email.is_none() && !test_agent {
        return Ok(Err(fail("CONSENTS_SIGNED_HUMAN_EMAIL_REQUIRED", 403)));
    }
    if !(8..=120).contains(&key.len()) {
        return Ok(Err(fail("IDEMPOTENCY_KEY_REQUIRED", 422)));
    }
    let digest = hash("email-grant", &input.id, &json!([&input, &actor.identity]))?;
    if let Some(prior) = receipt(db, &actor.tenant, key).await? {
        if prior.digest != digest {
            return Ok(Err(fail("IDEMPOTENCY_CONFLICT", 409)));
        }
        let item = decode(&prior.state)?;
        if item.authorization_id().as_str() != input.id || item.effect() != ConsentEffect::Grant {
            return Ok(Err(fail("IDEMPOTENCY_CONFLICT", 409)));
        }
        return Ok(Ok(SuppressionOutcome {
            item,
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
    let contact = match ContactPointReference::try_new(&input.contact_point_id) {
        Ok(x) => x,
        Err(e) => return Ok(Err(fail(e.code, 422))),
    };
    let purpose = match PurposeCode::try_new(input.purpose) {
        Ok(x) => x,
        Err(e) => return Ok(Err(fail(e.code, 422))),
    };
    let jurisdiction = match JurisdictionCode::try_new(input.jurisdiction) {
        Ok(x) => x,
        Err(e) => return Ok(Err(fail(e.code, 422))),
    };
    if !party_exists(db, &actor.tenant, party.as_str()).await? {
        return Ok(Err(fail("CONSENTS_PARTY_NOT_FOUND", 422)));
    }
    let Some(point) = contact_points::load(db, &actor.tenant, contact.as_str()).await? else {
        return Ok(Err(fail("CONSENTS_CONTACT_NOT_FOUND", 422)));
    };
    let now = now_nanos();
    let proof_matches_actor = point.verification().evidence().is_some_and(|e| {
        e.evidence_ref().starts_with(if test_agent {
            "crm-dev-agent-email:"
        } else {
            "access-email:"
        })
    });
    if point.party_ref().as_str() != party.as_str()
        || point.kind() != ContactPointKind::Email
        || human_email.is_some_and(|email| point.normalized_value() != email)
        || point.status() != ContactPointStatus::Active
        || !point.is_valid_at(now)
        || !point.verification().is_verified()
        || !proof_matches_actor
    {
        return Ok(Err(fail("CONSENTS_VERIFIED_EMAIL_OWNERSHIP_REQUIRED", 403)));
    }
    let evidence = contact_proof(&point)
        .ok_or_else(|| worker::Error::RustError("CONTACT_EVIDENCE_MISSING".into()))?;
    let item = match ConsentAuthorization::create(CreateConsentAuthorization {
        authorization_id: id,
        party_ref: party.clone(),
        contact_point_ref: Some(contact),
        purpose,
        channel: CommunicationChannel::Email,
        effect: ConsentEffect::Grant,
        legal_basis: LegalBasisCode::try_new(if test_agent {
            "consent.test.agent"
        } else {
            "consent.affirmative"
        })
        .map_err(|e| worker::Error::RustError(e.code))?,
        jurisdiction,
        source: SourceCode::try_new(if test_agent {
            "crm.dev.agent"
        } else {
            "cloudflare.access.email"
        })
        .map_err(|e| worker::Error::RustError(e.code))?,
        evidence_ref: EvidenceReference::try_new(evidence)
            .map_err(|e| worker::Error::RustError(e.code))?,
        effective_from_unix_nanos: now,
        expires_at_unix_nanos: None,
        occurred_at_unix_nanos: now,
    }) {
        Ok(x) => x,
        Err(e) => return Ok(Err(fail(e.code, 422))),
    };
    let saved = state(&item)?;
    // Atomic ContactPoint-version guard closes the check->insert race.
    // D1 trigger commits grant receipt/audit in this SAME SQL statement.
    let result = bind(db,
        "INSERT INTO crm_consent_records(tenant,id,party_id,state,version,idem_key,idem_digest,request_id,operator_identity,last_action) SELECT ?,?,?,?,?,?,?,?,?,'consents.grant' FROM crm_contact_point_records WHERE tenant=? AND id=? AND version=?",
        &[JsValue::from_str(&actor.tenant),
        JsValue::from_str(item.authorization_id().as_str()),
        JsValue::from_str(party.as_str()), JsValue::from_str(&saved),
        JsValue::from_f64(item.version() as f64), JsValue::from_str(key),
        JsValue::from_str(&digest), JsValue::from_str(trace),
        JsValue::from_str(&actor.identity), JsValue::from_str(&actor.tenant),
        JsValue::from_str(point.contact_point_id().as_str()),
        JsValue::from_f64(point.version() as f64)])?.run().await;
    let Ok(changed) = result else {
        return Ok(Err(fail("CONSENTS_GRANT_CREATE_CONFLICT", 409)));
    };
    if changed.meta()?.and_then(|m| m.changes).unwrap_or(0) == 0 {
        return Ok(Err(fail("CONSENTS_CONTACT_CHANGED", 409)));
    }
    Ok(Ok(SuppressionOutcome {
        item,
        replayed: false,
    }))
}

// Active GRANT revocation can be initiated by its original signed human or
// a tenant admin. DENY assertions cannot use this transition.
pub(super) async fn withdraw_typed(
    id: &str,
    expected_version: i64,
    key: &str,
    db: &D1Database,
    actor: &access::Actor,
    trace: &str,
) -> Result<std::result::Result<SuppressionOutcome, OperationFailure>> {
    if !(8..=120).contains(&key.len()) {
        return Ok(Err(fail("IDEMPOTENCY_KEY_REQUIRED", 422)));
    }
    let id = match ConsentAuthorizationId::try_new(id) {
        Ok(x) => x,
        Err(e) => return Ok(Err(fail(e.code, 422))),
    };
    let Some(existing) = bind(
        db,
        "SELECT state,operator_identity FROM crm_consent_records WHERE tenant=? AND id=?",
        &[
            JsValue::from_str(&actor.tenant),
            JsValue::from_str(id.as_str()),
        ],
    )?
    .first::<ExistingConsent>(None)
    .await?
    else {
        return Ok(Err(fail("NOT_FOUND", 404)));
    };
    if actor.role != "admin"
        && (contact_points::signed_human_email(&actor.identity).is_none()
            || existing.operator_identity != actor.identity)
    {
        return Ok(Err(fail("CONSENTS_WITHDRAW_NOT_OWNER", 403)));
    }
    let digest = hash("withdraw", id.as_str(), &expected_version)?;
    if let Some(prior) = receipt(db, &actor.tenant, key).await? {
        if prior.digest != digest {
            return Ok(Err(fail("IDEMPOTENCY_CONFLICT", 409)));
        }
        let item = decode(&prior.state)?;
        if item.authorization_id() != &id {
            return Ok(Err(fail("IDEMPOTENCY_CONFLICT", 409)));
        }
        return Ok(Ok(SuppressionOutcome {
            item,
            replayed: true,
        }));
    }
    let mut item = decode(&existing.state)?;
    if let Err(e) = item.withdraw(WithdrawConsentAuthorization {
        expected_version,
        occurred_at_unix_nanos: now_nanos(),
    }) {
        return Ok(Err(fail(
            e.code.clone(),
            if e.code == "CONSENTS_VERSION_CONFLICT" || e.code == "CONSENTS_ALREADY_WITHDRAWN" {
                409
            } else {
                422
            },
        )));
    }
    let saved = state(&item)?;
    let r = bind(db,
        "UPDATE crm_consent_records SET state=?,version=?,idem_key=?,idem_digest=?,request_id=?,operator_identity=?,last_action='consents.withdraw' WHERE tenant=? AND id=? AND version=?",
        &[JsValue::from_str(&saved),JsValue::from_f64(item.version() as f64),
        JsValue::from_str(key),JsValue::from_str(&digest),JsValue::from_str(trace),
        JsValue::from_str(&actor.identity),JsValue::from_str(&actor.tenant),
        JsValue::from_str(id.as_str()),JsValue::from_f64(expected_version as f64)])?.run().await;
    let Ok(changed) = r else {
        return Ok(Err(fail("CONSENTS_WITHDRAW_CONFLICT", 409)));
    };
    if changed.meta()?.and_then(|m| m.changes).unwrap_or(0) == 0 {
        return Ok(Err(fail("CONSENTS_VERSION_CONFLICT", 409)));
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
    // A past grant is ineffective if the endpoint was changed, deactivated
    // or verified again. The original evidence is bound to its verification
    // epoch, not only to a reusable contact ID.
    let current_proof = if let Some(contact_id) = scope.contact_point_ref.as_ref() {
        let point = contact_points::load(db, tenant, contact_id.as_str()).await?;
        point
            .filter(|p| {
                p.kind() == ContactPointKind::Email
                    && p.status() == ContactPointStatus::Active
                    && p.is_valid_at(now_nanos())
            })
            .and_then(|p| contact_proof(&p))
    } else {
        None
    };
    let rows = all(db, tenant).await?;
    let applicable: Vec<&ConsentAuthorization> = rows
        .iter()
        .filter(|item| {
            if item.effect() != ConsentEffect::Grant {
                return true;
            }
            item.source().as_str() == "cloudflare.access.email"
                && item
                    .contact_point_ref()
                    .is_some_and(|c| scope.contact_point_ref.as_ref().is_some_and(|r| r == c))
                && current_proof.as_deref() == Some(item.evidence_ref().as_str())
        })
        .collect();
    let decision = evaluate_communication_authorization(&scope, applicable.iter().copied())
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
