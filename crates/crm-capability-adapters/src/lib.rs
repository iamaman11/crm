#![forbid(unsafe_code)]

//! Concrete, concurrency-safe adapters for the capability and query execution gateways.
//! These adapters contain no transport or business-module code and can be
//! replaced by durable stores without changing gateway contracts.

mod approval;
mod authorization;
mod query_visibility;
mod rate_limit;
mod registry;

pub use approval::*;
pub use authorization::*;
pub use query_visibility::*;
pub use rate_limit::*;
pub use registry::*;

/// Architecture marker for `crm-capability-adapters`.
pub const CRATE_NAME: &str = "crm-capability-adapters";
