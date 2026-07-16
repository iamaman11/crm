use crate::{
    ACKNOWLEDGE_FINDING_CAPABILITY, ACKNOWLEDGE_FINDING_REQUEST_SCHEMA,
    ACKNOWLEDGE_FINDING_RESPONSE_SCHEMA, ASSIGN_FINDING_CAPABILITY, ASSIGN_FINDING_REQUEST_SCHEMA,
    ASSIGN_FINDING_RESPONSE_SCHEMA, FINDING_ASSIGNMENT_CHANGED_EVENT_SCHEMA,
    FINDING_ASSIGNMENT_CHANGED_EVENT_TYPE, FINDING_STATUS_CHANGED_EVENT_SCHEMA,
    FINDING_STATUS_CHANGED_EVENT_TYPE, MODULE_ID, WAIVE_FINDING_CAPABILITY,
    WAIVE_FINDING_REQUEST_SCHEMA, WAIVE_FINDING_RESPONSE_SCHEMA, party_finding_persisted_contract,
    party_finding_persisted_payload, party_finding_to_wire,
};
use crm_capability_plan_support::{self as support, EventSpec};
use crm_capability_runtime::{CapabilityDefinition, CapabilityRequest};
use crm_core_data::{
    AggregatePresence, AggregateTarget, BatchMutationPlan, CapabilityBatchExecutionPlan,
    RecordMutation, TransactionalAggregatePlanner,
};
use crm_data_quality::{PartyFinding, decode_finding_state};
use crm_module_sdk::{ActorId, DataClass, ErrorCategory, RecordId, RecordSnapshot, SdkError};
use crm_proto_contracts::crm::data_quality::v1 as wire;

#[derive(Debug, Default, Clone, Copy)]
pub struct DataQualityFindingStewardshipPlanner;

impl TransactionalAggregatePlanner for DataQualityFindingStewardshipPlanner {
    fn target(
        &self,
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
    ) -> Result<AggregateTarget, SdkError> {
        ensure_definition(definition, request)?;
        Ok(AggregateTarget {
            reference: finding_record_ref(definition, request)?,
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
        let current = current.ok_or_else(finding_unavailable)?;
        let expected_version = expected_version(definition, request)?;
        if expected_version <= 0 || expected_version != current.version {
            return Err(version_conflict());
        }
        let finding = party_finding_from_snapshot(current)?;
        let now = request.context.execution.request_started_at_unix_nanos;
        let updated = apply_command(definition, request, &finding, now)?;
        let aggregate_version = current
            .version
            .checked_add(1)
            .ok_or_else(|| invalid_plan("finding version overflowed"))?;
        let public_finding = party_finding_to_wire(&updated, aggregate_version);
        let (output, event) = match definition.capability_id.as_str() {
            ASSIGN_FINDING_CAPABILITY => {
                let output = support::protobuf_payload(
                    MODULE_ID,
                    ASSIGN_FINDING_RESPONSE_SCHEMA,
                    DataClass::Personal,
                    &wire::AssignDataQualityFindingResponse {
                        finding: Some(public_finding.clone()),
                    },
                )?;
                let event = support::event_evidence_with_data_class(
                    request,
                    current.reference.clone(),
                    MODULE_ID,
                    EventSpec {
                        event_type: FINDING_ASSIGNMENT_CHANGED_EVENT_TYPE,
                        event_schema_id: FINDING_ASSIGNMENT_CHANGED_EVENT_SCHEMA,
                        aggregate_version,
                        previous_version: Some(current.version),
                    },
                    DataClass::Personal,
                    &wire::DataQualityFindingAssignmentChangedEvent {
                        finding: Some(public_finding),
                    },
                )?;
                (output, event)
            }
            ACKNOWLEDGE_FINDING_CAPABILITY => {
                let output = support::protobuf_payload(
                    MODULE_ID,
                    ACKNOWLEDGE_FINDING_RESPONSE_SCHEMA,
                    DataClass::Personal,
                    &wire::AcknowledgeDataQualityFindingResponse {
                        finding: Some(public_finding.clone()),
                    },
                )?;
                let event = support::event_evidence_with_data_class(
                    request,
                    current.reference.clone(),
                    MODULE_ID,
                    EventSpec {
                        event_type: FINDING_STATUS_CHANGED_EVENT_TYPE,
                        event_schema_id: FINDING_STATUS_CHANGED_EVENT_SCHEMA,
                        aggregate_version,
                        previous_version: Some(current.version),
                    },
                    DataClass::Personal,
                    &wire::DataQualityFindingStatusChangedEvent {
                        finding: Some(public_finding),
                    },
                )?;
                (output, event)
            }
            WAIVE_FINDING_CAPABILITY => {
                let output = support::protobuf_payload(
                    MODULE_ID,
                    WAIVE_FINDING_RESPONSE_SCHEMA,
                    DataClass::Personal,
                    &wire::WaiveDataQualityFindingResponse {
                        finding: Some(public_finding.clone()),
                    },
                )?;
                let event = support::event_evidence_with_data_class(
                    request,
                    current.reference.clone(),
                    MODULE_ID,
                    EventSpec {
                        event_type: FINDING_STATUS_CHANGED_EVENT_TYPE,
                        event_schema_id: FINDING_STATUS_CHANGED_EVENT_SCHEMA,
                        aggregate_version,
                        previous_version: Some(current.version),
                    },
                    DataClass::Personal,
                    &wire::DataQualityFindingStatusChangedEvent {
                        finding: Some(public_finding),
                    },
                )?;
                (output, event)
            }
            _ => return Err(unsupported()),
        };
        let audit = support::audit_intent(
            request,
            &current.reference,
            aggregate_version,
            definition.capability_id.as_str(),
            &output.bytes,
        )?;
        Ok(CapabilityBatchExecutionPlan {
            batch: BatchMutationPlan {
                context: request.context.clone(),
                records: vec![RecordMutation::Update {
                    reference: current.reference.clone(),
                    expected_version: current.version,
                    payload: party_finding_persisted_payload(&updated)?,
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

fn apply_command(
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
    finding: &PartyFinding,
    now: i64,
) -> Result<PartyFinding, SdkError> {
    match definition.capability_id.as_str() {
        ASSIGN_FINDING_CAPABILITY => {
            let command: wire::AssignDataQualityFindingRequest =
                decode(request, ASSIGN_FINDING_REQUEST_SCHEMA)?;
            let actor = command
                .assigned_actor_id
                .map(ActorId::try_new)
                .transpose()
                .map_err(|error| {
                    SdkError::invalid_argument(
                        "data_quality.finding.assigned_actor_id",
                        error.to_string(),
                    )
                })?;
            finding.assign(actor, now)
        }
        ACKNOWLEDGE_FINDING_CAPABILITY => {
            let command: wire::AcknowledgeDataQualityFindingRequest =
                decode(request, ACKNOWLEDGE_FINDING_REQUEST_SCHEMA)?;
            let observation_id = required_observation_id(command.expected_current_observation_ref)?;
            finding.acknowledge(&observation_id, now)
        }
        WAIVE_FINDING_CAPABILITY => {
            let command: wire::WaiveDataQualityFindingRequest =
                decode(request, WAIVE_FINDING_REQUEST_SCHEMA)?;
            let observation_id = required_observation_id(command.expected_current_observation_ref)?;
            finding.waive(&observation_id, command.reason, now)
        }
        _ => Err(unsupported()),
    }
}

pub fn party_finding_from_snapshot(snapshot: &RecordSnapshot) -> Result<PartyFinding, SdkError> {
    let bytes = support::persisted_json_bytes_with_data_class(
        snapshot,
        party_finding_persisted_contract(),
        DataClass::Personal,
    )?;
    let finding = decode_finding_state(bytes)?;
    if snapshot.version <= 0 || finding.finding_id() != snapshot.reference.record_id.as_str() {
        return Err(invalid_plan(
            "persisted finding identity or version is invalid",
        ));
    }
    Ok(finding)
}

fn finding_record_ref(
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
) -> Result<crm_module_sdk::RecordRef, SdkError> {
    let finding_id = match definition.capability_id.as_str() {
        ASSIGN_FINDING_CAPABILITY => {
            let command: wire::AssignDataQualityFindingRequest =
                decode(request, ASSIGN_FINDING_REQUEST_SCHEMA)?;
            required_finding_id(command.finding_ref)?
        }
        ACKNOWLEDGE_FINDING_CAPABILITY => {
            let command: wire::AcknowledgeDataQualityFindingRequest =
                decode(request, ACKNOWLEDGE_FINDING_REQUEST_SCHEMA)?;
            required_finding_id(command.finding_ref)?
        }
        WAIVE_FINDING_CAPABILITY => {
            let command: wire::WaiveDataQualityFindingRequest =
                decode(request, WAIVE_FINDING_REQUEST_SCHEMA)?;
            required_finding_id(command.finding_ref)?
        }
        _ => return Err(unsupported()),
    };
    support::record_ref(
        crm_data_quality::FINDING_RECORD_TYPE,
        finding_id.as_str(),
        "data_quality.finding_ref.finding_id",
    )
}

fn expected_version(
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
) -> Result<i64, SdkError> {
    match definition.capability_id.as_str() {
        ASSIGN_FINDING_CAPABILITY => Ok(decode::<wire::AssignDataQualityFindingRequest>(
            request,
            ASSIGN_FINDING_REQUEST_SCHEMA,
        )?
        .expected_version),
        ACKNOWLEDGE_FINDING_CAPABILITY => Ok(decode::<wire::AcknowledgeDataQualityFindingRequest>(
            request,
            ACKNOWLEDGE_FINDING_REQUEST_SCHEMA,
        )?
        .expected_version),
        WAIVE_FINDING_CAPABILITY => Ok(decode::<wire::WaiveDataQualityFindingRequest>(
            request,
            WAIVE_FINDING_REQUEST_SCHEMA,
        )?
        .expected_version),
        _ => Err(unsupported()),
    }
}

fn decode<T: prost::Message + Default>(
    request: &CapabilityRequest,
    schema: &'static str,
) -> Result<T, SdkError> {
    support::decode_request_with_data_class(request, MODULE_ID, schema, DataClass::Personal)
}

fn required_finding_id(value: Option<wire::DataQualityFindingRef>) -> Result<RecordId, SdkError> {
    RecordId::try_new(value.ok_or_else(|| missing("finding_ref"))?.finding_id).map_err(|error| {
        SdkError::invalid_argument("data_quality.finding_ref.finding_id", error.to_string())
    })
}

fn required_observation_id(
    value: Option<wire::DataQualityFindingObservationRef>,
) -> Result<String, SdkError> {
    RecordId::try_new(
        value
            .ok_or_else(|| missing("expected_current_observation_ref"))?
            .finding_observation_id,
    )
    .map(RecordId::into_inner)
    .map_err(|error| {
        SdkError::invalid_argument(
            "data_quality.finding.expected_current_observation_ref.finding_observation_id",
            error.to_string(),
        )
    })
}

fn ensure_definition(
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
) -> Result<(), SdkError> {
    if ![
        ASSIGN_FINDING_CAPABILITY,
        ACKNOWLEDGE_FINDING_CAPABILITY,
        WAIVE_FINDING_CAPABILITY,
    ]
    .contains(&definition.capability_id.as_str())
        || definition.owner_module_id.as_str() != MODULE_ID
        || definition.capability_id.as_str() != request.context.execution.capability_id.as_str()
    {
        return Err(unsupported());
    }
    Ok(())
}

fn missing(field: &'static str) -> SdkError {
    SdkError::invalid_argument(field, "The required finding reference is missing")
}

fn finding_unavailable() -> SdkError {
    SdkError::new(
        "DATA_QUALITY_FINDING_NOT_FOUND",
        ErrorCategory::NotFound,
        false,
        "The requested Data Quality finding was not found.",
    )
}

fn version_conflict() -> SdkError {
    SdkError::new(
        "DATA_QUALITY_FINDING_VERSION_CONFLICT",
        ErrorCategory::Conflict,
        false,
        "The Data Quality finding changed before the stewardship action could be applied.",
    )
}

fn unsupported() -> SdkError {
    invalid_plan("capability definition does not match finding stewardship")
}

fn invalid_plan(reference: &str) -> SdkError {
    SdkError::new(
        "DATA_QUALITY_FINDING_STEWARDSHIP_PLAN_INVALID",
        ErrorCategory::Internal,
        false,
        "The Data Quality finding stewardship action could not be planned safely.",
    )
    .with_internal_reference(reference)
}
