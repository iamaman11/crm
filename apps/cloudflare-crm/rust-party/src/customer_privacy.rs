//! D1 operations for the original PrivacyCase domain. No HTTP/JSON business API.
//! Verification and privacy execution remain outside this intake capability.
use super::{access, bind, contact_points, now_nanos, parties};
use crm_customer_privacy::{
    PrivacyCase, PrivacyCaseKind, PrivacyDomainError, decode_privacy_case_state,
    encode_privacy_case_state,
};
use crm_contact_points::{ContactPointKind, ContactPointStatus, VerificationState};
use crm_module_sdk::{ActorId, RecordId, SchemaVersion, TenantId};
use crm_parties::PartyKind;
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use wasm_bindgen::JsValue;
use worker::{D1Database, Result};
#[derive(Deserialize)]
struct CreatorRow {
    created_by: String,
}
#[derive(Deserialize)]
struct StateRow {
    state: String,
}
#[derive(Deserialize)]
struct Receipt {
    digest: String,
    state: String,
}
#[derive(Deserialize)]
struct PageRow {
    id: String,
    state: String,
}
pub(super) struct Saved {
    pub(super) case: PrivacyCase,
    pub(super) replayed: bool,
}
pub(super) struct Failure {
    pub(super) code: String,
    pub(super) status: u16,
}
pub(super) enum Transition {
    Submit,
    Cancel,
    VerifyOwnSubject {
        submitted_party_id: RecordId,
        contact_point_id: RecordId,
    },
}
fn fail(code: impl Into<String>, status: u16) -> Failure {
    Failure {
        code: code.into(),
        status,
    }
}
fn encode(c: &PrivacyCase) -> Result<String> {
    String::from_utf8(encode_privacy_case_state(c).map_err(|e| worker::Error::RustError(e.code))?)
        .map_err(|e| worker::Error::RustError(e.to_string()))
}
fn decode(s: &str) -> Result<PrivacyCase> {
    decode_privacy_case_state(s.as_bytes()).map_err(|e| worker::Error::RustError(e.code))
}
fn digest(op: &str, id: &str, payload: &Value) -> String {
    format!(
        "{:x}",
        Sha256::digest(
            serde_json::to_vec(&json!([op, id, payload])).expect("static digest serialization")
        )
    )
}

async fn receipt(db: &D1Database, tenant: &str, k: &str) -> Result<Option<Receipt>> {
    bind(
        db,
        "SELECT digest,state FROM crm_privacy_case_receipts WHERE tenant=? AND key=?",
        &[JsValue::from_str(tenant), JsValue::from_str(k)],
    )?
    .first::<Receipt>(None)
    .await
}
pub(super) async fn stored(db: &D1Database, tenant: &str, id: &str) -> Result<Option<PrivacyCase>> {
    let row = bind(
        db,
        "SELECT state FROM crm_privacy_case_records WHERE tenant=? AND id=?",
        &[JsValue::from_str(tenant), JsValue::from_str(id)],
    )?
    .first::<StateRow>(None)
    .await?;
    row.map(|x| decode(&x.state)).transpose()
}

async fn replay(
    db: &D1Database,
    tenant: &str,
    key: &str,
    hash: &str,
) -> Result<Option<std::result::Result<Saved, Failure>>> {
    receipt(db, tenant, key)
        .await?
        .map(|r| -> Result<std::result::Result<Saved, Failure>> {
            if r.digest != hash {
                return Ok(Err(fail("IDEMPOTENCY_CONFLICT", 409)));
            }
            Ok(Ok(Saved {
                case: decode(&r.state)?,
                replayed: true,
            }))
        })
        .transpose()
}
pub(super) async fn create_typed(
    id: RecordId,
    kind: PrivacyCaseKind,
    key: &str,
    db: &D1Database,
    actor: &access::Actor,
    trace: &str,
) -> Result<std::result::Result<Saved, Failure>> {
    let tenant = actor.tenant.as_str();
    if !(8..=120).contains(&key.len()) {
        return Ok(Err(fail("IDEMPOTENCY_KEY_REQUIRED", 422)));
    }
    let hash = digest("create", id.as_str(), &json!([kind]));
    if let Some(saved) = replay(db, tenant, key, &hash).await? {
        return Ok(saved);
    }
    let tenant_id =
        TenantId::try_new(tenant).map_err(|e| worker::Error::RustError(e.to_string()))?;
    let policy =
        SchemaVersion::try_new("1.0.0").map_err(|e| worker::Error::RustError(e.to_string()))?;
    let case = match PrivacyCase::new(id, tenant_id, kind, policy, now_nanos(), None) {
        Ok(case) => case,
        Err(e) => return Ok(Err(fail(e.code(), 422))),
    };
    let state = encode(&case)?;
    let inserted=bind(db,
        "INSERT INTO crm_privacy_case_records(tenant,id,state,version,idem_key,idem_digest,request_id,created_by,actor) VALUES(?,?,?,?,?,?,?,?,?)",
        &[JsValue::from_str(tenant),JsValue::from_str(case.case_id().as_str()),
          JsValue::from_str(&state),JsValue::from_str(&case.version().to_string()),
          JsValue::from_str(key),JsValue::from_str(&hash),JsValue::from_str(trace),
          JsValue::from_str(&actor.identity),JsValue::from_str(&actor.identity)])?.run().await;
    if inserted.is_err() {
        if let Some(saved) = replay(db, tenant, key, &hash).await? {
            return Ok(saved);
        }
        return Ok(Err(fail("PRIVACY_CASE_CREATE_CONFLICT", 409)));
    }
    Ok(Ok(Saved {
        case,
        replayed: false,
    }))
}
pub(super) async fn transition_typed(
    id: &str,
    action: Transition,
    expected_version: u64,
    key: &str,
    db: &D1Database,
    actor: &access::Actor,
    trace: &str,
) -> Result<std::result::Result<Saved, Failure>> {
    let tenant = actor.tenant.as_str();
    if !(8..=120).contains(&key.len()) {
        return Ok(Err(fail("IDEMPOTENCY_KEY_REQUIRED", 422)));
    }
    let (operation, payload) = match &action {
        Transition::Submit => ("submit", json!([expected_version])),
        Transition::Cancel => ("cancel", json!([expected_version])),
        Transition::VerifyOwnSubject {
            submitted_party_id,
            contact_point_id,
        } => (
            "verify-own-subject",
            json!([expected_version, submitted_party_id.as_str(), contact_point_id.as_str()]),
        ),
    };
    let hash = digest(operation, id, &payload);
    // This is a human self-verification, not a staff claim: never accept a
    // signed service token, dev-agent evidence, or a merely listed email.
    // Revalidate even on receipt replay so withdrawn ownership cannot be used.
    let contact_guard = if let Transition::VerifyOwnSubject {
        submitted_party_id,
        contact_point_id,
    } = &action
    {
        let Some(email) = contact_points::signed_human_email(&actor.identity) else {
            return Ok(Err(fail("PRIVACY_SIGNED_HUMAN_EMAIL_REQUIRED", 403)));
        };
        let Some(point) = contact_points::load(db, tenant, contact_point_id.as_str()).await?
        else {
            return Ok(Err(fail("PRIVACY_SUBJECT_OWNERSHIP_REQUIRED", 403)));
        };
        let trusted = matches!(
            point.verification(),
            VerificationState::Verified(evidence)
                if evidence.evidence_ref().starts_with("access-email:")
        );
        if !trusted
            || point.kind() != ContactPointKind::Email
            || point.status() != ContactPointStatus::Active
            || point.normalized_value() != email
            || point.party_ref().as_str() != submitted_party_id.as_str()
            || !point.is_valid_at(now_nanos())
        {
            return Ok(Err(fail("PRIVACY_SUBJECT_OWNERSHIP_REQUIRED", 403)));
        }
        // A submitted case must belong to this exact signed human, not
        // merely be an unrelated admin-created case that happens to be visible.
        let creator = bind(
            db,
            "SELECT created_by FROM crm_privacy_case_records WHERE tenant=? AND id=?",
            &[JsValue::from_str(tenant), JsValue::from_str(id)],
        )?
        .first::<CreatorRow>(None)
        .await?;
        if creator.is_none_or(|row| row.created_by != actor.identity) {
            return Ok(Err(fail("PRIVACY_CASE_OWNERSHIP_REQUIRED", 403)));
        }
        let Some(party) = parties::stored(db, tenant, submitted_party_id.as_str()).await?
        else {
            return Ok(Err(fail("PRIVACY_SUBJECT_OWNERSHIP_REQUIRED", 403)));
        };
        if party.kind() != PartyKind::Person {
            return Ok(Err(fail("PRIVACY_SUBJECT_OWNERSHIP_REQUIRED", 403)));
        }
        Some((
            submitted_party_id.as_str().to_owned(),
            contact_point_id.as_str().to_owned(),
            point.version(),
        ))
    } else {
        None
    };
    if let Some(saved) = replay(db, tenant, key, &hash).await? {
        return Ok(saved);
    }
    let Some(mut case) = stored(db, tenant, id).await? else {
        return Ok(Err(fail("NOT_FOUND", 404)));
    };
    let now = now_nanos();
    let result = match action {
        Transition::Submit => case.submit(expected_version, now),
        Transition::Cancel => case.cancel(expected_version, now),
        Transition::VerifyOwnSubject {
            submitted_party_id,
            ..
        } => {
            let verified_by = ActorId::try_new(actor.identity.clone())
                .map_err(|_| worker::Error::RustError("PRIVACY_ACTOR_INVALID".into()))?;
            // The active CRM has no merge topology; generation 1 names the
            // current unmerged canonical Party. R5b must explicitly re-check
            // canonical identity generations before any export or erasure.
            case.verify_subject(
                expected_version,
                submitted_party_id.clone(),
                submitted_party_id,
                1,
                crm_customer_privacy::SubjectVerificationMethod::AuthenticatedPortal,
                verified_by,
                now,
            )
        }
    };
    if let Err(error) = result {
        return Ok(Err(fail(
            error.code(),
            match error {
                PrivacyDomainError::InvalidArgument { .. } => 422,
                PrivacyDomainError::InvalidTransition { .. } => 412,
                PrivacyDomainError::VersionConflict { .. } => 409,
            },
        )));
    }
    let state = encode(&case)?;
    let mut values = vec![
        JsValue::from_str(&state),
        JsValue::from_str(&case.version().to_string()),
        JsValue::from_str(key),
        JsValue::from_str(&hash),
        JsValue::from_str(trace),
        JsValue::from_str(&actor.identity),
        JsValue::from_str(tenant),
        JsValue::from_str(id),
        JsValue::from_str(&expected_version.to_string()),
    ];
    // Same D1 statement atomically checks that the already validated contact
    // was not changed between read and case commit (no second persistence path).
    let sql = if let Some((party_id, contact_id, point_version)) = contact_guard {
        values.extend([
            JsValue::from_str(&actor.identity),
            JsValue::from_str(tenant),
            JsValue::from_str(&contact_id),
            JsValue::from_str(&party_id),
            JsValue::from_f64(point_version as f64),
        ]);
        "UPDATE crm_privacy_case_records SET state=?,version=?,idem_key=?,idem_digest=?,request_id=?,actor=? WHERE tenant=? AND id=? AND version=? AND created_by=? AND EXISTS (SELECT 1 FROM crm_contact_point_records WHERE tenant=? AND id=? AND party_id=? AND version=?)"
    } else {
        "UPDATE crm_privacy_case_records SET state=?,version=?,idem_key=?,idem_digest=?,request_id=?,actor=? WHERE tenant=? AND id=? AND version=?"
    };
    let result = bind(db, sql, &values)?.run().await;
    let changed = match result {
        Ok(result) => result.meta()?.and_then(|m| m.changes).unwrap_or(0) > 0,
        Err(_) => false,
    };
    if !changed {
        if let Some(saved) = replay(db, tenant, key, &hash).await? {
            return Ok(saved);
        }
        return Ok(Err(fail("PRIVACY_CASE_VERSION_CONFLICT", 409)));
    }
    Ok(Ok(Saved {
        case,
        replayed: false,
    }))
}
pub(super) async fn list_page_typed(
    db: &D1Database,
    tenant: &str,
    after: &str,
    size: usize,
) -> Result<(Vec<PrivacyCase>, Option<String>)> {
    let mut rows = bind(
        db,
        "SELECT id,state FROM crm_privacy_case_records WHERE tenant=? AND id>? ORDER BY id LIMIT ?",
        &[
            JsValue::from_str(tenant),
            JsValue::from_str(after),
            JsValue::from_f64((size + 1) as f64),
        ],
    )?
    .all()
    .await?
    .results::<PageRow>()?;
    let next = if rows.len() > size {
        rows.truncate(size);
        rows.last().map(|r| r.id.clone())
    } else {
        None
    };
    let cases = rows
        .into_iter()
        .map(|row| decode(&row.state))
        .collect::<Result<Vec<_>>>()?;
    Ok((cases, next))
}
