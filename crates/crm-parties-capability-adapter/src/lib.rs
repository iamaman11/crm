#![forbid(unsafe_code)]

mod merge_planner;
mod planner;

pub use merge_planner::*;
pub use planner::*;

/// Architecture marker for the production Party capability adapter.
pub const CRATE_NAME: &str = "crm-parties-capability-adapter";
