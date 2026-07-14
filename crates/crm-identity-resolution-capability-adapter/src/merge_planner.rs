use crate::{
    MERGE_CAPABILITY, MERGE_EVENT_SCHEMA, MERGE_EVENT_TYPE, MERGE_OPERATION_RECORD_TYPE,
    MERGE_REQUEST_SCHEMA, MERGE_RESPONSE_SCHEMA, MODULE_ID, UNMERGE_CAPABILITY,
    UNMERGE_EVENT_SCHEMA, UNMERGE_EVENT_TYPE, UNMERGE_REQUEST_SCHEMA, UNMERGE_RESPONSE_SCHEMA,
};
use crm_capability_plan_support::{self as support, EventSpec, PersistedPayloadContract};
use crm_capability_runtime::{CapabilityDefinition, CapabilityRequest};
use crm_core_data::{
    AggregatePresence, AggregateTarget, BatchMutationPlan, CapabilityBatchExecutionPlan,
    RecordMutation, TransactionalAggregatePlanner,
};
use crm_identity_resolution::{
    CreateMergeOperation, DecisionReference, FieldPath, LineageDecisionReasonCode,
    MERGE_OPERATION_STATE_MAXIMUM_BYTES, MERGE_OPERATION_STATE_RETENTION_POLICY_ID,
    MERGE_OPERATION_STATE_SCHEMA_ID, MERGE_OPERATION_STATE_SCHEMA_VERSION, MergeOperation,
    MergeOperationId, MergeOperationStatus, PartyReference, SourceValueDigest,
    SurvivorshipSelection, UnmergeMergeOperation, decode_merge_operation_state,
    encode_merge_operation_state, merge_operation_state_descriptor_hash,
};
use crm_module_sdk::{DataClass, RecordSnapshot, SdkError};
use crm_proto_contracts::crm::{
    core::v1 as core, customer::v1 as customer, identity_resolution::v1 as wire,
};

#[derive(Debug, Default, Clone, Copy)]
pub struct MergeLineageCapabilityPlanner;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergePartyVersionExpectation {
    pub party_ref: PartyReference,
    pub expected_version: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergeReferenceScope {
    pub source: MergePartyVersionExpectation,
    pub survivor: MergePartyVersionExpectation,
    pub provenance: Vec<MergePartyVersionExpectation>,
}

impl TransactionalAggregatePlanner for MergeLineageCapabilityPlanner {
    fn target(
        &self,
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
    ) -> Result<AggregateTarget, SdkError> {
        ensure_definition(definition, request)?;
        let (operation_id, presence) = match definition.capability_id.as_str() {
            MERGE_CAPABILITY => {
                let command: wire::MergePartyRequest = support::decode_request_with_data_class(
                    request,
                    MODULE_ID,
                    MERGE_REQUEST_SCHEMA,
                    DataClass::Personal,
                )?;
                (
                    merge_operation_id_from_ref(
                        command.merge_operation_ref,
                        "identity_resolution.merge.merge_operation_ref",
                    )?,
                    AggregatePresence::MustBeAbsent,
                )
            }
            UNMERGE_CAPABILITY => {
                let command: wire::UnmergePartyRequest = support::decode_request_with_data_class(
                    request,
                    MODULE_ID,
                    UNMERGE_REQUEST_SCHEMA,
                    DataClass::Personal,
                )?;
                (
                    merge_operation_id_from_ref(
                        command.merge_operation_ref,
                        "identity_resolution.unmerge.merge_operation_ref",
                    )?,
                    AggregatePresence::MustExist,
                )
            }
            _ => return Err(unsupported_capability()),
        };
        Ok(AggregateTarget {
            reference: support::record_ref(
                MERGE_OPERATION_RECORD_TYPE,
                operation_id.as_str(),
                "identity_resolution.merge.merge_operation_ref.merge_operation_id",
            )?,
            presence,
        })
    }

    fn plan(
        &self,
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
        current: Option<&RecordSnapshot>,
    ) -> Result<CapabilityBatchExecutionPlan, SdkError> {
        ensure_definition(definition, request)?;
        match definition.capability_id.as_str() {
            MERGE_CAPABILITY => plan_merge(definition, request, current),
            UNMERGE_CAPABILITY => plan_unmerge(definition, request, current),
            _ => Err(unsupported_capability()),
        }
    }
}

pub fn merge_reference_scope_from_request(
    request: &CapabilityRequest,
) -> Result<MergeReferenceScope, SdkError> {
    let command: wire::MergePartyRequest = support::decode_request_with_data_class(
        request,
        MODULE_ID,
        MERGE_REQUEST_SCHEMA,
        DataClass::Personal,
    )?;
    let source = required_party_expectation(
        command.source_party_ref.as_ref(),
        command.source_party_version,
        "identity_resolution.merge.source_party_ref",
        "identity_resolution.merge.source_party_version",
    )?;
    let survivor = required_party_expectation(
        command.survivor_party_ref.as_ref(),
        command.survivor_party_version,
        "identity_resolution.merge.survivor_party_ref",
        "identity_resolution.merge.survivor_party_version",
    )?;
    let provenance = command
        .survivorship
        .iter()
        .map(|selection| {
            required_party_expectation(
                selection.provenance_party_ref.as_ref(),
                selection.provenance_party_version,
                "identity_resolution.merge.survivorship.provenance_party_ref",
                "identity_resolution.merge.survivorship.provenance_party_version",
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(MergeReferenceScope {
        source,
        survivor,
        provenance,
    })
}

pub fn merge_operation_from_snapshot(
    snapshot: &RecordSnapshot,
) -> Result<MergeOperation, SdkError> {
    let bytes = support::persisted_json_bytes_with_data_class(
        snapshot,
        merge_persisted_contract(),
        DataClass::Personal,
    )?;
    decode_merge_operation_state(bytes)
}

pub fn merge_operation_to_wire(operation: &MergeOperation) -> wire::MergeOperation {
    wire::MergeOperation {
        merge_operation_ref: Some(wire::MergeOperationRef {
            merge_operation_id: operation.operation_id().as_str().to_owned(),
        }),
        source_party_ref: Some(customer::PartyRef {
            party_id: operation.source_party_ref().as_str().to_owned(),
        }),
        source_party_version: operation.source_party_version(),
        survivor_party_ref: Some(customer::PartyRef {
            party_id: operation.survivor_party_ref().as_str().to_owned(),
        }),
        survivor_party_version: operation.survivor_party_version(),
        decision_ref: operation.decision_ref().as_str().to_owned(),
        decided_by_actor_id: operation.decided_by().as_str().to_owned(),
        reason: operation.reason().as_str().to_owned(),
        survivorship: operation
            .survivorship()
            .iter()
            .map(|selection| wire::SurvivorshipSelection {
                field_path: selection.field_path().as_str().to_owned(),
                provenance_party_ref: Some(customer::PartyRef {
                    party_id: selection.provenance_party_ref().as_str().to_owned(),
                }),
                provenance_party_version: selection.provenance_party_version(),
                source_value_sha256: selection.source_value_digest().as_bytes().to_vec(),
                evidence_ref: selection.evidence_ref().as_str().to_owned(),
            })
            .collect(),
        status: match operation.status() {
            MergeOperationStatus::Active => wire::MergeOperationStatus::Active as i32,
            MergeOperationStatus::Unmerged => wire::MergeOperationStatus::Unmerged as i32,
        },
        unmerge_decision: operation
            .unmerge_decision()
            .map(|decision| wire::MergeUnmergeDecision {
                decision_ref: decision.decision_ref().as_str().to_owned(),
                decided_by_actor_id: decision.decided_by().as_str().to_owned(),
                reason: decision.reason().as_str().to_owned(),
                occurred_at: Some(core::UnixTime {
                    unix_nanos: decision.occurred_at_unix_nanos(),
                }),
            }),
        resource_version: Some(customer::CustomerResourceVersion {
            version: operation.version(),
            created_at: Some(core::UnixTime {
                unix_nanos: operation.created_at_unix_nanos(),
            }),
            updated_at: Some(core::UnixTime {
                unix_nanos: operation.updated_at_unix_nanos(),
            }),
        }),
    }
}

pub fn merge_persisted_contract() -> PersistedPayloadContract<'static> {
    PersistedPayloadContract {
        owner: MODULE_ID,
        schema_id: MERGE_OPERATION_STATE_SCHEMA_ID,
        schema_version: MERGE_OPERATION_STATE_SCHEMA_VERSION,
        descriptor_hash: merge_operation_state_descriptor_hash(),
        maximum_size_bytes: MERGE_OPERATION_STATE_MAXIMUM_BYTES,
        retention_policy_id: MERGE_OPERATION_STATE_RETENTION_POLICY_ID,
    }
}

pub fn merge_persisted_payload(
    operation: &MergeOperation,
) -> Result<crm_module_sdk::TypedPayload, SdkError> {
    support::persisted_json_payload_with_data_class(
        merge_persisted_contract(),
        DataClass::Personal,
        encode_merge_operation_state(operation)?,
    )
}

fn plan_merge(
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
    current: Option<&RecordSnapshot>,
) -> Result<CapabilityBatchExecutionPlan, SdkError> {
    if current.is_some() {
        return Err(invalid_plan());
    }
    let command: wire::MergePartyRequest = support::decode_request_with_data_class(
        request,
        MODULE_ID,
        MERGE_REQUEST_SCHEMA,
        DataClass::Personal,
    )?;
    let operation = merge_operation_from_command(command, request)?;
    let aggregate = support::record_ref(
        MERGE_OPERATION_RECORD_TYPE,
        operation.operation_id().as_str(),
        "identity_resolution.merge.merge_operation_ref.merge_operation_id",
    )?;
    let public_operation = merge_operation_to_wire(&operation);
    let output = support::protobuf_payload(
        MODULE_ID,
        MERGE_RESPONSE_SCHEMA,
        DataClass::Personal,
        &wire::MergePartyResponse {
            merge_operation: Some(public_operation.clone()),
        },
    )?;
    let event = support::event_evidence_with_data_class(
        request,
        aggregate.clone(),
        MODULE_ID,
        EventSpec {
            event_type: MERGE_EVENT_TYPE,
            event_schema_id: MERGE_EVENT_SCHEMA,
            aggregate_version: operation.version(),
            previous_version: None,
        },
        DataClass::Personal,
        &wire::PartyMergedEvent {
            merge_operation: Some(public_operation),
        },
    )?;
    mutation_plan(
        definition,
        request,
        aggregate.clone(),
        RecordMutation::Create {
            reference: aggregate,
            payload: merge_persisted_payload(&operation)?,
        },
        event,
        output,
    )
}

fn plan_unmerge(
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
    current: Option<&RecordSnapshot>,
) -> Result<CapabilityBatchExecutionPlan, SdkError> {
    let current = current.ok_or_else(invalid_plan)?;
    let command: wire::UnmergePartyRequest = support::decode_request_with_data_class(
        request,
        MODULE_ID,
        UNMERGE_REQUEST_SCHEMA,
        DataClass::Personal,
    )?;
    let requested_id = merge_operation_id_from_ref(
        command.merge_operation_ref,
        "identity_resolution.unmerge.merge_operation_ref",
    )?;
    if current.reference.record_id.as_str() != requested_id.as_str() {
        return Err(SdkError::invalid_argument(
            "identity_resolution.unmerge.merge_operation_ref",
            "merge operation reference does not match the loaded aggregate",
        ));
    }
    let mut operation = merge_operation_from_snapshot(current)?;
    operation.unmerge(UnmergeMergeOperation {
        expected_version: command.expected_version,
        decision_ref: DecisionReference::try_new(command.decision_ref)?,
        decided_by: request.context.execution.actor_id.clone(),
        reason: LineageDecisionReasonCode::try_new(command.reason)?,
        occurred_at_unix_nanos: request.context.execution.request_started_at_unix_nanos,
    })?;
    let aggregate = current.reference.clone();
    let public_operation = merge_operation_to_wire(&operation);
    let output = support::protobuf_payload(
        MODULE_ID,
        UNMERGE_RESPONSE_SCHEMA,
        DataClass::Personal,
        &wire::UnmergePartyResponse {
            merge_operation: Some(public_operation.clone()),
        },
    )?;
    let event = support::event_evidence_with_data_class(
        request,
        aggregate.clone(),
        MODULE_ID,
        EventSpec {
            event_type: UNMERGE_EVENT_TYPE,
            event_schema_id: UNMERGE_EVENT_SCHEMA,
            aggregate_version: operation.version(),
            previous_version: Some(current.version),
        },
        DataClass::Personal,
        &wire::PartyUnmergedEvent {
            merge_operation: Some(public_operation),
        },
    )?;
    mutation_plan(
        definition,
        request,
        aggregate.clone(),
        RecordMutation::Update {
            reference: aggregate,
            expected_version: current.version,
            payload: merge_persisted_payload(&operation)?,
        },
        event,
        output,
    )
}

fn merge_operation_from_command(
    command: wire::MergePartyRequest,
    request: &CapabilityRequest,
) -> Result<MergeOperation, SdkError> {
    let operation_id = merge_operation_id_from_ref(
        command.merge_operation_ref,
        "identity_resolution.merge.merge_operation_ref",
    )?;
    let source_party_ref = required_party_ref(
        command.source_party_ref.as_ref(),
        "identity_resolution.merge.source_party_ref",
    )?;
    let survivor_party_ref = required_party_ref(
        command.survivor_party_ref.as_ref(),
        "identity_resolution.merge.survivor_party_ref",
    )?;
    let survivorship = command
        .survivorship
        .into_iter()
        .map(survivorship_from_wire)
        .collect::<Result<Vec<_>, _>>()?;
    MergeOperation::create(CreateMergeOperation {
        operation_id,
        source_party_ref,
        source_party_version: command.source_party_version,
        survivor_party_ref,
        survivor_party_version: command.survivor_party_version,
        decision_ref: DecisionReference::try_new(command.decision_ref)?,
        decided_by: request.context.execution.actor_id.clone(),
        reason: LineageDecisionReasonCode::try_new(command.reason)?,
        survivorship,
        occurred_at_unix_nanos: request.context.execution.request_started_at_unix_nanos,
    })
}

fn survivorship_from_wire(
    selection: wire::SurvivorshipSelection,
) -> Result<SurvivorshipSelection, SdkError> {
    let digest: [u8; 32] = selection.source_value_sha256.try_into().map_err(|_| {
        SdkError::invalid_argument(
            "identity_resolution.merge.survivorship.source_value_sha256",
            "source value digest must be exactly 32 bytes",
        )
    })?;
    SurvivorshipSelection::try_new(
        FieldPath::try_new(selection.field_path)?,
        required_party_ref(
            selection.provenance_party_ref.as_ref(),
            "identity_resolution.merge.survivorship.provenance_party_ref",
        )?,
        selection.provenance_party_version,
        SourceValueDigest::from_bytes(digest),
        crm_identity_resolution::EvidenceReference::try_new(selection.evidence_ref)?,
    )
}

fn required_party_expectation(
    value: Option<&customer::PartyRef>,
    expected_version: i64,
    party_field: &'static str,
    version_field: &'static str,
) -> Result<MergePartyVersionExpectation, SdkError> {
    if expected_version <= 0 {
        return Err(SdkError::invalid_argument(
            version_field,
            "Party version must be positive",
        ));
    }
    Ok(MergePartyVersionExpectation {
        party_ref: required_party_ref(value, party_field)?,
        expected_version,
    })
}

fn required_party_ref(
    value: Option<&customer::PartyRef>,
    field: &'static str,
) -> Result<PartyReference, SdkError> {
    let value =
        value.ok_or_else(|| SdkError::invalid_argument(field, "Party reference is required"))?;
    PartyReference::try_new(value.party_id.clone())
}

fn merge_operation_id_from_ref(
    value: Option<wire::MergeOperationRef>,
    field: &'static str,
) -> Result<MergeOperationId, SdkError> {
    let value = value.ok_or_else(|| {
        SdkError::invalid_argument(field, "merge operation reference is required")
    })?;
    MergeOperationId::try_new(value.merge_operation_id)
}

fn ensure_definition(
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
) -> Result<(), SdkError> {
    if !matches!(
        definition.capability_id.as_str(),
        MERGE_CAPABILITY | UNMERGE_CAPABILITY
    ) || definition.capability_id != request.context.execution.capability_id
        || definition.capability_version != request.context.execution.capability_version
    {
        return Err(unsupported_capability());
    }
    Ok(())
}

fn mutation_plan(
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
    aggregate: crm_module_sdk::RecordRef,
    mutation: RecordMutation,
    event: crm_core_data::EventEvidence,
    output: crm_module_sdk::TypedPayload,
) -> Result<CapabilityBatchExecutionPlan, SdkError> {
    let audit = support::audit_intent(
        request,
        &aggregate,
        event.aggregate_version,
        definition.capability_id.as_str(),
        &output.bytes,
    )?;
    Ok(CapabilityBatchExecutionPlan {
        batch: BatchMutationPlan {
            context: request.context.clone(),
            records: vec![mutation],
            relationships: Vec::new(),
            events: vec![event],
            idempotency: support::capability_idempotency(definition, request)?,
            audits: vec![audit],
        },
        output: Some(output),
    })
}

fn invalid_plan() -> SdkError {
    SdkError::new(
        "IDENTITY_RESOLUTION_MERGE_PLAN_INVALID",
        crm_module_sdk::ErrorCategory::Internal,
        false,
        "The Identity Resolution merge operation could not be planned safely.",
    )
}

fn unsupported_capability() -> SdkError {
    SdkError::new(
        "IDENTITY_RESOLUTION_MERGE_CAPABILITY_UNSUPPORTED",
        crm_module_sdk::ErrorCategory::Internal,
        false,
        "The Identity Resolution merge capability is unsupported.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_persisted_contract_is_personal_json_and_stably_hashed() {
        let payload = support::persisted_json_payload_with_data_class(
            merge_persisted_contract(),
            DataClass::Personal,
            encode_merge_operation_state(
                &MergeOperation::create(CreateMergeOperation {
                    operation_id: MergeOperationId::try_new("merge-op-contract-test").unwrap(),
                    source_party_ref: PartyReference::try_new("party-a").unwrap(),
                    source_party_version: 1,
                    survivor_party_ref: PartyReference::try_new("party-b").unwrap(),
                    survivor_party_version: 2,
                    decision_ref: DecisionReference::try_new("approval://merge/test").unwrap(),
                    decided_by: crm_module_sdk::ActorId::try_new("reviewer-a").unwrap(),
                    reason: LineageDecisionReasonCode::try_new("duplicate.confirmed").unwrap(),
                    survivorship: Vec::new(),
                    occurred_at_unix_nanos: 100,
                })
                .unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(payload.schema_id.as_str(), MERGE_OPERATION_STATE_SCHEMA_ID);
        assert_eq!(payload.data_class, DataClass::Personal);
        assert_ne!(payload.descriptor_hash, [0; 32]);
    }
}
