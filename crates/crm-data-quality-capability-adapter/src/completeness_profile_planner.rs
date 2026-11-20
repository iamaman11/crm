use crate::{
    MODULE_ID, PARTY_COMPLETENESS_PROFILE_PUBLISHED_EVENT_SCHEMA,
    PARTY_COMPLETENESS_PROFILE_PUBLISHED_EVENT_TYPE, PUBLISH_PARTY_COMPLETENESS_PROFILE_CAPABILITY,
    PUBLISH_PARTY_COMPLETENESS_PROFILE_REQUEST_SCHEMA,
    PUBLISH_PARTY_COMPLETENESS_PROFILE_RESPONSE_SCHEMA,
};
use crm_capability_plan_support::{self as support, EventSpec, PersistedPayloadContract};
use crm_capability_runtime::{CapabilityDefinition, CapabilityRequest};
use crm_core_data::{
    AggregatePresence, AggregateTarget, BatchMutationPlan, CapabilityBatchExecutionPlan,
    RecordMutation, TransactionalAggregatePlanner,
};
use crm_data_quality::{
    ComponentKey, PARTY_COMPLETENESS_PROFILE_VERSION_RECORD_TYPE,
    PARTY_COMPLETENESS_PROFILE_VERSION_STATE_MAXIMUM_BYTES,
    PARTY_COMPLETENESS_PROFILE_VERSION_STATE_RETENTION_POLICY_ID,
    PARTY_COMPLETENESS_PROFILE_VERSION_STATE_SCHEMA_ID,
    PARTY_COMPLETENESS_PROFILE_VERSION_STATE_SCHEMA_VERSION, PartyCompletenessComponent,
    PartyCompletenessProfileVersion, PartyRuleSetVersion, RuleKey,
    decode_party_completeness_profile_version_state,
    encode_party_completeness_profile_version_state,
    party_completeness_profile_version_state_descriptor_hash,
};
use crm_module_sdk::{DataClass, ErrorCategory, RecordSnapshot, SdkError};
use crm_proto_contracts::crm::data_quality::v1 as wire;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletenessProfileReferenceScope {
    pub rule_set_version_id: String,
}

#[derive(Debug, Clone)]
pub struct DataQualityCompletenessProfileCapabilityPlanner {
    rule_set: PartyRuleSetVersion,
}

impl DataQualityCompletenessProfileCapabilityPlanner {
    pub fn new(rule_set: PartyRuleSetVersion) -> Self {
        Self { rule_set }
    }
}

impl TransactionalAggregatePlanner for DataQualityCompletenessProfileCapabilityPlanner {
    fn target(
        &self,
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
    ) -> Result<AggregateTarget, SdkError> {
        ensure_definition(definition, request)?;
        let command: wire::PublishPartyCompletenessProfileVersionRequest = decode_request(request)?;
        let profile =
            party_completeness_profile_from_definition(command.definition, &self.rule_set)?;
        Ok(AggregateTarget {
            reference: completeness_profile_record_ref(&profile)?,
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
            return Err(invalid_plan());
        }

        let command: wire::PublishPartyCompletenessProfileVersionRequest = decode_request(request)?;
        let profile =
            party_completeness_profile_from_definition(command.definition, &self.rule_set)?;
        let aggregate = completeness_profile_record_ref(&profile)?;
        let public_profile = party_completeness_profile_to_wire(&profile);
        let output = support::protobuf_payload(
            MODULE_ID,
            PUBLISH_PARTY_COMPLETENESS_PROFILE_RESPONSE_SCHEMA,
            DataClass::Confidential,
            &wire::PublishPartyCompletenessProfileVersionResponse {
                completeness_profile_version: Some(public_profile.clone()),
            },
        )?;
        let event = support::event_evidence_with_data_class(
            request,
            aggregate.clone(),
            MODULE_ID,
            EventSpec {
                event_type: PARTY_COMPLETENESS_PROFILE_PUBLISHED_EVENT_TYPE,
                event_schema_id: PARTY_COMPLETENESS_PROFILE_PUBLISHED_EVENT_SCHEMA,
                aggregate_version: 1,
                previous_version: None,
            },
            DataClass::Confidential,
            &wire::PartyCompletenessProfileVersionPublishedEvent {
                completeness_profile_version: Some(public_profile),
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
                    payload: party_completeness_profile_persisted_payload(&profile)?,
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

pub fn completeness_profile_reference_scope_from_request(
    request: &CapabilityRequest,
) -> Result<CompletenessProfileReferenceScope, SdkError> {
    let command: wire::PublishPartyCompletenessProfileVersionRequest = decode_request(request)?;
    let definition = command.definition.ok_or_else(|| {
        SdkError::invalid_argument(
            "data_quality.party_completeness_profile.definition",
            "Party completeness-profile definition is required",
        )
    })?;
    ensure_completeness_semantic_version(definition.completeness_semantic_version)?;
    let rule_set_version_id = definition
        .rule_set_version_ref
        .ok_or_else(|| {
            SdkError::invalid_argument(
                "data_quality.party_completeness_profile.definition.rule_set_version_ref",
                "Party rule-set version reference is required",
            )
        })?
        .rule_set_version_id;
    if rule_set_version_id.is_empty() {
        return Err(SdkError::invalid_argument(
            "data_quality.party_completeness_profile.definition.rule_set_version_ref.rule_set_version_id",
            "Party rule-set version reference is invalid",
        ));
    }
    Ok(CompletenessProfileReferenceScope {
        rule_set_version_id,
    })
}

pub fn party_completeness_profile_from_definition(
    definition: Option<wire::PartyCompletenessProfileDefinition>,
    rule_set: &PartyRuleSetVersion,
) -> Result<PartyCompletenessProfileVersion, SdkError> {
    let definition = definition.ok_or_else(|| {
        SdkError::invalid_argument(
            "data_quality.party_completeness_profile.definition",
            "Party completeness-profile definition is required",
        )
    })?;
    ensure_completeness_semantic_version(definition.completeness_semantic_version)?;
    let rule_set_ref = definition.rule_set_version_ref.ok_or_else(|| {
        SdkError::invalid_argument(
            "data_quality.party_completeness_profile.definition.rule_set_version_ref",
            "Party rule-set version reference is required",
        )
    })?;
    if rule_set_ref.rule_set_version_id != rule_set.version_id().as_str() {
        return Err(reference_unavailable());
    }
    PartyCompletenessProfileVersion::publish(
        rule_set,
        definition
            .components
            .into_iter()
            .map(|component| {
                PartyCompletenessComponent::try_new(
                    ComponentKey::try_new(component.component_key)?,
                    RuleKey::try_new(component.rule_key)?,
                    component.weight_basis_points,
                )
            })
            .collect::<Result<Vec<_>, _>>()?,
    )
}

pub fn party_completeness_profile_to_wire(
    profile: &PartyCompletenessProfileVersion,
) -> wire::PartyCompletenessProfileVersion {
    wire::PartyCompletenessProfileVersion {
        completeness_profile_version_ref: Some(wire::PartyCompletenessProfileVersionRef {
            completeness_profile_version_id: profile.version_id().as_str().to_owned(),
        }),
        definition: Some(wire::PartyCompletenessProfileDefinition {
            completeness_semantic_version: wire::PartyCompletenessSemanticVersion::V1 as i32,
            rule_set_version_ref: Some(wire::PartyRuleSetVersionRef {
                rule_set_version_id: profile.rule_set_version_id().as_str().to_owned(),
            }),
            components: profile
                .components()
                .iter()
                .map(|component| wire::PartyCompletenessComponent {
                    component_key: component.component_key().as_str().to_owned(),
                    rule_key: component.rule_key().as_str().to_owned(),
                    weight_basis_points: component.weight_basis_points(),
                })
                .collect(),
        }),
    }
}

pub fn party_completeness_profile_persisted_contract() -> PersistedPayloadContract<'static> {
    PersistedPayloadContract {
        owner: MODULE_ID,
        schema_id: PARTY_COMPLETENESS_PROFILE_VERSION_STATE_SCHEMA_ID,
        schema_version: PARTY_COMPLETENESS_PROFILE_VERSION_STATE_SCHEMA_VERSION,
        descriptor_hash: party_completeness_profile_version_state_descriptor_hash(),
        maximum_size_bytes: PARTY_COMPLETENESS_PROFILE_VERSION_STATE_MAXIMUM_BYTES,
        retention_policy_id: PARTY_COMPLETENESS_PROFILE_VERSION_STATE_RETENTION_POLICY_ID,
    }
}

pub fn party_completeness_profile_persisted_payload(
    profile: &PartyCompletenessProfileVersion,
) -> Result<crm_module_sdk::TypedPayload, SdkError> {
    support::persisted_json_payload_with_data_class(
        party_completeness_profile_persisted_contract(),
        DataClass::Confidential,
        encode_party_completeness_profile_version_state(profile)?,
    )
}

pub fn party_completeness_profile_from_snapshot(
    snapshot: &RecordSnapshot,
    rule_set: &PartyRuleSetVersion,
) -> Result<PartyCompletenessProfileVersion, SdkError> {
    if snapshot.reference.record_type.as_str() != PARTY_COMPLETENESS_PROFILE_VERSION_RECORD_TYPE {
        return Err(invalid_plan());
    }
    let bytes = support::persisted_json_bytes_with_data_class(
        snapshot,
        party_completeness_profile_persisted_contract(),
        DataClass::Confidential,
    )?;
    let profile = decode_party_completeness_profile_version_state(bytes, rule_set)?;
    if snapshot.reference.record_id.as_str() != profile.version_id().as_str() {
        return Err(invalid_plan());
    }
    Ok(profile)
}

pub fn completeness_profile_rule_set_version_id_from_snapshot(
    snapshot: &RecordSnapshot,
) -> Result<String, SdkError> {
    if snapshot.reference.record_type.as_str() != PARTY_COMPLETENESS_PROFILE_VERSION_RECORD_TYPE {
        return Err(invalid_plan());
    }
    let bytes = support::persisted_json_bytes_with_data_class(
        snapshot,
        party_completeness_profile_persisted_contract(),
        DataClass::Confidential,
    )?;
    crm_data_quality::party_completeness_profile_rule_set_version_id_from_state(bytes)
}

fn completeness_profile_record_ref(
    profile: &PartyCompletenessProfileVersion,
) -> Result<crm_module_sdk::RecordRef, SdkError> {
    support::record_ref(
        PARTY_COMPLETENESS_PROFILE_VERSION_RECORD_TYPE,
        profile.version_id().as_str(),
        "data_quality.party_completeness_profile_version_ref.completeness_profile_version_id",
    )
}

fn ensure_completeness_semantic_version(value: i32) -> Result<(), SdkError> {
    match wire::PartyCompletenessSemanticVersion::try_from(value) {
        Ok(wire::PartyCompletenessSemanticVersion::V1) => Ok(()),
        Ok(wire::PartyCompletenessSemanticVersion::Unspecified) | Err(_) => {
            Err(SdkError::invalid_argument(
                "data_quality.party_completeness_profile.definition.completeness_semantic_version",
                "Party completeness semantic version must be V1",
            ))
        }
    }
}

fn decode_request<T: prost::Message + Default>(request: &CapabilityRequest) -> Result<T, SdkError> {
    support::decode_request_with_data_class(
        request,
        MODULE_ID,
        PUBLISH_PARTY_COMPLETENESS_PROFILE_REQUEST_SCHEMA,
        DataClass::Confidential,
    )
}

fn ensure_definition(
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
) -> Result<(), SdkError> {
    if definition.capability_id.as_str() != PUBLISH_PARTY_COMPLETENESS_PROFILE_CAPABILITY
        || definition.owner_module_id.as_str() != MODULE_ID
        || definition.capability_id.as_str() != request.context.execution.capability_id.as_str()
    {
        return Err(invalid_plan());
    }
    Ok(())
}

fn reference_unavailable() -> SdkError {
    SdkError::new(
        "DATA_QUALITY_COMPLETENESS_RULE_SET_UNAVAILABLE",
        ErrorCategory::InvalidArgument,
        false,
        "The referenced Party rule-set version is unavailable.",
    )
}

fn invalid_plan() -> SdkError {
    SdkError::new(
        "DATA_QUALITY_COMPLETENESS_PROFILE_CAPABILITY_PLAN_INVALID",
        ErrorCategory::Internal,
        false,
        "The Party completeness-profile publication could not be planned safely.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crm_data_quality::{
        DisplayNameMinUtf8Bytes, PartyQualityEvaluator, PartyQualityRule, QualitySeverity,
    };

    fn rule_set() -> PartyRuleSetVersion {
        PartyRuleSetVersion::publish(vec![
            PartyQualityRule::try_new(
                RuleKey::try_new("display_name.minimum").unwrap(),
                QualitySeverity::Warning,
                PartyQualityEvaluator::DisplayNameMinUtf8Bytes(
                    DisplayNameMinUtf8Bytes::try_new(4).unwrap(),
                ),
                "Display name length",
                "Use a meaningful display name.",
            )
            .unwrap(),
        ])
        .unwrap()
    }

    #[test]
    fn definition_binds_exact_rule_set_and_canonicalizes_components() {
        let rule_set = rule_set();
        let first = party_completeness_profile_from_definition(
            Some(wire::PartyCompletenessProfileDefinition {
                completeness_semantic_version: wire::PartyCompletenessSemanticVersion::V1 as i32,
                rule_set_version_ref: Some(wire::PartyRuleSetVersionRef {
                    rule_set_version_id: rule_set.version_id().as_str().to_owned(),
                }),
                components: vec![wire::PartyCompletenessComponent {
                    component_key: "display_name.minimum".to_owned(),
                    rule_key: "display_name.minimum".to_owned(),
                    weight_basis_points: 10_000,
                }],
            }),
            &rule_set,
        )
        .unwrap();
        let second = party_completeness_profile_from_definition(
            party_completeness_profile_to_wire(&first).definition,
            &rule_set,
        )
        .unwrap();
        assert_eq!(first, second);
    }

    #[test]
    fn definition_rejects_different_or_unknown_rule_set_binding() {
        let rule_set = rule_set();
        let result = party_completeness_profile_from_definition(
            Some(wire::PartyCompletenessProfileDefinition {
                completeness_semantic_version: wire::PartyCompletenessSemanticVersion::V1 as i32,
                rule_set_version_ref: Some(wire::PartyRuleSetVersionRef {
                    rule_set_version_id: "dq-party-rule-set-unavailable".to_owned(),
                }),
                components: vec![wire::PartyCompletenessComponent {
                    component_key: "display_name.minimum".to_owned(),
                    rule_key: "display_name.minimum".to_owned(),
                    weight_basis_points: 10_000,
                }],
            }),
            &rule_set,
        );
        assert_eq!(
            result.unwrap_err().code,
            "DATA_QUALITY_COMPLETENESS_RULE_SET_UNAVAILABLE"
        );
    }
}
