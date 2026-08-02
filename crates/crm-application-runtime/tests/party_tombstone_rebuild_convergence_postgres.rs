use crm_application_composition::ModuleActivationPort;
use crm_core_data::PostgresDataStore;
use crm_customer_360_composition::{
    CUSTOMER_360_CONTRIBUTION_RESOURCE_TYPE, CUSTOMER_360_PROJECTION_ID,
    Customer360ProjectionWorker,
};
use crm_customer_privacy_production::{
    ACTION_PLAN_RECORD_TYPE, ACTION_PLAN_STATE_MAXIMUM_BYTES,
    ACTION_PLAN_STATE_RETENTION_POLICY_ID, ACTION_PLAN_STATE_SCHEMA_ID,
    ACTION_PLAN_STATE_SCHEMA_VERSION, ActionPlanningPolicy, ContributionCompletenessProof,
    CustomerPrivacyProductionDependencies, DiscoveryOwnerScopeContribution, DiscoveryScopeSnapshot,
    EvidenceClass, OwnerExecutionInvocation, OwnerScopeContract, OwnerScopeContribution,
    OwnerScopeRegistry, PrivacyActionPlan, PrivacyCase, PrivacyCaseKind, PrivacyOwnerOutcomeStatus,
    PrivacyRetentionDecisionSet, ScopeDiscoveryLineage, ScopeResource, SubjectVerificationMethod,
    action_plan_state_descriptor_hash, build_canonical_internal_owner_execution,
    encode_action_plan_state, privacy_case_persisted_payload, retention_decision_persisted_payload,
};
use crm_global_search_composition::{GlobalSearchWorker, INITIAL_GLOBAL_SEARCH_GENERATION_ID};
use crm_module_sdk::{
    ActorId, CapabilityId, CapabilityVersion, CorrelationId, DataClass, ModuleId, PayloadEncoding,
    PortFuture, RecordId, RecordRef, RetentionPolicyId, SchemaId, SchemaVersion, SdkError,
    TenantId, TraceId, TypedPayload,
};
use crm_parties_capability_adapter::{RECORD_TYPE as PARTY_RECORD_TYPE, persisted_contract};
use crm_query_runtime::{QueryRequest, QueryVisibilityAuthorizer, QueryVisibilityDecision};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Postgres, Row, Transaction};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

const ORIGINAL_NAME: &str = "Ada Historical Projection";
const SEARCH_PROJECTION_ID: &str = "search.global.g3";
const OWNER_ACTION_COMMAND_DESCRIPTOR: &[u8] = b"crm.customer-privacy.owner_action.command/v1:tenant_id,privacy_case_id,action_plan_id,action_plan_digest,retention_decision_id,retention_decision_digest,attempt_id,attempt_digest,item_sequence,attempt_generation,item_digest,owner_module_id,owner_capability_id,owner_capability_version,target_idempotency_key,resource_type,resource_id,resource_version_decimal_string,action_code,planned_at_unix_nanos_decimal_string";
const CAPTURED_AT: i64 = 8_000_000;
const PLANNED_AT: i64 = 9_000_000;
const DECIDED_AT: i64 = 10_000_000;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn production_owner_execution_rebuilds_stale_party_derived_state() {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!("skipping Party rebuild convergence because DATABASE_URL is absent");
        return;
    };
    let admin_database_url = std::env::var("ADMIN_DATABASE_URL")
        .expect("ADMIN_DATABASE_URL must accompany DATABASE_URL");
    let admin = PgPool::connect(&admin_database_url)
        .await
        .expect("connect Party rebuild convergence admin pool");
    let run_id = unique_id();
    let tenant = format!("tenant-party-rebuild-{run_id}");
    let actor = format!("actor-party-rebuild-{run_id}");
    let case_id = format!("privacy-case-party-rebuild-{run_id}");
    let party_id = format!("party-rebuild-{run_id}");

    seed_tenant_and_actor(&admin, &tenant, &actor).await;
    seed_owner_action_capability(&admin).await;
    let (privacy_case, plan, decision) =
        build_case_plan_and_decision(&tenant, &actor, &case_id, &party_id);
    seed_record(
        &admin,
        &tenant,
        "customer-privacy.case",
        &case_id,
        privacy_case.version(),
        privacy_case_persisted_payload(&privacy_case).expect("encode privacy case fixture"),
        &format!("fixture-case-{run_id}"),
    )
    .await;
    seed_record(
        &admin,
        &tenant,
        ACTION_PLAN_RECORD_TYPE,
        plan.plan_id().as_str(),
        1,
        action_plan_payload(&plan),
        &format!("fixture-plan-{run_id}"),
    )
    .await;
    seed_record(
        &admin,
        &tenant,
        "customer-privacy.retention-decision",
        decision.decision_id().as_str(),
        1,
        retention_decision_persisted_payload(&decision).expect("encode retention decision fixture"),
        &format!("fixture-decision-{run_id}"),
    )
    .await;
    seed_record(
        &admin,
        &tenant,
        PARTY_RECORD_TYPE,
        &party_id,
        1,
        party_payload(&party_id),
        &format!("fixture-party-{run_id}"),
    )
    .await;

    let application = PgPool::connect(&database_url)
        .await
        .expect("connect Party rebuild convergence application pool");
    let store = PostgresDataStore::from_pool(application);
    let service =
        build_canonical_internal_owner_execution(&CustomerPrivacyProductionDependencies {
            store: store.clone(),
            activation: Arc::new(AlwaysActive),
            cursor_key: [0x71; 32],
            visibility_authorizer: Arc::new(DenyVisibility),
        })
        .expect("build canonical internal owner execution registry");
    let execution = service
        .execute_next(OwnerExecutionInvocation {
            tenant_id: TenantId::try_new(tenant.clone()).unwrap(),
            privacy_case_id: RecordId::try_new(case_id.clone()).unwrap(),
            action_plan_id: plan.plan_id().clone(),
            retention_decision_id: decision.decision_id().clone(),
            actor_id: ActorId::try_new(actor.clone()).unwrap(),
            request_id: crm_module_sdk::RequestId::try_new(format!(
                "party-rebuild-request-{run_id}"
            ))
            .unwrap(),
            correlation_id: CorrelationId::try_new(format!("party-rebuild-correlation-{run_id}"))
                .unwrap(),
            trace_id: TraceId::try_new(format!("party-rebuild-trace-{run_id}")).unwrap(),
            initiating_capability_id: CapabilityId::try_new("customer_privacy.case.approve")
                .unwrap(),
            initiating_capability_version: CapabilityVersion::try_new("1.0.0").unwrap(),
            request_started_at_unix_nanos: PLANNED_AT - 1,
            planned_at_unix_nanos: PLANNED_AT,
            trusted_internal: true,
        })
        .await
        .expect("execute real Party owner action through canonical registry");
    assert!(execution.owner_invoked);
    assert!(execution.complete);
    assert_eq!(
        execution.outcome.as_ref().map(|outcome| outcome.status()),
        Some(PrivacyOwnerOutcomeStatus::Succeeded)
    );
    assert_eq!(
        execution
            .attempt
            .as_ref()
            .map(|attempt| attempt.action_code()),
        Some("anonymize")
    );

    let action_event_id: String = sqlx::query_scalar(
        r#"
        SELECT event_id
        FROM crm.outbox_events
        WHERE tenant_id = $1
          AND event_type = 'parties.privacy.action.apply.completed'
          AND aggregate_type = $2
          AND aggregate_id = $3
        ORDER BY occurred_at DESC, event_id DESC
        LIMIT 1
        "#,
    )
    .bind(&tenant)
    .bind(PARTY_RECORD_TYPE)
    .bind(&party_id)
    .fetch_one(&admin)
    .await
    .expect("production owner action emits immutable convergence event");

    seed_stale_customer_360(&admin, &tenant, &party_id, &action_event_id).await;
    assert_customer_360_stale(&admin, &tenant, &party_id).await;

    let tenant_id = TenantId::try_new(tenant.clone()).unwrap();
    let rebuilt = Customer360ProjectionWorker::new(store.clone())
        .expect("construct Customer 360 rebuild worker")
        .rebuild(tenant_id.clone(), 200)
        .await
        .expect("rebuild Customer 360 from immutable history");
    assert!(rebuilt >= 1);
    GlobalSearchWorker::new(store.clone())
        .expect("construct global search worker")
        .reindex(tenant_id.clone(), 200)
        .await
        .expect("reindex global search from immutable history");

    assert_authoritative_party_minimized(&admin, &tenant, &party_id).await;
    assert_customer_360_tombstone(&admin, &tenant, &party_id).await;
    assert_search_tombstone(&admin, &tenant, &party_id).await;

    let authoritative_before_repeat = authoritative_counts(&admin, &tenant).await;
    Customer360ProjectionWorker::new(store.clone())
        .expect("construct repeat Customer 360 worker")
        .rebuild(tenant_id.clone(), 200)
        .await
        .expect("repeat Customer 360 rebuild");
    GlobalSearchWorker::new(store)
        .expect("construct repeat global search worker")
        .reindex(tenant_id, 200)
        .await
        .expect("repeat global search reindex");
    assert_eq!(
        authoritative_counts(&admin, &tenant).await,
        authoritative_before_repeat,
        "derived-state rebuilds must not mutate authoritative Party, outbox or audit evidence"
    );
    assert_customer_360_tombstone(&admin, &tenant, &party_id).await;
    assert_search_tombstone(&admin, &tenant, &party_id).await;
}

#[derive(Debug)]
struct AlwaysActive;

impl ModuleActivationPort for AlwaysActive {
    fn is_active<'a>(
        &'a self,
        _tenant_id: &'a TenantId,
        _module_id: &'a ModuleId,
    ) -> PortFuture<'a, Result<bool, SdkError>> {
        Box::pin(async { Ok(true) })
    }
}

#[derive(Debug)]
struct DenyVisibility;

impl QueryVisibilityAuthorizer for DenyVisibility {
    fn authorize_visibility<'a>(
        &'a self,
        _request: &'a QueryRequest,
        _resource: &'a RecordRef,
    ) -> PortFuture<'a, Result<QueryVisibilityDecision, SdkError>> {
        Box::pin(async {
            Ok(QueryVisibilityDecision::denied(
                "party-rebuild-test",
                "not-used",
            ))
        })
    }
}

fn build_case_plan_and_decision(
    tenant: &str,
    actor: &str,
    case_id: &str,
    party_id: &str,
) -> (PrivacyCase, PrivacyActionPlan, PrivacyRetentionDecisionSet) {
    let tenant_id = TenantId::try_new(tenant).unwrap();
    let canonical_party_id = RecordId::try_new(party_id).unwrap();
    let privacy_case_id = RecordId::try_new(case_id).unwrap();
    let contract = OwnerScopeContract::new(
        ModuleId::try_new("crm.parties").unwrap(),
        CapabilityId::try_new("parties.privacy.scope.contribute").unwrap(),
        CapabilityVersion::try_new("1.0.0").unwrap(),
    );
    let registry = OwnerScopeRegistry::new(
        SchemaVersion::try_new("party-rebuild-registry/1").unwrap(),
        [contract.clone()],
    )
    .unwrap();
    let lineage = ScopeDiscoveryLineage::new(
        privacy_case_id.clone(),
        tenant_id.clone(),
        canonical_party_id.clone(),
        1,
        registry.registry_version().clone(),
        *registry.digest(),
        "ERASURE_DISCOVERY",
        CAPTURED_AT / 1_000_000,
    )
    .unwrap();
    let contribution = OwnerScopeContribution::new(
        contract,
        tenant_id.clone(),
        canonical_party_id.clone(),
        1,
        [ScopeResource::new(
            PARTY_RECORD_TYPE.to_owned(),
            canonical_party_id.clone(),
            1,
            DataClass::Personal,
            EvidenceClass::RetainMinimizedEvidence,
            RetentionPolicyId::try_new("crm.parties.business_record").unwrap(),
        )
        .unwrap()],
        ContributionCompletenessProof::new(true, 1, 1, 1, [0x72; 32]).unwrap(),
    )
    .unwrap();
    let discovery = DiscoveryOwnerScopeContribution::new(lineage.clone(), contribution).unwrap();
    let snapshot =
        DiscoveryScopeSnapshot::finalize(lineage, registry, CAPTURED_AT, [discovery]).unwrap();

    let mut privacy_case = PrivacyCase::new(
        privacy_case_id,
        tenant_id,
        PrivacyCaseKind::Erasure,
        SchemaVersion::try_new("privacy-policy/1").unwrap(),
        500_000,
        None,
    )
    .unwrap();
    privacy_case.submit(1, 600_000).unwrap();
    privacy_case
        .verify_subject(
            2,
            RecordId::try_new(format!("submitted-{party_id}")).unwrap(),
            canonical_party_id,
            1,
            SubjectVerificationMethod::AuthenticatedPortal,
            ActorId::try_new(actor).unwrap(),
            700_000,
        )
        .unwrap();
    privacy_case.begin_scoping(3, 800_000).unwrap();
    privacy_case
        .record_scope(4, snapshot.snapshot_id().clone(), CAPTURED_AT)
        .unwrap();
    let plan = PrivacyActionPlan::build(
        &snapshot,
        privacy_case.version(),
        PrivacyCaseKind::Erasure,
        ActionPlanningPolicy::new(
            SchemaVersion::try_new("privacy-policy/1").unwrap(),
            "EU",
            false,
            false,
        )
        .unwrap(),
        PLANNED_AT,
    )
    .unwrap();
    privacy_case
        .record_plan(5, plan.plan_id().clone(), false, PLANNED_AT)
        .unwrap();
    let decision = PrivacyRetentionDecisionSet::build(&plan, &[], DECIDED_AT).unwrap();
    (privacy_case, plan, decision)
}

fn action_plan_payload(plan: &PrivacyActionPlan) -> TypedPayload {
    TypedPayload {
        owner: ModuleId::try_new("crm.customer-privacy").unwrap(),
        schema_id: SchemaId::try_new(ACTION_PLAN_STATE_SCHEMA_ID).unwrap(),
        schema_version: SchemaVersion::try_new(ACTION_PLAN_STATE_SCHEMA_VERSION).unwrap(),
        descriptor_hash: action_plan_state_descriptor_hash(),
        data_class: DataClass::Confidential,
        encoding: PayloadEncoding::Json,
        maximum_size_bytes: ACTION_PLAN_STATE_MAXIMUM_BYTES,
        retention_policy_id: RetentionPolicyId::try_new(ACTION_PLAN_STATE_RETENTION_POLICY_ID)
            .unwrap(),
        bytes: encode_action_plan_state(plan).unwrap(),
    }
}

fn party_payload(party_id: &str) -> TypedPayload {
    let contract = persisted_contract();
    TypedPayload {
        owner: ModuleId::try_new(contract.owner).unwrap(),
        schema_id: SchemaId::try_new(contract.schema_id).unwrap(),
        schema_version: SchemaVersion::try_new(contract.schema_version).unwrap(),
        descriptor_hash: contract.descriptor_hash,
        data_class: DataClass::Personal,
        encoding: PayloadEncoding::Json,
        maximum_size_bytes: contract.maximum_size_bytes,
        retention_policy_id: RetentionPolicyId::try_new(contract.retention_policy_id).unwrap(),
        bytes: serde_json::to_vec(&json!({
            "schema_version": "crm.parties.state/v1",
            "party_id": party_id,
            "kind": "person",
            "display_name": ORIGINAL_NAME,
            "version": 1
        }))
        .unwrap(),
    }
}

async fn seed_tenant_and_actor(admin: &PgPool, tenant: &str, actor: &str) {
    sqlx::query(
        "INSERT INTO crm.tenants (tenant_id, status, data_region) VALUES ($1, 'active', 'eu-central') ON CONFLICT (tenant_id) DO NOTHING",
    )
    .bind(tenant)
    .execute(admin)
    .await
    .unwrap();
    let mut transaction = admin.begin().await.unwrap();
    sqlx::query("SET LOCAL session_replication_role = replica")
        .execute(&mut *transaction)
        .await
        .unwrap();
    sqlx::query(
        r#"
        INSERT INTO crm.actors (
          tenant_id, actor_id, actor_type, status, display_name,
          version, last_business_transaction_id
        ) VALUES ($1, $2, 'service', 'active', 'Party Rebuild Convergence', 1, 'fixture-actor')
        ON CONFLICT (tenant_id, actor_id) DO NOTHING
        "#,
    )
    .bind(tenant)
    .bind(actor)
    .execute(&mut *transaction)
    .await
    .unwrap();
    transaction.commit().await.unwrap();
}

async fn seed_owner_action_capability(admin: &PgPool) {
    let descriptor_hash: [u8; 32] = Sha256::digest(OWNER_ACTION_COMMAND_DESCRIPTOR).into();
    sqlx::query(
        r#"
        INSERT INTO crm.capability_registry (
          capability_id, capability_version, owner_module_id, owner_module_version,
          service_name, method_name, input_descriptor_hash, output_descriptor_hash,
          risk_level, idempotency_required, audit_required, approval_required,
          ai_callable, marketplace_callable, bulk_allowed, export_allowed,
          data_classes_touched
        ) VALUES ('parties.privacy.action.apply', '1.0.0', 'crm.parties', '0.3.0',
                  'crm.parties.internal.PrivacyOwnerAction', 'Apply', $1, $1,
                  'critical', true, true, false, false, false, false, false,
                  ARRAY['personal', 'restricted']::text[])
        ON CONFLICT (capability_id, capability_version) DO NOTHING
        "#,
    )
    .bind(descriptor_hash.as_slice())
    .execute(admin)
    .await
    .unwrap();
}

async fn seed_record(
    admin: &PgPool,
    tenant: &str,
    record_type: &str,
    record_id: &str,
    version: u64,
    payload: TypedPayload,
    transaction_id: &str,
) {
    let mut transaction = admin.begin().await.unwrap();
    sqlx::query("SET LOCAL session_replication_role = replica")
        .execute(&mut *transaction)
        .await
        .unwrap();
    insert_fixture_transaction(&mut transaction, tenant, transaction_id).await;
    sqlx::query(
        r#"
        INSERT INTO crm.records (
          tenant_id, record_type, record_id, version, owner_module_id,
          schema_id, schema_version, descriptor_hash, data_class,
          payload_encoding, maximum_payload_size, retention_policy_id,
          payload_bytes, last_business_transaction_id
        ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,'json',$10,$11,$12,$13)
        "#,
    )
    .bind(tenant)
    .bind(record_type)
    .bind(record_id)
    .bind(i64::try_from(version).unwrap())
    .bind(payload.owner.as_str())
    .bind(payload.schema_id.as_str())
    .bind(payload.schema_version.as_str())
    .bind(payload.descriptor_hash.as_slice())
    .bind(data_class_name(payload.data_class))
    .bind(i64::try_from(payload.maximum_size_bytes).unwrap())
    .bind(payload.retention_policy_id.as_str())
    .bind(payload.bytes)
    .bind(transaction_id)
    .execute(&mut *transaction)
    .await
    .unwrap();
    transaction.commit().await.unwrap();
}

async fn insert_fixture_transaction(
    transaction: &mut Transaction<'_, Postgres>,
    tenant: &str,
    transaction_id: &str,
) {
    sqlx::query(
        r#"
        INSERT INTO crm.business_transactions (
          tenant_id, business_transaction_id, actor_id, request_id,
          correlation_id, trace_id, capability_id, capability_version,
          expected_outbox_events, expected_audit_records, expected_idempotency_records
        ) VALUES ($1,$2,'fixture',$3,$4,$5,'test.record.mutate','1.0.0',1,1,1)
        "#,
    )
    .bind(tenant)
    .bind(transaction_id)
    .bind(format!("request-{transaction_id}"))
    .bind(format!("correlation-{transaction_id}"))
    .bind(format!("trace-{transaction_id}"))
    .execute(&mut **transaction)
    .await
    .unwrap();
}

async fn seed_stale_customer_360(
    admin: &PgPool,
    tenant: &str,
    party_id: &str,
    source_event_id: &str,
) {
    sqlx::query(
        r#"
        INSERT INTO crm.projection_documents (
          tenant_id, projection_id, resource_type, resource_id,
          source_version, source_event_id, document
        ) VALUES (
          $1, $2, $3, 'party:' || $4, 1, $5,
          jsonb_build_object(
            'projection_schema_version', '1',
            'contribution_kind', 'party',
            'root_party_ids', jsonb_build_array($4::text),
            'source_owner_module_id', 'crm.parties',
            'source_resource_type', $6::text,
            'source_resource_id', $4::text,
            'source_version', 1,
            'source_event_id', $5::text,
            'snapshot', jsonb_build_object(
              'snapshot_kind', 'party',
              'kind', 'person',
              'display_name', $7::text,
              'privacy_lifecycle', 'active'
            )
          )
        )
        ON CONFLICT (tenant_id, projection_id, resource_type, resource_id) DO UPDATE
        SET source_version = EXCLUDED.source_version,
            source_event_id = EXCLUDED.source_event_id,
            document = EXCLUDED.document
        "#,
    )
    .bind(tenant)
    .bind(CUSTOMER_360_PROJECTION_ID)
    .bind(CUSTOMER_360_CONTRIBUTION_RESOURCE_TYPE)
    .bind(party_id)
    .bind(source_event_id)
    .bind(PARTY_RECORD_TYPE)
    .bind(ORIGINAL_NAME)
    .execute(admin)
    .await
    .unwrap();
}

async fn assert_customer_360_stale(admin: &PgPool, tenant: &str, party_id: &str) {
    let row = sqlx::query(
        r#"
        SELECT source_version,
               document #>> '{snapshot,display_name}' AS display_name,
               document -> 'root_party_ids' @> jsonb_build_array($4::text) AS has_root
        FROM crm.projection_documents
        WHERE tenant_id = $1 AND projection_id = $2
          AND resource_type = $3 AND resource_id = 'party:' || $4
        "#,
    )
    .bind(tenant)
    .bind(CUSTOMER_360_PROJECTION_ID)
    .bind(CUSTOMER_360_CONTRIBUTION_RESOURCE_TYPE)
    .bind(party_id)
    .fetch_one(admin)
    .await
    .unwrap();
    assert_eq!(row.try_get::<i64, _>("source_version").unwrap(), 1);
    assert_eq!(
        row.try_get::<String, _>("display_name").unwrap(),
        ORIGINAL_NAME
    );
    assert!(row.try_get::<bool, _>("has_root").unwrap());
}

async fn assert_authoritative_party_minimized(admin: &PgPool, tenant: &str, party_id: &str) {
    let row = sqlx::query(
        "SELECT version, payload_bytes, deleted_at IS NULL AS retained_tombstone FROM crm.records WHERE tenant_id = $1 AND record_type = $2 AND record_id = $3",
    )
    .bind(tenant)
    .bind(PARTY_RECORD_TYPE)
    .bind(party_id)
    .fetch_one(admin)
    .await
    .unwrap();
    let state: Value =
        serde_json::from_slice(&row.try_get::<Vec<u8>, _>("payload_bytes").unwrap()).unwrap();
    assert_eq!(row.try_get::<i64, _>("version").unwrap(), 2);
    assert_eq!(state["version"], 2);
    let display_name = state["display_name"].as_str().unwrap();
    assert!(display_name.starts_with("minimized person "));
    assert!(!display_name.contains(ORIGINAL_NAME));
    assert!(row.try_get::<bool, _>("retained_tombstone").unwrap());
}

async fn assert_customer_360_tombstone(admin: &PgPool, tenant: &str, party_id: &str) {
    let row = sqlx::query(
        r#"
        SELECT source_version,
               document -> 'root_party_ids' = '[]'::jsonb AS roots_removed,
               document #>> '{snapshot,kind}' AS kind,
               document #>> '{snapshot,display_name}' AS display_name,
               document #>> '{snapshot,privacy_lifecycle}' AS lifecycle,
               document::text LIKE '%' || $5 || '%' AS leaks_original
        FROM crm.projection_documents
        WHERE tenant_id = $1 AND projection_id = $2
          AND resource_type = $3 AND resource_id = 'party:' || $4
        "#,
    )
    .bind(tenant)
    .bind(CUSTOMER_360_PROJECTION_ID)
    .bind(CUSTOMER_360_CONTRIBUTION_RESOURCE_TYPE)
    .bind(party_id)
    .bind(ORIGINAL_NAME)
    .fetch_one(admin)
    .await
    .unwrap();
    assert_eq!(row.try_get::<i64, _>("source_version").unwrap(), 2);
    assert!(row.try_get::<bool, _>("roots_removed").unwrap());
    assert_eq!(row.try_get::<String, _>("kind").unwrap(), "suppressed");
    assert_eq!(
        row.try_get::<String, _>("display_name").unwrap(),
        "suppressed"
    );
    assert_eq!(
        row.try_get::<String, _>("lifecycle").unwrap(),
        "privacy_minimized"
    );
    assert!(!row.try_get::<bool, _>("leaks_original").unwrap());

    let selectable: i64 = sqlx::query_scalar(
        r#"
        SELECT count(*) FROM crm.projection_documents
        WHERE tenant_id = $1 AND projection_id = $2 AND resource_type = $3
          AND (document -> 'root_party_ids') @> jsonb_build_array($4::text)
        "#,
    )
    .bind(tenant)
    .bind(CUSTOMER_360_PROJECTION_ID)
    .bind(CUSTOMER_360_CONTRIBUTION_RESOURCE_TYPE)
    .bind(party_id)
    .fetch_one(admin)
    .await
    .unwrap();
    assert_eq!(selectable, 0);
}

async fn assert_search_tombstone(admin: &PgPool, tenant: &str, party_id: &str) {
    assert_eq!(INITIAL_GLOBAL_SEARCH_GENERATION_ID, "g3");
    let row = sqlx::query(
        r#"
        SELECT source_version,
               document #>> '{display_fields,privacy_lifecycle}' AS lifecycle,
               document::text LIKE '%' || $5 || '%' AS leaks_original
        FROM crm.projection_documents
        WHERE tenant_id = $1 AND projection_id = $2
          AND resource_type = $3 AND resource_id = $4
        "#,
    )
    .bind(tenant)
    .bind(SEARCH_PROJECTION_ID)
    .bind(PARTY_RECORD_TYPE)
    .bind(party_id)
    .bind(ORIGINAL_NAME)
    .fetch_one(admin)
    .await
    .unwrap();
    assert_eq!(row.try_get::<i64, _>("source_version").unwrap(), 2);
    assert_eq!(
        row.try_get::<String, _>("lifecycle").unwrap(),
        "privacy_minimized"
    );
    assert!(!row.try_get::<bool, _>("leaks_original").unwrap());
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct AuthoritativeCounts {
    records: i64,
    outbox_events: i64,
    audit_records: i64,
}

async fn authoritative_counts(admin: &PgPool, tenant: &str) -> AuthoritativeCounts {
    AuthoritativeCounts {
        records: sqlx::query_scalar("SELECT count(*) FROM crm.records WHERE tenant_id = $1")
            .bind(tenant)
            .fetch_one(admin)
            .await
            .unwrap(),
        outbox_events: sqlx::query_scalar(
            "SELECT count(*) FROM crm.outbox_events WHERE tenant_id = $1",
        )
        .bind(tenant)
        .fetch_one(admin)
        .await
        .unwrap(),
        audit_records: sqlx::query_scalar(
            "SELECT count(*) FROM crm.audit_records WHERE tenant_id = $1",
        )
        .bind(tenant)
        .fetch_one(admin)
        .await
        .unwrap(),
    }
}

fn data_class_name(data_class: DataClass) -> &'static str {
    match data_class {
        DataClass::Confidential => "confidential",
        DataClass::Personal => "personal",
        DataClass::Restricted => "restricted",
        other => panic!("unsupported fixture data class: {other:?}"),
    }
}

fn unique_id() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock after unix epoch")
        .as_nanos()
}
