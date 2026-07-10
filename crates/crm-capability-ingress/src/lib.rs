#![forbid(unsafe_code)]

//! Authenticated HTTP and gRPC ingress for the capability gateway.
//! Transport adapters in this crate construct the complete execution context
//! and never call business modules or persistence adapters directly.

mod authentication;
mod context;
mod core;
mod grpc;
mod http;

pub use authentication::*;
pub use context::*;
pub use core::*;
pub use grpc::*;
pub use http::*;

/// Architecture marker for `crm-capability-ingress`.
pub const CRATE_NAME: &str = "crm-capability-ingress";
