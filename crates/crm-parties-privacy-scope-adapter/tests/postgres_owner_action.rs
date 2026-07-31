use crm_capability_runtime::{CapabilityRequest, TransactionalCapabilityExecutor};
use crm_core_data::{PostgresDataStore, PostgresPrivacyOwnerActionExecutor};
use crm_customer_privacy::{
    ActionPlanningPolicy, ContributionCompletenessProof, DiscoveryOwnerScopeContribution,
    DiscoveryScopeSnapshot, EvidenceClass, OwnerScopeContract, OwnerScopeContribution,
    OwnerScopeRegistry, PrivacyActionPlan, PrivacyCaseKind, PrivacyOwnerActionAttempt,
    PrivacyOwnerActionCommand, PrivacyRetentionDecisionSet, ScopeDiscoveryLineage, ScopeResource,
    discovery_sha256, encode_owner_action_command,
};
use crm_customer_privacy_owner_scope_support::owner_action_input_payload;
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

const TENANT_A: &str = "tenant-parties-owner-action-a";
const TENANT_B: &str = "tenant-parties-owner-action-b";
const PARTY_ID: &str = "party-owner-action-postgres-1";
const ORIGINAL_NAME: &str = "Ada Owner Action";
const BASE_TIME_NANOS: i64 = 1_800_000_000_000_000_000;

#[tokio::test]
async fn postgres_owner_action_is_replay_safe_tenant_bound_and_fail_closed() {
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be configured");
    let admin_database_url =
        std::env::var("ADMIN_DATABASE_URL").expect("ADMIN_DATABASE_URL must be configured");
    let admin = PgPool::connect(&admin_database_url).await.unwrap();
    seed_owner_action_capability(&admin).await;
    seed_party(&admin).await;

    let application = PgPool::connect(&database_url).await.unwrap();
    let store = PostgresDataStore::from_pool(application);
    let definition = parties_privacy_action_definition().unwrap();
    let executor = PostgresPrivacyOwnerActionExecutor::new(
        store,
        Arc::new(parties_privacy_action_planner()),
    );

    let applied_attempt = attempt(
        TENANT_A,
        "applied",
        1,
        EvidenceClass::RetainMinimizedEvidence,
    );
    let applied_request = capability_request(&definition, &applied_attempt);
    let applied_transaction = applied_request
        .context
        .execution
        .business_transaction_id
        .as_str()
        .to_owned();
    let first = executor
        .execute(&definition, applied_request.clone())
        .await
        .unwrap();
    assert!(!first.replayed);
    assert_eq!(first.affected_resources.len(), 1);
    assert_eq!(first.affected_resources[0].version, Some(2));

    let replay = executor.execute(&definition, applied_request).await.unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.affected_resources[0].version, Some(2));

    let stored = sqlx::query(
        r#"
        SELECT version, payload_bytes, deleted_at IS NULL AS not_deleted,
               last_business_transaction_id
        FROM crm.records
        WHERE tenant_id = $1 AND record_type = $2 AND record_id = $3
        "#,
    )
    .bind(TENANT_A)
    .bind(RECORD_TYPE)
    .bind(PARTY_ID)
    .fetch_one(&admin)
    .await
    .unwrap();
    let version: i64 = stored.try_get("version").unwrap();
    let payload: Vec<u8> = stored.try_get("payload_bytes").unwrap();
    let not_deleted: bool = stored.try_get("not_deleted").unwrap();
    let last_transaction: String = stored.try_get("last_business_transaction_id").unwrap();
    let party = decode_party_state(&payload).unwrap();
    assert_eq!(version, 2);
    assert_eq!(party.version(), 2);
    assert!(party.display_name().starts_with("minimized person "));
    assert!(!party.display_name().contains(ORIGINAL_NAME));
    assert!(not_deleted);
    assert_eq!(last_transaction, applied_transaction);

    assert_evidence_counts(&admin, TENANT_A, &applied_transaction, 1, 1, 1, 1).await;

    let stale = attempt(
        TENANT_A,
        "stale",
        1,
        EvidenceClass::RetainMinimizedEvidence,
    );
    let stale_transaction = transaction_id(&stale);
    let error = executor
        .execute(&definition, capability_request(&definition, &stale))
        .await
        .unwrap_err();
    assert_eq!(error.code, "PRIVACY_OWNER_RECORD_STALE");
    assert_evidence_counts(&admin, TENANT_A, &stale_transaction, 0, 0, 0, 0).await;

    let cross_tenant = attempt(
        TENANT_B,
        "cross-tenant",
        1,
        EvidenceClass::RetainMinimizedEvidence,
    );
    let cross_transaction = transaction_id(&cross_tenant);
    let error = executor
        .execute(
            &definition,
            capability_request(&definition, &cross_tenant),
        )
        .await
        .unwrap_err();
    assert_eq!(error.code, "PRIVACY_OWNER_RECORD_NOT_FOUND");
    assert_evidence_counts(&admin, TENANT_B, &cross_transaction, 0, 0, 0, 0).await;

    let crypto = attempt(
        TENANT_A,
        "crypto-shred",
        2,
        EvidenceClass::CryptoShreddableData,
    );
    let crypto_transaction = transaction_id(&crypto);
    let error = executor
        .execute(&definition, capability_request(&definition, &crypto))
        .await
        .unwrap_err();
    assert_eq!(error.code, "PRIVACY_OWNER_CRYPTO_SHRED_UNAVAILABLE");
    assert_evidence_counts(&admin, TENANT_A, &crypto_transaction, 0, 0, 0, 0).await;

    let final_version: i64 = sqlx::query_scalar(
        "SELECT version FROM crm.records WHERE tenant_id = $1 AND record_type = $2 AND record_id = $3",
    )
    .bind(TENANT_A)
    .bind(RECORD_TYPE)
    .bind(PARTY_ID)
    .fetch_one(&admin)
    .await
    .unwrap();
    assert_eq!(final_version, 2);
}

fn attempt(
    tenant: &str,
    suffix: &str,
    resource_version: u64,
    evidence_class: EvidenceClass,
) -> PrivacyOwnerActionAttempt {
    let tenant_id = TenantId::try_new(tenant).unwrap();
    let privacy_case_id = RecordId::try_new(format!("privacy-case-{suffix}")).unwrap();
    let party_id = RecordId::try_new(PARTY_ID).unwrap();
    let contract = OwnerScopeContract::new(
        ModuleId::try_new("crm.parties").unwrap(),
        CapabilityId::try_new(SCOPE_CAPABILITY_ID).unwrap(),
        CapabilityVersion::try_new(SCOPE_CAPABILITY_VERSION).unwrap(),
    );
    let registry = OwnerScopeRegistry::new(
        SchemaVersion::try_new("owner-action-test-registry/1").unwrap(),
        [contract.clone()],
    )
    .unwrap();
    let lineage = ScopeDiscoveryLineage::new(
        privacy_case_id.clone(),
        tenant_id.clone(),
        party_id.clone(),
        1,
        registry.registry_version().clone(),
        *registry.digest(),
        "ERASURE_DISCOVERY".to_owned(),
        BASE_TIME_NANOS / 1_000_000,
    )
    .unwrap();
    let resource = ScopeResource::new(
        RECORD_TYPE.to_owned(),
        party_id.clone(),
        resource_version,
        DataClass::Personal,
        evidence_class,
        RetentionPolicyId::try_new("crm.parties.business_record").unwrap(),
    )
    .unwrap();
    let contribution = OwnerScopeContribution::new(
        contract,
        tenant_id.clone(),
        party_id,
        1,
        vec![resource],
        ContributionCompletenessProof::new(true, 1, 1, 1, [0x71; 32]).unwrap(),
    )
    .unwrap();
    let discovery_contribution =
        DiscoveryOwnerScopeContribution::new(lineage.clone(), contribution).unwrap();
    let snapshot = DiscoveryScopeSnapshot::finalize(
        lineage,
        registry,
        BASE_TIME_NANOS,
        [discovery_contribution],
    )
    .unwrap();
    let plan = PrivacyActionPlan::build(
        &snapshot,
        1,
        PrivacyCaseKind::Erasure,
        ActionPlanningPolicy::new(
            SchemaVersion::try_new("owner-action-test-policy/1").unwrap(),
            "GLOBAL".to_owned(),
            false,
            false,
        )
        .unwrap(),
        BASE_TIME_NANOS + 1,
    )
    .unwrap();
    let decision = PrivacyRetentionDecisionSet::build(&plan, &[], BASE_TIME_NANOS + 2).unwrap();
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
    .unwrap()
}

fn capability_request(
    definition: &crm_capability_runtime::CapabilityDefinition,
    attempt: &PrivacyOwnerActionAttempt,
) -> CapabilityRequest {
    let command = PrivacyOwnerActionCommand::from_attempt(attempt).unwrap();
    let input = owner_action_input_payload(encode_owner_action_command(&command).unwrap()).unwrap();
    let transaction = transaction_id(attempt);
    CapabilityRequest {
        context: ModuleExecutionContext {
            module_id: definition.owner_module_id.clone(),
            execution: ExecutionContext {
                tenant_id: attempt.tenant_id().clone(),
                actor_id: ActorId::try_new("privacy-owner-action-test").unwrap(),
                request_id: RequestId::try_new(format!("request-{}", attempt.attempt_id())).unwrap(),
                correlation_id: CorrelationId::try_new("privacy-owner-action-correlation").unwrap(),
                causation_id: CausationId::try_new(format!("cause-{}", attempt.attempt_id())).unwrap(),
                trace_id: TraceId::try_new("privacy-owner-action-trace").unwrap(),
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

fn transaction_id(attempt: &PrivacyOwnerActionAttempt) -> String {
    format!("privacy-owner-test-{}", attempt.attempt_id())
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
        )
        VALUES ($1, '1.0.0', 'crm.parties', '0.3.0',
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

async fn seed_party(admin: &PgPool) {
    let contract = persisted_contract();
    let party = Party::create(CreateParty {
        party_id: PartyId::try_new(PARTY_ID).unwrap(),
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
        ON CONFLICT (tenant_id, business_transaction_id) DO NOTHING
        "#,
    )
    .bind(TENANT_A)
    .bind("fixture-parties-owner-action")
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
        )
        VALUES ($1, $2, $3, 'crm.parties', 1, $4, $5, $6, 'personal', 'json',
                $7, $8, $9, 'fixture-parties-owner-action', clock_timestamp(),
                clock_timestamp(), NULL)
        ON CONFLICT (tenant_id, record_type, record_id) DO UPDATE
        SET version = EXCLUDED.version,
            schema_id = EXCLUDED.schema_id,
            schema_version = EXCLUDED.schema_version,
            descriptor_hash = EXCLUDED.descriptor_hash,
            data_class = EXCLUDED.data_class,
            payload_encoding = EXCLUDED.payload_encoding,
            maximum_payload_size = EXCLUDED.maximum_payload_size,
            retention_policy_id = EXCLUDED.retention_policy_id,
            payload_bytes = EXCLUDED.payload_bytes,
            last_business_transaction_id = EXCLUDED.last_business_transaction_id,
            deleted_at = NULL
        "#,
    )
    .bind(TENANT_A)
    .bind(RECORD_TYPE)
    .bind(PARTY_ID)
    .bind(contract.schema_id)
    .bind(contract.schema_version)
    .bind(contract.descriptor_hash.as_slice())
    .bind(i64::try_from(contract.maximum_size_bytes).unwrap())
    .bind(contract.retention_policy_id)
    .bind(payload)
    .execute(&mut *transaction)
    .await
    .unwrap();
    transaction.commit().await.unwrap();
}

async fn assert_evidence_counts(
    admin: &PgPool,
    tenant: &str,
    business_transaction_id: &str,
    transactions: i64,
    outbox: i64,
    audits: i64,
    idempotency: i64,
) {
    let transaction_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM crm.business_transactions WHERE tenant_id = $1 AND business_transaction_id = $2",
    )
    .bind(tenant)
    .bind(business_transaction_id)
    .fetch_one(admin)
    .await
    .unwrap();
    let outbox_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM crm.outbox_events WHERE tenant_id = $1 AND business_transaction_id = $2",
    )
    .bind(tenant)
    .bind(business_transaction_id)
    .fetch_one(admin)
    .await
    .unwrap();
    let audit_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM crm.audit_records WHERE tenant_id = $1 AND business_transaction_id = $2",
    )
    .bind(tenant)
    .bind(business_transaction_id)
    .fetch_one(admin)
    .await
    .unwrap();
    let idempotency_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM crm.idempotency_records WHERE tenant_id = $1 AND business_transaction_id = $2",
    )
    .bind(tenant)
    .bind(business_transaction_id)
    .fetch_one(admin)
    .await
    .unwrap();
    assert_eq!(transaction_count, transactions);
    assert_eq!(outbox_count, outbox);
    assert_eq!(audit_count, audits);
    assert_eq!(idempotency_count, idempotency);
}
