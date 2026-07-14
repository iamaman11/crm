#![cfg(feature = "postgres-integration")]

use crm_core_data::{PostgresDataStore, PostgresImmutableFileArtifactStore};
use crm_core_files::{
    AppendImmutableFileChunk, CreateImmutableFileArtifact, FileArtifactStatus,
    ImmutableFileArtifactStore,
};
use crm_module_sdk::{
    ActorId, BusinessTransactionId, CapabilityId, CapabilityVersion, CausationId, CorrelationId,
    DataClass, ExecutionContext, FileId, IdempotencyKey, ModuleExecutionContext, ModuleId,
    RequestId, RetentionPolicyId, SchemaVersion, TenantId, TraceId,
};
use sha2::{Digest, Sha256};
use sqlx::PgPool;

const TENANT_A: &str = "tenant-a";
const TENANT_B: &str = "tenant-b";
const FILE_ID: &str = "phase8a7-import-source-artifact";
const OWNER_MODULE_ID: &str = "crm.customer-data-operations";

#[tokio::test(flavor = "current_thread")]
async fn immutable_artifact_upload_is_sequential_replay_safe_finalized_and_tenant_isolated() {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!("skipping PostgreSQL file artifact acceptance because DATABASE_URL is absent");
        return;
    };
    let admin_database_url = std::env::var("ADMIN_DATABASE_URL")
        .expect("ADMIN_DATABASE_URL must accompany DATABASE_URL");
    let store = PostgresDataStore::connect(&database_url, 4)
        .await
        .expect("connect file artifact store");
    let artifacts = PostgresImmutableFileArtifactStore::new(store);
    let admin = PgPool::connect(&admin_database_url)
        .await
        .expect("connect file artifact evidence reader");
    cleanup(&admin).await;

    let bytes = b"kind,display_name\nperson,Ada Lovelace\norganization,Analytical Engines Ltd\n";
    let expected_sha256: [u8; 32] = Sha256::digest(bytes).into();
    let context_a = context(TENANT_A, "artifact-create-a");
    let created = artifacts
        .create(
            &context_a,
            CreateImmutableFileArtifact {
                file_id: FileId::try_new(FILE_ID).unwrap(),
                owner_module_id: ModuleId::try_new(OWNER_MODULE_ID).unwrap(),
                media_type: "text/csv".to_owned(),
                data_class: DataClass::Personal,
                retention_policy_id: RetentionPolicyId::try_new("crm.customer_data.import_source")
                    .unwrap(),
                expected_size_bytes: bytes.len() as u64,
                expected_sha256,
            },
        )
        .await
        .expect("create immutable artifact");
    assert_eq!(created.status, FileArtifactStatus::Uploading);
    assert_eq!(created.next_chunk_index, 0);

    let chunks = bytes.chunks(24).collect::<Vec<_>>();
    let first = append(&artifacts, &context_a, 0, chunks[0]).await;
    assert!(!first.replayed);
    let replay = append(&artifacts, &context_a, 0, chunks[0]).await;
    assert!(replay.replayed);

    if chunks.len() > 2 {
        let out_of_order = artifacts
            .append_chunk(
                &context_a,
                chunk_command(2, chunks[2]),
            )
            .await
            .unwrap_err();
        assert_eq!(out_of_order.code, "FILE_ARTIFACT_CHUNK_OUT_OF_ORDER");
    }

    for (index, chunk) in chunks.iter().enumerate().skip(1) {
        let result = append(&artifacts, &context_a, index as u64, chunk).await;
        assert!(!result.replayed);
    }

    let finalized = artifacts
        .finalize(&context_a, &FileId::try_new(FILE_ID).unwrap())
        .await
        .expect("finalize exact artifact");
    assert_eq!(finalized.status, FileArtifactStatus::Finalized);
    assert_eq!(finalized.received_size_bytes, bytes.len() as u64);

    let read = artifacts
        .read_finalized(&context_a, &FileId::try_new(FILE_ID).unwrap())
        .await
        .expect("read finalized artifact");
    assert_eq!(read.bytes, bytes);
    assert_eq!(read.metadata.expected_sha256, expected_sha256);

    let immutable = artifacts
        .append_chunk(&context_a, chunk_command(chunks.len() as u64, b"x"))
        .await
        .unwrap_err();
    assert_eq!(immutable.code, "FILE_ARTIFACT_ALREADY_FINALIZED");

    let context_b = context(TENANT_B, "artifact-read-b");
    let isolated = artifacts
        .read_finalized(&context_b, &FileId::try_new(FILE_ID).unwrap())
        .await
        .unwrap_err();
    assert_eq!(isolated.code, "FILE_ARTIFACT_NOT_FOUND");

    let evidence = sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM crm.file_artifacts WHERE tenant_id = $1 AND file_id = $2",
    )
    .bind(TENANT_A)
    .bind(FILE_ID)
    .fetch_one(&admin)
    .await
    .expect("read artifact evidence count");
    assert_eq!(evidence, 1);

    cleanup(&admin).await;
}

async fn append(
    store: &PostgresImmutableFileArtifactStore,
    context: &ModuleExecutionContext,
    index: u64,
    bytes: &[u8],
) -> crm_core_files::FileArtifactAppendResult {
    store
        .append_chunk(context, chunk_command(index, bytes))
        .await
        .expect("append file artifact chunk")
}

fn chunk_command(index: u64, bytes: &[u8]) -> AppendImmutableFileChunk {
    AppendImmutableFileChunk {
        file_id: FileId::try_new(FILE_ID).unwrap(),
        chunk_index: index,
        chunk_sha256: Sha256::digest(bytes).into(),
        bytes: bytes.to_vec(),
    }
}

fn context(tenant_id: &str, suffix: &str) -> ModuleExecutionContext {
    ModuleExecutionContext {
        module_id: ModuleId::try_new(OWNER_MODULE_ID).unwrap(),
        execution: ExecutionContext {
            tenant_id: TenantId::try_new(tenant_id).unwrap(),
            actor_id: ActorId::try_new("file-artifact-test-actor").unwrap(),
            request_id: RequestId::try_new(format!("request-{suffix}")).unwrap(),
            correlation_id: CorrelationId::try_new(format!("correlation-{suffix}")).unwrap(),
            causation_id: CausationId::try_new(format!("causation-{suffix}")).unwrap(),
            trace_id: TraceId::try_new(format!("trace-{suffix}")).unwrap(),
            capability_id: CapabilityId::try_new("customer_data.import.party.source.upload")
                .unwrap(),
            capability_version: CapabilityVersion::try_new("1.0.0").unwrap(),
            idempotency_key: IdempotencyKey::try_new(format!("idempotency-{suffix}")).unwrap(),
            business_transaction_id: BusinessTransactionId::try_new(format!("tx-{suffix}"))
                .unwrap(),
            schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
            request_started_at_unix_nanos: 1_000,
        },
    }
}

async fn cleanup(admin: &PgPool) {
    sqlx::query("DELETE FROM crm.file_artifacts WHERE file_id = $1")
        .bind(FILE_ID)
        .execute(admin)
        .await
        .expect("clean file artifact acceptance state");
}
