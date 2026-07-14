#![cfg(unix)]

use crm_application_runtime::{
    application_mutation_definitions, application_query_definitions,
    gateway_v1::{
        MutateRequest as GatewayMutateRequest, QueryRequest as GatewayQueryRequest,
        TypedPayload as GatewayTypedPayload,
        application_gateway_service_client::ApplicationGatewayServiceClient,
    },
};
use crm_capability_runtime::CapabilityDefinition;
use crm_module_sdk::{DataClass, PayloadEncoding, RetentionPolicyId, TypedPayload};
use crm_proto_contracts::crm::customer_data_operations::v1 as cdo;
use prost::Message;
use sha2::{Digest, Sha256};
use sqlx::{Executor, PgPool};
use std::net::TcpListener;
use std::process::Stdio;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::process::{Child, Command};
use tokio::time::{sleep, timeout};
use tonic::{Code, Request, Status};

const TENANT_A: &str = "tenant-a";
const TENANT_B: &str = "tenant-b";
const ACTOR: &str = "actor-a";
const TOKEN: &str = "import-process-bearer-token-0123456789abcdef0123456789abcdef";

const SOURCE_CREATE: &str = "customer_data.import.party.source.create";
const SOURCE_CHUNK_APPEND: &str = "customer_data.import.party.source.chunk.append";
const SOURCE_FINALIZE: &str = "customer_data.import.party.source.finalize";
const SOURCE_JOB_CREATE: &str = "customer_data.import.party.source.job.create";
const SOURCE_ROWS_VALIDATE: &str = "customer_data.import.party.source.rows.validate";
const VALIDATION_FINALIZE: &str = "customer_data.import.party.validation.finalize";
const EXECUTION_START: &str = "customer_data.import.party.execution.start";
const IMPORT_GET: &str = "customer_data.import.party.get";
const IMPORT_LIST: &str = "customer_data.import.party.list";
const IMPORT_ROWS_LIST: &str = "customer_data.import.party.rows.list";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PartyTargetEffects {
    records: i64,
    idempotency: i64,
    events: i64,
    audits: i64,
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn crm_api_process_proves_artifact_dry_run_and_crash_restart_import_execution() {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!("skipping import process acceptance because DATABASE_URL is absent");
        return;
    };
    let admin_database_url = std::env::var("ADMIN_DATABASE_URL")
        .expect("ADMIN_DATABASE_URL must accompany DATABASE_URL");
    let admin = PgPool::connect(&admin_database_url)
        .await
        .expect("connect import process evidence reader");

    admin
        .execute(sqlx::raw_sql(include_str!(
            "../../../database/tests/0005_party_adapter.sql"
        )))
        .await
        .expect("publish Party capability registry fixture");
    admin
        .execute(sqlx::raw_sql(include_str!(
            "../../../database/tests/0012_customer_data_operations_adapter.sql"
        )))
        .await
        .expect("publish customer-data operations registry and worker fixture");

    let (mut child, http_addr, grpc_addr) = spawn_api(&database_url).await;
    let http = reqwest::Client::new();
    wait_until_ready(&http, &mut child, &http_addr).await;
    let mut grpc = connect_grpc(&grpc_addr).await;

    let baseline = party_target_effects(&admin, TENANT_A).await;
    let suffix = unique_suffix();

    // Dry run: one valid row plus one invalid kind. Exact bytes are uploaded/finalized, parsed
    // server-side, and persisted only as import-owned evidence. RequireAllValid must refuse start.
    let dry_source_id = format!("import-source-dry-{suffix}");
    let dry_job_id = format!("import-job-dry-{suffix}");
    let dry_csv = format!(
        "kind,display_name,external_id\nperson,Dry Run Valid {suffix},dry-valid-{suffix}\nunsupported,Dry Run Invalid {suffix},dry-invalid-{suffix}\n"
    )
    .into_bytes();
    upload_source(
        &mut grpc,
        TENANT_A,
        &dry_source_id,
        &dry_csv,
        "dry-source",
    )
    .await;

    let cross_tenant_finalize = mutate_message(
        &mut grpc,
        SOURCE_FINALIZE,
        cdo::FinalizePartyImportSourceArtifactRequest {
            source_artifact_ref: Some(source_ref(&dry_source_id)),
        },
        TENANT_B,
        "cross-tenant-source-finalize",
    )
    .await
    .expect_err("tenant B must not discover tenant A import source artifact");
    assert_eq!(cross_tenant_finalize.code(), Code::NotFound);

    create_job_from_source(
        &mut grpc,
        &dry_job_id,
        &dry_source_id,
        cdo::PartialExecutionPolicy::RequireAllValid,
        "dry-job-create",
    )
    .await;
    let dry_rows = validate_source(&mut grpc, &dry_job_id, "dry-job-validate").await;
    assert_eq!(dry_rows.len(), 2);
    assert_eq!(
        dry_rows
            .iter()
            .filter(|row| row.status == cdo::ImportRowStatus::Valid as i32)
            .count(),
        1
    );
    assert_eq!(
        dry_rows
            .iter()
            .filter(|row| row.status == cdo::ImportRowStatus::Invalid as i32)
            .count(),
        1
    );
    assert_eq!(party_target_effects(&admin, TENANT_A).await, baseline);

    let dry_job = get_job(&mut grpc, TENANT_A, &dry_job_id).await;
    assert_eq!(dry_job.total_rows, 2);
    assert_eq!(dry_job.valid_rows, 1);
    assert_eq!(dry_job.invalid_rows, 1);
    let dry_finalized = finalize_validation(
        &mut grpc,
        &dry_job_id,
        resource_version(&dry_job),
        "dry-job-finalize",
    )
    .await;
    assert_eq!(dry_finalized.status, cdo::ImportJobStatus::Validated as i32);
    let dry_start = mutate_message(
        &mut grpc,
        EXECUTION_START,
        cdo::StartPartyImportExecutionRequest {
            import_job_ref: Some(job_ref(&dry_job_id)),
            expected_version: resource_version(&dry_finalized),
        },
        TENANT_A,
        "dry-job-start-rejected",
    )
    .await
    .expect_err("RequireAllValid import with invalid rows must not execute");
    assert_eq!(dry_start.code(), Code::Aborted);
    assert_eq!(party_target_effects(&admin, TENANT_A).await, baseline);

    // Crash/restart scenario: one valid source row. A test-only PostgreSQL trigger delays the
    // import-owned row outcome update after the governed Party create has already committed.
    let crash_source_id = format!("import-source-crash-{suffix}");
    let crash_job_id = format!("import-job-crash-{suffix}");
    let crash_csv = format!(
        "kind,display_name,external_id\nperson,Crash Restart Party {suffix},crash-{suffix}\n"
    )
    .into_bytes();
    upload_source(
        &mut grpc,
        TENANT_A,
        &crash_source_id,
        &crash_csv,
        "crash-source",
    )
    .await;
    create_job_from_source(
        &mut grpc,
        &crash_job_id,
        &crash_source_id,
        cdo::PartialExecutionPolicy::AllValidRows,
        "crash-job-create",
    )
    .await;
    let crash_rows = validate_source(&mut grpc, &crash_job_id, "crash-job-validate").await;
    assert_eq!(crash_rows.len(), 1);
    assert_eq!(crash_rows[0].status, cdo::ImportRowStatus::Valid as i32);
    let target_party_id = crash_rows[0]
        .prepared_party
        .as_ref()
        .and_then(|prepared| prepared.party_ref.as_ref())
        .expect("validated import row must prepare target Party identity")
        .party_id
        .clone();
    assert!(!target_party_id.is_empty());
    assert_eq!(party_target_effects(&admin, TENANT_A).await, baseline);

    let crash_job = get_job(&mut grpc, TENANT_A, &crash_job_id).await;
    let crash_finalized = finalize_validation(
        &mut grpc,
        &crash_job_id,
        resource_version(&crash_job),
        "crash-job-finalize",
    )
    .await;

    install_import_outcome_delay_trigger(&admin).await;
    let started = start_execution(
        &mut grpc,
        &crash_job_id,
        resource_version(&crash_finalized),
        "crash-job-start",
    )
    .await;
    assert_eq!(started.status, cdo::ImportJobStatus::Executing as i32);

    wait_for_party_record(&admin, &target_party_id).await;
    assert_eq!(party_record_count(&admin, &target_party_id).await, 1);

    child.kill().await.expect("force-kill crm-api in target-success crash window");
    let _ = child.wait().await;
    drop_import_outcome_delay_trigger(&admin).await;

    // The target mutation committed before the crash, but the import-owned success/checkpoint
    // transaction was interrupted. Restart must repeat the exact target capability idempotently.
    assert_eq!(party_record_count(&admin, &target_party_id).await, 1);
    assert_eq!(party_create_idempotency_count(&admin, &target_party_id).await, 1);

    let (mut restarted, restarted_http_addr, restarted_grpc_addr) = spawn_api(&database_url).await;
    wait_until_ready(&http, &mut restarted, &restarted_http_addr).await;
    let mut restarted_grpc = connect_grpc(&restarted_grpc_addr).await;

    let completed = wait_for_completed_job(&mut restarted_grpc, &crash_job_id).await;
    assert_eq!(completed.status, cdo::ImportJobStatus::Completed as i32);
    assert_eq!(completed.total_rows, 1);
    assert_eq!(completed.valid_rows, 1);
    assert_eq!(completed.invalid_rows, 0);
    assert_eq!(completed.succeeded_rows, 1);
    assert_eq!(completed.checkpoint_row_position, 1);

    let completed_rows = list_rows(&mut restarted_grpc, TENANT_A, &crash_job_id).await;
    assert_eq!(completed_rows.len(), 1);
    assert_eq!(completed_rows[0].status, cdo::ImportRowStatus::Succeeded as i32);
    assert_eq!(completed_rows[0].execution_attempts, 1);
    assert_eq!(
        completed_rows[0]
            .target_party_ref
            .as_ref()
            .expect("succeeded row target Party reference")
            .party_id,
        target_party_id
    );
    assert_eq!(party_record_count(&admin, &target_party_id).await, 1);
    assert_eq!(party_create_idempotency_count(&admin, &target_party_id).await, 1);

    let final_effects = party_target_effects(&admin, TENANT_A).await;
    assert_eq!(final_effects.records, baseline.records + 1);
    assert_eq!(final_effects.idempotency, baseline.idempotency + 1);
    assert_eq!(final_effects.events, baseline.events + 1);
    assert_eq!(final_effects.audits, baseline.audits + 1);

    let cross_tenant_job = query_message(
        &mut restarted_grpc,
        IMPORT_GET,
        cdo::GetPartyImportJobRequest {
            import_job_ref: Some(job_ref(&crash_job_id)),
        },
        TENANT_B,
    )
    .await
    .expect_err("tenant B must not discover tenant A import job");
    assert_eq!(cross_tenant_job.code(), Code::NotFound);

    prove_cursor_tamper_rejection(&mut restarted_grpc).await;

    send_sigint(&restarted).await;
    let exit = timeout(Duration::from_secs(15), restarted.wait())
        .await
        .expect("restarted crm-api must stop within graceful-shutdown budget")
        .expect("wait for restarted import acceptance crm-api process");
    assert!(exit.success(), "restarted crm-api exited unsuccessfully: {exit}");
}

async fn upload_source(
    grpc: &mut ApplicationGatewayServiceClient<tonic::transport::Channel>,
    tenant_id: &str,
    source_id: &str,
    bytes: &[u8],
    idempotency_prefix: &str,
) {
    let digest = sha256(bytes);
    let created = mutate_message(
        grpc,
        SOURCE_CREATE,
        cdo::CreatePartyImportSourceArtifactRequest {
            source_artifact_ref: Some(source_ref(source_id)),
            expected_size_bytes: bytes.len() as u64,
            expected_sha256: digest.to_vec(),
        },
        tenant_id,
        &format!("{idempotency_prefix}-create"),
    )
    .await
    .expect("create immutable import source artifact");
    let created = decode_mutation::<cdo::CreatePartyImportSourceArtifactResponse>(created);
    assert!(!created.source_artifact.expect("created source artifact").finalized);

    let chunk_digest = sha256(bytes);
    let appended = mutate_message(
        grpc,
        SOURCE_CHUNK_APPEND,
        cdo::AppendPartyImportSourceChunkRequest {
            source_artifact_ref: Some(source_ref(source_id)),
            chunk_index: 0,
            chunk_sha256: chunk_digest.to_vec(),
            chunk_bytes: bytes.to_vec(),
        },
        tenant_id,
        &format!("{idempotency_prefix}-append"),
    )
    .await
    .expect("append exact import source chunk");
    let appended = decode_mutation::<cdo::AppendPartyImportSourceChunkResponse>(appended);
    assert!(!appended.replayed);

    let identical_replay = mutate_message(
        grpc,
        SOURCE_CHUNK_APPEND,
        cdo::AppendPartyImportSourceChunkRequest {
            source_artifact_ref: Some(source_ref(source_id)),
            chunk_index: 0,
            chunk_sha256: chunk_digest.to_vec(),
            chunk_bytes: bytes.to_vec(),
        },
        tenant_id,
        &format!("{idempotency_prefix}-append-identical-replay"),
    )
    .await
    .expect("identical chunk replay must be accepted safely");
    assert!(decode_mutation::<cdo::AppendPartyImportSourceChunkResponse>(identical_replay).replayed);

    let finalized = mutate_message(
        grpc,
        SOURCE_FINALIZE,
        cdo::FinalizePartyImportSourceArtifactRequest {
            source_artifact_ref: Some(source_ref(source_id)),
        },
        tenant_id,
        &format!("{idempotency_prefix}-finalize"),
    )
    .await
    .expect("finalize immutable import source artifact");
    let finalized = decode_mutation::<cdo::FinalizePartyImportSourceArtifactResponse>(finalized)
        .source_artifact
        .expect("finalized source artifact");
    assert!(finalized.finalized);
    assert_eq!(finalized.expected_size_bytes, bytes.len() as u64);
    assert_eq!(finalized.received_size_bytes, bytes.len() as u64);
    assert_eq!(finalized.expected_sha256, digest);
}

async fn create_job_from_source(
    grpc: &mut ApplicationGatewayServiceClient<tonic::transport::Channel>,
    job_id: &str,
    source_id: &str,
    policy: cdo::PartialExecutionPolicy,
    idempotency_key: &str,
) -> cdo::ImportJob {
    let response = mutate_message(
        grpc,
        SOURCE_JOB_CREATE,
        cdo::CreatePartyImportJobFromSourceArtifactRequest {
            import_job_ref: Some(job_ref(job_id)),
            source_artifact_ref: Some(source_ref(source_id)),
            source_name: format!("{job_id}.csv"),
            source_system_id: "process-acceptance-source".to_owned(),
            parser_profile: Some(parser_profile()),
            mapping: Some(cdo::PartyImportMapping {
                target_party_id_column: None,
                party_kind_column: "kind".to_owned(),
                display_name_column: "display_name".to_owned(),
                source_external_id_column: Some("external_id".to_owned()),
                external_row_key_column: Some("external_id".to_owned()),
            }),
            partial_execution_policy: policy as i32,
        },
        TENANT_A,
        idempotency_key,
    )
    .await
    .expect("create import job from finalized source bytes");
    decode_mutation::<cdo::CreatePartyImportJobFromSourceArtifactResponse>(response)
        .import_job
        .expect("created import job")
}

async fn validate_source(
    grpc: &mut ApplicationGatewayServiceClient<tonic::transport::Channel>,
    job_id: &str,
    idempotency_key: &str,
) -> Vec<cdo::ImportRow> {
    let response = mutate_message(
        grpc,
        SOURCE_ROWS_VALIDATE,
        cdo::ValidatePartyImportSourceBatchRequest {
            import_job_ref: Some(job_ref(job_id)),
            start_row_position: 1,
            max_rows: 500,
        },
        TENANT_A,
        idempotency_key,
    )
    .await
    .expect("validate exact finalized source bytes server-side");
    let response = decode_mutation::<cdo::ValidatePartyImportSourceBatchResponse>(response);
    assert!(response.source_exhausted);
    assert_eq!(response.next_row_position, 0);
    response.import_rows
}

async fn finalize_validation(
    grpc: &mut ApplicationGatewayServiceClient<tonic::transport::Channel>,
    job_id: &str,
    expected_version: i64,
    idempotency_key: &str,
) -> cdo::ImportJob {
    let response = mutate_message(
        grpc,
        VALIDATION_FINALIZE,
        cdo::FinalizePartyImportValidationRequest {
            import_job_ref: Some(job_ref(job_id)),
            expected_version,
        },
        TENANT_A,
        idempotency_key,
    )
    .await
    .expect("finalize authoritative import validation");
    decode_mutation::<cdo::FinalizePartyImportValidationResponse>(response)
        .import_job
        .expect("finalized import job")
}

async fn start_execution(
    grpc: &mut ApplicationGatewayServiceClient<tonic::transport::Channel>,
    job_id: &str,
    expected_version: i64,
    idempotency_key: &str,
) -> cdo::ImportJob {
    let response = mutate_message(
        grpc,
        EXECUTION_START,
        cdo::StartPartyImportExecutionRequest {
            import_job_ref: Some(job_ref(job_id)),
            expected_version,
        },
        TENANT_A,
        idempotency_key,
    )
    .await
    .expect("start governed import execution");
    decode_mutation::<cdo::StartPartyImportExecutionResponse>(response)
        .import_job
        .expect("started import job")
}

async fn get_job(
    grpc: &mut ApplicationGatewayServiceClient<tonic::transport::Channel>,
    tenant_id: &str,
    job_id: &str,
) -> cdo::ImportJob {
    let response = query_message(
        grpc,
        IMPORT_GET,
        cdo::GetPartyImportJobRequest {
            import_job_ref: Some(job_ref(job_id)),
        },
        tenant_id,
    )
    .await
    .expect("query import job through production gateway");
    decode_query::<cdo::GetPartyImportJobResponse>(response)
        .import_job
        .expect("queried import job")
}

async fn list_rows(
    grpc: &mut ApplicationGatewayServiceClient<tonic::transport::Channel>,
    tenant_id: &str,
    job_id: &str,
) -> Vec<cdo::ImportRow> {
    let response = query_message(
        grpc,
        IMPORT_ROWS_LIST,
        cdo::ListPartyImportRowsRequest {
            import_job_ref: Some(job_ref(job_id)),
            status: cdo::ImportRowStatus::Unspecified as i32,
            page_size: 100,
            cursor: String::new(),
        },
        tenant_id,
    )
    .await
    .expect("list import rows through production gateway");
    decode_query::<cdo::ListPartyImportRowsResponse>(response).import_rows
}

async fn wait_for_completed_job(
    grpc: &mut ApplicationGatewayServiceClient<tonic::transport::Channel>,
    job_id: &str,
) -> cdo::ImportJob {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let job = get_job(grpc, TENANT_A, job_id).await;
        if job.status == cdo::ImportJobStatus::Completed as i32 {
            return job;
        }
        assert!(
            Instant::now() < deadline,
            "import job did not complete after process restart"
        );
        sleep(Duration::from_millis(200)).await;
    }
}

async fn prove_cursor_tamper_rejection(
    grpc: &mut ApplicationGatewayServiceClient<tonic::transport::Channel>,
) {
    let first = query_message(
        grpc,
        IMPORT_LIST,
        cdo::ListPartyImportJobsRequest {
            status: cdo::ImportJobStatus::Unspecified as i32,
            page_size: 1,
            cursor: String::new(),
        },
        TENANT_A,
    )
    .await
    .expect("list first import-job page");
    let first = decode_query::<cdo::ListPartyImportJobsResponse>(first);
    assert_eq!(first.import_jobs.len(), 1);
    assert!(!first.next_cursor.is_empty());

    let mut tampered = first.next_cursor.into_bytes();
    let last = tampered.last_mut().expect("non-empty signed cursor");
    *last = if *last == b'A' { b'B' } else { b'A' };
    let tampered = String::from_utf8(tampered).expect("ASCII signed cursor");
    let error = query_message(
        grpc,
        IMPORT_LIST,
        cdo::ListPartyImportJobsRequest {
            status: cdo::ImportJobStatus::Unspecified as i32,
            page_size: 1,
            cursor: tampered,
        },
        TENANT_A,
    )
    .await
    .expect_err("tampered signed cursor must be rejected");
    assert_eq!(error.code(), Code::InvalidArgument);
}

async fn install_import_outcome_delay_trigger(admin: &PgPool) {
    admin
        .execute(sqlx::raw_sql(
            r#"
            CREATE OR REPLACE FUNCTION crm.test_delay_import_row_outcome()
            RETURNS trigger
            LANGUAGE plpgsql
            AS $$
            BEGIN
              PERFORM pg_sleep(30);
              RETURN NEW;
            END;
            $$;

            DROP TRIGGER IF EXISTS test_delay_import_row_outcome ON crm.records;
            CREATE TRIGGER test_delay_import_row_outcome
            BEFORE UPDATE ON crm.records
            FOR EACH ROW
            WHEN (OLD.record_type = 'customer_data.import_row')
            EXECUTE FUNCTION crm.test_delay_import_row_outcome();
            "#,
        ))
        .await
        .expect("install test-only import outcome delay trigger");
}

async fn drop_import_outcome_delay_trigger(admin: &PgPool) {
    admin
        .execute(sqlx::raw_sql(
            r#"
            DROP TRIGGER IF EXISTS test_delay_import_row_outcome ON crm.records;
            DROP FUNCTION IF EXISTS crm.test_delay_import_row_outcome();
            "#,
        ))
        .await
        .expect("remove test-only import outcome delay trigger");
}

async fn wait_for_party_record(admin: &PgPool, party_id: &str) {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        if party_record_count(admin, party_id).await == 1 {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "governed Party target mutation did not commit before crash deadline"
        );
        sleep(Duration::from_millis(50)).await;
    }
}

async fn party_target_effects(admin: &PgPool, tenant_id: &str) -> PartyTargetEffects {
    PartyTargetEffects {
        records: sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM crm.records WHERE tenant_id = $1 AND owner_module_id = 'crm.parties' AND record_type = 'parties.party' AND deleted_at IS NULL",
        )
        .bind(tenant_id)
        .fetch_one(admin)
        .await
        .expect("count Party records"),
        idempotency: sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM crm.idempotency_records WHERE tenant_id = $1 AND idempotency_scope = 'parties.party.create@1.0.0' AND status = 'completed'",
        )
        .bind(tenant_id)
        .fetch_one(admin)
        .await
        .expect("count Party create idempotency evidence"),
        events: sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM crm.outbox_events WHERE tenant_id = $1 AND event_type = 'parties.party.created'",
        )
        .bind(tenant_id)
        .fetch_one(admin)
        .await
        .expect("count Party create outbox events"),
        audits: sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM crm.audit_records WHERE tenant_id = $1 AND capability_id = 'parties.party.create' AND capability_version = '1.0.0'",
        )
        .bind(tenant_id)
        .fetch_one(admin)
        .await
        .expect("count Party create audit records"),
    }
}

async fn party_record_count(admin: &PgPool, party_id: &str) -> i64 {
    sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM crm.records WHERE tenant_id = $1 AND record_type = 'parties.party' AND record_id = $2 AND deleted_at IS NULL",
    )
    .bind(TENANT_A)
    .bind(party_id)
    .fetch_one(admin)
    .await
    .expect("count exact target Party record")
}

async fn party_create_idempotency_count(admin: &PgPool, _party_id: &str) -> i64 {
    sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM crm.idempotency_records WHERE tenant_id = $1 AND idempotency_scope = 'parties.party.create@1.0.0' AND status = 'completed'",
    )
    .bind(TENANT_A)
    .fetch_one(admin)
    .await
    .expect("count governed Party create idempotency evidence")
}

async fn mutate_message<M: Message>(
    client: &mut ApplicationGatewayServiceClient<tonic::transport::Channel>,
    capability_id: &str,
    message: M,
    tenant_id: &str,
    idempotency_key: &str,
) -> Result<crm_application_runtime::gateway_v1::MutateResponse, Status> {
    let definition = mutation_definition(capability_id);
    mutate(client, &definition, payload(&definition, message), tenant_id, idempotency_key).await
}

async fn query_message<M: Message>(
    client: &mut ApplicationGatewayServiceClient<tonic::transport::Channel>,
    capability_id: &str,
    message: M,
    tenant_id: &str,
) -> Result<crm_application_runtime::gateway_v1::QueryResponse, Status> {
    let definition = query_definition(capability_id);
    query(client, &definition, payload(&definition, message), tenant_id).await
}

async fn mutate(
    client: &mut ApplicationGatewayServiceClient<tonic::transport::Channel>,
    definition: &CapabilityDefinition,
    input: TypedPayload,
    tenant_id: &str,
    idempotency_key: &str,
) -> Result<crm_application_runtime::gateway_v1::MutateResponse, Status> {
    let mut request = Request::new(GatewayMutateRequest {
        owner_module_id: definition.owner_module_id.as_str().to_owned(),
        capability_id: definition.capability_id.as_str().to_owned(),
        capability_version: definition.capability_version.as_str().to_owned(),
        input: Some(wire_payload(input)),
        approval: None,
    });
    request.metadata_mut().insert(
        "x-tenant-id",
        tenant_id.parse().expect("valid tenant metadata"),
    );
    request.metadata_mut().insert(
        "idempotency-key",
        idempotency_key.parse().expect("valid idempotency metadata"),
    );
    request.metadata_mut().insert(
        "authorization",
        format!("Bearer {TOKEN}")
            .parse()
            .expect("valid authorization metadata"),
    );
    client
        .mutate(request)
        .await
        .map(|response| response.into_inner())
}

async fn query(
    client: &mut ApplicationGatewayServiceClient<tonic::transport::Channel>,
    definition: &CapabilityDefinition,
    input: TypedPayload,
    tenant_id: &str,
) -> Result<crm_application_runtime::gateway_v1::QueryResponse, Status> {
    let mut request = Request::new(GatewayQueryRequest {
        owner_module_id: definition.owner_module_id.as_str().to_owned(),
        capability_id: definition.capability_id.as_str().to_owned(),
        capability_version: definition.capability_version.as_str().to_owned(),
        input: Some(wire_payload(input)),
    });
    request.metadata_mut().insert(
        "x-tenant-id",
        tenant_id.parse().expect("valid tenant metadata"),
    );
    request.metadata_mut().insert(
        "authorization",
        format!("Bearer {TOKEN}")
            .parse()
            .expect("valid authorization metadata"),
    );
    client
        .query(request)
        .await
        .map(|response| response.into_inner())
}

fn mutation_definition(capability_id: &str) -> CapabilityDefinition {
    application_mutation_definitions()
        .expect("valid application mutation definitions")
        .into_iter()
        .find(|definition| definition.capability_id.as_str() == capability_id)
        .unwrap_or_else(|| panic!("missing application mutation definition: {capability_id}"))
}

fn query_definition(capability_id: &str) -> CapabilityDefinition {
    application_query_definitions()
        .expect("valid application query definitions")
        .into_iter()
        .find(|definition| definition.capability_id.as_str() == capability_id)
        .unwrap_or_else(|| panic!("missing application query definition: {capability_id}"))
}

fn decode_mutation<M: Message + Default>(
    response: crm_application_runtime::gateway_v1::MutateResponse,
) -> M {
    M::decode(
        response
            .output
            .expect("mutation output")
            .payload
            .as_slice(),
    )
    .expect("decode mutation response")
}

fn decode_query<M: Message + Default>(response: crm_application_runtime::gateway_v1::QueryResponse) -> M {
    M::decode(response.output.expect("query output").payload.as_slice())
        .expect("decode query response")
}

fn parser_profile() -> cdo::ImportParserProfile {
    cdo::ImportParserProfile {
        format: cdo::ImportSourceFormat::Csv as i32,
        encoding: cdo::ImportTextEncoding::Utf8 as i32,
        delimiter_ascii: u32::from(b','),
        quote_ascii: u32::from(b'"'),
        header_mode: cdo::ImportHeaderMode::RequiredFirstRow as i32,
        parser_version: cdo::ImportParserVersion::CsvV1 as i32,
        canonicalization_version: cdo::ImportCanonicalizationVersion::V1 as i32,
    }
}

fn job_ref(job_id: &str) -> cdo::ImportJobRef {
    cdo::ImportJobRef {
        import_job_id: job_id.to_owned(),
    }
}

fn source_ref(source_id: &str) -> cdo::PartyImportSourceArtifactRef {
    cdo::PartyImportSourceArtifactRef {
        file_id: source_id.to_owned(),
    }
}

fn resource_version(job: &cdo::ImportJob) -> i64 {
    job.resource_version
        .as_ref()
        .expect("import job resource version")
        .version
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn payload<M: Message>(definition: &CapabilityDefinition, message: M) -> TypedPayload {
    let data_class = *definition
        .input_contract
        .allowed_data_classes
        .first()
        .expect("governed input contract must declare a data class");
    let payload = TypedPayload {
        owner: definition.input_contract.owner.clone(),
        schema_id: definition.input_contract.schema_id.clone(),
        schema_version: definition.input_contract.schema_version.clone(),
        descriptor_hash: definition.input_contract.descriptor_hash,
        data_class,
        encoding: PayloadEncoding::Protobuf,
        maximum_size_bytes: definition.input_contract.maximum_size_bytes,
        retention_policy_id: RetentionPolicyId::try_new("standard").unwrap(),
        bytes: message.encode_to_vec(),
    };
    payload.validate().expect("valid governed input payload");
    payload
}

fn wire_payload(payload: TypedPayload) -> GatewayTypedPayload {
    GatewayTypedPayload {
        owner_module_id: payload.owner.as_str().to_owned(),
        schema_id: payload.schema_id.as_str().to_owned(),
        schema_version: payload.schema_version.as_str().to_owned(),
        descriptor_hash: payload.descriptor_hash.to_vec(),
        data_class: data_class_name(payload.data_class).to_owned(),
        encoding: "protobuf".to_owned(),
        maximum_size_bytes: payload.maximum_size_bytes,
        retention_policy_id: payload.retention_policy_id.as_str().to_owned(),
        payload: payload.bytes,
    }
}

fn data_class_name(data_class: DataClass) -> &'static str {
    match data_class {
        DataClass::Public => "public",
        DataClass::Internal => "internal",
        DataClass::Confidential => "confidential",
        DataClass::Restricted => "restricted",
        DataClass::Personal => "personal",
        DataClass::SensitivePersonal => "sensitive_personal",
        DataClass::Biometric => "biometric",
        DataClass::Financial => "financial",
        DataClass::Credential => "credential",
    }
}

async fn spawn_api(database_url: &str) -> (Child, String, String) {
    let http_port = free_port();
    let mut grpc_port = free_port();
    while grpc_port == http_port {
        grpc_port = free_port();
    }
    let http_addr = format!("127.0.0.1:{http_port}");
    let grpc_addr = format!("127.0.0.1:{grpc_port}");
    let child = Command::new(env!("CARGO_BIN_EXE_crm-api"))
        .env("CRM_DATABASE_URL", database_url)
        .env("CRM_HTTP_BIND", &http_addr)
        .env("CRM_GRPC_BIND", &grpc_addr)
        .env("CRM_API_BEARER_TOKEN", TOKEN)
        .env("CRM_API_ACTOR_ID", ACTOR)
        .env("CRM_API_TENANTS", format!("{TENANT_A},{TENANT_B}"))
        .env(
            "CRM_CURSOR_SIGNING_KEY",
            "import-process-cursor-signing-key-0123456789abcdef",
        )
        .env(
            "CRM_APPROVAL_SIGNING_KEY",
            "import-process-approval-signing-key-0123456789abcdef",
        )
        .env("CRM_BOOTSTRAP_ALLOW_PHASE6", "true")
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .kill_on_drop(true)
        .spawn()
        .expect("spawn production crm-api process for import acceptance");
    (child, http_addr, grpc_addr)
}

async fn wait_until_ready(client: &reqwest::Client, child: &mut Child, http_addr: &str) {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if let Some(status) = child.try_wait().expect("poll import acceptance crm-api process") {
            panic!("crm-api exited before import acceptance readiness: {status}");
        }
        if let Ok(response) = client
            .get(format!("http://{http_addr}/readyz"))
            .send()
            .await
            && response.status().is_success()
        {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "import acceptance crm-api readiness timed out"
        );
        sleep(Duration::from_millis(200)).await;
    }
}

async fn connect_grpc(
    grpc_addr: &str,
) -> ApplicationGatewayServiceClient<tonic::transport::Channel> {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        match ApplicationGatewayServiceClient::connect(format!("http://{grpc_addr}")).await {
            Ok(client) => return client,
            Err(error) => {
                assert!(
                    Instant::now() < deadline,
                    "import acceptance gRPC listener timed out: {error}"
                );
                sleep(Duration::from_millis(200)).await;
            }
        }
    }
}

async fn send_sigint(child: &Child) {
    let pid = child.id().expect("running crm-api process has a PID");
    let status = Command::new("kill")
        .arg("-INT")
        .arg(pid.to_string())
        .status()
        .await
        .expect("send SIGINT to import acceptance crm-api");
    assert!(status.success(), "kill -INT failed: {status}");
}

fn unique_suffix() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time after Unix epoch")
        .as_nanos()
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("bind ephemeral import acceptance port")
        .local_addr()
        .expect("read ephemeral import acceptance port")
        .port()
}
