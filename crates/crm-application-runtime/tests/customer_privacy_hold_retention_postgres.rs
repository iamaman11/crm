use crm_core_data::PostgresDataStore;
use crm_customer_privacy_production::{
    ACTION_PLAN_RECORD_TYPE, ACTION_PLAN_STATE_MAXIMUM_BYTES,
    ACTION_PLAN_STATE_RETENTION_POLICY_ID, ACTION_PLAN_STATE_SCHEMA_ID,
    ACTION_PLAN_STATE_SCHEMA_VERSION, ActionPlanningPolicy, ContributionCompletenessProof,
    CustomerDataLegalHold, DiscoveryOwnerScopeContribution, DiscoveryScopeSnapshot, EvidenceClass,
    LegalHoldScope, OwnerScopeContract, OwnerScopeContribution, OwnerScopeRegistry,
    PlannedPrivacyAction, PostgresRetentionEvaluationPersistence, PrivacyActionPlan, PrivacyCase,
    PrivacyCaseKind, RetentionDecisionReason, RetentionEvaluationInvocation,
    RetentionEvaluationPersistencePort, ScopeDiscoveryLineage, ScopeResource,
    SubjectVerificationMethod, action_plan_state_descriptor_hash, encode_action_plan_state,
    legal_hold_persisted_payload, privacy_case_persisted_payload,
};
use crm_module_sdk::{
    ActorId, CapabilityId, CapabilityVersion, CorrelationId, DataClass, ModuleId, PayloadEncoding,
    RecordId, RetentionPolicyId, SchemaId, SchemaVersion, TenantId, TraceId, TypedPayload,
};
use sqlx::{PgPool, Postgres, Transaction};
use std::sync::Arc;

const TENANT_A: &str = "tenant-a";
const TENANT_B: &str = "tenant-b";
const ACTOR: &str = "actor-a";
const CASE_ID: &str = "retention-case-a";
const PARTY_ID: &str = "retention-party-a";
const ACTIVE_HOLD_ID: &str = "retention-hold-active";
const FUTURE_HOLD_ID: &str = "retention-hold-future";
const CROSS_TENANT_HOLD_ID: &str = "retention-hold-cross-tenant";
const MALFORMED_HOLD_ID: &str = "retention-hold-malformed";
const EFFECTIVE_REQUEST_MS: i64 = 1;
const CAPTURED_AT: i64 = 2_000_000;
const PLANNED_AT: i64 = 3_000_000;
const EVALUATED_AT: i64 = 4_000_000;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn phase_260_enforces_hold_retention_precedence_replay_and_fail_closed_state() {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!("skipping hold-retention PostgreSQL test because DATABASE_URL is absent");
        return;
    };
    let admin_database_url = std::env::var("ADMIN_DATABASE_URL")
        .expect("ADMIN_DATABASE_URL must accompany DATABASE_URL");
    let admin = PgPool::connect(&admin_database_url)
        .await
        .expect("connect hold-retention admin pool");
    cleanup(&admin).await;

    let (privacy_case, plan) = build_case_and_plan();
    seed_record(
        &admin,
        TENANT_A,
        "customer-privacy.case",
        CASE_ID,
        privacy_case.version(),
        privacy_case_persisted_payload(&privacy_case).expect("encode privacy case fixture"),
        "retention-fixture-case",
    )
    .await;
    seed_record(
        &admin,
        TENANT_A,
        ACTION_PLAN_RECORD_TYPE,
        plan.plan_id().as_str(),
        1,
        action_plan_payload(&plan),
        "retention-fixture-plan",
    )
    .await;

    let active_hold = hold(
        TENANT_A,
        ACTIVE_HOLD_ID,
        LegalHoldScope::DataClass(DataClass::Personal),
        2_500_000,
        Some(8_000_000),
    );
    seed_record(
        &admin,
        TENANT_A,
        "customer-privacy.legal-hold",
        ACTIVE_HOLD_ID,
        1,
        legal_hold_persisted_payload(&active_hold).expect("encode active hold fixture"),
        "retention-fixture-active-hold",
    )
    .await;
    let future_hold = hold(
        TENANT_A,
        FUTURE_HOLD_ID,
        LegalHoldScope::AllCustomerData,
        9_000_000,
        None,
    );
    seed_record(
        &admin,
        TENANT_A,
        "customer-privacy.legal-hold",
        FUTURE_HOLD_ID,
        1,
        legal_hold_persisted_payload(&future_hold).expect("encode future hold fixture"),
        "retention-fixture-future-hold",
    )
    .await;
    let cross_tenant_hold = hold(
        TENANT_B,
        CROSS_TENANT_HOLD_ID,
        LegalHoldScope::AllCustomerData,
        2_500_000,
        None,
    );
    seed_record(
        &admin,
        TENANT_B,
        "customer-privacy.legal-hold",
        CROSS_TENANT_HOLD_ID,
        1,
        legal_hold_persisted_payload(&cross_tenant_hold).expect("encode cross-tenant hold fixture"),
        "retention-fixture-cross-hold",
    )
    .await;

    let app = PgPool::connect(&database_url)
        .await
        .expect("connect hold-retention app pool");
    let port =
        PostgresRetentionEvaluationPersistence::new(Arc::new(PostgresDataStore::from_pool(app)));
    let invocation = invocation(
        plan.plan_id().clone(),
        EVALUATED_AT,
        "first",
        "customer_privacy.case.approve",
    );
    let before = decision_count(&admin).await;
    let first = port
        .evaluate_and_persist(&invocation)
        .await
        .expect("evaluate retention precedence on clean FORCE-RLS storage");
    assert!(!first.replayed);
    assert_eq!(decision_count(&admin).await, before + 1);
    assert_eq!(first.decision.items().len(), 3);

    let personal = item(&first.decision, "resource-personal");
    assert_eq!(personal.approved_action(), PlannedPrivacyAction::Delete);
    assert_eq!(personal.final_action(), PlannedPrivacyAction::Retain);
    assert_eq!(personal.reason(), RetentionDecisionReason::ActiveLegalHold);
    let legal_hold = personal.legal_hold().expect("active legal-hold evidence");
    assert_eq!(legal_hold.hold_id().as_str(), ACTIVE_HOLD_ID);
    assert_eq!(legal_hold.authority_reference().as_str(), "authority-1");
    assert_eq!(legal_hold.reason_code(), "LITIGATION_HOLD");
    assert_eq!(legal_hold.matching_hold_count(), 1);

    let financial = item(&first.decision, "resource-financial");
    assert_eq!(financial.approved_action(), PlannedPrivacyAction::Retain);
    assert_eq!(financial.final_action(), PlannedPrivacyAction::Retain);
    assert_eq!(
        financial.reason(),
        RetentionDecisionReason::MandatoryRetention
    );
    assert!(financial.legal_hold().is_none());

    let rebuildable = item(&first.decision, "resource-rebuildable");
    assert_eq!(rebuildable.approved_action(), PlannedPrivacyAction::Delete);
    assert_eq!(rebuildable.final_action(), PlannedPrivacyAction::Delete);
    assert_eq!(
        rebuildable.reason(),
        RetentionDecisionReason::ApprovedPrivacyAction
    );
    assert!(rebuildable.legal_hold().is_none());

    let replay = port
        .evaluate_and_persist(&invocation)
        .await
        .expect("replay exact retention evaluation");
    assert!(replay.replayed);
    assert_eq!(replay.decision, first.decision);
    assert_eq!(decision_count(&admin).await, before + 1);

    let mut malformed = legal_hold_persisted_payload(&hold(
        TENANT_A,
        MALFORMED_HOLD_ID,
        LegalHoldScope::AllCustomerData,
        2_500_000,
        None,
    ))
    .expect("encode malformed hold base");
    malformed.descriptor_hash = [99; 32];
    seed_record(
        &admin,
        TENANT_A,
        "customer-privacy.legal-hold",
        MALFORMED_HOLD_ID,
        1,
        malformed,
        "retention-fixture-malformed-hold",
    )
    .await;

    let malformed_invocation = self::invocation(
        plan.plan_id().clone(),
        5_000_000,
        "malformed",
        "customer_privacy.legal_hold.place",
    );
    let before_malformed = decision_count(&admin).await;
    let error = port
        .evaluate_and_persist(&malformed_invocation)
        .await
        .expect_err("malformed matching hold must fail closed");
    assert!(matches!(
        error.code.as_str(),
        "CUSTOMER_PRIVACY_RETENTION_EVIDENCE_INVALID"
            | "CUSTOMER_PRIVACY_PERSISTENCE_ADAPTER_INVALID"
    ));
    assert_eq!(decision_count(&admin).await, before_malformed);

    cleanup(&admin).await;
}

fn build_case_and_plan() -> (PrivacyCase, PrivacyActionPlan) {
    let tenant_id = TenantId::try_new(TENANT_A).unwrap();
    let canonical_party_id = RecordId::try_new(PARTY_ID).unwrap();
    let privacy_case_id = RecordId::try_new(CASE_ID).unwrap();
    let contract = OwnerScopeContract::new(
        ModuleId::try_new("crm.parties").unwrap(),
        CapabilityId::try_new("parties.privacy.scope.contribute").unwrap(),
        CapabilityVersion::try_new("1.0.0").unwrap(),
    );
    let registry = OwnerScopeRegistry::new(
        SchemaVersion::try_new("retention-registry/1").unwrap(),
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
        resource(
            "party.personal",
            "resource-personal",
            DataClass::Personal,
            EvidenceClass::DestroyableSubjectData,
        ),
        resource(
            "party.financial",
            "resource-financial",
            DataClass::Financial,
            EvidenceClass::ImmutableRequiredEvidence,
        ),
        resource(
            "party.rebuildable",
            "resource-rebuildable",
            DataClass::Internal,
            EvidenceClass::DerivedRebuildableState,
        ),
    ];
    let completeness = ContributionCompletenessProof::new(true, 1, 3, 3, [7; 32]).unwrap();
    let contribution = OwnerScopeContribution::new(
        contract,
        tenant_id.clone(),
        canonical_party_id.clone(),
        1,
        resources,
        completeness,
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
    (privacy_case, plan)
}

fn resource(
    resource_type: &str,
    resource_id: &str,
    data_class: DataClass,
    evidence_class: EvidenceClass,
) -> ScopeResource {
    ScopeResource::new(
        resource_type,
        RecordId::try_new(resource_id).unwrap(),
        1,
        data_class,
        evidence_class,
        RetentionPolicyId::try_new(format!("retention-{resource_id}")).unwrap(),
    )
    .unwrap()
}

fn hold(
    tenant: &str,
    hold_id: &str,
    scope: LegalHoldScope,
    effective_from: i64,
    effective_until: Option<i64>,
) -> CustomerDataLegalHold {
    CustomerDataLegalHold::place(
        RecordId::try_new(hold_id).unwrap(),
        TenantId::try_new(tenant).unwrap(),
        RecordId::try_new(PARTY_ID).unwrap(),
        scope,
        RecordId::try_new("authority-1").unwrap(),
        "LITIGATION_HOLD",
        SchemaVersion::try_new("privacy-policy/1").unwrap(),
        ActorId::try_new(ACTOR).unwrap(),
        effective_from,
        effective_until,
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
    evaluated_at_unix_nanos: i64,
    suffix: &str,
    initiating_capability_id: &str,
) -> RetentionEvaluationInvocation {
    RetentionEvaluationInvocation {
        tenant_id: TenantId::try_new(TENANT_A).unwrap(),
        privacy_case_id: RecordId::try_new(CASE_ID).unwrap(),
        action_plan_id,
        actor_id: ActorId::try_new(ACTOR).unwrap(),
        request_id: crm_module_sdk::RequestId::try_new(format!("retention-request-{suffix}"))
            .unwrap(),
        correlation_id: CorrelationId::try_new(format!("retention-correlation-{suffix}")).unwrap(),
        trace_id: TraceId::try_new(format!("retention-trace-{suffix}")).unwrap(),
        initiating_capability_id: CapabilityId::try_new(initiating_capability_id).unwrap(),
        initiating_capability_version: CapabilityVersion::try_new("1.0.0").unwrap(),
        request_started_at_unix_nanos: evaluated_at_unix_nanos - 1_000,
        evaluated_at_unix_nanos,
        trusted_internal: true,
    }
}

fn item<'a>(
    decision: &'a crm_customer_privacy_production::PrivacyRetentionDecisionSet,
    resource_id: &str,
) -> &'a crm_customer_privacy_production::PrivacyRetentionDecisionItem {
    decision
        .items()
        .iter()
        .find(|item| item.resource_id().as_str() == resource_id)
        .expect("retention decision item")
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
    let mut transaction = admin.begin().await.expect("begin retention fixture");
    sqlx::query("SET LOCAL session_replication_role = replica")
        .execute(&mut *transaction)
        .await
        .expect("disable trigger-backed fixture verification");
    insert_fixture_transaction(&mut transaction, tenant, transaction_id).await;
    let maximum_size = i64::try_from(payload.maximum_size_bytes).unwrap();
    let version = i64::try_from(version).unwrap();
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
    .bind(version)
    .bind(payload.owner.as_str())
    .bind(payload.schema_id.as_str())
    .bind(payload.schema_version.as_str())
    .bind(payload.descriptor_hash.as_slice())
    .bind(data_class_name(payload.data_class))
    .bind(maximum_size)
    .bind(payload.retention_policy_id.as_str())
    .bind(payload.bytes)
    .bind(transaction_id)
    .execute(&mut *transaction)
    .await
    .expect("insert retention fixture record");
    transaction
        .commit()
        .await
        .expect("commit retention fixture");
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
    .bind(if tenant == TENANT_A {
        "actor-a"
    } else {
        "actor-b"
    })
    .bind(format!("request-{transaction_id}"))
    .bind(format!("correlation-{transaction_id}"))
    .bind(format!("trace-{transaction_id}"))
    .execute(&mut **transaction)
    .await
    .expect("insert retention fixture business transaction");
}

async fn decision_count(admin: &PgPool) -> i64 {
    sqlx::query_scalar(
        "SELECT count(*) FROM crm.records WHERE tenant_id = $1 AND owner_module_id = 'crm.customer-privacy' AND record_type = 'customer-privacy.retention-decision' AND deleted_at IS NULL",
    )
    .bind(TENANT_A)
    .fetch_one(admin)
    .await
    .expect("count retention decisions")
}

async fn cleanup(admin: &PgPool) {
    let mut transaction = admin.begin().await.expect("begin retention cleanup");
    sqlx::query("SET LOCAL session_replication_role = replica")
        .execute(&mut *transaction)
        .await
        .expect("disable cleanup verification");
    sqlx::query(
        "DELETE FROM crm.records WHERE record_id = ANY($1) OR (tenant_id = $2 AND record_type = 'customer-privacy.retention-decision')",
    )
    .bind(&[
        CASE_ID,
        ACTIVE_HOLD_ID,
        FUTURE_HOLD_ID,
        CROSS_TENANT_HOLD_ID,
        MALFORMED_HOLD_ID,
    ][..])
    .bind(TENANT_A)
    .execute(&mut *transaction)
    .await
    .expect("delete retention records");
    sqlx::query(
        "DELETE FROM crm.business_transactions WHERE starts_with(business_transaction_id, 'retention-') OR starts_with(request_id, 'retention-request-')",
    )
    .execute(&mut *transaction)
    .await
    .expect("delete retention transactions");
    transaction
        .commit()
        .await
        .expect("commit retention cleanup");
}

fn data_class_name(data_class: DataClass) -> &'static str {
    match data_class {
        DataClass::Confidential => "confidential",
        DataClass::Personal => "personal",
        other => panic!("unsupported fixture data class: {other:?}"),
    }
}
