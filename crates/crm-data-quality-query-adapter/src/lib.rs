#![forbid(unsafe_code)]

mod party_source_registry;
pub use party_source_registry::{
    register_party_quality_query_adapter, registered_party_quality_query_adapter,
};

include!("query_support.rs");
include!("query_definition_gets.rs");
include!("query_finding_get.rs");
include!("query_lists.rs");
include!("query_store.rs");
include!("query_catalog.rs");
include!("query_dispatch.rs");
include!("query_wire_evaluation.rs");
include!("query_wire_finding.rs");
include!("query_wire_completeness.rs");
include!("query_decode.rs");
include!("query_filters.rs");
include!("query_common.rs");
