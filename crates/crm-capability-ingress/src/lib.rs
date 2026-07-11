#![forbid(unsafe_code)]

//! Authenticated HTTP and gRPC ingress for governed capability and query gateways.
//! Transport adapters in this crate construct the complete execution context
//! and never call business modules or persistence adapters directly.

mod authentication;
mod context;
mod core;
mod grpc;
mod http;
mod query_context;
mod query_core;
mod query_grpc;
mod query_http;

pub use authentication::*;
pub use context::*;
pub use core::*;
pub use grpc::*;
pub use http::*;
pub use query_context::*;
pub use query_core::*;
pub use query_grpc::*;
pub use query_http::*;

/// Architecture marker for `crm-capability-ingress`.
pub const CRATE_NAME: &str = "crm-capability-ingress";
