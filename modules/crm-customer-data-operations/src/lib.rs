//! Pure customer-data import coordination domain.
//!
//! This crate owns import job, immutable source/mapping identity, deterministic row identity,
//! row outcome and resumable checkpoint semantics. It does not own Party records and has no
//! infrastructure or direct customer-master storage access. Target-owner writes are intentionally
//! deferred to governed application composition rather than exposed from this pure domain crate.

pub mod domain;

pub use domain::*;
