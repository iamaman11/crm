//! Read-only original crm-identity-resolution boundary. The domain crate
//! exclusively decides observed same-email pairs; there is NO merge or identity
//! confirmation. Connect uses these typed D1 reads.
use crm_contact_points::{ContactPointKind, ContactPointStatus};
use crm_identity_resolution::{
    CanonicalPartyPair, PartyReference, RecordedEmailContact, discover_shared_recorded_emails,
};
use crm_parties::Party;
use serde::Deserialize;
use wasm_bindgen::JsValue;
use worker::{D1Database, Result};

use super::{bind, contact_points, now_nanos, parties};

#[derive(Deserialize)]
struct ContactRow {
    state: String,
}

pub(super) enum CandidateError {
    ScanLimit,
    SourceChanged,
}
pub(super) struct Candidate {
    pub(super) first: Party,
    pub(super) second: Party,
    pub(super) first_contact_point_id: String,
    pub(super) second_contact_point_id: String,
    pub(super) both_verified: bool,
}

/// The canonical candidate computation is performed ONCE by the original
/// crm-identity-resolution domain. No ContactPoint claim becomes proof.
pub(super) async fn candidates_typed(
    db: &D1Database,
    tenant: &str,
) -> Result<std::result::Result<Vec<Candidate>, CandidateError>> {
    // Refuse a silently truncated scan instead of inventing complete coverage.
    let rows = bind(
        db,
        "SELECT state FROM crm_contact_point_records WHERE tenant = ? ORDER BY id LIMIT 301",
        &[JsValue::from_str(tenant)],
    )?
    .all()
    .await?
    .results::<ContactRow>()?;
    if rows.len() > 300 {
        return Ok(Err(CandidateError::ScanLimit));
    }
    let now = now_nanos();
    let mut contacts = Vec::new();
    for row in rows {
        let point = contact_points::decode(&row.state)?;
        if point.kind() != ContactPointKind::Email
            || point.status() != ContactPointStatus::Active
            || !point.is_valid_at(now)
        {
            continue;
        }
        let party = PartyReference::try_new(point.party_ref().as_str())
            .map_err(|e| worker::Error::RustError(e.code))?;
        contacts.push(RecordedEmailContact {
            party,
            contact_id: point.contact_point_id().as_str().to_owned(),
            normalized_email: point.normalized_value().to_owned(),
            verified: point.verification().is_verified(),
        });
    }
    let observations =
        discover_shared_recorded_emails(&contacts).map_err(|e| worker::Error::RustError(e.code))?;
    let mut found = Vec::with_capacity(observations.len());
    for item in observations {
        let Some(first) = parties::stored(db, tenant, item.pair.left().as_str()).await? else {
            return Ok(Err(CandidateError::SourceChanged));
        };
        let Some(second) = parties::stored(db, tenant, item.pair.right().as_str()).await? else {
            return Ok(Err(CandidateError::SourceChanged));
        };
        found.push(Candidate {
            first,
            second,
            first_contact_point_id: item.left_contact_id,
            second_contact_point_id: item.right_contact_id,
            both_verified: item.both_verified,
        });
    }
    Ok(Ok(found))
}

pub(super) struct PartyComparison {
    pub(super) first: Party,
    pub(super) second: Party,
}

/// Caller must first build canonical pair using the original domain; this
/// reader always returns selected-first order, never claims identity.
pub(super) async fn compare_typed(
    db: &D1Database,
    tenant: &str,
    pair: &CanonicalPartyPair,
    first_id: &str,
) -> Result<Option<PartyComparison>> {
    let Some(left) = parties::stored(db, tenant, pair.left().as_str()).await? else {
        return Ok(None);
    };
    let Some(right) = parties::stored(db, tenant, pair.right().as_str()).await? else {
        return Ok(None);
    };
    let (first, second) = if first_id == pair.left().as_str() {
        (left, right)
    } else {
        (right, left)
    };
    Ok(Some(PartyComparison { first, second }))
}
