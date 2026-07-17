use crm_customer_enrichment::{CRATE_NAME, MODULE_ID};

#[test]
#[ignore = "foundation gate: replace with governed production acceptance before raising readiness"]
fn production_acceptance_todo() {
    assert_eq!(CRATE_NAME, "crm-customer-enrichment");
    assert_eq!(MODULE_ID, "crm.customer-enrichment");
    panic!("replace foundation acceptance placeholder with governed production acceptance");
}
