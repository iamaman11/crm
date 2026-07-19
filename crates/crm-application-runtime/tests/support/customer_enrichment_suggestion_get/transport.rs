use crm_capability_adapters::{AuthorizationGrant, QueryVisibilityGrant};
use crm_capability_ingress::{
    HttpCapabilityBody, HttpCapabilityMiddleware, HttpCapabilityRequest, HttpQueryBody,
    HttpQueryMiddleware, HttpQueryRequest, IDEMPOTENCY_KEY_HEADER, TENANT_HEADER,
};
use crm_capability_runtime::CapabilityDefinition;
use crm_customer_enrichment::Suggestion;
use crm_module_sdk::{
    ActorId, DataClass, PayloadEncoding, RecordType, RetentionPolicyId, SchemaVersion, TenantId,
    TypedPayload,
};
use crm_proto_contracts::crm::customer_enrichment::v1 as wire;
use http::{HeaderMap, HeaderValue};
use prost::Message;
use std::collections::BTreeSet;

pub const TENANT: &str = "tenant-suggestion-production-a";
pub const OTHER_TENANT: &str = "tenant-suggestion-production-b";
pub const ACTOR: &str = "suggestion-production-reader-a";
pub const NOW: i64 = 1_700_000_700_000_000_000;
pub const SUGGESTION_RECORD_TYPE: &str = "customer_enrichment.suggestion";
pub const REVIEW_RECORD_TYPE: &str = "customer_enrichment.review_decision";
pub const PARTY_RECORD_TYPE: &str = "parties.party";

pub fn access_token() -> String {
    "suggestion-production-access-0001".to_owned()
}

pub fn authorization_grant(definition: &CapabilityDefinition) -> AuthorizationGrant {
    AuthorizationGrant {
        tenant_id: tenant(TENANT),
        actor_id: actor(),
        policy_id: definition.authorization_policy_id.clone(),
        capability_id: definition.capability_id.clone(),
        capability_version: definition.capability_version.clone(),
        owner_module_id: definition.owner_module_id.clone(),
        policy_version: "suggestion-production-auth-v1".to_owned(),
        expires_at_unix_nanos: Some(NOW + 10_000_000_000_000),
    }
}

pub fn visibility_grant(
    definition: &CapabilityDefinition,
    record_type: &str,
    allowed_fields: BTreeSet<String>,
) -> QueryVisibilityGrant {
    QueryVisibilityGrant {
        tenant_id: tenant(TENANT),
        actor_id: actor(),
        capability_id: definition.capability_id.clone(),
        capability_version: definition.capability_version.clone(),
        owner_module_id: definition.owner_module_id.clone(),
        record_type: RecordType::try_new(record_type).unwrap(),
        record_id: None,
        allowed_fields,
        policy_version: "suggestion-production-visibility-v1".to_owned(),
        expires_at_unix_nanos: Some(NOW + 10_000_000_000_000),
    }
}

pub async fn execute_get(
    http: &HttpQueryMiddleware,
    definition: &CapabilityDefinition,
    suggestion: &Suggestion,
    requested_tenant: &'static str,
) -> crm_capability_ingress::HttpQueryResponse {
    execute_query(
        http,
        definition,
        &wire::GetSuggestionRequest {
            suggestion_ref: Some(wire::SuggestionRef {
                suggestion_id: suggestion.suggestion_id().as_str().to_owned(),
            }),
        },
        requested_tenant,
    )
    .await
}

pub async fn execute_query<M: Message>(
    http: &HttpQueryMiddleware,
    definition: &CapabilityDefinition,
    message: &M,
    requested_tenant: &'static str,
) -> crm_capability_ingress::HttpQueryResponse {
    let mut headers = authenticated_headers(requested_tenant);
    http.handle(HttpQueryRequest {
        headers: std::mem::take(&mut headers),
        route: crm_capability_ingress::CapabilityRoute {
            owner_module_id: definition.owner_module_id.clone(),
            capability_id: definition.capability_id.clone(),
            capability_version: definition.capability_version.clone(),
            schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
        },
        input: payload(definition, message),
    })
    .await
}

pub async fn execute_mutation<M: Message>(
    http: &HttpCapabilityMiddleware,
    definition: &CapabilityDefinition,
    message: &M,
    requested_tenant: &'static str,
    idempotency_key: &'static str,
) -> crm_capability_ingress::HttpCapabilityResponse {
    let mut headers = authenticated_headers(requested_tenant);
    headers.insert(
        IDEMPOTENCY_KEY_HEADER,
        HeaderValue::from_static(idempotency_key),
    );
    http.handle(HttpCapabilityRequest {
        headers,
        route: crm_capability_ingress::CapabilityRoute {
            owner_module_id: definition.owner_module_id.clone(),
            capability_id: definition.capability_id.clone(),
            capability_version: definition.capability_version.clone(),
            schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
        },
        input: payload(definition, message),
        approval: None,
    })
    .await
}

fn authenticated_headers(requested_tenant: &'static str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        "authorization",
        HeaderValue::from_str(&format!("Bearer {}", access_token())).unwrap(),
    );
    headers.insert(TENANT_HEADER, HeaderValue::from_static(requested_tenant));
    headers
}

fn payload<M: Message>(definition: &CapabilityDefinition, message: &M) -> TypedPayload {
    let payload = TypedPayload {
        owner: definition.input_contract.owner.clone(),
        schema_id: definition.input_contract.schema_id.clone(),
        schema_version: definition.input_contract.schema_version.clone(),
        descriptor_hash: definition.input_contract.descriptor_hash,
        data_class: DataClass::Personal,
        encoding: PayloadEncoding::Protobuf,
        maximum_size_bytes: definition.input_contract.maximum_size_bytes,
        retention_policy_id: RetentionPolicyId::try_new("standard").unwrap(),
        bytes: message.encode_to_vec(),
    };
    payload
        .validate()
        .expect("valid suggestion transport payload");
    payload
}

pub fn success_payload(body: HttpQueryBody) -> TypedPayload {
    match body {
        HttpQueryBody::Success(result) => result.output,
        HttpQueryBody::Error(error) => panic!("expected success, got {error:?}"),
    }
}

pub fn success_mutation_payload(body: HttpCapabilityBody) -> TypedPayload {
    match body {
        HttpCapabilityBody::Success(result) => result.output.expect("mutation output"),
        HttpCapabilityBody::Error(error) => panic!("expected success, got {error:?}"),
    }
}

pub fn assert_error_code(body: HttpQueryBody, expected: &str) {
    match body {
        HttpQueryBody::Error(error) => assert_eq!(error.code, expected),
        HttpQueryBody::Success(_) => panic!("expected error code {expected}"),
    }
}

pub fn assert_mutation_error_code(body: HttpCapabilityBody, expected: &str) {
    match body {
        HttpCapabilityBody::Error(error) => assert_eq!(error.code, expected),
        HttpCapabilityBody::Success(_) => panic!("expected error code {expected}"),
    }
}

pub fn suggestion_fields() -> BTreeSet<String> {
    [
        "enrichment_request_ref",
        "provider_response_receipt_ref",
        "provider_profile_version_ref",
        "mapping_version_ref",
        "target",
        "proposed_value",
        "proposed_value_digest",
        "observed_at_unix_ms",
        "retrieved_at_unix_ms",
        "effective_at_unix_ms",
        "fresh_until_unix_ms",
        "expires_at_unix_ms",
        "confidence_basis_points",
        "policy_evidence",
        "evidence_references",
        "lifecycle_status",
        "superseded_by_suggestion_ref",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}

pub fn review_fields() -> BTreeSet<String> {
    [
        "suggestion_ref",
        "target_party_resource_version",
        "proposed_value_digest",
        "reviewed_by_actor_id",
        "kind",
        "policy_version",
        "safe_reason_code",
        "approval_evidence_reference",
        "decided_at_unix_ms",
        "expires_at_unix_ms",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}

pub fn tenant(value: &str) -> TenantId {
    TenantId::try_new(value).unwrap()
}

pub fn actor() -> ActorId {
    ActorId::try_new(ACTOR).unwrap()
}
