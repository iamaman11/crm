use crate::{
    MODULE_ID, PARTY_REMEDIATION_COMPLETED_EVENT_SCHEMA, PARTY_REMEDIATION_COMPLETED_EVENT_TYPE,
    REMEDIATE_PARTY_DISPLAY_NAME_CAPABILITY, REMEDIATE_PARTY_DISPLAY_NAME_REQUEST_SCHEMA,
    REMEDIATE_PARTY_DISPLAY_NAME_RESPONSE_SCHEMA,
};
use crm_capability_plan_support::{self as support, EventSpec, PersistedPayloadContract};
use crm_capability_runtime::{CapabilityDefinition, CapabilityRequest};
use crm_core_data::{
    AggregatePresence, AggregateTarget, BatchMutationPlan, CapabilityBatchExecutionPlan,
    RecordMutation, TransactionalAggregatePlanner,
};
use crm_data_quality::{
    REMEDIATION_ATTEMPT_RECORD_TYPE, REMEDIATION_ATTEMPT_STATE_MAXIMUM_BYTES,
    REMEDIATION_ATTEMPT_STATE_RETENTION_POLICY_ID, REMEDIATION_ATTEMPT_STATE_SCHEMA_ID,
    REMEDIATION_ATTEMPT_STATE_SCHEMA_VERSION, PartyDisplayNameRemediationAttempt,
    encode_remediation_attempt_state, remediation_attempt_state_descriptor_hash,
};
use crm_module_sdk::{DataClass, ErrorCategory, RecordSnapshot, SdkError};
use crm_proto_contracts::crm::{core::v1 as core, customer::v1 as customer, data_quality::v1 as wire};

#[derive(Debug, Clone)]
pub struct DataQualityRemediationCompletionPlanner {
    attempt: PartyDisplayNameRemediationAttempt,
    updated_party: crm_proto_contracts::crm::parties::v1::Party,
}

impl DataQualityRemediationCompletionPlanner {
    pub fn new(
        attempt: PartyDisplayNameRemediationAttempt,
        updated_party: crm_proto_contracts::crm::parties::v1::Party,
    ) -> Result<Self, SdkError> {
        let party_ref = updated_party
            .party_ref
            .as_ref()
            .ok_or_else(|| invalid_plan("updated Party reference is missing"))?;
        let version = updated_party
            .resource_version
            .as_ref()
            .ok_or_else(|| invalid_plan("updated Party version is missing"))?
            .version;
        if party_ref.party_id != attempt.party_id().as_str()
            || version != attempt.updated_party_version()
            || updated_party.display_name != attempt.requested_display_name()
        {
            return Err(invalid_plan(
                "updated Party does not match remediation attempt evidence",
            ));
        }
        Ok(Self {
            attempt,
            updated_party,
        })
    }
}

impl TransactionalAggregatePlanner for DataQualityRemediationCompletionPlanner {
    fn target(
        &self,
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
    ) -> Result<AggregateTarget, SdkError> {
        ensure_definition(definition, request)?;
        Ok(AggregateTarget {
            reference: remediation_attempt_record_ref(&self.attempt)?,
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
            return Err(invalid_plan("remediation attempt already exists"));
        }
        let command: wire::RemediatePartyDisplayNameRequest =
            support::decode_request_with_data_class(
                request,
                MODULE_ID,
                REMEDIATE_PARTY_DISPLAY_NAME_REQUEST_SCHEMA,
                DataClass::Personal,
            )?;
        verify_command(&command, &self.attempt)?;
        let aggregate = remediation_attempt_record_ref(&self.attempt)?;
        let public_attempt = remediation_attempt_to_wire(&self.attempt, 1);
        let output = support::protobuf_payload(
            MODULE_ID,
            REMEDIATE_PARTY_DISPLAY_NAME_RESPONSE_SCHEMA,
            DataClass::Personal,
            &wire::RemediatePartyDisplayNameResponse {
                remediation_attempt: Some(public_attempt.clone()),
                party: Some(self.updated_party.clone()),
            },
        )?;
        let event = support::event_evidence_with_data_class(
            request,
            aggregate.clone(),
            MODULE_ID,
            EventSpec {
                event_type: PARTY_REMEDIATION_COMPLETED_EVENT_TYPE,
                event_schema_id: PARTY_REMEDIATION_COMPLETED_EVENT_SCHEMA,
                aggregate_version: 1,
                previous_version: None,
            },
            DataClass::Personal,
            &wire::PartyDisplayNameRemediationCompletedEvent {
                remediation_attempt: Some(public_attempt),
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
                    payload: remediation_attempt_persisted_payload(&self.attempt)?,
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

pub fn remediation_attempt_persisted_contract() -> PersistedPayloadContract<'static> {
    PersistedPayloadContract {
        owner: MODULE_ID,
        schema_id: REMEDIATION_ATTEMPT_STATE_SCHEMA_ID,
        schema_version: REMEDIATION_ATTEMPT_STATE_SCHEMA_VERSION,
        descriptor_hash: remediation_attempt_state_descriptor_hash(),
        maximum_size_bytes: REMEDIATION_ATTEMPT_STATE_MAXIMUM_BYTES,
        retention_policy_id: REMEDIATION_ATTEMPT_STATE_RETENTION_POLICY_ID,
    }
}

pub fn remediation_attempt_persisted_payload(
    attempt: &PartyDisplayNameRemediationAttempt,
) -> Result<crm_module_sdk::TypedPayload, SdkError> {
    support::persisted_json_payload_with_data_class(
        remediation_attempt_persisted_contract(),
        DataClass::Personal,
        encode_remediation_attempt_state(attempt)?,
    )
}

pub fn remediation_attempt_record_ref(
    attempt: &PartyDisplayNameRemediationAttempt,
) -> Result<crm_module_sdk::RecordRef, SdkError> {
    support::record_ref(
        REMEDIATION_ATTEMPT_RECORD_TYPE,
        attempt.attempt_id(),
        "data_quality.remediation_attempt_ref.remediation_attempt_id",
    )
}

pub fn remediation_attempt_to_wire(
    attempt: &PartyDisplayNameRemediationAttempt,
    version: i64,
) -> wire::PartyDisplayNameRemediationAttempt {
    wire::PartyDisplayNameRemediationAttempt {
        remediation_attempt_ref: Some(wire::DataQualityRemediationAttemptRef {
            remediation_attempt_id: attempt.attempt_id().to_owned(),
        }),
        finding_ref: Some(wire::DataQualityFindingRef {
            finding_id: attempt.finding_id().to_owned(),
        }),
        finding_observation_ref: Some(wire::DataQualityFindingObservationRef {
            finding_observation_id: attempt.observation_id().to_owned(),
        }),
        party_ref: Some(customer::PartyRef {
            party_id: attempt.party_id().as_str().to_owned(),
        }),
        expected_party_resource_version: Some(customer::CustomerResourceVersion {
            version: attempt.expected_party_version(),
            created_at: None,
            updated_at: None,
        }),
        requested_display_name: attempt.requested_display_name().to_owned(),
        target_idempotency_key: attempt.target_idempotency_key().as_str().to_owned(),
        updated_party_resource_version: Some(customer::CustomerResourceVersion {
            version: attempt.updated_party_version(),
            created_at: None,
            updated_at: None,
        }),
        completed_at: Some(core::UnixTime {
            unix_nanos: attempt.completed_at(),
        }),
        resource_version: Some(customer::CustomerResourceVersion {
            version,
            created_at: Some(core::UnixTime {
                unix_nanos: attempt.completed_at(),
            }),
            updated_at: Some(core::UnixTime {
                unix_nanos: attempt.completed_at(),
            }),
        }),
    }
}

fn verify_command(
    command: &wire::RemediatePartyDisplayNameRequest,
    attempt: &PartyDisplayNameRemediationAttempt,
) -> Result<(), SdkError> {
    if command.expected_finding_version != attempt.expected_finding_version()
        || command.expected_party_resource_version != attempt.expected_party_version()
        || command.display_name != attempt.requested_display_name()
        || command
            .finding_ref
            .as_ref()
            .is_none_or(|value| value.finding_id != attempt.finding_id())
        || command
            .expected_current_observation_ref
            .as_ref()
            .is_none_or(|value| value.finding_observation_id != attempt.observation_id())
    {
        return Err(invalid_plan(
            "remediation command differs from completed attempt evidence",
        ));
    }
    Ok(())
}

fn ensure_definition(
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
) -> Result<(), SdkError> {
    if definition.capability_id.as_str() != REMEDIATE_PARTY_DISPLAY_NAME_CAPABILITY
        || definition.owner_module_id.as_str() != MODULE_ID
        || request.context.execution.capability_id.as_str()
            != REMEDIATE_PARTY_DISPLAY_NAME_CAPABILITY
    {
        return Err(invalid_plan(
            "capability definition does not match remediation completion",
        ));
    }
    Ok(())
}

fn invalid_plan(reference: &str) -> SdkError {
    SdkError::new(
        "DATA_QUALITY_REMEDIATION_COMPLETION_PLAN_INVALID",
        ErrorCategory::Internal,
        false,
        "The Party display-name remediation outcome could not be committed safely.",
    )
    .with_internal_reference(reference)
}
