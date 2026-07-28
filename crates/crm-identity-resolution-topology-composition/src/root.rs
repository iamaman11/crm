#![forbid(unsafe_code)]

mod current_canonical;
#[path = "lib.rs"]
mod topology;

pub use current_canonical::*;
pub use topology::*;
