use crm_capability_ingress::semantic_input_hash;
use crm_capability_plan_support as capability_support;
use crm_capability_runtime::{
    CapabilityDefinition, CapabilityRequest, TransactionalCapabilityExecutor,
};
use crm_consents::{
    CommunicationChannel, ConsentAuthorization, ConsentAuthorizationId, ConsentEffect,
    CreateConsentAuthorization, EvidenceReference, JurisdictionCode, LegalBasisCode, PartyReference,
    PurposeCode, SourceCode, WithdrawConsentAuthorization,
};
use crm_consents_capability_adapter::{
    RECORD_TYPE as CONSENT_RECORD_TYPE, persisted_payload as consent_persisted_payload,
};
use crm_core_data::{
    AuditIntent, IdempotencyEvidence, PostgresDataStore, PostgresTransactionalAggregateExecutor,
    RecordCreatePlan,
};
use crm_customer_enrichment::{
    MappingDraft, MappingNormalization, MappingVersion, ProviderProfileDraft,
    ProviderProfileVersion, RawPayloadPolicy, TargetField,
};
use crm_customer_enrichment_capability_adapter::{
    MODULE_ID as CUSTOMER_ENRICHMENT_MODULE_ID, mapping_persisted_payload, mapping_record_ref,
    provider_profile_persisted_payload, provider_profile_record_ref,
};
use crm_module_sdk::{
    BusinessTransactionId, CapabilityId, CapabilityVersion, CausationId, CorrelationId, DataClass,
    DomainEvent, EventType, ExecutionContext, IdempotencyKey, ModuleExecutionContext, ModuleId,
    RecordId, RecordRef, RecordType, RequestId, SchemaVersion, SdkError, TraceId,
};
use crm_parties_capability_adapter::{
    CREATE_CAPABILITY as PARTY_CREATE_CAPABILITY,
    CREATE_REQUEST_SCHEMA as PARTY_CREATE_REQUEST_SCHEMA, MODULE_ID as PARTIES_MODULE_ID,
    PartyCapabilityPlanner, capability_definition as party_capability_definition,
};
use crm_proto_contracts::crm::{customer::v1 as customer_wire, parties::v1 as party_wire};
use std::sync::Arc;

use super::customer_enrichment_suggestion_get::{NOW, TENANT, actor, tenant};

pub const PARTY_ID: &str = "party-consent-policy-1";
pub const WRONG_PARTY_ID: &str = "party-consent-policy-wrong";
pub const PURPOSE: &str = "customer_profile_enrichment";
pub const LEGAL_BASIS: &str = "consent";
pub const SEED_CAPABILITY: &str = "customer_enrichment.review.seed";

#[derive(Debug, Clone)]
pub struct DefinitionFixture {
    pub profile: ProviderProfileVersion,
    pub mapping: MappingVersion,
}

#[derive(Debug, Clone, Copy)]
pub enum ConsentFixtureKind {
    Valid,
    WrongParty,
    WrongPurpose,
    WrongLegalBasis,
    Deny,
    Withdrawn,
    NotYetEffective,
    Expired,
}

impl ConsentFixtureKind {
    pub const fn suffix(self) -> &'static str {
        match self {
            Self::Valid => "valid",
            Self::WrongParty => "wrong-party",
            Self::WrongPurpose => "wrong-purpose",
            Self::WrongLegalBasis => "wrong-legal-basis",
            Self::Deny => "deny",
            Self::Withdrawn => "withdrawn",
            Self::NotYetEffective => "not-yet-effective",
            Self::Expired => "expired",
        }
    }

    pub fn authorization_id(self) -> String {
        format!("consent-policy-{}", self.suffix())
    }
}

pub async fn seed_party(store: &PostgresDataStore) -> Result<(), SdkError> {
    let definition = party_capability_definition(PARTY_CREATE_CAPABILITY)?;
    let executor = PostgresTransactionalAggregateExecutor::new(
        store.clone(),
        Arc::new(PartyCapabilityPlanner),
    );
    let command = party_wire::CreatePartyRequest {
        party_ref: Some(customer_wire::PartyRef {
            party_id: PARTY_ID.to_owned(),
        }),
        kind: party_wire::PartyKind::Organization as i32,
        display_name: "Consent Policy Company".to_owned(),
    };
    executor
        .execute(
            &definition,
            capability_request(
                &definition,
                PARTIES_MODULE_ID,
                PARTY_CREATE_REQUEST_SCHEMA,
                DataClass::Personal,
                &command,
                "consent-policy-party",
            )?,
        )
        .await?;
    Ok(())
}

pub async fn seed_definitions(store: &PostgresDataStore) -> Result<DefinitionFixture, SdkError> {
    let now_ms = u64::try_from(NOW / 1_000_000).map_err(configuration_error)?;
    let profile = ProviderProfileVersion::publish(ProviderProfileDraft {
        provider_key: "consent-policy-registry".to_owned(),
        adapter_kind: "registry_http_v1".to_owned(),
        adapter_contract_version: "1.0.0".to_owned(),
        supported_target_fields: vec![TargetField::PartyDisplayName],
        purpose_codes: vec![PURPOSE.to_owned()],
        license_id: "Consent policy fixture licence".to_owned(),
        permitted_use_class: "customer_master_review".to_owned(),
        residency_region: "eu".to_owned(),
        retention_days: 30,
        raw_payload_policy: RawPayloadPolicy::DigestOnly,
        credential_handle_aliases: vec!["consent_policy_primary".to_owned()],
        effective_at_unix_ms: now_ms - 10_000,
        expires_at_unix_ms: Some(now_ms + 1_000_000),
    })?;
    let mapping = MappingVersion::publish(MappingDraft {
        mapping_key: "consent_policy_party_display_name".to_owned(),
        provider_profile_version_id: profile.version_id().clone(),
        provider_response_field_path: "organization.legal_name".to_owned(),
        target_field: TargetField::PartyDisplayName,
        normalization: MappingNormalization::CanonicalPartyDisplayNameV1,
        maximum_suggestions_per_response: 1,
        confidence_required: true,
    })?;

    seed_record(
        store,
        provider_profile_record_ref(&profile)?,
        provider_profile_persisted_payload(&profile)?,
        "provider-profile",
    )
    .await?;
    seed_record(
        store,
        mapping_record_ref(&mapping)?,
        mapping_persisted_payload(&mapping)?,
        "mapping",
    )
    .await?;

    Ok(DefinitionFixture { profile, mapping })
}

pub async fn seed_consent(
    store: &PostgresDataStore,
    kind: ConsentFixtureKind,
) -> Result<String, SdkError> {
    let authorization_id = kind.authorization_id();
    let occurred_at = match kind {
        ConsentFixtureKind::Expired => NOW - 3_000_000_000,
        _ => NOW - 2_000_000_000,
    };
    let effective_from = match kind {
        ConsentFixtureKind::NotYetEffective => NOW + 1_000_000_000,
        ConsentFixtureKind::Expired => NOW - 2_000_000_000,
        _ => NOW - 1_000_000_000,
    };
    let expires_at = match kind {
        ConsentFixtureKind::Expired => Some(NOW - 500_000_000),
        _ => Some(NOW + 10_000_000_000),
    };
    let mut authorization = ConsentAuthorization::create(CreateConsentAuthorization {
        authorization_id: ConsentAuthorizationId::try_new(authorization_id.clone())?,
        party_ref: PartyReference::try_new(match kind {
            ConsentFixtureKind::WrongParty => WRONG_PARTY_ID,
            _ => PARTY_ID,
        })?,
        contact_point_ref: None,
        purpose: PurposeCode::try_new(match kind {
            ConsentFixtureKind::WrongPurpose => "unrelated_purpose",
            _ => PURPOSE,
        })?,
        channel: CommunicationChannel::Email,
        effect: match kind {
            ConsentFixtureKind::Deny => ConsentEffect::Deny,
            _ => ConsentEffect::Grant,
        },
        legal_basis: LegalBasisCode::try_new(match kind {
            ConsentFixtureKind::WrongLegalBasis => "legitimate_interest",
            _ => LEGAL_BASIS,
        })?,
        jurisdiction: JurisdictionCode::try_new("eu")?,
        source: SourceCode::try_new("consent_policy_fixture")?,
        evidence_ref: EvidenceReference::try_new(format!("evidence-{authorization_id}"))?,
        effective_from_unix_nanos: effective_from,
        expires_at_unix_nanos: expires_at,
        occurred_at_unix_nanos: occurred_at,
    })?;
    if matches!(kind, ConsentFixtureKind::Withdrawn) {
        authorization.withdraw(WithdrawConsentAuthorization {
            expected_version: 1,
            occurred_at_unix_nanos: NOW - 500_000_000,
        })?;
    }

    seed_record(
        store,
        RecordRef {
            record_type: RecordType::try_new(CONSENT_RECORD_TYPE).map_err(configuration_error)?,
            record_id: RecordId::try_new(authorization_id.clone()).map_err(configuration_error)?,
        },
        consent_persisted_payload(&authorization)?,
        &format!("consent-{}", kind.suffix()),
    )
    .await?;
    Ok(authorization_id)
}

async fn seed_record(
    store: &PostgresDataStore,
    reference: RecordRef,
    payload: crm_module_sdk::TypedPayload,
    suffix: &str,
) -> Result<(), SdkError> {
    let identity = format!("consent-policy-seed-{suffix}");
    let event_payload = payload.clone();
    store
        .create_record(&RecordCreatePlan {
            context: seed_context(&identity)?,
            record: reference.clone(),
            record_payload: payload,
            event_id: format!("{identity}-event"),
            event: DomainEvent {
                event_type: EventType::try_new("customer_enrichment.consent_policy.fixture_seeded")
                    .map_err(configuration_error)?,
                aggregate: reference,
                expected_aggregate_version: None,
                deduplication_key: format!("{identity}-event"),
                payload: event_payload.clone(),
            },
            idempotency: IdempotencyEvidence {
                scope: format!("{SEED_CAPABILITY}@1.0.0"),
                key: identity.clone(),
                request_hash: semantic_input_hash(&event_payload),
                expires_at_unix_nanos: NOW + 86_400_000_000_000,
            },
            audit: AuditIntent {
                audit_record_id: format!("{identity}-audit"),
                canonicalization_profile: "crm.cjson/v1".to_owned(),
                canonical_envelope: format!("{{\"fixture\":\"{suffix}\"}}").into_bytes(),
                occurred_at_unix_nanos: NOW - 100_000_000,
            },
        })
        .await
        .map_err(configuration_error)?;
    Ok(())
}

fn seed_context(identity: &str) -> Result<ModuleExecutionContext, SdkError> {
    Ok(ModuleExecutionContext {
        module_id: ModuleId::try_new(CUSTOMER_ENRICHMENT_MODULE_ID).map_err(configuration_error)?,
        execution: ExecutionContext {
            tenant_id: tenant(TENANT),
            actor_id: actor(),
            request_id: RequestId::try_new(format!("{identity}-request"))
                .map_err(configuration_error)?,
            correlation_id: CorrelationId::try_new(format!("{identity}-correlation"))
                .map_err(configuration_error)?,
            causation_id: CausationId::try_new(format!("{identity}-causation"))
                .map_err(configuration_error)?,
            trace_id: TraceId::try_new(format!("{identity}-trace"))
                .map_err(configuration_error)?,
            capability_id: CapabilityId::try_new(SEED_CAPABILITY).map_err(configuration_error)?,
            capability_version: CapabilityVersion::try_new("1.0.0")
                .map_err(configuration_error)?,
            idempotency_key: IdempotencyKey::try_new(identity).map_err(configuration_error)?,
            business_transaction_id: BusinessTransactionId::try_new(format!(
                "{identity}-transaction"
            ))
            .map_err(configuration_error)?,
            schema_version: SchemaVersion::try_new("1.0.0").map_err(configuration_error)?,
            request_started_at_unix_nanos: NOW - 100_000_000,
        },
    })
}

fn capability_request<M: prost::Message>(
    definition: &CapabilityDefinition,
    module_id: &str,
    schema_id: &str,
    data_class: DataClass,
    message: &M,
    identity: &str,
) -> Result<CapabilityRequest, SdkError> {
    let input = capability_support::protobuf_payload(module_id, schema_id, data_class, message)?;
    Ok(CapabilityRequest {
        context: ModuleExecutionContext {
            module_id: ModuleId::try_new(module_id).map_err(configuration_error)?,
            execution: ExecutionContext {
                tenant_id: tenant(TENANT),
                actor_id: actor(),
                request_id: RequestId::try_new(format!("{identity}-request"))
                    .map_err(configuration_error)?,
                correlation_id: CorrelationId::try_new(format!("{identity}-correlation"))
                    .map_err(configuration_error)?,
                causation_id: CausationId::try_new(format!("{identity}-causation"))
                    .map_err(configuration_error)?,
                trace_id: TraceId::try_new(format!("{identity}-trace"))
                    .map_err(configuration_error)?,
                capability_id: definition.capability_id.clone(),
                capability_version: definition.capability_version.clone(),
                idempotency_key: IdempotencyKey::try_new(identity).map_err(configuration_error)?,
                business_transaction_id: BusinessTransactionId::try_new(format!(
                    "{identity}-transaction"
                ))
                .map_err(configuration_error)?,
                schema_version: SchemaVersion::try_new("1.0.0").map_err(configuration_error)?,
                request_started_at_unix_nanos: NOW - 100_000_000,
            },
        },
        input_hash: semantic_input_hash(&input),
        input,
        approval: None,
    })
}

fn configuration_error(error: impl std::fmt::Display) -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_CONSENT_FIXTURE_INVALID",
        crm_module_sdk::ErrorCategory::Internal,
        false,
        "The Customer Enrichment Consent fixture is invalid.",
    )
    .with_internal_reference(error.to_string())
}
