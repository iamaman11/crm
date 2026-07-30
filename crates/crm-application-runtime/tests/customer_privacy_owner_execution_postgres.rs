use crm_core_data::PostgresDataStore;
use crm_customer_privacy_production::{
    ACTION_PLAN_RECORD_TYPE, ACTION_PLAN_STATE_MAXIMUM_BYTES,
    ACTION_PLAN_STATE_RETENTION_POLICY_ID, ACTION_PLAN_STATE_SCHEMA_ID,
    ACTION_PLAN_STATE_SCHEMA_VERSION, ActionPlanningPolicy, ContributionCompletenessProof,
    DiscoveryOwnerScopeContribution, DiscoveryScopeSnapshot, EvidenceClass, ExecutionPreparation,
    OwnerExecutionInvocation, OwnerExecutionPersistencePort, OwnerScopeContract,
    OwnerScopeContribution, OwnerScopeRegistry, PostgresOwnerExecutionPersistence,
    PostgresPrivacyReadPersistence, PrivacyActionPlan, PrivacyCase, PrivacyCaseKind,
    PrivacyOwnerActionOutcome, PrivacyOwnerOutcomePosition, PrivacyOwnerOutcomeStatus,
    PrivacyReadContext, PrivacyReadPersistencePort, PrivacyRetentionDecisionSet,
    ScopeDiscoveryLineage, ScopeResource, SubjectVerificationMethod,
    action_plan_state_descriptor_hash, encode_action_plan_state, privacy_case_persisted_payload,
    retention_decision_persisted_payload,
};
use crm_module_sdk::{
    ActorId, CapabilityId, CapabilityVersion, CorrelationId, DataClass, ModuleId, PayloadEncoding,
    RecordId, RetentionPolicyId, SchemaId, SchemaVersion, TenantId, TraceId, TypedPayload,
};
use sqlx::{PgPool, Postgres, Row, Transaction};
use std::sync::Arc;

const TENANT_A: &str = "tenant-a";
const TENANT_B: &str = "tenant-b";
const ACTOR: &str = "actor-a";
const CASE_ID: &str = "owner-execution-case-a";
const PARTY_ID: &str = "owner-execution-party-a";
const EFFECTIVE_REQUEST_MS: i64 = 1;
const CAPTURED_AT: i64 = 2_000_000;
const PLANNED_AT: i64 = 3_000_000;
const DECIDED_AT: i64 = 4_000_000;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn phase_280_owner_execution_is_replay_safe_resumable_and_tenant_bound() {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!("skipping owner-execution PostgreSQL test because DATABASE_URL is absent");
        return;
    };
    let admin_database_url = std::env::var("ADMIN_DATABASE_URL")
        .expect("ADMIN_DATABASE_URL must accompany DATABASE_URL");
    let admin = PgPool::connect(&admin_database_url)
        .await
        .expect("connect owner-execution admin pool");
    cleanup(&admin).await;

    let (privacy_case, plan, decision) = build_case_plan_and_decision();
    seed_record(
        &admin,
        TENANT_A,
        "customer-privacy.case",
        CASE_ID,
        privacy_case.version(),
        privacy_case_persisted_payload(&privacy_case).expect("encode privacy case fixture"),
        "owner-execution-fixture-case",
    )
    .await;
    seed_record(
        &admin,
        TENANT_A,
        ACTION_PLAN_RECORD_TYPE,
        plan.plan_id().as_str(),
        1,
        action_plan_payload(&plan),
        "owner-execution-fixture-plan",
    )
    .await;
    seed_record(
        &admin,
        TENANT_A,
        "customer-privacy.retention-decision",
        decision.decision_id().as_str(),
        1,
        retention_decision_persisted_payload(&decision).expect("encode retention decision fixture"),
        "owner-execution-fixture-decision",
    )
    .await;

    let app = PgPool::connect(&database_url)
        .await
        .expect("connect owner-execution app pool");
    let store = Arc::new(PostgresDataStore::from_pool(app));
    let execution = PostgresOwnerExecutionPersistence::new(store.clone());
    let reads = PostgresPrivacyReadPersistence::new(store);

    let missing_plan = OwnerExecutionInvocation {
        action_plan_id: RecordId::try_new("missing-action-plan").unwrap(),
        ..invocation(
            plan.plan_id().clone(),
            decision.decision_id().clone(),
            5_000_000,
            "missing-plan",
        )
    };
    let error = execution
        .prepare_next(&missing_plan)
        .await
        .expect_err("missing immutable plan must fail closed");
    assert_eq!(
        error.code.as_str(),
        "CUSTOMER_PRIVACY_OWNER_EXECUTION_EVIDENCE_INVALID"
    );
    assert_eq!(checkpoint_count(&admin, TENANT_A).await, 0);

    let missing_decision = OwnerExecutionInvocation {
        retention_decision_id: RecordId::try_new("missing-retention-decision").unwrap(),
        ..invocation(
            plan.plan_id().clone(),
            decision.decision_id().clone(),
            5_000_000,
            "missing-decision",
        )
    };
    let error = execution
        .prepare_next(&missing_decision)
        .await
        .expect_err("missing retention decision must fail closed");
    assert_eq!(
        error.code.as_str(),
        "CUSTOMER_PRIVACY_OWNER_EXECUTION_EVIDENCE_INVALID"
    );
    assert_eq!(checkpoint_count(&admin, TENANT_A).await, 0);

    let cross_tenant = OwnerExecutionInvocation {
        tenant_id: TenantId::try_new(TENANT_B).unwrap(),
        ..invocation(
            plan.plan_id().clone(),
            decision.decision_id().clone(),
            5_000_000,
            "cross-tenant",
        )
    };
    let error = execution
        .prepare_next(&cross_tenant)
        .await
        .expect_err("cross-tenant execution must conceal source existence");
    assert_eq!(error.code.as_str(), "CUSTOMER_PRIVACY_CASE_NOT_FOUND");
    assert_eq!(checkpoint_count(&admin, TENANT_B).await, 0);

    let first_invocation = invocation(
        plan.plan_id().clone(),
        decision.decision_id().clone(),
        5_000_000,
        "first",
    );
    let first_attempt = ready(
        execution
            .prepare_next(&first_invocation)
            .await
            .expect("durably prepare first attempt"),
        false,
    );
    assert_eq!(first_attempt.item_sequence(), 1);
    assert_eq!(first_attempt.attempt_generation(), 0);
    assert_eq!(attempt_count(&admin, TENANT_A).await, 1);
    assert_checkpoint(&admin, 1, false).await;

    let pre_invocation_replay = ready(
        execution
            .prepare_next(&first_invocation)
            .await
            .expect("recover crash before owner invocation"),
        true,
    );
    assert_eq!(pre_invocation_replay, first_attempt);
    assert_eq!(attempt_count(&admin, TENANT_A).await, 1);

    let post_owner_result_replay = ready(
        execution
            .prepare_next(&invocation(
                plan.plan_id().clone(),
                decision.decision_id().clone(),
                5_100_000,
                "post-owner-result",
            ))
            .await
            .expect("recover crash after owner result but before durable outcome"),
        true,
    );
    assert_eq!(
        post_owner_result_replay.attempt_id(),
        first_attempt.attempt_id()
    );
    assert_eq!(
        post_owner_result_replay.target_idempotency_key(),
        first_attempt.target_idempotency_key()
    );

    let retryable = PrivacyOwnerActionOutcome::record(
        &first_attempt,
        PrivacyOwnerOutcomeStatus::FailedRetryable,
        Some("OWNER_TEMPORARILY_UNAVAILABLE".to_owned()),
        5_200_000,
    )
    .unwrap();
    assert!(
        execution
            .record_outcome(&first_invocation, &first_attempt, &retryable)
            .await
            .expect("append retryable outcome")
    );
    assert!(
        !execution
            .record_outcome(&first_invocation, &first_attempt, &retryable)
            .await
            .expect("exact retryable outcome replay")
    );
    assert_eq!(outcome_count(&admin, TENANT_A).await, 1);
    assert_checkpoint(&admin, 1, false).await;

    let retry_invocation = invocation(
        plan.plan_id().clone(),
        decision.decision_id().clone(),
        6_000_000,
        "retry",
    );
    let retry_attempt = ready(
        execution
            .prepare_next(&retry_invocation)
            .await
            .expect("prepare retry generation"),
        false,
    );
    assert_eq!(retry_attempt.item_sequence(), 1);
    assert_eq!(retry_attempt.attempt_generation(), 1);
    assert_ne!(retry_attempt.attempt_id(), first_attempt.attempt_id());
    assert_eq!(
        retry_attempt.target_idempotency_key(),
        first_attempt.target_idempotency_key(),
        "owner target key must remain permanent across retry generations"
    );

    let retry_success = PrivacyOwnerActionOutcome::record(
        &retry_attempt,
        PrivacyOwnerOutcomeStatus::Succeeded,
        None,
        6_100_000,
    )
    .unwrap();
    assert!(
        execution
            .record_outcome(&retry_invocation, &retry_attempt, &retry_success)
            .await
            .expect("append successful retry outcome")
    );

    let conflicting = PrivacyOwnerActionOutcome::record(
        &retry_attempt,
        PrivacyOwnerOutcomeStatus::FailedTerminal,
        Some("CONFLICTING_REPLAY".to_owned()),
        6_100_000,
    )
    .unwrap();
    let error = execution
        .record_outcome(&retry_invocation, &retry_attempt, &conflicting)
        .await
        .expect_err("conflicting replay must fail closed");
    assert_eq!(
        error.code.as_str(),
        "CUSTOMER_PRIVACY_OWNER_EXECUTION_CONFLICT"
    );
    assert_eq!(outcome_count(&admin, TENANT_A).await, 2);

    let second_invocation = invocation(
        plan.plan_id().clone(),
        decision.decision_id().clone(),
        7_000_000,
        "second",
    );
    let second_attempt = ready(
        execution
            .prepare_next(&second_invocation)
            .await
            .expect("normalize post-outcome crash and prepare second item"),
        false,
    );
    assert_eq!(second_attempt.item_sequence(), 2);
    assert_eq!(second_attempt.attempt_generation(), 0);
    assert_checkpoint(&admin, 2, false).await;

    let second_success = PrivacyOwnerActionOutcome::record(
        &second_attempt,
        PrivacyOwnerOutcomeStatus::Succeeded,
        None,
        7_100_000,
    )
    .unwrap();
    assert!(
        execution
            .record_outcome(&second_invocation, &second_attempt, &second_success)
            .await
            .expect("append second successful outcome")
    );
    assert_eq!(outcome_count(&admin, TENANT_A).await, 3);

    let completion_invocation = invocation(
        plan.plan_id().clone(),
        decision.decision_id().clone(),
        8_000_000,
        "complete",
    );
    let completion = execution
        .prepare_next(&completion_invocation)
        .await
        .expect("recover post-outcome pre-checkpoint crash and complete");
    match completion {
        ExecutionPreparation::Complete {
            total_items,
            durable_outcomes,
        } => {
            assert_eq!(total_items, 2);
            assert_eq!(durable_outcomes, 2);
        }
        other => panic!("expected terminal completion, got {other:?}"),
    }
    assert_checkpoint(&admin, 3, true).await;
    assert_eq!(attempt_count(&admin, TENANT_A).await, 3);
    assert_eq!(outcome_count(&admin, TENANT_A).await, 3);

    let replay = execution
        .prepare_next(&completion_invocation)
        .await
        .expect("replay completed execution");
    assert!(matches!(replay, ExecutionPreparation::Complete { .. }));
    assert_eq!(attempt_count(&admin, TENANT_A).await, 3);
    assert_eq!(outcome_count(&admin, TENANT_A).await, 3);

    let before_reads = execution_table_counts(&admin).await;
    let context = read_context(TENANT_A, "page-a");
    let page_one = reads
        .load_owner_outcomes(
            &context,
            &RecordId::try_new(CASE_ID).unwrap(),
            plan.plan_id(),
            None,
            None,
            1,
        )
        .await
        .expect("read first bounded outcome page");
    assert_eq!(page_one.outcomes.len(), 1);
    assert!(page_one.has_more);
    let first_position = position(&page_one.outcomes[0]);

    let page_two = reads
        .load_owner_outcomes(
            &read_context(TENANT_A, "page-b"),
            &RecordId::try_new(CASE_ID).unwrap(),
            plan.plan_id(),
            None,
            Some(&first_position),
            1,
        )
        .await
        .expect("read second keyset page");
    assert_eq!(page_two.outcomes.len(), 1);
    assert!(page_two.has_more);
    let second_position = position(&page_two.outcomes[0]);
    assert!(second_position.item_sequence >= first_position.item_sequence);

    let page_three = reads
        .load_owner_outcomes(
            &read_context(TENANT_A, "page-c"),
            &RecordId::try_new(CASE_ID).unwrap(),
            plan.plan_id(),
            Some(&ModuleId::try_new("crm.parties").unwrap()),
            Some(&second_position),
            1,
        )
        .await
        .expect("read filtered terminal keyset page");
    assert_eq!(page_three.outcomes.len(), 1);
    assert!(!page_three.has_more);
    assert_eq!(page_three.outcomes[0].item_sequence(), 2);

    let wrong_owner = reads
        .load_owner_outcomes(
            &read_context(TENANT_A, "wrong-owner"),
            &RecordId::try_new(CASE_ID).unwrap(),
            plan.plan_id(),
            Some(&ModuleId::try_new("crm.consents").unwrap()),
            None,
            64,
        )
        .await
        .expect("owner filter must be bounded and safe");
    assert!(wrong_owner.outcomes.is_empty());
    assert!(!wrong_owner.has_more);

    let cross_tenant_page = reads
        .load_owner_outcomes(
            &read_context(TENANT_B, "cross-tenant"),
            &RecordId::try_new(CASE_ID).unwrap(),
            plan.plan_id(),
            None,
            None,
            64,
        )
        .await
        .expect("cross-tenant read must conceal outcomes");
    assert!(cross_tenant_page.outcomes.is_empty());
    assert_eq!(execution_table_counts(&admin).await, before_reads);

    assert_immutable_evidence(
        &admin,
        retry_attempt.attempt_id().as_str(),
        retry_success.outcome_id().as_str(),
    )
    .await;
    cleanup(&admin).await;
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
        SchemaVersion::try_new("owner-execution-registry/1").unwrap(),
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
        EFFECTIVE_REQUEST_MS,
    )
    .unwrap();
    let resources = [
        resource("party.profile", "owner-execution-resource-a"),
        resource("party.preferences", "owner-execution-resource-b"),
    ];
    let contribution = OwnerScopeContribution::new(
        contract,
        tenant_id.clone(),
        canonical_party_id.clone(),
        1,
        resources,
        ContributionCompletenessProof::new(true, 1, 2, 2, [7; 32]).unwrap(),
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
            RecordId::try_new("submitted-party-a").unwrap(),
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

fn resource(resource_type: &str, resource_id: &str) -> ScopeResource {
    ScopeResource::new(
        resource_type,
        RecordId::try_new(resource_id).unwrap(),
        1,
        DataClass::Personal,
        EvidenceClass::DestroyableSubjectData,
        RetentionPolicyId::try_new(format!("retention-{resource_id}")).unwrap(),
    )
    .unwrap()
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

fn invocation(
    action_plan_id: RecordId,
    retention_decision_id: RecordId,
    planned_at_unix_nanos: i64,
    suffix: &str,
) -> OwnerExecutionInvocation {
    OwnerExecutionInvocation {
        tenant_id: TenantId::try_new(TENANT_A).unwrap(),
        privacy_case_id: RecordId::try_new(CASE_ID).unwrap(),
        action_plan_id,
        retention_decision_id,
        actor_id: ActorId::try_new(ACTOR).unwrap(),
        request_id: crm_module_sdk::RequestId::try_new(format!("owner-execution-request-{suffix}"))
            .unwrap(),
        correlation_id: CorrelationId::try_new(format!("owner-execution-correlation-{suffix}"))
            .unwrap(),
        trace_id: TraceId::try_new(format!("owner-execution-trace-{suffix}")).unwrap(),
        initiating_capability_id: CapabilityId::try_new("customer_privacy.case.approve").unwrap(),
        initiating_capability_version: CapabilityVersion::try_new("1.0.0").unwrap(),
        request_started_at_unix_nanos: planned_at_unix_nanos - 1_000,
        planned_at_unix_nanos,
        trusted_internal: true,
    }
}

fn read_context(tenant: &str, suffix: &str) -> PrivacyReadContext {
    PrivacyReadContext {
        tenant_id: TenantId::try_new(tenant).unwrap(),
        actor_id: ActorId::try_new(if tenant == TENANT_A { ACTOR } else { "actor-b" }).unwrap(),
        request_id: crm_module_sdk::RequestId::try_new(format!("owner-outcome-read-{suffix}"))
            .unwrap(),
        correlation_id: CorrelationId::try_new(format!("owner-outcome-correlation-{suffix}"))
            .unwrap(),
        trace_id: TraceId::try_new(format!("owner-outcome-trace-{suffix}")).unwrap(),
        capability_id: CapabilityId::try_new("customer_privacy.case.owner_outcomes.list").unwrap(),
        capability_version: CapabilityVersion::try_new("1.0.0").unwrap(),
        request_started_at_unix_nanos: 9_000_000,
    }
}

fn ready(
    preparation: ExecutionPreparation,
    replayed: bool,
) -> crm_customer_privacy_production::PrivacyOwnerActionAttempt {
    match preparation {
        ExecutionPreparation::Ready {
            attempt,
            attempt_replayed,
        } => {
            assert_eq!(attempt_replayed, replayed);
            *attempt
        }
        other => panic!("expected ready attempt, got {other:?}"),
    }
}

fn position(outcome: &PrivacyOwnerActionOutcome) -> PrivacyOwnerOutcomePosition {
    PrivacyOwnerOutcomePosition {
        item_sequence: outcome.item_sequence(),
        attempt_generation: outcome.attempt_generation(),
        outcome_id: outcome.outcome_id().clone(),
    }
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
    let mut transaction = admin.begin().await.expect("begin owner-execution fixture");
    sqlx::query("SET LOCAL session_replication_role = replica")
        .execute(&mut *transaction)
        .await
        .expect("disable trigger-backed fixture verification");
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
    .expect("insert owner-execution fixture record");
    transaction
        .commit()
        .await
        .expect("commit owner-execution fixture");
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
        ) VALUES ($1,$2,$3,$4,$5,$6,'test.record.mutate','1.0.0',1,1,1)
        "#,
    )
    .bind(tenant)
    .bind(transaction_id)
    .bind(if tenant == TENANT_A { ACTOR } else { "actor-b" })
    .bind(format!("request-{transaction_id}"))
    .bind(format!("correlation-{transaction_id}"))
    .bind(format!("trace-{transaction_id}"))
    .execute(&mut **transaction)
    .await
    .expect("insert owner-execution fixture business transaction");
}

async fn assert_checkpoint(admin: &PgPool, next_sequence: i32, complete: bool) {
    let row = sqlx::query(
        r#"
        SELECT next_sequence, completed_at_unix_nanos, converging_case_version
        FROM crm.customer_privacy_owner_execution_checkpoints
        WHERE tenant_id = $1 AND privacy_case_id = $2
        "#,
    )
    .bind(TENANT_A)
    .bind(CASE_ID)
    .fetch_one(admin)
    .await
    .expect("load owner-execution checkpoint");
    assert_eq!(row.get::<i32, _>("next_sequence"), next_sequence);
    assert_eq!(
        row.try_get::<Option<i64>, _>("completed_at_unix_nanos")
            .unwrap()
            .is_some(),
        complete
    );
    assert_eq!(
        row.try_get::<Option<i64>, _>("converging_case_version")
            .unwrap()
            .is_some(),
        complete
    );
}

async fn assert_immutable_evidence(admin: &PgPool, attempt_id: &str, outcome_id: &str) {
    let attempt_update = sqlx::query(
        "UPDATE crm.customer_privacy_owner_action_attempts SET action_code = action_code WHERE tenant_id = $1 AND attempt_id = $2",
    )
    .bind(TENANT_A)
    .bind(attempt_id)
    .execute(admin)
    .await;
    assert!(
        attempt_update.is_err(),
        "attempt evidence update must be rejected"
    );

    let outcome_delete = sqlx::query(
        "DELETE FROM crm.customer_privacy_owner_action_outcomes WHERE tenant_id = $1 AND outcome_id = $2",
    )
    .bind(TENANT_A)
    .bind(outcome_id)
    .execute(admin)
    .await;
    assert!(
        outcome_delete.is_err(),
        "outcome evidence delete must be rejected"
    );
}

async fn checkpoint_count(admin: &PgPool, tenant: &str) -> i64 {
    sqlx::query_scalar(
        "SELECT count(*) FROM crm.customer_privacy_owner_execution_checkpoints WHERE tenant_id = $1",
    )
    .bind(tenant)
    .fetch_one(admin)
    .await
    .expect("count owner-execution checkpoints")
}

async fn attempt_count(admin: &PgPool, tenant: &str) -> i64 {
    sqlx::query_scalar(
        "SELECT count(*) FROM crm.customer_privacy_owner_action_attempts WHERE tenant_id = $1",
    )
    .bind(tenant)
    .fetch_one(admin)
    .await
    .expect("count owner-execution attempts")
}

async fn outcome_count(admin: &PgPool, tenant: &str) -> i64 {
    sqlx::query_scalar(
        "SELECT count(*) FROM crm.customer_privacy_owner_action_outcomes WHERE tenant_id = $1",
    )
    .bind(tenant)
    .fetch_one(admin)
    .await
    .expect("count owner-execution outcomes")
}

async fn execution_table_counts(admin: &PgPool) -> (i64, i64, i64, i64) {
    (
        checkpoint_count(admin, TENANT_A).await,
        attempt_count(admin, TENANT_A).await,
        outcome_count(admin, TENANT_A).await,
        sqlx::query_scalar(
            "SELECT count(*) FROM crm.customer_privacy_owner_execution_audit WHERE tenant_id = $1",
        )
        .bind(TENANT_A)
        .fetch_one(admin)
        .await
        .expect("count owner-execution audit"),
    )
}

async fn cleanup(admin: &PgPool) {
    let mut transaction = admin.begin().await.expect("begin owner-execution cleanup");
    sqlx::query("SET LOCAL session_replication_role = replica")
        .execute(&mut *transaction)
        .await
        .expect("disable owner-execution cleanup verification");
    sqlx::query(
        "DELETE FROM crm.customer_privacy_owner_execution_audit WHERE tenant_id IN ($1, $2)",
    )
    .bind(TENANT_A)
    .bind(TENANT_B)
    .execute(&mut *transaction)
    .await
    .expect("delete owner-execution audit");
    sqlx::query(
        "DELETE FROM crm.customer_privacy_owner_action_outcomes WHERE tenant_id IN ($1, $2)",
    )
    .bind(TENANT_A)
    .bind(TENANT_B)
    .execute(&mut *transaction)
    .await
    .expect("delete owner-execution outcomes");
    sqlx::query(
        "DELETE FROM crm.customer_privacy_owner_action_attempts WHERE tenant_id IN ($1, $2)",
    )
    .bind(TENANT_A)
    .bind(TENANT_B)
    .execute(&mut *transaction)
    .await
    .expect("delete owner-execution attempts");
    sqlx::query(
        "DELETE FROM crm.customer_privacy_owner_execution_checkpoints WHERE tenant_id IN ($1, $2)",
    )
    .bind(TENANT_A)
    .bind(TENANT_B)
    .execute(&mut *transaction)
    .await
    .expect("delete owner-execution checkpoints");
    sqlx::query(
        "DELETE FROM crm.records WHERE record_id = $1 OR starts_with(last_business_transaction_id, 'owner-execution-fixture-')",
    )
    .bind(CASE_ID)
    .execute(&mut *transaction)
    .await
    .expect("delete owner-execution fixture records");
    sqlx::query(
        "DELETE FROM crm.business_transactions WHERE starts_with(business_transaction_id, 'owner-execution-fixture-') OR starts_with(request_id, 'owner-execution-request-')",
    )
    .execute(&mut *transaction)
    .await
    .expect("delete owner-execution fixture transactions");
    transaction
        .commit()
        .await
        .expect("commit owner-execution cleanup");
}

fn data_class_name(data_class: DataClass) -> &'static str {
    match data_class {
        DataClass::Confidential => "confidential",
        DataClass::Personal => "personal",
        other => panic!("unsupported fixture data class: {other:?}"),
    }
}
