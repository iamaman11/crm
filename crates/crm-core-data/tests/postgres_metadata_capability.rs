#![cfg(feature = "postgres-integration")]

use crm_capability_runtime::{
    CapabilityRequest, TransactionalCapabilityExecutor,
};
use crm_core_data::{PostgresDataStore, PostgresMetadataCapabilityExecutor};
use crm_metadata_api_adapter::{
    METADATA_MODULE_ID, PUBLISH_BUNDLE_CAPABILITY, PUBLISH_REQUEST_SCHEMA,
    metadata_capability_definition, protobuf_payload,
};
use crm_metadata_schema::METADATA_DEFINITION_SCHEMA_VERSION;
use crm_module_sdk::{
    ActorId, BusinessTransactionId, CapabilityId, CapabilityVersion, CausationId,
    CorrelationId, DataClass, ExecutionContext, IdempotencyKey, ModuleExecutionContext, ModuleId,
    RequestId, SchemaVersion, TenantId, TraceId,
};
use crm_proto_contracts::crm::metadata::v1 as wire;
use prost::Message;
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Row};

#[tokio::test(flavor = "current_thread")]
async fn governed_metadata_publish_is_atomic_audited_and_replay_safe() {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!("skipping PostgreSQL metadata capability acceptance because DATABASE_URL is absent");
        return;
    };
    let admin_database_url = std::env::var("ADMIN_DATABASE_URL")
        .expect("ADMIN_DATABASE_URL must accompany DATABASE_URL");
    let store = PostgresDataStore::connect(&database_url, 8)
        .await
        .expect("connect metadata capability executor");
    let executor = PostgresMetadataCapabilityExecutor::new(store);
    let admin = PgPool::connect(&admin_database_url)
        .await
        .expect("connect metadata capability evidence reader");
    cleanup(&admin).await;

    let definition = metadata_capability_definition(PUBLISH_BUNDLE_CAPABILITY).unwrap();
    let request = publish_request("publish-v1");

    let first = executor
        .execute(&definition, request.clone())
        .await
        .expect("execute governed metadata publish");
    assert!(!first.replayed);
    let response = wire::PublishMetadataBundleResponse::decode(
        first.output.as_ref().unwrap().bytes.as_slice(),
    )
    .unwrap();
    assert!(response.newly_published);
    assert_eq!(response.revision_id.len(), 64);
    assert_eq!(
        first.affected_resources[0].resource_type,
        "metadata.revision"
    );
    assert_eq!(first.affected_resources[0].resource_id, response.revision_id);

    assert_eq!(evidence_counts(&admin).await, (1, 1, 1, 1, 0));
    let marker = sqlx::query(
        r#"
        SELECT expected_outbox_events, expected_audit_records, expected_idempotency_records
        FROM crm.business_transactions
        WHERE tenant_id = 'tenant-a' AND business_transaction_id = 'tx-publish-v1'
        "#,
    )
    .fetch_one(&admin)
    .await
    .expect("load metadata business transaction marker");
    assert_eq!(marker.try_get::<i32, _>("expected_outbox_events").unwrap(), 0);
    assert_eq!(marker.try_get::<i32, _>("expected_audit_records").unwrap(), 1);
    assert_eq!(
        marker
            .try_get::<i32, _>("expected_idempotency_records")
            .unwrap(),
        1
    );

    let audit = sqlx::query(
        r#"
        SELECT canonicalization_profile, canonical_envelope
        FROM crm.audit_records
        WHERE tenant_id = 'tenant-a'
        "#,
    )
    .fetch_one(&admin)
    .await
    .expect("load canonical metadata audit evidence");
    assert_eq!(
        audit
            .try_get::<String, _>("canonicalization_profile")
            .unwrap(),
        "crm.cjson/v1"
    );
    let envelope: serde_json::Value = serde_json::from_slice(
        &audit
            .try_get::<Vec<u8>, _>("canonical_envelope")
            .unwrap(),
    )
    .unwrap();
    assert_eq!(envelope["capability_id"], PUBLISH_BUNDLE_CAPABILITY);
    assert_eq!(envelope["aggregate_type"], "metadata.revision");
    assert_eq!(envelope["transaction_id"], "tx-publish-v1");

    let replay = executor
        .execute(&definition, request)
        .await
        .expect("replay governed metadata publish");
    assert!(replay.replayed);
    assert_eq!(replay.output, first.output);
    assert_eq!(replay.affected_resources, first.affected_resources);
    assert_eq!(
        evidence_counts(&admin).await,
        (1, 1, 1, 1, 0),
        "idempotent replay must not duplicate metadata, audit, idempotency, transaction or outbox evidence"
    );
}

fn publish_request(suffix: &str) -> CapabilityRequest {
    let input = protobuf_payload(
        METADATA_MODULE_ID,
        PUBLISH_REQUEST_SCHEMA,
        DataClass::Confidential,
        &wire::PublishMetadataBundleRequest {
            definitions: vec![wire::MetadataDefinitionInput {
                schema_version: METADATA_DEFINITION_SCHEMA_VERSION.to_owned(),
                definition_json: br#"{"kind":"object","definition":{"id":"crm.sales.deal","owner_module_id":"crm.sales","label":"Deal","plural_label":"Deals","description":null,"tags":["sales"]}}"#.to_vec(),
            }],
        },
    )
    .unwrap();
    let input_hash: [u8; 32] = Sha256::digest(&input.bytes).into();
    CapabilityRequest {
        context: ModuleExecutionContext {
            module_id: ModuleId::try_new(METADATA_MODULE_ID).unwrap(),
            execution: ExecutionContext {
                tenant_id: TenantId::try_new("tenant-a").unwrap(),
                actor_id: ActorId::try_new("actor-a").unwrap(),
                request_id: RequestId::try_new(format!("request-{suffix}")).unwrap(),
                correlation_id: CorrelationId::try_new(format!("correlation-{suffix}")).unwrap(),
                causation_id: CausationId::try_new(format!("causation-{suffix}")).unwrap(),
                trace_id: TraceId::try_new(format!("trace-{suffix}")).unwrap(),
                capability_id: CapabilityId::try_new(PUBLISH_BUNDLE_CAPABILITY).unwrap(),
                capability_version: CapabilityVersion::try_new("1.0.0").unwrap(),
                idempotency_key: IdempotencyKey::try_new(format!("idempotency-{suffix}")).unwrap(),
                business_transaction_id: BusinessTransactionId::try_new(format!("tx-{suffix}"))
                    .unwrap(),
                schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
                request_started_at_unix_nanos: 1_000_000_000,
            },
        },
        input,
        input_hash,
        approval: None,
    }
}

async fn cleanup(admin: &PgPool) {
    sqlx::query(
        r#"
        TRUNCATE TABLE
          crm.metadata_transitions,
          crm.metadata_rollback_stack,
          crm.metadata_activation_heads,
          crm.metadata_revision_dependencies,
          crm.metadata_revision_documents,
          crm.metadata_revisions_v2,
          crm.outbox_events,
          crm.audit_records,
          crm.audit_heads,
          crm.idempotency_records,
          crm.business_transactions
        CASCADE
        "#,
    )
    .execute(admin)
    .await
    .expect("truncate metadata capability acceptance fixtures");
}

async fn evidence_counts(admin: &PgPool) -> (i64, i64, i64, i64, i64) {
    let row = sqlx::query(
        r#"
        SELECT
          (SELECT count(*) FROM crm.metadata_transitions) AS transition_count,
          (SELECT count(*) FROM crm.audit_records) AS audit_count,
          (SELECT count(*) FROM crm.idempotency_records) AS idempotency_count,
          (SELECT count(*) FROM crm.business_transactions) AS transaction_count,
          (SELECT count(*) FROM crm.outbox_events) AS outbox_count
        "#,
    )
    .fetch_one(admin)
    .await
    .expect("read metadata capability evidence counts");
    (
        row.try_get("transition_count").unwrap(),
        row.try_get("audit_count").unwrap(),
        row.try_get("idempotency_count").unwrap(),
        row.try_get("transaction_count").unwrap(),
        row.try_get("outbox_count").unwrap(),
    )
}
