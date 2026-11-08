//! CI-only, *runtime-executed* original owner-codec checks in the same native
//! workers-rs WASM module. A PASS is not a D1/UI capability acceptance.
use serde_json::{Value, json};
use worker::{Response, Result};

pub fn run() -> Result<Response> {
    // Each expression calls the actual original Rust implementation in workerd.
    // Invalid persisted state MUST be rejected, not silently deserialized.
    // A real domain/D1 E2E is a separate, stronger acceptance condition.
    let checks: [(&str, bool); 13] = [
        (crm_parties::MODULE_ID,
            crm_parties::decode_party_state(b"{}").is_err()
                && crm_parties::party_state_descriptor_hash() != [0; 32]),
        (crm_customer_accounts::MODULE_ID,
            crm_customer_accounts::decode_account_state(b"{}").is_err()
                && crm_customer_accounts::account_state_descriptor_hash() != [0; 32]),
        (crm_contact_points::MODULE_ID,
            crm_contact_points::decode_contact_point_state(b"{}").is_err()
                && crm_contact_points::contact_point_state_descriptor_hash() != [0; 32]),
        (crm_party_relationships::MODULE_ID,
            crm_party_relationships::decode_party_relationship_state(b"{}").is_err()
                && crm_party_relationships::party_relationship_state_descriptor_hash() != [0; 32]),
        (crm_consents::MODULE_ID,
            crm_consents::decode_consent_authorization_state(b"{}").is_err()
                && crm_consents::consent_authorization_state_descriptor_hash() != [0; 32]),
        ("crm.sales",
            crm_sales::decode_deal_state(b"{}").is_err()
                && crm_sales::deal_state_descriptor_hash() != [0; 32]),
        ("crm.activities",
            crm_activities::decode_task_state(b"{}").is_err()
                && crm_activities::task_state_descriptor_hash() != [0; 32]),
        (crm_identity_resolution::MODULE_ID,
            crm_identity_resolution::decode_duplicate_candidate_case_state(b"{}").is_err()
                && crm_identity_resolution::duplicate_candidate_case_state_descriptor_hash() != [0; 32]),
        ("crm.customer-data-operations",
            crm_customer_data_operations::decode_import_job_state(b"{}").is_err()
                && crm_customer_data_operations::import_job_state_descriptor_hash() != [0; 32]),
        (crm_data_quality::MODULE_ID,
            crm_data_quality::decode_party_rule_set_version_state(b"{}").is_err()
                && crm_data_quality::party_rule_set_version_state_descriptor_hash() != [0; 32]),
        (crm_customer_enrichment::MODULE_ID,
            crm_customer_enrichment::decode_enrichment_request_state(b"{}").is_err()
                && crm_customer_enrichment::enrichment_request_state_descriptor_hash() != [0; 32]),
        (crm_customer_privacy::MODULE_ID,
            crm_customer_privacy::decode_privacy_case_state(b"{}").is_err()
                && crm_customer_privacy::privacy_case_state_descriptor_hash() != [0; 32]),
        (crm_sales_activities_link::MODULE_ID,
            serde_json::from_str::<crm_sales_activities_link::DealLifecycleStatus>(r#""open""#).is_ok()
                && serde_json::from_str::<crm_sales_activities_link::DealLifecycleStatus>(r#""invalid""#).is_err()
                && crm_sales_activities_link::SOURCE_MODULE_ID == "crm.sales"
                && crm_sales_activities_link::TARGET_MODULE_ID == "crm.activities"),
    ];
    let items: Vec<Value> = checks.iter().map(|(module, passed)| json!({
        "module": module,
        "runtime_proof": "original_rust_domain_codec_or_link_contract",
        "passed": passed,
        "http_api": "NOT_TESTED",
        "d1_persistence": "NOT_TESTED",
        "user_ui": "NOT_TESTED"
    })).collect();
    let all = checks.iter().all(|(_, passed)| *passed);
    let status = if all { 200 } else { 503 };
    Response::from_json(&json!({
        "result": if all { "PASS" } else { "FAIL" },
        "scope": "local_workerd_single_wasm_original_rust_smoke_only",
        "original_crates_called": checks.len(),
        "full_application_integrations_accepted": 0,
        "modules": items
    })).map(|r| r.with_status(status))
}
