#![forbid(unsafe_code)]

//! Governed execution composition for validated customer-data Party imports.
//!
//! The pure customer-data-operations domain owns import coordination state while
//! `crm.parties` remains the only Party owner. This composition selects the next
//! authoritative source position, invokes exact Party creation through a
//! `CapabilityClient`, and delegates import-owned outcome/checkpoint persistence
//! to a private sink. No public bulk-write or direct Party storage path exists here.

pub mod export_execution_reader;
pub use export_execution_reader::*;
pub mod export_execution_sink;
pub use export_execution_sink::*;
pub mod export_execution_source;
pub use export_execution_source::*;
pub mod export_execution_worker;
pub use export_execution_worker::*;
pub mod export_selection_reader;
pub use export_selection_reader::*;
pub mod export_selection_sink;
pub use export_selection_sink::*;
pub mod export_selection_worker;
pub use export_selection_worker::*;
pub mod postgres_reader;
pub use postgres_reader::*;
pub mod outcome_plan;
pub use outcome_plan::*;
pub mod postgres_outcome_sink;
pub use postgres_outcome_sink::*;
pub mod worker;
pub use worker::*;

pub const MODULE_ID: &str = "crm.customer-data-operations";
pub const CONTRACT_VERSION: &str = "1.0.0";
