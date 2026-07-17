use crate::{
    MODULE_ID, PROVIDER_PROFILE_PUBLISHED_EVENT_SCHEMA, PROVIDER_PROFILE_PUBLISHED_EVENT_TYPE,
    PUBLISH_PROVIDER_PROFILE_CAPABILITY, PUBLISH_PROVIDER_PROFILE_REQUEST_SCHEMA,
    PUBLISH_PROVIDER_PROFILE_RESPONSE_SCHEMA,
};
use crm_capability_plan_support::{self as support, EventSpec, PersistedPayloadContract};
use crm_capability_runtime::{CapabilityDefinition, CapabilityRequest};
use crm_core_data::{
    AggregatePresence, AggregateTarget, BatchMutationPlan, CapabilityBatchExecutionPlan,
    RecordMutation, TransactionalAggregatePlanner,
};
use crm_customer_enrichment::{
    DEFINITION_STATE_RETENTION_POLICY_ID, DEFINITION_STATE_SCHEMA_VERSION,
    PROVIDER_PROFILE_VERSION_RECORD_TYPE, PROVIDER_PROFILE_VERSION_STATE_MAXIMUM_BYTES,
    PROVIDER_PROFILE_VERSION_STATE_SCHEMA_ID, ProviderProfileDraft, ProviderProfileVersion,
    RawPayloadPolicy, TargetField, encode_provider_profile_version_state,
    provider_profile_version_state_descriptor_hash,
};
use crm_module_sdk::{DataClass, ErrorCategory, RecordSnapshot, SdkError};
use crm_proto_contracts::crm::customer_enrichment::v1 as wire;

#[derive(Debug, Default, Clone, Copy)]
pub struct CustomerEnrichmentProviderProfileCapabilityPlanner;

impl TransactionalAggregatePlanner for CustomerEnrichmentProviderProfileCapabilityPlanner {
    fn target(
        &self,
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
    ) -> Result<AggregateTarget, SdkError> {
        ensure_definition(definition, request)?;
        let command: wire::PublishProviderProfileVersionRequest = decode_request(request)?;
        let profile = provider_profile_from_definition(command.definition)?;
        Ok(AggregateTarget {
            reference: provider_profile_record_ref(&profile)?,
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
            return Err(invalid_plan("immutable provider profile already exists"));
        }

        let command: wire::PublishProviderProfileVersionRequest = decode_request(request)?;
        let profile = provider_profile_from_definition(command.definition)?;
        let aggregate = provider_profile_record_ref(&profile)?;
        let public_profile = provider_profile_to_wire(&profile);
        let output = support::protobuf_payload(
            MODULE_ID,
            PUBLISH_PROVIDER_PROFILE_RESPONSE_SCHEMA,
            DataClass::Confidential,
            &wire::PublishProviderProfileVersionResponse {
                provider_profile_version: Some(public_profile.clone()),
            },
        )?;
        let event = support::event_evidence_with_data_class(
            request,
            aggregate.clone(),
            MODULE_ID,
            EventSpec {
                event_type: PROVIDER_PROFILE_PUBLISHED_EVENT_TYPE,
                event_schema_id: PROVIDER_PROFILE_PUBLISHED_EVENT_SCHEMA,
                aggregate_version: 1,
                previous_version: None,
            },
            DataClass::Confidential,
            &wire::ProviderProfileVersionPublishedEvent {
                provider_profile_version: Some(public_profile),
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
                    payload: provider_profile_persisted_payload(&profile)?,
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

pub fn provider_profile_from_definition(
    definition: Option<wire::ProviderProfileDefinition>,
) -> Result<ProviderProfileVersion, SdkError> {
    let definition = definition.ok_or_else(|| {
        SdkError::invalid_argument(
            "customer_enrichment.provider_profile.definition",
            "Provider profile definition is required",
        )
    })?;

    let supported_target_fields = definition
        .supported_target_fields
        .into_iter()
        .map(target_field_from_wire)
        .collect::<Result<Vec<_>, _>>()?;
    let raw_payload_policy = match wire::RawProviderPayloadPolicy::try_from(
        definition.raw_payload_policy,
    ) {
        Ok(wire::RawProviderPayloadPolicy::DigestOnly) => RawPayloadPolicy::DigestOnly,
        Ok(wire::RawProviderPayloadPolicy::GovernedProtectedEvidence) => {
            RawPayloadPolicy::GovernedProtectedEvidence
        }
        Ok(wire::RawProviderPayloadPolicy::Unspecified) | Err(_) => {
            return Err(SdkError::invalid_argument(
                "customer_enrichment.provider_profile.definition.raw_payload_policy",
                "Provider raw-payload policy is unsupported",
            ));
        }
    };

    ProviderProfileVersion::publish(ProviderProfileDraft {
        provider_key: definition.provider_key,
        adapter_kind: definition.adapter_kind,
        adapter_contract_version: definition.adapter_contract_version,
        supported_target_fields,
        purpose_codes: definition.purpose_codes,
        license_id: definition.license_id,
        permitted_use_class: definition.permitted_use_class,
        residency_region: definition.residency_region,
        retention_days: definition.retention_days,
        raw_payload_policy,
        credential_handle_aliases: definition.credential_handle_aliases,
        effective_at_unix_ms: non_negative_timestamp(
            definition.effective_at_unix_ms,
            "customer_enrichment.provider_profile.definition.effective_at_unix_ms",
        )?,
        expires_at_unix_ms: definition
            .expires_at_unix_ms
            .map(|value| {
                non_negative_timestamp(
                    value,
                    "customer_enrichment.provider_profile.definition.expires_at_unix_ms",
                )
            })
            .transpose()?,
    })
}

pub fn provider_profile_to_wire(profile: &ProviderProfileVersion) -> wire::ProviderProfileVersion {
    wire::ProviderProfileVersion {
        provider_profile_version_ref: Some(wire::ProviderProfileVersionRef {
            provider_profile_version_id: profile.version_id().as_str().to_owned(),
        }),
        definition: Some(wire::ProviderProfileDefinition {
            provider_key: profile.provider_key().to_owned(),
            adapter_kind: profile.adapter_kind().to_owned(),
            adapter_contract_version: profile.adapter_contract_version().to_owned(),
            supported_target_fields: profile
                .supported_target_fields()
                .iter()
                .copied()
                .map(target_field_to_wire)
                .collect(),
            purpose_codes: profile.purpose_codes().to_vec(),
            license_id: profile.license_id().to_owned(),
            permitted_use_class: profile.permitted_use_class().to_owned(),
            residency_region: profile.residency_region().to_owned(),
            retention_days: profile.retention_days(),
            raw_payload_policy: match profile.raw_payload_policy() {
                RawPayloadPolicy::DigestOnly => wire::RawProviderPayloadPolicy::DigestOnly as i32,
                RawPayloadPolicy::GovernedProtectedEvidence => {
                    wire::RawProviderPayloadPolicy::GovernedProtectedEvidence as i32
                }
            },
            credential_handle_aliases: profile.credential_handle_aliases().to_vec(),
            effective_at_unix_ms: profile.effective_at_unix_ms() as i64,
            expires_at_unix_ms: profile.expires_at_unix_ms().map(|value| value as i64),
        }),
    }
}

pub fn provider_profile_persisted_contract() -> PersistedPayloadContract<'static> {
    PersistedPayloadContract {
        owner: MODULE_ID,
        schema_id: PROVIDER_PROFILE_VERSION_STATE_SCHEMA_ID,
        schema_version: DEFINITION_STATE_SCHEMA_VERSION,
        descriptor_hash: provider_profile_version_state_descriptor_hash(),
        maximum_size_bytes: PROVIDER_PROFILE_VERSION_STATE_MAXIMUM_BYTES,
        retention_policy_id: DEFINITION_STATE_RETENTION_POLICY_ID,
    }
}

pub fn provider_profile_persisted_payload(
    profile: &ProviderProfileVersion,
) -> Result<crm_module_sdk::TypedPayload, SdkError> {
    support::persisted_json_payload_with_data_class(
        provider_profile_persisted_contract(),
        DataClass::Confidential,
        encode_provider_profile_version_state(profile)?,
    )
}

pub fn provider_profile_record_ref(
    profile: &ProviderProfileVersion,
) -> Result<crm_module_sdk::RecordRef, SdkError> {
    support::record_ref(
        PROVIDER_PROFILE_VERSION_RECORD_TYPE,
        profile.version_id().as_str(),
        "customer_enrichment.provider_profile_version_ref.provider_profile_version_id",
    )
}

fn target_field_from_wire(value: i32) -> Result<TargetField, SdkError> {
    match wire::EnrichmentTargetField::try_from(value) {
        Ok(wire::EnrichmentTargetField::PartyDisplayName) => Ok(TargetField::PartyDisplayName),
        Ok(wire::EnrichmentTargetField::Unspecified) | Err(_) => {
            Err(SdkError::invalid_argument(
                "customer_enrichment.provider_profile.definition.supported_target_fields",
                "Provider target field is unsupported",
            ))
        }
    }
}

fn target_field_to_wire(value: TargetField) -> i32 {
    match value {
        TargetField::PartyDisplayName => wire::EnrichmentTargetField::PartyDisplayName as i32,
    }
}

fn non_negative_timestamp(value: i64, field: &'static str) -> Result<u64, SdkError> {
    value
        .try_into()
        .map_err(|_| SdkError::invalid_argument(field, "Timestamp must not be negative"))
}

fn decode_request<T: prost::Message + Default>(request: &CapabilityRequest) -> Result<T, SdkError> {
    support::decode_request_with_data_class(
        request,
        MODULE_ID,
        PUBLISH_PROVIDER_PROFILE_REQUEST_SCHEMA,
        DataClass::Confidential,
    )
}

fn ensure_definition(
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
) -> Result<(), SdkError> {
    if definition.capability_id.as_str() != PUBLISH_PROVIDER_PROFILE_CAPABILITY
        || definition.owner_module_id.as_str() != MODULE_ID
        || definition.capability_id.as_str() != request.context.execution.capability_id.as_str()
    {
        return Err(invalid_plan("capability definition does not match request context"));
    }
    Ok(())
}

fn invalid_plan(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_PROVIDER_PROFILE_CAPABILITY_PLAN_INVALID",
        ErrorCategory::Internal,
        false,
        "The provider profile publication could not be planned safely.",
    )
    .with_internal_reference(reference.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn definition() -> wire::ProviderProfileDefinition {
        wire::ProviderProfileDefinition {
            provider_key: "company_registry".to_owned(),
            adapter_kind: "registry_http_v1".to_owned(),
            adapter_contract_version: "1.0.0".to_owned(),
            supported_target_fields: vec![wire::EnrichmentTargetField::PartyDisplayName as i32],
            purpose_codes: vec!["customer_profile_enrichment".to_owned()],
            license_id: "Registry commercial data licence v3".to_owned(),
            permitted_use_class: "customer_master_review".to_owned(),
            residency_region: "eu".to_owned(),
            retention_days: 30,
            raw_payload_policy: wire::RawProviderPayloadPolicy::DigestOnly as i32,
            credential_handle_aliases: vec!["registry_primary".to_owned()],
            effective_at_unix_ms: 1_000,
            expires_at_unix_ms: Some(2_000),
        }
    }

    #[test]
    fn wire_definition_round_trips_through_domain() {
        let profile = provider_profile_from_definition(Some(definition())).unwrap();
        let wire = provider_profile_to_wire(&profile);
        assert_eq!(wire.definition, Some(definition()));
        assert_eq!(
            wire.provider_profile_version_ref
                .unwrap()
                .provider_profile_version_id,
            profile.version_id().as_str()
        );
    }

    #[test]
    fn negative_timestamps_and_unspecified_policy_are_rejected() {
        let mut negative = definition();
        negative.effective_at_unix_ms = -1;
        assert!(provider_profile_from_definition(Some(negative)).is_err());

        let mut unspecified = definition();
        unspecified.raw_payload_policy = wire::RawProviderPayloadPolicy::Unspecified as i32;
        assert!(provider_profile_from_definition(Some(unspecified)).is_err());
    }

    #[test]
    fn persisted_contract_is_exact_and_confidential() {
        let profile = provider_profile_from_definition(Some(definition())).unwrap();
        let payload = provider_profile_persisted_payload(&profile).unwrap();
        assert_eq!(payload.schema_id, PROVIDER_PROFILE_VERSION_STATE_SCHEMA_ID);
        assert_eq!(payload.schema_version, DEFINITION_STATE_SCHEMA_VERSION);
        assert_eq!(payload.data_class, DataClass::Confidential);
    }
}
