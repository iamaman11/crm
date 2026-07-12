#![cfg(feature = "postgres-integration")]

use crm_core_data::PostgresDataStore;
use crm_module_sdk::{RecordType, TenantId};
use crm_search_runtime::{
    SearchCandidateCursor, SearchCandidateRequest, SearchCandidateStore, SearchIndexId,
};
use sqlx::PgPool;
use std::collections::BTreeSet;

const TENANT_A: &str = "tenant-a";
const TENANT_B: &str = "tenant-b";
const INDEX_ID: &str = "crm.global-search";
const GENERATION_ONE: &str = "generation-1";
const GENERATION_TWO: &str = "generation-2";
const PROJECTION_ONE: &str = "search.global.g1";
const PROJECTION_TWO: &str = "search.global.g2";
const PROJECTION_B: &str = "search.global.b1";

#[tokio::test(flavor = "current_thread")]
async fn active_generation_search_is_deterministic_switchable_and_tenant_isolated() {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!("skipping PostgreSQL search acceptance because DATABASE_URL is absent");
        return;
    };
    let admin_database_url = std::env::var("ADMIN_DATABASE_URL")
        .expect("ADMIN_DATABASE_URL must accompany DATABASE_URL");
    let store = PostgresDataStore::connect(&database_url, 4)
        .await
        .expect("connect search store");
    let admin = PgPool::connect(&admin_database_url)
        .await
        .expect("connect search evidence reader");

    cleanup(&admin).await;
    seed_projection_generation(
        &admin,
        PROJECTION_ONE,
        &[
            ("sales.deal", "deal-acme-renewal", 1, "Acme Renewal"),
            ("sales.deal", "deal-acme-expansion", 1, "Acme Expansion"),
            ("sales.deal", "deal-beta", 1, "Beta Renewal"),
        ],
    )
    .await;
    seed_projection_generation(
        &admin,
        PROJECTION_TWO,
        &[("sales.deal", "deal-acme-next", 2, "Acme Next Generation")],
    )
    .await;

    let tenant_a = TenantId::try_new(TENANT_A).unwrap();
    store
        .register_search_generation(&tenant_a, INDEX_ID, GENERATION_ONE, PROJECTION_ONE, "1")
        .await
        .expect("register generation one");
    store
        .activate_search_generation(&tenant_a, INDEX_ID, GENERATION_ONE)
        .await
        .expect("activate generation one");

    let first = SearchCandidateStore::search_candidates(&store, request(TENANT_A, None, 1))
        .await
        .expect("search first candidate page");
    assert_eq!(first.candidates.len(), 1);
    assert_eq!(
        first.candidates[0].matched_fields,
        BTreeSet::from(["name".to_owned()])
    );
    let first_id = first.candidates[0].resource.record_id.as_str().to_owned();
    let continuation = first
        .next_after
        .clone()
        .expect("first page must expose a continuation");

    let second =
        SearchCandidateStore::search_candidates(&store, request(TENANT_A, Some(continuation), 1))
            .await
            .expect("search second candidate page");
    assert_eq!(second.candidates.len(), 1);
    assert_ne!(second.candidates[0].resource.record_id.as_str(), first_id);
    assert!(
        second.candidates[0].rank_micros <= first.candidates[0].rank_micros,
        "candidate rank must be monotonically non-increasing across cursor pages"
    );

    store
        .register_search_generation(&tenant_a, INDEX_ID, GENERATION_TWO, PROJECTION_TWO, "1")
        .await
        .expect("register generation two");
    store
        .activate_search_generation(&tenant_a, INDEX_ID, GENERATION_TWO)
        .await
        .expect("atomically activate generation two");

    let switched = SearchCandidateStore::search_candidates(&store, request(TENANT_A, None, 10))
        .await
        .expect("search active generation two");
    assert_eq!(switched.candidates.len(), 1);
    assert_eq!(
        switched.candidates[0].resource.record_id.as_str(),
        "deal-acme-next"
    );

    let tenant_b = TenantId::try_new(TENANT_B).unwrap();
    store
        .register_search_generation(&tenant_b, INDEX_ID, "generation-b", PROJECTION_B, "1")
        .await
        .expect("register isolated tenant-b generation");
    store
        .activate_search_generation(&tenant_b, INDEX_ID, "generation-b")
        .await
        .expect("activate isolated tenant-b generation");
    let tenant_b_page =
        SearchCandidateStore::search_candidates(&store, request(TENANT_B, None, 10))
            .await
            .expect("tenant-b search remains non-disclosing");
    assert!(tenant_b_page.candidates.is_empty());
}

fn request(
    tenant_id: &str,
    after: Option<SearchCandidateCursor>,
    page_size: u32,
) -> SearchCandidateRequest {
    SearchCandidateRequest {
        tenant_id: TenantId::try_new(tenant_id).unwrap(),
        index_id: SearchIndexId::try_new(INDEX_ID).unwrap(),
        normalized_text: "acme".to_owned(),
        resource_types: BTreeSet::from([RecordType::try_new("sales.deal").unwrap()]),
        after,
        page_size,
    }
}

async fn seed_projection_generation(
    admin: &PgPool,
    projection_id: &str,
    documents: &[(&str, &str, i64, &str)],
) {
    sqlx::query(
        r#"
        INSERT INTO crm.projection_checkpoints (
          tenant_id, projection_id, last_occurred_at, last_event_id,
          applied_event_count, status
        )
        VALUES ($1, $2, clock_timestamp(), 'event-bootstrap-a', $3, 'active')
        "#,
    )
    .bind(TENANT_A)
    .bind(projection_id)
    .bind(i64::try_from(documents.len()).unwrap())
    .execute(admin)
    .await
    .expect("seed search projection checkpoint");

    for (resource_type, resource_id, source_version, name) in documents {
        sqlx::query(
            r#"
            INSERT INTO crm.projection_documents (
              tenant_id, projection_id, resource_type, resource_id,
              source_version, source_event_id, document
            )
            VALUES (
              $1, $2, $3, $4, $5, 'event-bootstrap-a',
              jsonb_build_object(
                'index_id', $6::text,
                'generation_id', $7::text,
                'schema_version', '1',
                'owner_module_id', 'crm.sales',
                'search_text', $8::text,
                'searchable_fields', jsonb_build_object('name', $8::text),
                'display_fields', jsonb_build_object('name', $8::text)
              )
            )
            "#,
        )
        .bind(TENANT_A)
        .bind(projection_id)
        .bind(resource_type)
        .bind(resource_id)
        .bind(source_version)
        .bind(INDEX_ID)
        .bind(projection_id)
        .bind(name)
        .execute(admin)
        .await
        .expect("seed search projection document");
    }
}

async fn cleanup(admin: &PgPool) {
    sqlx::query(
        "DELETE FROM crm.search_index_generations WHERE tenant_id IN ($1, $2) AND index_id = $3",
    )
    .bind(TENANT_A)
    .bind(TENANT_B)
    .bind(INDEX_ID)
    .execute(admin)
    .await
    .expect("clean search generation registry");
    sqlx::query(
        "DELETE FROM crm.projection_documents WHERE tenant_id = $1 AND projection_id = ANY($2::text[])",
    )
    .bind(TENANT_A)
    .bind(vec![PROJECTION_ONE, PROJECTION_TWO])
    .execute(admin)
    .await
    .expect("clean search projection documents");
    sqlx::query(
        "DELETE FROM crm.projection_checkpoints WHERE tenant_id = $1 AND projection_id = ANY($2::text[])",
    )
    .bind(TENANT_A)
    .bind(vec![PROJECTION_ONE, PROJECTION_TWO])
    .execute(admin)
    .await
    .expect("clean search projection checkpoints");
}
