from pathlib import Path


def replace_once(path: str, old: str, new: str) -> None:
    file = Path(path)
    text = file.read_text()
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{path}: expected one anchor, found {count}: {old[:180]!r}")
    file.write_text(text.replace(old, new, 1))


def replace_between(path: str, start: str, end: str, replacement: str) -> None:
    file = Path(path)
    text = file.read_text()
    if text.count(start) != 1 or text.count(end) != 1:
        raise SystemExit(f"{path}: bounded replacement anchors are not unique")
    start_index = text.index(start)
    end_index = text.index(end, start_index)
    file.write_text(text[:start_index] + replacement + text[end_index:])


support = "crates/crm-customer-enrichment-review-composition/tests/support/mod.rs"
replace_between(
    support,
    "pub fn suggestion() -> Suggestion {",
    "pub async fn seed_suggestion(",
    '''pub fn suggestion() -> Suggestion {
    suggestion_at(
        "review-domain-request",
        "review-provider-replay-1",
        30,
        1_500,
        7,
    )
}

pub fn refreshed_suggestion() -> Suggestion {
    suggestion_at(
        "review-domain-request-refreshed",
        "review-provider-replay-2",
        45,
        2_000,
        8,
    )
}

fn suggestion_at(
    request_key: &str,
    replay_key: &str,
    retrieved_at_unix_ms: u64,
    expires_at_unix_ms: u64,
    party_resource_version: u64,
) -> Suggestion {
    let profile = ProviderProfileVersion::publish(ProviderProfileDraft {
        provider_key: "review-registry".to_owned(),
        adapter_kind: "review-http-v1".to_owned(),
        adapter_contract_version: "1.0.0".to_owned(),
        supported_target_fields: vec![TargetField::PartyDisplayName],
        purpose_codes: vec!["customer_profile_enrichment".to_owned()],
        license_id: "Review registry licence".to_owned(),
        permitted_use_class: "customer_master_review".to_owned(),
        residency_region: "eu".to_owned(),
        retention_days: 30,
        raw_payload_policy: RawPayloadPolicy::GovernedProtectedEvidence,
        credential_handle_aliases: vec!["review_registry_primary".to_owned()],
        effective_at_unix_ms: 1,
        expires_at_unix_ms: Some(5_000),
    })
    .unwrap();
    let mapping = MappingVersion::publish(MappingDraft {
        mapping_key: "review_party_display_name".to_owned(),
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
        requested_by: ActorId::try_new("worker-a").unwrap(),
        idempotency_key: IdempotencyKey::try_new(request_key).unwrap(),
        target: TargetSnapshot::try_new(
            "party-review-1",
            party_resource_version,
            TargetField::PartyDisplayName,
        )
        .unwrap(),
        provider_profile_version_id: profile.version_id().clone(),
        mapping_version_id: mapping.version_id().clone(),
        requested_fields: vec![TargetField::PartyDisplayName],
        policy_evidence: RequestPolicyEvidence::try_new(
            "customer_profile_enrichment",
            "legitimate_interest",
            Some("consent-review-1".to_owned()),
            "request-policy-v1",
        )
        .unwrap(),
        created_at_unix_ms: 1,
        deadline_at_unix_ms: 1_000,
        expires_at_unix_ms: 2_500,
    })
    .unwrap();
    request.queue(10).unwrap();
    request.mark_dispatched(10).unwrap();
    let receipt = ProviderResponseReceipt::record(ProviderResponseReceiptDraft {
        request_id: request.request_id().clone(),
        provider_profile_version_id: profile.version_id().clone(),
        mapping_version_id: mapping.version_id().clone(),
        replay_key: replay_key.to_owned(),
        provider_correlation_id: Some(format!("correlation-{replay_key}")),
        response_class: ProviderResponseClass::Success,
        canonical_response_digest: [u8::try_from(retrieved_at_unix_ms).unwrap(); 32],
        provider_observed_at_unix_ms: Some(retrieved_at_unix_ms - 1),
        retrieved_at_unix_ms,
        metered_units: 1,
        protected_evidence_reference: Some(format!("evidence-{replay_key}")),
    })
    .unwrap();
    Suggestion::materialize(SuggestionDraft {
        request_id: request.request_id().clone(),
        response_receipt_id: receipt.receipt_id().clone(),
        provider_profile_version_id: profile.version_id().clone(),
        mapping_version_id: mapping.version_id().clone(),
        target: request.target().clone(),
        proposed_value: "Reviewed Company".to_owned(),
        observed_at_unix_ms: Some(retrieved_at_unix_ms - 1),
        retrieved_at_unix_ms,
        effective_at_unix_ms: retrieved_at_unix_ms,
        fresh_until_unix_ms: 1_000,
        expires_at_unix_ms,
        confidence_basis_points: Some(9_000),
        purpose_code: "customer_profile_enrichment".to_owned(),
        legal_basis_code: "legitimate_interest".to_owned(),
        license_id: "Review registry licence".to_owned(),
        permitted_use_class: "customer_master_review".to_owned(),
        residency_region: "eu".to_owned(),
        retention_days: 30,
        consent_evidence_reference: Some("consent-review-1".to_owned()),
        evidence_references: vec![format!("evidence-{replay_key}")],
    })
    .unwrap()
}

''',
)
replace_between(
    support,
    "pub async fn seed_suggestion(",
    "pub fn accept_request(",
    '''pub async fn seed_suggestion(
    store: &PostgresDataStore,
    suggestion: &Suggestion,
) -> Result<(), Box<dyn std::error::Error>> {
    seed_suggestion_with_suffix(store, suggestion, "suggestion").await
}

pub async fn seed_suggestion_with_suffix(
    store: &PostgresDataStore,
    suggestion: &Suggestion,
    suffix: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let reference = suggestion_record_ref(suggestion.suggestion_id().as_str())?;
    let event_payload = support::protobuf_payload(
        MODULE_ID,
        SUGGESTION_MATERIALIZED_EVENT_SCHEMA,
        DataClass::Personal,
        &wire::SuggestionMaterializedEvent {
            suggestion: Some(suggestion_to_wire(suggestion, None, 50)?),
        },
    )?;
    let request_hash = semantic_input_hash(&event_payload);
    store
        .create_record(&RecordCreatePlan {
            context: context(
                &format!("review-seed-request-{suffix}"),
                SEED_CAPABILITY,
                &format!("review-seed-idempotency-{suffix}"),
                &format!("review-seed-tx-{suffix}"),
                50_000_000,
            ),
            record: reference.clone(),
            record_payload: suggestion_persisted_payload(suggestion)?,
            event_id: format!("review-seed-event-{suffix}"),
            event: DomainEvent {
                event_type: EventType::try_new(SUGGESTION_MATERIALIZED_EVENT_TYPE)?,
                aggregate: reference,
                expected_aggregate_version: None,
                deduplication_key: format!("review-seed-event-{suffix}"),
                payload: event_payload,
            },
            idempotency: IdempotencyEvidence {
                scope: format!("{SEED_CAPABILITY}@1.0.0"),
                key: format!("review-seed-idempotency-{suffix}"),
                request_hash,
                expires_at_unix_nanos: 86_400_050_000_000,
            },
            audit: AuditIntent {
                audit_record_id: format!("review-seed-audit-{suffix}"),
                canonicalization_profile: "crm.cjson/v1".to_owned(),
                canonical_envelope: format!("{{\"seed\":\"{suffix}\"}}").into_bytes(),
                occurred_at_unix_nanos: 50_000_000,
            },
        })
        .await?;
    Ok(())
}

''',
)

process = "crates/crm-customer-enrichment-review-composition/tests/postgres_review_process.rs"
replace_once(
    process,
    "use support::{accept_request, seed_suggestion, suggestion};\n",
    "use support::{\n    accept_request, refreshed_suggestion, seed_suggestion, seed_suggestion_with_suffix, suggestion,\n};\n",
)
replace_once(
    process,
    '''struct ProcessVisibility {
    hide_party: bool,
}
''',
    '''struct ProcessVisibility {
    hide_party: bool,
    hidden_suggestion_id: Option<String>,
}
''',
)
replace_once(
    process,
    '''            if self.hide_party && resource.record_type.as_str() == "parties.party" {
                return Ok(QueryVisibilityDecision::denied(
                    "visibility-party-hidden",
                    "visibility-v1",
                ));
            }
''',
    '''            if self.hide_party && resource.record_type.as_str() == "parties.party" {
                return Ok(QueryVisibilityDecision::denied(
                    "visibility-party-hidden",
                    "visibility-v1",
                ));
            }
            if resource.record_type.as_str() == "customer_enrichment.suggestion"
                && self.hidden_suggestion_id.as_deref() == Some(resource.record_id.as_str())
            {
                return Ok(QueryVisibilityDecision::denied(
                    "visibility-suggestion-hidden",
                    "visibility-v1",
                ));
            }
''',
)
replace_once(
    process,
    "        Arc::new(ProcessVisibility { hide_party: false }),\n",
    '''        Arc::new(ProcessVisibility {
            hide_party: false,
            hidden_suggestion_id: None,
        }),
''',
)
replace_once(
    process,
    '''    assert_eq!(list_output.suggestions.len(), 1);
    assert!(list_output.next_cursor.is_empty());

    let hidden_queries = CustomerEnrichmentSuggestionQueryAdapter::new(
''',
    '''    assert_eq!(list_output.suggestions.len(), 1);
    assert!(list_output.next_cursor.is_empty());

    let refreshed = refreshed_suggestion();
    seed_suggestion_with_suffix(&query_store, &refreshed, "refreshed")
        .await
        .expect("seed refreshed immutable suggestion");

    let superseded_result = visible_queries
        .execute(&get_definition, get_request.clone())
        .await
        .expect("derive visible supersession");
    let superseded_output =
        wire::GetSuggestionResponse::decode(superseded_result.output.bytes.as_slice()).unwrap();
    let superseded = superseded_output.suggestion.unwrap();
    assert_eq!(
        superseded.lifecycle_status,
        wire::SuggestionLifecycleStatus::Superseded as i32
    );
    assert_eq!(
        superseded
            .superseded_by_suggestion_ref
            .unwrap()
            .suggestion_id,
        refreshed.suggestion_id().as_str()
    );

    let superseded_list_request = query_request(
        &list_definition.capability_id,
        LIST_SUGGESTIONS_BY_PARTY_REQUEST_SCHEMA,
        &wire::ListSuggestionsByPartyRequest {
            party_ref: Some(PartyRef {
                party_id: "party-review-1".to_owned(),
            }),
            provider_profile_version_ref: accepted_suggestion.provider_profile_version_ref.clone(),
            status: Some(wire::SuggestionLifecycleStatus::Superseded as i32),
            page_size: 10,
            cursor: String::new(),
        },
        "review-list-superseded",
    );
    let superseded_list_result = visible_queries
        .execute(&list_definition, superseded_list_request)
        .await
        .expect("list visible superseded suggestion");
    let superseded_list = wire::ListSuggestionsByPartyResponse::decode(
        superseded_list_result.output.bytes.as_slice(),
    )
    .unwrap();
    assert_eq!(superseded_list.suggestions.len(), 1);
    assert_eq!(
        superseded_list.suggestions[0]
            .suggestion_ref
            .as_ref()
            .unwrap()
            .suggestion_id,
        suggestion.suggestion_id().as_str()
    );

    let hidden_successor_queries = CustomerEnrichmentSuggestionQueryAdapter::new(
        query_store.clone(),
        CursorCodec::new([93; 32]).expect("construct hidden-successor cursor codec"),
        Arc::new(ProcessVisibility {
            hide_party: false,
            hidden_suggestion_id: Some(refreshed.suggestion_id().as_str().to_owned()),
        }),
    );
    let hidden_successor_result = hidden_successor_queries
        .execute(&get_definition, get_request.clone())
        .await
        .expect("hidden successor must not affect visible lifecycle");
    let hidden_successor_output = wire::GetSuggestionResponse::decode(
        hidden_successor_result.output.bytes.as_slice(),
    )
    .unwrap();
    let hidden_successor_suggestion = hidden_successor_output.suggestion.unwrap();
    assert_eq!(
        hidden_successor_suggestion.lifecycle_status,
        wire::SuggestionLifecycleStatus::Accepted as i32
    );
    assert!(
        hidden_successor_suggestion
            .superseded_by_suggestion_ref
            .is_none()
    );

    let expired_get_request = query_request_at(
        &get_definition.capability_id,
        GET_SUGGESTION_REQUEST_SCHEMA,
        &wire::GetSuggestionRequest {
            suggestion_ref: accepted_suggestion.suggestion_ref.clone(),
        },
        "review-get-expired",
        1_600_000_000,
    );
    let expired_result = hidden_successor_queries
        .execute(&get_definition, expired_get_request)
        .await
        .expect("derive expiry at query time");
    let expired_output =
        wire::GetSuggestionResponse::decode(expired_result.output.bytes.as_slice()).unwrap();
    assert_eq!(
        expired_output.suggestion.unwrap().lifecycle_status,
        wire::SuggestionLifecycleStatus::Expired as i32
    );

    let hidden_queries = CustomerEnrichmentSuggestionQueryAdapter::new(
''',
)
replace_once(
    process,
    "        Arc::new(ProcessVisibility { hide_party: true }),\n",
    '''        Arc::new(ProcessVisibility {
            hide_party: true,
            hidden_suggestion_id: None,
        }),
''',
)
for old, new in [("        2\n", "        3\n"), ("        2\n", "        3\n"), ("        2\n", "        3\n"), ("        2\n", "        3\n")]:
    text = Path(process).read_text()
    marker = old
    index = text.find(marker, text.find("assert_eq!("))
    if index == -1:
        raise SystemExit("review process count anchor missing")
    Path(process).write_text(text[:index] + new + text[index + len(old):])
replace_once(
    process,
    '''fn query_request<M: Message>(
    capability_id: &CapabilityId,
    schema: &'static str,
    message: &M,
    request_id: &str,
) -> QueryRequest {
    let input =
        plan_support::protobuf_payload(MODULE_ID, schema, DataClass::Personal, message).unwrap();
    QueryRequest {
''',
    '''fn query_request<M: Message>(
    capability_id: &CapabilityId,
    schema: &'static str,
    message: &M,
    request_id: &str,
) -> QueryRequest {
    query_request_at(capability_id, schema, message, request_id, 50_000_000)
}

fn query_request_at<M: Message>(
    capability_id: &CapabilityId,
    schema: &'static str,
    message: &M,
    request_id: &str,
    request_started_at_unix_nanos: i64,
) -> QueryRequest {
    let input =
        plan_support::protobuf_payload(MODULE_ID, schema, DataClass::Personal, message).unwrap();
    QueryRequest {
''',
)
replace_once(
    process,
    "            request_started_at_unix_nanos: 50_000_000,\n",
    "            request_started_at_unix_nanos,\n",
)
