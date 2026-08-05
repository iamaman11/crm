use crm_application_runtime::{application_mutation_definitions, application_query_definitions};
use serde::Deserialize;
use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

const PRIVACY_OWNER: &str = "crm.customer-privacy";
const CREATE: &str = "customer_privacy.case.create";
const SUBMIT: &str = "customer_privacy.case.submit";
const SUBJECT_VERIFY: &str = "customer_privacy.case.subject.verify";
const APPROVE: &str = "customer_privacy.case.approve";
const CANCEL: &str = "customer_privacy.case.cancel";
const RESTRICTION_PLACE: &str = "customer_privacy.restriction.place";
const RESTRICTION_RELEASE: &str = "customer_privacy.restriction.release";
const LEGAL_HOLD_PLACE: &str = "customer_privacy.legal_hold.place";
const LEGAL_HOLD_RELEASE: &str = "customer_privacy.legal_hold.release";
const GET_CASE: &str = "customer_privacy.case.get";
const LIST_CASES: &str = "customer_privacy.case.list";
const GET_RESTRICTION: &str = "customer_privacy.restriction.get";
const GET_LEGAL_HOLD: &str = "customer_privacy.legal_hold.get";
const LIST_LEGAL_HOLDS: &str = "customer_privacy.legal_hold.list_by_subject";
const GET_PLAN: &str = "customer_privacy.case.plan.get";
const LIST_OWNER_OUTCOMES: &str = "customer_privacy.case.owner_outcomes.list";

#[derive(Debug, Deserialize)]
struct RouteClassifications {
    worker_runtime_routes: Vec<ClassifiedRoute>,
    non_runtime_contract_routes: Vec<ClassifiedRoute>,
}

#[derive(Debug, Deserialize)]
struct ClassifiedRoute {
    owner_module_id: String,
    id: String,
    version: String,
}

#[test]
fn customer_privacy_runtime_inventory_is_exactly_nine_mutations_and_seven_queries() {
    let runtime_privacy_mutations = application_mutation_definitions()
        .unwrap()
        .into_iter()
        .filter(|definition| definition.owner_module_id.as_str() == PRIVACY_OWNER)
        .map(|definition| {
            (
                definition.capability_id.as_str().to_owned(),
                definition.capability_version.as_str().to_owned(),
            )
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        runtime_privacy_mutations,
        BTreeSet::from([
            (CREATE.to_owned(), "1.0.0".to_owned()),
            (SUBMIT.to_owned(), "1.0.0".to_owned()),
            (SUBJECT_VERIFY.to_owned(), "1.0.0".to_owned()),
            (APPROVE.to_owned(), "1.0.0".to_owned()),
            (CANCEL.to_owned(), "1.0.0".to_owned()),
            (RESTRICTION_PLACE.to_owned(), "1.0.0".to_owned()),
            (RESTRICTION_RELEASE.to_owned(), "1.0.0".to_owned()),
            (LEGAL_HOLD_PLACE.to_owned(), "1.0.0".to_owned()),
            (LEGAL_HOLD_RELEASE.to_owned(), "1.0.0".to_owned()),
        ])
    );

    let runtime_privacy_queries = application_query_definitions()
        .unwrap()
        .into_iter()
        .filter(|definition| definition.owner_module_id.as_str() == PRIVACY_OWNER)
        .map(|definition| {
            (
                definition.capability_id.as_str().to_owned(),
                definition.capability_version.as_str().to_owned(),
            )
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        runtime_privacy_queries,
        BTreeSet::from([
            (GET_CASE.to_owned(), "1.0.0".to_owned()),
            (LIST_CASES.to_owned(), "1.0.0".to_owned()),
            (GET_RESTRICTION.to_owned(), "1.0.0".to_owned()),
            (GET_LEGAL_HOLD.to_owned(), "1.0.0".to_owned()),
            (LIST_LEGAL_HOLDS.to_owned(), "1.0.0".to_owned()),
            (GET_PLAN.to_owned(), "1.0.0".to_owned()),
            (LIST_OWNER_OUTCOMES.to_owned(), "1.0.0".to_owned()),
        ])
    );
}

#[test]
fn all_public_privacy_routes_are_runtime_and_worker_inventory_is_unchanged() {
    let classifications = classifications();
    let actual_non_runtime = classifications
        .non_runtime_contract_routes
        .iter()
        .filter(|route| route.owner_module_id == PRIVACY_OWNER)
        .map(|route| (route.id.clone(), route.version.clone()))
        .collect::<BTreeSet<_>>();
    assert!(actual_non_runtime.is_empty());

    let actual_workers = classifications
        .worker_runtime_routes
        .iter()
        .map(|route| {
            (
                route.owner_module_id.clone(),
                route.id.clone(),
                route.version.clone(),
            )
        })
        .collect::<BTreeSet<_>>();
    let expected_workers = [
        "customer_enrichment.request.dispatch",
        "customer_enrichment.response.record",
        "customer_enrichment.suggestions.materialize",
        "customer_enrichment.party.display_name.apply",
        "customer_enrichment.application.outcome.record",
    ]
    .into_iter()
    .map(|id| {
        (
            "crm.customer-enrichment".to_owned(),
            id.to_owned(),
            "1.0.0".to_owned(),
        )
    })
    .collect::<BTreeSet<_>>();
    assert_eq!(actual_workers, expected_workers);

    assert!(
        classifications
            .worker_runtime_routes
            .iter()
            .chain(classifications.non_runtime_contract_routes.iter())
            .all(|route| !route.id.contains("crypto_shred") && !route.id.contains("crypto-shred")),
        "control lifecycle promotion may not introduce or reclassify crypto-shred coordinates"
    );
}

fn classifications() -> RouteClassifications {
    serde_json::from_slice(
        &fs::read(root().join("contracts/production-route-classifications.json")).unwrap(),
    )
    .unwrap()
}

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
