use crm_customer_enrichment::{CRATE_NAME, MODULE_ID};
use serde_json::Value;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

const ACCEPTED_SOURCE_CHECKPOINT: &str = "f92d101206886e3ceaf94d0e56e52580cec21093";
const MERGE_COMMIT: &str = "150e44b95d9dbdc08c1792563de03ec73f34aed1";

#[test]
fn production_acceptance_is_bound_to_exact_inventory_and_real_process_evidence() {
    assert_eq!(CRATE_NAME, "crm-customer-enrichment");
    assert_eq!(MODULE_ID, "crm.customer-enrichment");

    let root = repository_root();
    let promotion =
        read_json(&root.join("contracts/customer-enrichment-production-promotion.json"));
    assert_eq!(promotion["module_id"], MODULE_ID);
    assert_eq!(
        promotion["schema_version"],
        "crm.customer-enrichment.production-promotion/v1"
    );
    assert_eq!(
        promotion["accepted_source_checkpoint"],
        ACCEPTED_SOURCE_CHECKPOINT
    );
    assert_eq!(promotion["merge_commit"], MERGE_COMMIT);

    let inventory = &promotion["current_runtime_inventory"];
    assert_exact_coordinates(
        &inventory["mutations"],
        &[
            "customer_enrichment.provider_profile.publish@1.0.0",
            "customer_enrichment.mapping.publish@1.0.0",
            "customer_enrichment.request.create@1.0.0",
            "customer_enrichment.request.cancel@1.0.0",
            "customer_enrichment.suggestion.reject@1.0.0",
            "customer_enrichment.suggestion.accept@1.0.0",
        ],
    );
    assert_exact_coordinates(
        &inventory["queries"],
        &[
            "customer_enrichment.provider_profile.get@1.0.0",
            "customer_enrichment.mapping.get@1.0.0",
            "customer_enrichment.request.get@1.0.0",
            "customer_enrichment.request.list@1.0.0",
            "customer_enrichment.suggestion.get@1.0.0",
            "customer_enrichment.suggestion.list_by_party@1.0.0",
        ],
    );
    assert_exact_coordinates(
        &inventory["workers"],
        &[
            "customer_enrichment.request.dispatch@1.0.0",
            "customer_enrichment.response.record@1.0.0",
            "customer_enrichment.suggestions.materialize@1.0.0",
            "customer_enrichment.party.display_name.apply@1.0.0",
            "customer_enrichment.application.outcome.record@1.0.0",
        ],
    );
    assert_eq!(
        promotion["global_invariants"]["required_exact_head_workflows"],
        17
    );
    assert_eq!(
        promotion["global_invariants"]["central_business_route_switches_allowed"],
        false
    );
    assert_eq!(promotion["promotion_stages"][0]["state"], "complete");

    let classifications = read_json(&root.join("contracts/production-route-classifications.json"));
    let worker_coordinates =
        classified_coordinates(&classifications["worker_runtime_routes"], MODULE_ID);
    let expected_workers = inventory["workers"]
        .as_array()
        .expect("worker inventory must be an array")
        .iter()
        .map(|value| {
            value
                .as_str()
                .expect("worker coordinate must be a string")
                .to_owned()
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(worker_coordinates, expected_workers);

    let non_runtime = classified_coordinates(
        &classifications["non_runtime_contract_routes"],
        MODULE_ID,
    );
    assert_eq!(
        non_runtime,
        BTreeSet::from(["customer_enrichment.privacy.scope.contribute@1.0.0".to_owned()]),
        "only the separately governed privacy scope contribution may remain contract-only"
    );
    for category in ["mutations", "queries", "workers"] {
        for coordinate in inventory[category]
            .as_array()
            .expect("runtime inventory category must be an array")
        {
            let coordinate = coordinate
                .as_str()
                .expect("runtime inventory coordinate must be a string");
            assert!(
                !non_runtime.contains(coordinate),
                "completed production coordinate remained non-runtime: {coordinate}"
            );
        }
    }

    let owner_scope =
        read_json(&root.join("contracts/customer-privacy-owner-scope-contracts.json"));
    let enrichment_scope = owner_scope["owners"]
        .as_array()
        .expect("owner scope registry must be an array")
        .iter()
        .find(|entry| entry["module_id"] == MODULE_ID)
        .expect("Customer Enrichment scope contract must be registered");
    assert_eq!(
        enrichment_scope["capability_id"],
        "customer_enrichment.privacy.scope.contribute"
    );
    assert_eq!(enrichment_scope["version"], "1.0.0");
    assert_eq!(owner_scope["state"], "contract_only_non_runtime");

    let workflow = read_text(&root.join(".github/workflows/application-runtime.yml"));
    assert!(workflow.contains("customer_enrichment_process_e2e"));
    assert!(workflow.contains("prepare_customer_enrichment_consent_policy_database.sh"));

    let process =
        read_text(&root.join("services/crm-api/tests/customer_enrichment_process_e2e.rs"));
    for evidence in [
        "CUSTOMER_ENRICHMENT_REQUEST_CONSENT_DENIED",
        "MODULE_NOT_ACTIVE",
        "CAPABILITY_PERMISSION_DENIED",
        "legitimate_interest_request_payload",
    ] {
        assert!(
            process.contains(evidence),
            "missing process evidence {evidence}"
        );
    }

    let transport = read_text(
        &root.join("services/crm-api/tests/support/customer_enrichment_process/transport.rs"),
    );
    assert!(transport.contains("CARGO_BIN_EXE_crm-api"));
}

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("resolve repository root")
}

fn read_json(path: &Path) -> Value {
    serde_json::from_str(&read_text(path)).expect("parse governed JSON contract")
}

fn read_text(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
}

fn assert_exact_coordinates(value: &Value, expected: &[&str]) {
    let actual = value
        .as_array()
        .expect("production inventory entry must be an array")
        .iter()
        .map(|value| value.as_str().expect("coordinate must be a string"))
        .collect::<Vec<_>>();
    assert_eq!(actual, expected);
}

fn classified_coordinates(value: &Value, owner_module_id: &str) -> BTreeSet<String> {
    value
        .as_array()
        .expect("route classification must be an array")
        .iter()
        .filter(|entry| entry["owner_module_id"] == owner_module_id)
        .map(|entry| {
            format!(
                "{}@{}",
                entry["id"]
                    .as_str()
                    .expect("classified id must be a string"),
                entry["version"]
                    .as_str()
                    .expect("classified version must be a string")
            )
        })
        .collect()
}
