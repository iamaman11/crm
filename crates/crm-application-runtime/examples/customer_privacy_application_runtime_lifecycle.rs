use crm_application_composition::ModuleActivationPort;
use crm_capability_ingress::semantic_input_hash;
use crm_capability_plan_support as capability_support;
use crm_capability_runtime::{CapabilityDefinition, CapabilityRequest};
use crm_core_data::PostgresDataStore;
use crm_customer_privacy_production::{
    ACTION_PLAN_RECORD_TYPE, ACTION_PLAN_STATE_MAXIMUM_BYTES,
    ACTION_PLAN_STATE_RETENTION_POLICY_ID, ACTION_PLAN_STATE_SCHEMA_ID,
    ACTION_PLAN_STATE_SCHEMA_VERSION, ActionPlanningPolicy, ContributionCompletenessProof,
    DiscoveryOwnerScopeContribution, DiscoveryScopeSnapshot, EvidenceClass, OwnerScopeContract,
    OwnerScopeContribution, OwnerScopeRegistry, PrivacyActionPlan, PrivacyCase, PrivacyCaseKind,
    PrivacyRetentionDecisionSet, ScopeDiscoveryLineage, ScopeResource, SubjectVerificationMethod,
    action_plan_state_descriptor_hash, encode_action_plan_state, privacy_case_persisted_payload,
    retention_decision_persisted_payload,
};
use crm_module_sdk::{
    ActorId, BusinessTransactionId, CapabilityId, CapabilityVersion, CausationId, CorrelationId,
    DataClass, ExecutionContext, IdempotencyKey, ModuleExecutionContext, ModuleId, PayloadEncoding,
    PortFuture, RecordId, RecordRef, RequestId, RetentionPolicyId, SchemaId, SchemaVersion, SdkError,
    TenantId, TraceId, TypedPayload,
};
use crm_party_reference_composition::{
    PartiesProductionDependencies, build_contribution, parties_runtime_identity,
};
use crm_proto_contracts::crm::{customer::v1 as customer_wire, parties::v1 as party_wire};
use crm_query_runtime::{QueryRequest, QueryVisibilityAuthorizer, QueryVisibilityDecision};
use serde_json::Value;
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Postgres, Row, Transaction};
use std::collections::BTreeSet;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::time::sleep;

const ORIGINAL_NAME: &str = "Application Runtime Privacy Subject";
const CUSTOMER_PRIVACY_MODULE_ID: &str = "crm.customer-privacy";
const OWNER_ACTION_COMMAND_DESCRIPTOR: &[u8] = b"crm.customer-privacy.owner_action.command/v1:tenant_id,privacy_case_id,action_plan_id,action_plan_digest,retention_decision_id,retention_decision_digest,attempt_id,attempt_digest,item_sequence,attempt_generation,item_digest,owner_module_id,owner_capability_id,owner_capability_version,target_idempotency_key,resource_type,resource_id,resource_version_decimal_string,action_code,planned_at_unix_nanos_decimal_string";
const CAPTURED_AT: i64 = 8_000_000;
const PLANNED_AT: i64 = 9_000_000;
const DECIDED_AT: i64 = 10_000_000;

#[tokio::main(flavor = "multi_thread", worker_threads = 2)]
async fn main() {
    let binary = std::env::var("CRM_API_BINARY")
        .expect("CRM_API_BINARY must name the assembled crm-api binary");
    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must support assembled runtime lifecycle acceptance");
    let admin_database_url = std::env::var("ADMIN_DATABASE_URL")
        .expect("ADMIN_DATABASE_URL must accompany DATABASE_URL");
    let admin = PgPool::connect(&admin_database_url)
        .await
        .expect("connect assembled runtime lifecycle admin pool");
    let store = PostgresDataStore::connect(&database_url, 6)
        .await
        .expect("connect assembled runtime lifecycle Party store");
    let run_id = unique_id();
    let tenant = format!("tenant-runtime-privacy-{run_id}");
    let actor = format!("actor-runtime-privacy-{run_id}");
    let first_case = format!("privacy-case-runtime-first-{run_id}");
    let first_party = format!("party-runtime-first-{run_id}");
    let second_case = format!("privacy-case-runtime-disabled-{run_id}");
    let second_party = format!("party-runtime-disabled-{run_id}");

    seed_tenant_and_actor(&admin, &tenant, &actor).await;
    seed_owner_action_capability(&admin).await;
    let first_planned_version = seed_bundle(
        &admin,
        &store,
        &tenant,
        &actor,
        &first_case,
        &first_party,
        "first",
    )
    .await;

    let mut first_process = spawn_crm_api(&binary, &database_url, &tenant, &actor, true);
    wait_until_ready(&mut first_process).await;
    wait_for_completed_lifecycle(
        &admin,
        &mut first_process,
        &tenant,
        &first_case,
        &first_party,
        first_planned_version,
    )
    .await;
    first_process.stop();

    let completed = execution_snapshot(&admin, &tenant, &first_case, &first_party).await;
    assert_eq!(completed.checkpoints, 1);
    assert_eq!(completed.attempts, 1);
    assert_eq!(completed.outcomes, 1);
    assert!(completed.audit >= 3);
    assert_eq!(completed.owner_events, 1);
    assert_eq!(completed.party_version, 2);
    assert!(completed.case_version > i64::try_from(first_planned_version).unwrap());
    assert_completed_evidence(&admin, &tenant, &first_case, &first_party).await;

    let mut replay_process = spawn_crm_api(&binary, &database_url, &tenant, &actor, false);
    wait_until_ready(&mut replay_process).await;
    sleep(Duration::from_millis(2_200)).await;
    replay_process.assert_running();
    replay_process.stop();
    assert_eq!(
        execution_snapshot(&admin, &tenant, &first_case, &first_party).await,
        completed,
        "runtime restart must replay without duplicate authoritative effects"
    );

    let second_planned_version = seed_bundle(
        &admin,
        &store,
        &tenant,
        &actor,
        &second_case,
        &second_party,
        "disabled",
    )
    .await;
    uninstall_customer_privacy(&admin, &tenant).await;
    let disabled_before = execution_snapshot(&admin, &tenant, &second_case, &second_party).await;
    assert_eq!(disabled_before.checkpoints, 0);
    assert_eq!(disabled_before.attempts, 0);
    assert_eq!(disabled_before.outcomes, 0);
    assert_eq!(disabled_before.audit, 0);
    assert_eq!(disabled_before.owner_events, 0);
    assert_eq!(disabled_before.party_version, 1);
    assert_eq!(
        disabled_before.case_version,
        i64::try_from(second_planned_version).unwrap()
    );

    let mut disabled_process = spawn_crm_api(&binary, &database_url, &tenant, &actor, false);
    wait_until_ready(&mut disabled_process).await;
    sleep(Duration::from_millis(2_200)).await;
    disabled_process.assert_running();
    disabled_process.stop();
    assert_eq!(
        execution_snapshot(&admin, &tenant, &second_case, &second_party).await,
        disabled_before,
        "uninstalled Customer Privacy must prevent discovery and owner effects"
    );
    assert_party_original(&admin, &tenant, &second_party).await;
}

#[derive(Debug, Clone, Copy)]
struct AlwaysActiveModule;

impl ModuleActivationPort for AlwaysActiveModule {
    fn is_active<'a>(
        &'a self,
        _tenant_id: &'a TenantId,
        _module_id: &'a ModuleId,
    ) -> PortFuture<'a, Result<bool, SdkError>> {
        Box::pin(async { Ok(true) })
    }
}

#[derive(Debug, Clone, Copy)]
struct AllowAllVisibility;

impl QueryVisibilityAuthorizer for AllowAllVisibility {
    fn authorize_visibility<'a>(
        &'a self,
        _request: &'a QueryRequest,
        _resource: &'a RecordRef,
    ) -> PortFuture<'a, Result<QueryVisibilityDecision, SdkError>> {
        Box::pin(async {
            Ok(QueryVisibilityDecision {
                resource_visible: true,
                allowed_fields: BTreeSet::new(),
                decision_id: "step22e-parties-lifecycle-fixture".to_owned(),
                policy_version: "step22e-parties-lifecycle-fixture/v1".to_owned(),
            })
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ExecutionSnapshot {
    checkpoints: i64,
    attempts: i64,
    outcomes: i64,
    audit: i64,
    owner_events: i64,
    party_version: i64,
    case_version: i64,
}

struct CrmApiProcess {
    child: Child,
    http_addr: String,
    stopped: bool,
}

impl CrmApiProcess {
    fn assert_running(&mut self) {
        assert!(
            self.child
                .try_wait()
                .expect("poll crm-api process")
                .is_none(),
            "crm-api exited before lifecycle acceptance completed"
        );
    }

    fn stop(&mut self) {
        if self.stopped {
            return;
        }
        self.assert_running();
        let status = Command::new("kill")
            .arg("-INT")
            .arg(self.child.id().to_string())
            .status()
            .expect("send SIGINT to crm-api");
        assert!(status.success(), "kill -INT failed: {status}");
        let exit = self.child.wait().expect("wait for crm-api process");
        assert!(exit.success(), "crm-api exited unsuccessfully: {exit}");
        self.stopped = true;
    }
}

impl Drop for CrmApiProcess {
    fn drop(&mut self) {
        if !self.stopped {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

fn spawn_crm_api(
    binary: &str,
    database_url: &str,
    tenant: &str,
    actor: &str,
    bootstrap: bool,
) -> CrmApiProcess {
    let http_addr = format!("127.0.0.1:{}", free_port());
    let grpc_addr = format!("127.0.0.1:{}", free_port());
    let child = Command::new(binary)
        .env("CRM_DATABASE_URL", database_url)
        .env("CRM_HTTP_BIND", &http_addr)
        .env("CRM_GRPC_BIND", grpc_addr)
        .env("CRM_API_BEARER_TOKEN", "runtime-lifecycle-token")
        .env("CRM_API_ACTOR_ID", actor)
        .env("CRM_API_TENANTS", tenant)
        .env(
            "CRM_CURSOR_SIGNING_KEY",
            "runtime-lifecycle-cursor-signing-key-0123456789abcdef",
        )
        .env(
            "CRM_APPROVAL_SIGNING_KEY",
            "runtime-lifecycle-approval-signing-key-0123456789abcdef",
        )
        .env(
            "CRM_BOOTSTRAP_ALLOW_PHASE6",
            if bootstrap { "true" } else { "false" },
        )
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("spawn assembled crm-api runtime");
    CrmApiProcess {
        child,
        http_addr,
        stopped: false,
    }
}

async fn wait_until_ready(process: &mut CrmApiProcess) {
    let deadline = Instant::now() + Duration::from_secs(45);
    loop {
        process.assert_running();
        if ready_response(&process.http_addr) {
            return;
        }
        assert!(Instant::now() < deadline, "crm-api readiness timed out");
        sleep(Duration::from_millis(200)).await;
    }
}

fn ready_response(http_addr: &str) -> bool {
    let Ok(mut stream) = TcpStream::connect(http_addr) else {
        return false;
    };
    let timeout = Some(Duration::from_millis(500));
    if stream.set_read_timeout(timeout).is_err() || stream.set_write_timeout(timeout).is_err() {
        return false;
    }
    if stream
        .write_all(
            format!("GET /readyz HTTP/1.1\r\nHost: {http_addr}\r\nConnection: close\r\n\r\n")
                .as_bytes(),
        )
        .is_err()
    {
        return false;
    }
    let mut response = String::new();
    stream.read_to_string(&mut response).is_ok() && response.starts_with("HTTP/1.1 200")
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("bind ephemeral lifecycle port")
        .local_addr()
        .expect("read lifecycle port")
        .port()
}

async fn wait_for_completed_lifecycle(
    admin: &PgPool,
    process: &mut CrmApiProcess,
    tenant: &str,
    case_id: &str,
    party_id: &str,
    planned_case_version: u64,
) {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        process.assert_running();
        let snapshot = execution_snapshot(admin, tenant, case_id, party_id).await;
        if snapshot.checkpoints == 1
            && snapshot.attempts == 1
            && snapshot.outcomes == 1
            && snapshot.owner_events == 1
            && snapshot.party_version == 2
            && snapshot.case_version > i64::try_from(planned_case_version).unwrap()
        {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "assembled runtime did not complete owner lifecycle: {snapshot:?}"
        );
        sleep(Duration::from_millis(200)).await;
    }
}

async fn execution_snapshot(
    admin: &PgPool,
    tenant: &str,
    case_id: &str,
    party_id: &str,
) -> ExecutionSnapshot {
    let party_record_type = party_record_type();
    ExecutionSnapshot {
        checkpoints: sqlx::query_scalar(
            "SELECT count(*) FROM crm.customer_privacy_owner_execution_checkpoints WHERE tenant_id = $1 AND privacy_case_id = $2",
        )
        .bind(tenant)
        .bind(case_id)
        .fetch_one(admin)
        .await
        .expect("count runtime owner-execution checkpoints"),
        attempts: sqlx::query_scalar(
            "SELECT count(*) FROM crm.customer_privacy_owner_action_attempts WHERE tenant_id = $1 AND privacy_case_id = $2",
        )
        .bind(tenant)
        .bind(case_id)
        .fetch_one(admin)
        .await
        .expect("count runtime owner-action attempts"),
        outcomes: sqlx::query_scalar(
            "SELECT count(*) FROM crm.customer_privacy_owner_action_outcomes WHERE tenant_id = $1 AND privacy_case_id = $2",
        )
        .bind(tenant)
        .bind(case_id)
        .fetch_one(admin)
        .await
        .expect("count runtime owner-action outcomes"),
        audit: sqlx::query_scalar(
            "SELECT count(*) FROM crm.customer_privacy_owner_execution_audit WHERE tenant_id = $1 AND privacy_case_id = $2",
        )
        .bind(tenant)
        .bind(case_id)
        .fetch_one(admin)
        .await
        .expect("count runtime owner-execution audit rows"),
        owner_events: sqlx::query_scalar(
            r#"
            SELECT count(*) FROM crm.outbox_events
            WHERE tenant_id = $1
              AND event_type = 'parties.privacy.action.apply.completed'
              AND aggregate_type = $2 AND aggregate_id = $3
            "#,
        )
        .bind(tenant)
        .bind(party_record_type)
        .bind(party_id)
        .fetch_one(admin)
        .await
        .expect("count runtime owner events"),
        party_version: record_version(admin, tenant, party_record_type, party_id).await,
        case_version: record_version(admin, tenant, "customer-privacy.case", case_id).await,
    }
}

async fn record_version(admin: &PgPool, tenant: &str, record_type: &str, record_id: &str) -> i64 {
    sqlx::query_scalar(
        "SELECT version FROM crm.records WHERE tenant_id = $1 AND record_type = $2 AND record_id = $3",
    )
    .bind(tenant)
    .bind(record_type)
    .bind(record_id)
    .fetch_one(admin)
    .await
    .expect("load runtime lifecycle record version")
}

async fn assert_completed_evidence(admin: &PgPool, tenant: &str, case_id: &str, party_id: &str) {
    let outcome_status: String = sqlx::query_scalar(
        "SELECT status FROM crm.customer_privacy_owner_action_outcomes WHERE tenant_id = $1 AND privacy_case_id = $2",
    )
    .bind(tenant)
    .bind(case_id)
    .fetch_one(admin)
    .await
    .expect("load successful runtime owner outcome");
    assert_eq!(outcome_status, "succeeded");

    let checkpoint = sqlx::query(
        r#"
        SELECT total_items, next_sequence, completed_at_unix_nanos,
               converging_case_version
        FROM crm.customer_privacy_owner_execution_checkpoints
        WHERE tenant_id = $1 AND privacy_case_id = $2
        "#,
    )
    .bind(tenant)
    .bind(case_id)
    .fetch_one(admin)
    .await
    .expect("load completed runtime checkpoint");
    assert_eq!(checkpoint.get::<i32, _>("total_items"), 1);
    assert_eq!(checkpoint.get::<i32, _>("next_sequence"), 2);
    assert!(
        checkpoint
            .try_get::<Option<i64>, _>("completed_at_unix_nanos")
            .unwrap()
            .is_some()
    );
    let converging_version = checkpoint
        .try_get::<Option<i64>, _>("converging_case_version")
        .unwrap()
        .expect("runtime checkpoint must bind the converging case version");
    assert_eq!(
        converging_version,
        record_version(admin, tenant, "customer-privacy.case", case_id).await
    );

    let row = sqlx::query(
        "SELECT version, payload_bytes FROM crm.records WHERE tenant_id = $1 AND record_type = $2 AND record_id = $3",
    )
    .bind(tenant)
    .bind(party_record_type())
    .bind(party_id)
    .fetch_one(admin)
    .await
    .expect("load minimized runtime Party");
    assert_eq!(row.get::<i64, _>("version"), 2);
    let state: Value = serde_json::from_slice(&row.get::<Vec<u8>, _>("payload_bytes"))
        .expect("decode minimized runtime Party");
    let display_name = state["display_name"]
        .as_str()
        .expect("minimized Party display name");
    assert!(display_name.starts_with("minimized person "));
    assert!(!display_name.contains(ORIGINAL_NAME));
}

async fn assert_party_original(admin: &PgPool, tenant: &str, party_id: &str) {
    let row = sqlx::query(
        "SELECT version, payload_bytes FROM crm.records WHERE tenant_id = $1 AND record_type = $2 AND record_id = $3",
    )
    .bind(tenant)
    .bind(party_record_type())
    .bind(party_id)
    .fetch_one(admin)
    .await
    .expect("load disabled runtime Party");
    assert_eq!(row.get::<i64, _>("version"), 1);
    let state: Value = serde_json::from_slice(&row.get::<Vec<u8>, _>("payload_bytes"))
        .expect("decode disabled runtime Party");
    assert_eq!(state["display_name"], ORIGINAL_NAME);
}

async fn seed_bundle(
    admin: &PgPool,
    store: &PostgresDataStore,
    tenant: &str,
    actor: &str,
    case_id: &str,
    party_id: &str,
    suffix: &str,
) -> u64 {
    let (privacy_case, plan, decision) =
        build_case_plan_and_decision(tenant, actor, case_id, party_id);
    seed_record(
        admin,
        tenant,
        actor,
        (
            format!("runtime-case-{suffix}-{case_id}"),
            "test.record.mutate",
        ),
        ("customer-privacy.case", case_id, privacy_case.version()),
        privacy_case_persisted_payload(&privacy_case).expect("encode runtime privacy case"),
    )
    .await;
    seed_record(
        admin,
        tenant,
        actor,
        (
            format!("runtime-plan-{suffix}-{case_id}"),
            "test.record.mutate",
        ),
        (ACTION_PLAN_RECORD_TYPE, plan.plan_id().as_str(), 1),
        action_plan_payload(&plan),
    )
    .await;
    seed_party(store, tenant, actor, party_id, suffix).await;
    seed_record(
        admin,
        tenant,
        actor,
        (
            format!("runtime-decision-{suffix}-{case_id}"),
            "customer_privacy.case.approve",
        ),
        (
            "customer-privacy.retention-decision",
            decision.decision_id().as_str(),
            1,
        ),
        retention_decision_persisted_payload(&decision).expect("encode runtime retention decision"),
    )
    .await;
    privacy_case.version()
}

async fn seed_party(
    store: &PostgresDataStore,
    tenant: &str,
    actor: &str,
    party_id: &str,
    suffix: &str,
) {
    let composition = build_contribution(PartiesProductionDependencies {
        store: store.clone(),
        activation: Arc::new(AlwaysActiveModule),
        visibility_authorizer: Arc::new(AllowAllVisibility),
        cursor_key: [0x50; 32],
    })
    .expect("build Parties lifecycle contribution")
    .build()
    .expect("assemble Parties lifecycle contribution");
    let (parties_module_id, _, create_capability, _) = parties_runtime_identity();
    let definition = composition
        .mutation_definitions()
        .iter()
        .find(|definition| {
            definition.owner_module_id.as_str() == parties_module_id
                && definition.capability_id.as_str() == create_capability
        })
        .expect("Parties create definition in lifecycle contribution");
    let command = party_wire::CreatePartyRequest {
        party_ref: Some(customer_wire::PartyRef {
            party_id: party_id.to_owned(),
        }),
        kind: party_wire::PartyKind::Person as i32,
        display_name: ORIGINAL_NAME.to_owned(),
    };
    let input = capability_support::protobuf_payload(
        parties_module_id,
        definition.input_contract.schema_id.as_str(),
        DataClass::Personal,
        &command,
    )
    .expect("encode lifecycle Party seed request");
    let identity = format!("runtime-party-seed-{suffix}");
    let request = CapabilityRequest {
        context: ModuleExecutionContext {
            module_id: ModuleId::try_new(parties_module_id).expect("Parties module id"),
            execution: ExecutionContext {
                tenant_id: TenantId::try_new(tenant).expect("lifecycle tenant id"),
                actor_id: ActorId::try_new(actor).expect("lifecycle actor id"),
                request_id: RequestId::try_new(format!("{identity}-request"))
                    .expect("Party seed request id"),
                correlation_id: CorrelationId::try_new(format!("{identity}-correlation"))
                    .expect("Party seed correlation id"),
                causation_id: CausationId::try_new(format!("{identity}-causation"))
                    .expect("Party seed causation id"),
                trace_id: TraceId::try_new(format!("{identity}-trace"))
                    .expect("Party seed trace id"),
                capability_id: definition.capability_id.clone(),
                capability_version: definition.capability_version.clone(),
                idempotency_key: IdempotencyKey::try_new(identity.clone())
                    .expect("Party seed idempotency key"),
                business_transaction_id: BusinessTransactionId::try_new(format!(
                    "{identity}-transaction"
                ))
                .expect("Party seed business transaction id"),
                schema_version: SchemaVersion::try_new("1.0.0")
                    .expect("Party seed schema version"),
                request_started_at_unix_nanos: CAPTURED_AT - 1_000_000,
            },
        },
        input_hash: semantic_input_hash(&input),
        input,
        approval: None,
    };
    composition
        .mutation_executor()
        .execute(definition, request)
        .await
        .expect("seed lifecycle Party through owner contribution");
}

fn party_record_type() -> &'static str {
    let (_, record_type, _, _) = parties_runtime_identity();
    record_type
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
        SchemaVersion::try_new("runtime-lifecycle-registry/1").unwrap(),
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
            party_record_type(),
            canonical_party_id.clone(),
            1,
            DataClass::Personal,
            EvidenceClass::RetainMinimizedEvidence,
            RetentionPolicyId::try_new("crm.parties.business_record").unwrap(),
        )
        .unwrap()],
        ContributionCompletenessProof::new(true, 1, 1, 1, [0x79; 32]).unwrap(),
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
        owner: ModuleId::try_new(CUSTOMER_PRIVACY_MODULE_ID).unwrap(),
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

async fn seed_tenant_and_actor(admin: &PgPool, tenant: &str, actor: &str) {
    sqlx::query(
        "INSERT INTO crm.tenants (tenant_id, status, data_region) VALUES ($1, 'active', 'eu-central') ON CONFLICT (tenant_id) DO NOTHING",
    )
    .bind(tenant)
    .execute(admin)
    .await
    .expect("seed runtime lifecycle tenant");
    let mut transaction = admin.begin().await.expect("begin runtime actor fixture");
    sqlx::query("SET LOCAL session_replication_role = replica")
        .execute(&mut *transaction)
        .await
        .expect("disable runtime actor fixture verification");
    sqlx::query(
        r#"
        INSERT INTO crm.actors (
          tenant_id, actor_id, actor_type, status, display_name,
          version, last_business_transaction_id
        ) VALUES ($1, $2, 'service', 'active', 'Runtime Privacy Worker', 1, $3)
        ON CONFLICT (tenant_id, actor_id) DO NOTHING
        "#,
    )
    .bind(tenant)
    .bind(actor)
    .bind(format!("runtime-actor-{tenant}"))
    .execute(&mut *transaction)
    .await
    .expect("seed runtime lifecycle actor");
    transaction
        .commit()
        .await
        .expect("commit runtime actor fixture");
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
    .expect("seed runtime owner-action capability");
}

async fn seed_record(
    admin: &PgPool,
    tenant: &str,
    actor: &str,
    transaction_identity: (String, &str),
    record_identity: (&str, &str, u64),
    payload: TypedPayload,
) {
    let (transaction_id, capability_id) = transaction_identity;
    let (record_type, record_id, version) = record_identity;
    let mut transaction = admin.begin().await.expect("begin runtime record fixture");
    sqlx::query("SET LOCAL session_replication_role = replica")
        .execute(&mut *transaction)
        .await
        .expect("disable runtime record fixture verification");
    insert_fixture_transaction(
        &mut transaction,
        tenant,
        actor,
        &transaction_id,
        capability_id,
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
    .bind(&transaction_id)
    .execute(&mut *transaction)
    .await
    .expect("insert runtime lifecycle record");
    transaction
        .commit()
        .await
        .expect("commit runtime record fixture");
}

async fn insert_fixture_transaction(
    transaction: &mut Transaction<'_, Postgres>,
    tenant: &str,
    actor: &str,
    transaction_id: &str,
    capability_id: &str,
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
    .bind(tenant)
    .bind(transaction_id)
    .bind(actor)
    .bind(format!("request-{transaction_id}"))
    .bind(format!("correlation-{transaction_id}"))
    .bind(format!("trace-{transaction_id}"))
    .bind(capability_id)
    .execute(&mut **transaction)
    .await
    .expect("insert runtime lifecycle business transaction");
}

async fn uninstall_customer_privacy(admin: &PgPool, tenant: &str) {
    let mut transaction = admin
        .begin()
        .await
        .expect("begin runtime uninstall fixture");
    sqlx::query("SET LOCAL session_replication_role = replica")
        .execute(&mut *transaction)
        .await
        .expect("disable runtime uninstall verification");
    let result =
        sqlx::query("DELETE FROM crm.module_installations WHERE tenant_id = $1 AND module_id = $2")
            .bind(tenant)
            .bind(CUSTOMER_PRIVACY_MODULE_ID)
            .execute(&mut *transaction)
            .await
            .expect("remove Customer Privacy installation");
    assert_eq!(result.rows_affected(), 1);
    transaction
        .commit()
        .await
        .expect("commit runtime uninstall fixture");
}

fn data_class_name(data_class: DataClass) -> &'static str {
    match data_class {
        DataClass::Confidential => "confidential",
        DataClass::Personal => "personal",
        other => panic!("unsupported runtime fixture data class: {other:?}"),
    }
}

fn unique_id() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock after Unix epoch")
        .as_nanos()
}
