//! Read-only operator comparison of two authoritative Party records.
//! Uses the ORIGINAL identity-resolution canonical pair contract. This is NOT
//! candidate discovery, a scored MatchEvidenceSnapshot, or a merge operation.
//! There is no trustworthy matcher/evidence producer connected yet.
use crm_identity_resolution::{CanonicalPartyPair, PartyReference};
use serde_json::json;
use worker::{D1Database, Method, Request, Response, Result};

use super::{error, parties, response};

pub(super) async fn route(req: Request, db: D1Database, tenant: &str) -> Result<Response> {
    if req.method() != Method::Get {
        return error("NOT_FOUND", 404);
    }
    let path = req.path();
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
