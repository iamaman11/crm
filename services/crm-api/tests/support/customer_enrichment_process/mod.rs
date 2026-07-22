mod database;
mod fixtures;
mod transport;

pub use database::*;
pub use fixtures::*;
pub use transport::*;

pub const TOKEN: &str = "customer-enrichment-process-token";
pub const ACTOR: &str = "actor-a";
pub const TENANT_A: &str = "tenant-a";
pub const TENANT_B: &str = "tenant-b";
pub const TENANT_OUTSIDE_TOKEN: &str = "tenant-c";
pub const MODULE_ID: &str = "crm.customer-enrichment";
pub const SECRET_MARKER: &str = "raw-provider-secret-never-expose";

#[test]
fn exported_process_support_surface_remains_linked() {
    let _ = TOKEN;
    let _ = ACTOR;
    let _ = TENANT_A;
    let _ = TENANT_B;
    let _ = TENANT_OUTSIDE_TOKEN;
    let _ = MODULE_ID;
    let _ = SECRET_MARKER;

    let _ = evidence_counts;
    let _ = set_customer_enrichment_status;

    let _ = PUBLISH_PROFILE;
    let _ = PUBLISH_MAPPING;
    let _ = CREATE_REQUEST;
    let _ = GET_PROFILE;
    let _ = PARTY_CREATE;
    let _ = PARTY_ID;
    let _ = PURPOSE;
    let _ = mutation_definition;
    let _ = query_definition;
    let _ = profile_payload;
    let _ = mapping_payload;
    let _ = party_payload;
    let _ = missing_consent_request_payload;
    let _ = legitimate_interest_request_payload;
    let _ = get_profile_payload;
    let _ = decode_profile_id;
    let _ = decode_mapping_id;
    let _ = decode_profile_query;
    let _ = assert_customer_enrichment_owner;

    let _ = spawn_crm_api;
    let _ = wait_until_ready;
    let _ = connect_grpc;
    let _ = mutate;
    let _ = query;
    let _ = http_mutate;
    let _ = stop_process;
    let _ = free_port;
}
