use crm_application_runtime::{application_mutation_definitions, application_query_definitions};
use crm_capability_runtime::CapabilityDefinition;
use crm_module_sdk::{DataClass, PayloadEncoding, RetentionPolicyId, TypedPayload};
use crm_proto_contracts::crm::{
    customer::v1 as customer, customer_enrichment::v1 as enrichment, parties::v1 as parties,
};
use prost::Message;
use std::time::{SystemTime, UNIX_EPOCH};

use super::{MODULE_ID, SECRET_MARKER};

pub const PUBLISH_PROFILE: &str = "customer_enrichment.provider_profile.publish";
pub const PUBLISH_MAPPING: &str = "customer_enrichment.mapping.publish";
pub const CREATE_REQUEST: &str = "customer_enrichment.request.create";
pub const GET_PROFILE: &str = "customer_enrichment.provider_profile.get";
pub const PARTY_CREATE: &str = "parties.party.create";
pub const PARTY_ID: &str = "party-crm-api-enrichment-1";
pub const PURPOSE: &str = "customer_profile_enrichment";

pub fn mutation_definition(capability_id: &str) -> CapabilityDefinition {
    application_mutation_definitions()
        .expect("valid production mutation definitions")
        .into_iter()
        .find(|definition| definition.capability_id.as_str() == capability_id)
        .unwrap_or_else(|| panic!("missing production mutation definition {capability_id}"))
}

pub fn query_definition(capability_id: &str) -> CapabilityDefinition {
    application_query_definitions()
        .expect("valid production query definitions")
        .into_iter()
        .find(|definition| definition.capability_id.as_str() == capability_id)
        .unwrap_or_else(|| panic!("missing production query definition {capability_id}"))
}

pub fn profile_payload(definition: &CapabilityDefinition, provider_key: &str) -> TypedPayload {
    payload(
        definition,
        enrichment::PublishProviderProfileVersionRequest {
            definition: Some(enrichment::ProviderProfileDefinition {
                provider_key: provider_key.to_owned(),
                adapter_kind: "registry_http_v1".to_owned(),
                adapter_contract_version: "1.0.0".to_owned(),
                supported_target_fields: vec![
                    enrichment::EnrichmentTargetField::PartyDisplayName as i32,
                ],
                purpose_codes: vec![PURPOSE.to_owned()],
                license_id: "crm-api-process-license".to_owned(),
                permitted_use_class: "customer_master_review".to_owned(),
                residency_region: "eu".to_owned(),
                retention_days: 30,
                raw_payload_policy: enrichment::RawProviderPayloadPolicy::DigestOnly as i32,
                credential_handle_aliases: vec![SECRET_MARKER.to_owned()],
                effective_at_unix_ms: now_millis() - 60_000,
                expires_at_unix_ms: Some(now_millis() + 3_600_000),
            }),
        },
    )
}

pub fn mapping_payload(definition: &CapabilityDefinition, profile_id: &str) -> TypedPayload {
    payload(
        definition,
        enrichment::PublishMappingVersionRequest {
            definition: Some(enrichment::MappingDefinition {
                mapping_key: "crm_api_party_display_name".to_owned(),
                provider_profile_version_ref: Some(enrichment::ProviderProfileVersionRef {
                    provider_profile_version_id: profile_id.to_owned(),
                }),
                provider_response_field_path: "organization.legal_name".to_owned(),
                target_field: enrichment::EnrichmentTargetField::PartyDisplayName as i32,
                normalization: enrichment::MappingNormalization::CanonicalPartyDisplayNameV1 as i32,
                maximum_suggestions_per_response: 1,
                confidence_required: true,
            }),
        },
    )
}

pub fn party_payload(definition: &CapabilityDefinition) -> TypedPayload {
    payload(
        definition,
        parties::CreatePartyRequest {
            party_ref: Some(customer::PartyRef {
                party_id: PARTY_ID.to_owned(),
            }),
            kind: parties::PartyKind::Organization as i32,
            display_name: "CRM API Enrichment Company".to_owned(),
        },
    )
}

pub fn missing_consent_request_payload(
    definition: &CapabilityDefinition,
    profile_id: &str,
    mapping_id: &str,
) -> TypedPayload {
    let now = now_millis();
    payload(
        definition,
        enrichment::CreateEnrichmentRequestRequest {
            target: Some(enrichment::EnrichmentTargetSnapshot {
                party_ref: Some(customer::PartyRef {
                    party_id: PARTY_ID.to_owned(),
                }),
                party_resource_version: 1,
                target_field: enrichment::EnrichmentTargetField::PartyDisplayName as i32,
            }),
            provider_profile_version_ref: Some(enrichment::ProviderProfileVersionRef {
                provider_profile_version_id: profile_id.to_owned(),
            }),
            mapping_version_ref: Some(enrichment::MappingVersionRef {
                mapping_version_id: mapping_id.to_owned(),
            }),
            requested_fields: vec![enrichment::EnrichmentTargetField::PartyDisplayName as i32],
            policy_evidence: Some(enrichment::EnrichmentRequestPolicyEvidence {
                purpose_code: PURPOSE.to_owned(),
                legal_basis_code: "consent".to_owned(),
                consent_evidence_reference: None,
                policy_version: "request-policy-v1".to_owned(),
            }),
            deadline_at_unix_ms: now + 60_000,
            expires_at_unix_ms: now + 120_000,
        },
    )
}

pub fn get_profile_payload(definition: &CapabilityDefinition, profile_id: &str) -> TypedPayload {
    payload(
        definition,
        enrichment::GetProviderProfileVersionRequest {
            provider_profile_version_ref: Some(enrichment::ProviderProfileVersionRef {
                provider_profile_version_id: profile_id.to_owned(),
            }),
        },
    )
}

pub fn decode_profile_id(response: &crm_application_runtime::gateway_v1::MutateResponse) -> String {
    enrichment::PublishProviderProfileVersionResponse::decode(
        response
            .output
            .as_ref()
            .expect("profile publish output")
            .payload
            .as_slice(),
    )
    .expect("decode profile publish response")
    .provider_profile_version
    .and_then(|profile| profile.provider_profile_version_ref)
    .expect("published profile reference")
    .provider_profile_version_id
}

pub fn decode_mapping_id(response: &crm_application_runtime::gateway_v1::MutateResponse) -> String {
    enrichment::PublishMappingVersionResponse::decode(
        response
            .output
            .as_ref()
            .expect("mapping publish output")
            .payload
            .as_slice(),
    )
    .expect("decode mapping publish response")
    .mapping_version
    .and_then(|mapping| mapping.mapping_version_ref)
    .expect("published mapping reference")
    .mapping_version_id
}

pub fn decode_profile_query(
    response: crm_application_runtime::gateway_v1::QueryResponse,
) -> enrichment::ProviderProfileVersion {
    enrichment::GetProviderProfileVersionResponse::decode(
        response
            .output
            .expect("profile query output")
            .payload
            .as_slice(),
    )
    .expect("decode profile query response")
    .provider_profile_version
    .expect("queried profile")
}

pub fn payload<M: Message>(definition: &CapabilityDefinition, message: M) -> TypedPayload {
    let data_class = *definition
        .input_contract
        .allowed_data_classes
        .first()
        .expect("input contract data class");
    let payload = TypedPayload {
        owner: definition.input_contract.owner.clone(),
        schema_id: definition.input_contract.schema_id.clone(),
        schema_version: definition.input_contract.schema_version.clone(),
        descriptor_hash: definition.input_contract.descriptor_hash,
        data_class,
        encoding: PayloadEncoding::Protobuf,
        maximum_size_bytes: definition.input_contract.maximum_size_bytes,
        retention_policy_id: RetentionPolicyId::try_new("standard").unwrap(),
        bytes: message.encode_to_vec(),
    };
    payload.validate().expect("valid governed process payload");
    payload
}

pub fn data_class_name(data_class: DataClass) -> &'static str {
    match data_class {
        DataClass::Public => "public",
        DataClass::Internal => "internal",
        DataClass::Confidential => "confidential",
        DataClass::Restricted => "restricted",
        DataClass::Personal => "personal",
        DataClass::SensitivePersonal => "sensitive_personal",
        DataClass::Biometric => "biometric",
        DataClass::Financial => "financial",
        DataClass::Credential => "credential",
    }
}

fn now_millis() -> i64 {
    i64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time after Unix epoch")
            .as_millis(),
    )
    .expect("current timestamp fits i64")
}

pub fn assert_customer_enrichment_owner(definition: &CapabilityDefinition) {
    assert_eq!(definition.owner_module_id.as_str(), MODULE_ID);
}
