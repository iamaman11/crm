#[path = "customer_enrichment_suggestion_get/domain.rs"]
#[allow(dead_code)]
mod domain;
#[path = "customer_enrichment_suggestion_get/list.rs"]
mod list;
#[path = "customer_enrichment_suggestion_get/transport.rs"]
mod transport;

pub use domain::*;
// The umbrella support module is compiled by get/list, review and application
// integration targets; only the read-surface target consumes list helpers.
#[allow(unused_imports)]
pub use list::*;
pub use transport::*;
