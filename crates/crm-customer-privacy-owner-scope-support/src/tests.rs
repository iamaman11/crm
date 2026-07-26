use super::{CommonLineageError, framed_digest, validate_common_lineage};
use crm_customer_privacy::{CANONICAL_SCOPE_REGISTRY_VERSION, OwnerScopeRegistry};
use crm_module_sdk::{
    ActorId, CapabilityId, CapabilityVersion, CorrelationId, RequestId, SchemaVersion, TenantId,
    TraceId,
};
use crm_proto_contracts::crm::{customer::v1 as customer, customer_privacy::v1 as privacy};
use crm_query_runtime::QueryExecutionContext;

fn context() -> QueryExecutionContext {
    QueryExecutionContext {
        tenant_id: TenantId::try_new("tenant-a").unwrap(),
        actor_id: ActorId::try_new("privacy-worker").unwrap(),
        request_id: RequestId::try_new("request-shared-scope").unwrap(),
        correlation_id: CorrelationId::try_new("correlation-shared-scope").unwrap(),
        trace_id: TraceId::try_new("trace-shared-scope").unwrap(),
        capability_id: CapabilityId::try_new("owner.privacy.scope.contribute").unwrap(),
        capability_version: CapabilityVersion::try_new("1.0.0").unwrap(),
        schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
        request_started_at_unix_nanos: 2_000_000_000,
    }
}

fn lineage() -> privacy::PrivacyScopeContributionLineage {
    let registry = OwnerScopeRegistry::canonical_v1().unwrap();
    privacy::PrivacyScopeContributionLineage {
        privacy_case_id: "privacy-case-shared".to_owned(),
        tenant_id: "tenant-a".to_owned(),
        canonical_party_ref: Some(customer::PartyRef {
            party_id: "party-a".to_owned(),
        }),
        identity_resolution_generation: 7,
        registry_version: CANONICAL_SCOPE_REGISTRY_VERSION.to_owned(),
        registry_digest_sha256: registry.digest().to_vec(),
        purpose_code: "PRIVACY_ERASURE_SCOPE".to_owned(),
        effective_request_at_unix_ms: 1_000,
    }
}

#[test]
fn framed_digest_preserves_field_boundaries_and_domain() {
    let first = framed_digest(b"privacy-scope/v1", &[b"ab", b"c"]);
    let second = framed_digest(b"privacy-scope/v1", &[b"a", b"bc"]);
    let other_domain = framed_digest(b"privacy-scope/v2", &[b"ab", b"c"]);
    assert_ne!(first, second);
    assert_ne!(first, other_domain);
    assert_eq!(first, framed_digest(b"privacy-scope/v1", &[b"ab", b"c"]));
}

#[test]
fn common_lineage_defaults_page_size_and_preserves_identity() {
    let validated = validate_common_lineage(&context(), lineage(), 0, 64, 128).unwrap();
    assert_eq!(validated.canonical_party_id.as_str(), "party-a");
    assert_eq!(validated.identity_resolution_generation, 7);
    assert_eq!(validated.page_size, 64);
    assert_eq!(validated.lineage.privacy_case_id, "privacy-case-shared");
}

#[test]
fn common_lineage_rejects_registry_substitution_and_future_time() {
    let mut substituted = lineage();
    substituted.registry_digest_sha256 = vec![9; 32];
    assert_eq!(
        validate_common_lineage(&context(), substituted, 64, 64, 128).unwrap_err(),
        CommonLineageError::RegistryMismatch
    );

    let mut future = lineage();
    future.effective_request_at_unix_ms = 3_000;
    assert_eq!(
        validate_common_lineage(&context(), future, 64, 64, 128).unwrap_err(),
        CommonLineageError::RequestTimeInvalid
    );
}
