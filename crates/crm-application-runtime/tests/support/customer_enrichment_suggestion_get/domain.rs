use super::{actor, tenant, NOW, TENANT};
use crm_capability_ingress::semantic_input_hash;
use crm_capability_plan_support as support;
use crm_core_data::{AuditIntent, IdempotencyEvidence, PostgresDataStore, RecordCreatePlan};
use crm_customer_enrichment::{
    EnrichmentRequest, EnrichmentRequestDraft, MappingDraft, MappingNormalization, MappingVersion,
    ProviderProfileDraft, ProviderProfileVersion, ProviderResponseClass, ProviderResponseReceipt,
    ProviderResponseReceiptDraft, RawPayloadPolicy, RequestPolicyEvidence, Suggestion,
    SuggestionDraft, TargetField, TargetSnapshot,
};
use crm_customer_enrichment_capability_adapter::MODULE_ID;
use crm_customer_enrichment_review_adapter::{
    suggestion_persisted_payload, suggestion_record_ref, suggestion_to_wire,
};
use crm_module_sdk::{
    BusinessTransactionId, CapabilityId, CapabilityVersion, CausationId, CorrelationId, DataClass,
    DomainEvent, EventType, ExecutionContext, IdempotencyKey, ModuleExecutionContext, ModuleId,
    RequestId, SchemaVersion, TraceId,
};
use crm_proto_contracts::crm::customer_enrichment::v1 as wire;
use sqlx::PgPool;
use std::collections::BTreeSet;

const SEED_CAPABILITY: &str = "customer_enrichment.review.seed";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EvidenceCounts {
    records: i64,
    outbox: i64,
    audits: i64,
    idempotency: i64,
    transactions: i64,
}

pub async fn activate_customer_enrichment(store: &PostgresDataStore) {
    store
        .bootstrap_activate_published_modules(
            &BTreeSet::from([tenant(TENANT)]),
            &BTreeSet::from([MODULE_ID.to_owned()]),
        )
        .await
        .expect("activate Customer Enrichment through durable state");
}

pub fn suggestion() -> Suggestion {
    let now_ms = u64::try_from(NOW / 1_000_000).unwrap();
    let profile = ProviderProfileVersion::publish(ProviderProfileDraft {
        provider_key: "production-suggestion-registry".to_owned(),
        adapter_kind: "production-suggestion-http-v1".to_owned(),
        adapter_contract_version: "1.0.0".to_owned(),
        supported_target_fields: vec![TargetField::PartyDisplayName],
        purpose_codes: vec!["customer_profile_enrichment".to_owned()],
        license_id: "Production suggestion registry licence".to_owned(),
        permitted_use_class: "customer_master_review".to_owned(),
        residency_region: "eu".to_owned(),
        retention_days: 30,
        raw_payload_policy: RawPayloadPolicy::GovernedProtectedEvidence,
        credential_handle_aliases: vec!["production_suggestion_primary".to_owned()],
        effective_at_unix_ms: now_ms - 10_000,
        expires_at_unix_ms: Some(now_ms + 1_000_000),
    })
    .unwrap();
    let mapping = MappingVersion::publish(MappingDraft {
        mapping_key: "production_party_display_name".to_owned(),
        provider_profile_version_id: profile.version_id().clone(),
        provider_response_field_path: "organization.legal_name".to_owned(),
        target_field: TargetField::PartyDisplayName,
        normalization: MappingNormalization::CanonicalPartyDisplayNameV1,
        maximum_suggestions_per_response: 1,
        confidence_required: true,
    })
    .unwrap();
    let mut request = EnrichmentRequest::create(EnrichmentRequestDraft {
        tenant_id: tenant(TENANT),
        requested_by: actor(),
        idempotency_key: IdempotencyKey::try_new("suggestion-production-domain-request").unwrap(),
        target: TargetSnapshot::try_new(
            "party-production-suggestion-1",
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
            Some("consent-production-suggestion-1".to_owned()),
            "request-policy-v1",
        )
        .unwrap(),
        created_at_unix_ms: now_ms - 5_000,
        deadline_at_unix_ms: now_ms + 100_000,
        expires_at_unix_ms: now_ms + 200_000,
    })
    .unwrap();
    request.queue(now_ms - 4_000).unwrap();
    request.mark_dispatched(now_ms - 3_000).unwrap();
    let receipt = ProviderResponseReceipt::record(ProviderResponseReceiptDraft {
        request_id: request.request_id().clone(),
        provider_profile_version_id: profile.version_id().clone(),
        mapping_version_id: mapping.version_id().clone(),
        replay_key: "suggestion-production-provider-replay-1".to_owned(),
        provider_correlation_id: Some("suggestion-production-correlation-1".to_owned()),
        response_class: ProviderResponseClass::Success,
        canonical_response_digest: [91; 32],
        provider_observed_at_unix_ms: Some(now_ms - 2_500),
        retrieved_at_unix_ms: now_ms - 2_000,
        metered_units: 1,
        protected_evidence_reference: Some("suggestion-production-evidence-1".to_owned()),
    })
    .unwrap();
    Suggestion::materialize(SuggestionDraft {
        request_id: request.request_id().clone(),
        response_receipt_id: receipt.receipt_id().clone(),
        provider_profile_version_id: profile.version_id().clone(),
        mapping_version_id: mapping.version_id().clone(),
        target: request.target().clone(),
        proposed_value: "Production Company".to_owned(),
        observed_at_unix_ms: Some(now_ms - 2_500),
        retrieved_at_unix_ms: now_ms - 2_000,
        effective_at_unix_ms: now_ms - 2_500,
        fresh_until_unix_ms: now_ms + 100_000,
        expires_at_unix_ms: now_ms + 150_000,
        confidence_basis_points: Some(9_000),
        purpose_code: "customer_profile_enrichment".to_owned(),
        legal_basis_code: "legitimate_interest".to_owned(),
        license_id: "Production suggestion registry licence".to_owned(),
        permitted_use_class: "customer_master_review".to_owned(),
        residency_region: "eu".to_owned(),
        retention_days: 30,
        consent_evidence_reference: Some("consent-production-suggestion-1".to_owned()),
        evidence_references: vec!["suggestion-production-evidence-1".to_owned()],
    })
    .unwrap()
}

pub async fn seed_suggestion(
    store: &PostgresDataStore,
    suggestion: &Suggestion,
) -> Result<(), Box<dyn std::error::Error>> {
    let reference = suggestion_record_ref(suggestion.suggestion_id().as_str())?;
    let event_payload = support::protobuf_payload(
        MODULE_ID,
        "crm.customer_enrichment.v1.SuggestionMaterializedEvent",
        DataClass::Personal,
        &wire::SuggestionMaterializedEvent {
            suggestion: Some(suggestion_to_wire(
                suggestion,
                None,
                u64::try_from(NOW / 1_000_000)?,
            )?),
        },
    )?;
    store
        .create_record(&RecordCreatePlan {
            context: seed_context(),
            record: reference.clone(),
            record_payload: suggestion_persisted_payload(suggestion)?,
            event_id: "suggestion-production-seed-event".to_owned(),
            event: DomainEvent {
                event_type: EventType::try_new("customer_enrichment.suggestion.materialized")?,
                aggregate: reference,
                expected_aggregate_version: None,
                deduplication_key: "suggestion-production-seed-event".to_owned(),
                payload: event_payload.clone(),
            },
            idempotency: IdempotencyEvidence {
                scope: format!("{SEED_CAPABILITY}@1.0.0"),
                key: "suggestion-production-seed-idempotency".to_owned(),
                request_hash: semantic_input_hash(&event_payload),
                expires_at_unix_nanos: NOW + 86_400_000_000_000,
            },
            audit: AuditIntent {
                audit_record_id: "suggestion-production-seed-audit".to_owned(),
                canonicalization_profile: "crm.cjson/v1".to_owned(),
                canonical_envelope: b"{\"seed\":\"production_suggestion\"}".to_vec(),
                occurred_at_unix_nanos: NOW - 1_000_000,
            },
        })
        .await?;
    Ok(())
}

fn seed_context() -> ModuleExecutionContext {
    ModuleExecutionContext {
        module_id: ModuleId::try_new(MODULE_ID).unwrap(),
        execution: ExecutionContext {
            tenant_id: tenant(TENANT),
            actor_id: actor(),
            request_id: RequestId::try_new("suggestion-production-seed-request").unwrap(),
            correlation_id: CorrelationId::try_new("suggestion-production-seed-correlation")
                .unwrap(),
            causation_id: CausationId::try_new("suggestion-production-seed-causation").unwrap(),
            trace_id: TraceId::try_new("suggestion-production-seed-trace").unwrap(),
            capability_id: CapabilityId::try_new(SEED_CAPABILITY).unwrap(),
            capability_version: CapabilityVersion::try_new("1.0.0").unwrap(),
            idempotency_key: IdempotencyKey::try_new("suggestion-production-seed-idempotency")
                .unwrap(),
            business_transaction_id: BusinessTransactionId::try_new(
                "suggestion-production-seed-transaction",
            )
            .unwrap(),
            schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
            request_started_at_unix_nanos: NOW - 1_000_000,
        },
    }
}

pub async fn set_installation_status(admin: &PgPool, status: &str) {
    sqlx::query(
        "UPDATE crm.module_installations SET status = $1, generation = generation + 1, updated_at = clock_timestamp() WHERE tenant_id = $2 AND module_id = $3",
    )
    .bind(status)
    .bind(TENANT)
    .bind(MODULE_ID)
    .execute(admin)
    .await
    .expect("update durable module installation status");
}

pub async fn evidence_counts(pool: &PgPool) -> EvidenceCounts {
    EvidenceCounts {
        records: scalar(pool, "SELECT count(*) FROM crm.records").await,
        outbox: scalar(pool, "SELECT count(*) FROM crm.outbox_events").await,
        audits: scalar(pool, "SELECT count(*) FROM crm.audit_records").await,
        idempotency: scalar(pool, "SELECT count(*) FROM crm.idempotency_records").await,
        transactions: scalar(pool, "SELECT count(*) FROM crm.business_transactions").await,
    }
}

async fn scalar(pool: &PgPool, statement: &'static str) -> i64 {
    sqlx::query_scalar(statement)
        .fetch_one(pool)
        .await
        .expect("read production evidence count")
}
