use crm_capability_runtime::{
    CapabilityDefinition, CapabilityRequest, CapabilityRisk, PayloadContract,
};
use crm_core_data::{
    AuditIntent, EventEvidence, FaultInjection, FileArtifactCapabilityEvidence,
    FileArtifactCapabilityMutation, IdempotencyEvidence, PostgresDataStore, RecordCreatePlan,
};
use crm_core_files::CreateImmutableFileArtifact;
use crm_module_sdk::{
    ActorId, BusinessTransactionId, CapabilityId, CapabilityVersion, CausationId, CorrelationId,
    DataClass, DomainEvent, ErrorCategory, EventType, ExecutionContext, FileId, IdempotencyKey,
    ModuleExecutionContext, ModuleId, PayloadEncoding, RecordId, RecordRef, RecordType, RequestId,
    ResourceRef, RetentionPolicyId, SchemaId, SchemaVersion, SdkError, TenantId, TraceId,
    TypedPayload,
};
use sqlx::PgPool;

fn context(tenant_id: &str, transaction_id: &str, idempotency_key: &str) -> ModuleExecutionContext {
    ModuleExecutionContext {
        module_id: ModuleId::try_new("crm.test").unwrap(),
        execution: ExecutionContext {
            tenant_id: TenantId::try_new(tenant_id).unwrap(),
            actor_id: ActorId::try_new(if tenant_id == "tenant-a" {
                "actor-a"
            } else {
                "actor-b"
            })
            .unwrap(),
            request_id: RequestId::try_new(format!("request-{transaction_id}")).unwrap(),
            correlation_id: CorrelationId::try_new(format!("correlation-{transaction_id}"))
                .unwrap(),
            causation_id: CausationId::try_new(format!("causation-{transaction_id}")).unwrap(),
            trace_id: TraceId::try_new(format!("trace-{transaction_id}")).unwrap(),
            capability_id: CapabilityId::try_new("test.record.mutate").unwrap(),
            capability_version: CapabilityVersion::try_new("1.0.0").unwrap(),
            idempotency_key: IdempotencyKey::try_new(idempotency_key).unwrap(),
            business_transaction_id: BusinessTransactionId::try_new(transaction_id).unwrap(),
            schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
            request_started_at_unix_nanos: 1_700_000_000_000_000_000,
        },
    }
}

fn payload(value: u8, schema: &str) -> TypedPayload {
    TypedPayload {
        owner: ModuleId::try_new("crm.test").unwrap(),
        schema_id: SchemaId::try_new(schema).unwrap(),
        schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
        descriptor_hash: [value.max(1); 32],
        data_class: DataClass::Internal,
        encoding: PayloadEncoding::Protobuf,
        maximum_size_bytes: 1024,
        retention_policy_id: RetentionPolicyId::try_new("standard").unwrap(),
        bytes: vec![value],
    }
}

fn plan(record_id: &str, transaction_id: &str, idempotency_key: &str) -> RecordCreatePlan {
    let record = RecordRef {
        record_type: RecordType::try_new("test.record").unwrap(),
        record_id: RecordId::try_new(record_id).unwrap(),
    };
    RecordCreatePlan {
        context: context("tenant-a", transaction_id, idempotency_key),
        record: record.clone(),
        record_payload: payload(71, "test.record.v1"),
        event_id: format!("event-{transaction_id}"),
        event: DomainEvent {
            event_type: EventType::try_new("test.record.created").unwrap(),
            aggregate: record,
            expected_aggregate_version: None,
            deduplication_key: format!("dedupe-{transaction_id}"),
            payload: payload(72, "test.record.created.v1"),
        },
        idempotency: IdempotencyEvidence {
            scope: "test.record.mutate@1.0.0".to_owned(),
            key: idempotency_key.to_owned(),
            request_hash: [73; 32],
            expires_at_unix_nanos: 1_800_000_000_000_000_000,
        },
        audit: AuditIntent {
            audit_record_id: format!("audit-{transaction_id}"),
            canonicalization_profile: "crm.cjson/v1".to_owned(),
            canonical_envelope: format!(r#"{{"transaction":"{transaction_id}"}}"#).into_bytes(),
            occurred_at_unix_nanos: 1_700_000_000_000_000_000,
        },
    }
}

#[tokio::test(flavor = "current_thread")]
async fn postgres_adapter_enforces_atomicity_and_tenant_visibility() {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!(
            "skipping PostgreSQL integration scenario because DATABASE_URL is not configured"
        );
        return;
    };
    let store = PostgresDataStore::connect(&database_url, 4)
        .await
        .expect("connect to PostgreSQL");

    let valid = plan("rust-valid-record", "tx-rust-valid", "idem-rust-valid");
    let created = store
        .create_record(&valid)
        .await
        .expect("complete mutation transaction");
    assert_eq!(created.version, 1);

    let visible = store
        .get_record(&valid.context, &valid.record)
        .await
        .expect("read own tenant record");
    assert_eq!(visible, Some(created));

    let other_tenant = context("tenant-b", "tx-rust-read-b", "idem-rust-read-b");
    let hidden = store
        .get_record(&other_tenant, &valid.record)
        .await
        .expect("cross-tenant read must be safely filtered");
    assert_eq!(hidden, None);

    let invalid = plan(
        "rust-invalid-record",
        "tx-rust-invalid",
        "idem-rust-invalid",
    );
    assert!(
        store
            .create_record_with_fault(&invalid, FaultInjection::OmitOutbox)
            .await
            .is_err(),
        "missing outbox evidence must abort the transaction"
    );
    let missing = store
        .get_record(&invalid.context, &invalid.record)
        .await
        .expect("read after failed transaction");
    assert_eq!(missing, None);

    let follow_up = plan(
        "rust-follow-up-record",
        "tx-rust-follow-up",
        "idem-rust-follow-up",
    );
    store
        .create_record(&follow_up)
        .await
        .expect("audit head must roll back with failed transaction");
}

#[tokio::test(flavor = "current_thread")]
async fn file_artifact_capability_commits_business_state_and_evidence_atomically() {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!(
            "skipping PostgreSQL file artifact capability scenario because DATABASE_URL is absent"
        );
        return;
    };
    let admin_database_url = std::env::var("ADMIN_DATABASE_URL")
        .expect("ADMIN_DATABASE_URL must accompany DATABASE_URL");
    let store = PostgresDataStore::connect(&database_url, 4)
        .await
        .expect("connect file artifact capability store");
    let admin = PgPool::connect(&admin_database_url)
        .await
        .expect("connect file artifact evidence reader");
    let actor_bootstrap = async {
        let mut transaction = admin.begin().await?;
        sqlx::query(
            r#"
            SELECT
              set_config('app.tenant_id', 'tenant-b', true),
              set_config('app.actor_id', 'actor-b', true),
              set_config('app.request_id', 'request-file-artifact-actor-bootstrap', true),
              set_config('app.capability_id', 'test.record.mutate', true),
              set_config('app.capability_version', '1.0.0', true),
              set_config('app.business_transaction_id', 'tx-file-artifact-actor-bootstrap', true)
            "#,
        )
        .execute(&mut *transaction)
        .await?;
        sqlx::query(
            "INSERT INTO crm.actors (tenant_id, actor_id, actor_type, status, display_name, last_business_transaction_id) \
             VALUES ($1, $2, 'service', 'active', $3, $4) \
             ON CONFLICT (tenant_id, actor_id) DO NOTHING",
        )
        .bind("tenant-b")
        .bind("actor-b")
        .bind("Tenant B file artifact acceptance actor")
        .bind("tx-file-artifact-actor-bootstrap")
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await
    }
    .await;
    if let Err(error) = actor_bootstrap {
        let diagnostic = format!("actor_bootstrap_error={error:?}\n");
        write_artifact_diagnostic(&diagnostic);
        panic!("bootstrap isolated file artifact acceptance actor failed: {diagnostic}");
    }

    let suffix = std::process::id();
    let file_id = format!("atomic-file-{suffix}");
    let transaction_id = format!("tx-file-artifact-{suffix}");
    let idempotency_key = format!("idem-file-artifact-{suffix}");
    let request = file_capability_request(&transaction_id, &idempotency_key);
    let definition = file_capability_definition();
    let aggregate = RecordRef {
        record_type: RecordType::try_new("file_artifact").unwrap(),
        record_id: RecordId::try_new(file_id.clone()).unwrap(),
    };

    let result = store
        .execute_file_artifact_capability(
            &definition,
            request.clone(),
            FileArtifactCapabilityMutation::Create(CreateImmutableFileArtifact {
                file_id: FileId::try_new(file_id.clone()).unwrap(),
                owner_module_id: ModuleId::try_new("crm.test").unwrap(),
                media_type: "text/csv".to_owned(),
                data_class: DataClass::Internal,
                retention_policy_id: RetentionPolicyId::try_new("standard").unwrap(),
                expected_size_bytes: 3,
                expected_sha256: [91; 32],
            }),
            |mutation, request| {
                let output = payload(92, "test.file.create.response");
                Ok(FileArtifactCapabilityEvidence {
                    output: output.clone(),
                    events: vec![EventEvidence {
                        event_id: format!("event-{transaction_id}"),
                        event: DomainEvent {
                            event_type: EventType::try_new("test.file.created").unwrap(),
                            aggregate: aggregate.clone(),
                            expected_aggregate_version: None,
                            deduplication_key: format!("dedupe-{transaction_id}"),
                            payload: payload(93, "test.file.created.v1"),
                        },
                        aggregate_version: 1,
                        event_sequence: 1,
                        occurred_at_unix_nanos: request
                            .context
                            .execution
                            .request_started_at_unix_nanos,
                    }],
                    audits: vec![AuditIntent {
                        audit_record_id: format!("audit-{transaction_id}"),
                        canonicalization_profile: "crm.cjson/v1".to_owned(),
                        canonical_envelope: br#"{"action":"create_file_artifact"}"#.to_vec(),
                        occurred_at_unix_nanos: request
                            .context
                            .execution
                            .request_started_at_unix_nanos,
                    }],
                    affected_resources: vec![ResourceRef {
                        resource_type: "file_artifact".to_owned(),
                        resource_id: mutation.metadata.file_id.as_str().to_owned(),
                        version: Some(1),
                    }],
                })
            },
        )
        .await;
    let result = match result {
        Ok(result) => result,
        Err(error) => {
            let diagnostic = format!("execute_error={error:?}\n");
            write_artifact_diagnostic(&diagnostic);
            panic!("commit evidenced file artifact capability transaction failed: {diagnostic}");
        }
    };
    assert!(!result.replayed);

    let artifact_count = count(
        &admin,
        "SELECT count(*) FROM crm.file_artifacts WHERE tenant_id = $1 AND file_id = $2",
        "tenant-b",
        &file_id,
    )
    .await;
    let idempotency_count = count(
        &admin,
        "SELECT count(*) FROM crm.idempotency_records WHERE tenant_id = $1 AND business_transaction_id = $2 AND status = 'completed'",
        "tenant-b",
        &transaction_id,
    )
    .await;
    let outbox_count = count(
        &admin,
        "SELECT count(*) FROM crm.outbox_events WHERE tenant_id = $1 AND business_transaction_id = $2",
        "tenant-b",
        &transaction_id,
    )
    .await;
    let audit_count = count(
        &admin,
        "SELECT count(*) FROM crm.audit_records WHERE tenant_id = $1 AND business_transaction_id = $2",
        "tenant-b",
        &transaction_id,
    )
    .await;
    let transaction_count = count(
        &admin,
        "SELECT count(*) FROM crm.business_transactions WHERE tenant_id = $1 AND business_transaction_id = $2 AND expected_outbox_events = 1 AND expected_audit_records = 1 AND expected_idempotency_records = 1",
        "tenant-b",
        &transaction_id,
    )
    .await;
    write_artifact_diagnostic(&format!(
        "stage=committed artifact={artifact_count} idempotency={idempotency_count} outbox={outbox_count} audit={audit_count} transaction={transaction_count}\n"
    ));
    assert_eq!(artifact_count, 1);
    assert_eq!(idempotency_count, 1);
    assert_eq!(outbox_count, 1);
    assert_eq!(audit_count, 1);
    assert_eq!(transaction_count, 1);

    let rollback_file_id = format!("atomic-file-rollback-{suffix}");
    let rollback_transaction_id = format!("tx-file-artifact-rollback-{suffix}");
    let rollback_request = file_capability_request(
        &rollback_transaction_id,
        &format!("idem-file-artifact-rollback-{suffix}"),
    );
    let rollback = store
        .execute_file_artifact_capability(
            &definition,
            rollback_request,
            FileArtifactCapabilityMutation::Create(CreateImmutableFileArtifact {
                file_id: FileId::try_new(rollback_file_id.clone()).unwrap(),
                owner_module_id: ModuleId::try_new("crm.test").unwrap(),
                media_type: "text/csv".to_owned(),
                data_class: DataClass::Internal,
                retention_policy_id: RetentionPolicyId::try_new("standard").unwrap(),
                expected_size_bytes: 3,
                expected_sha256: [94; 32],
            }),
            |_mutation, _request| {
                Err(SdkError::new(
                    "TEST_FILE_EVIDENCE_FAILURE",
                    ErrorCategory::Internal,
                    false,
                    "Synthetic evidence failure.",
                ))
            },
        )
        .await;
    assert!(rollback.is_err());
    assert_eq!(
        count(
            &admin,
            "SELECT count(*) FROM crm.file_artifacts WHERE tenant_id = $1 AND file_id = $2",
            "tenant-b",
            &rollback_file_id,
        )
        .await,
        0
    );
    assert_eq!(
        count(
            &admin,
            "SELECT count(*) FROM crm.idempotency_records WHERE tenant_id = $1 AND business_transaction_id = $2",
            "tenant-b",
            &rollback_transaction_id,
        )
        .await,
        0
    );

    sqlx::query("DELETE FROM crm.file_artifacts WHERE tenant_id = $1 AND file_id = $2")
        .bind("tenant-b")
        .bind(&file_id)
        .execute(&admin)
        .await
        .expect("remove file artifact acceptance fixture before migration rollback");
}

fn file_capability_definition() -> CapabilityDefinition {
    let contract = PayloadContract {
        owner: ModuleId::try_new("crm.test").unwrap(),
        schema_id: SchemaId::try_new("test.file.create.request").unwrap(),
        schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
        descriptor_hash: [95; 32],
        allowed_data_classes: vec![DataClass::Internal],
        allowed_encodings: vec![PayloadEncoding::Protobuf],
        maximum_size_bytes: 1024,
    };
    CapabilityDefinition {
        capability_id: CapabilityId::try_new("test.record.mutate").unwrap(),
        capability_version: CapabilityVersion::try_new("1.0.0").unwrap(),
        owner_module_id: ModuleId::try_new("crm.test").unwrap(),
        input_contract: contract.clone(),
        output_contract: Some(PayloadContract {
            schema_id: SchemaId::try_new("test.file.create.response").unwrap(),
            descriptor_hash: [92; 32],
            ..contract
        }),
        risk: CapabilityRisk::High,
        mutation: true,
        requires_idempotency: true,
        requires_approval: false,
        authorization_policy_id: "test.record.mutate".to_owned(),
        rate_limit_policy_id: None,
    }
}

fn file_capability_request(transaction_id: &str, idempotency_key: &str) -> CapabilityRequest {
    let mut execution = context("tenant-b", transaction_id, idempotency_key);
    execution.execution.capability_id = CapabilityId::try_new("test.record.mutate").unwrap();
    CapabilityRequest {
        context: execution,
        input: TypedPayload {
            owner: ModuleId::try_new("crm.test").unwrap(),
            schema_id: SchemaId::try_new("test.file.create.request").unwrap(),
            schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
            descriptor_hash: [95; 32],
            data_class: DataClass::Internal,
            encoding: PayloadEncoding::Protobuf,
            maximum_size_bytes: 1024,
            retention_policy_id: RetentionPolicyId::try_new("standard").unwrap(),
            bytes: vec![1],
        },
        input_hash: [96; 32],
        approval: None,
    }
}

fn write_artifact_diagnostic(contents: &str) {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/atomic_file_artifact_failure.txt"
    );
    let _ = std::fs::write(path, contents);
}

async fn count(pool: &PgPool, sql: &'static str, first: &str, second: &str) -> i64 {
    sqlx::query_scalar::<_, i64>(sql)
        .bind(first)
        .bind(second)
        .fetch_one(pool)
        .await
        .expect("read PostgreSQL evidence count")
}
