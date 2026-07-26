mod support;

use support::*;

use crm_capability_runtime::TransactionalCapabilityExecutor;
use crm_consents_capability_adapter::{
    CREATE_CAPABILITY as CREATE_CONSENT_CAPABILITY, ConsentCapabilityPlanner,
    capability_definition as consent_definition,
};
use crm_consents_privacy_scope_adapter::{
    CAPABILITY_ID, ConsentsPrivacyScopeQueryAdapter, consents_privacy_scope_definition,
};
use crm_core_data::{PostgresDataStore, PostgresTransactionalAggregateExecutor};
use crm_parties_capability_adapter::{
    CREATE_CAPABILITY as CREATE_PARTY_CAPABILITY, PartyCapabilityPlanner,
    capability_definition as party_definition,
};
use crm_proto_contracts::crm::customer_privacy::v1 as privacy;
use crm_query_runtime::QueryExecutor;
use prost::Message;
use sqlx::PgPool;
use std::sync::Arc;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn consents_scope_is_paginated_tenant_bound_strict_and_side_effect_free() {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!("skipping Consents privacy scope PostgreSQL proof without DATABASE_URL");
        return;
    };
    let admin_url = std::env::var("ADMIN_DATABASE_URL")
        .expect("ADMIN_DATABASE_URL must accompany DATABASE_URL");

    let store = PostgresDataStore::connect(&database_url, 8)
        .await
        .expect("connect Consents privacy scope runtime store");
    let admin = PgPool::connect(&admin_url)
        .await
        .expect("connect Consents privacy scope evidence reader");
    let party_executor: Arc<dyn TransactionalCapabilityExecutor> =
        Arc::new(PostgresTransactionalAggregateExecutor::new(
            store.clone(),
            Arc::new(PartyCapabilityPlanner),
        ));
    let consent_executor: Arc<dyn TransactionalCapabilityExecutor> =
        Arc::new(PostgresTransactionalAggregateExecutor::new(
            store.clone(),
            Arc::new(ConsentCapabilityPlanner),
        ));
    let party_definition = party_definition(CREATE_PARTY_CAPABILITY).unwrap();
    let consent_definition = consent_definition(CREATE_CONSENT_CAPABILITY).unwrap();

    for (party_id, seed) in [
        ("party-scope", 11),
        ("party-empty", 12),
        ("party-redirected", 13),
        ("party-survivor", 14),
    ] {
        create_party(&party_executor, &party_definition, TENANT_A, party_id, seed).await;
    }

    for (authorization_id, seed) in [
        ("consent-001", 21),
        ("consent-002", 22),
        ("consent-003", 23),
    ] {
        create_consent(
            &consent_executor,
            &consent_definition,
            TENANT_A,
            "party-scope",
            authorization_id,
            seed,
        )
        .await;
    }
    create_consent(
        &consent_executor,
        &consent_definition,
        TENANT_A,
        "party-redirected",
        "consent-redirected",
        24,
    )
    .await;
    insert_canonical_redirect(&admin, "party-redirected", "party-survivor").await;

    let adapter = ConsentsPrivacyScopeQueryAdapter::new(store);
    let definition = consents_privacy_scope_definition().unwrap();

    let before_first = write_surface_counts(&admin).await;
    let first = adapter
        .execute(
            &definition,
            scope_request(TENANT_A, "party-scope", 1, 2, "", "first-page"),
        )
        .await
        .expect("read first authoritative Consents scope page");
    assert_eq!(write_surface_counts(&admin).await, before_first);
    let first_wire =
        privacy::ConsentsPrivacyScopeContributionResponse::decode(first.output.bytes.as_slice())
            .expect("decode first Consents scope response");
    let first_contribution = first_wire
        .contribution
        .expect("first contribution envelope");
    assert_eq!(first_contribution.owner_module_id, crm_consents::MODULE_ID);
    assert_eq!(first_contribution.capability_id, CAPABILITY_ID);
    assert_eq!(
        first_contribution
            .resources
            .iter()
            .map(|resource| resource.resource_id.as_str())
            .collect::<Vec<_>>(),
        vec!["consent-001", "consent-002"]
    );
    assert!(first_contribution.resources.iter().all(|resource| {
        resource.evidence_class
            == privacy::PrivacyScopeEvidenceClass::ImmutableRequiredEvidence as i32
            && resource.data_class == privacy::CustomerDataClass::Personal as i32
    }));
    let first_evidence = first_contribution
        .page_evidence
        .expect("first page evidence");
    assert_eq!(first_evidence.page_number, 1);
    assert_eq!(first_evidence.scanned_resource_count, 3);
    assert_eq!(first_evidence.emitted_resource_count, 2);
    assert!(!first_evidence.terminal_complete);
    assert!(!first_evidence.next_cursor.is_empty());
    assert!(
        !first
            .output
            .bytes
            .windows(b"privacy.marketing".len())
            .any(|value| { value == b"privacy.marketing" })
    );
    assert!(
        !first
            .output
            .bytes
            .windows(b"evidence://consent".len())
            .any(|value| { value == b"evidence://consent" })
    );

    let before_second = write_surface_counts(&admin).await;
    let second = adapter
        .execute(
            &definition,
            scope_request(
                TENANT_A,
                "party-scope",
                1,
                2,
                &first_evidence.next_cursor,
                "second-page",
            ),
        )
        .await
        .expect("read terminal authoritative Consents scope page");
    assert_eq!(write_surface_counts(&admin).await, before_second);
    let second_wire =
        privacy::ConsentsPrivacyScopeContributionResponse::decode(second.output.bytes.as_slice())
            .expect("decode second Consents scope response");
    let second_contribution = second_wire
        .contribution
        .expect("second contribution envelope");
    assert_eq!(second_contribution.resources.len(), 1);
    assert_eq!(second_contribution.resources[0].resource_id, "consent-003");
    let second_evidence = second_contribution
        .page_evidence
        .expect("second page evidence");
    assert_eq!(second_evidence.page_number, 2);
    assert!(second_evidence.terminal_complete);
    assert!(second_evidence.next_cursor.is_empty());

    let empty_before = write_surface_counts(&admin).await;
    let empty = adapter
        .execute(
            &definition,
            scope_request(TENANT_A, "party-empty", 1, 0, "", "empty"),
        )
        .await
        .expect("read empty authoritative Consents scope");
    assert_eq!(write_surface_counts(&admin).await, empty_before);
    let empty_wire =
        privacy::ConsentsPrivacyScopeContributionResponse::decode(empty.output.bytes.as_slice())
            .expect("decode empty Consents scope response");
    let empty_contribution = empty_wire
        .contribution
        .expect("empty contribution envelope");
    assert!(empty_contribution.resources.is_empty());
    assert!(empty_contribution.page_evidence.unwrap().terminal_complete);

    let stale_before = write_surface_counts(&admin).await;
    let stale = adapter
        .execute(
            &definition,
            scope_request(TENANT_A, "party-scope", 2, 2, "", "stale-generation"),
        )
        .await
        .expect_err("stale topology generation must fail closed");
    assert_eq!(stale.code, "CONSENTS_PRIVACY_SCOPE_LINEAGE_INVALID");
    assert!(stale.retryable);
    assert_eq!(write_surface_counts(&admin).await, stale_before);

    let cross_tenant_before = write_surface_counts(&admin).await;
    let cross_tenant = adapter
        .execute(
            &definition,
            scope_request(TENANT_B, "party-scope", 1, 2, "", "cross-tenant"),
        )
        .await
        .expect_err("cross-tenant Consents scope must be concealed");
    assert_eq!(cross_tenant.code, "CONSENTS_PRIVACY_SCOPE_LINEAGE_INVALID");
    assert_eq!(write_surface_counts(&admin).await, cross_tenant_before);

    let redirected_before = write_surface_counts(&admin).await;
    let redirected = adapter
        .execute(
            &definition,
            scope_request(TENANT_A, "party-redirected", 1, 2, "", "redirected-party"),
        )
        .await
        .expect_err("noncanonical Party scope must fail closed");
    assert_eq!(redirected.code, "CONSENTS_PRIVACY_SCOPE_LINEAGE_INVALID");
    assert_eq!(write_surface_counts(&admin).await, redirected_before);

    let rebound_before = write_surface_counts(&admin).await;
    let rebound = adapter
        .execute(
            &definition,
            scope_request(
                TENANT_A,
                "party-scope",
                1,
                1,
                &first_evidence.next_cursor,
                "cursor-rebound",
            ),
        )
        .await
        .expect_err("cursor rebound to another page size must fail");
    assert_eq!(rebound.code, "CONSENTS_PRIVACY_SCOPE_CURSOR_INVALID");
    assert_eq!(write_surface_counts(&admin).await, rebound_before);

    corrupt_consent_metadata(&admin, "consent-003").await;
    let malformed_before = write_surface_counts(&admin).await;
    let malformed = adapter
        .execute(
            &definition,
            scope_request(TENANT_A, "party-scope", 1, 128, "", "malformed"),
        )
        .await
        .expect_err("malformed Consent state must fail closed");
    assert_eq!(
        malformed.code,
        "CONSENTS_PRIVACY_SCOPE_STORED_STATE_INVALID"
    );
    assert_eq!(write_surface_counts(&admin).await, malformed_before);
}
