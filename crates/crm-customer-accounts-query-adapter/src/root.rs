#![forbid(unsafe_code)]

#[path = "lib.rs"]
mod query;
mod production_contribution;

pub use production_contribution::{
    CustomerAccountsProductionDependencies, build_contribution,
    mutation_capability_definitions,
};
pub use query::*;
