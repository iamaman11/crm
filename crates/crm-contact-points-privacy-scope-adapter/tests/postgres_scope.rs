#[path = "postgres_scope/support.rs"]
mod support;

use support::*;

use crm_capability_runtime::TransactionalCapabilityExecutor;
use crm_contact_points_capability_adapter::{
    CREATE_CAPABILITY as CREATE_CONTACT_POINT_CAPABILITY, ContactPointCapabilityPlanner,
    UPDATE_CAPABILITY as UPDATE_CONTACT_POINT_CAPABILITY,
    VERIFY_CAPABILITY as VERIFY_CONTACT_POINT_CAPABILITY,
    capability_definition as contact_point_definition,
};
use crm_contact_points_privacy_scope_adapter::{
    CAPABILITY_ID, ContactPointsPrivacyScopeQueryAdapter, contact_points_privacy_scope_definition,
};
use crm_core_data::{PostgresDataStore, PostgresTransactionalAggregateExecutor};
use crm_parties_capability_adapter::{
    CREATE_CAPABILITY as CREATE_PARTY_CAPABILITY, PartyCapabilityPlanner,
    capability_definition as party_definition,
};
use crm_proto_contracts::crm::{
    contact_points::v1 as contact_points, customer_privacy::v1 as privacy,
};
use crm_query_runtime::{QueryExecutor, QueryRequest};
use prost::Message;
use sqlx::PgPool;
use std::sync::Arc;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn contact_points_scope_is_bounded_strict_tenant_bound_and_side_effect_free() {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!("skipping Contact Points privacy scope proof without DATABASE_URL");
        return;
    };
    let admin_url = std::env::var("ADMIN_DATABASE_URL")
        .expect("ADMIN_DATABASE_URL must accompany DATABASE_URL");

    let store = PostgresDataStore::connect(&database_url, 8)
        .await
        .expect("connect Contact Points privacy scope runtime store");
    let admin = PgPool::connect(&admin_url)
        .await
        .expect("connect Contact Points privacy scope evidence reader");
    let party_executor: Arc<dyn TransactionalCapabilityExecutor> =
        Arc::new(PostgresTransactionalAggregateExecutor::new(
            store.clone(),
            Arc::new(PartyCapabilityPlanner),
        ));
    let contact_point_executor: Arc<dyn TransactionalCapabilityExecutor> =
        Arc::new(PostgresTransactionalAggregateExecutor::new(
            store.clone(),
            Arc::new(ContactPointCapabilityPlanner),
        ));
    let party_definition = party_definition(CREATE_PARTY_CAPABILITY).unwrap();
    let create_contact_point_definition =
        contact_point_definition(CREATE_CONTACT_POINT_CAPABILITY).unwrap();
    let update_contact_point_definition =
        contact_point_definition(UPDATE_CONTACT_POINT_CAPABILITY).unwrap();
    let verify_contact_point_definition =
        contact_point_definition(VERIFY_CONTACT_POINT_CAPABILITY).unwrap();

    for (party_id, seed) in [
        ("party-scope", 11),
        ("party-other", 12),
        ("party-empty", 13),
        ("party-redirected", 14),
        ("party-survivor", 15),
    ] {
        create_party(&party_executor, &party_definition, TENANT_A, party_id, seed).await;
    }

    create_contact_point(
        &contact_point_executor,
        &create_contact_point_definition,
        TENANT_A,
        "contact-point-001",
        "party-scope",
        contact_points::ContactPointKind::Email,
        "Scope.Primary@EXAMPLE.COM",
        true,
        21,
    )
    .await;
    create_contact_point(
        &contact_point_executor,
        &create_contact_point_definition,
        TENANT_A,
        "contact-point-002",
        "party-other",
        contact_points::ContactPointKind::Phone,
        "+12025550102",
        false,
        22,
    )
    .await;
    create_contact_point(
        &contact_point_executor,
        &create_contact_point_definition,
        TENANT_A,
        "contact-point-003",
        "party-scope",
        contact_points::ContactPointKind::Messaging,
        "chat:scope-member@example.net",
        false,
        23,
    )
    .await;
    verify_contact_point(
        &contact_point_executor,
        &verify_contact_point_definition,
        TENANT_A,
        "contact-point-003",
        1,
        "private-verification-evidence-003",
        24,
    )
    .await;
    update_contact_point_status(
        &contact_point_executor,
        &update_contact_point_definition,
        TENANT_A,
        "contact-point-003",
        2,
        "chat:scope-member@example.net",
        contact_points::ContactPointStatus::Inactive,
        false,
        25,
    )
    .await;
    create_contact_point(
        &contact_point_executor,
        &create_contact_point_definition,
        TENANT_A,
        "contact-point-malformed",
        "party-scope",
        contact_points::ContactPointKind::Web,
        "https://private.example.test/profile",
        false,
        26,
    )
    .await;
    create_contact_point(
        &contact_point_executor,
        &create_contact_point_definition,
        TENANT_A,
        "contact-point-redirected",
        "party-redirected",
        contact_points::ContactPointKind::Postal,
        "Private Postal Address",
        false,
        27,
    )
    .await;

    let adapter = ContactPointsPrivacyScopeQueryAdapter::new(store);
    let definition = contact_points_privacy_scope_definition().unwrap();

    let before_first = write_surface_counts(&admin).await;
    let first = adapter
        .execute(
            &definition,
            scope_request(TENANT_A, "party-scope", 1, 1, "", "paged-scope"),
        )
        .await
        .expect("read first authoritative Contact Point scope page");
    assert_eq!(write_surface_counts(&admin).await, before_first);
    assert_response_omits_contact_point_state(
        &first.output.bytes,
        &[
            "Scope.Primary@EXAMPLE.COM",
            "\"status\"",
            "\"party_associations\"",
        ],
    );
    let first_wire = privacy::ContactPointsPrivacyScopeContributionResponse::decode(
        first.output.bytes.as_slice(),
    )
    .expect("decode first Contact Points scope response");
    let first_contribution = first_wire
        .contribution
        .expect("first contribution envelope");
    assert_eq!(
        first_contribution.owner_module_id,
        crm_contact_points::MODULE_ID
    );
    assert_eq!(first_contribution.capability_id, CAPABILITY_ID);
    assert_eq!(first_contribution.resources.len(), 1);
    assert_eq!(
        first_contribution.resources[0].resource_id,
        "contact-point-001"
    );
    assert_eq!(
        first_contribution.resources[0].evidence_class,
        privacy::PrivacyScopeEvidenceClass::RetainMinimizedEvidence as i32
    );
    let first_evidence = first_contribution
        .page_evidence
        .expect("first page evidence");
    assert_eq!(first_evidence.page_number, 1);
    assert_eq!(first_evidence.scanned_resource_count, 1);
    assert_eq!(first_evidence.emitted_resource_count, 1);
    assert!(!first_evidence.terminal_complete);
    assert!(!first_evidence.next_cursor.is_empty());

    let before_second = write_surface_counts(&admin).await;
    let second = adapter
        .execute(
            &definition,
            scope_request(
                TENANT_A,
                "party-scope",
                1,
                1,
                &first_evidence.next_cursor,
                "paged-scope",
            ),
        )
        .await
        .expect("read sparse second Contact Point scope page");
    assert_eq!(write_surface_counts(&admin).await, before_second);
    assert_response_omits_contact_point_state(
        &second.output.bytes,
        &[
            "chat:scope-member@example.net",
            "party-other",
            "\"status\"",
            "\"party_associations\"",
        ],
    );
    let second_wire = privacy::ContactPointsPrivacyScopeContributionResponse::decode(
        second.output.bytes.as_slice(),
    )
    .expect("decode second Contact Points scope response");
    let second_contribution = second_wire
        .contribution
        .expect("second contribution envelope");
    assert_eq!(second_contribution.resources.len(), 1);
    assert_eq!(
        second_contribution.resources[0].resource_id,
        "contact-point-003"
    );
    let second_evidence = second_contribution
        .page_evidence
        .expect("second page evidence");
    assert_eq!(second_evidence.page_number, 2);
    assert_eq!(second_evidence.scanned_resource_count, 2);
    assert_eq!(second_evidence.emitted_resource_count, 1);
    assert!(!second_evidence.terminal_complete);

    let terminal_before = write_surface_counts(&admin).await;
    let terminal = adapter
        .execute(
            &definition,
            scope_request(
                TENANT_A,
                "party-redirected",
                1,
                1,
                "",
                "terminal-primary-page",
            ),
        )
        .await
        .expect("read terminal non-empty Contact Point scope page");
    assert_eq!(write_surface_counts(&admin).await, terminal_before);
    assert_response_omits_contact_point_state(
        &terminal.output.bytes,
        &[
            "Private Postal Address",
            "\"status\"",
            "\"party_associations\"",
        ],
    );
    let terminal_wire = privacy::ContactPointsPrivacyScopeContributionResponse::decode(
        terminal.output.bytes.as_slice(),
    )
    .expect("decode terminal Contact Points scope response");
    let terminal_contribution = terminal_wire
        .contribution
        .expect("terminal contribution envelope");
    assert_eq!(terminal_contribution.resources.len(), 1);
    assert_eq!(
        terminal_contribution.resources[0].resource_id,
        "contact-point-redirected"
    );
    let terminal_evidence = terminal_contribution
        .page_evidence
        .expect("terminal page evidence");
    assert_eq!(terminal_evidence.page_number, 1);
    assert_eq!(terminal_evidence.emitted_resource_count, 1);
    assert!(terminal_evidence.terminal_complete);
    assert!(terminal_evidence.next_cursor.is_empty());

    let empty_before = write_surface_counts(&admin).await;
    let empty = adapter
        .execute(
            &definition,
            scope_request(TENANT_A, "party-empty", 1, 0, "", "empty"),
        )
        .await
        .expect("read empty authoritative Contact Point scope");
    assert_eq!(write_surface_counts(&admin).await, empty_before);
    let empty_wire = privacy::ContactPointsPrivacyScopeContributionResponse::decode(
        empty.output.bytes.as_slice(),
    )
    .expect("decode empty Contact Points scope response");
    let empty_contribution = empty_wire
        .contribution
        .expect("empty contribution envelope");
    assert!(empty_contribution.resources.is_empty());
    assert!(empty_contribution.page_evidence.unwrap().terminal_complete);

    let rebound_before = write_surface_counts(&admin).await;
    let rebound = adapter
        .execute(
            &definition,
            scope_request(
                TENANT_A,
                "party-scope",
                1,
                2,
                &first_evidence.next_cursor,
                "paged-scope",
            ),
        )
        .await
        .expect_err("cursor rebound to another page size must fail");
    assert_eq!(rebound.code, "CONTACT_POINTS_PRIVACY_SCOPE_CURSOR_INVALID");
    assert_eq!(write_surface_counts(&admin).await, rebound_before);

    let mut corrupted_cursor = first_evidence.next_cursor.clone().into_bytes();
    corrupted_cursor[0] = if corrupted_cursor[0] == b'A' {
        b'B'
    } else {
        b'A'
    };
    let corrupted_cursor = String::from_utf8(corrupted_cursor).expect("cursor remains UTF-8");
    let corrupt_cursor_before = write_surface_counts(&admin).await;
    let corrupt_cursor = adapter
        .execute(
            &definition,
            scope_request(
                TENANT_A,
                "party-scope",
                1,
                1,
                &corrupted_cursor,
                "paged-scope",
            ),
        )
        .await
        .expect_err("corrupted cursor must fail closed");
    assert_eq!(
        corrupt_cursor.code,
        "CONTACT_POINTS_PRIVACY_SCOPE_CURSOR_INVALID"
    );
    assert_eq!(write_surface_counts(&admin).await, corrupt_cursor_before);

    let stale_before = write_surface_counts(&admin).await;
    let stale = adapter
        .execute(
            &definition,
            scope_request(TENANT_A, "party-scope", 2, 1, "", "stale-generation"),
        )
        .await
        .expect_err("stale topology generation must fail closed");
    assert_eq!(stale.code, "CONTACT_POINTS_PRIVACY_SCOPE_LINEAGE_INVALID");
    assert!(stale.retryable);
    assert_eq!(write_surface_counts(&admin).await, stale_before);

    let cross_tenant_before = write_surface_counts(&admin).await;
    let cross_tenant = adapter
        .execute(
            &definition,
            scope_request(TENANT_B, "party-scope", 1, 1, "", "cross-tenant"),
        )
        .await
        .expect_err("cross-tenant Contact Point scope must be concealed");
    assert_eq!(
        cross_tenant.code,
        "CONTACT_POINTS_PRIVACY_SCOPE_LINEAGE_INVALID"
    );
    assert_eq!(write_surface_counts(&admin).await, cross_tenant_before);

    insert_canonical_redirect(&admin, "party-redirected", "party-survivor").await;
    let redirected_before = write_surface_counts(&admin).await;
    let redirected = adapter
        .execute(
            &definition,
            scope_request(TENANT_A, "party-redirected", 2, 1, "", "redirected-party"),
        )
        .await
        .expect_err("noncanonical Party scope must fail closed");
    assert_eq!(
        redirected.code,
        "CONTACT_POINTS_PRIVACY_SCOPE_LINEAGE_INVALID"
    );
    assert_eq!(write_surface_counts(&admin).await, redirected_before);

    corrupt_contact_point_metadata(&admin, "contact-point-malformed").await;
    let malformed_before = write_surface_counts(&admin).await;
    let malformed = adapter
        .execute(
            &definition,
            scope_request(TENANT_A, "party-scope", 2, 128, "", "malformed"),
        )
        .await
        .expect_err("malformed Contact Point state must fail closed");
    assert_eq!(
        malformed.code,
        "CONTACT_POINTS_PRIVACY_SCOPE_STORED_STATE_INVALID"
    );
    assert_eq!(write_surface_counts(&admin).await, malformed_before);

    assert_no_query_side_writes(
        &admin,
        scope_request(TENANT_A, "party-scope", 2, 1, "", "final"),
        &adapter,
        &definition,
    )
    .await;
}

fn assert_response_omits_contact_point_state(bytes: &[u8], forbidden_values: &[&str]) {
    for forbidden in forbidden_values {
        assert!(
            !bytes
                .windows(forbidden.len())
                .any(|candidate| candidate == forbidden.as_bytes()),
            "response leaked forbidden Contact Point state value: {forbidden}"
        );
    }
}

async fn assert_no_query_side_writes(
    admin: &PgPool,
    request: QueryRequest,
    adapter: &ContactPointsPrivacyScopeQueryAdapter,
    definition: &crm_capability_runtime::CapabilityDefinition,
) {
    let before = write_surface_counts(admin).await;
    adapter
        .execute(definition, request)
        .await
        .expect("repeat final side-effect-free Contact Point scope");
    assert_eq!(write_surface_counts(admin).await, before);
}
