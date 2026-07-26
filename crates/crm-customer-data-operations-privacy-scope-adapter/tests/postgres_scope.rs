#![allow(clippy::too_many_arguments)]

#[path = "postgres_scope/support.rs"]
mod support;

use support::*;

use crm_capability_runtime::TransactionalCapabilityExecutor;
use crm_core_data::{PostgresDataStore, PostgresTransactionalAggregateExecutor};
use crm_customer_data_operations_privacy_scope_adapter::{
    CustomerDataOperationsPrivacyScopeQueryAdapter, customer_data_privacy_scope_definition,
};
use crm_identity_resolution_capability_adapter::{
    IdentityResolutionCapabilityPlanner, MERGE_CAPABILITY,
    capability_definition as identity_definition,
};
use crm_parties_capability_adapter::{
    CREATE_CAPABILITY as CREATE_PARTY_CAPABILITY, PartyCapabilityPlanner,
    capability_definition as party_definition,
};
use crm_proto_contracts::crm::customer_privacy::v1 as privacy;
use crm_query_runtime::QueryExecutor;
use prost::Message;
use sqlx::PgPool;
use std::collections::BTreeMap;
use std::sync::Arc;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn customer_data_scope_is_alias_aware_complete_reference_only_and_side_effect_free() {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!("skipping Customer Data privacy scope proof without DATABASE_URL");
        return;
    };
    let admin_url = std::env::var("ADMIN_DATABASE_URL")
        .expect("ADMIN_DATABASE_URL must accompany DATABASE_URL");

    let store = PostgresDataStore::connect(&database_url, 8)
        .await
        .expect("connect Customer Data privacy scope runtime store");
    let admin = PgPool::connect(&admin_url)
        .await
        .expect("connect Customer Data privacy scope evidence reader");
    let party_executor: Arc<dyn TransactionalCapabilityExecutor> =
        Arc::new(PostgresTransactionalAggregateExecutor::new(
            store.clone(),
            Arc::new(PartyCapabilityPlanner),
        ));
    let identity_executor: Arc<dyn TransactionalCapabilityExecutor> =
        Arc::new(PostgresTransactionalAggregateExecutor::new(
            store.clone(),
            Arc::new(IdentityResolutionCapabilityPlanner),
        ));
    let party_create = party_definition(CREATE_PARTY_CAPABILITY).unwrap();
    let merge_execute = identity_definition(MERGE_CAPABILITY).unwrap();

    for (party_id, seed) in [
        ("party-canonical", 11),
        ("party-alias", 12),
        ("party-unrelated", 13),
    ] {
        create_party(&party_executor, &party_create, TENANT_A, party_id, seed).await;
    }
    create_party(
        &party_executor,
        &party_create,
        TENANT_B,
        "party-canonical",
        21,
    )
    .await;
    merge_party(
        &identity_executor,
        &merge_execute,
        TENANT_A,
        "merge-customer-data-alias",
        "party-alias",
        "party-canonical",
        31,
    )
    .await;

    let relevant_import_alias = insert_valid_import_row(
        &admin,
        TENANT_A,
        "import-job-relevant",
        1,
        "party-alias",
        "Private Alias Import Name",
        false,
    )
    .await;
    let relevant_import_canonical = insert_valid_import_row(
        &admin,
        TENANT_A,
        "import-job-relevant",
        2,
        "party-canonical",
        "Private Canonical Import Name",
        true,
    )
    .await;
    let unrelated_import =
        insert_pending_import_row(&admin, TENANT_A, "import-job-unrelated", 1).await;

    let relevant_export = insert_export_evidence(
        &admin,
        TENANT_A,
        "export-job-relevant",
        1,
        "party-alias",
        "party-alias,person,Private Export Name\n",
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    )
    .await;
    let unrelated_export = insert_export_evidence(
        &admin,
        TENANT_A,
        "export-job-relevant",
        2,
        "party-unrelated",
        "party-unrelated,person,Unrelated Export Name\n",
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
    )
    .await;
    let tenant_b_export = insert_export_evidence(
        &admin,
        TENANT_B,
        "export-job-tenant-b",
        1,
        "party-canonical",
        "party-canonical,person,Tenant B Export Name\n",
        "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
    )
    .await;

    let generation_a = current_generation(&admin, TENANT_A).await;
    let definition = customer_data_privacy_scope_definition().unwrap();
    let adapter = CustomerDataOperationsPrivacyScopeQueryAdapter::new(store);
    let before = write_surface_counts(&admin).await;

    let mut cursor = String::new();
    let mut page = 1_u32;
    let mut resources = Vec::new();
    let mut encoded_pages = Vec::new();
    loop {
        let result = adapter
            .execute(
                &definition,
                scope_request(
                    TENANT_A,
                    "party-canonical",
                    generation_a,
                    2,
                    &cursor,
                    "customer-data-pages",
                ),
            )
            .await
            .expect("enumerate authoritative Customer Data privacy scope");
        assert_eq!(write_surface_counts(&admin).await, before);
        assert_response_omits_private_customer_data(&result.output.bytes);
        encoded_pages.push(result.output.bytes.clone());
        let response = decode(&result.output.bytes);
        let contribution = response.contribution.unwrap();
        let evidence = contribution.page_evidence.unwrap();
        assert_eq!(evidence.page_number, page);
        assert_eq!(
            evidence.emitted_resource_count as usize,
            contribution.resources.len()
        );
        assert!(evidence.scanned_resource_count >= 9);
        resources.extend(contribution.resources);
        if evidence.terminal_complete {
            assert!(evidence.next_cursor.is_empty());
            break;
        }
        assert!(!evidence.next_cursor.is_empty());
        cursor = evidence.next_cursor;
        page += 1;
        assert!(page <= 4, "four-family pagination must terminate");
    }

    assert_eq!(page, 3);
    assert_eq!(resources.len(), 5);
    let by_type = resources.into_iter().fold(
        BTreeMap::<String, Vec<String>>::new(),
        |mut output, resource| {
            output
                .entry(resource.resource_type)
                .or_default()
                .push(resource.resource_id);
            output
        },
    );
    assert_eq!(
        by_type,
        BTreeMap::from([
            (
                "customer_data.export_execution_outcome".to_owned(),
                vec![relevant_export.outcome.clone()],
            ),
            (
                "customer_data.export_execution_stage".to_owned(),
                vec![relevant_export.stage.clone()],
            ),
            (
                "customer_data.export_selection_item".to_owned(),
                vec![relevant_export.selection.clone()],
            ),
            ("customer_data.import_row".to_owned(), {
                let mut ids = vec![relevant_import_alias.clone(), relevant_import_canonical];
                ids.sort();
                ids
            },),
        ])
    );
    assert!(!by_type.contains_key("customer_data.import_job"));
    assert!(!by_type.contains_key("customer_data.export_job"));
    assert!(!by_type.contains_key("customer_data.export_selection_boundary"));
    assert!(!by_type.contains_key("customer_data.export_selection_progress"));

    for forbidden_id in [
        unrelated_import,
        unrelated_export.selection,
        unrelated_export.stage,
        unrelated_export.outcome,
        tenant_b_export.selection,
        tenant_b_export.stage,
        tenant_b_export.outcome,
    ] {
        assert!(
            encoded_pages.iter().all(|bytes| !bytes
                .windows(forbidden_id.len())
                .any(|value| value == forbidden_id.as_bytes())),
            "unrelated or cross-tenant resource leaked: {forbidden_id}"
        );
    }

    let tenant_b_generation = current_generation(&admin, TENANT_B).await;
    let tenant_b = adapter
        .execute(
            &definition,
            scope_request(
                TENANT_B,
                "party-canonical",
                tenant_b_generation,
                10,
                "",
                "customer-data-tenant-b",
            ),
        )
        .await
        .expect("enumerate tenant B Customer Data scope");
    let tenant_b_resources = decode(&tenant_b.output.bytes)
        .contribution
        .unwrap()
        .resources;
    assert_eq!(tenant_b_resources.len(), 3);
    assert_eq!(write_surface_counts(&admin).await, before);

    let stale = adapter
        .execute(
            &definition,
            scope_request(
                TENANT_A,
                "party-canonical",
                generation_a - 1,
                2,
                "",
                "customer-data-stale",
            ),
        )
        .await
        .expect_err("stale topology generation must fail closed");
    assert_eq!(stale.code, "CUSTOMER_DATA_PRIVACY_SCOPE_LINEAGE_INVALID");
    assert_eq!(write_surface_counts(&admin).await, before);

    let first_page = decode(&encoded_pages[0])
        .contribution
        .unwrap()
        .page_evidence
        .unwrap();
    let rebound = adapter
        .execute(
            &definition,
            scope_request(
                TENANT_A,
                "party-canonical",
                generation_a,
                3,
                &first_page.next_cursor,
                "customer-data-pages",
            ),
        )
        .await
        .expect_err("cursor page-size rebinding must fail closed");
    assert_eq!(rebound.code, "CUSTOMER_DATA_PRIVACY_SCOPE_CURSOR_INVALID");
    assert_eq!(write_surface_counts(&admin).await, before);

    corrupt_selection_metadata(&admin, &relevant_export.selection).await;
    let corrupted_baseline = write_surface_counts(&admin).await;
    let corrupted = adapter
        .execute(
            &definition,
            scope_request(
                TENANT_A,
                "party-canonical",
                generation_a,
                10,
                "",
                "customer-data-corrupted",
            ),
        )
        .await
        .expect_err("malformed owner persistence must fail closed");
    assert_eq!(
        corrupted.code,
        "CUSTOMER_DATA_PRIVACY_SCOPE_EXPORT_SELECTION_STATE_INVALID"
    );
    assert_eq!(write_surface_counts(&admin).await, corrupted_baseline);
}

fn decode(bytes: &[u8]) -> privacy::CustomerDataPrivacyScopeContributionResponse {
    privacy::CustomerDataPrivacyScopeContributionResponse::decode(bytes)
        .expect("decode Customer Data privacy scope response")
}

fn assert_response_omits_private_customer_data(bytes: &[u8]) {
    for forbidden in [
        "party-canonical",
        "party-alias",
        "party-unrelated",
        "Private Alias Import Name",
        "Private Canonical Import Name",
        "private-source-import-job-relevant-1",
        "Private Export Name",
        "Unrelated Export Name",
        "Tenant B Export Name",
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
    ] {
        assert!(
            !bytes
                .windows(forbidden.len())
                .any(|candidate| candidate == forbidden.as_bytes()),
            "response leaked forbidden Customer Data value: {forbidden}"
        );
    }
}
