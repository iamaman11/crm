use crm_customer_enrichment_application_composition::{
    CustomerEnrichmentPartyApplicationOrchestrator,
    PartyDisplayNameApplicationOrchestrationResult,
};

#[test]
fn application_orchestration_surface_is_explicit_and_non_runtime() {
    fn assert_send_sync<T: Send + Sync>() {}

    assert_send_sync::<CustomerEnrichmentPartyApplicationOrchestrator>();
    assert_send_sync::<PartyDisplayNameApplicationOrchestrationResult>();
}
