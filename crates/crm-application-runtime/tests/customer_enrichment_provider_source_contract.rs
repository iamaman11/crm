use crm_application_runtime::GovernedCustomerEnrichmentProviderSource;
use crm_customer_enrichment_provider_process_composition::ProviderDispatchSourcePort;

#[test]
fn governed_provider_source_is_a_thread_safe_internal_process_port() {
    fn assert_contract<T>()
    where
        T: ProviderDispatchSourcePort + Clone + std::fmt::Debug + Send + Sync + 'static,
    {
    }

    assert_contract::<GovernedCustomerEnrichmentProviderSource>();
}
