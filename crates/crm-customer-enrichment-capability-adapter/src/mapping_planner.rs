use crate::{
    MAPPING_PUBLISHED_EVENT_SCHEMA, MAPPING_PUBLISHED_EVENT_TYPE, MODULE_ID,
    PUBLISH_MAPPING_CAPABILITY, PUBLISH_MAPPING_REQUEST_SCHEMA, PUBLISH_MAPPING_RESPONSE_SCHEMA,
};
use crm_capability_plan_support::{self as support, EventSpec, PersistedPayloadContract};
use crm_capability_runtime::{CapabilityDefinition, CapabilityRequest};
use crm_core_data::{
    AggregatePresence, AggregateTarget, BatchMutationPlan, CapabilityBatchExecutionPlan,
    RecordMutation, TransactionalAggregatePlanner,
};
use crm_customer_enrichment::{
    DEFINITION_STATE_RETENTION_POLICY_ID, DEFINITION_STATE_SCHEMA_VERSION, MAPPING_VERSION_RECORD_TYPE,
    MAPPING_VERSION_STATE_MAXIMUM_BYTES, MAPPING_VERSION_STATE_SCHEMA_ID, MappingDraft,
    MappingNormalization, MappingVersion, ProviderProfileVersionId, TargetField,
    encode_mapping_version_state, mapping_version_state_descriptor_hash,
};
use crm_module_sdk::{DataClass, ErrorCategory, RecordSnapshot, SdkError};
use crm_proto_contracts::crm::customer_enrichment::v1 as wire;

#[derive(Debug, Default, Clone, Copy)]
pub struct CustomerEnrichmentMappingCapabilityPlanner;

impl TransactionalAggregatePlanner for CustomerEnrichmentMappingCapabilityPlanner {
    fn target(
        &self,
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
    ) -> Result<AggregateTarget, SdkError> {
        ensure_definition(definition, request)?;
        let command: wire::PublishMappingVersionRequest = decode_request(request)?;
        let mapping = mapping_from_definition(command.definition)?;
        Ok(AggregateTarget {
            reference: mapping_record_ref(&mapping)?,
            presence: AggregatePresence::MustBeAbsent,
        })
    }

    fn plan(
        &self,
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
        current: Option<&RecordSnapshot>,
    ) -> Result<CapabilityBatchExecutionPlan, SdkError> {
        ensure_definition(definition, request)?;
        if current.is_some() {
            return Err(invalid_plan("immutable mapping version already exists"));
        }
        let command: wire::PublishMappingVersionRequest = decode_request(request)?;
        let mapping = mapping_from_definition(command.definition)?;
        let aggregate = mapping_record_ref(&mapping)?;
        let public_mapping = mapping_to_wire(&mapping);
        let output = support::protobuf_payload(
            MODULE_ID,
            PUBLISH_MAPPING_RESPONSE_SCHEMA,
            DataClass::Confidential,
            &wire::PublishMappingVersionResponse {
                mapping_version: Some(public_mapping.clone()),
            },
        )?;
        let event = support::event_evidence_with_data_class(
            request,
            aggregate.clone(),
            MODULE_ID,
            EventSpec {
                event_type: MAPPING_PUBLISHED_EVENT_TYPE,
                event_schema_id: MAPPING_PUBLISHED_EVENT_SCHEMA,
                aggregate_version: 1,
                previous_version: None,
            },
            DataClass::Confidential,
            &wire::MappingVersionPublishedEvent {
                mapping_version: Some(public_mapping),
            },
        )?;
        let audit = support::audit_intent(
            request,
            &aggregate,
            1,
            definition.capability_id.as_str(),
            &output.bytes,
        )?;
        Ok(CapabilityBatchExecutionPlan {
            batch: BatchMutationPlan {
                context: request.context.clone(),
                records: vec![RecordMutation::Create {
                    reference: aggregate,
                    payload: mapping_persisted_payload(&mapping)?,
                }],
                relationships: Vec::new(),
                events: vec![event],
                idempotency: support::capability_idempotency(definition, request)?,
                audits: vec![audit],
            },
            output: Some(output),
        })
    }
}

pub fn mapping_from_definition(
    definition: Option<wire::MappingDefinition>,
) -> Result<MappingVersion, SdkError> {
    let definition = definition.ok_or_else(|| {
        SdkError::invalid_argument(
            "customer_enrichment.mapping.definition",
            "Mapping definition is required",
        )
    })?;
    let provider_profile_version_ref = definition.provider_profile_version_ref.ok_or_else(|| {
        SdkError::invalid_argument(
            "customer_enrichment.mapping.definition.provider_profile_version_ref",
            "Provider-profile version reference is required",
        )
    })?;
    MappingVersion::publish(MappingDraft {
        mapping_key: definition.mapping_key,
        provider_profile_version_id: provider_profile_version_id_from_external(
            provider_profile_version_ref.provider_profile_version_id,
        )?,
        provider_response_field_path: definition.provider_response_field_path,
        target_field: target_field_from_wire(definition.target_field)?,
        normalization: normalization_from_wire(definition.normalization)?,
        maximum_suggestions_per_response: definition.maximum_suggestions_per_response,
        confidence_required: definition.confidence_required,
    })
}

pub fn mapping_to_wire(mapping: &MappingVersion) -> wire::MappingVersion {
    wire::MappingVersion {
        mapping_version_ref: Some(wire::MappingVersionRef {
            mapping_version_id: mapping.version_id().as_str().to_owned(),
        }),
        definition: Some(wire::MappingDefinition {
            mapping_key: mapping.mapping_key().to_owned(),
            provider_profile_version_ref: Some(wire::ProviderProfileVersionRef {
                provider_profile_version_id: mapping
                    .provider_profile_version_id()
                    .as_str()
                    .to_owned(),
            }),
            provider_response_field_path: mapping.provider_response_field_path().to_owned(),
            target_field: target_field_to_wire(mapping.target_field()),
            normalization: normalization_to_wire(mapping.normalization()),
            maximum_suggestions_per_response: mapping.maximum_suggestions_per_response(),
            confidence_required: mapping.confidence_required(),
        }),
    }
}

pub fn provider_profile_version_id_from_external(
    value: String,
) -> Result<ProviderProfileVersionId, SdkError> {
    const PREFIX: &str = "enrichment-provider-profile-";
    let suffix = value.strip_prefix(PREFIX).ok_or_else(invalid_provider_profile_id)?;
    if suffix.len() != 64
        || !suffix
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return Err(invalid_provider_profile_id());
    }
    serde_json::from_value(serde_json::Value::String(value)).map_err(|_| invalid_provider_profile_id())
}

pub fn mapping_persisted_contract() -> PersistedPayloadContract<'static> {
    PersistedPayloadContract {
        owner: MODULE_ID,
        schema_id: MAPPING_VERSION_STATE_SCHEMA_ID,
        schema_version: DEFINITION_STATE_SCHEMA_VERSION,
        descriptor_hash: mapping_version_state_descriptor_hash(),
        maximum_size_bytes: MAPPING_VERSION_STATE_MAXIMUM_BYTES,
        retention_policy_id: DEFINITION_STATE_RETENTION_POLICY_ID,
    }
}

pub fn mapping_persisted_payload(
    mapping: &MappingVersion,
) -> Result<crm_module_sdk::TypedPayload, SdkError> {
    support::persisted_json_payload_with_data_class(
        mapping_persisted_contract(),
        DataClass::Confidential,
        encode_mapping_version_state(mapping)?,
    )
}

pub fn mapping_record_ref(mapping: &MappingVersion) -> Result<crm_module_sdk::RecordRef, SdkError> {
    support::record_ref(
        MAPPING_VERSION_RECORD_TYPE,
        mapping.version_id().as_str(),
        "customer_enrichment.mapping_version_ref.mapping_version_id",
    )
}

fn target_field_from_wire(value: i32) -> Result<TargetField, SdkError> {
    match wire::EnrichmentTargetField::try_from(value) {
        Ok(wire::EnrichmentTargetField::PartyDisplayName) => Ok(TargetField::PartyDisplayName),
        Ok(wire::EnrichmentTargetField::Unspecified) | Err(_) => Err(SdkError::invalid_argument(
            "customer_enrichment.mapping.definition.target_field",
            "Mapping target field is unsupported",
        )),
    }
}

fn target_field_to_wire(value: TargetField) -> i32 {
    match value {
        TargetField::PartyDisplayName => wire::EnrichmentTargetField::PartyDisplayName as i32,
    }
}

fn normalization_from_wire(value: i32) -> Result<MappingNormalization, SdkError> {
    match wire::MappingNormalization::try_from(value) {
        Ok(wire::MappingNormalization::CanonicalPartyDisplayNameV1) => {
            Ok(MappingNormalization::CanonicalPartyDisplayNameV1)
        }
        Ok(wire::MappingNormalization::Unspecified) | Err(_) => Err(SdkError::invalid_argument(
            "customer_enrichment.mapping.definition.normalization",
            "Mapping normalization is unsupported",
        )),
    }
}

fn normalization_to_wire(value: MappingNormalization) -> i32 {
    match value {
        MappingNormalization::CanonicalPartyDisplayNameV1 => {
            wire::MappingNormalization::CanonicalPartyDisplayNameV1 as i32
        }
    }
}

fn decode_request<T: prost::Message + Default>(request: &CapabilityRequest) -> Result<T, SdkError> {
    support::decode_request_with_data_class(
        request,
        MODULE_ID,
        PUBLISH_MAPPING_REQUEST_SCHEMA,
        DataClass::Confidential,
    )
}

fn ensure_definition(
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
) -> Result<(), SdkError> {
    if definition.capability_id.as_str() != PUBLISH_MAPPING_CAPABILITY
        || definition.owner_module_id.as_str() != MODULE_ID
        || definition.capability_id.as_str() != request.context.execution.capability_id.as_str()
    {
        return Err(invalid_plan(
            "capability definition does not match request context",
        ));
    }
    Ok(())
}

fn invalid_provider_profile_id() -> SdkError {
    SdkError::invalid_argument(
        "customer_enrichment.mapping.definition.provider_profile_version_ref.provider_profile_version_id",
        "Provider-profile version identity must be a canonical content-derived identifier",
    )
}

fn invalid_plan(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_MAPPING_CAPABILITY_PLAN_INVALID",
        ErrorCategory::Internal,
        false,
        "The mapping publication could not be planned safely.",
    )
    .with_internal_reference(reference.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile_id() -> String {
        format!("enrichment-provider-profile-{}", "a".repeat(64))
    }

    fn definition() -> wire::MappingDefinition {
        wire::MappingDefinition {
            mapping_key: "registry_display_name".to_owned(),
            provider_profile_version_ref: Some(wire::ProviderProfileVersionRef {
                provider_profile_version_id: profile_id(),
            }),
            provider_response_field_path: "company.legal_name".to_owned(),
            target_field: wire::EnrichmentTargetField::PartyDisplayName as i32,
            normalization: wire::MappingNormalization::CanonicalPartyDisplayNameV1 as i32,
            maximum_suggestions_per_response: 1,
            confidence_required: true,
        }
    }

    #[test]
    fn wire_definition_round_trips_through_domain() {
        let mapping = mapping_from_definition(Some(definition())).unwrap();
        let wire = mapping_to_wire(&mapping);
        assert_eq!(wire.definition, Some(definition()));
        assert_eq!(
            wire.mapping_version_ref.unwrap().mapping_version_id,
            mapping.version_id().as_str()
        );
    }

    #[test]
    fn provider_profile_identity_format_is_strict() {
        assert!(provider_profile_version_id_from_external(profile_id()).is_ok());
        assert!(provider_profile_version_id_from_external("bad".to_owned()).is_err());
        assert!(provider_profile_version_id_from_external(format!(
            "enrichment-provider-profile-{}",
            "A".repeat(64)
        ))
        .is_err());
    }

    #[test]
    fn persisted_contract_is_exact_and_confidential() {
        let mapping = mapping_from_definition(Some(definition())).unwrap();
        let payload = mapping_persisted_payload(&mapping).unwrap();
        assert_eq!(payload.schema_id.as_str(), MAPPING_VERSION_STATE_SCHEMA_ID);
        assert_eq!(payload.schema_version.as_str(), DEFINITION_STATE_SCHEMA_VERSION);
        assert_eq!(payload.data_class, DataClass::Confidential);
    }
}
