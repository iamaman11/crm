#![forbid(unsafe_code)]

mod production_contribution;
#[path = "lib.rs"]
mod query;

pub use production_contribution::{
    CustomerAccountsProductionDependencies, build_contribution, mutation_capability_definitions,
};
pub use query::*;
