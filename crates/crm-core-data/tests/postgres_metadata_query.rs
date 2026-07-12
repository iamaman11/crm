#![cfg(feature = "postgres-integration")]

use crm_core_data::{PostgresDataStore, PostgresMetadataQueryStore, PostgresMetadataStore};
use crm_metadata_query_adapter::MetadataQueryStore;
use crm_metadata_runtime::{
    MetadataBundleDraft, MetadataDocument, MetadataId, MetadataKey, MetadataKind,
};
use crm_module_sdk::{
    ActorId, BusinessTransactionId, CapabilityId, CapabilityVersion, CausationId, CorrelationId,
    ExecutionContext, IdempotencyKey, ModuleExecutionContext, ModuleId, RequestId, SchemaVersion,
    TenantId, TraceId,
};
use sqlx::{PgPool, Row};

const TENANT_A: &str = "tenant-a";
const TENANT_B: &str = "tenant-b";

#[tokio::test(flavor = "current_thread")]
async fn metadata_query_store_is_tenant_only_non_mutating_and_runtime_impact_driven() {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!("skipping PostgreSQL metadata query acceptance because DATABASE_URL is absent");
        return;
    };
    let admin_database_url = std::env::var("ADMIN_DATABASE_URL")
        .expect("ADMIN_DATABASE_URL must accompany DATABASE_URL");
    let data_store = PostgresDataStore::connect(&database_url, 8)
        .await
        .expect("connect metadata query store");
    let mutation_store = PostgresMetadataStore::new(data_store.clone());
    let query_store = PostgresMetadataQueryStore::new(data_store);
    let admin = PgPool::connect(&admin_database_url)
        .await
        .expect("connect metadata query evidence reader");

    cleanup(&admin).await;

    let first = bundle("object-v1", Some("field-v1"));
    let first_revision = first.revision_id();
    mutation_store
        .publish(&context(TENANT_A, "publish-v1"), &first, 1_000_000_000)
        .await
        .expect("publish first metadata revision");

    let before_reads = evidence_counts(&admin).await;

    let tenant_a = TenantId::try_new(TENANT_A).unwrap();
    let tenant_b = TenantId::try_new(TENANT_B).unwrap();
    let round_trip = query_store
        .revision(&tenant_a, &first_revision)
        .await
        .expect("read tenant-a revision")
        .expect("tenant-a revision exists");
    assert_eq!(round_trip.revision_id(), first_revision);
    assert_eq!(round_trip.documents(), first.documents());

    assert!(
        query_store
            .revision(&tenant_b, &first_revision)
            .await
            .expect("cross-tenant revision read remains non-disclosing")
            .is_none()
    );
    assert_eq!(
        query_store
            .impact_for(&tenant_b, &first_revision)
            .await
            .unwrap_err()
            .code,
        "METADATA_REVISION_NOT_FOUND"
    );

    let initial_state = query_store
        .tenant_state(&tenant_a)
        .await
        .expect("read initial tenant state");
    assert_eq!(initial_state.generation, 0);
    assert_eq!(initial_state.active_revision, None);

    mutation_store
        .activate(
            &context(TENANT_A, "activate-v1"),
            &first_revision,
            0,
            false,
            2_000_000_000,
        )
        .await
        .expect("activate first metadata revision");

    let candidate = bundle("object-v2", None);
    let candidate_revision = candidate.revision_id();
    mutation_store
        .publish(&context(TENANT_A, "publish-v2"), &candidate, 3_000_000_000)
        .await
        .expect("publish breaking candidate");

    let before_governed_reads = evidence_counts(&admin).await;
    let impact = query_store
        .impact_for(&tenant_a, &candidate_revision)
        .await
        .expect("load runtime-derived impact");
    assert!(impact.has_breaking_changes());
    assert_eq!(impact.current_revision, Some(first_revision));
    assert_eq!(impact.candidate_revision, candidate_revision);

    let state = query_store
        .tenant_state(&tenant_a)
        .await
        .expect("read active tenant state");
    assert_eq!(state.generation, 1);
    assert_eq!(state.rollback_depth, 0);
    assert_eq!(state.active_revision, impact.current_revision);

    let after_governed_reads = evidence_counts(&admin).await;
    assert_eq!(
        after_governed_reads, before_governed_reads,
        "metadata query reads must not create transition or global audit evidence"
    );
    assert!(
        before_governed_reads.0 > before_reads.0,
        "intervening publish/activate mutations should increase transition evidence"
    );
}

fn bundle(object_content: &str, field_content: Option<&str>) -> MetadataBundleDraft {
    let object_key = key(MetadataKind::Object, "crm.sales.deal");
    let mut documents = vec![
        MetadataDocument::new(
            object_key.clone(),
            "crm.metadata.definition/v1",
            object_content.as_bytes().to_vec(),
            Vec::new(),
        )
        .unwrap(),
    ];
    if let Some(field_content) = field_content {
        documents.push(
            MetadataDocument::new(
                key(MetadataKind::Field, "crm.sales.deal.name"),
                "crm.metadata.definition/v1",
                field_content.as_bytes().to_vec(),
                vec![object_key],
            )
            .unwrap(),
        );
    }
    MetadataBundleDraft::new(documents).unwrap()
}

fn key(kind: MetadataKind, id: &str) -> MetadataKey {
    MetadataKey::new(kind, MetadataId::try_new(id).unwrap())
}

fn context(tenant_id: &str, operation: &str) -> ModuleExecutionContext {
    let suffix = format!("metadata-query-{operation}");
    ModuleExecutionContext {
        module_id: ModuleId::try_new("crm.metadata").unwrap(),
        execution: ExecutionContext {
            tenant_id: TenantId::try_new(tenant_id).unwrap(),
            actor_id: ActorId::try_new(if tenant_id == TENANT_A {
                "actor-a"
            } else {
                "actor-b"
            })
            .unwrap(),
            request_id: RequestId::try_new(format!("request-{suffix}")).unwrap(),
            correlation_id: CorrelationId::try_new(format!("correlation-{suffix}")).unwrap(),
            causation_id: CausationId::try_new(format!("causation-{suffix}")).unwrap(),
            trace_id: TraceId::try_new(format!("trace-{suffix}")).unwrap(),
            capability_id: CapabilityId::try_new("metadata.bundle.publish").unwrap(),
            capability_version: CapabilityVersion::try_new("1.0.0").unwrap(),
            idempotency_key: IdempotencyKey::try_new(format!("idempotency-{suffix}")).unwrap(),
            business_transaction_id: BusinessTransactionId::try_new(format!("tx-{suffix}"))
                .unwrap(),
            schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
            request_started_at_unix_nanos: 1,
        },
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
          crm.audit_records,
          crm.audit_heads
        "#,
    )
    .execute(admin)
    .await
    .expect("truncate metadata query acceptance fixtures");
}

async fn evidence_counts(admin: &PgPool) -> (i64, i64) {
    let row = sqlx::query(
        r#"
        SELECT
          (SELECT count(*) FROM crm.metadata_transitions) AS transition_count,
          (SELECT count(*) FROM crm.audit_records) AS audit_count
        "#,
    )
    .fetch_one(admin)
    .await
    .expect("read metadata query evidence counts");
    (
        row.try_get("transition_count").unwrap(),
        row.try_get("audit_count").unwrap(),
    )
}
