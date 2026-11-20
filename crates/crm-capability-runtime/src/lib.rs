#![forbid(unsafe_code)]

//! Deterministic, infrastructure-neutral capability execution boundary.
//!
//! The runtime resolves an exact versioned capability, verifies its typed input
//! contract, runs semantic/rate/approval checks, performs live authorization as
//! the final awaited decision and delegates all side effects to one transactional
//! executor port.

pub mod gateway;
pub mod ports;
pub mod testing;
pub mod types;

pub use gateway::*;
pub use ports::*;
pub use types::*;
