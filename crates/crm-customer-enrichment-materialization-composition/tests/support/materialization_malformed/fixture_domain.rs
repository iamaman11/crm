#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct EvidenceCounts {
    records: i64,
    events: i64,
    audits: i64,
    idempotency: i64,
    transactions: i64,
}

struct Fixture {
    profile: ProviderProfileVersion,
    mapping: MappingVersion,
    request: EnrichmentRequest,
    receipt: ProviderResponseReceipt,
}

fn fixture() -> Fixture {
    let profile = ProviderProfileVersion::publish(ProviderProfileDraft {
        provider_key: "company_registry_materialization_malformed".to_owned(),
        adapter_kind: "registry_http_v1".to_owned(),
        adapter_contract_version: "1.0.0".to_owned(),
        supported_target_fields: vec![TargetField::PartyDisplayName],
        purpose_codes: vec!["customer_profile_enrichment".to_owned()],
        license_id: "Registry malformed-evidence licence".to_owned(),
        permitted_use_class: "customer_master_review".to_owned(),
        residency_region: "eu".to_owned(),
        retention_days: 30,
        raw_payload_policy: RawPayloadPolicy::GovernedProtectedEvidence,
        credential_handle_aliases: vec!["registry_materialization_malformed".to_owned()],
        effective_at_unix_ms: 1,
        expires_at_unix_ms: Some(5_000),
    })
    .unwrap();
    let mapping = MappingVersion::publish(MappingDraft {
        mapping_key: "party_display_name_materialization_malformed".to_owned(),
        provider_profile_version_id: profile.version_id().clone(),
        provider_response_field_path: "organization.legal_name".to_owned(),
        target_field: TargetField::PartyDisplayName,
        normalization: MappingNormalization::CanonicalPartyDisplayNameV1,
        maximum_suggestions_per_response: 1,
        confidence_required: true,
    })
    .unwrap();
    let mut request = EnrichmentRequest::create(EnrichmentRequestDraft {
        tenant_id: TenantId::try_new(TENANT_ID).unwrap(),
        requested_by: ActorId::try_new(ACTOR_ID).unwrap(),
        idempotency_key: IdempotencyKey::try_new("materialization-malformed-domain-request")
            .unwrap(),
        target: TargetSnapshot::try_new(
            "party-materialization-malformed-1",
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
            Some("consent-materialization-malformed-1".to_owned()),
            "materialization-malformed-policy-v1",
        )
        .unwrap(),
        created_at_unix_ms: 1,
        deadline_at_unix_ms: 1_000,
        expires_at_unix_ms: 2_000,
    })
    .unwrap();
    request.queue(10).unwrap();
    request.mark_dispatched(10).unwrap();
    let receipt = ProviderResponseReceipt::record(ProviderResponseReceiptDraft {
        request_id: request.request_id().clone(),
        provider_profile_version_id: profile.version_id().clone(),
        mapping_version_id: mapping.version_id().clone(),
        replay_key: "materialization-malformed-provider-replay-1".to_owned(),
        provider_correlation_id: Some(
            "materialization-malformed-provider-correlation-1".to_owned(),
        ),
        response_class: ProviderResponseClass::Success,
        canonical_response_digest: [84; 32],
        provider_observed_at_unix_ms: Some(20),
        retrieved_at_unix_ms: 30,
        metered_units: 1,
        protected_evidence_reference: Some(FILE_ID.to_owned()),
    })
    .unwrap();
    request
        .record_response(receipt.receipt_id().clone(), 30)
        .unwrap();
    Fixture {
        profile,
        mapping,
        request,
        receipt,
    }
}

async fn seed_dependencies(
    store: &PostgresDataStore,
    fixture: &Fixture,
) -> Result<(), Box<dyn std::error::Error>> {
    seed_record(
        store,
        SeedRecord {
            suffix: "malformed-profile",
            at_unix_ms: 1,
            reference: provider_profile_record_ref(&fixture.profile)?,
            record_payload: provider_profile_persisted_payload(&fixture.profile)?,
            event_type: PROVIDER_PROFILE_PUBLISHED_EVENT_TYPE,
            event_payload: support::protobuf_payload(
                MODULE_ID,
                PROVIDER_PROFILE_PUBLISHED_EVENT_SCHEMA,
                DataClass::Confidential,
                &wire::ProviderProfileVersionPublishedEvent {
                    provider_profile_version: Some(provider_profile_to_wire(&fixture.profile)),
                },
            )?,
        },
    )
    .await?;
    seed_record(
        store,
        SeedRecord {
            suffix: "malformed-mapping",
            at_unix_ms: 2,
            reference: mapping_record_ref(&fixture.mapping)?,
            record_payload: mapping_persisted_payload(&fixture.mapping)?,
            event_type: MAPPING_PUBLISHED_EVENT_TYPE,
            event_payload: support::protobuf_payload(
                MODULE_ID,
                MAPPING_PUBLISHED_EVENT_SCHEMA,
                DataClass::Confidential,
                &wire::MappingVersionPublishedEvent {
                    mapping_version: Some(mapping_to_wire(&fixture.mapping)),
                },
            )?,
        },
    )
    .await?;
    seed_record(
        store,
        SeedRecord {
            suffix: "malformed-request",
            at_unix_ms: 3,
            reference: enrichment_request_record_ref(&fixture.request)?,
            record_payload: enrichment_request_persisted_payload(&fixture.request)?,
            event_type: ENRICHMENT_REQUEST_CREATED_EVENT_TYPE,
            event_payload: support::protobuf_payload(
                MODULE_ID,
                ENRICHMENT_REQUEST_CREATED_EVENT_SCHEMA,
                DataClass::Personal,
                &wire::EnrichmentRequestCreatedEvent {
                    enrichment_request: Some(enrichment_request_to_wire(&fixture.request)?),
                },
            )?,
        },
    )
    .await?;
    seed_record(
        store,
        SeedRecord {
            suffix: "malformed-receipt",
            at_unix_ms: 40,
            reference: receipt_record_ref(&fixture.receipt)?,
            record_payload: receipt_persisted_payload(&fixture.receipt)?,
            event_type: PROVIDER_RESPONSE_RECORDED_EVENT_TYPE,
            event_payload: support::protobuf_payload(
                MODULE_ID,
                PROVIDER_RESPONSE_RECORDED_EVENT_SCHEMA,
                DataClass::Personal,
                &wire::ProviderResponseRecordedEvent {
                    provider_response_receipt: Some(receipt_to_wire(fixture)),
                },
            )?,
        },
    )
    .await?;
    Ok(())
}

async fn apply_recorded_provider_outcome(
    store: &PostgresDataStore,
    fixture: &Fixture,
) -> Result<(), SdkError> {
    let tenant_id = TenantId::try_new(TENANT_ID).unwrap();
    let page = ProjectionStore::list_event_history(
        store,
        EventHistoryRequest {
            tenant_id: tenant_id.clone(),
            consumer_module_id: ModuleId::try_new(MODULE_ID).unwrap(),
            event_types: vec![EventType::try_new(ENRICHMENT_REQUEST_CREATED_EVENT_TYPE).unwrap()],
            after: None,
            page_size: 100,
        },
    )
    .await?;
    let delivery = page
        .deliveries
        .into_iter()
        .find(|delivery| {
            delivery.aggregate.record_id.as_str() == fixture.request.request_id().as_str()
        })
        .expect("malformed request-created delivery exists");
    let outcome = ProviderProcessCanonicalOutcome::response_recorded(
        fixture.request.request_id().as_str().to_owned(),
        fixture.request.retry_generation(),
        fixture.receipt.receipt_id().as_str().to_owned(),
        delivery.event_id.as_str().to_owned(),
    )?;
    ProjectionStore::apply_projection_event(
        store,
        ProjectionEventApplication {
            projection_id: PROVIDER_PROCESS_PROJECTION_ID.to_owned(),
            writes: vec![ProjectionDocumentWrite {
                resource_type: PROVIDER_PROCESS_OUTCOME_RESOURCE_TYPE.to_owned(),
                resource_id: fixture.request.request_id().as_str().to_owned(),
                source_version: delivery.aggregate_version,
                document: outcome.to_projection_document()?,
            }],
            delivery,
        },
    )
    .await?;
    Ok(())
}

fn materialization_process(
    store: PostgresDataStore,
    artifacts: Arc<PostgresImmutableFileArtifactStore>,
) -> CustomerEnrichmentMaterializationProcessWorker {
    CustomerEnrichmentMaterializationProcessWorker::new(
        store.clone(),
        Arc::new(GovernedFileProviderSuggestionCandidateEvidenceSource::new(
            artifacts,
        )),
        Arc::new(PostgresCustomerEnrichmentSuggestionMaterializationWorker::new(store)),
        ActorId::try_new(ACTOR_ID).unwrap(),
    )
    .expect("compose malformed materialization process")
}
