//! Read-only original Identity Resolution boundary: Party comparison and
//! observable same-email candidate suggestions. No identity verdict, score,
//! persisted duplicate case, self-attested verification or merge.
use crm_identity_resolution::{
    CanonicalPartyPair, PartyReference, RecordedEmailContact,
    discover_shared_recorded_emails,
};
use crm_contact_points::{ContactPointKind, ContactPointStatus};
use serde::Deserialize;
use serde_json::json;
use wasm_bindgen::JsValue;
use worker::{D1Database, Method, Request, Response, Result};

use super::{bind, contact_points, error, now_nanos, parties, response};

#[derive(Deserialize)]
struct ContactRow { state: String }

async fn candidates(db: &D1Database, tenant: &str) -> Result<Response> {
    // Fail closed when the bounded read cannot represent the full tenant set.
    // Never claim that a silently truncated scan found all possible matches.
    let rows = bind(db,
        "SELECT state FROM crm_contact_point_records WHERE tenant = ? ORDER BY id LIMIT 301",
        &[JsValue::from_str(tenant)])?
        .all().await?.results::<ContactRow>()?;
    if rows.len() > 300 {
        return error("IDENTITY_RESOLUTION_SCAN_LIMIT_REACHED", 503);
    }
    let now = now_nanos();
    let mut contacts = Vec::new();
    for row in rows {
        let point = contact_points::decode(&row.state)?;
        if point.kind() != ContactPointKind::Email
            || point.status() != ContactPointStatus::Active
            || !point.is_valid_at(now) { continue; }
        let party = PartyReference::try_new(point.party_ref().as_str())
            .map_err(|e| worker::Error::RustError(e.code))?;
        contacts.push(RecordedEmailContact {
            party,
            contact_id: point.contact_point_id().as_str().to_owned(),
            normalized_email: point.normalized_value().to_owned(),
            verified: point.verification().is_verified(),
        });
    }
    let observations = discover_shared_recorded_emails(&contacts)
        .map_err(|e| worker::Error::RustError(e.code))?;
    let mut items = Vec::new();
    for item in observations {
        // Party remains the authoritative owner. Both records must still be
        // present in the same verified tenant; never return a partial pair.
        let Some(left) = parties::stored(db, tenant, item.pair.left().as_str()).await? else {
            return error("IDENTITY_RESOLUTION_SOURCE_CHANGED", 409);
        };
        let Some(right) = parties::stored(db, tenant, item.pair.right().as_str()).await? else {
            return error("IDENTITY_RESOLUTION_SOURCE_CHANGED", 409);
        };
        items.push(json!({
            "first": parties::view(&left),
            "second": parties::view(&right),
            "contact_point_ids": [item.left_contact_id, item.right_contact_id],
            "source_kind": "recorded_exact_normalized_email",
            "both_contacts_verified": item.both_verified,
            "identity_confirmed": false
        }));
    }
    response(json!({
        "items": items,
        "source": "original_crm_contact_points_and_identity_resolution",
        "coverage": "all_current_active_email_contacts_up_to_300",
        "match_score": "NOT_COMPUTED",
        "candidate_cases": "NOT_CONNECTED",
        "read_only": true
    }), 200)
}

pub(super) async fn route(req: Request, db: D1Database, tenant: &str) -> Result<Response> {
    if req.method() != Method::Get {
        return error("NOT_FOUND", 404);
    }
    let path = req.path();
    if path == "/api/identity-resolution/candidates" {
        return candidates(&db, tenant).await;
    }
    let Some(tail) = path.strip_prefix("/api/identity-resolution/compare/") else {
        return error("NOT_FOUND", 404);
    };
    let mut parts = tail.split('/');
    let (Some(left_id), Some(right_id), None) = (parts.next(), parts.next(), parts.next()) else {
        return error("NOT_FOUND", 404);
    };
    let left = match PartyReference::try_new(left_id) {
        Ok(party) => party,
        Err(e) => return error(&e.code, 422),
    };
    let right = match PartyReference::try_new(right_id) {
        Ok(party) => party,
        Err(e) => return error(&e.code, 422),
    };
    // The authoritative owner rejects a self-pair and sorts the pair
    // deterministically. No comparison logic or score is reimplemented.
    let pair = match CanonicalPartyPair::try_new(left, right) {
        Ok(pair) => pair,
        Err(e) => return error(&e.code, 422),
    };
    // Each read is scoped by the VERIFIED Access tenant; a Party belonging
    // to a different tenant must be indistinguishable from a missing record.
    let Some(left) = parties::stored(&db, tenant, pair.left().as_str()).await? else {
        return error("NOT_FOUND", 404);
    };
    let Some(right) = parties::stored(&db, tenant, pair.right().as_str()).await? else {
        return error("NOT_FOUND", 404);
    };
    // Canonical storage/validation and caller selection order are distinct:
    // do not display the lower Party ID as "first" when the user selected it second.
    let (first, second) = if left_id == pair.left().as_str() {
        (parties::view(&left), parties::view(&right))
    } else {
        (parties::view(&right), parties::view(&left))
    };
    response(
        json!({
            "first": first,
            "second": second,
            "canonical_pair": {
                "left_id": pair.left().as_str(),
                "right_id": pair.right().as_str()
            },
            "source": "original_crm_parties_d1",
            "candidate_cases": "NOT_CONNECTED",
            "match_evidence": "NOT_ASSESSED",
            "merge": "NOT_PERFORMED",
            "read_only": true
        }),
        200,
    )
}
