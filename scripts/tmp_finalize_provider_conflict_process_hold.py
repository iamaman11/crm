from pathlib import Path


def replace_once(path: Path, old: str, new: str, label: str) -> None:
    text = path.read_text()
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected exactly one match, found {count}")
    path.write_text(text.replace(old, new, 1))


process_test = Path(
    "crates/crm-customer-enrichment-provider-process-composition/tests/postgres_conflict_process_hold.rs"
)
replace_once(
    process_test,
    "use crm_core_events::{ProjectionStore, ProjectionStore as _};",
    "use crm_core_events::ProjectionStore;",
    "provider conflict process projection import",
)

persistence = Path(
    "crates/crm-customer-enrichment-provider-process-composition/src/conflict_persistence.rs"
)
replace_once(
    persistence,
    "        assert_eq!(plan.batch.relationships.len(), 0);\n",
    "",
    "stale relationship assertion",
)

postgres = Path(
    "crates/crm-customer-enrichment-provider-process-composition/tests/postgres_conflict_persistence.rs"
)
replace_once(
    postgres,
    "use crm_core_data::PostgresDataStore;\n",
    "use crm_capability_plan_support as support;\nuse crm_core_data::{AuditIntent, IdempotencyEvidence, PostgresDataStore, RecordCreatePlan};\n",
    "conflict persistence seed imports",
)
replace_once(
    postgres,
    """use crm_customer_enrichment::{
    EnrichmentRequestId, PROVIDER_RESPONSE_CONFLICT_RECORD_TYPE, ProviderResponseConflictDraft,
    ProviderResponseReceiptId,
};
""",
    """use crm_customer_enrichment::{
    EnrichmentRequest, EnrichmentRequestDraft, EnrichmentRequestId, MappingDraft,
    MappingNormalization, MappingVersion, PROVIDER_RESPONSE_CONFLICT_RECORD_TYPE,
    ProviderProfileDraft, ProviderProfileVersion, ProviderResponseConflictDraft,
    ProviderResponseReceiptId, RawPayloadPolicy, RequestPolicyEvidence, TargetField,
    TargetSnapshot,
};
use crm_customer_enrichment_capability_adapter::{
    ENRICHMENT_REQUEST_CREATED_EVENT_SCHEMA, ENRICHMENT_REQUEST_CREATED_EVENT_TYPE, MODULE_ID,
    enrichment_request_persisted_payload, enrichment_request_record_ref, enrichment_request_to_wire,
};
""",
    "conflict persistence canonical request imports",
)
replace_once(
    postgres,
    "use crm_module_sdk::{ActorId, CausationId, CorrelationId, RecordId, TenantId, TraceId};\n",
    """use crm_module_sdk::{
    ActorId, BusinessTransactionId, CapabilityId, CapabilityVersion, CausationId, CorrelationId,
    DataClass, DomainEvent, EventType, ExecutionContext, IdempotencyKey, ModuleExecutionContext,
    ModuleId, RecordId, RequestId, SchemaVersion, TenantId, TraceId,
};
use crm_proto_contracts::crm::customer_enrichment::v1 as wire;
""",
    "conflict persistence seed context imports",
)
replace_once(
    postgres,
    """    let admin = PgPool::connect(&admin_database_url)
        .await
        .expect("connect conflict evidence reader");
    let persistence = PostgresProviderResponseConflictStore::new(store);

    let first = persistence
        .record(draft(), lineage())
""",
    """    let admin = PgPool::connect(&admin_database_url)
        .await
        .expect("connect conflict evidence reader");
    let request = canonical_request();
    seed_request(&store, &request)
        .await
        .expect("seed canonical enrichment request for conflict relationship");
    let persistence = PostgresProviderResponseConflictStore::new(store.clone());

    let first = persistence
        .record(draft(request.request_id().clone()), lineage())
""",
    "seed canonical request before conflict persistence",
)
replace_once(
    postgres,
    """    let replay = persistence
        .record(draft(), lineage())
""",
    """    let replay = persistence
        .record(draft(request.request_id().clone()), lineage())
""",
    "replay canonical request conflict draft",
)
replace_once(
    postgres,
    """fn draft() -> ProviderResponseConflictDraft {
    ProviderResponseConflictDraft {
        tenant_id: TenantId::try_new(TENANT_ID).unwrap(),
        request_id: request_id(1),
""",
    """fn draft(request_id: EnrichmentRequestId) -> ProviderResponseConflictDraft {
    ProviderResponseConflictDraft {
        tenant_id: TenantId::try_new(TENANT_ID).unwrap(),
        request_id,
""",
    "conflict draft uses canonical request identity",
)
replace_once(
    postgres,
    """fn request_id(byte: u8) -> EnrichmentRequestId {
    serde_json::from_str(&format!(
        "\\\"enrichment-request-{}\\\"",
        format!("{byte:02x}").repeat(32)
    ))
    .unwrap()
}

""",
    """fn canonical_request() -> EnrichmentRequest {
    let profile = ProviderProfileVersion::publish(ProviderProfileDraft {
        provider_key: "company_registry_conflict_persistence".to_owned(),
        adapter_kind: "registry_http_v1".to_owned(),
        adapter_contract_version: "1.0.0".to_owned(),
        supported_target_fields: vec![TargetField::PartyDisplayName],
        purpose_codes: vec!["customer_profile_enrichment".to_owned()],
        license_id: "Registry conflict persistence licence".to_owned(),
        permitted_use_class: "customer_master_review".to_owned(),
        residency_region: "eu".to_owned(),
        retention_days: 30,
        raw_payload_policy: RawPayloadPolicy::DigestOnly,
        credential_handle_aliases: vec!["registry_conflict_persistence".to_owned()],
        effective_at_unix_ms: 1,
        expires_at_unix_ms: Some(5_000),
    })
    .unwrap();
    let mapping = MappingVersion::publish(MappingDraft {
        mapping_key: "party_display_name_conflict_persistence".to_owned(),
        provider_profile_version_id: profile.version_id().clone(),
        provider_response_field_path: "organization.legal_name".to_owned(),
        target_field: TargetField::PartyDisplayName,
        normalization: MappingNormalization::CanonicalPartyDisplayNameV1,
        maximum_suggestions_per_response: 1,
        confidence_required: true,
    })
    .unwrap();
    EnrichmentRequest::create(EnrichmentRequestDraft {
        tenant_id: TenantId::try_new(TENANT_ID).unwrap(),
        requested_by: ActorId::try_new(ACTOR_ID).unwrap(),
        idempotency_key: IdempotencyKey::try_new("provider-conflict-persistence-request").unwrap(),
        target: TargetSnapshot::try_new(
            "party-provider-conflict-persistence-1",
            7,
            TargetField::PartyDisplayName,
        )
        .unwrap(),
        provider_profile_version_id: profile.version_id().clone(),
        mapping_version_id: mapping.version_id().clone(),
        requested_fields: vec![TargetField::PartyDisplayName],
        policy_evidence: RequestPolicyEvidence::try_new(
            "customer_profile_enrichment",
            "legitimate_interest",
            None,
            "provider-conflict-persistence-policy-v1",
        )
        .unwrap(),
        created_at_unix_ms: 10,
        deadline_at_unix_ms: 1_000,
        expires_at_unix_ms: 2_000,
    })
    .unwrap()
}

async fn seed_request(
    store: &PostgresDataStore,
    request: &EnrichmentRequest,
) -> Result<(), Box<dyn std::error::Error>> {
    let record = enrichment_request_record_ref(request)?;
    let payload = support::protobuf_payload(
        MODULE_ID,
        ENRICHMENT_REQUEST_CREATED_EVENT_SCHEMA,
        DataClass::Personal,
        &wire::EnrichmentRequestCreatedEvent {
            enrichment_request: Some(enrichment_request_to_wire(request)?),
        },
    )?;
    store
        .create_record(&RecordCreatePlan {
            context: seed_context(),
            record: record.clone(),
            record_payload: enrichment_request_persisted_payload(request)?,
            event_id: "provider-conflict-persistence-seed-event".to_owned(),
            event: DomainEvent {
                event_type: EventType::try_new(ENRICHMENT_REQUEST_CREATED_EVENT_TYPE)?,
                aggregate: record,
                expected_aggregate_version: None,
                deduplication_key: "provider-conflict-persistence-seed".to_owned(),
                payload,
            },
            idempotency: IdempotencyEvidence {
                scope: "customer_enrichment.provider_conflict.seed@1.0.0".to_owned(),
                key: "provider-conflict-persistence-seed".to_owned(),
                request_hash: [41; 32],
                expires_at_unix_nanos: 86_400_000_000_000,
            },
            audit: AuditIntent {
                audit_record_id: "provider-conflict-persistence-seed-audit".to_owned(),
                canonicalization_profile: "crm.cjson/v1".to_owned(),
                canonical_envelope: b"{\\\"operation\\\":\\\"seed_provider_conflict_persistence\\\"}".to_vec(),
                occurred_at_unix_nanos: 10_000_000,
            },
        })
        .await?;
    Ok(())
}

fn seed_context() -> ModuleExecutionContext {
    ModuleExecutionContext {
        module_id: ModuleId::try_new(MODULE_ID).unwrap(),
        execution: ExecutionContext {
            tenant_id: TenantId::try_new(TENANT_ID).unwrap(),
            actor_id: ActorId::try_new(ACTOR_ID).unwrap(),
            request_id: RequestId::try_new("provider-conflict-persistence-seed-request").unwrap(),
            correlation_id: CorrelationId::try_new("provider-conflict-persistence-seed-correlation")
                .unwrap(),
            causation_id: CausationId::try_new("provider-conflict-persistence-seed-causation")
                .unwrap(),
            trace_id: TraceId::try_new("provider-conflict-persistence-seed-trace").unwrap(),
            capability_id: CapabilityId::try_new("customer_enrichment.provider_conflict.seed")
                .unwrap(),
            capability_version: CapabilityVersion::try_new("1.0.0").unwrap(),
            idempotency_key: IdempotencyKey::try_new("provider-conflict-persistence-seed")
                .unwrap(),
            business_transaction_id: BusinessTransactionId::try_new(
                "provider-conflict-persistence-seed-tx",
            )
            .unwrap(),
            schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
            request_started_at_unix_nanos: 10_000_000,
        },
    }
}

""",
    "canonical conflict persistence request fixture",
)

print("finalized provider conflict process hold tests")
