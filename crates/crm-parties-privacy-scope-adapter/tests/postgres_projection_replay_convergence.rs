use crm_capability_runtime::{CapabilityRequest, TransactionalCapabilityExecutor};
use crm_core_data::{PostgresDataStore, PostgresPrivacyOwnerActionExecutor};
use crm_customer_360_composition::{
    CUSTOMER_360_CONTRIBUTION_RESOURCE_TYPE, CUSTOMER_360_PROJECTION_ID,
    Customer360ProjectionWorker,
};
use crm_customer_privacy::{
    ActionPlanningPolicy, ContributionCompletenessProof, DiscoveryOwnerScopeContribution,
    DiscoveryScopeSnapshot, EvidenceClass, OwnerScopeContract, OwnerScopeContribution,
    OwnerScopeRegistry, PrivacyActionPlan, PrivacyCaseKind, PrivacyOwnerActionAttempt,
    PrivacyOwnerActionCommand, PrivacyRetentionDecisionSet, ScopeDiscoveryLineage, ScopeResource,
    discovery_sha256, encode_owner_action_command,
};
use crm_customer_privacy_owner_scope_support::owner_action_input_payload;
use crm_global_search_composition::{
    GlobalSearchWorker, INITIAL_GLOBAL_SEARCH_GENERATION_ID,
};
use crm_module_sdk::{
    ActorId, BusinessTransactionId, CapabilityId, CapabilityVersion, CausationId, CorrelationId,
    DataClass, ExecutionContext, ModuleExecutionContext, ModuleId, RecordId, RequestId,
    RetentionPolicyId, SchemaVersion, TenantId, TraceId,
};
use crm_parties::{CreateParty, Party, PartyId, PartyKind, decode_party_state};
use crm_parties_capability_adapter::{RECORD_TYPE, persisted_contract, persisted_payload};
use crm_parties_privacy_scope_adapter::{
    CAPABILITY_ID as SCOPE_CAPABILITY_ID, CAPABILITY_VERSION as SCOPE_CAPABILITY_VERSION,
    OWNER_ACTION_CAPABILITY_ID, parties_privacy_action_definition, parties_privacy_action_planner,
};
use sqlx::{PgPool, Row};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

const ACTOR_ID: &str = "privacy-replay-convergence";
const ORIGINAL_NAME: &str = "Ada Historical Projection";
const LEGACY_CUSTOMER_360_PROJECTION_ID: &str = "customer.customer-360.v1";
const SEARCH_PROJECTION_ID: &str = "search.global.g3";
const BASE_TIME_NANOS: i64 = 1_800_000_000_000_000_000;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn party_owner_action_replays_into_fresh_customer_360_and_search_generations() {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!("skipping Party replay convergence because DATABASE_URL is absent");
        return;
    };
    let admin_database_url = std::env::var("ADMIN_DATABASE_URL")
        .expect("ADMIN_DATABASE_URL must accompany DATABASE_URL");
    let admin = PgPool::connect(&admin_database_url)
        .await
        .expect("connect replay convergence admin pool");
    let run_id = unique_id();
    let tenant = format!("tenant-party-replay-{run_id}");
    let party_id = format!("party-replay-{run_id}");

    seed_tenant(&admin, &tenant).await;
    seed_actor(&admin, &tenant).await;
    seed_owner_action_capability(&admin).await;
    seed_party(&admin, &tenant, &party_id).await;

    let application = PgPool::connect(&database_url)
        .await
        .expect("connect replay convergence application pool");
    let store = PostgresDataStore::from_pool(application);
    let definition = parties_privacy_action_definition().expect("owner action definition");
    let executor = PostgresPrivacyOwnerActionExecutor::new(
        store.clone(),
        Arc::new(parties_privacy_action_planner()),
    );
    let attempt = attempt(&tenant, &party_id, 1).expect("build canonical privacy attempt");
    let request = capability_request(&definition, &attempt);
    let result = executor
        .execute(&definition, request)
        .await
        .expect("apply authoritative Party anonymization");
    assert!(!result.replayed);
    assert_eq!(result.affected_resources[0].version, Some(2));

    let action_event = sqlx::query(
        r#"
        SELECT event_id, occurred_at
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
    .bind(RECORD_TYPE)
    .bind(&party_id)
    .fetch_one(&admin)
    .await
    .expect("owner action emits immutable convergence event");
    let action_event_id: String = action_event.try_get("event_id").unwrap();
    let action_occurred_at: chrono::DateTime<chrono::Utc> =
        action_event.try_get("occurred_at").unwrap();

    seed_stale_legacy_customer_360(
        &admin,
        &tenant,
        &party_id,
        &action_event_id,
        action_occurred_at,
    )
    .await;

    assert_eq!(CUSTOMER_360_PROJECTION_ID, "customer.customer-360.v2");
    let tenant_id = TenantId::try_new(tenant.clone()).unwrap();
    drain_customer_360(
        &Customer360ProjectionWorker::new(store.clone()).expect("construct Customer 360 worker"),
        tenant_id.clone(),
    )
    .await;
    GlobalSearchWorker::new(store.clone())
        .expect("construct global search worker")
        .ensure_ready(tenant_id.clone(), 200)
        .await
        .expect("build fresh global-search generation");

    assert_authoritative_party_minimized(&admin, &tenant, &party_id).await;
    assert_legacy_customer_360_is_stale(&admin, &tenant, &party_id).await;
    assert_customer_360_v2_tombstone(&admin, &tenant, &party_id).await;
    assert_search_g3_tombstone(&admin, &tenant, &party_id).await;

    let authoritative_before_rebuild = authoritative_counts(&admin, &tenant).await;
    let rebuilt = Customer360ProjectionWorker::new(store.clone())
        .expect("construct Customer 360 rebuild worker")
        .rebuild(tenant_id.clone(), 200)
        .await
        .expect("rebuild Customer 360 v2 from immutable history");
    assert!(rebuilt >= 1);
    GlobalSearchWorker::new(store)
        .expect("construct global search reindex worker")
        .reindex(tenant_id, 200)
        .await
        .expect("reindex global search from immutable history");

    assert_eq!(
        authoritative_counts(&admin, &tenant).await,
        authoritative_before_rebuild,
        "derived-state rebuilds must not mutate authoritative Party, outbox or audit evidence"
    );
    assert_customer_360_v2_tombstone(&admin, &tenant, &party_id).await;
    assert_search_g3_tombstone(&admin, &tenant, &party_id).await;
}

async fn drain_customer_360(worker: &Customer360ProjectionWorker, tenant_id: TenantId) {
    loop {
        let batch = worker
            .run_batch(tenant_id.clone(), 200)
            .await
            .expect("catch up Customer 360 v2");
        if !batch.has_more {
            break;
        }
    }
}

async fn assert_authoritative_party_minimized(admin: &PgPool, tenant: &str, party_id: &str) {
    let row = sqlx::query(
        r#"
        SELECT version, payload_bytes, deleted_at IS NULL AS retained_tombstone
        FROM crm.records
        WHERE tenant_id = $1 AND record_type = $2 AND record_id = $3
        "#,
    )
    .bind(tenant)
    .bind(RECORD_TYPE)
    .bind(party_id)
    .fetch_one(admin)
    .await
    .expect("read authoritative Party tombstone");
    let version: i64 = row.try_get("version").unwrap();
    let payload: Vec<u8> = row.try_get("payload_bytes").unwrap();
    let retained_tombstone: bool = row.try_get("retained_tombstone").unwrap();
    let party = decode_party_state(&payload).expect("decode minimized Party state");
    assert_eq!(version, 2);
    assert_eq!(party.version(), 2);
    assert!(party.display_name().starts_with("minimized person "));
    assert!(!party.display_name().contains(ORIGINAL_NAME));
    assert!(retained_tombstone);
}

async fn assert_legacy_customer_360_is_stale(admin: &PgPool, tenant: &str, party_id: &str) {
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
    .bind(LEGACY_CUSTOMER_360_PROJECTION_ID)
    .bind(CUSTOMER_360_CONTRIBUTION_RESOURCE_TYPE)
    .bind(party_id)
    .fetch_one(admin)
    .await
    .expect("legacy v1 stale document remains isolated");
    assert_eq!(row.try_get::<i64, _>("source_version").unwrap(), 1);
    assert_eq!(
        row.try_get::<String, _>("display_name").unwrap(),
        ORIGINAL_NAME
    );
    assert!(row.try_get::<bool, _>("has_root").unwrap());
}

async fn assert_customer_360_v2_tombstone(admin: &PgPool, tenant: &str, party_id: &str) {
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
    .expect("read Customer 360 v2 Party tombstone");
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
        SELECT count(*)
        FROM crm.projection_documents
        WHERE tenant_id = $1 AND projection_id = $2
          AND resource_type = $3
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
    assert_eq!(selectable, 0, "privacy tombstone must not be a Customer 360 root");
}

async fn assert_search_g3_tombstone(admin: &PgPool, tenant: &str, party_id: &str) {
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
    .bind(RECORD_TYPE)
    .bind(party_id)
    .bind(ORIGINAL_NAME)
    .fetch_one(admin)
    .await
    .expect("read search g3 Party tombstone");
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

async fn seed_stale_legacy_customer_360(
    admin: &PgPool,
    tenant: &str,
    party_id: &str,
    source_event_id: &str,
    occurred_at: chrono::DateTime<chrono::Utc>,
) {
    sqlx::query(
        r#"
        INSERT INTO crm.projection_checkpoints (
          tenant_id, projection_id, last_occurred_at, last_event_id,
          applied_event_count, status
        ) VALUES ($1, $2, $3, $4, 1, 'active')
        ON CONFLICT (tenant_id, projection_id) DO UPDATE
        SET last_occurred_at = EXCLUDED.last_occurred_at,
            last_event_id = EXCLUDED.last_event_id,
            applied_event_count = EXCLUDED.applied_event_count,
            status = EXCLUDED.status
        "#,
    )
    .bind(tenant)
    .bind(LEGACY_CUSTOMER_360_PROJECTION_ID)
    .bind(occurred_at)
    .bind(source_event_id)
    .execute(admin)
    .await
    .unwrap();
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
    .bind(LEGACY_CUSTOMER_360_PROJECTION_ID)
    .bind(CUSTOMER_360_CONTRIBUTION_RESOURCE_TYPE)
    .bind(party_id)
    .bind(source_event_id)
    .bind(RECORD_TYPE)
    .bind(ORIGINAL_NAME)
    .execute(admin)
    .await
    .unwrap();
}

fn attempt(tenant: &str, party_id: &str, resource_version: u64) -> Result<PrivacyOwnerActionAttempt, String> {
    let tenant_id = TenantId::try_new(tenant).unwrap();
    let privacy_case_id = RecordId::try_new(format!("privacy-case-{party_id}")).unwrap();
    let party_record_id = RecordId::try_new(party_id).unwrap();
    let contract = OwnerScopeContract::new(
        ModuleId::try_new("crm.parties").unwrap(),
        CapabilityId::try_new(SCOPE_CAPABILITY_ID).unwrap(),
        CapabilityVersion::try_new(SCOPE_CAPABILITY_VERSION).unwrap(),
    );
    let registry = OwnerScopeRegistry::new(
        SchemaVersion::try_new("replay-convergence-registry/1").unwrap(),
        [contract.clone()],
    )
    .unwrap();
    let lineage = ScopeDiscoveryLineage::new(
        privacy_case_id.clone(),
        tenant_id.clone(),
        party_record_id.clone(),
        1,
        registry.registry_version().clone(),
        *registry.digest(),
        "ERASURE_DISCOVERY".to_owned(),
        BASE_TIME_NANOS / 1_000_000,
    )
    .unwrap();
    let resource = ScopeResource::new(
        RECORD_TYPE.to_owned(),
        party_record_id.clone(),
        resource_version,
        DataClass::Personal,
        EvidenceClass::RetainMinimizedEvidence,
        RetentionPolicyId::try_new("crm.parties.business_record").unwrap(),
    )
    .unwrap();
    let contribution = OwnerScopeContribution::new(
        contract,
        tenant_id.clone(),
        party_record_id,
        1,
        vec![resource],
        ContributionCompletenessProof::new(true, 1, 1, 1, [0x73; 32]).unwrap(),
    )
    .unwrap();
    let discovery = DiscoveryOwnerScopeContribution::new(lineage.clone(), contribution).unwrap();
    let snapshot = DiscoveryScopeSnapshot::finalize(
        lineage,
        registry,
        BASE_TIME_NANOS,
        [discovery],
    )
    .unwrap();
    let plan = PrivacyActionPlan::build(
        &snapshot,
        1,
        PrivacyCaseKind::Erasure,
        ActionPlanningPolicy::new(
            SchemaVersion::try_new("replay-convergence-policy/1").unwrap(),
            "GLOBAL".to_owned(),
            false,
            false,
        )
        .unwrap(),
        BASE_TIME_NANOS + 1,
    )
    .map_err(|error| format!("{error:?}"))?;
    let decision = PrivacyRetentionDecisionSet::build(&plan, &[], BASE_TIME_NANOS + 2)
        .map_err(|error| format!("{error:?}"))?;
    PrivacyOwnerActionAttempt::build(
        tenant_id,
        privacy_case_id,
        plan.plan_id().clone(),
        *plan.digest(),
        decision.decision_id().clone(),
        *decision.digest(),
        &decision.items()[0],
        0,
        BASE_TIME_NANOS + 3,
    )
    .map_err(|error| format!("{error:?}"))
}

fn capability_request(
    definition: &crm_capability_runtime::CapabilityDefinition,
    attempt: &PrivacyOwnerActionAttempt,
) -> CapabilityRequest {
    let command = PrivacyOwnerActionCommand::from_attempt(attempt).unwrap();
    let input = owner_action_input_payload(encode_owner_action_command(&command).unwrap()).unwrap();
    let transaction = format!("privacy-replay-{}", attempt.attempt_id());
    CapabilityRequest {
        context: ModuleExecutionContext {
            module_id: definition.owner_module_id.clone(),
            execution: ExecutionContext {
                tenant_id: attempt.tenant_id().clone(),
                actor_id: ActorId::try_new(ACTOR_ID).unwrap(),
                request_id: RequestId::try_new(format!("request-{}", attempt.attempt_id())).unwrap(),
                correlation_id: CorrelationId::try_new("privacy-replay-correlation").unwrap(),
                causation_id: CausationId::try_new(format!("cause-{}", attempt.attempt_id())).unwrap(),
                trace_id: TraceId::try_new("privacy-replay-trace").unwrap(),
                capability_id: definition.capability_id.clone(),
                capability_version: definition.capability_version.clone(),
                idempotency_key: attempt.target_idempotency_key().clone(),
                business_transaction_id: BusinessTransactionId::try_new(transaction).unwrap(),
                schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
                request_started_at_unix_nanos: attempt.planned_at_unix_nanos(),
            },
        },
        input_hash: discovery_sha256(&input.bytes),
        input,
        approval: None,
    }
}

async fn seed_tenant(admin: &PgPool, tenant: &str) {
    sqlx::query(
        "INSERT INTO crm.tenants (tenant_id, status, data_region) VALUES ($1, 'active', 'eu-central') ON CONFLICT (tenant_id) DO NOTHING",
    )
    .bind(tenant)
    .execute(admin)
    .await
    .unwrap();
}

async fn seed_actor(admin: &PgPool, tenant: &str) {
    let mut transaction = admin.begin().await.unwrap();
    sqlx::query("SET LOCAL session_replication_role = 'replica'")
        .execute(&mut *transaction)
        .await
        .unwrap();
    sqlx::query(
        r#"
        INSERT INTO crm.actors (
          tenant_id, actor_id, actor_type, status, display_name,
          version, last_business_transaction_id
        ) VALUES ($1, $2, 'service', 'active', 'Privacy Replay Convergence', 1, 'fixture-actor')
        ON CONFLICT (tenant_id, actor_id) DO NOTHING
        "#,
    )
    .bind(tenant)
    .bind(ACTOR_ID)
    .execute(&mut *transaction)
    .await
    .unwrap();
    transaction.commit().await.unwrap();
}

async fn seed_owner_action_capability(admin: &PgPool) {
    sqlx::query(
        r#"
        INSERT INTO crm.capability_registry (
          capability_id, capability_version, owner_module_id, owner_module_version,
          service_name, method_name, input_descriptor_hash, output_descriptor_hash,
          risk_level, idempotency_required, audit_required, approval_required,
          ai_callable, marketplace_callable, bulk_allowed, export_allowed,
          data_classes_touched
        ) VALUES ($1, '1.0.0', 'crm.parties', '0.3.0',
                  'crm.parties.internal.PrivacyOwnerAction', 'Apply', $2, $2,
                  'critical', true, true, false, false, false, false, false,
                  ARRAY['personal', 'restricted']::text[])
        ON CONFLICT (capability_id, capability_version) DO NOTHING
        "#,
    )
    .bind(OWNER_ACTION_CAPABILITY_ID)
    .bind(crm_customer_privacy::owner_action_command_descriptor_hash().as_slice())
    .execute(admin)
    .await
    .unwrap();
}

async fn seed_party(admin: &PgPool, tenant: &str, party_id: &str) {
    let contract = persisted_contract();
    let party = Party::create(CreateParty {
        party_id: PartyId::try_new(party_id).unwrap(),
        kind: PartyKind::Person,
        display_name: ORIGINAL_NAME.to_owned(),
        occurred_at_unix_nanos: 10,
    })
    .unwrap();
    let payload = persisted_payload(&party).unwrap().bytes;
    let mut transaction = admin.begin().await.unwrap();
    sqlx::query("SET LOCAL session_replication_role = 'replica'")
        .execute(&mut *transaction)
        .await
        .unwrap();
    sqlx::query(
        r#"
        INSERT INTO crm.business_transactions (
          tenant_id, business_transaction_id, actor_id, request_id,
          correlation_id, trace_id, capability_id, capability_version,
          expected_outbox_events, expected_audit_records, expected_idempotency_records
        ) VALUES ($1, $2, 'fixture', 'fixture-request', 'fixture-correlation',
                  'fixture-trace', 'parties.party.create', '1.0.0', 0, 0, 0)
        "#,
    )
    .bind(tenant)
    .bind(format!("fixture-party-{party_id}"))
    .execute(&mut *transaction)
    .await
    .unwrap();
    sqlx::query(
        r#"
        INSERT INTO crm.records (
          tenant_id, record_type, record_id, owner_module_id, version,
          schema_id, schema_version, descriptor_hash, data_class, payload_encoding,
          maximum_payload_size, retention_policy_id, payload_bytes,
          last_business_transaction_id, created_at, updated_at, deleted_at
        ) VALUES ($1, $2, $3, 'crm.parties', 1, $4, $5, $6, 'personal', 'json',
                  $7, $8, $9, $10, clock_timestamp(), clock_timestamp(), NULL)
        "#,
    )
    .bind(tenant)
    .bind(RECORD_TYPE)
    .bind(party_id)
    .bind(contract.schema_id)
    .bind(contract.schema_version)
    .bind(contract.descriptor_hash.as_slice())
    .bind(i64::try_from(contract.maximum_size_bytes).unwrap())
    .bind(contract.retention_policy_id)
    .bind(payload)
    .bind(format!("fixture-party-{party_id}"))
    .execute(&mut *transaction)
    .await
    .unwrap();
    transaction.commit().await.unwrap();
}

fn unique_id() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock after unix epoch")
        .as_nanos()
}
