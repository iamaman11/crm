#![forbid(unsafe_code)]

//! Thin production process host for Ultimate CRM.
//!
//! All dependency assembly, governed transports, PostgreSQL adapters and
//! background workers live in `crm-application-runtime`. This package exposes
//! no direct owner-module or infrastructure bypass.

pub use crm_application_runtime::{ApplicationConfig, ApplicationRuntime, run_from_env};

pub const CRATE_NAME: &str = "crm-api";
