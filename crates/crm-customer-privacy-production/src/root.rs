#![forbid(unsafe_code)]

#[path = "lib.rs"]
mod legacy;
mod restriction;

pub use legacy::*;
pub use restriction::*;
