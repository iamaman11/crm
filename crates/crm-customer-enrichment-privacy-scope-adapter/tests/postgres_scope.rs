#![allow(clippy::too_many_arguments)]

use crm_capability_plan_support::{self as support, PersistedPayloadContract};
use crm_capability_runtime::{
    CapabilityDefinition, CapabilityRequest, TransactionalCapabilityExecutor,
};
use crm_core_data::{PostgresDataStore, PostgresTransactionalAggregateExecutor};
use crm_customer_enrichment::{
    APPLICATION_ATTEMPT_RECORD_TYPE, ApprovalRequirement, ENRICHMENT_REQUEST_RECORD_TYPE,
    EnrichmentRequest, EnrichmentRequestDraft, LIFECYCLE_STATE_RETENTION_POLICY_ID,
    LIFECYCLE_STATE_SCHEMA_VERSION, MAPPING_VERSION_RECORD_TYPE, MappingDraft,
    MappingNormalization, MappingVersion, PROVIDER_PROFILE_VERSION_RECORD_TYPE,
    PROVIDER_RESPONSE_CONFLICT_RECORD_TYPE, PROVIDER_RESPONSE_RECEIPT_RECORD_TYPE,
    PROVIDER_RESPONSE_RECEIPT_STATE_MAXIMUM_BYTES, PROVIDER_RESPONSE_RECEIPT_STATE_SCHEMA_ID,
    PROVIDER_USAGE_ENTRY_RECORD_TYPE, PROVIDER_USAGE_ENTRY_STATE_MAXIMUM_BYTES,
    PROVIDER_USAGE_ENTRY_STATE_RETENTION_POLICY_ID, PROVIDER_USAGE_ENTRY_STATE_SCHEMA_ID,
    PROVIDER_USAGE_ENTRY_STATE_SCHEMA_VERSION, ProviderProfileDraft, ProviderProfileVersion,
    ProviderResponseClass, ProviderResponseConflict, ProviderResponseConflictDraft,
    ProviderResponseReceipt, ProviderResponseReceiptDraft, ProviderUsageEntry,
    ProviderUsageEntryDraft, ProviderUsageKind, REVIEW_DECISION_RECORD_TYPE, RawPayloadPolicy,
    RequestPolicyEvidence, ReviewDecision, ReviewDecisionKind, SUGGESTION_RECORD_TYPE, Suggestion,
    SuggestionDraft, TargetField, TargetSnapshot, encode_application_attempt_state,
    encode_enrichment_request_state, encode_mapping_version_state,
    encode_provider_profile_version_state, encode_provider_response_conflict_state,
    encode_provider_response_receipt_state, encode_provider_usage_entry_state,
    encode_review_decision_state, encode_suggestion_state,
    provider_response_receipt_state_descriptor_hash, provider_usage_entry_state_descriptor_hash,
};
use crm_customer_enrichment_application_adapter::application_attempt_persisted_contract;
use crm_customer_enrichment_capability_adapter::{
    MODULE_ID, REQUEST_PARTY_RELATIONSHIP_TYPE, REQUEST_PARTY_SOURCE_RECORD_TYPE,
    enrichment_request_persisted_contract, mapping_persisted_contract,
    provider_profile_persisted_contract,
};
use crm_customer_enrichment_privacy_scope_adapter::{
    CAPABILITY_ID, CAPABILITY_VERSION, CONTRACT_SCHEMA_VERSION,
    CustomerEnrichmentPrivacyScopeQueryAdapter, INPUT_MAXIMUM_BYTES, INPUT_RETENTION_POLICY_ID,
    INPUT_SCHEMA_ID, customer_enrichment_privacy_scope_definition,
};
use crm_customer_enrichment_provider_process_composition::provider_response_conflict_persisted_contract;
use crm_customer_enrichment_review_adapter::{
    review_decision_persisted_contract, suggestion_persisted_contract,
};
use crm_customer_privacy::{CANONICAL_SCOPE_REGISTRY_VERSION, OwnerScopeRegistry};
use crm_identity_resolution_capability_adapter::{
    IdentityResolutionCapabilityPlanner, MERGE_CAPABILITY,
    capability_definition as identity_definition,
};
use crm_module_sdk::{
    ActorId, BusinessTransactionId, CapabilityId, CapabilityVersion, CausationId, CorrelationId,
    DataClass, ExecutionContext, IdempotencyKey, ModuleExecutionContext, ModuleId, PayloadEncoding,
    RecordId, RequestId, RetentionPolicyId, SchemaId, SchemaVersion, TenantId, TraceId,
    TypedPayload,
};
use crm_parties_capability_adapter::{
    CREATE_CAPABILITY as CREATE_PARTY_CAPABILITY, PartyCapabilityPlanner,
    capability_definition as party_definition,
};
use crm_proto_contracts::{
    crm::{
        customer::v1 as customer, customer_privacy::v1 as privacy,
        identity_resolution::v1 as identity, parties::v1 as parties,
    },
    message_descriptor_hash,
};
use crm_query_runtime::{QueryExecutionContext, QueryExecutor, QueryRequest};
use prost::Message;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use std::collections::BTreeMap;
use std::sync::Arc;

const TENANT_A: &str = "tenant-a";
const TENANT_B: &str = "tenant-b";
const ACTOR: &str = "privacy-worker";
const REQUEST_PARTY_LINK_SCHEMA_ID: &str = "crm.customer-enrichment.request.party-link";
const REQUEST_PARTY_LINK_SCHEMA_VERSION: &str = "1.0.0";
const REQUEST_PARTY_LINK_MAXIMUM_BYTES: u64 = 1_024;
const REQUEST_PARTY_LINK_DESCRIPTOR_HASH: [u8; 32] = [
    234, 78, 62, 183, 114, 97, 170, 255, 30, 94, 169, 60, 144, 234, 17, 235, 225, 88, 121,
    223, 86, 225, 45, 149, 201, 194, 155, 186, 10, 226, 131, 230,
];

#[derive(Debug, Clone)]
struct GraphIds {
    request: String,
    receipt: String,
    conflict: String,
    suggestion: String,
    review: String,
    application: String,
    usage: String,
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn enrichment_scope_is_relationship_rooted_strict_minimized_and_side_effect_free() {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!("skipping Customer Enrichment privacy scope proof without DATABASE_URL");
        return;
    };
    let admin_url = std::env::var("ADMIN_DATABASE_URL")
        .expect("ADMIN_DATABASE_URL must accompany DATABASE_URL");

    let store = PostgresDataStore::connect(&database_url, 8)
        .await
        .expect("connect Customer Enrichment privacy runtime store");
    let admin = PgPool::connect(&admin_url)
        .await
        .expect("connect Customer Enrichment privacy evidence reader");
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
        "merge-enrichment-alias",
        "party-alias",
        "party-canonical",
        31,
    )
    .await;

    let (profile, mapping) = definitions();
    insert_definitions(&admin, TENANT_A, &profile, &mapping).await;
    insert_definitions(&admin, TENANT_B, &profile, &mapping).await;

    let alias = insert_complete_graph(
        &admin,
        TENANT_A,
        "party-alias",
        "Private Proposed Alias Name",
        1_000,
        &profile,
        &mapping,
    )
    .await;
    let canonical = insert_complete_graph(
        &admin,
        TENANT_A,
        "party-canonical",
        "Private Proposed Canonical Name",
        2_000,
        &profile,
        &mapping,
    )
    .await;
    let unrelated = insert_complete_graph(
        &admin,
        TENANT_A,
        "party-unrelated",
        "Private Proposed Unrelated Name",
        3_000,
        &profile,
        &mapping,
    )
    .await;
    let tenant_b = insert_complete_graph(
        &admin,
        TENANT_B,
        "party-canonical",
        "Private Proposed Tenant B Name",
        4_000,
        &profile,
        &mapping,
    )
    .await;

    prove_relationship_and_record_primary_key_scans(&admin, TENANT_A).await;

    let generation_a = current_generation(&admin, TENANT_A).await;
    let definition = customer_enrichment_privacy_scope_definition().unwrap();
    let adapter = CustomerEnrichmentPrivacyScopeQueryAdapter::new(store);
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
                    3,
                    &cursor,
                    "enrichment-pages",
                ),
            )
            .await
            .expect("enumerate authoritative Customer Enrichment privacy scope");
        assert_eq!(write_surface_counts(&admin).await, before);
        assert_response_omits_private_enrichment_values(&result.output.bytes);
        encoded_pages.push(result.output.bytes.clone());
        let response = decode(&result.output.bytes);
        let contribution = response.contribution.unwrap();
        assert_eq!(contribution.owner_module_id, MODULE_ID);
        assert_eq!(contribution.capability_id, CAPABILITY_ID);
        assert_eq!(contribution.capability_version, CAPABILITY_VERSION);
        let evidence = contribution.page_evidence.unwrap();
        assert_eq!(evidence.page_number, page);
        assert_eq!(evidence.scanned_resource_count, 24);
        assert_eq!(
            evidence.emitted_resource_count as usize,
            contribution.resources.len()
        );
        resources.extend(contribution.resources);
        if evidence.terminal_complete {
            assert!(evidence.next_cursor.is_empty());
            break;
        }
        assert!(!evidence.next_cursor.is_empty());
        cursor = evidence.next_cursor;
        page += 1;
        assert!(page <= 6, "seven-family pagination must terminate");
    }

    assert_eq!(page, 5);
    assert_eq!(resources.len(), 14);
    let by_type = resources.into_iter().fold(
        BTreeMap::<String, Vec<String>>::new(),
        |mut output, resource| {
            assert_eq!(
                resource.data_class,
                privacy::CustomerDataClass::Personal as i32
            );
            assert_eq!(
                resource.evidence_class,
                privacy::PrivacyScopeEvidenceClass::RetainMinimizedEvidence as i32
            );
            output
                .entry(resource.resource_type)
                .or_default()
                .push(resource.resource_id);
            output
        },
    );
    assert_eq!(by_type, expected_two_graphs(&alias, &canonical));
    assert!(!by_type.contains_key(PROVIDER_PROFILE_VERSION_RECORD_TYPE));
    assert!(!by_type.contains_key(MAPPING_VERSION_RECORD_TYPE));

    for forbidden_id in graph_values(&unrelated) {
        assert!(
            encoded_pages.iter().all(|bytes| !contains(bytes, forbidden_id)),
            "unrelated resource leaked: {forbidden_id}"
        );
    }
    for forbidden_id in graph_values(&tenant_b) {
        assert!(
            encoded_pages.iter().all(|bytes| !contains(bytes, forbidden_id)),
            "cross-tenant resource leaked: {forbidden_id}"
        );
    }

    let generation_b = current_generation(&admin, TENANT_B).await;
    let tenant_b_response = adapter
        .execute(
            &definition,
            scope_request(
                TENANT_B,
                "party-canonical",
                generation_b,
                20,
                "",
                "enrichment-tenant-b",
            ),
        )
        .await
        .expect("enumerate tenant B Enrichment scope");
    assert_eq!(
        decode(&tenant_b_response.output.bytes)
            .contribution
            .unwrap()
            .resources
            .len(),
        7
    );
    assert_eq!(write_surface_counts(&admin).await, before);

    let stale = adapter
        .execute(
            &definition,
            scope_request(
                TENANT_A,
                "party-canonical",
                generation_a - 1,
                3,
                "",
                "enrichment-stale",
            ),
        )
        .await
        .expect_err("stale topology generation must fail closed");
    assert_eq!(
        stale.code,
        "CUSTOMER_ENRICHMENT_PRIVACY_SCOPE_LINEAGE_INVALID"
    );
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
                4,
                &first_page.next_cursor,
                "enrichment-pages",
            ),
        )
        .await
        .expect_err("cursor page-size rebinding must fail closed");
    assert_eq!(
        rebound.code,
        "CUSTOMER_ENRICHMENT_PRIVACY_SCOPE_CURSOR_INVALID"
    );
    assert_eq!(write_surface_counts(&admin).await, before);

    set_record_data_class(&admin, TENANT_A, &unrelated.usage, "personal").await;
    let malformed_baseline = write_surface_counts(&admin).await;
    let malformed = adapter
        .execute(
            &definition,
            scope_request(
                TENANT_A,
                "party-canonical",
                generation_a,
                20,
                "",
                "enrichment-malformed",
            ),
        )
        .await
        .expect_err("malformed unrelated owner persistence must fail closed");
    assert_eq!(
        malformed.code,
        "CUSTOMER_ENRICHMENT_PRIVACY_SCOPE_STORED_STATE_INVALID"
    );
    assert_eq!(write_surface_counts(&admin).await, malformed_baseline);
    set_record_data_class(&admin, TENANT_A, &unrelated.usage, "confidential").await;

    delete_record(
        &admin,
        TENANT_A,
        PROVIDER_RESPONSE_RECEIPT_RECORD_TYPE,
        &alias.receipt,
    )
    .await;
    let association_baseline = write_surface_counts(&admin).await;
    let orphaned = adapter
        .execute(
            &definition,
            scope_request(
                TENANT_A,
                "party-canonical",
                generation_a,
                20,
                "",
                "enrichment-orphaned",
            ),
        )
        .await
        .expect_err("missing receipt must fail closed");
    assert_eq!(
        orphaned.code,
        "CUSTOMER_ENRICHMENT_PRIVACY_SCOPE_ASSOCIATION_STATE_INVALID"
    );
    assert_eq!(write_surface_counts(&admin).await, association_baseline);
}

fn definitions() -> (ProviderProfileVersion, MappingVersion) {
    let profile = ProviderProfileVersion::publish(ProviderProfileDraft {
        provider_key: "private-provider".to_owned(),
        adapter_kind: "private-adapter".to_owned(),
        adapter_contract_version: "1.0.0".to_owned(),
        supported_target_fields: vec![TargetField::PartyDisplayName],
        purpose_codes: vec!["customer_enrichment".to_owned()],
        license_id: "Private Provider License".to_owned(),
        permitted_use_class: "customer_master".to_owned(),
        residency_region: "eu".to_owned(),
        retention_days: 30,
        raw_payload_policy: RawPayloadPolicy::GovernedProtectedEvidence,
        credential_handle_aliases: vec!["private-secret-handle".to_owned()],
        effective_at_unix_ms: 1,
        expires_at_unix_ms: None,
    })
    .unwrap();
    let mapping = MappingVersion::publish(MappingDraft {
        mapping_key: "private-display-name-mapping".to_owned(),
        provider_profile_version_id: profile.version_id().clone(),
        provider_response_field_path: "private.payload.name".to_owned(),
        target_field: TargetField::PartyDisplayName,
        normalization: MappingNormalization::CanonicalPartyDisplayNameV1,
        maximum_suggestions_per_response: 1,
        confidence_required: false,
    })
    .unwrap();
    (profile, mapping)
}

async fn insert_definitions(
    admin: &PgPool,
    tenant: &str,
    profile: &ProviderProfileVersion,
    mapping: &MappingVersion,
) {
    insert_record(
        admin,
        tenant,
        PROVIDER_PROFILE_VERSION_RECORD_TYPE,
        profile.version_id().as_str(),
        1,
        provider_profile_persisted_contract(),
        DataClass::Confidential,
        encode_provider_profile_version_state(profile).unwrap(),
    )
    .await;
    insert_record(
        admin,
        tenant,
        MAPPING_VERSION_RECORD_TYPE,
        mapping.version_id().as_str(),
        1,
        mapping_persisted_contract(),
        DataClass::Confidential,
        encode_mapping_version_state(mapping).unwrap(),
    )
    .await;
}

async fn insert_complete_graph(
    admin: &PgPool,
    tenant: &str,
    party_id: &str,
    proposed_value: &str,
    seed: u64,
    profile: &ProviderProfileVersion,
    mapping: &MappingVersion,
) -> GraphIds {
    let target = TargetSnapshot::try_new(party_id, 1, TargetField::PartyDisplayName).unwrap();
    let mut request = EnrichmentRequest::create(EnrichmentRequestDraft {
        tenant_id: TenantId::try_new(tenant).unwrap(),
        requested_by: ActorId::try_new(format!("private-requester-{seed}")).unwrap(),
        idempotency_key: IdempotencyKey::try_new(format!("private-request-key-{seed}")).unwrap(),
        target: target.clone(),
        provider_profile_version_id: profile.version_id().clone(),
        mapping_version_id: mapping.version_id().clone(),
        requested_fields: vec![TargetField::PartyDisplayName],
        policy_evidence: RequestPolicyEvidence::try_new(
            "customer_enrichment",
            "consent",
            Some(format!("private-consent-evidence-{seed}")),
            "1.0.0",
        )
        .unwrap(),
        created_at_unix_ms: seed,
        deadline_at_unix_ms: seed + 100,
        expires_at_unix_ms: seed + 1_000,
    })
    .unwrap();
    request.queue(seed + 1).unwrap();
    request.mark_dispatched(seed + 2).unwrap();
    let receipt = ProviderResponseReceipt::record(ProviderResponseReceiptDraft {
        request_id: request.request_id().clone(),
        provider_profile_version_id: profile.version_id().clone(),
        mapping_version_id: mapping.version_id().clone(),
        replay_key: format!("private-replay-{seed}"),
        provider_correlation_id: Some(format!("private-correlation-{seed}")),
        response_class: ProviderResponseClass::Success,
        canonical_response_digest: [7; 32],
        provider_observed_at_unix_ms: Some(seed + 2),
        retrieved_at_unix_ms: seed + 3,
        metered_units: 1,
        protected_evidence_reference: Some(format!("private-provider-evidence-{seed}")),
    })
    .unwrap();
    request
        .record_response(receipt.receipt_id().clone(), seed + 3)
        .unwrap();
    request.mark_suggestions_materialized(seed + 4).unwrap();
    request.complete(seed + 5).unwrap();

    let suggestion = Suggestion::materialize(SuggestionDraft {
        request_id: request.request_id().clone(),
        response_receipt_id: receipt.receipt_id().clone(),
        provider_profile_version_id: profile.version_id().clone(),
        mapping_version_id: mapping.version_id().clone(),
        target,
        proposed_value: proposed_value.to_owned(),
        observed_at_unix_ms: Some(seed + 2),
        retrieved_at_unix_ms: seed + 3,
        effective_at_unix_ms: seed + 3,
        fresh_until_unix_ms: seed + 500,
        expires_at_unix_ms: seed + 900,
        confidence_basis_points: Some(9_500),
        purpose_code: "customer_enrichment".to_owned(),
        legal_basis_code: "consent".to_owned(),
        license_id: "Private Provider License".to_owned(),
        permitted_use_class: "customer_master".to_owned(),
        residency_region: "eu".to_owned(),
        retention_days: 30,
        consent_evidence_reference: Some(format!("private-consent-evidence-{seed}")),
        evidence_references: vec![format!("private-suggestion-evidence-{seed}")],
    })
    .unwrap();
    let review = ReviewDecision::decide(
        &suggestion,
        ActorId::try_new(format!("private-reviewer-{seed}")).unwrap(),
        ReviewDecisionKind::Accepted,
        "1.0.0",
        "private-approved",
        ApprovalRequirement::NotRequired,
        None,
        seed + 4,
        Some(seed + 800),
    )
    .unwrap();
    let application = crm_customer_enrichment::ApplicationAttempt::plan(
        TenantId::try_new(tenant).unwrap(),
        &suggestion,
        &review,
        0,
        seed + 5,
    )
    .unwrap();
    let conflict = ProviderResponseConflict::record(ProviderResponseConflictDraft {
        tenant_id: TenantId::try_new(tenant).unwrap(),
        request_id: request.request_id().clone(),
        retry_generation: 0,
        first_receipt_id: receipt.receipt_id().clone(),
        conflicting_semantic_fingerprint: [9; 32],
        detected_at_unix_ms: seed + 4,
    })
    .unwrap();
    let usage = ProviderUsageEntry::record(ProviderUsageEntryDraft {
        request_id: request.request_id().clone(),
        response_receipt_id: Some(receipt.receipt_id().clone()),
        provider_profile_version_id: profile.version_id().clone(),
        kind: ProviderUsageKind::ResponseReceived,
        metered_units: 1,
        quota_bucket: None,
        quota_remaining: None,
        provider_observed_at_unix_ms: Some(seed + 2),
        recorded_at_unix_ms: seed + 3,
        safe_provider_code: Some("private-provider-code".to_owned()),
    })
    .unwrap();

    insert_record(
        admin,
        tenant,
        ENRICHMENT_REQUEST_RECORD_TYPE,
        request.request_id().as_str(),
        6,
        enrichment_request_persisted_contract(),
        DataClass::Personal,
        encode_enrichment_request_state(&request).unwrap(),
    )
    .await;
    insert_request_relationship(admin, tenant, party_id, request.request_id().as_str()).await;
    insert_record(
        admin,
        tenant,
        PROVIDER_RESPONSE_RECEIPT_RECORD_TYPE,
        receipt.receipt_id().as_str(),
        1,
        provider_response_receipt_contract(),
        DataClass::Personal,
        encode_provider_response_receipt_state(&receipt).unwrap(),
    )
    .await;
    insert_record(
        admin,
        tenant,
        PROVIDER_RESPONSE_CONFLICT_RECORD_TYPE,
        conflict.conflict_id().as_str(),
        1,
        provider_response_conflict_persisted_contract(),
        DataClass::Confidential,
        encode_provider_response_conflict_state(&conflict).unwrap(),
    )
    .await;
    insert_record(
        admin,
        tenant,
        SUGGESTION_RECORD_TYPE,
        suggestion.suggestion_id().as_str(),
        1,
        suggestion_persisted_contract(),
        DataClass::Personal,
        encode_suggestion_state(&suggestion).unwrap(),
    )
    .await;
    insert_record(
        admin,
        tenant,
        REVIEW_DECISION_RECORD_TYPE,
        review.decision_id().as_str(),
        1,
        review_decision_persisted_contract(),
        DataClass::Personal,
        encode_review_decision_state(&review).unwrap(),
    )
    .await;
    insert_record(
        admin,
        tenant,
        APPLICATION_ATTEMPT_RECORD_TYPE,
        application.attempt_id().as_str(),
        1,
        application_attempt_persisted_contract(),
        DataClass::Personal,
        encode_application_attempt_state(&application).unwrap(),
    )
    .await;
    insert_record(
        admin,
        tenant,
        PROVIDER_USAGE_ENTRY_RECORD_TYPE,
        usage.usage_entry_id().as_str(),
        1,
        provider_usage_contract(),
        DataClass::Confidential,
        encode_provider_usage_entry_state(&usage).unwrap(),
    )
    .await;

    GraphIds {
        request: request.request_id().as_str().to_owned(),
        receipt: receipt.receipt_id().as_str().to_owned(),
        conflict: conflict.conflict_id().as_str().to_owned(),
        suggestion: suggestion.suggestion_id().as_str().to_owned(),
        review: review.decision_id().as_str().to_owned(),
        application: application.attempt_id().as_str().to_owned(),
        usage: usage.usage_entry_id().as_str().to_owned(),
    }
}

fn provider_response_receipt_contract() -> PersistedPayloadContract<'static> {
    PersistedPayloadContract {
        owner: MODULE_ID,
        schema_id: PROVIDER_RESPONSE_RECEIPT_STATE_SCHEMA_ID,
        schema_version: LIFECYCLE_STATE_SCHEMA_VERSION,
        descriptor_hash: provider_response_receipt_state_descriptor_hash(),
        maximum_size_bytes: PROVIDER_RESPONSE_RECEIPT_STATE_MAXIMUM_BYTES,
        retention_policy_id: LIFECYCLE_STATE_RETENTION_POLICY_ID,
    }
}

fn provider_usage_contract() -> PersistedPayloadContract<'static> {
    PersistedPayloadContract {
        owner: MODULE_ID,
        schema_id: PROVIDER_USAGE_ENTRY_STATE_SCHEMA_ID,
        schema_version: PROVIDER_USAGE_ENTRY_STATE_SCHEMA_VERSION,
        descriptor_hash: provider_usage_entry_state_descriptor_hash(),
        maximum_size_bytes: PROVIDER_USAGE_ENTRY_STATE_MAXIMUM_BYTES,
        retention_policy_id: PROVIDER_USAGE_ENTRY_STATE_RETENTION_POLICY_ID,
    }
}

async fn insert_record(
    admin: &PgPool,
    tenant: &str,
    record_type: &str,
    record_id: &str,
    version: i64,
    contract: PersistedPayloadContract<'_>,
    data_class: DataClass,
    payload_bytes: Vec<u8>,
) {
    let mut transaction = admin.begin().await.unwrap();
    sqlx::query("SET LOCAL session_replication_role = 'replica'")
        .execute(&mut *transaction)
        .await
        .unwrap();
    sqlx::query(
        r#"
        INSERT INTO crm.records (
          tenant_id, record_type, record_id, version, owner_module_id,
          schema_id, schema_version, descriptor_hash, data_class, payload_encoding,
          maximum_payload_size, retention_policy_id, payload_bytes,
          last_business_transaction_id
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, 'json', $10, $11, $12, $13)
        "#,
    )
    .bind(tenant)
    .bind(record_type)
    .bind(record_id)
    .bind(version)
    .bind(contract.owner)
    .bind(contract.schema_id)
    .bind(contract.schema_version)
    .bind(contract.descriptor_hash.as_slice())
    .bind(data_class_name(data_class))
    .bind(i64::try_from(contract.maximum_size_bytes).unwrap())
    .bind(contract.retention_policy_id)
    .bind(payload_bytes)
    .bind(format!("fixture-{record_id}"))
    .execute(&mut *transaction)
    .await
    .unwrap();
    transaction.commit().await.unwrap();
}

async fn insert_request_relationship(
    admin: &PgPool,
    tenant: &str,
    party_id: &str,
    request_id: &str,
) {
    let mut transaction = admin.begin().await.unwrap();
    sqlx::query("SET LOCAL session_replication_role = 'replica'")
        .execute(&mut *transaction)
        .await
        .unwrap();
    sqlx::query(
        r#"
        INSERT INTO crm.relationships (
          tenant_id, relationship_type,
          source_record_type, source_record_id,
          target_record_type, target_record_id,
          version, owner_module_id, schema_id, schema_version, descriptor_hash,
          data_class, payload_encoding, maximum_payload_size, retention_policy_id,
          payload_bytes, last_business_transaction_id
        )
        VALUES ($1, $2, $3, $4, $5, $6, 1, $7, $8, $9, $10,
                'personal', 'json', $11, $12, '{}'::bytea, $13)
        "#,
    )
    .bind(tenant)
    .bind(REQUEST_PARTY_RELATIONSHIP_TYPE)
    .bind(REQUEST_PARTY_SOURCE_RECORD_TYPE)
    .bind(party_id)
    .bind(ENRICHMENT_REQUEST_RECORD_TYPE)
    .bind(request_id)
    .bind(MODULE_ID)
    .bind(REQUEST_PARTY_LINK_SCHEMA_ID)
    .bind(REQUEST_PARTY_LINK_SCHEMA_VERSION)
    .bind(REQUEST_PARTY_LINK_DESCRIPTOR_HASH.as_slice())
    .bind(i64::try_from(REQUEST_PARTY_LINK_MAXIMUM_BYTES).unwrap())
    .bind(LIFECYCLE_STATE_RETENTION_POLICY_ID)
    .bind(format!("fixture-link-{request_id}"))
    .execute(&mut *transaction)
    .await
    .unwrap();
    transaction.commit().await.unwrap();
}

fn data_class_name(value: DataClass) -> &'static str {
    match value {
        DataClass::Personal => "personal",
        DataClass::Confidential => "confidential",
        _ => panic!("unsupported fixture data class"),
    }
}

async fn create_party(
    executor: &Arc<dyn TransactionalCapabilityExecutor>,
    definition: &CapabilityDefinition,
    tenant: &str,
    party_id: &str,
    seed: u8,
) {
    executor
        .execute(
            definition,
            capability_request(
                crm_parties::MODULE_ID,
                CREATE_PARTY_CAPABILITY,
                crm_parties_capability_adapter::CREATE_REQUEST_SCHEMA,
                tenant,
                &format!("party-{party_id}"),
                100_000_000 + i64::from(seed),
                &parties::CreatePartyRequest {
                    party_ref: Some(customer::PartyRef {
                        party_id: party_id.to_owned(),
                    }),
                    kind: parties::PartyKind::Person as i32,
                    display_name: format!("Enrichment Privacy Subject {party_id}"),
                },
            ),
        )
        .await
        .unwrap();
}

async fn merge_party(
    executor: &Arc<dyn TransactionalCapabilityExecutor>,
    definition: &CapabilityDefinition,
    tenant: &str,
    operation_id: &str,
    source_party_id: &str,
    survivor_party_id: &str,
    seed: u8,
) {
    executor
        .execute(
            definition,
            capability_request(
                crm_identity_resolution::MODULE_ID,
                MERGE_CAPABILITY,
                crm_identity_resolution_capability_adapter::MERGE_REQUEST_SCHEMA,
                tenant,
                &format!("merge-{operation_id}"),
                200_000_000 + i64::from(seed),
                &identity::MergePartyRequest {
                    merge_operation_ref: Some(identity::MergeOperationRef {
                        merge_operation_id: operation_id.to_owned(),
                    }),
                    source_party_ref: Some(customer::PartyRef {
                        party_id: source_party_id.to_owned(),
                    }),
                    source_party_version: 1,
                    survivor_party_ref: Some(customer::PartyRef {
                        party_id: survivor_party_id.to_owned(),
                    }),
                    survivor_party_version: 1,
                    decision_ref: format!("approval://{operation_id}"),
                    reason: "duplicate.confirmed".to_owned(),
                    survivorship: vec![identity::SurvivorshipSelection {
                        field_path: "display_name".to_owned(),
                        provenance_party_ref: Some(customer::PartyRef {
                            party_id: source_party_id.to_owned(),
                        }),
                        provenance_party_version: 1,
                        source_value_sha256: [seed; 32].to_vec(),
                        evidence_ref: format!("evidence://{operation_id}"),
                    }],
                },
            ),
        )
        .await
        .unwrap();
}

fn capability_request<M: Message>(
    module_id: &str,
    capability_id: &str,
    input_schema: &str,
    tenant: &str,
    identity: &str,
    started_at: i64,
    command: &M,
) -> CapabilityRequest {
    let bytes = command.encode_to_vec();
    let input = TypedPayload {
        owner: ModuleId::try_new(module_id).unwrap(),
        schema_id: SchemaId::try_new(input_schema).unwrap(),
        schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
        descriptor_hash: message_descriptor_hash(input_schema),
        data_class: DataClass::Personal,
        encoding: PayloadEncoding::Protobuf,
        maximum_size_bytes: support::MAX_PROTOBUF_BYTES,
        retention_policy_id: RetentionPolicyId::try_new("standard").unwrap(),
        bytes,
    };
    CapabilityRequest {
        context: ModuleExecutionContext {
            module_id: ModuleId::try_new(module_id).unwrap(),
            execution: ExecutionContext {
                tenant_id: TenantId::try_new(tenant).unwrap(),
                actor_id: ActorId::try_new("actor-a").unwrap(),
                request_id: RequestId::try_new(format!("request-{identity}")).unwrap(),
                correlation_id: CorrelationId::try_new(format!("correlation-{identity}")).unwrap(),
                causation_id: CausationId::try_new(format!("causation-{identity}")).unwrap(),
                trace_id: TraceId::try_new(format!("trace-{identity}")).unwrap(),
                capability_id: CapabilityId::try_new(capability_id).unwrap(),
                capability_version: CapabilityVersion::try_new("1.0.0").unwrap(),
                idempotency_key: IdempotencyKey::try_new(format!("{identity}-key")).unwrap(),
                business_transaction_id: BusinessTransactionId::try_new(format!("{identity}-tx"))
                    .unwrap(),
                schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
                request_started_at_unix_nanos: started_at,
            },
        },
        input_hash: Sha256::digest(&input.bytes).into(),
        input,
        approval: None,
    }
}

fn scope_request(
    tenant: &str,
    party_id: &str,
    generation: u64,
    page_size: u32,
    cursor: &str,
    identity: &str,
) -> QueryRequest {
    let registry = OwnerScopeRegistry::canonical_v1().unwrap();
    let wire = privacy::CustomerEnrichmentPrivacyScopeContributionRequest {
        contribution: Some(privacy::PrivacyScopeContributionRequestEnvelope {
            lineage: Some(privacy::PrivacyScopeContributionLineage {
                privacy_case_id: format!("privacy-case-{identity}"),
                tenant_id: tenant.to_owned(),
                canonical_party_ref: Some(customer::PartyRef {
                    party_id: party_id.to_owned(),
                }),
                identity_resolution_generation: generation,
                registry_version: CANONICAL_SCOPE_REGISTRY_VERSION.to_owned(),
                registry_digest_sha256: registry.digest().to_vec(),
                purpose_code: "PRIVACY_ERASURE_SCOPE".to_owned(),
                effective_request_at_unix_ms: 1_000,
            }),
            page_size,
            cursor: cursor.to_owned(),
        }),
    };
    let bytes = wire.encode_to_vec();
    QueryRequest {
        owner_module_id: ModuleId::try_new(MODULE_ID).unwrap(),
        context: QueryExecutionContext {
            tenant_id: TenantId::try_new(tenant).unwrap(),
            actor_id: ActorId::try_new(ACTOR).unwrap(),
            request_id: RequestId::try_new(format!("request-{identity}")).unwrap(),
            correlation_id: CorrelationId::try_new(format!("correlation-{identity}")).unwrap(),
            trace_id: TraceId::try_new(format!("trace-{identity}")).unwrap(),
            capability_id: CapabilityId::try_new(CAPABILITY_ID).unwrap(),
            capability_version: CapabilityVersion::try_new(CAPABILITY_VERSION).unwrap(),
            schema_version: SchemaVersion::try_new(CONTRACT_SCHEMA_VERSION).unwrap(),
            request_started_at_unix_nanos: 2_000_000_000,
        },
        input: TypedPayload {
            owner: ModuleId::try_new(MODULE_ID).unwrap(),
            schema_id: SchemaId::try_new(INPUT_SCHEMA_ID).unwrap(),
            schema_version: SchemaVersion::try_new(CONTRACT_SCHEMA_VERSION).unwrap(),
            descriptor_hash: message_descriptor_hash(INPUT_SCHEMA_ID),
            data_class: DataClass::Confidential,
            encoding: PayloadEncoding::Protobuf,
            maximum_size_bytes: INPUT_MAXIMUM_BYTES,
            retention_policy_id: RetentionPolicyId::try_new(INPUT_RETENTION_POLICY_ID).unwrap(),
            bytes: bytes.clone(),
        },
        input_hash: Sha256::digest(&bytes).into(),
    }
}

async fn current_generation(admin: &PgPool, tenant: &str) -> u64 {
    let mut transaction = admin.begin().await.unwrap();
    sqlx::query("SELECT set_config('app.tenant_id', $1, true)")
        .bind(tenant)
        .execute(&mut *transaction)
        .await
        .unwrap();
    let generation: i64 =
        sqlx::query_scalar("SELECT crm.current_identity_resolution_generation($1)")
            .bind(tenant)
            .fetch_one(&mut *transaction)
            .await
            .unwrap();
    transaction.commit().await.unwrap();
    u64::try_from(generation).unwrap()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct WriteSurfaceCounts {
    records: i64,
    relationships: i64,
    business_transactions: i64,
    idempotency_records: i64,
    outbox_events: i64,
    outbox_delivery: i64,
    audit_heads: i64,
    audit_records: i64,
}

async fn write_surface_counts(pool: &PgPool) -> WriteSurfaceCounts {
    WriteSurfaceCounts {
        records: sqlx::query_scalar("SELECT count(*)::bigint FROM crm.records")
            .fetch_one(pool)
            .await
            .unwrap(),
        relationships: sqlx::query_scalar("SELECT count(*)::bigint FROM crm.relationships")
            .fetch_one(pool)
            .await
            .unwrap(),
        business_transactions: sqlx::query_scalar(
            "SELECT count(*)::bigint FROM crm.business_transactions",
        )
        .fetch_one(pool)
        .await
        .unwrap(),
        idempotency_records: sqlx::query_scalar(
            "SELECT count(*)::bigint FROM crm.idempotency_records",
        )
        .fetch_one(pool)
        .await
        .unwrap(),
        outbox_events: sqlx::query_scalar("SELECT count(*)::bigint FROM crm.outbox_events")
            .fetch_one(pool)
            .await
            .unwrap(),
        outbox_delivery: sqlx::query_scalar("SELECT count(*)::bigint FROM crm.outbox_delivery")
            .fetch_one(pool)
            .await
            .unwrap(),
        audit_heads: sqlx::query_scalar("SELECT count(*)::bigint FROM crm.audit_heads")
            .fetch_one(pool)
            .await
            .unwrap(),
        audit_records: sqlx::query_scalar("SELECT count(*)::bigint FROM crm.audit_records")
            .fetch_one(pool)
            .await
            .unwrap(),
    }
}

async fn prove_relationship_and_record_primary_key_scans(admin: &PgPool, tenant: &str) {
    let mut transaction = admin.begin().await.unwrap();
    sqlx::query("SET LOCAL enable_seqscan = off")
        .execute(&mut *transaction)
        .await
        .unwrap();
    let relationship_plan = sqlx::query_scalar::<_, String>(
        r#"
        EXPLAIN (COSTS OFF)
        SELECT source_record_id, target_record_id
          FROM crm.relationships
         WHERE tenant_id = $1
           AND relationship_type = 'customer_enrichment.request.party'
           AND source_record_type = 'parties.party'
           AND target_record_type = 'customer_enrichment.request'
           AND (source_record_id, target_record_id) > ('', '')
         ORDER BY source_record_id ASC, target_record_id ASC
         LIMIT 512
        "#,
    )
    .bind(tenant)
    .fetch_all(&mut *transaction)
    .await
    .unwrap()
    .join("\n");
    assert!(
        relationship_plan.contains("relationships_pkey"),
        "unexpected relationship plan: {relationship_plan}"
    );
    assert!(!relationship_plan.contains("Seq Scan"));

    let record_plan = sqlx::query_scalar::<_, String>(
        r#"
        EXPLAIN (COSTS OFF)
        SELECT record_id, version
          FROM crm.records
         WHERE tenant_id = $1
           AND owner_module_id = 'crm.customer-enrichment'
           AND record_type = 'customer_enrichment.request'
           AND record_id > ''
           AND deleted_at IS NULL
         ORDER BY record_id ASC
         LIMIT 512
        "#,
    )
    .bind(tenant)
    .fetch_all(&mut *transaction)
    .await
    .unwrap()
    .join("\n");
    assert!(record_plan.contains("records_pkey"));
    assert!(!record_plan.contains("Seq Scan"));
    transaction.rollback().await.unwrap();
}

async fn set_record_data_class(admin: &PgPool, tenant: &str, record_id: &str, value: &str) {
    let mut transaction = admin.begin().await.unwrap();
    sqlx::query("SET LOCAL session_replication_role = 'replica'")
        .execute(&mut *transaction)
        .await
        .unwrap();
    sqlx::query("UPDATE crm.records SET data_class = $3 WHERE tenant_id = $1 AND record_id = $2")
        .bind(tenant)
        .bind(record_id)
        .bind(value)
        .execute(&mut *transaction)
        .await
        .unwrap();
    transaction.commit().await.unwrap();
}

async fn delete_record(admin: &PgPool, tenant: &str, record_type: &str, record_id: &str) {
    let mut transaction = admin.begin().await.unwrap();
    sqlx::query("SET LOCAL session_replication_role = 'replica'")
        .execute(&mut *transaction)
        .await
        .unwrap();
    sqlx::query(
        "DELETE FROM crm.records WHERE tenant_id = $1 AND record_type = $2 AND record_id = $3",
    )
    .bind(tenant)
    .bind(record_type)
    .bind(record_id)
    .execute(&mut *transaction)
    .await
    .unwrap();
    transaction.commit().await.unwrap();
}

fn decode(bytes: &[u8]) -> privacy::CustomerEnrichmentPrivacyScopeContributionResponse {
    privacy::CustomerEnrichmentPrivacyScopeContributionResponse::decode(bytes)
        .expect("decode Customer Enrichment privacy scope response")
}

fn expected_two_graphs(left: &GraphIds, right: &GraphIds) -> BTreeMap<String, Vec<String>> {
    let mut map = BTreeMap::new();
    for (record_type, left_id, right_id) in [
        (ENRICHMENT_REQUEST_RECORD_TYPE, &left.request, &right.request),
        (
            PROVIDER_RESPONSE_RECEIPT_RECORD_TYPE,
            &left.receipt,
            &right.receipt,
        ),
        (
            PROVIDER_RESPONSE_CONFLICT_RECORD_TYPE,
            &left.conflict,
            &right.conflict,
        ),
        (SUGGESTION_RECORD_TYPE, &left.suggestion, &right.suggestion),
        (REVIEW_DECISION_RECORD_TYPE, &left.review, &right.review),
        (
            APPLICATION_ATTEMPT_RECORD_TYPE,
            &left.application,
            &right.application,
        ),
        (PROVIDER_USAGE_ENTRY_RECORD_TYPE, &left.usage, &right.usage),
    ] {
        let mut ids = vec![left_id.clone(), right_id.clone()];
        ids.sort();
        map.insert(record_type.to_owned(), ids);
    }
    map
}

fn graph_values(graph: &GraphIds) -> [&str; 7] {
    [
        &graph.request,
        &graph.receipt,
        &graph.conflict,
        &graph.suggestion,
        &graph.review,
        &graph.application,
        &graph.usage,
    ]
}

fn assert_response_omits_private_enrichment_values(bytes: &[u8]) {
    for forbidden in [
        "party-alias",
        "party-unrelated",
        "Private Proposed Alias Name",
        "Private Proposed Canonical Name",
        "Private Proposed Unrelated Name",
        "Private Proposed Tenant B Name",
        "private-provider",
        "private-adapter",
        "private-secret-handle",
        "Private Provider License",
        "private.payload.name",
        "private-replay",
        "private-correlation",
        "private-provider-evidence",
        "private-suggestion-evidence",
        "private-reviewer",
        "private-approved",
        "private-provider-code",
    ] {
        assert!(
            !contains(bytes, forbidden),
            "response leaked forbidden Customer Enrichment value: {forbidden}"
        );
    }
}

fn contains(bytes: &[u8], value: &str) -> bool {
    bytes
        .windows(value.len())
        .any(|candidate| candidate == value.as_bytes())
}
