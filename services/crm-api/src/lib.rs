#![forbid(unsafe_code)]

//! Composition boundary for public CRM transports.
//! Route implementations must call the governed capability ingress and must not
//! invoke module mutation or PostgreSQL batch APIs directly.

pub use crm_capability_ingress::{GrpcCapabilityMiddleware, HttpCapabilityMiddleware};

/// Architecture marker for `crm-api`.
pub const CRATE_NAME: &str = "crm-api";
