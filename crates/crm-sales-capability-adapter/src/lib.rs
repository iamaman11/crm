#![forbid(unsafe_code)]

#[expect(
    deprecated,
    reason = "Published Sales v1 compatibility fields remain populated until their governed contract-retirement window closes."
)]
mod planner;

pub use planner::*;
