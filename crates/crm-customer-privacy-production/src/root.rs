#![forbid(unsafe_code)]

#[path = "lib.rs"]
mod legacy;
mod legal_hold;
mod restriction;

pub use legacy::*;
pub use legal_hold::*;
pub use restriction::*;
