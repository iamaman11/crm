#![cfg(unix)]

use crm_application_runtime::{
    application_mutation_definitions, application_query_definitions,
    gateway_v1::{
        ApprovalEvidence as GatewayApprovalEvidence, MutateRequest as GatewayMutateRequest,
        QueryRequest as GatewayQueryRequest, TypedPayload as GatewayTypedPayload,
        application_gateway_service_client::ApplicationGatewayServiceClient,
    },
};
use crm_capability_adapters::HmacSha256ApprovalVerifier;
use crm_capability_ingress::semantic_input_hash;
use crm_capability_runtime::{ApprovalEvidence, CapabilityDefinition};
use crm_module_sdk::{ActorId, DataClass, PayloadEncoding, RetentionPolicyId, TypedPayload};
use crm_proto_contracts::crm::{
    customer::v1 as customer, customer_data_operations::v1 as cdo, parties::v1 as parties,
};
use prost::Message;
use sha2::{Digest, Sha256};
use sqlx::{Executor, PgPool};
use std::net::TcpListener;
use std::process::Stdio;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::process::{Child, Command};
use tokio::time::sleep;
use tonic::{Request, Status};

const TENANT: &str = "tenant-a";
const ACTOR: &str = "actor-a";
const TOKEN: &str = "export-process-bearer-token-0123456789abcdef0123456789abcdef";
const APPROVAL_KEY: &str = "export-process-approval-signing-key-0123456789abcdef";

const PARTY_CREATE: &str = "parties.party.create";
const EXPORT_CREATE: &str = "customer_data.export.party.create";
const EXPORT_START: &str = "customer_data.export.party.execution.start";
const EXPORT_GET: &str = "customer_data.export.party.get";
const EXPORT_ARTIFACT_DOWNLOAD_CAPABILITY: &str = "customer_data.export.party.artifact.download";

const EXPORT_OUTCOME_RECORD_TYPE: &str = "customer_data.export_execution_outcome";
const CUSTOMER_DATA_OPERATIONS_MODULE_ID: &str = "crm.customer-data-operations";
const EXPORT_COMPLETED_EVENT_TYPE: &str = "customer_data.export.party.completed";
const ARTIFACT_ID_DOMAIN: &[u8] = b"crm.customer-data-operations.party-export-artifact/v1";

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn crm_api_process_recovers_both_party_export_execution_crash_windows() {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!("skipping export process acceptance because DATABASE_URL is absent");
        return;
    };
    let admin_database_url = std::env::var("ADMIN_DATABASE_URL")
        .expect("ADMIN_DATABASE_URL must accompany DATABASE_URL");
    let admin = PgPool::connect(&admin_database_url)
        .await
        .expect("connect export process evidence reader");
    for fixture in [
        include_str!("../../../database/tests/0005_party_adapter.sql"),
        include_str!("../../../database/tests/0013_customer_data_export_adapter.sql"),
    ] {
        admin
            .execute(sqlx::raw_sql(fixture))
            .await
            .expect("publish export process production adapter registry fixture");
    }

    let http_port = free_port();
    let mut grpc_port = free_port();
    while grpc_port == http_port {
        grpc_port = free_port();
    }
    let http_addr = format!("127.0.0.1:{http_port}");
    let grpc_addr = format!("127.0.0.1:{grpc_port}");
    let http = reqwest::Client::new();

    let party_create = mutation_definition(PARTY_CREATE);
    let export_create = mutation_definition(EXPORT_CREATE);
    let export_start = mutation_definition(EXPORT_START);
    let export_get = query_definition(EXPORT_GET);

    let mut child = spawn_crm_api(&database_url, &http_addr, &grpc_addr);
    wait_until_ready(&http, &mut child, &http_addr).await;
    let mut grpc = connect_grpc(&grpc_addr).await;

    let party_id = unique_id("export-party");
    create_party(&mut grpc, &party_create, &party_id).await;

    // Crash window 1: the deterministic artifact row chunk is durable, but the exact execution
    // outcome/checkpoint transaction is still missing. Restart must replay the same chunk and then
    // commit one outcome/checkpoint without duplicate bytes.
    let first_job_id = unique_id("export-chunk-replay-job");
    let created_first = create_export_job(&mut grpc, &export_create, &first_job_id).await;
    assert_eq!(
        created_first.status,
        cdo::PartyExportJobStatus::Created as i32
    );
    let selecting_first = start_export(
        &mut grpc,
        &export_start,
        &first_job_id,
        resource_version(&created_first),
        "export-first-selection-start",
    )
    .await;
    assert_eq!(
        selecting_first.status,
        cdo::PartyExportJobStatus::Selecting as i32
    );
    let ready_first = wait_for_export_status(
        &mut grpc,
        &export_get,
        &first_job_id,
        cdo::PartyExportJobStatus::Ready,
    )
    .await;
    assert_eq!(
        ready_first
            .selection
            .as_ref()
            .expect("first export selection evidence")
            .selected_resources,
        1
    );

    install_outcome_delay_trigger(&admin).await;
    let executing_first = start_export(
        &mut grpc,
        &export_start,
        &first_job_id,
        resource_version(&ready_first),
        "export-first-execution-start",
    )
    .await;
    assert_eq!(
        executing_first.status,
        cdo::PartyExportJobStatus::Executing as i32
    );
    let first_file_id = expected_artifact_file_id(&first_job_id, &ready_first);
    wait_for_artifact_chunk_count(&admin, &first_file_id, 2).await;
    assert_eq!(outcome_record_count(&admin).await, 0);

    force_kill(&mut child).await;
    remove_outcome_delay_trigger(&admin).await;
    assert_eq!(artifact_chunk_count(&admin, &first_file_id).await, 2);
    assert_eq!(outcome_record_count(&admin).await, 0);

    child = spawn_crm_api(&database_url, &http_addr, &grpc_addr);
    wait_until_ready(&http, &mut child, &http_addr).await;
    grpc = connect_grpc(&grpc_addr).await;
    let completed_first = wait_for_export_status(
        &mut grpc,
        &export_get,
        &first_job_id,
        cdo::PartyExportJobStatus::Completed,
    )
    .await;
    assert_completed_single_party_export(&completed_first, &first_file_id);
    assert_eq!(artifact_chunk_count(&admin, &first_file_id).await, 2);
    assert_eq!(
        distinct_artifact_chunk_count(&admin, &first_file_id).await,
        2
    );
    assert_eq!(outcome_record_count(&admin).await, 1);
    assert_eq!(completed_event_count(&admin).await, 1);
    verify_artifact_disclosure(&http, &http_addr, &first_job_id, &completed_first, &admin).await;

    // Crash window 2: the immutable artifact has finalized in its own committed transaction, while
    // the export-job completion transaction is deliberately blocked. Restart must reuse the same
    // finalized artifact metadata and commit the missing completion evidence exactly once.
    let artifacts_before_second = artifact_count(&admin).await;
    let second_job_id = unique_id("export-finalization-recovery-job");
    let created_second = create_export_job(&mut grpc, &export_create, &second_job_id).await;
    let _selecting_second = start_export(
        &mut grpc,
        &export_start,
        &second_job_id,
        resource_version(&created_second),
        "export-second-selection-start",
    )
    .await;
    let ready_second = wait_for_export_status(
        &mut grpc,
        &export_get,
        &second_job_id,
        cdo::PartyExportJobStatus::Ready,
    )
    .await;
    let second_file_id = expected_artifact_file_id(&second_job_id, &ready_second);
    install_completion_delay_trigger(&admin, &second_job_id, &second_file_id).await;
    let executing_second = start_export(
        &mut grpc,
        &export_start,
        &second_job_id,
        resource_version(&ready_second),
        "export-second-execution-start",
    )
    .await;
    assert_eq!(
        executing_second.status,
        cdo::PartyExportJobStatus::Executing as i32
    );
    wait_for_artifact_status(&admin, &second_file_id, "finalized").await;
    assert_eq!(completed_event_count(&admin).await, 1);

    force_kill(&mut child).await;
    remove_completion_delay_trigger(&admin).await;
    assert_eq!(artifact_status(&admin, &second_file_id).await, "finalized");
    assert_eq!(artifact_count(&admin).await, artifacts_before_second + 1);
    assert_eq!(completed_event_count(&admin).await, 1);

    child = spawn_crm_api(&database_url, &http_addr, &grpc_addr);
    wait_until_ready(&http, &mut child, &http_addr).await;
    grpc = connect_grpc(&grpc_addr).await;
    let completed_second = wait_for_export_status(
        &mut grpc,
        &export_get,
        &second_job_id,
        cdo::PartyExportJobStatus::Completed,
    )
    .await;
    assert_completed_single_party_export(&completed_second, &second_file_id);
    assert_eq!(artifact_count(&admin).await, artifacts_before_second + 1);
    assert_eq!(artifact_status(&admin, &second_file_id).await, "finalized");
    assert_eq!(completed_event_count(&admin).await, 2);

    send_sigint(&child).await;
    let status = child
        .wait()
        .await
        .expect("wait for export acceptance crm-api");
    assert!(status.success(), "crm-api exited unsuccessfully: {status}");
}

async fn create_party(
    client: &mut ApplicationGatewayServiceClient<tonic::transport::Channel>,
    definition: &CapabilityDefinition,
    party_id: &str,
) {
    mutate(
        client,
        definition,
        payload(
            definition,
            parties::CreatePartyRequest {
                party_ref: Some(customer::PartyRef {
                    party_id: party_id.to_owned(),
                }),
                kind: parties::PartyKind::Person as i32,
                display_name: "Export Recovery Subject".to_owned(),
            },
        ),
        TENANT,
        &unique_id("export-create-party"),
        None,
    )
    .await
    .expect("create Party prerequisite through production gateway");
}

async fn create_export_job(
    client: &mut ApplicationGatewayServiceClient<tonic::transport::Channel>,
    definition: &CapabilityDefinition,
    job_id: &str,
) -> cdo::PartyExportJob {
    let response = mutate(
        client,
        definition,
        payload(
            definition,
            cdo::CreatePartyExportJobRequest {
                export_job_ref: Some(cdo::ExportJobRef {
                    export_job_id: job_id.to_owned(),
                }),
                specification: Some(cdo::PartyExportSpecification {
                    scope: Some(cdo::PartyExportScope {
                        kind: Some(parties::PartyKind::Person as i32),
                        maximum_resources: 1,
                    }),
                    profile: Some(cdo::PartyExportProfile {
                        profile_version: cdo::PartyExportProfileVersion::V1 as i32,
                        format: cdo::PartyExportFormat::CsvUtf8 as i32,
                        canonicalization_version: cdo::PartyExportCanonicalizationVersion::V1
                            as i32,
                        fields: vec![
                            cdo::PartyExportField::PartyId as i32,
                            cdo::PartyExportField::Kind as i32,
                            cdo::PartyExportField::DisplayName as i32,
                            cdo::PartyExportField::ResourceVersion as i32,
                        ],
                        retention_policy_id: "standard".to_owned(),
                    }),
                }),
            },
        ),
        TENANT,
        &unique_id("export-create-job"),
        None,
    )
    .await
    .expect("create Party export job through production gateway");
    cdo::CreatePartyExportJobResponse::decode(response.output.unwrap().payload.as_slice())
        .expect("decode create Party export response")
        .export_job
        .expect("created Party export job")
}

async fn start_export(
    client: &mut ApplicationGatewayServiceClient<tonic::transport::Channel>,
    definition: &CapabilityDefinition,
    job_id: &str,
    expected_version: i64,
    idempotency_prefix: &str,
) -> cdo::PartyExportJob {
    let input = payload(
        definition,
        cdo::StartPartyExportExecutionRequest {
            export_job_ref: Some(cdo::ExportJobRef {
                export_job_id: job_id.to_owned(),
            }),
            expected_version,
        },
    );
    let approval = signed_gateway_approval(definition, &input);
    let response = mutate(
        client,
        definition,
        input,
        TENANT,
        &unique_id(idempotency_prefix),
        Some(approval),
    )
    .await
    .expect("start Party export through approved production gateway");
    cdo::StartPartyExportExecutionResponse::decode(response.output.unwrap().payload.as_slice())
        .expect("decode start Party export response")
        .export_job
        .expect("started Party export job")
}

async fn wait_for_export_status(
    client: &mut ApplicationGatewayServiceClient<tonic::transport::Channel>,
    definition: &CapabilityDefinition,
    job_id: &str,
    expected_status: cdo::PartyExportJobStatus,
) -> cdo::PartyExportJob {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let response = query(
            client,
            definition,
            payload(
                definition,
                cdo::GetPartyExportJobRequest {
                    export_job_ref: Some(cdo::ExportJobRef {
                        export_job_id: job_id.to_owned(),
                    }),
                },
            ),
            TENANT,
        )
        .await
        .expect("query Party export job through production gateway");
        let job =
            cdo::GetPartyExportJobResponse::decode(response.output.unwrap().payload.as_slice())
                .expect("decode Party export get response")
                .export_job
                .expect("queried Party export job");
        if job.status == expected_status as i32 {
            return job;
        }
        assert!(
            Instant::now() < deadline,
            "Party export job {job_id} did not reach {expected_status:?}; last status={}",
            job.status
        );
        sleep(Duration::from_millis(200)).await;
    }
}

fn assert_completed_single_party_export(job: &cdo::PartyExportJob, expected_file_id: &str) {
    assert_eq!(job.status, cdo::PartyExportJobStatus::Completed as i32);
    assert_eq!(job.checkpoint_manifest_position, 1);
    let selection = job.selection.as_ref().expect("completed export selection");
    assert_eq!(selection.selected_resources, 1);
    let artifact = job.artifact.as_ref().expect("completed export artifact");
    assert_eq!(artifact.file_id, expected_file_id);
    assert_eq!(artifact.media_type, "text/csv; charset=utf-8");
    assert_eq!(artifact.content_sha256.len(), 32);
    assert_eq!(artifact.retention_policy_id, "standard");
    let reconciliation = job
        .reconciliation
        .as_ref()
        .expect("completed export reconciliation");
    assert_eq!(reconciliation.selected_resources, 1);
    assert_eq!(reconciliation.emitted_rows, 1);
    assert_eq!(reconciliation.excluded_not_visible, 0);
    assert_eq!(reconciliation.excluded_version_changed, 0);
    assert_eq!(reconciliation.excluded_unavailable, 0);
}

fn resource_version(job: &cdo::PartyExportJob) -> i64 {
    job.resource_version
        .as_ref()
        .expect("Party export resource version")
        .version
}

fn expected_artifact_file_id(job_id: &str, job: &cdo::PartyExportJob) -> String {
    let selection = job.selection.as_ref().expect("ready export selection");
    assert_eq!(selection.manifest_sha256.len(), 32);
    let manifest_sha256 = hex(&selection.manifest_sha256);
    let mut hasher = Sha256::new();
    hasher.update(ARTIFACT_ID_DOMAIN);
    hash_part(&mut hasher, job_id.as_bytes());
    hash_part(&mut hasher, job.export_specification_version_id.as_bytes());
    hash_part(&mut hasher, manifest_sha256.as_bytes());
    format!("cdo-export-artifact-{}", hex(&hasher.finalize()))
}

async fn verify_artifact_disclosure(
    http: &reqwest::Client,
    http_addr: &str,
    job_id: &str,
    job: &cdo::PartyExportJob,
    admin: &PgPool,
) {
    let url = format!("http://{http_addr}/v1/customer-data/exports/{job_id}/artifact");
    let audit_before = disclosure_audit_count(admin).await;

    let unauthenticated = http
        .get(&url)
        .header("x-tenant-id", TENANT)
        .send()
        .await
        .expect("request unauthenticated export artifact disclosure");
    assert_eq!(unauthenticated.status(), reqwest::StatusCode::UNAUTHORIZED);
    assert_eq!(disclosure_audit_count(admin).await, audit_before);

    let forbidden_tenant = http
        .get(&url)
        .bearer_auth(TOKEN)
        .header("x-tenant-id", "tenant-b")
        .send()
        .await
        .expect("request cross-tenant export artifact disclosure");
    assert_eq!(forbidden_tenant.status(), reqwest::StatusCode::FORBIDDEN);
    assert_eq!(disclosure_audit_count(admin).await, audit_before);

    let request_id = unique_id("export-artifact-disclosure");
    let response = http
        .get(&url)
        .bearer_auth(TOKEN)
        .header("x-tenant-id", TENANT)
        .header("x-request-id", request_id)
        .send()
        .await
        .expect("download governed Party export artifact");
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    let headers = response.headers().clone();
    let artifact = job
        .artifact
        .as_ref()
        .expect("completed export artifact evidence");
    let expected_sha256 = hex(&artifact.content_sha256);
    assert_eq!(
        headers.get(reqwest::header::CONTENT_TYPE).unwrap(),
        "text/csv; charset=utf-8"
    );
    assert_eq!(
        headers
            .get(reqwest::header::CONTENT_LENGTH)
            .unwrap()
            .to_str()
            .unwrap(),
        artifact.size_bytes.to_string().as_str()
    );
    assert_eq!(
        headers.get(reqwest::header::CACHE_CONTROL).unwrap(),
        "private, no-store"
    );
    assert_eq!(
        headers
            .get(reqwest::header::ETAG)
            .unwrap()
            .to_str()
            .unwrap(),
        format!("\"sha256-{expected_sha256}\"").as_str()
    );
    assert_eq!(
        headers.get("x-content-sha256").unwrap().to_str().unwrap(),
        expected_sha256.as_str()
    );
    assert_eq!(headers.get("x-content-type-options").unwrap(), "nosniff");
    let bytes = response
        .bytes()
        .await
        .expect("read governed Party export bytes");
    assert_eq!(bytes.len() as u64, artifact.size_bytes);
    assert_eq!(
        Sha256::digest(&bytes).as_slice(),
        artifact.content_sha256.as_slice()
    );
    assert!(bytes.starts_with(b"party_id,"));
    assert_eq!(disclosure_audit_count(admin).await, audit_before + 1);
}

async fn install_outcome_delay_trigger(admin: &PgPool) {
    admin
        .execute(sqlx::raw_sql(
            r#"
            CREATE OR REPLACE FUNCTION crm.test_delay_export_outcome_insert()
            RETURNS trigger
            LANGUAGE plpgsql
            AS $$
            BEGIN
              IF NEW.record_type = 'customer_data.export_execution_outcome' THEN
                PERFORM pg_sleep(30);
              END IF;
              RETURN NEW;
            END;
            $$;
            DROP TRIGGER IF EXISTS test_delay_export_outcome_insert ON crm.records;
            CREATE TRIGGER test_delay_export_outcome_insert
            BEFORE INSERT ON crm.records
            FOR EACH ROW
            EXECUTE FUNCTION crm.test_delay_export_outcome_insert();
            "#,
        ))
        .await
        .expect("install export outcome crash-window trigger");
}

async fn remove_outcome_delay_trigger(admin: &PgPool) {
    admin
        .execute(sqlx::raw_sql(
            r#"
            DROP TRIGGER IF EXISTS test_delay_export_outcome_insert ON crm.records;
            DROP FUNCTION IF EXISTS crm.test_delay_export_outcome_insert();
            "#,
        ))
        .await
        .expect("remove export outcome crash-window trigger");
}

async fn install_completion_delay_trigger(admin: &PgPool, job_id: &str, file_id: &str) {
    admin
        .execute(sqlx::raw_sql(
            r#"
            CREATE TABLE IF NOT EXISTS crm.test_export_completion_delay_target (
              job_id text PRIMARY KEY,
              file_id text NOT NULL
            );
            GRANT SELECT ON crm.test_export_completion_delay_target TO crm_app_test;
            TRUNCATE TABLE crm.test_export_completion_delay_target;
            CREATE OR REPLACE FUNCTION crm.test_delay_export_completion_update()
            RETURNS trigger
            LANGUAGE plpgsql
            AS $$
            BEGIN
              IF EXISTS (
                SELECT 1
                  FROM crm.test_export_completion_delay_target AS target
                  JOIN crm.file_artifacts AS artifact
                    ON artifact.tenant_id = NEW.tenant_id
                   AND artifact.file_id = target.file_id
                   AND artifact.owner_module_id = 'crm.customer-data-operations'
                   AND artifact.status = 'finalized'
                 WHERE target.job_id = NEW.record_id
              )
              THEN
                PERFORM pg_sleep(30);
              END IF;
              RETURN NEW;
            END;
            $$;
            DROP TRIGGER IF EXISTS test_delay_export_completion_update ON crm.records;
            CREATE TRIGGER test_delay_export_completion_update
            BEFORE UPDATE ON crm.records
            FOR EACH ROW
            WHEN (OLD.record_type = 'customer_data.export_job')
            EXECUTE FUNCTION crm.test_delay_export_completion_update();
            "#,
        ))
        .await
        .expect("install export completion crash-window trigger");
    sqlx::query(
        "INSERT INTO crm.test_export_completion_delay_target (job_id, file_id) VALUES ($1, $2)",
    )
    .bind(job_id)
    .bind(file_id)
    .execute(admin)
    .await
    .expect("bind exact export completion crash-window target");
}

async fn remove_completion_delay_trigger(admin: &PgPool) {
    admin
        .execute(sqlx::raw_sql(
            r#"
            DROP TRIGGER IF EXISTS test_delay_export_completion_update ON crm.records;
            DROP FUNCTION IF EXISTS crm.test_delay_export_completion_update();
            DROP TABLE IF EXISTS crm.test_export_completion_delay_target;
            "#,
        ))
        .await
        .expect("remove export completion crash-window trigger");
}

async fn wait_for_artifact_chunk_count(admin: &PgPool, file_id: &str, expected: i64) {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if artifact_chunk_count(admin, file_id).await >= expected {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "artifact {file_id} did not reach {expected} durable chunks"
        );
        sleep(Duration::from_millis(100)).await;
    }
}

async fn wait_for_artifact_status(admin: &PgPool, file_id: &str, expected: &str) {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let status = artifact_status_optional(admin, file_id).await;
        if status.as_deref() == Some(expected) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "artifact {file_id} did not reach status {expected}; last status={status:?}"
        );
        sleep(Duration::from_millis(100)).await;
    }
}

async fn artifact_chunk_count(admin: &PgPool, file_id: &str) -> i64 {
    sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM crm.file_artifact_chunks WHERE tenant_id = $1 AND file_id = $2",
    )
    .bind(TENANT)
    .bind(file_id)
    .fetch_one(admin)
    .await
    .expect("count export artifact chunks")
}

async fn distinct_artifact_chunk_count(admin: &PgPool, file_id: &str) -> i64 {
    sqlx::query_scalar::<_, i64>(
        "SELECT count(DISTINCT chunk_index) FROM crm.file_artifact_chunks WHERE tenant_id = $1 AND file_id = $2",
    )
    .bind(TENANT)
    .bind(file_id)
    .fetch_one(admin)
    .await
    .expect("count distinct export artifact chunks")
}

async fn artifact_count(admin: &PgPool) -> i64 {
    sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM crm.file_artifacts WHERE tenant_id = $1 AND owner_module_id = $2",
    )
    .bind(TENANT)
    .bind(CUSTOMER_DATA_OPERATIONS_MODULE_ID)
    .fetch_one(admin)
    .await
    .expect("count customer-data export artifacts")
}

async fn artifact_status(admin: &PgPool, file_id: &str) -> String {
    artifact_status_optional(admin, file_id)
        .await
        .expect("export artifact exists")
}

async fn artifact_status_optional(admin: &PgPool, file_id: &str) -> Option<String> {
    sqlx::query_scalar::<_, String>(
        "SELECT status FROM crm.file_artifacts WHERE tenant_id = $1 AND file_id = $2",
    )
    .bind(TENANT)
    .bind(file_id)
    .fetch_optional(admin)
    .await
    .expect("read export artifact status")
}

async fn outcome_record_count(admin: &PgPool) -> i64 {
    sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM crm.records WHERE tenant_id = $1 AND owner_module_id = $2 AND record_type = $3 AND deleted_at IS NULL",
    )
    .bind(TENANT)
    .bind(CUSTOMER_DATA_OPERATIONS_MODULE_ID)
    .bind(EXPORT_OUTCOME_RECORD_TYPE)
    .fetch_one(admin)
    .await
    .expect("count export execution outcomes")
}

async fn disclosure_audit_count(admin: &PgPool) -> i64 {
    sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM crm.audit_records WHERE tenant_id = $1 AND capability_id = $2",
    )
    .bind(TENANT)
    .bind(EXPORT_ARTIFACT_DOWNLOAD_CAPABILITY)
    .fetch_one(admin)
    .await
    .expect("count export artifact disclosure audits")
}

async fn completed_event_count(admin: &PgPool) -> i64 {
    sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM crm.outbox_events WHERE tenant_id = $1 AND event_type = $2",
    )
    .bind(TENANT)
    .bind(EXPORT_COMPLETED_EVENT_TYPE)
    .fetch_one(admin)
    .await
    .expect("count export completion events")
}

fn signed_gateway_approval(
    definition: &CapabilityDefinition,
    input: &TypedPayload,
) -> GatewayApprovalEvidence {
    let verifier = HmacSha256ApprovalVerifier::try_new(APPROVAL_KEY.as_bytes().to_vec())
        .expect("valid export acceptance approval signing key");
    let mut approval = ApprovalEvidence {
        approval_id: unique_id("export-approval"),
        actor_id: ActorId::try_new(ACTOR).expect("valid export acceptance actor"),
        capability_id: definition.capability_id.clone(),
        capability_version: definition.capability_version.clone(),
        input_hash: semantic_input_hash(input),
        policy_version: "customer-data-export-approval/v1".to_owned(),
        expires_at_unix_nanos: now_nanos() + 300_000_000_000,
        opaque_proof: Vec::new(),
    };
    approval.opaque_proof = verifier.sign(&approval);
    GatewayApprovalEvidence {
        approval_id: approval.approval_id,
        actor_id: approval.actor_id.as_str().to_owned(),
        capability_id: approval.capability_id.as_str().to_owned(),
        capability_version: approval.capability_version.as_str().to_owned(),
        input_hash: approval.input_hash.to_vec(),
        policy_version: approval.policy_version,
        expires_at_unix_nanos: approval.expires_at_unix_nanos,
        opaque_proof: approval.opaque_proof,
    }
}

async fn mutate(
    client: &mut ApplicationGatewayServiceClient<tonic::transport::Channel>,
    definition: &CapabilityDefinition,
    input: TypedPayload,
    tenant_id: &str,
    idempotency_key: &str,
    approval: Option<GatewayApprovalEvidence>,
) -> Result<crm_application_runtime::gateway_v1::MutateResponse, Status> {
    let mut request = Request::new(GatewayMutateRequest {
        owner_module_id: definition.owner_module_id.as_str().to_owned(),
        capability_id: definition.capability_id.as_str().to_owned(),
        capability_version: definition.capability_version.as_str().to_owned(),
        input: Some(wire_payload(input)),
        approval,
    });
    request
        .metadata_mut()
        .insert("x-tenant-id", tenant_id.parse().unwrap());
    request
        .metadata_mut()
        .insert("idempotency-key", idempotency_key.parse().unwrap());
    request
        .metadata_mut()
        .insert("authorization", format!("Bearer {TOKEN}").parse().unwrap());
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
    request
        .metadata_mut()
        .insert("x-tenant-id", tenant_id.parse().unwrap());
    request
        .metadata_mut()
        .insert("authorization", format!("Bearer {TOKEN}").parse().unwrap());
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

fn payload<M: Message>(definition: &CapabilityDefinition, message: M) -> TypedPayload {
    let data_class = *definition
        .input_contract
        .allowed_data_classes
        .first()
        .expect("capability input data class");
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

fn spawn_crm_api(database_url: &str, http_addr: &str, grpc_addr: &str) -> Child {
    Command::new(env!("CARGO_BIN_EXE_crm-api"))
        .env("CRM_DATABASE_URL", database_url)
        .env("CRM_HTTP_BIND", http_addr)
        .env("CRM_GRPC_BIND", grpc_addr)
        .env("CRM_API_BEARER_TOKEN", TOKEN)
        .env("CRM_API_ACTOR_ID", ACTOR)
        .env("CRM_API_TENANTS", TENANT)
        .env(
            "CRM_CURSOR_SIGNING_KEY",
            "export-process-cursor-signing-key-0123456789abcdef",
        )
        .env("CRM_APPROVAL_SIGNING_KEY", APPROVAL_KEY)
        .env("CRM_BOOTSTRAP_ALLOW_PHASE6", "true")
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .kill_on_drop(true)
        .spawn()
        .expect("spawn production crm-api process for export acceptance")
}

async fn wait_until_ready(client: &reqwest::Client, child: &mut Child, http_addr: &str) {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if let Some(status) = child.try_wait().expect("poll export acceptance crm-api") {
            panic!("crm-api exited before export acceptance readiness: {status}");
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
            "export acceptance crm-api readiness timed out"
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
                    "export acceptance gRPC listener timed out: {error}"
                );
                sleep(Duration::from_millis(200)).await;
            }
        }
    }
}

async fn force_kill(child: &mut Child) {
    child
        .kill()
        .await
        .expect("force-kill export acceptance crm-api");
    let _ = child.wait().await;
}

async fn send_sigint(child: &Child) {
    let pid = child
        .id()
        .expect("running export acceptance crm-api has a PID");
    let status = Command::new("kill")
        .arg("-INT")
        .arg(pid.to_string())
        .status()
        .await
        .expect("send SIGINT to export acceptance crm-api");
    assert!(status.success(), "kill -INT failed: {status}");
}

fn hash_part(hasher: &mut Sha256, value: &[u8]) {
    hasher.update((value.len() as u64).to_be_bytes());
    hasher.update(value);
}

fn hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        write!(&mut output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("bind ephemeral export acceptance port")
        .local_addr()
        .expect("read ephemeral export acceptance port")
        .port()
}

fn unique_id(prefix: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time after Unix epoch")
        .as_nanos();
    format!("{prefix}-{}-{nanos}", std::process::id())
}

fn now_nanos() -> i64 {
    i64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time after Unix epoch")
            .as_nanos(),
    )
    .expect("current Unix nanos fit i64")
}
