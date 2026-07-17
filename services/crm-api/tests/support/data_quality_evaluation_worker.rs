use super::data_quality_evaluation_fixture::{
    INTERNAL_MATERIALIZE, INTERNAL_STAGE, TENANT, WORKER_ACTOR,
};
use crm_application_runtime::SystemClock;
use crm_capability_adapters::{
    AuthorizationGrant, LiveAuthorizationStore, LiveCapabilityAuthorizer,
    LiveQueryVisibilityAuthorizer, LiveQueryVisibilityStore, QueryVisibilityGrant,
};
use crm_capability_runtime::CapabilityAuthorizer;
use crm_core_data::PostgresDataStore;
use crm_data_quality_source_composition::{
    GovernedPartyQualitySource, PartyEvaluationStageWorker, PartyQualitySource,
    PostgresPartyEvaluationMaterializationSink, PostgresPartyEvaluationStageSink,
};
use crm_module_sdk::{
    ActorId, CapabilityId, CapabilityVersion, Clock, ModuleId, RecordType, TenantId,
};
use crm_parties_query_adapter::{
    GET_CAPABILITY as PARTY_GET_CAPABILITY, PartyQueryAdapter, query_capability_definition,
};
use crm_query_runtime::{CursorCodec, QueryAuthorizer};
use std::collections::BTreeSet;
use std::sync::Arc;

pub struct EvaluationWorkerRuntime {
    #[allow(dead_code)]
    pub store: PostgresDataStore,
    pub worker: PartyEvaluationStageWorker,
}

pub async fn build_evaluation_worker(database_url: &str) -> EvaluationWorkerRuntime {
    let store = PostgresDataStore::connect(database_url, 6)
        .await
        .expect("connect evaluation worker store");
    let clock: Arc<dyn Clock> = Arc::new(SystemClock);
    let now = clock.now_unix_nanos();
    let expires_at = now
        .checked_add(60_000_000_000)
        .expect("bounded evaluation worker grant expiry");
    let tenant_id = TenantId::try_new(TENANT).unwrap();
    let actor_id = ActorId::try_new(WORKER_ACTOR).unwrap();
    let party_definition =
        query_capability_definition(PARTY_GET_CAPABILITY).expect("valid Party GET definition");

    let authorization_store = LiveAuthorizationStore::default();
    authorization_store
        .upsert(AuthorizationGrant {
            tenant_id: tenant_id.clone(),
            actor_id: actor_id.clone(),
            policy_id: party_definition.authorization_policy_id.clone(),
            capability_id: party_definition.capability_id.clone(),
            capability_version: party_definition.capability_version.clone(),
            owner_module_id: party_definition.owner_module_id.clone(),
            policy_version: "data-quality-evaluation-party-source/v1".to_owned(),
            expires_at_unix_nanos: Some(expires_at),
        })
        .expect("grant worker Party GET authorization");
    for (capability_id, policy_version) in [
        (INTERNAL_STAGE, "data-quality-evaluation-stage/v1"),
        (
            INTERNAL_MATERIALIZE,
            "data-quality-evaluation-materialization/v1",
        ),
    ] {
        authorization_store
            .upsert(AuthorizationGrant {
                tenant_id: tenant_id.clone(),
                actor_id: actor_id.clone(),
                policy_id: capability_id.to_owned(),
                capability_id: CapabilityId::try_new(capability_id).unwrap(),
                capability_version: CapabilityVersion::try_new("1.0.0").unwrap(),
                owner_module_id: ModuleId::try_new("crm.data-quality").unwrap(),
                policy_version: policy_version.to_owned(),
                expires_at_unix_nanos: Some(expires_at),
            })
            .expect("grant worker internal evaluation authorization");
    }
    let live_authorizer = Arc::new(LiveCapabilityAuthorizer::new(
        authorization_store,
        Arc::clone(&clock),
    ));
    let query_authorizer: Arc<dyn QueryAuthorizer> = live_authorizer.clone();
    let capability_authorizer: Arc<dyn CapabilityAuthorizer> = live_authorizer;

    let visibility_store = LiveQueryVisibilityStore::default();
    visibility_store
        .upsert(QueryVisibilityGrant {
            tenant_id,
            actor_id,
            capability_id: party_definition.capability_id,
            capability_version: party_definition.capability_version,
            owner_module_id: ModuleId::try_new("crm.parties").unwrap(),
            record_type: RecordType::try_new("parties.party").unwrap(),
            record_id: None,
            allowed_fields: BTreeSet::from(["kind".to_owned(), "display_name".to_owned()]),
            policy_version: "data-quality-evaluation-visibility/v1".to_owned(),
            expires_at_unix_nanos: Some(expires_at),
        })
        .expect("grant worker Party field visibility");
    let visibility = Arc::new(LiveQueryVisibilityAuthorizer::new(
        visibility_store,
        Arc::clone(&clock),
    ));
    let party_adapter = Arc::new(
        PartyQueryAdapter::new(
            store.clone(),
            CursorCodec::new([13; 32]).expect("valid evaluation cursor key"),
            visibility,
        )
        .expect("construct governed Party query adapter"),
    );
    let source: Arc<dyn PartyQualitySource> = Arc::new(GovernedPartyQualitySource::new(
        party_adapter,
        query_authorizer,
    ));
    let stage_sink = Arc::new(PostgresPartyEvaluationStageSink::new(
        store.clone(),
        Arc::clone(&capability_authorizer),
    ));
    let materialization_sink = Arc::new(PostgresPartyEvaluationMaterializationSink::new(
        store.clone(),
        capability_authorizer,
    ));
    let worker = PartyEvaluationStageWorker::new_with_materialization(
        store.clone(),
        source,
        stage_sink,
        materialization_sink,
        clock,
    )
    .expect("construct evaluation worker");
    EvaluationWorkerRuntime { store, worker }
}
