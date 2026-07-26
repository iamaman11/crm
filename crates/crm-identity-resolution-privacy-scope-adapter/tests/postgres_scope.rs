#[path = "postgres_scope/support.rs"]
mod support;

use support::*;

use crm_capability_runtime::TransactionalCapabilityExecutor;
use crm_core_data::{PostgresDataStore, PostgresTransactionalAggregateExecutor};
use crm_identity_resolution::{CanonicalPartyPair, DuplicateCandidateCaseId, PartyReference};
use crm_identity_resolution_capability_adapter::{
    IdentityResolutionCapabilityPlanner, MERGE_CAPABILITY, REGISTER_CAPABILITY, UNMERGE_CAPABILITY,
    capability_definition as identity_definition,
};
use crm_identity_resolution_privacy_scope_adapter::{
    IdentityResolutionPrivacyScopeQueryAdapter, identity_resolution_privacy_scope_definition,
};
use crm_parties_capability_adapter::{
    CREATE_CAPABILITY as CREATE_PARTY_CAPABILITY, PartyCapabilityPlanner,
    capability_definition as party_definition,
};
use crm_proto_contracts::crm::customer_privacy::v1 as privacy;
use crm_query_runtime::{QueryExecutor, QueryRequest};
use prost::Message;
use sqlx::PgPool;
use std::collections::BTreeMap;
use std::sync::Arc;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn identity_resolution_scope_is_alias_aware_complete_reference_only_and_side_effect_free() {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!("skipping Identity Resolution privacy scope proof without DATABASE_URL");
        return;
    };
    let admin_url = std::env::var("ADMIN_DATABASE_URL")
        .expect("ADMIN_DATABASE_URL must accompany DATABASE_URL");

    let store = PostgresDataStore::connect(&database_url, 8)
        .await
        .expect("connect Identity Resolution privacy scope runtime store");
    let admin = PgPool::connect(&admin_url)
        .await
        .expect("connect Identity Resolution privacy scope evidence reader");
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
    let candidate_register = identity_definition(REGISTER_CAPABILITY).unwrap();
    let merge_execute = identity_definition(MERGE_CAPABILITY).unwrap();
    let merge_unmerge = identity_definition(UNMERGE_CAPABILITY).unwrap();

    for (party_id, seed) in [
        ("party-canonical", 11),
        ("party-alias-one", 12),
        ("party-alias-two", 13),
        ("party-alias-three", 14),
        ("party-candidate-other-one", 15),
        ("party-candidate-other-two", 16),
        ("party-unrelated-source", 17),
        ("party-unrelated-survivor", 18),
        ("party-unmerge-survivor", 19),
        ("party-extra-source", 20),
        ("party-extra-survivor", 21),
    ] {
        create_party(&party_executor, &party_create, TENANT_A, party_id, seed).await;
    }
    create_party(
        &party_executor,
        &party_create,
        TENANT_B,
        "party-canonical",
        31,
    )
    .await;

    register_candidate(
        &identity_executor,
        &candidate_register,
        TENANT_A,
        "party-alias-two",
        "party-candidate-other-one",
        "deterministic.alias-two.v1",
        "name.exact",
        "evidence://candidate/alias-two",
        41,
    )
    .await;
    register_candidate(
        &identity_executor,
        &candidate_register,
        TENANT_A,
        "party-alias-three",
        "party-candidate-other-two",
        "deterministic.alias-three.v1",
        "email.exact",
        "evidence://candidate/alias-three",
        42,
    )
    .await;
    register_candidate(
        &identity_executor,
        &candidate_register,
        TENANT_A,
        "party-unrelated-source",
        "party-unrelated-survivor",
        "deterministic.unrelated.v1",
        "phone.exact",
        "evidence://candidate/unrelated",
        43,
    )
    .await;

    merge_party(
        &identity_executor,
        &merge_execute,
        TENANT_A,
        "merge-a-chain-alias-two",
        "party-alias-two",
        "party-alias-one",
        "party-alias-two",
        "display_name",
        "evidence://merge/chain-one",
        51,
    )
    .await;
    merge_party(
        &identity_executor,
        &merge_execute,
        TENANT_A,
        "merge-b-chain-alias-one",
        "party-alias-one",
        "party-canonical",
        "party-alias-one",
        "display_name",
        "evidence://merge/chain-two",
        52,
    )
    .await;
    merge_party(
        &identity_executor,
        &merge_execute,
        TENANT_A,
        "merge-c-converging-alias",
        "party-alias-three",
        "party-canonical",
        "party-alias-three",
        "primary_email",
        "evidence://merge/converging",
        53,
    )
    .await;
    merge_party(
        &identity_executor,
        &merge_execute,
        TENANT_A,
        "merge-d-historical-unmerged",
        "party-canonical",
        "party-unmerge-survivor",
        "party-canonical",
        "legal_name",
        "evidence://merge/unmerged",
        54,
    )
    .await;
    unmerge_party(
        &identity_executor,
        &merge_unmerge,
        TENANT_A,
        "merge-d-historical-unmerged",
        1,
        1,
        55,
    )
    .await;
    merge_party(
        &identity_executor,
        &merge_execute,
        TENANT_A,
        "merge-e-provenance-only",
        "party-unrelated-source",
        "party-unrelated-survivor",
        "party-alias-three",
        "preferred_language",
        "evidence://merge/provenance-only",
        56,
    )
    .await;
    merge_party(
        &identity_executor,
        &merge_execute,
        TENANT_A,
        "merge-z-unrelated",
        "party-extra-source",
        "party-extra-survivor",
        "party-extra-source",
        "display_name",
        "evidence://merge/unrelated",
        57,
    )
    .await;

    let generation = current_generation(&admin, TENANT_A).await;
    assert!(generation > 1, "merge topology generation must advance");
    assert_eq!(current_generation(&admin, TENANT_B).await, 1);

    let relevant_candidate_ids = [
        candidate_id("party-alias-two", "party-candidate-other-one"),
        candidate_id("party-alias-three", "party-candidate-other-two"),
    ];
    let unrelated_candidate_id =
        candidate_id("party-unrelated-source", "party-unrelated-survivor");
    let adapter = IdentityResolutionPrivacyScopeQueryAdapter::new(store);
    let definition = identity_resolution_privacy_scope_definition().unwrap();

    let before_pages = write_surface_counts(&admin).await;
    let first = adapter
        .execute(
            &definition,
            scope_request(
                TENANT_A,
                "party-canonical",
                generation,
                2,
                "",
                "paged-identity-scope",
            ),
        )
        .await
        .expect("read first Identity Resolution privacy scope page");
    assert_eq!(write_surface_counts(&admin).await, before_pages);
    assert_response_omits_private_identity_state(&first.output.bytes);
    let first = decode(first.output.bytes.as_slice());
    assert_eq!(first.resources.len(), 2);
    assert!(first
        .resources
        .iter()
        .all(|resource| resource.resource_type == "identity_resolution.candidate_case"));
    let first_evidence = first.page_evidence.expect("first page evidence");
    assert!(!first_evidence.terminal_complete);
    assert!(!first_evidence.next_cursor.is_empty());

    let second = execute_page(
        &adapter,
        &definition,
        scope_request(
            TENANT_A,
            "party-canonical",
            generation,
            2,
            &first_evidence.next_cursor,
            "paged-identity-scope",
        ),
        &admin,
    )
    .await;
    assert_eq!(second.resources.len(), 2);
    let second_evidence = second.page_evidence.expect("second page evidence");
    assert!(!second_evidence.terminal_complete);

    let third = execute_page(
        &adapter,
        &definition,
        scope_request(
            TENANT_A,
            "party-canonical",
            generation,
            2,
            &second_evidence.next_cursor,
            "paged-identity-scope",
        ),
        &admin,
    )
    .await;
    assert_eq!(third.resources.len(), 2);
    let third_evidence = third.page_evidence.expect("third page evidence");
    assert!(!third_evidence.terminal_complete);

    let fourth = execute_page(
        &adapter,
        &definition,
        scope_request(
            TENANT_A,
            "party-canonical",
            generation,
            2,
            &third_evidence.next_cursor,
            "paged-identity-scope",
        ),
        &admin,
    )
    .await;
    assert_eq!(fourth.resources.len(), 1);
    let fourth_evidence = fourth.page_evidence.expect("terminal page evidence");
    assert!(fourth_evidence.terminal_complete);
    assert!(fourth_evidence.next_cursor.is_empty());

    let mut collected = BTreeMap::new();
    for contribution in [first, second, third, fourth] {
        for resource in contribution.resources {
            collected.insert(resource.resource_id, (resource.resource_type, resource.resource_version));
        }
    }
    assert_eq!(collected.len(), 7);
    for candidate_id in &relevant_candidate_ids {
        assert_eq!(
            collected.get(candidate_id),
            Some(&("identity_resolution.candidate_case".to_owned(), 1))
        );
    }
    assert!(!collected.contains_key(&unrelated_candidate_id));
    for operation_id in [
        "merge-a-chain-alias-two",
        "merge-b-chain-alias-one",
        "merge-c-converging-alias",
        "merge-d-historical-unmerged",
        "merge-e-provenance-only",
    ] {
        assert_eq!(
            collected.get(operation_id).map(|value| value.0.as_str()),
            Some("identity_resolution.merge_operation")
        );
    }
    assert_eq!(
        collected
            .get("merge-d-historical-unmerged")
            .map(|value| value.1),
        Some(2)
    );
    assert!(!collected.contains_key("merge-z-unrelated"));

    let empty_before = write_surface_counts(&admin).await;
    let empty = adapter
        .execute(
            &definition,
            scope_request(TENANT_B, "party-canonical", 1, 0, "", "cross-tenant-empty"),
        )
        .await
        .expect("cross-tenant owner scope must be empty and concealed");
    assert_eq!(write_surface_counts(&admin).await, empty_before);
    let empty = decode(empty.output.bytes.as_slice());
    assert!(empty.resources.is_empty());
    assert!(empty.page_evidence.unwrap().terminal_complete);

    let stale_before = write_surface_counts(&admin).await;
    let stale = adapter
        .execute(
            &definition,
            scope_request(
                TENANT_A,
                "party-canonical",
                generation - 1,
                2,
                "",
                "stale-generation",
            ),
        )
        .await
        .expect_err("stale topology generation must fail closed");
    assert_eq!(stale.code, "IDENTITY_RESOLUTION_PRIVACY_SCOPE_LINEAGE_INVALID");
    assert!(stale.retryable);
    assert_eq!(write_surface_counts(&admin).await, stale_before);

    let rebound_before = write_surface_counts(&admin).await;
    let rebound = adapter
        .execute(
            &definition,
            scope_request(
                TENANT_A,
                "party-canonical",
                generation,
                3,
                &first_evidence.next_cursor,
                "paged-identity-scope",
            ),
        )
        .await
        .expect_err("cursor rebound to another page size must fail");
    assert_eq!(rebound.code, "IDENTITY_RESOLUTION_PRIVACY_SCOPE_CURSOR_INVALID");
    assert_eq!(write_surface_counts(&admin).await, rebound_before);

    corrupt_candidate_metadata(&admin, &relevant_candidate_ids[0]).await;
    let malformed_before = write_surface_counts(&admin).await;
    let malformed = adapter
        .execute(
            &definition,
            scope_request(
                TENANT_A,
                "party-canonical",
                generation,
                128,
                "",
                "malformed-candidate",
            ),
        )
        .await
        .expect_err("malformed candidate state must fail closed");
    assert_eq!(
        malformed.code,
        "IDENTITY_RESOLUTION_PRIVACY_SCOPE_CANDIDATE_STATE_INVALID"
    );
    assert_eq!(write_surface_counts(&admin).await, malformed_before);
}

async fn execute_page(
    adapter: &IdentityResolutionPrivacyScopeQueryAdapter,
    definition: &crm_capability_runtime::CapabilityDefinition,
    request: QueryRequest,
    admin: &PgPool,
) -> privacy::PrivacyScopeContributionResponseEnvelope {
    let before = write_surface_counts(admin).await;
    let result = adapter
        .execute(definition, request)
        .await
        .expect("read next Identity Resolution privacy scope page");
    assert_eq!(write_surface_counts(admin).await, before);
    assert_response_omits_private_identity_state(&result.output.bytes);
    decode(result.output.bytes.as_slice())
}

fn decode(bytes: &[u8]) -> privacy::PrivacyScopeContributionResponseEnvelope {
    privacy::IdentityResolutionPrivacyScopeContributionResponse::decode(bytes)
        .expect("decode Identity Resolution privacy scope response")
        .contribution
        .expect("Identity Resolution contribution envelope")
}

fn candidate_id(first: &str, second: &str) -> String {
    let pair = CanonicalPartyPair::try_new(
        PartyReference::try_new(first).unwrap(),
        PartyReference::try_new(second).unwrap(),
    )
    .unwrap();
    DuplicateCandidateCaseId::for_pair(&pair)
        .unwrap()
        .as_str()
        .to_owned()
}

fn assert_response_omits_private_identity_state(bytes: &[u8]) {
    for forbidden in [
        "party-canonical",
        "party-alias-one",
        "party-alias-two",
        "party-alias-three",
        "party-candidate-other-one",
        "party-candidate-other-two",
        "party-unrelated-source",
        "party-unrelated-survivor",
        "deterministic.alias-two.v1",
        "name.exact",
        "email.exact",
        "party.normalized",
        "evidence://candidate/alias-two",
        "evidence://merge/provenance-only",
        "approval://merge-e-provenance-only",
        "actor-a",
        "duplicate.confirmed",
        "preferred_language",
        "display_name",
    ] {
        assert!(
            !bytes
                .windows(forbidden.len())
                .any(|candidate| candidate == forbidden.as_bytes()),
            "response leaked forbidden Identity Resolution state value: {forbidden}"
        );
    }
}
