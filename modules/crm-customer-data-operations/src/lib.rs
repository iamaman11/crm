#![expect(
    clippy::too_many_arguments,
    reason = "strict import-job persisted-state validation currently checks one canonical counter state shape; remove this expectation when that helper is refactored to a typed state value"
)]

//! Pure customer-data import coordination domain.
//!
//! This crate owns import job, immutable source/parser/mapping identity, deterministic row identity,
//! row outcome and resumable checkpoint semantics. It does not own Party records and has no
//! infrastructure or direct customer-master storage access. Target-owner writes are intentionally
//! deferred to governed application composition rather than exposed from this pure domain crate.
//! Its private job and row state is encoded through strict versioned deterministic persistence.
//! Source-system identifiers remain import-owned evidence and never become canonical Party IDs.
//! Validation progress and finalization are server-derived, version-checked and durably routed
//! through the production application capability boundary before target-owner execution begins.
//! Execution ordering is derived from a complete authoritative source-position index rather than
//! relationship pagination order. The current owner/query boundary is cataloged and routed by the
//! production application runtime; target Party execution remains a separate governed composition.

pub mod domain;
pub mod execution;
pub mod persistence;
pub mod profile;
pub mod validation;

pub use domain::*;
pub use execution::*;
pub use persistence::*;
pub use profile::*;
pub use validation::*;
