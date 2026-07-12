#![cfg(feature = "postgres-integration")]

use crm_core_data::{
    MetadataPersistenceError, MetadataTransitionAction, PostgresDataStore, PostgresMetadataStore,
};
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
async fn durable_metadata_publication_is_tenant_isolated_optimistic_and_rollback_safe() {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!("skipping PostgreSQL metadata acceptance because DATABASE_URL is absent");
        return;
    };
    let admin_database_url = std::env::var("ADMIN_DATABASE_URL")
        .expect("ADMIN_DATABASE_URL must accompany DATABASE_URL");
    let data_store = PostgresDataStore::connect(&database_url, 8)
        .await
        .expect("connect metadata store");
    let store = PostgresMetadataStore::new(data_store);
    let admin = PgPool::connect(&admin_database_url)
        .await
        .expect("connect metadata evidence reader");

    cleanup(&admin).await;

    let first = bundle("object-v1", Some("field-v1"));
    let first_revision = first.revision_id();
    let published = store
        .publish(&context(TENANT_A, "publish-v1"), &first, 1_000_000_000)
        .await
        .expect("publish first metadata revision");
    assert_eq!(published.revision_id, first_revision);
    assert!(!published.already_published);

    let round_trip = store
        .revision(&context(TENANT_A, "read-v1"), &first_revision)
        .await
        .expect("read first revision")
        .expect("first revision exists");
    assert_eq!(round_trip.revision_id(), first_revision);
    assert_eq!(round_trip.documents(), first.documents());

    let duplicate = store
        .publish(
            &context(TENANT_A, "publish-v1-repeat"),
            &first,
            2_000_000_000,
        )
        .await
        .expect("republish identical revision");
    assert!(duplicate.already_published);
    assert_eq!(duplicate.revision_id, first_revision);

    assert!(
        store
            .revision(&context(TENANT_B, "read-a-revision"), &first_revision)
            .await
            .expect("cross-tenant revision read remains non-disclosing")
            .is_none()
    );
    assert!(matches!(
        store
            .impact_for(&context(TENANT_B, "impact-a-revision"), &first_revision)
            .await,
        Err(MetadataPersistenceError::RevisionNotFound(_))
    ));
    assert!(matches!(
        store
            .activate(
                &context(TENANT_B, "activate-a-revision"),
                &first_revision,
                0,
                false,
                3_000_000_000,
            )
            .await,
        Err(MetadataPersistenceError::RevisionNotFound(_))
    ));

    let breaking = bundle("object-v2", None);
    let breaking_revision = breaking.revision_id();
    store
        .publish(
            &context(TENANT_A, "publish-breaking"),
            &breaking,
            4_000_000_000,
        )
        .await
        .expect("publish breaking candidate");

    let activated_first = store
        .activate(
            &context(TENANT_A, "activate-v1"),
            &first_revision,
            0,
            false,
            5_000_000_000,
        )
        .await
        .expect("activate first revision");
    assert_eq!(activated_first.generation, 1);
    assert_eq!(activated_first.previous_revision, None);

    assert!(matches!(
        store
            .activate(
                &context(TENANT_A, "activate-breaking-stale"),
                &breaking_revision,
                0,
                true,
                6_000_000_000,
            )
            .await,
        Err(MetadataPersistenceError::GenerationConflict {
            expected: 0,
            actual: 1
        })
    ));
    assert!(matches!(
        store
            .activate(
                &context(TENANT_A, "activate-breaking-unconfirmed"),
                &breaking_revision,
                1,
                false,
                7_000_000_000,
            )
            .await,
        Err(MetadataPersistenceError::BreakingChangeConfirmationRequired(_))
    ));

    let activated_breaking = store
        .activate(
            &context(TENANT_A, "activate-breaking"),
            &breaking_revision,
            1,
            true,
            8_000_000_000,
        )
        .await
        .expect("activate confirmed breaking revision");
    assert_eq!(activated_breaking.generation, 2);
    assert_eq!(activated_breaking.previous_revision, Some(first_revision.clone()));

    let rollback = store
        .rollback(
            &context(TENANT_A, "rollback-breaking"),
            2,
            9_000_000_000,
        )
        .await
        .expect("rollback to first revision");
    assert_eq!(rollback.generation, 3);
    assert_eq!(rollback.active_revision, first_revision);
    assert_eq!(rollback.replaced_revision, breaking_revision);
    let state_after_rollback = store
        .tenant_state(&context(TENANT_A, "state-after-rollback"))
        .await
        .expect("load state after rollback");
    assert_eq!(state_after_rollback.generation, 3);
    assert_eq!(state_after_rollback.rollback_depth, 0);

    assert!(matches!(
        store
            .rollback(
                &context(TENANT_A, "rollback-cannot-toggle"),
                3,
                10_000_000_000,
            )
            .await,
        Err(MetadataPersistenceError::RollbackUnavailable)
    ));

    let candidate_three = bundle("object-v3", Some("field-v3"));
    let candidate_four = bundle("object-v4", Some("field-v4"));
    let revision_three = candidate_three.revision_id();
    let revision_four = candidate_four.revision_id();
    store
        .publish(
            &context(TENANT_A, "publish-v3"),
            &candidate_three,
            11_000_000_000,
        )
        .await
        .expect("publish concurrent candidate three");
    store
        .publish(
            &context(TENANT_A, "publish-v4"),
            &candidate_four,
            12_000_000_000,
        )
        .await
        .expect("publish concurrent candidate four");

    let first_concurrent = store.clone();
    let second_concurrent = store.clone();
    let first_concurrent_context = context(TENANT_A, "activate-v3-concurrent");
    let second_concurrent_context = context(TENANT_A, "activate-v4-concurrent");
    let (left, right) = tokio::join!(
        first_concurrent.activate(
            &first_concurrent_context,
            &revision_three,
            3,
            false,
            13_000_000_000,
        ),
        second_concurrent.activate(
            &second_concurrent_context,
            &revision_four,
            3,
            false,
            14_000_000_000,
        )
    );
    let successes = usize::from(left.is_ok()) + usize::from(right.is_ok());
    let conflicts = usize::from(matches!(
        left,
        Err(MetadataPersistenceError::GenerationConflict {
            expected: 3,
            actual: 4
        })
    )) + usize::from(matches!(
        right,
        Err(MetadataPersistenceError::GenerationConflict {
            expected: 3,
            actual: 4
        })
    ));
    assert_eq!(successes, 1, "exactly one concurrent activation must win");
    assert_eq!(conflicts, 1, "the stale concurrent writer must conflict");

    let concurrent_state = store
        .tenant_state(&context(TENANT_A, "state-after-concurrency"))
        .await
        .expect("load state after concurrent activation");
    assert_eq!(concurrent_state.generation, 4);
    assert_eq!(concurrent_state.rollback_depth, 1);

    let final_rollback = store
        .rollback(
            &context(TENANT_A, "rollback-concurrent-winner"),
            4,
            15_000_000_000,
        )
        .await
        .expect("rollback concurrent winner");
    assert_eq!(final_rollback.generation, 5);
    assert_eq!(final_rollback.active_revision, first_revision);
    assert_eq!(
        store
            .tenant_state(&context(TENANT_A, "final-state"))
            .await
            .expect("load final state")
            .rollback_depth,
        0
    );

    let transitions = store
        .transitions(&context(TENANT_A, "read-transitions"))
        .await
        .expect("read transition evidence");
    assert_eq!(
        transitions
            .iter()
            .filter(|transition| transition.action == MetadataTransitionAction::Publish)
            .count(),
        4,
        "idempotent re-publication must not duplicate publish evidence"
    );
    assert_eq!(
        transitions
            .iter()
            .filter(|transition| transition.action == MetadataTransitionAction::Activate)
            .count(),
        3
    );
    assert_eq!(
        transitions
            .iter()
            .filter(|transition| transition.action == MetadataTransitionAction::Rollback)
            .count(),
        2
    );
    assert!(
        store
            .transitions(&context(TENANT_B, "read-a-transitions"))
            .await
            .expect("tenant-b transition read remains isolated")
            .is_empty()
    );

    assert_force_rls(&admin).await;
    assert_immutable_revision(&database_url, &first_revision).await;
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
    let suffix = format!("metadata-{operation}");
    ModuleExecutionContext {
        module_id: ModuleId::try_new("crm.platform.metadata").unwrap(),
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
            capability_id: CapabilityId::try_new("metadata.definition.manage").unwrap(),
            capability_version: CapabilityVersion::try_new("1.0.0").unwrap(),
            idempotency_key: IdempotencyKey::try_new(format!("idempotency-{suffix}")).unwrap(),
            business_transaction_id: BusinessTransactionId::try_new(format!("tx-{suffix}")).unwrap(),
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
          crm.metadata_revisions_v2
        "#,
    )
    .execute(admin)
    .await
    .expect("truncate metadata acceptance fixtures");
}

async fn assert_force_rls(admin: &PgPool) {
    let row = sqlx::query(
        r#"
        SELECT count(*) AS protected_count
        FROM pg_class AS c
        JOIN pg_namespace AS n ON n.oid = c.relnamespace
        WHERE n.nspname = 'crm'
          AND c.relname = ANY($1::text[])
          AND c.relrowsecurity
          AND c.relforcerowsecurity
        "#,
    )
    .bind(vec![
        "metadata_revisions_v2",
        "metadata_revision_documents",
        "metadata_revision_dependencies",
        "metadata_activation_heads",
        "metadata_rollback_stack",
        "metadata_transitions",
    ])
    .fetch_one(admin)
    .await
    .expect("inspect metadata RLS flags");
    assert_eq!(row.try_get::<i64, _>("protected_count").unwrap(), 6);
}

async fn assert_immutable_revision(database_url: &str, revision_id: &crm_metadata_runtime::MetadataRevisionId) {
    let pool = PgPool::connect(database_url)
        .await
        .expect("connect immutable revision verifier");
    let mut transaction = pool.begin().await.expect("begin immutability verifier");
    let context = context(TENANT_A, "immutability-check");
    sqlx::query(
        r#"
        SELECT
          set_config('app.tenant_id', $1, true),
          set_config('app.actor_id', $2, true),
          set_config('app.request_id', $3, true),
          set_config('app.capability_id', $4, true),
          set_config('app.capability_version', $5, true),
          set_config('app.business_transaction_id', $6, true)
        "#,
    )
    .bind(context.execution.tenant_id.as_str())
    .bind(context.execution.actor_id.as_str())
    .bind(context.execution.request_id.as_str())
    .bind(context.execution.capability_id.as_str())
    .bind(context.execution.capability_version.as_str())
    .bind(context.execution.business_transaction_id.as_str())
    .execute(&mut *transaction)
    .await
    .expect("bind immutability verification context");
    let result = sqlx::query(
        r#"
        UPDATE crm.metadata_revisions_v2
        SET published_at = published_at
        WHERE tenant_id = $1 AND revision_id = $2
        "#,
    )
    .bind(TENANT_A)
    .bind(revision_id.as_bytes().as_slice())
    .execute(&mut *transaction)
    .await;
    assert!(result.is_err(), "immutable metadata revision accepted UPDATE");
    transaction.rollback().await.expect("rollback immutability check");
}
