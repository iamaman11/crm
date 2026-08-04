use crm_core_data::PostgresDataStore;
use crm_customer_privacy_production::{
    ACTION_PLAN_RECORD_TYPE, ACTION_PLAN_STATE_MAXIMUM_BYTES,
    ACTION_PLAN_STATE_RETENTION_POLICY_ID, ACTION_PLAN_STATE_SCHEMA_ID,
    ACTION_PLAN_STATE_SCHEMA_VERSION, ActionPlanningPolicy, ContributionCompletenessProof,
    DiscoveryOwnerScopeContribution, DiscoveryScopeSnapshot, EvidenceClass, ExecutionPreparation,
    OwnerExecutionPersistencePort, OwnerScopeContract, OwnerScopeContribution, OwnerScopeRegistry,
    PostgresOwnerExecutionPersistence, PrivacyActionPlan, PrivacyCase, PrivacyCaseKind,
    PrivacyOwnerActionOutcome, PrivacyOwnerOutcomeStatus, PrivacyRetentionDecisionSet,
    ScopeDiscoveryLineage, ScopeResource, SubjectVerificationMethod,
    action_plan_state_descriptor_hash, encode_action_plan_state, privacy_case_persisted_payload,
    retention_decision_persisted_payload,
};
use crm_module_sdk::{
    ActorId, CapabilityId, CapabilityVersion, DataClass, ModuleId, PayloadEncoding, RecordId,
    RetentionPolicyId, SchemaId, SchemaVersion, TenantId, TypedPayload,
};
use sqlx::{PgPool, Postgres, Transaction};
use std::sync::Arc;

const TENANT_A: &str = "tenant-a";
const TENANT_B: &str = "tenant-b";
const ACTOR: &str = "actor-a";
const CASE_ID: &str = "owner-ready-case-a";
const PARTY_ID: &str = "owner-ready-party-a";
const READY_TRANSACTION: &str = "owner-ready-retention-transaction";
const READY_REQUEST: &str = "owner-ready-retention-request";
const READY_CORRELATION: &str = "owner-ready-retention-correlation";
const READY_TRACE: &str = "owner-ready-retention-trace";
const CAPTURED_AT: i64 = 2_000_000;
const PLANNED_AT: i64 = 3_000_000;
const DECIDED_AT: i64 = 4_000_000;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn postgres_ready_work_is_bounded_resumable_lineage_exact_and_tenant_bound() {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!("skipping ready-work PostgreSQL test because DATABASE_URL is absent");
        return;
    };
    let admin_database_url = std::env::var("ADMIN_DATABASE_URL")
        .expect("ADMIN_DATABASE_URL must accompany DATABASE_URL");
    let admin = PgPool::connect(&admin_database_url)
        .await
        .expect("connect ready-work admin pool");
    cleanup(&admin).await;

    let (privacy_case, plan, decision) = build_case_plan_and_decision();
    seed_record(
        &admin,
        "owner-ready-fixture-case",
        "test.record.mutate",
        "request-owner-ready-fixture-case",
        "correlation-owner-ready-fixture-case",
        "trace-owner-ready-fixture-case",
        "customer-privacy.case",
        CASE_ID,
        privacy_case.version(),
        privacy_case_persisted_payload(&privacy_case).expect("encode ready case"),
    )
    .await;
    seed_record(
        &admin,
        "owner-ready-fixture-plan",
        "test.record.mutate",
        "request-owner-ready-fixture-plan",
        "correlation-owner-ready-fixture-plan",
        "trace-owner-ready-fixture-plan",
        ACTION_PLAN_RECORD_TYPE,
        plan.plan_id().as_str(),
        1,
        action_plan_payload(&plan),
    )
    .await;
    seed_record(
        &admin,
        READY_TRANSACTION,
        "customer_privacy.case.approve",
        READY_REQUEST,
        READY_CORRELATION,
        READY_TRACE,
        "customer-privacy.retention-decision",
        decision.decision_id().as_str(),
        1,
        retention_decision_persisted_payload(&decision).expect("encode ready decision"),
    )
    .await;

    let app = PgPool::connect(&database_url)
        .await
        .expect("connect ready-work app pool");
    let persistence: Arc<dyn OwnerExecutionPersistencePort> =
        PostgresOwnerExecutionPersistence::new(Arc::new(PostgresDataStore::from_pool(app))).into();
    let tenant_a = TenantId::try_new(TENANT_A).unwrap();
    let tenant_b = TenantId::try_new(TENANT_B).unwrap();

    let initial = load_ready(&persistence, &tenant_a, 5_000_000, 64).await;
    assert_eq!(initial.len(), 1);
    let invocation = &initial[0];
    assert_eq!(invocation.tenant_id, tenant_a);
    assert_eq!(invocation.privacy_case_id.as_str(), CASE_ID);
    assert_eq!(invocation.action_plan_id, *plan.plan_id());
    assert_eq!(invocation.retention_decision_id, *decision.decision_id());
    assert_eq!(invocation.actor_id.as_str(), ACTOR);
    assert_eq!(invocation.request_id.as_str(), READY_REQUEST);
    assert_eq!(invocation.correlation_id.as_str(), READY_CORRELATION);
    assert_eq!(invocation.trace_id.as_str(), READY_TRACE);
    assert_eq!(
        invocation.initiating_capability_id.as_str(),
        "customer_privacy.case.approve"
    );
    assert_eq!(invocation.initiating_capability_version.as_str(), "1.0.0");
    assert_eq!(invocation.request_started_at_unix_nanos, DECIDED_AT);
    assert_eq!(invocation.planned_at_unix_nanos, 5_000_000);
    assert!(invocation.trusted_internal);

    assert!(
        load_ready(&persistence, &tenant_b, 5_000_000, 64)
            .await
            .is_empty()
    );
    let error = persistence
        .load_ready(&tenant_a, 5_000_000, 0)
        .await
        .expect_err("zero ready-work bound must fail closed");
    assert_eq!(
        error.code.as_str(),
        "CUSTOMER_PRIVACY_OWNER_EXECUTION_READY_WORK_INVALID"
    );

    let first_preparation = persistence
        .prepare_next(invocation)
        .await
        .expect("initialize ready-work checkpoint and attempt");
    let first_attempt = match first_preparation {
        ExecutionPreparation::Ready { attempt, .. } => *attempt,
        other => panic!("expected ready attempt, got {other:?}"),
    };

    let resumed = load_ready(&persistence, &tenant_a, 5_500_000, 64).await;
    assert_eq!(resumed.len(), 1);
    assert_eq!(resumed[0].privacy_case_id.as_str(), CASE_ID);
    assert_eq!(resumed[0].request_id.as_str(), READY_REQUEST);
    assert_eq!(resumed[0].correlation_id.as_str(), READY_CORRELATION);
    assert_eq!(resumed[0].trace_id.as_str(), READY_TRACE);
    assert_eq!(resumed[0].planned_at_unix_nanos, 5_500_000);

    let outcome = PrivacyOwnerActionOutcome::record(
        &first_attempt,
        PrivacyOwnerOutcomeStatus::Succeeded,
        None,
        5_600_000,
    )
    .unwrap();
    assert!(
        persistence
            .record_outcome(&resumed[0], &first_attempt, &outcome)
            .await
            .expect("record ready-work outcome")
    );
    let checkpoint = persistence
        .advance_checkpoint(&resumed[0])
        .await
        .expect("complete ready-work checkpoint");
    assert!(checkpoint.complete);
    assert!(
        load_ready(&persistence, &tenant_a, 6_000_000, 64)
            .await
            .is_empty()
    );

    cleanup(&admin).await;
}

async fn load_ready(
    persistence: &Arc<dyn OwnerExecutionPersistencePort>,
    tenant_id: &TenantId,
    now_unix_nanos: i64,
    maximum_items: u32,
) -> Vec<crm_customer_privacy_production::OwnerExecutionInvocation> {
    persistence
        .load_ready(tenant_id, now_unix_nanos, maximum_items)
        .await
        .expect("load PostgreSQL owner-execution ready work")
}

fn build_case_plan_and_decision() -> (PrivacyCase, PrivacyActionPlan, PrivacyRetentionDecisionSet) {
    let tenant_id = TenantId::try_new(TENANT_A).unwrap();
    let canonical_party_id = RecordId::try_new(PARTY_ID).unwrap();
    let privacy_case_id = RecordId::try_new(CASE_ID).unwrap();
    let contract = OwnerScopeContract::new(
        ModuleId::try_new("crm.parties").unwrap(),
        CapabilityId::try_new("parties.privacy.scope.contribute").unwrap(),
        CapabilityVersion::try_new("1.0.0").unwrap(),
    );
    let registry = OwnerScopeRegistry::new(
        SchemaVersion::try_new("owner-ready-registry/1").unwrap(),
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
        1,
    )
    .unwrap();
    let contribution = OwnerScopeContribution::new(
        contract,
        tenant_id.clone(),
        canonical_party_id.clone(),
        1,
        [ScopeResource::new(
            "party.profile",
            RecordId::try_new("owner-ready-resource-a").unwrap(),
            1,
            DataClass::Personal,
            EvidenceClass::DestroyableSubjectData,
            RetentionPolicyId::try_new("owner-ready-retention").unwrap(),
        )
        .unwrap()],
        ContributionCompletenessProof::new(true, 1, 1, 1, [8; 32]).unwrap(),
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
            RecordId::try_new("owner-ready-submitted-party").unwrap(),
            canonical_party_id,
            1,
            SubjectVerificationMethod::AuthenticatedPortal,
            ActorId::try_new(ACTOR).unwrap(),
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

#[allow(clippy::too_many_arguments)]
async fn seed_record(
    admin: &PgPool,
    transaction_id: &str,
    capability_id: &str,
    request_id: &str,
    correlation_id: &str,
    trace_id: &str,
    record_type: &str,
    record_id: &str,
    version: u64,
    payload: TypedPayload,
) {
    let mut transaction = admin.begin().await.expect("begin ready-work fixture");
    sqlx::query("SET LOCAL session_replication_role = replica")
        .execute(&mut *transaction)
        .await
        .expect("disable trigger-backed fixture verification");
    insert_fixture_transaction(
        &mut transaction,
        transaction_id,
        capability_id,
        request_id,
        correlation_id,
        trace_id,
    )
    .await;
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
    .bind(TENANT_A)
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
    .expect("insert ready-work fixture record");
    transaction
        .commit()
        .await
        .expect("commit ready-work fixture");
}

async fn insert_fixture_transaction(
    transaction: &mut Transaction<'_, Postgres>,
    transaction_id: &str,
    capability_id: &str,
    request_id: &str,
    correlation_id: &str,
    trace_id: &str,
) {
    sqlx::query(
        r#"
        INSERT INTO crm.business_transactions (
          tenant_id, business_transaction_id, actor_id, request_id,
          correlation_id, trace_id, capability_id, capability_version,
          expected_outbox_events, expected_audit_records, expected_idempotency_records
        ) VALUES ($1,$2,$3,$4,$5,$6,$7,'1.0.0',1,1,1)
        "#,
    )
    .bind(TENANT_A)
    .bind(transaction_id)
    .bind(ACTOR)
    .bind(request_id)
    .bind(correlation_id)
    .bind(trace_id)
    .bind(capability_id)
    .execute(&mut **transaction)
    .await
    .expect("insert ready-work fixture business transaction");
}

async fn cleanup(admin: &PgPool) {
    let mut transaction = admin.begin().await.expect("begin ready-work cleanup");
    sqlx::query("SET LOCAL session_replication_role = replica")
        .execute(&mut *transaction)
        .await
        .expect("disable ready-work cleanup verification");
    for statement in [
        "DELETE FROM crm.customer_privacy_owner_execution_audit WHERE tenant_id = 'tenant-a' AND privacy_case_id = 'owner-ready-case-a'",
        "DELETE FROM crm.customer_privacy_owner_action_outcomes WHERE tenant_id = 'tenant-a' AND privacy_case_id = 'owner-ready-case-a'",
        "DELETE FROM crm.customer_privacy_owner_action_attempts WHERE tenant_id = 'tenant-a' AND privacy_case_id = 'owner-ready-case-a'",
        "DELETE FROM crm.customer_privacy_owner_execution_checkpoints WHERE tenant_id = 'tenant-a' AND privacy_case_id = 'owner-ready-case-a'",
        "DELETE FROM crm.outbox_events WHERE tenant_id = 'tenant-a' AND (aggregate_id = 'owner-ready-case-a' OR business_transaction_id LIKE 'privacy-owner-%')",
        "DELETE FROM crm.audit_records WHERE tenant_id = 'tenant-a' AND business_transaction_id LIKE 'privacy-owner-%'",
        "DELETE FROM crm.idempotency_records WHERE tenant_id = 'tenant-a' AND business_transaction_id LIKE 'privacy-owner-%'",
        "DELETE FROM crm.records WHERE tenant_id = 'tenant-a' AND (record_id = 'owner-ready-case-a' OR last_business_transaction_id LIKE 'owner-ready-%')",
        "DELETE FROM crm.business_transactions WHERE tenant_id = 'tenant-a' AND (business_transaction_id LIKE 'owner-ready-%' OR business_transaction_id LIKE 'privacy-owner-%' OR request_id = 'owner-ready-retention-request')",
    ] {
        sqlx::query(statement)
            .execute(&mut *transaction)
            .await
            .expect("clean ready-work evidence");
    }
    transaction
        .commit()
        .await
        .expect("commit ready-work cleanup");
}

fn data_class_name(data_class: DataClass) -> &'static str {
    match data_class {
        DataClass::Confidential => "confidential",
        DataClass::Personal => "personal",
        other => panic!("unsupported ready-work data class: {other:?}"),
    }
}
