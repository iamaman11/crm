use crate::{
    MODULE_ID, PARTY_EVALUATION_STAGED_EVENT_SCHEMA, PARTY_EVALUATION_STAGED_EVENT_TYPE,
    STAGE_PARTY_EVALUATION_INPUT_CAPABILITY, STAGE_PARTY_EVALUATION_INPUT_REQUEST_SCHEMA,
    STAGE_PARTY_EVALUATION_INPUT_RESPONSE_SCHEMA, party_evaluation_job_from_snapshot,
    party_evaluation_job_persisted_payload, party_evaluation_job_to_wire,
};
use crm_capability_plan_support::{self as support, EventSpec, PersistedPayloadContract};
use crm_capability_runtime::{CapabilityDefinition, CapabilityRequest};
use crm_core_data::{
    AggregatePresence, AggregateTarget, BatchMutationPlan, CapabilityBatchExecutionPlan,
    RecordMutation, TransactionalAggregatePlanner,
};
use crm_data_quality::{
    EvaluatedPartyKind, PARTY_EVALUATION_INPUT_RECORD_TYPE,
    PARTY_EVALUATION_INPUT_STATE_MAXIMUM_BYTES, PARTY_EVALUATION_INPUT_STATE_RETENTION_POLICY_ID,
    PARTY_EVALUATION_INPUT_STATE_SCHEMA_ID, PARTY_EVALUATION_INPUT_STATE_SCHEMA_VERSION,
    PartyEvaluationInputSnapshot, PartyEvaluationJobStatus, encode_party_evaluation_input_state,
    party_evaluation_input_state_descriptor_hash,
};
use crm_module_sdk::{DataClass, ErrorCategory, RecordId, RecordSnapshot, SdkError};
use crm_proto_contracts::crm::{
    customer::v1 as customer, data_quality::v1 as wire, parties::v1 as parties,
};

#[derive(Debug, Default, Clone, Copy)]
pub struct DataQualityEvaluationStagePlanner;

impl TransactionalAggregatePlanner for DataQualityEvaluationStagePlanner {
    fn target(
        &self,
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
    ) -> Result<AggregateTarget, SdkError> {
        ensure_definition(definition, request)?;
        let command = decode_request(request)?;
        Ok(AggregateTarget {
            reference: job_record_ref(command.evaluation_job_ref)?,
            presence: AggregatePresence::MustExist,
        })
    }

    fn plan(
        &self,
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
        current: Option<&RecordSnapshot>,
    ) -> Result<CapabilityBatchExecutionPlan, SdkError> {
        ensure_definition(definition, request)?;
        let current = current.ok_or_else(|| invalid_plan("evaluation job is unavailable"))?;
        let command = decode_request(request)?;
        if command.expected_job_version <= 0 || command.expected_job_version != current.version {
            return Err(SdkError::new(
                "DATA_QUALITY_EVALUATION_STAGE_VERSION_CONFLICT",
                ErrorCategory::Conflict,
                false,
                "The Party evaluation job changed before it could be staged.",
            ));
        }
        let job = party_evaluation_job_from_snapshot(current)?;
        if job.status() != PartyEvaluationJobStatus::Created {
            return Err(invalid_plan("only a created evaluation job can be staged"));
        }
        let requested_party_id = record_id(
            command
                .party_ref
                .ok_or_else(|| missing("party_ref"))?
                .party_id,
            "data_quality.party_ref.party_id",
        )?;
        if requested_party_id != *job.party_id() {
            return Err(invalid_plan("staged Party differs from the evaluation job"));
        }
        let kind = match parties::PartyKind::try_from(command.party_kind) {
            Ok(parties::PartyKind::Person) => EvaluatedPartyKind::Person,
            Ok(parties::PartyKind::Organization) => EvaluatedPartyKind::Organization,
            Ok(parties::PartyKind::Unspecified) | Err(_) => {
                return Err(SdkError::invalid_argument(
                    "data_quality.party_kind",
                    "Party kind must be PERSON or ORGANIZATION",
                ));
            }
        };
        let (staged_job, input) = job.stage(
            kind,
            command.display_name,
            command.party_resource_version,
            request.context.execution.request_started_at_unix_nanos,
        )?;
        let aggregate = current.reference.clone();
        let aggregate_version = current
            .version
            .checked_add(1)
            .ok_or_else(|| invalid_plan("evaluation job version overflowed"))?;
        let public_job = party_evaluation_job_to_wire(&staged_job, aggregate_version);
        let output = support::protobuf_payload(
            MODULE_ID,
            STAGE_PARTY_EVALUATION_INPUT_RESPONSE_SCHEMA,
            DataClass::Personal,
            &wire::StagePartyEvaluationInputResponse {
                evaluation_job: Some(public_job.clone()),
            },
        )?;
        let event = support::event_evidence_with_data_class(
            request,
            aggregate.clone(),
            MODULE_ID,
            EventSpec {
                event_type: PARTY_EVALUATION_STAGED_EVENT_TYPE,
                event_schema_id: PARTY_EVALUATION_STAGED_EVENT_SCHEMA,
                aggregate_version,
                previous_version: Some(current.version),
            },
            DataClass::Personal,
            &wire::PartyEvaluationStagedEvent {
                evaluation_job: Some(public_job),
            },
        )?;
        let audit = support::audit_intent(
            request,
            &aggregate,
            aggregate_version,
            definition.capability_id.as_str(),
            &output.bytes,
        )?;
        Ok(CapabilityBatchExecutionPlan {
            batch: BatchMutationPlan {
                context: request.context.clone(),
                records: vec![
                    RecordMutation::Update {
                        reference: aggregate,
                        expected_version: current.version,
                        payload: party_evaluation_job_persisted_payload(&staged_job)?,
                    },
                    RecordMutation::Create {
                        reference: party_evaluation_input_record_ref(&input)?,
                        payload: party_evaluation_input_persisted_payload(&input)?,
                    },
                ],
                relationships: Vec::new(),
                events: vec![event],
                idempotency: support::capability_idempotency(definition, request)?,
                audits: vec![audit],
            },
            output: Some(output),
        })
    }
}

pub fn party_evaluation_input_persisted_contract() -> PersistedPayloadContract<'static> {
    PersistedPayloadContract {
        owner: MODULE_ID,
        schema_id: PARTY_EVALUATION_INPUT_STATE_SCHEMA_ID,
        schema_version: PARTY_EVALUATION_INPUT_STATE_SCHEMA_VERSION,
        descriptor_hash: party_evaluation_input_state_descriptor_hash(),
        maximum_size_bytes: PARTY_EVALUATION_INPUT_STATE_MAXIMUM_BYTES,
        retention_policy_id: PARTY_EVALUATION_INPUT_STATE_RETENTION_POLICY_ID,
    }
}

pub fn party_evaluation_input_persisted_payload(
    input: &PartyEvaluationInputSnapshot,
) -> Result<crm_module_sdk::TypedPayload, SdkError> {
    support::persisted_json_payload_with_data_class(
        party_evaluation_input_persisted_contract(),
        DataClass::Personal,
        encode_party_evaluation_input_state(input)?,
    )
}

pub fn party_evaluation_input_record_ref(
    input: &PartyEvaluationInputSnapshot,
) -> Result<crm_module_sdk::RecordRef, SdkError> {
    support::record_ref(
        PARTY_EVALUATION_INPUT_RECORD_TYPE,
        input.job_id().as_str(),
        "data_quality.evaluation_input.job_id",
    )
}

fn decode_request(
    request: &CapabilityRequest,
) -> Result<wire::StagePartyEvaluationInputRequest, SdkError> {
    support::decode_request_with_data_class(
        request,
        MODULE_ID,
        STAGE_PARTY_EVALUATION_INPUT_REQUEST_SCHEMA,
        DataClass::Personal,
    )
}

fn job_record_ref(
    value: Option<wire::PartyEvaluationJobRef>,
) -> Result<crm_module_sdk::RecordRef, SdkError> {
    let job_id = record_id(
        value
            .ok_or_else(|| missing("evaluation_job_ref"))?
            .evaluation_job_id,
        "data_quality.evaluation_job_ref.evaluation_job_id",
    )?;
    support::record_ref(
        crm_data_quality::PARTY_EVALUATION_JOB_RECORD_TYPE,
        job_id.as_str(),
        "data_quality.evaluation_job_ref.evaluation_job_id",
    )
}

fn record_id(value: String, field: &'static str) -> Result<RecordId, SdkError> {
    RecordId::try_new(value).map_err(|error| SdkError::invalid_argument(field, error.to_string()))
}

fn missing(field: &'static str) -> SdkError {
    SdkError::invalid_argument(field, "The reference is required")
}

fn ensure_definition(
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
) -> Result<(), SdkError> {
    if definition.capability_id.as_str() != STAGE_PARTY_EVALUATION_INPUT_CAPABILITY
        || definition.owner_module_id.as_str() != MODULE_ID
        || definition.capability_id.as_str() != request.context.execution.capability_id.as_str()
    {
        return Err(invalid_plan(
            "capability definition does not match the request",
        ));
    }
    Ok(())
}

fn invalid_plan(reference: &str) -> SdkError {
    SdkError::new(
        "DATA_QUALITY_EVALUATION_STAGE_PLAN_INVALID",
        ErrorCategory::Internal,
        false,
        "The Party evaluation input could not be staged safely.",
    )
    .with_internal_reference(reference)
}
