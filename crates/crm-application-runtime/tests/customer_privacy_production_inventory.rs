use crm_application_runtime::{application_mutation_definitions, application_query_definitions};
use serde::Deserialize;
use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

const PRIVACY_OWNER: &str = "crm.customer-privacy";
const CREATE: &str = "customer_privacy.case.create";

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
fn customer_privacy_runtime_inventory_promotes_exactly_case_create() {
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
        BTreeSet::from([(CREATE.to_owned(), "1.0.0".to_owned())])
    );

    assert!(
        application_query_definitions()
            .unwrap()
            .iter()
            .all(|definition| definition.owner_module_id.as_str() != PRIVACY_OWNER),
        "no Customer Privacy query may enter runtime with case.create"
    );
}

#[test]
fn remaining_public_privacy_routes_stay_non_runtime_and_worker_inventory_is_unchanged() {
    let classifications = classifications();
    let actual_non_runtime = classifications
        .non_runtime_contract_routes
        .iter()
        .filter(|route| route.owner_module_id == PRIVACY_OWNER)
        .map(|route| (route.id.clone(), route.version.clone()))
        .collect::<BTreeSet<_>>();
    let expected_non_runtime = [
        "customer_privacy.case.submit",
        "customer_privacy.case.subject.verify",
        "customer_privacy.case.approve",
        "customer_privacy.case.cancel",
        "customer_privacy.case.get",
        "customer_privacy.case.list",
        "customer_privacy.case.plan.get",
        "customer_privacy.case.owner_outcomes.list",
        "customer_privacy.restriction.place",
        "customer_privacy.restriction.release",
        "customer_privacy.restriction.get",
        "customer_privacy.legal_hold.place",
        "customer_privacy.legal_hold.release",
        "customer_privacy.legal_hold.get",
        "customer_privacy.legal_hold.list_by_subject",
    ]
    .into_iter()
    .map(|id| (id.to_owned(), "1.0.0".to_owned()))
    .collect::<BTreeSet<_>>();
    assert_eq!(actual_non_runtime, expected_non_runtime);
    assert!(!actual_non_runtime.iter().any(|(id, _)| id == CREATE));

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
        "case.create promotion may not introduce or reclassify crypto-shred coordinates"
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
