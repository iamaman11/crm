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
mod customer_enrichment_suggestion_list_promotion;
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
pub use config::*;
pub use customer_enrichment_suggestion_list_promotion::{
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
    application_mutation_definitions, declared_business_module_ids,
};
pub use platform::*;
pub use process::*;
pub use runtime::*;

pub const CRATE_NAME: &str = "crm-application-runtime";
