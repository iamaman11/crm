#![expect(
    clippy::too_many_arguments,
    reason = "strict customer-data persisted-state validation currently checks canonical import/export state shapes; remove this expectation when those helpers are refactored to typed state values"
)]

//! Pure customer-data operations coordination domain.
//!
//! This crate owns governed import/export job identity, immutable source/specification/profile
//! evidence, deterministic row/work identity, bounded outcomes, reconciliation and resumable
//! checkpoint semantics. It does not own Party or other customer-master records and has no direct
//! customer-master storage access. Owner-domain reads and writes are intentionally deferred to
//! governed application composition rather than exposed from this pure domain crate.
//!
//! Import source-system identifiers remain import-owned evidence and never become canonical Party
//! IDs. Exact source bytes are interpreted only by the versioned strict parser profile before any
//! validated row state is planned. Import target writes re-enter the exact owner capability path.
//!
//! Export jobs bind immutable specification and selection evidence. Exported bytes are derived
//! artifacts, never authoritative customer state. Selection and serialization must use governed
//! owner-domain query composition with live authorization; the pure domain owns only lifecycle,
//! checkpoint, artifact-reference and reconciliation invariants.

pub mod domain;
pub mod execution;
pub mod export;
pub mod export_persistence;
pub mod export_selection;
pub mod export_selection_persistence;
pub mod persistence;
pub mod profile;
pub mod source_parser;
pub mod validation;

pub use domain::*;
pub use execution::*;
pub use export::*;
pub use export_persistence::*;
pub use export_selection::*;
pub use export_selection_persistence::*;
pub use persistence::*;
pub use profile::*;
pub use source_parser::*;
pub use validation::*;
