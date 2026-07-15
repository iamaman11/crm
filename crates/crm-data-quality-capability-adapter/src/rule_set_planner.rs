use crate::{
    MODULE_ID, PARTY_RULE_SET_PUBLISHED_EVENT_SCHEMA, PARTY_RULE_SET_PUBLISHED_EVENT_TYPE,
    PUBLISH_PARTY_RULE_SET_CAPABILITY, PUBLISH_PARTY_RULE_SET_REQUEST_SCHEMA,
    PUBLISH_PARTY_RULE_SET_RESPONSE_SCHEMA,
};
use crm_capability_plan_support::{self as support, EventSpec, PersistedPayloadContract};
use crm_capability_runtime::{CapabilityDefinition, CapabilityRequest};
use crm_core_data::{
    AggregatePresence, AggregateTarget, BatchMutationPlan, CapabilityBatchExecutionPlan,
    RecordMutation, TransactionalAggregatePlanner,
};
use crm_data_quality::{
    DisplayNameMinUtf8Bytes, DisplayNamePlaceholderExactAsciiCasefold,
    PARTY_RULE_SET_VERSION_RECORD_TYPE, PARTY_RULE_SET_VERSION_STATE_MAXIMUM_BYTES,
    PARTY_RULE_SET_VERSION_STATE_RETENTION_POLICY_ID, PARTY_RULE_SET_VERSION_STATE_SCHEMA_ID,
    PARTY_RULE_SET_VERSION_STATE_SCHEMA_VERSION, PartyQualityEvaluator, PartyQualityRule,
    PartyRuleSetVersion, QualitySeverity, RuleKey, decode_party_rule_set_version_state,
    encode_party_rule_set_version_state, party_rule_set_version_state_descriptor_hash,
};
use crm_module_sdk::{DataClass, ErrorCategory, RecordSnapshot, SdkError};
use crm_proto_contracts::crm::data_quality::v1 as wire;

#[derive(Debug, Default, Clone, Copy)]
pub struct DataQualityRuleSetCapabilityPlanner;

impl TransactionalAggregatePlanner for DataQualityRuleSetCapabilityPlanner {
    fn target(
        &self,
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
    ) -> Result<AggregateTarget, SdkError> {
        ensure_definition(definition, request)?;
        let command: wire::PublishPartyRuleSetVersionRequest = decode_request(request)?;
        let rule_set = party_rule_set_from_definition(command.definition)?;
        Ok(AggregateTarget {
            reference: rule_set_record_ref(&rule_set)?,
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

        let command: wire::PublishPartyRuleSetVersionRequest = decode_request(request)?;
        let rule_set = party_rule_set_from_definition(command.definition)?;
        let aggregate = rule_set_record_ref(&rule_set)?;
        let public_rule_set = party_rule_set_to_wire(&rule_set);
        let output = support::protobuf_payload(
            MODULE_ID,
            PUBLISH_PARTY_RULE_SET_RESPONSE_SCHEMA,
            DataClass::Confidential,
            &wire::PublishPartyRuleSetVersionResponse {
                rule_set_version: Some(public_rule_set.clone()),
            },
        )?;
        let event = support::event_evidence_with_data_class(
            request,
            aggregate.clone(),
            MODULE_ID,
            EventSpec {
                event_type: PARTY_RULE_SET_PUBLISHED_EVENT_TYPE,
                event_schema_id: PARTY_RULE_SET_PUBLISHED_EVENT_SCHEMA,
                aggregate_version: 1,
                previous_version: None,
            },
            DataClass::Confidential,
            &wire::PartyRuleSetVersionPublishedEvent {
                rule_set_version: Some(public_rule_set),
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
                    payload: party_rule_set_persisted_payload(&rule_set)?,
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

pub fn party_rule_set_from_definition(
    definition: Option<wire::PartyRuleSetDefinition>,
) -> Result<PartyRuleSetVersion, SdkError> {
    let definition = definition.ok_or_else(|| {
        SdkError::invalid_argument(
            "data_quality.party_rule_set.definition",
            "Party rule-set definition is required",
        )
    })?;
    match wire::PartyQualityEvaluatorSemanticVersion::try_from(
        definition.evaluator_semantic_version,
    ) {
        Ok(wire::PartyQualityEvaluatorSemanticVersion::V1) => {}
        Ok(wire::PartyQualityEvaluatorSemanticVersion::Unspecified) | Err(_) => {
            return Err(SdkError::invalid_argument(
                "data_quality.party_rule_set.definition.evaluator_semantic_version",
                "Party quality evaluator semantic version must be V1",
            ));
        }
    }

    PartyRuleSetVersion::publish(
        definition
            .rules
            .into_iter()
            .map(party_quality_rule_from_wire)
            .collect::<Result<Vec<_>, _>>()?,
    )
}

pub fn party_rule_set_to_wire(rule_set: &PartyRuleSetVersion) -> wire::PartyRuleSetVersion {
    wire::PartyRuleSetVersion {
        rule_set_version_ref: Some(wire::PartyRuleSetVersionRef {
            rule_set_version_id: rule_set.version_id().as_str().to_owned(),
        }),
        definition: Some(wire::PartyRuleSetDefinition {
            evaluator_semantic_version: wire::PartyQualityEvaluatorSemanticVersion::V1 as i32,
            rules: rule_set
                .rules()
                .iter()
                .map(party_quality_rule_to_wire)
                .collect(),
        }),
    }
}

pub fn party_rule_set_persisted_contract() -> PersistedPayloadContract<'static> {
    PersistedPayloadContract {
        owner: MODULE_ID,
        schema_id: PARTY_RULE_SET_VERSION_STATE_SCHEMA_ID,
        schema_version: PARTY_RULE_SET_VERSION_STATE_SCHEMA_VERSION,
        descriptor_hash: party_rule_set_version_state_descriptor_hash(),
        maximum_size_bytes: PARTY_RULE_SET_VERSION_STATE_MAXIMUM_BYTES,
        retention_policy_id: PARTY_RULE_SET_VERSION_STATE_RETENTION_POLICY_ID,
    }
}

pub fn party_rule_set_persisted_payload(
    rule_set: &PartyRuleSetVersion,
) -> Result<crm_module_sdk::TypedPayload, SdkError> {
    support::persisted_json_payload_with_data_class(
        party_rule_set_persisted_contract(),
        DataClass::Confidential,
        encode_party_rule_set_version_state(rule_set)?,
    )
}

pub fn party_rule_set_from_snapshot(
    snapshot: &RecordSnapshot,
) -> Result<PartyRuleSetVersion, SdkError> {
    if snapshot.reference.record_type.as_str() != PARTY_RULE_SET_VERSION_RECORD_TYPE {
        return Err(invalid_plan());
    }
    let contract = party_rule_set_persisted_contract();
    support::validate_persisted_payload(
        &snapshot.payload,
        &contract,
        DataClass::Confidential,
    )?;
    let rule_set = decode_party_rule_set_version_state(snapshot.payload.bytes.as_slice())?;
    if snapshot.reference.record_id.as_str() != rule_set.version_id().as_str() {
        return Err(invalid_plan());
    }
    Ok(rule_set)
}

fn party_quality_rule_from_wire(value: wire::PartyQualityRule) -> Result<PartyQualityRule, SdkError> {
    let severity = match wire::QualitySeverity::try_from(value.severity) {
        Ok(wire::QualitySeverity::Info) => QualitySeverity::Info,
        Ok(wire::QualitySeverity::Warning) => QualitySeverity::Warning,
        Ok(wire::QualitySeverity::Error) => QualitySeverity::Error,
        Ok(wire::QualitySeverity::Critical) => QualitySeverity::Critical,
        Ok(wire::QualitySeverity::Unspecified) | Err(_) => {
            return Err(SdkError::invalid_argument(
                "data_quality.party_rule_set.definition.rules.severity",
                "Party quality rule severity is unsupported",
            ));
        }
    };
    let evaluator = match value.evaluator.ok_or_else(|| {
        SdkError::invalid_argument(
            "data_quality.party_rule_set.definition.rules.evaluator",
            "Party quality rule evaluator is required",
        )
    })? {
        wire::party_quality_rule::Evaluator::DisplayNameMinUtf8Bytes(parameters) => {
            PartyQualityEvaluator::DisplayNameMinUtf8Bytes(DisplayNameMinUtf8Bytes::try_new(
                parameters.minimum_utf8_bytes,
            )?)
        }
        wire::party_quality_rule::Evaluator::DisplayNamePlaceholderExactAsciiCasefold(
            parameters,
        ) => PartyQualityEvaluator::DisplayNamePlaceholderExactAsciiCasefold(
            DisplayNamePlaceholderExactAsciiCasefold::try_new(parameters.placeholder_tokens)?,
        ),
    };
    PartyQualityRule::try_new(
        RuleKey::try_new(value.rule_key)?,
        severity,
        evaluator,
        value.title,
        value.remediation_guidance,
    )
}

fn party_quality_rule_to_wire(rule: &PartyQualityRule) -> wire::PartyQualityRule {
    let evaluator = match rule.evaluator() {
        PartyQualityEvaluator::DisplayNameMinUtf8Bytes(parameters) => {
            wire::party_quality_rule::Evaluator::DisplayNameMinUtf8Bytes(
                wire::PartyDisplayNameMinUtf8BytesEvaluator {
                    minimum_utf8_bytes: parameters.minimum_utf8_bytes(),
                },
            )
        }
        PartyQualityEvaluator::DisplayNamePlaceholderExactAsciiCasefold(parameters) => {
            wire::party_quality_rule::Evaluator::DisplayNamePlaceholderExactAsciiCasefold(
                wire::PartyDisplayNamePlaceholderExactAsciiCasefoldEvaluator {
                    placeholder_tokens: parameters.placeholder_tokens().to_vec(),
                },
            )
        }
    };
    wire::PartyQualityRule {
        rule_key: rule.rule_key().as_str().to_owned(),
        severity: match rule.severity() {
            QualitySeverity::Info => wire::QualitySeverity::Info as i32,
            QualitySeverity::Warning => wire::QualitySeverity::Warning as i32,
            QualitySeverity::Error => wire::QualitySeverity::Error as i32,
            QualitySeverity::Critical => wire::QualitySeverity::Critical as i32,
        },
        evaluator: Some(evaluator),
        title: rule.title().to_owned(),
        remediation_guidance: rule.remediation_guidance().to_owned(),
    }
}

fn rule_set_record_ref(
    rule_set: &PartyRuleSetVersion,
) -> Result<crm_module_sdk::RecordRef, SdkError> {
    support::record_ref(
        PARTY_RULE_SET_VERSION_RECORD_TYPE,
        rule_set.version_id().as_str(),
        "data_quality.party_rule_set_version_ref.rule_set_version_id",
    )
}

fn decode_request<T: prost::Message + Default>(request: &CapabilityRequest) -> Result<T, SdkError> {
    support::decode_request_with_data_class(
        request,
        MODULE_ID,
        PUBLISH_PARTY_RULE_SET_REQUEST_SCHEMA,
        DataClass::Confidential,
    )
}

fn ensure_definition(
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
) -> Result<(), SdkError> {
    if definition.capability_id.as_str() != PUBLISH_PARTY_RULE_SET_CAPABILITY
        || definition.owner_module_id.as_str() != MODULE_ID
        || definition.capability_id.as_str() != request.context.execution.capability_id.as_str()
    {
        return Err(invalid_plan());
    }
    Ok(())
}

fn invalid_plan() -> SdkError {
    SdkError::new(
        "DATA_QUALITY_RULE_SET_CAPABILITY_PLAN_INVALID",
        ErrorCategory::Internal,
        false,
        "The Party rule-set publication could not be planned safely.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn definition(rules: Vec<wire::PartyQualityRule>) -> wire::PartyRuleSetDefinition {
        wire::PartyRuleSetDefinition {
            evaluator_semantic_version: wire::PartyQualityEvaluatorSemanticVersion::V1 as i32,
            rules,
        }
    }

    fn minimum_rule(key: &str, minimum: u32) -> wire::PartyQualityRule {
        wire::PartyQualityRule {
            rule_key: key.to_owned(),
            severity: wire::QualitySeverity::Warning as i32,
            evaluator: Some(
                wire::party_quality_rule::Evaluator::DisplayNameMinUtf8Bytes(
                    wire::PartyDisplayNameMinUtf8BytesEvaluator {
                        minimum_utf8_bytes: minimum,
                    },
                ),
            ),
            title: "Display name length".to_owned(),
            remediation_guidance: "Replace the display name with a meaningful customer name."
                .to_owned(),
        }
    }

    fn placeholder_rule(key: &str, tokens: &[&str]) -> wire::PartyQualityRule {
        wire::PartyQualityRule {
            rule_key: key.to_owned(),
            severity: wire::QualitySeverity::Error as i32,
            evaluator: Some(
                wire::party_quality_rule::Evaluator::DisplayNamePlaceholderExactAsciiCasefold(
                    wire::PartyDisplayNamePlaceholderExactAsciiCasefoldEvaluator {
                        placeholder_tokens: tokens
                            .iter()
                            .map(|value| (*value).to_owned())
                            .collect(),
                    },
                ),
            ),
            title: "Placeholder display name".to_owned(),
            remediation_guidance: "Replace the placeholder with the real customer name."
                .to_owned(),
        }
    }

    #[test]
    fn wire_definition_is_canonicalized_before_publication_identity() {
        let first = party_rule_set_from_definition(Some(definition(vec![
            placeholder_rule("display_name.placeholder", &[" UNKNOWN ", "N/A"]),
            minimum_rule("display_name.minimum", 4),
        ])))
        .unwrap();
        let second = party_rule_set_from_definition(Some(definition(vec![
            minimum_rule("display_name.minimum", 4),
            placeholder_rule("display_name.placeholder", &["n/a", "unknown"]),
        ])))
        .unwrap();

        assert_eq!(first.version_id(), second.version_id());
        assert_eq!(party_rule_set_to_wire(&first), party_rule_set_to_wire(&second));
    }

    #[test]
    fn wire_definition_rejects_unspecified_semantics_and_evaluator() {
        let mut unsupported = definition(vec![minimum_rule("display_name.minimum", 4)]);
        unsupported.evaluator_semantic_version =
            wire::PartyQualityEvaluatorSemanticVersion::Unspecified as i32;
        assert!(party_rule_set_from_definition(Some(unsupported)).is_err());

        let mut missing_evaluator = minimum_rule("display_name.minimum", 4);
        missing_evaluator.evaluator = None;
        assert!(party_rule_set_from_definition(Some(definition(vec![missing_evaluator]))).is_err());
    }

    #[test]
    fn persisted_payload_round_trip_revalidates_content_addressed_identity() {
        let rule_set = party_rule_set_from_definition(Some(definition(vec![minimum_rule(
            "display_name.minimum",
            4,
        )])))
        .unwrap();
        let snapshot = RecordSnapshot {
            reference: rule_set_record_ref(&rule_set).unwrap(),
            version: 1,
            payload: party_rule_set_persisted_payload(&rule_set).unwrap(),
        };

        assert_eq!(party_rule_set_from_snapshot(&snapshot).unwrap(), rule_set);
    }
}
