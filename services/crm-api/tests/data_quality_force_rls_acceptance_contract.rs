#![cfg(unix)]

#[test]
fn data_quality_process_acceptance_keeps_direct_application_role_force_rls_proof() {
    let acceptance = include_str!("data_quality_query_process_e2e.rs");

    for required_evidence in [
        "SELECT current_user",
        "assert_eq!(current_user, \"crm_app_test\")",
        "FORCE RLS must hide tenant B Data Quality records under tenant A context",
        "application role must see the same record under its owning tenant context",
    ] {
        assert!(
            acceptance.contains(required_evidence),
            "Data Quality process acceptance lost required FORCE RLS evidence: {required_evidence}"
        );
    }
}
