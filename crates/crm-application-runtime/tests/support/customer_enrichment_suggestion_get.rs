#[path = "customer_enrichment_suggestion_get/domain.rs"]
mod domain;
#[path = "customer_enrichment_suggestion_get/list.rs"]
mod list;
#[path = "customer_enrichment_suggestion_get/transport.rs"]
mod transport;

pub use domain::*;
pub use list::*;
pub use transport::*;
