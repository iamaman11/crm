#![expect(
    clippy::too_many_arguments,
    reason = "strict import-job persisted-state validation currently checks one canonical counter state shape; remove this expectation when that helper is refactored to a typed state value"
)]

//! Pure customer-data import coordination domain.
//!
//! This crate owns import job, immutable source/mapping identity, deterministic row identity,
//! row outcome and resumable checkpoint semantics. It does not own Party records and has no
//! infrastructure or direct customer-master storage access. Target-owner writes are intentionally
//! deferred to governed application composition rather than exposed from this pure domain crate.

pub mod domain;

pub use domain::*;
