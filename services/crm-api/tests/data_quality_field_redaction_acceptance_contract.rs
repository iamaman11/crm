const PROCESS_SOURCE: &str = include_str!("data_quality_field_redaction_process_e2e.rs");

#[test]
fn live_data_quality_field_redaction_proof_remains_wired() {
    assert_eq!(PROCESS_SOURCE.matches("definition.is_none()").count(), 2);
    assert!(PROCESS_SOURCE.contains("Some(HIDDEN_FIELDS)"));
    assert!(PROCESS_SOURCE.contains("cross_tenant.message(), missing.message()"));
}
