use crm_customer_enrichment::{CRATE_NAME, MODULE_ID};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

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
    serde_json::from_str(&read_text(path)).expect("parse governed production-promotion contract")
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
