#![forbid(unsafe_code)]

//! Production process composition for Ultimate CRM.
//!
//! This crate is the only boundary that assembles infrastructure, governed
//! gateways, transport middleware and background workers. Business owner
//! modules remain outside the process host and are reached only through their
//! published composition/adapters.

mod config;
mod data_quality_query_registration;
mod data_quality_registration;
mod export_artifact_download;
mod export_artifact_download_http;
mod export_execution_source;
mod export_selection_bootstrap;
mod export_selection_source;
mod gateway_grpc;
mod governed_metadata;
mod platform;
mod process;
mod runtime;

pub use config::*;
pub use data_quality_query_registration::*;
pub use data_quality_registration::*;
pub use export_artifact_download::*;
pub(crate) use export_artifact_download_http::export_artifact_download_router;
pub use export_execution_source::*;
pub(crate) use export_selection_bootstrap::bootstrap_export_selection_worker_access;
pub use export_selection_source::*;
pub use gateway_grpc::*;
pub use governed_metadata::ApplicationCapabilityExecutorRouter;
pub use platform::*;
pub use process::*;
pub use runtime::*;

pub const CRATE_NAME: &str = "crm-application-runtime";
