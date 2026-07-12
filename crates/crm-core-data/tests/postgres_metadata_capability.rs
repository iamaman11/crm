#![cfg(feature = "postgres-integration")]

use crm_capability_runtime::{CapabilityRequest, TransactionalCapabilityExecutor};
use crm_core_data::{PostgresDataStore, PostgresMetadataCapabilityExecutor};
use crm_metadata_api_adapter::{
    ACTIVATE_REQUEST_SCHEMA, ACTIVATE_REVISION_CAPABILITY, METADATA_MODULE_ID,
    PUBLISH_BUNDLE_CAPABILITY, PUBLISH_REQUEST_SCHEMA, ROLLBACK_REQUEST_SCHEMA,
    ROLLBACK_REVISION_CAPABILITY, metadata_capability_definition, protobuf_payload,
};
use crm_metadata_schema::METADATA_DEFINITION_SCHEMA_VERSION;
use crm_module_sdk::{
    ActorId, BusinessTransactionId, CapabilityId, CapabilityVersion, CausationId, CorrelationId,
    DataClass, ExecutionContext, IdempotencyKey, ModuleExecutionContext, ModuleId, RequestId,
    SchemaVersion, TenantId, TraceId,
};
use crm_proto_contracts::crm::metadata::v1 as wire;
use prost::Message;
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Row};

#[tokio::test(flavor = "current_thread")]
async fn governed_metadata_lifecycle_is_atomic_audited_replay_safe_and_pop_only() {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!(
            "skipping PostgreSQL metadata capability acceptance because DATABASE_URL is absent"
        );
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

    let publish_definition = metadata_capability_definition(PUBLISH_BUNDLE_CAPABILITY).unwrap();
    let publish_v1 = request(
        PUBLISH_BUNDLE_CAPABILITY,
        PUBLISH_REQUEST_SCHEMA,
        &wire::PublishMetadataBundleRequest {
            definitions: bundle_v1_definitions(),
        },
        "publish-v1",
        1_000_000_000,
    );

    let first = executor
        .execute(&publish_definition, publish_v1.clone())
        .await
        .expect("execute governed metadata publish v1");
    assert!(!first.replayed);
    let published_v1 = wire::PublishMetadataBundleResponse::decode(
        first.output.as_ref().unwrap().bytes.as_slice(),
    )
    .unwrap();
    assert!(published_v1.newly_published);
    assert_eq!(published_v1.revision_id.len(), 64);
    assert_eq!(
        first.affected_resources[0].resource_type,
        "metadata.revision"
    );
    assert_eq!(
        first.affected_resources[0].resource_id,
        published_v1.revision_id
    );
    assert_eq!(evidence_counts(&admin).await, (1, 1, 1, 1, 0));
    assert_transaction_marker(&admin, "tx-publish-v1").await;
    assert_latest_audit(
        &admin,
        PUBLISH_BUNDLE_CAPABILITY,
        "metadata.revision",
        "tx-publish-v1",
    )
    .await;

    let replay = executor
        .execute(&publish_definition, publish_v1)
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

    let activate_definition = metadata_capability_definition(ACTIVATE_REVISION_CAPABILITY).unwrap();
    let activate_v1 = executor
        .execute(
            &activate_definition,
            request(
                ACTIVATE_REVISION_CAPABILITY,
                ACTIVATE_REQUEST_SCHEMA,
                &wire::ActivateMetadataRevisionRequest {
                    revision_id: published_v1.revision_id.clone(),
                    expected_generation: 0,
                    confirm_breaking_changes: false,
                },
                "activate-v1",
                2_000_000_000,
            ),
        )
        .await
        .expect("activate metadata revision v1");
    let activated_v1 = wire::ActivateMetadataRevisionResponse::decode(
        activate_v1.output.as_ref().unwrap().bytes.as_slice(),
    )
    .unwrap();
    let state_v1 = activated_v1.state.expect("activation state v1");
    assert_eq!(state_v1.generation, 1);
    assert_eq!(state_v1.rollback_depth, 0);
    assert_eq!(state_v1.active_revision_id, published_v1.revision_id);
    assert_eq!(evidence_counts(&admin).await, (2, 2, 2, 2, 0));

    let publish_v2 = executor
        .execute(
            &publish_definition,
            request(
                PUBLISH_BUNDLE_CAPABILITY,
                PUBLISH_REQUEST_SCHEMA,
                &wire::PublishMetadataBundleRequest {
                    definitions: bundle_v2_definitions(),
                },
                "publish-v2",
                3_000_000_000,
            ),
        )
        .await
        .expect("publish breaking metadata revision v2");
    let published_v2 = wire::PublishMetadataBundleResponse::decode(
        publish_v2.output.as_ref().unwrap().bytes.as_slice(),
    )
    .unwrap();
    assert!(published_v2.newly_published);
    assert_ne!(published_v2.revision_id, published_v1.revision_id);
    assert_eq!(evidence_counts(&admin).await, (3, 3, 3, 3, 0));

    let before_rejected_activation = evidence_counts(&admin).await;
    let rejected = executor
        .execute(
            &activate_definition,
            request(
                ACTIVATE_REVISION_CAPABILITY,
                ACTIVATE_REQUEST_SCHEMA,
                &wire::ActivateMetadataRevisionRequest {
                    revision_id: published_v2.revision_id.clone(),
                    expected_generation: 1,
                    confirm_breaking_changes: false,
                },
                "activate-v2-unconfirmed",
                4_000_000_000,
            ),
        )
        .await
        .expect_err("breaking activation without confirmation must fail");
    assert_eq!(
        rejected.code,
        "METADATA_BREAKING_CHANGE_CONFIRMATION_REQUIRED"
    );
    assert_eq!(
        evidence_counts(&admin).await,
        before_rejected_activation,
        "failed activation must roll back idempotency, audit, transaction and transition evidence"
    );

    let activate_v2 = executor
        .execute(
            &activate_definition,
            request(
                ACTIVATE_REVISION_CAPABILITY,
                ACTIVATE_REQUEST_SCHEMA,
                &wire::ActivateMetadataRevisionRequest {
                    revision_id: published_v2.revision_id.clone(),
                    expected_generation: 1,
                    confirm_breaking_changes: true,
                },
                "activate-v2-confirmed",
                5_000_000_000,
            ),
        )
        .await
        .expect("activate breaking metadata revision v2 with confirmation");
    let activated_v2 = wire::ActivateMetadataRevisionResponse::decode(
        activate_v2.output.as_ref().unwrap().bytes.as_slice(),
    )
    .unwrap();
    assert!(
        activated_v2
            .impact
            .as_ref()
            .expect("activation impact v2")
            .has_breaking_changes
    );
    let state_v2 = activated_v2.state.expect("activation state v2");
    assert_eq!(state_v2.generation, 2);
    assert_eq!(state_v2.rollback_depth, 1);
    assert_eq!(state_v2.active_revision_id, published_v2.revision_id);
    assert_eq!(evidence_counts(&admin).await, (4, 4, 4, 4, 0));

    let rollback_definition = metadata_capability_definition(ROLLBACK_REVISION_CAPABILITY).unwrap();
    let rollback = executor
        .execute(
            &rollback_definition,
            request(
                ROLLBACK_REVISION_CAPABILITY,
                ROLLBACK_REQUEST_SCHEMA,
                &wire::RollbackMetadataRevisionRequest {
                    expected_generation: 2,
                },
                "rollback-v2",
                6_000_000_000,
            ),
        )
        .await
        .expect("roll back metadata revision v2");
    let rolled_back = wire::RollbackMetadataRevisionResponse::decode(
        rollback.output.as_ref().unwrap().bytes.as_slice(),
    )
    .unwrap();
    let rollback_state = rolled_back.state.expect("rollback state");
    assert_eq!(rollback_state.generation, 3);
    assert_eq!(rollback_state.rollback_depth, 0);
    assert_eq!(rollback_state.active_revision_id, published_v1.revision_id);
    assert_eq!(evidence_counts(&admin).await, (5, 5, 5, 5, 0));

    let before_second_rollback = evidence_counts(&admin).await;
    let second_rollback = executor
        .execute(
            &rollback_definition,
            request(
                ROLLBACK_REVISION_CAPABILITY,
                ROLLBACK_REQUEST_SCHEMA,
                &wire::RollbackMetadataRevisionRequest {
                    expected_generation: 3,
                },
                "rollback-unavailable",
                7_000_000_000,
            ),
        )
        .await
        .expect_err("popped rollback history must not toggle the revision forward");
    assert_eq!(second_rollback.code, "METADATA_ROLLBACK_UNAVAILABLE");
    assert_eq!(evidence_counts(&admin).await, before_second_rollback);

    let actions = sqlx::query_scalar::<_, String>(
        r#"
        SELECT action
        FROM crm.metadata_transitions
        WHERE tenant_id = 'tenant-a'
        ORDER BY transition_sequence
        "#,
    )
    .fetch_all(&admin)
    .await
    .expect("load metadata transition sequence");
    assert_eq!(
        actions,
        ["publish", "activate", "publish", "activate", "rollback"]
    );
}

fn bundle_v1_definitions() -> Vec<wire::MetadataDefinitionInput> {
    vec![object_definition(), field_definition()]
}

fn bundle_v2_definitions() -> Vec<wire::MetadataDefinitionInput> {
    vec![object_definition()]
}

fn object_definition() -> wire::MetadataDefinitionInput {
    wire::MetadataDefinitionInput {
        schema_version: METADATA_DEFINITION_SCHEMA_VERSION.to_owned(),
        definition_json: br#"{"kind":"object","definition":{"id":"crm.sales.deal","owner_module_id":"crm.sales","label":"Deal","plural_label":"Deals","description":null,"tags":["sales"]}}"#.to_vec(),
    }
}

fn field_definition() -> wire::MetadataDefinitionInput {
    wire::MetadataDefinitionInput {
        schema_version: METADATA_DEFINITION_SCHEMA_VERSION.to_owned(),
        definition_json: br#"{"kind":"field","definition":{"id":"crm.sales.deal.name","object_id":"crm.sales.deal","label":"Name","data_class":"confidential","field_type":{"type":"text","config":{"max_length":200}},"required":true,"immutable":false}}"#.to_vec(),
    }
}

fn request<M>(
    capability_id: &str,
    schema_id: &str,
    message: &M,
    suffix: &str,
    request_started_at_unix_nanos: i64,
) -> CapabilityRequest
where
    M: Message,
{
    let input = protobuf_payload(
        METADATA_MODULE_ID,
        schema_id,
        DataClass::Confidential,
        message,
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
                capability_id: CapabilityId::try_new(capability_id).unwrap(),
                capability_version: CapabilityVersion::try_new("1.0.0").unwrap(),
                idempotency_key: IdempotencyKey::try_new(format!("idempotency-{suffix}")).unwrap(),
                business_transaction_id: BusinessTransactionId::try_new(format!("tx-{suffix}"))
                    .unwrap(),
                schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
                request_started_at_unix_nanos,
            },
        },
        input,
        input_hash,
        approval: None,
    }
}

async fn assert_transaction_marker(admin: &PgPool, transaction_id: &str) {
    let marker = sqlx::query(
        r#"
        SELECT expected_outbox_events, expected_audit_records, expected_idempotency_records
        FROM crm.business_transactions
        WHERE tenant_id = 'tenant-a' AND business_transaction_id = $1
        "#,
    )
    .bind(transaction_id)
    .fetch_one(admin)
    .await
    .expect("load metadata business transaction marker");
    assert_eq!(
        marker.try_get::<i32, _>("expected_outbox_events").unwrap(),
        0
    );
    assert_eq!(
        marker.try_get::<i32, _>("expected_audit_records").unwrap(),
        1
    );
    assert_eq!(
        marker
            .try_get::<i32, _>("expected_idempotency_records")
            .unwrap(),
        1
    );
}

async fn assert_latest_audit(
    admin: &PgPool,
    capability_id: &str,
    aggregate_type: &str,
    transaction_id: &str,
) {
    let audit = sqlx::query(
        r#"
        SELECT canonicalization_profile, canonical_envelope
        FROM crm.audit_records
        WHERE tenant_id = 'tenant-a'
        ORDER BY audit_sequence DESC
        LIMIT 1
        "#,
    )
    .fetch_one(admin)
    .await
    .expect("load canonical metadata audit evidence");
    assert_eq!(
        audit
            .try_get::<String, _>("canonicalization_profile")
            .unwrap(),
        "crm.cjson/v1"
    );
    let envelope: serde_json::Value =
        serde_json::from_slice(&audit.try_get::<Vec<u8>, _>("canonical_envelope").unwrap())
            .unwrap();
    assert_eq!(envelope["capability_id"], capability_id);
    assert_eq!(envelope["aggregate_type"], aggregate_type);
    assert_eq!(envelope["transaction_id"], transaction_id);
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
