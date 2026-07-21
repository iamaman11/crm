mod database;
mod fixtures;
mod transport;

pub use database::*;
pub use fixtures::*;
pub use transport::*;

pub const TOKEN: &str = "customer-enrichment-process-token";
pub const ACTOR: &str = "customer-enrichment-process-actor";
pub const TENANT_A: &str = "tenant-a";
pub const TENANT_B: &str = "tenant-b";
pub const TENANT_OUTSIDE_TOKEN: &str = "tenant-c";
pub const MODULE_ID: &str = "crm.customer-enrichment";
pub const SECRET_MARKER: &str = "raw-provider-secret-never-expose";
