use crate::contract::{
    CAPABILITY_ID, CAPABILITY_VERSION, DEFAULT_PAGE_SIZE, MAXIMUM_CURSOR_BYTES, MAXIMUM_PAGE_SIZE,
    MAX_PRIVACY_ASSOCIATION_RECORDS_REHYDRATED, MAX_PRIVACY_CANONICAL_PARTY_RESOLUTIONS,
    MAX_PRIVACY_COMPLETENESS_RESULTS_SCANNED, MAX_PRIVACY_DEFINITION_RECORDS_REHYDRATED,
    MAX_PRIVACY_EVALUATION_INPUTS_SCANNED, MAX_PRIVACY_EVALUATION_JOBS_SCANNED,
    MAX_PRIVACY_FINDINGS_SCANNED, MAX_PRIVACY_FINDING_OBSERVATIONS_SCANNED,
    MAX_PRIVACY_OWNER_RECORDS_SCANNED, MAX_PRIVACY_REMEDIATION_ATTEMPTS_SCANNED,
    MAX_PRIVACY_RULE_OUTCOMES_SCANNED, PRIVACY_OWNER_SCAN_BATCH_SIZE,
    data_quality_privacy_scope_definition,
};
use crate::request::ResourceFamily;
use crm_data_quality_capability_adapter::MODULE_ID;

#[test]
fn definition_is_exact_contract_only_query_coordinate() {
    let definition = data_quality_privacy_scope_definition().unwrap();
    assert_eq!(definition.owner_module_id.as_str(), MODULE_ID);
    assert_eq!(definition.capability_id.as_str(), CAPABILITY_ID);
    assert_eq!(definition.capability_version.as_str(), CAPABILITY_VERSION);
    assert!(!definition.mutation);
    assert!(!definition.requires_idempotency);
    assert!(!definition.requires_approval);
    assert_eq!(definition.authorization_policy_id, CAPABILITY_ID);
}

#[test]
fn frozen_bounds_match_the_entry_packet() {
    assert_eq!(DEFAULT_PAGE_SIZE, 64);
    assert_eq!(MAXIMUM_PAGE_SIZE, 128);
    assert_eq!(MAXIMUM_CURSOR_BYTES, 2_048);
    assert_eq!(MAX_PRIVACY_EVALUATION_JOBS_SCANNED, 8_192);
    assert_eq!(MAX_PRIVACY_EVALUATION_INPUTS_SCANNED, 8_192);
    assert_eq!(MAX_PRIVACY_RULE_OUTCOMES_SCANNED, 32_768);
    assert_eq!(MAX_PRIVACY_FINDINGS_SCANNED, 16_384);
    assert_eq!(MAX_PRIVACY_FINDING_OBSERVATIONS_SCANNED, 32_768);
    assert_eq!(MAX_PRIVACY_COMPLETENESS_RESULTS_SCANNED, 8_192);
    assert_eq!(MAX_PRIVACY_REMEDIATION_ATTEMPTS_SCANNED, 8_192);
    assert_eq!(MAX_PRIVACY_DEFINITION_RECORDS_REHYDRATED, 8_192);
    assert_eq!(MAX_PRIVACY_ASSOCIATION_RECORDS_REHYDRATED, 65_536);
    assert_eq!(MAX_PRIVACY_CANONICAL_PARTY_RESOLUTIONS, 65_536);
    assert_eq!(MAX_PRIVACY_OWNER_RECORDS_SCANNED, 65_536);
    assert_eq!(PRIVACY_OWNER_SCAN_BATCH_SIZE, 512);
}

#[test]
fn seven_family_order_and_tokens_are_stable() {
    let families = [
        ResourceFamily::EvaluationJob,
        ResourceFamily::EvaluationInput,
        ResourceFamily::RuleOutcome,
        ResourceFamily::Finding,
        ResourceFamily::FindingObservation,
        ResourceFamily::CompletenessResult,
        ResourceFamily::RemediationAttempt,
    ];
    assert_eq!(
        families.map(ResourceFamily::token),
        [
            "job",
            "input",
            "outcome",
            "finding",
            "observation",
            "completeness",
            "remediation",
        ]
    );
    assert!(families.windows(2).all(|pair| pair[0] < pair[1]));
}
