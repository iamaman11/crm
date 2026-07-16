const VISIBILITY_SOURCE: &str = include_str!(
    "../../../crates/crm-capability-adapters/src/query_visibility.rs"
);
const PROCESS_SOURCE: &str = include_str!("data_quality_field_redaction_process_e2e.rs");

#[test]
fn live_data_quality_field_redaction_proof_remains_wired() {
    for required in [
        "CRM_QUERY_HIDDEN_FIELDS",
        "deployment-field-ceiling/v1",
        "data_quality.party.rule_set.get|crm.data-quality|data_quality.party_rule_set_version|definition",
        "data_quality.party.completeness_profile.get|crm.data-quality|data_quality.party_rule_set_version|definition",
        "redacted_rule_set_version.definition.is_none()",
        "redacted_profile_version.definition.is_none()",
        "cross_tenant.message(), missing.message()",
    ] {
        assert!(
            VISIBILITY_SOURCE.contains(required) || PROCESS_SOURCE.contains(required),
            "missing live field-redaction acceptance marker: {required}"
        );
    }
}
