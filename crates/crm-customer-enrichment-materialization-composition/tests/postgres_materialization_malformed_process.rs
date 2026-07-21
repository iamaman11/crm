#![cfg(feature = "postgres-integration")]

include!("support/materialization_malformed/imports.rs");
include!("support/materialization_malformed/fixture_domain.rs");
include!("support/materialization_malformed/fixture_store.rs");
include!("support/materialization_malformed/test.rs");
