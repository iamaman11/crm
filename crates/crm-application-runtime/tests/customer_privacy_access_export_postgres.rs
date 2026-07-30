use crm_core_data::PostgresDataStore;
use crm_core_files::{
    AppendImmutableFileChunk, CreateImmutableFileArtifact, FileArtifactAppendResult,
    FileArtifactMetadata, FileArtifactStatus, FinalizedFileArtifact, ImmutableFileArtifactStore,
};
use crm_customer_data_operations_execution_composition::{
    PrivacyManifestExportPublisher, PrivacyManifestExportRequest,
};
use crm_customer_privacy_application::{
    ACCESS_EXPORT_CAPABILITY_VERSION, ACCESS_EXPORT_REQUEST_CAPABILITY, AccessExportInvocation,
    AccessExportPersistencePort, AccessExportPreparation,
};
use crm_customer_privacy_production::{
    ACTION_PLAN_RECORD_TYPE, ACTION_PLAN_STATE_MAXIMUM_BYTES,
    ACTION_PLAN_STATE_RETENTION_POLICY_ID, ACTION_PLAN_STATE_SCHEMA_ID,
    ACTION_PLAN_STATE_SCHEMA_VERSION, ActionPlanningPolicy, ContributionCompletenessProof,
    DiscoveryOwnerScopeContribution, DiscoveryScopeSnapshot, EvidenceClass, ExecutionPreparation,
    OwnerExecutionInvocation, OwnerExecutionPersistencePort, OwnerScopeContract,
    OwnerScopeContribution, OwnerScopeRegistry, PostgresAccessExportPersistence,
    PostgresOwnerExecutionPersistence, PrivacyActionPlan, PrivacyCase, PrivacyCaseKind,
    PrivacyRetentionDecisionSet, ScopeDiscoveryLineage, ScopeResource, SubjectVerificationMethod,
    action_plan_state_descriptor_hash, encode_access_export_manifest, encode_action_plan_state,
    privacy_case_persisted_payload, retention_decision_persisted_payload,
};
use crm_module_sdk::{
    ActorId, CapabilityId, CapabilityVersion, CorrelationId, DataClass, FileId,
    ModuleExecutionContext, ModuleId, PayloadEncoding, PortFuture, RecordId, RetentionPolicyId,
    SchemaId, SchemaVersion, SdkError, TenantId, TraceId, TypedPayload,
};
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Postgres, Transaction};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

const TENANT_A: &str = "tenant-access-export-a";
const TENANT_B: &str = "tenant-access-export-b";
const ACTOR: &str = "actor-access-export";
const CASE_ID: &str = "access-export-case-a";
const PARTY_ID: &str = "access-export-party-a";
const CAPTURED_AT: i64 = 2_000_000;
const PLANNED_AT: i64 = 3_000_000;
const DECIDED_AT: i64 = 4_000_000;
const EXECUTED_AT: i64 = 5_000_000;
const PREPARED_AT: i64 = 6_000_000;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn repository_step_10_access_export_recovers_finalized_artifact_before_case_link() {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!("skipping access-export PostgreSQL test because DATABASE_URL is absent");
        return;
    };
    let admin_database_url = std::env::var("ADMIN_DATABASE_URL")
        .expect("ADMIN_DATABASE_URL must accompany DATABASE_URL");
    let admin = PgPool::connect(&admin_database_url)
        .await
        .expect("connect access-export admin pool");
    cleanup(&admin).await;

    let (privacy_case, plan, decision) = build_case_plan_and_decision();
    seed_record(
        &admin,
        TENANT_A,
        "customer-privacy.case",
        CASE_ID,
        privacy_case.version(),
        privacy_case_persisted_payload(&privacy_case).expect("encode access case"),
        "access-export-fixture-case",
    )
    .await;
    seed_record(
        &admin,
        TENANT_A,
        ACTION_PLAN_RECORD_TYPE,
        plan.plan_id().as_str(),
        1,
        action_plan_payload(&plan),
        "access-export-fixture-plan",
    )
    .await;
    seed_record(
        &admin,
        TENANT_A,
        "customer-privacy.retention-decision",
        decision.decision_id().as_str(),
        1,
        retention_decision_persisted_payload(&decision).expect("encode retention decision"),
        "access-export-fixture-decision",
    )
    .await;

    let app = PgPool::connect(&database_url)
        .await
        .expect("connect access-export app pool");
    let store = Arc::new(PostgresDataStore::from_pool(app));
    let owner_execution = PostgresOwnerExecutionPersistence::new(store.clone());
    let owner_result = owner_execution
        .prepare_next(&owner_invocation(
            plan.plan_id().clone(),
            decision.decision_id().clone(),
        ))
        .await
        .expect("complete zero-action Access owner execution");
    assert!(matches!(
        owner_result,
        ExecutionPreparation::Complete { .. }
    ));

    let persistence = PostgresAccessExportPersistence::new(store.clone());
    let invocation = access_invocation(TENANT_A, "first");
    let prepared = match persistence
        .prepare(&invocation)
        .await
        .expect("durably prepare access export before artifact I/O")
    {
        AccessExportPreparation::Ready {
            reference,
            replayed,
        } => {
            assert!(!replayed);
            reference
        }
        AccessExportPreparation::Complete { .. } => {
            panic!("first access-export preparation cannot already be complete")
        }
    };
    assert_eq!(
        reference_version(&admin, TENANT_A, prepared.reference_id().as_str()).await,
        1
    );

    let preparation_replay = persistence
        .prepare(&access_invocation(TENANT_A, "prepare-replay"))
        .await
        .expect("recover crash before Customer Data Operations invocation");
    match preparation_replay {
        AccessExportPreparation::Ready {
            reference,
            replayed,
        } => {
            assert!(replayed);
            assert_eq!(reference, prepared);
        }
        AccessExportPreparation::Complete { .. } => {
            panic!("prepared reference must not become complete without target evidence")
        }
    }

    let file_store = Arc::new(MemoryFileStore::default());
    let publisher = PrivacyManifestExportPublisher::new((*store).clone(), file_store.clone());
    let target_request = PrivacyManifestExportRequest {
        tenant_id: TenantId::try_new(TENANT_A).unwrap(),
        privacy_case_id: RecordId::try_new(CASE_ID).unwrap(),
        export_job_id: prepared.export_job_id().clone(),
        target_idempotency_key: prepared.target_idempotency_key().as_str().to_owned(),
        manifest_id: prepared.manifest().manifest_id().clone(),
        manifest_digest: *prepared.manifest().digest(),
        manifest_bytes: encode_access_export_manifest(prepared.manifest()).unwrap(),
        actor_id: ActorId::try_new(ACTOR).unwrap(),
        correlation_id: CorrelationId::try_new("access-export-cdo-correlation").unwrap(),
        trace_id: TraceId::try_new("access-export-cdo-trace").unwrap(),
        prepared_at_unix_nanos: prepared.prepared_at_unix_nanos(),
    };
    let target = publisher
        .request(target_request.clone())
        .await
        .expect("create the CDO-owned durable job and immutable artifact");
    assert!(!target.replayed);
    assert_eq!(file_store.artifact_count(), 1);
    assert_eq!(
        reference_version(&admin, TENANT_A, prepared.reference_id().as_str()).await,
        1
    );

    let target_replay = publisher
        .request(target_request)
        .await
        .expect("recover artifact-finalized before Customer Privacy case link");
    assert!(target_replay.replayed);
    assert_eq!(target_replay.file_id, target.file_id);
    assert_eq!(target_replay.content_sha256, target.content_sha256);
    assert_eq!(file_store.artifact_count(), 1);

    let application_target = crm_customer_privacy_application::PrivacyExportTargetResult {
        export_job_id: target_replay.export_job_id.clone(),
        file_id: target_replay.file_id.clone(),
        media_type: target_replay.media_type.clone(),
        content_sha256: target_replay.content_sha256,
        size_bytes: target_replay.size_bytes,
        retention_policy_id: target_replay.retention_policy_id.clone(),
        completed_at_unix_nanos: target_replay.completed_at_unix_nanos,
        replayed: target_replay.replayed,
    };
    let (completed, completed_now) = persistence
        .complete(
            &access_invocation(TENANT_A, "complete"),
            &prepared,
            &application_target,
        )
        .await
        .expect("link finalized artifact to Customer Privacy");
    assert!(completed_now);
    assert_eq!(completed.artifact().unwrap().file_id(), &target.file_id);
    assert_eq!(
        reference_version(&admin, TENANT_A, completed.reference_id().as_str()).await,
        2
    );

    let (completion_replay, completed_now) = persistence
        .complete(
            &access_invocation(TENANT_A, "complete-replay"),
            &prepared,
            &application_target,
        )
        .await
        .expect("exact completion replay must be immutable");
    assert!(!completed_now);
    assert_eq!(completion_replay, completed);

    let final_replay = persistence
        .prepare(&access_invocation(TENANT_A, "final-replay"))
        .await
        .expect("completed access export must replay without target invocation");
    assert_eq!(
        final_replay,
        AccessExportPreparation::Complete {
            reference: completed.clone(),
        }
    );

    let cross_tenant_error = persistence
        .prepare(&access_invocation(TENANT_B, "cross-tenant"))
        .await
        .expect_err("cross-tenant access-export source must remain concealed");
    assert_eq!(
        cross_tenant_error.code.as_str(),
        "CUSTOMER_PRIVACY_CASE_NOT_FOUND"
    );
    assert_eq!(reference_count(&admin, TENANT_B).await, 0);

    let mut conflicting = application_target;
    conflicting.content_sha256 = [8; 32];
    let conflict = persistence
        .complete(
            &access_invocation(TENANT_A, "conflict"),
            &prepared,
            &conflicting,
        )
        .await
        .expect_err("conflicting artifact replay must fail closed");
    assert_eq!(
        conflict.code.as_str(),
        "CUSTOMER_PRIVACY_ACCESS_EXPORT_CONFLICT"
    );

    assert_eq!(reference_count(&admin, TENANT_A).await, 1);
    assert_eq!(privacy_export_job_count(&admin, TENANT_A).await, 1);
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
        SchemaVersion::try_new("access-export-registry/1").unwrap(),
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
        "ACCESS_DISCOVERY",
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
            RecordId::try_new("access-export-resource-a").unwrap(),
            1,
            DataClass::Personal,
            EvidenceClass::DestroyableSubjectData,
            RetentionPolicyId::try_new("retention-access-export-resource-a").unwrap(),
        )
        .unwrap()],
        ContributionCompletenessProof::new(true, 1, 1, 1, [7; 32]).unwrap(),
    )
    .unwrap();
    let discovery = DiscoveryOwnerScopeContribution::new(lineage.clone(), contribution).unwrap();
    let snapshot =
        DiscoveryScopeSnapshot::finalize(lineage, registry, CAPTURED_AT, [discovery]).unwrap();

    let mut privacy_case = PrivacyCase::new(
        privacy_case_id,
        tenant_id,
        PrivacyCaseKind::Access,
        SchemaVersion::try_new("privacy-policy/1").unwrap(),
        500_000,
        None,
    )
    .unwrap();
    privacy_case.submit(1, 600_000).unwrap();
    privacy_case
        .verify_subject(
            2,
            RecordId::try_new("submitted-access-party").unwrap(),
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
        PrivacyCaseKind::Access,
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

fn owner_invocation(
    action_plan_id: RecordId,
    retention_decision_id: RecordId,
) -> OwnerExecutionInvocation {
    OwnerExecutionInvocation {
        tenant_id: TenantId::try_new(TENANT_A).unwrap(),
        privacy_case_id: RecordId::try_new(CASE_ID).unwrap(),
        action_plan_id,
        retention_decision_id,
        actor_id: ActorId::try_new(ACTOR).unwrap(),
        request_id: crm_module_sdk::RequestId::try_new("access-export-owner-request").unwrap(),
        correlation_id: CorrelationId::try_new("access-export-owner-correlation").unwrap(),
        trace_id: TraceId::try_new("access-export-owner-trace").unwrap(),
        initiating_capability_id: CapabilityId::try_new("customer_privacy.case.approve").unwrap(),
        initiating_capability_version: CapabilityVersion::try_new("1.0.0").unwrap(),
        request_started_at_unix_nanos: EXECUTED_AT - 1_000,
        planned_at_unix_nanos: EXECUTED_AT,
        trusted_internal: true,
    }
}

fn access_invocation(tenant: &str, suffix: &str) -> AccessExportInvocation {
    AccessExportInvocation {
        tenant_id: TenantId::try_new(tenant).unwrap(),
        privacy_case_id: RecordId::try_new(CASE_ID).unwrap(),
        action_plan_id: build_case_plan_and_decision().1.plan_id().clone(),
        actor_id: ActorId::try_new(if tenant == TENANT_A { ACTOR } else { "actor-b" }).unwrap(),
        request_id: crm_module_sdk::RequestId::try_new(format!("access-export-request-{suffix}"))
            .unwrap(),
        correlation_id: CorrelationId::try_new(format!("access-export-correlation-{suffix}"))
            .unwrap(),
        trace_id: TraceId::try_new(format!("access-export-trace-{suffix}")).unwrap(),
        initiating_capability_id: CapabilityId::try_new(ACCESS_EXPORT_REQUEST_CAPABILITY).unwrap(),
        initiating_capability_version: CapabilityVersion::try_new(ACCESS_EXPORT_CAPABILITY_VERSION)
            .unwrap(),
        request_started_at_unix_nanos: PREPARED_AT,
        trusted_internal: true,
    }
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

async fn seed_record(
    admin: &PgPool,
    tenant: &str,
    record_type: &str,
    record_id: &str,
    version: i64,
    payload: TypedPayload,
    business_transaction_id: &str,
) {
    let mut transaction = admin.begin().await.expect("begin access-export fixture");
    seed_transaction(
        &mut transaction,
        tenant,
        business_transaction_id,
        &format!("{business_transaction_id}-request"),
    )
    .await;
    sqlx::query(
        r#"
        INSERT INTO crm.records (
          tenant_id, record_type, record_id, version, owner_module_id,
          schema_id, schema_version, descriptor_hash, data_class,
          payload_encoding, maximum_payload_size, retention_policy_id,
          payload_bytes, last_business_transaction_id
        ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14)
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
    .bind("json")
    .bind(i64::try_from(payload.maximum_size_bytes).unwrap())
    .bind(payload.retention_policy_id.as_str())
    .bind(payload.bytes)
    .bind(business_transaction_id)
    .execute(&mut *transaction)
    .await
    .expect("insert access-export fixture record");
    transaction
        .commit()
        .await
        .expect("commit access-export fixture record");
}

async fn seed_transaction(
    transaction: &mut Transaction<'_, Postgres>,
    tenant: &str,
    business_transaction_id: &str,
    request_id: &str,
) {
    sqlx::query(
        r#"
        INSERT INTO crm.business_transactions (
          tenant_id, business_transaction_id, actor_id, request_id,
          correlation_id, trace_id, capability_id, capability_version,
          expected_outbox_events, expected_audit_records, expected_idempotency_records
        ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,0,0,0)
        "#,
    )
    .bind(tenant)
    .bind(business_transaction_id)
    .bind(ACTOR)
    .bind(request_id)
    .bind(format!("{business_transaction_id}-correlation"))
    .bind(format!("{business_transaction_id}-trace"))
    .bind("customer_privacy.test.fixture")
    .bind("1.0.0")
    .execute(&mut **transaction)
    .await
    .expect("insert access-export fixture transaction");
}

async fn reference_version(admin: &PgPool, tenant: &str, reference_id: &str) -> i64 {
    sqlx::query_scalar(
        "SELECT version FROM crm.records WHERE tenant_id = $1 AND record_type = 'customer-privacy.access-export-reference' AND record_id = $2",
    )
    .bind(tenant)
    .bind(reference_id)
    .fetch_one(admin)
    .await
    .expect("read access-export reference version")
}

async fn reference_count(admin: &PgPool, tenant: &str) -> i64 {
    sqlx::query_scalar(
        "SELECT count(*) FROM crm.records WHERE tenant_id = $1 AND record_type = 'customer-privacy.access-export-reference'",
    )
    .bind(tenant)
    .fetch_one(admin)
    .await
    .expect("count access-export references")
}

async fn privacy_export_job_count(admin: &PgPool, tenant: &str) -> i64 {
    sqlx::query_scalar(
        "SELECT count(*) FROM crm.records WHERE tenant_id = $1 AND record_type = 'customer_data.privacy_export_job'",
    )
    .bind(tenant)
    .fetch_one(admin)
    .await
    .expect("count privacy export jobs")
}

async fn cleanup(admin: &PgPool) {
    let mut transaction = admin.begin().await.expect("begin access-export cleanup");
    sqlx::query("SET LOCAL session_replication_role = replica")
        .execute(&mut *transaction)
        .await
        .expect("disable cleanup verification");
    for table in [
        "crm.customer_privacy_owner_execution_audit",
        "crm.customer_privacy_owner_action_outcomes",
        "crm.customer_privacy_owner_action_attempts",
        "crm.customer_privacy_owner_execution_checkpoints",
        "crm.outbox_events",
        "crm.audit_records",
        "crm.idempotency_records",
        "crm.records",
        "crm.business_transactions",
    ] {
        sqlx::query(&format!("DELETE FROM {table} WHERE tenant_id IN ($1, $2)"))
            .bind(TENANT_A)
            .bind(TENANT_B)
            .execute(&mut *transaction)
            .await
            .unwrap_or_else(|error| panic!("cleanup {table}: {error}"));
    }
    transaction
        .commit()
        .await
        .expect("commit access-export cleanup");
}

fn data_class_name(data_class: DataClass) -> &'static str {
    match data_class {
        DataClass::Confidential => "confidential",
        DataClass::Personal => "personal",
        other => panic!("unsupported fixture data class: {other:?}"),
    }
}

#[derive(Default)]
struct MemoryFileStore {
    artifacts: Mutex<BTreeMap<String, MemoryArtifact>>,
}

#[derive(Clone)]
struct MemoryArtifact {
    metadata: FileArtifactMetadata,
    bytes: Vec<u8>,
}

impl MemoryFileStore {
    fn artifact_count(&self) -> usize {
        self.artifacts.lock().unwrap().len()
    }
}

impl ImmutableFileArtifactStore for MemoryFileStore {
    fn create<'a>(
        &'a self,
        _context: &'a ModuleExecutionContext,
        command: CreateImmutableFileArtifact,
    ) -> PortFuture<'a, Result<FileArtifactMetadata, SdkError>> {
        Box::pin(async move {
            command.validate()?;
            let mut artifacts = self.artifacts.lock().unwrap();
            if let Some(existing) = artifacts.get(command.file_id.as_str()) {
                if existing.metadata.file_id != command.file_id
                    || existing.metadata.owner_module_id != command.owner_module_id
                    || existing.metadata.media_type != command.media_type
                    || existing.metadata.data_class != command.data_class
                    || existing.metadata.retention_policy_id != command.retention_policy_id
                    || existing.metadata.expected_size_bytes != command.expected_size_bytes
                    || existing.metadata.expected_sha256 != command.expected_sha256
                {
                    return Err(SdkError::new(
                        "FILE_ARTIFACT_CONFLICT",
                        crm_module_sdk::ErrorCategory::Conflict,
                        false,
                        "The file artifact identity conflicts with immutable metadata.",
                    ));
                }
                return Ok(existing.metadata.clone());
            }
            let metadata = FileArtifactMetadata {
                file_id: command.file_id.clone(),
                owner_module_id: command.owner_module_id,
                media_type: command.media_type,
                data_class: command.data_class,
                retention_policy_id: command.retention_policy_id,
                expected_size_bytes: command.expected_size_bytes,
                expected_sha256: command.expected_sha256,
                status: FileArtifactStatus::Uploading,
                next_chunk_index: 0,
                received_size_bytes: 0,
            };
            artifacts.insert(
                command.file_id.as_str().to_owned(),
                MemoryArtifact {
                    metadata: metadata.clone(),
                    bytes: Vec::new(),
                },
            );
            Ok(metadata)
        })
    }

    fn append_chunk<'a>(
        &'a self,
        _context: &'a ModuleExecutionContext,
        command: AppendImmutableFileChunk,
    ) -> PortFuture<'a, Result<FileArtifactAppendResult, SdkError>> {
        Box::pin(async move {
            command.validate()?;
            let mut artifacts = self.artifacts.lock().unwrap();
            let artifact = artifacts.get_mut(command.file_id.as_str()).ok_or_else(|| {
                SdkError::new(
                    "FILE_ARTIFACT_NOT_FOUND",
                    crm_module_sdk::ErrorCategory::NotFound,
                    false,
                    "The file artifact was not found.",
                )
            })?;
            if artifact.metadata.status == FileArtifactStatus::Finalized {
                return Ok(FileArtifactAppendResult {
                    metadata: artifact.metadata.clone(),
                    replayed: true,
                });
            }
            if command.chunk_index < artifact.metadata.next_chunk_index {
                return Ok(FileArtifactAppendResult {
                    metadata: artifact.metadata.clone(),
                    replayed: true,
                });
            }
            if command.chunk_index != artifact.metadata.next_chunk_index
                || Sha256::digest(&command.bytes).as_slice() != command.chunk_sha256
            {
                return Err(SdkError::new(
                    "FILE_ARTIFACT_CHUNK_CONFLICT",
                    crm_module_sdk::ErrorCategory::Conflict,
                    false,
                    "The file artifact chunk conflicts with immutable upload state.",
                ));
            }
            artifact.bytes.extend_from_slice(&command.bytes);
            artifact.metadata.next_chunk_index += 1;
            artifact.metadata.received_size_bytes = artifact.bytes.len() as u64;
            Ok(FileArtifactAppendResult {
                metadata: artifact.metadata.clone(),
                replayed: false,
            })
        })
    }

    fn finalize<'a>(
        &'a self,
        _context: &'a ModuleExecutionContext,
        file_id: &'a FileId,
    ) -> PortFuture<'a, Result<FileArtifactMetadata, SdkError>> {
        Box::pin(async move {
            let mut artifacts = self.artifacts.lock().unwrap();
            let artifact = artifacts.get_mut(file_id.as_str()).ok_or_else(|| {
                SdkError::new(
                    "FILE_ARTIFACT_NOT_FOUND",
                    crm_module_sdk::ErrorCategory::NotFound,
                    false,
                    "The file artifact was not found.",
                )
            })?;
            if artifact.metadata.status == FileArtifactStatus::Finalized {
                return Ok(artifact.metadata.clone());
            }
            if artifact.bytes.len() as u64 != artifact.metadata.expected_size_bytes
                || Sha256::digest(&artifact.bytes).as_slice() != artifact.metadata.expected_sha256
            {
                return Err(SdkError::new(
                    "FILE_ARTIFACT_FINALIZE_CONFLICT",
                    crm_module_sdk::ErrorCategory::Conflict,
                    false,
                    "The file artifact bytes differ from immutable metadata.",
                ));
            }
            artifact.metadata.status = FileArtifactStatus::Finalized;
            Ok(artifact.metadata.clone())
        })
    }

    fn read_finalized<'a>(
        &'a self,
        _context: &'a ModuleExecutionContext,
        file_id: &'a FileId,
    ) -> PortFuture<'a, Result<FinalizedFileArtifact, SdkError>> {
        Box::pin(async move {
            let artifacts = self.artifacts.lock().unwrap();
            let artifact = artifacts.get(file_id.as_str()).ok_or_else(|| {
                SdkError::new(
                    "FILE_ARTIFACT_NOT_FOUND",
                    crm_module_sdk::ErrorCategory::NotFound,
                    false,
                    "The file artifact was not found.",
                )
            })?;
            if artifact.metadata.status != FileArtifactStatus::Finalized {
                return Err(SdkError::new(
                    "FILE_ARTIFACT_NOT_FINALIZED",
                    crm_module_sdk::ErrorCategory::Conflict,
                    true,
                    "The file artifact is not finalized.",
                ));
            }
            Ok(FinalizedFileArtifact {
                metadata: artifact.metadata.clone(),
                bytes: artifact.bytes.clone(),
            })
        })
    }
}
