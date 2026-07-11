#![forbid(unsafe_code)]

//! Production process composition for Ultimate CRM.
//!
//! This crate is the only boundary that assembles infrastructure, governed
//! gateways, transport middleware and background workers. Business owner
//! modules remain outside the process host and are reached only through their
//! published composition/adapters.

mod config;
mod gateway_grpc;
mod platform;
mod process;
mod runtime;

pub use config::*;
pub use gateway_grpc::*;
pub use platform::*;
pub use process::*;
pub use runtime::*;

pub const CRATE_NAME: &str = "crm-application-runtime";
