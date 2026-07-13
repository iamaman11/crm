use crm_capability_adapters::{
    LiveQueryVisibilityAuthorizer, LiveQueryVisibilityStore, QueryVisibilityGrant,
};
use crm_module_sdk::testing::FixedClock;
use crm_module_sdk::{
    ActorId, CapabilityId, CapabilityVersion, ModuleId, PayloadEncoding, PortFuture, RecordId,
    RecordRef, RecordType, RetentionPolicyId, SchemaVersion, TenantId, TraceId, TypedPayload,
};
use crm_proto_contracts::crm::search::v1 as search_proto;
use crm_query_runtime::{
    CursorCodec, QueryExecutionContext, QueryExecutor, QueryRequest, QueryVisibilityAuthorizer,
};
use crm_search_query_adapter::{
    SEARCH_MODULE_ID, SEARCH_QUERY_CAPABILITY, SearchQueryAdapter,
    search_query_capability_definition,
};
use crm_search_runtime::{
    SearchCandidate, SearchCandidatePage, SearchCandidateRequest, SearchCandidateStore,
    SearchIndexId,
};
use prost::Message;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

const TENANT: &str = "tenant-a";
const ACTOR: &str = "actor-a";
const DEAL_ID: &str = "deal-1";

#[derive(Debug, Default)]
struct OneDealSearchStore;

impl SearchCandidateStore for OneDealSearchStore {
    fn search_candidates<'a>(
        &'a self,
        request: SearchCandidateRequest,
    ) -> PortFuture<'a, Result<SearchCandidatePage, crm_module_sdk::SdkError>> {
        Box::pin(async move {
            if request.after.is_some() {
                return Ok(SearchCandidatePage {
                    candidates: Vec::new(),
                    next_after: None,
                });
            }
            Ok(SearchCandidatePage {
                candidates: vec![SearchCandidate {
                    owner_module_id: ModuleId::try_new("crm.sales").unwrap(),
                    resource: RecordRef {
                        record_type: RecordType::try_new("sales.deal").unwrap(),
                        record_id: RecordId::try_new(DEAL_ID).unwrap(),
                    },
                    source_version: 1,
                    rank_micros: 1_000_000,
                    searchable_fields: BTreeMap::from([(
                        "name".to_owned(),
                        "Acme Enterprise".to_owned(),
                    )]),
                    matched_fields: BTreeSet::from(["name".to_owned()]),
                    display_fields: BTreeMap::from([(
                        "name".to_owned(),
                        "Acme Enterprise".to_owned(),
                    )]),
                }],
                next_after: None,
            })
        })
    }
}

#[tokio::test(flavor = "current_thread")]
async fn live_field_grant_revocation_is_reflected_without_reindexing() {
    let definition = search_query_capability_definition().unwrap();
    let visibility_store = LiveQueryVisibilityStore::default();
    let grant = search_grant(BTreeSet::from(["name".to_owned()]));
    visibility_store.upsert(grant.clone()).unwrap();
    let visibility: Arc<dyn QueryVisibilityAuthorizer> =
        Arc::new(LiveQueryVisibilityAuthorizer::new(
            visibility_store.clone(),
            Arc::new(FixedClock::new(100)),
        ));
    let adapter = SearchQueryAdapter::new(
        SearchIndexId::try_new("crm.global-search").unwrap(),
        Arc::new(OneDealSearchStore),
        visibility,
        CursorCodec::new([7; 32]).unwrap(),
    )
    .unwrap();

    let visible = adapter
        .execute(&definition, query_request(&definition))
        .await
        .unwrap();
    let visible = search_proto::SearchResponse::decode(visible.output.bytes.as_slice()).unwrap();
    assert_eq!(visible.hits.len(), 1);
    assert_eq!(visible.hits[0].resource_id, DEAL_ID);
    assert_eq!(
        visible.hits[0].fields.get("name").unwrap(),
        "Acme Enterprise"
    );
    assert_eq!(visible.hits[0].matched_fields, vec!["name"]);

    assert!(visibility_store.revoke(&grant).unwrap());

    let revoked = adapter
        .execute(&definition, query_request(&definition))
        .await
        .unwrap();
    let revoked = search_proto::SearchResponse::decode(revoked.output.bytes.as_slice()).unwrap();
    assert!(revoked.hits.is_empty());
}

#[tokio::test(flavor = "current_thread")]
async fn match_on_a_hidden_field_discloses_neither_identity_nor_match_metadata() {
    let definition = search_query_capability_definition().unwrap();
    let visibility_store = LiveQueryVisibilityStore::default();
    visibility_store
        .upsert(search_grant(BTreeSet::from(["amount".to_owned()])))
        .unwrap();
    let visibility: Arc<dyn QueryVisibilityAuthorizer> = Arc::new(
        LiveQueryVisibilityAuthorizer::new(visibility_store, Arc::new(FixedClock::new(100))),
    );
    let adapter = SearchQueryAdapter::new(
        SearchIndexId::try_new("crm.global-search").unwrap(),
        Arc::new(OneDealSearchStore),
        visibility,
        CursorCodec::new([8; 32]).unwrap(),
    )
    .unwrap();

    let response = adapter
        .execute(&definition, query_request(&definition))
        .await
        .unwrap();
    let response = search_proto::SearchResponse::decode(response.output.bytes.as_slice()).unwrap();
    assert!(response.hits.is_empty());
}

fn search_grant(allowed_fields: BTreeSet<String>) -> QueryVisibilityGrant {
    QueryVisibilityGrant {
        tenant_id: TenantId::try_new(TENANT).unwrap(),
        actor_id: ActorId::try_new(ACTOR).unwrap(),
        capability_id: CapabilityId::try_new(SEARCH_QUERY_CAPABILITY).unwrap(),
        capability_version: CapabilityVersion::try_new("1.0.0").unwrap(),
        owner_module_id: ModuleId::try_new("crm.sales").unwrap(),
        record_type: RecordType::try_new("sales.deal").unwrap(),
        record_id: None,
        allowed_fields,
        policy_version: "search-visibility-v1".to_owned(),
        expires_at_unix_nanos: Some(1_000),
    }
}

fn query_request(definition: &crm_capability_runtime::CapabilityDefinition) -> QueryRequest {
    let command = search_proto::SearchRequest {
        text: "Acme".to_owned(),
        resource_types: vec!["sales.deal".to_owned()],
        page_size: 25,
        cursor: String::new(),
    };
    let data_class = *definition
        .input_contract
        .allowed_data_classes
        .first()
        .expect("search input contract must declare a data class");
    let payload = TypedPayload {
        owner: definition.input_contract.owner.clone(),
        schema_id: definition.input_contract.schema_id.clone(),
        schema_version: definition.input_contract.schema_version.clone(),
        descriptor_hash: definition.input_contract.descriptor_hash,
        data_class,
        encoding: PayloadEncoding::Protobuf,
        maximum_size_bytes: definition.input_contract.maximum_size_bytes,
        retention_policy_id: RetentionPolicyId::try_new("standard").unwrap(),
        bytes: command.encode_to_vec(),
    };
    payload.validate().unwrap();
    QueryRequest {
        owner_module_id: ModuleId::try_new(SEARCH_MODULE_ID).unwrap(),
        context: QueryExecutionContext {
            tenant_id: TenantId::try_new(TENANT).unwrap(),
            actor_id: ActorId::try_new(ACTOR).unwrap(),
            request_id: crm_module_sdk::RequestId::try_new("request-search-1").unwrap(),
            correlation_id: crm_module_sdk::CorrelationId::try_new("correlation-search-1").unwrap(),
            trace_id: TraceId::try_new("trace-search-1").unwrap(),
            capability_id: definition.capability_id.clone(),
            capability_version: definition.capability_version.clone(),
            schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
            request_started_at_unix_nanos: 100,
        },
        input: payload,
        input_hash: [9; 32],
    }
}
