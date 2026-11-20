#![forbid(unsafe_code)]

//! PostgreSQL composition for promoted Customer Privacy case mutations.
//!
//! Existing mutation adapters remain isolated in the legacy module while the
//! bounded approval runtime enters through its own transaction adapter.

mod approval;
#[path = "legacy.rs"]
mod legacy;

pub use approval::*;
pub use legacy::*;
