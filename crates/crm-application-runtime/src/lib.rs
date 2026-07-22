#![forbid(unsafe_code)]

//! Production process composition for Ultimate CRM.
//!
//! This crate is the only boundary that assembles infrastructure, governed
//! gateways, transport middleware and background workers. Business owner
//! modules remain outside the process host and are reached only through their
//! published composition/adapters.

mod background;
mod bootstrap_visibility;
mod config;
mod customer_enrichment_application_worker;
mod customer_enrichment_materialization_process;
mod customer_enrichment_provider_process;
mod customer_enrichment_provider_registry;
mod customer_enrichment_provider_source;
mod customer_enrichment_provider_worker;
mod customer_enrichment_reject_promotion;
mod customer_enrichment_suggestion_list_promotion;
mod customer_privacy_case_create_promotion;
#[cfg(feature = "customer-privacy-subject-verify-candidate")]
mod customer_privacy_subject_verify_candidate;
mod data_quality_capability_execution;
mod data_quality_registration;
mod export_artifact_download;
mod export_artifact_download_http;
mod export_execution_source;
mod export_selection_bootstrap;
mod export_selection_source;
mod gateway_grpc;
mod native_composition;
mod platform;
mod process;
mod runtime;

pub(crate) use background::{
    ProductionBackgroundWorkerDependencies, build_production_background_workers,
};
pub(crate) use bootstrap_visibility::{
    BootstrapVisibilityResource, build_bootstrap_visibility_registry,
};
#[cfg(not(feature = "customer-privacy-subject-verify-candidate"))]
pub(crate) use customer_privacy_case_create_promotion::build_production_composition as build_process_composition;
#[cfg(feature = "customer-privacy-subject-verify-candidate")]
pub(crate) use customer_privacy_subject_verify_candidate::build_candidate_process_composition as build_process_composition;
pub use config::*;
pub use customer_enrichment_application_worker::{
    CustomerEnrichmentApplicationWorkerDependencies, OWNER_APPLICATION_POLICY_VERSION,
    build_customer_enrichment_application_worker,
};
pub use customer_enrichment_materialization_process::{
    CustomerEnrichmentMaterializationProcessDependencies,
    build_customer_enrichment_materialization_process,
};
pub use customer_enrichment_provider_process::{
    CustomerEnrichmentProviderProcessDependencies, build_customer_enrichment_provider_process,
};
pub use customer_enrichment_provider_registry::{
    ProcessProviderSecretValueSource, ProviderSecretValueSourcePort, ProviderTransportCatalogPort,
    ProviderTransportRegistration, StaticProviderTransportCatalog,
    build_customer_enrichment_provider_registry,
    build_process_customer_enrichment_provider_transport_catalog,
};
pub use customer_enrichment_provider_source::GovernedCustomerEnrichmentProviderSource;
pub use customer_enrichment_provider_worker::{
    CustomerEnrichmentProviderWorkerDependencies, build_customer_enrichment_provider_worker,
};
pub use customer_privacy_case_create_promotion::{
    PRODUCTION_REVIEW_POLICY_VERSION, application_mutation_definitions,
    application_query_definitions, build_production_composition,
};
pub use data_quality_capability_execution::DataQualityCapabilityExecutor;
pub use data_quality_registration::*;
pub use export_artifact_download::*;
pub(crate) use export_artifact_download_http::export_artifact_download_router;
pub use export_execution_source::*;
pub(crate) use export_selection_bootstrap::bootstrap_export_selection_worker_access;
pub use export_selection_source::*;
pub use gateway_grpc::*;
pub use native_composition::{
    PostgresModuleActivation, ProductionCompositionDependencies, application_capability_catalog,
};
pub use platform::*;
pub use process::*;
pub use runtime::*;

pub const CRATE_NAME: &str = "crm-application-runtime";

pub fn declared_business_module_ids() -> std::collections::BTreeSet<String> {
    let mut module_ids = native_composition::declared_business_module_ids();
    module_ids.insert("crm.customer-privacy".to_owned());
    module_ids
}
