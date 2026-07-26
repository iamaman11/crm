#[path = "postgres_scope/support.rs"]
mod support;

use support::*;

use crm_capability_runtime::TransactionalCapabilityExecutor;
use crm_core_data::{PostgresDataStore, PostgresTransactionalAggregateExecutor};
use crm_customer_accounts_capability_adapter::{
    CREATE_CAPABILITY as CREATE_ACCOUNT_CAPABILITY, CustomerAccountCapabilityPlanner,
    capability_definition as account_definition,
};
use crm_customer_accounts_privacy_scope_adapter::{
    CAPABILITY_ID, CustomerAccountsPrivacyScopeQueryAdapter,
    customer_accounts_privacy_scope_definition,
};
use crm_parties_capability_adapter::{
    CREATE_CAPABILITY as CREATE_PARTY_CAPABILITY, PartyCapabilityPlanner,
    capability_definition as party_definition,
};
use crm_proto_contracts::crm::{accounts::v1 as accounts, customer_privacy::v1 as privacy};
use crm_query_runtime::{QueryExecutor, QueryRequest};
use prost::Message;
use sqlx::PgPool;
use std::sync::Arc;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn customer_accounts_scope_is_bounded_strict_tenant_bound_and_side_effect_free() {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!("skipping Customer Accounts privacy scope proof without DATABASE_URL");
        return;
    };
    let admin_url = std::env::var("ADMIN_DATABASE_URL")
        .expect("ADMIN_DATABASE_URL must accompany DATABASE_URL");

    let store = PostgresDataStore::connect(&database_url, 8)
        .await
        .expect("connect Customer Accounts privacy scope runtime store");
    let admin = PgPool::connect(&admin_url)
        .await
        .expect("connect Customer Accounts privacy scope evidence reader");
    let party_executor: Arc<dyn TransactionalCapabilityExecutor> =
        Arc::new(PostgresTransactionalAggregateExecutor::new(
            store.clone(),
            Arc::new(PartyCapabilityPlanner),
        ));
    let account_executor: Arc<dyn TransactionalCapabilityExecutor> =
        Arc::new(PostgresTransactionalAggregateExecutor::new(
            store.clone(),
            Arc::new(CustomerAccountCapabilityPlanner),
        ));
    let party_definition = party_definition(CREATE_PARTY_CAPABILITY).unwrap();
    let account_definition = account_definition(CREATE_ACCOUNT_CAPABILITY).unwrap();

    for (party_id, seed) in [
        ("party-scope", 11),
        ("party-other", 12),
        ("party-empty", 13),
        ("party-redirected", 14),
        ("party-survivor", 15),
    ] {
        create_party(
            &party_executor,
            &party_definition,
            TENANT_A,
            party_id,
            seed,
        )
        .await;
    }

    create_account(
        &account_executor,
        &account_definition,
        TENANT_A,
        "account-001",
        &[("party-scope", accounts::AccountPartyRole::Primary)],
        21,
    )
    .await;
    create_account(
        &account_executor,
        &account_definition,
        TENANT_A,
        "account-002",
        &[("party-other", accounts::AccountPartyRole::Primary)],
        22,
    )
    .await;
    create_account(
        &account_executor,
        &account_definition,
        TENANT_A,
        "account-003",
        &[
            ("party-other", accounts::AccountPartyRole::Primary),
            ("party-scope", accounts::AccountPartyRole::Member),
        ],
        23,
    )
    .await;
    create_account(
        &account_executor,
        &account_definition,
        TENANT_A,
        "account-malformed",
        &[("party-scope", accounts::AccountPartyRole::Primary)],
        24,
    )
    .await;
    create_account(
        &account_executor,
        &account_definition,
        TENANT_A,
        "account-redirected",
        &[("party-redirected", accounts::AccountPartyRole::Primary)],
        25,
    )
    .await;

    let adapter = CustomerAccountsPrivacyScopeQueryAdapter::new(store);
    let definition = customer_accounts_privacy_scope_definition().unwrap();

    let before_first = write_surface_counts(&admin).await;
    let first = adapter
        .execute(
            &definition,
            scope_request(TENANT_A, "party-scope", 1, 1, "", "first-page"),
        )
        .await
        .expect("read first authoritative Account scope page");
    assert_eq!(write_surface_counts(&admin).await, before_first);
    let first_wire = privacy::CustomerAccountsPrivacyScopeContributionResponse::decode(
        first.output.bytes.as_slice(),
    )
    .expect("decode first Customer Accounts scope response");
    let first_contribution = first_wire.contribution.expect("first contribution envelope");
    assert_eq!(
        first_contribution.owner_module_id,
        crm_customer_accounts::MODULE_ID
    );
    assert_eq!(first_contribution.capability_id, CAPABILITY_ID);
    assert_eq!(first_contribution.resources.len(), 1);
    assert_eq!(first_contribution.resources[0].resource_id, "account-001");
    assert_eq!(
        first_contribution.resources[0].evidence_class,
        privacy::PrivacyScopeEvidenceClass::RetainMinimizedEvidence as i32
    );
    assert!(
        !first
            .output
            .bytes
            .windows(b"Private Account account-001".len())
            .any(|value| value == b"Private Account account-001")
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
                "second-page",
            ),
        )
        .await
        .expect("read sparse second Account scope page");
    assert_eq!(write_surface_counts(&admin).await, before_second);
    let second_wire = privacy::CustomerAccountsPrivacyScopeContributionResponse::decode(
        second.output.bytes.as_slice(),
    )
    .expect("decode second Customer Accounts scope response");
    let second_contribution = second_wire
        .contribution
        .expect("second contribution envelope");
    assert_eq!(second_contribution.resources.len(), 1);
    assert_eq!(second_contribution.resources[0].resource_id, "account-003");
    let second_evidence = second_contribution
        .page_evidence
        .expect("second page evidence");
    assert_eq!(second_evidence.page_number, 2);
    assert_eq!(second_evidence.scanned_resource_count, 2);
    assert_eq!(second_evidence.emitted_resource_count, 1);
    assert!(!second_evidence.terminal_complete);

    let empty_before = write_surface_counts(&admin).await;
    let empty = adapter
        .execute(
            &definition,
            scope_request(TENANT_A, "party-empty", 1, 0, "", "empty"),
        )
        .await
        .expect("read empty authoritative Account scope");
    assert_eq!(write_surface_counts(&admin).await, empty_before);
    let empty_wire = privacy::CustomerAccountsPrivacyScopeContributionResponse::decode(
        empty.output.bytes.as_slice(),
    )
    .expect("decode empty Customer Accounts scope response");
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
                "cursor-rebound",
            ),
        )
        .await
        .expect_err("cursor rebound to another page size must fail");
    assert_eq!(
        rebound.code,
        "CUSTOMER_ACCOUNTS_PRIVACY_SCOPE_CURSOR_INVALID"
    );
    assert_eq!(write_surface_counts(&admin).await, rebound_before);

    let stale_before = write_surface_counts(&admin).await;
    let stale = adapter
        .execute(
            &definition,
            scope_request(TENANT_A, "party-scope", 2, 1, "", "stale-generation"),
        )
        .await
        .expect_err("stale topology generation must fail closed");
    assert_eq!(
        stale.code,
        "CUSTOMER_ACCOUNTS_PRIVACY_SCOPE_LINEAGE_INVALID"
    );
    assert!(stale.retryable);
    assert_eq!(write_surface_counts(&admin).await, stale_before);

    let cross_tenant_before = write_surface_counts(&admin).await;
    let cross_tenant = adapter
        .execute(
            &definition,
            scope_request(TENANT_B, "party-scope", 1, 1, "", "cross-tenant"),
        )
        .await
        .expect_err("cross-tenant Account scope must be concealed");
    assert_eq!(
        cross_tenant.code,
        "CUSTOMER_ACCOUNTS_PRIVACY_SCOPE_LINEAGE_INVALID"
    );
    assert_eq!(write_surface_counts(&admin).await, cross_tenant_before);

    insert_canonical_redirect(&admin, "party-redirected", "party-survivor").await;
    let redirected_before = write_surface_counts(&admin).await;
    let redirected = adapter
        .execute(
            &definition,
            scope_request(
                TENANT_A,
                "party-redirected",
                2,
                1,
                "",
                "redirected-party",
            ),
        )
        .await
        .expect_err("noncanonical Party scope must fail closed");
    assert_eq!(
        redirected.code,
        "CUSTOMER_ACCOUNTS_PRIVACY_SCOPE_LINEAGE_INVALID"
    );
    assert_eq!(write_surface_counts(&admin).await, redirected_before);

    corrupt_account_metadata(&admin, "account-malformed").await;
    let malformed_before = write_surface_counts(&admin).await;
    let malformed = adapter
        .execute(
            &definition,
            scope_request(TENANT_A, "party-scope", 2, 128, "", "malformed"),
        )
        .await
        .expect_err("malformed Account state must fail closed");
    assert_eq!(
        malformed.code,
        "CUSTOMER_ACCOUNTS_PRIVACY_SCOPE_STORED_STATE_INVALID"
    );
    assert_eq!(write_surface_counts(&admin).await, malformed_before);

    assert_no_query_side_writes(
        &admin,
        scope_request(TENANT_A, "party-empty", 2, 1, "", "final"),
        &adapter,
        &definition,
    )
    .await;
}

async fn assert_no_query_side_writes(
    admin: &PgPool,
    request: QueryRequest,
    adapter: &CustomerAccountsPrivacyScopeQueryAdapter,
    definition: &crm_capability_runtime::CapabilityDefinition,
) {
    let before = write_surface_counts(admin).await;
    adapter
        .execute(definition, request)
        .await
        .expect("repeat final side-effect-free Account scope");
    assert_eq!(write_surface_counts(admin).await, before);
}
