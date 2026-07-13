use crate::{
    CREATE_CAPABILITY, CREATE_REQUEST_SCHEMA, CREATE_RESPONSE_SCHEMA, CREATED_EVENT_SCHEMA,
    CREATED_EVENT_TYPE, MODULE_ID, MUTATION_CAPABILITY_IDS, RECORD_TYPE, UPDATE_CAPABILITY,
    UPDATE_REQUEST_SCHEMA, UPDATE_RESPONSE_SCHEMA, UPDATED_EVENT_SCHEMA, UPDATED_EVENT_TYPE,
};
use crm_capability_plan_support::{self as support, EventSpec, PersistedPayloadContract};
use crm_capability_runtime::{CapabilityDefinition, CapabilityRequest};
use crm_core_data::{
    AggregatePresence, AggregateTarget, BatchMutationPlan, CapabilityBatchExecutionPlan,
    RecordMutation, TransactionalAggregatePlanner,
};
use crm_module_sdk::{DataClass, RecordSnapshot, SdkError};
use crm_party_relationships::{
    CreatePartyRelationship, PARTY_RELATIONSHIP_STATE_MAXIMUM_BYTES,
    PARTY_RELATIONSHIP_STATE_RETENTION_POLICY_ID, PARTY_RELATIONSHIP_STATE_SCHEMA_ID,
    PARTY_RELATIONSHIP_STATE_SCHEMA_VERSION, PartyReference, PartyRelationship,
    PartyRelationshipId, PartyRelationshipStatus, RelationshipDirectionality, RelationshipType,
    UpdatePartyRelationship, decode_party_relationship_state, encode_party_relationship_state,
    party_relationship_state_descriptor_hash,
};
use crm_proto_contracts::crm::{
    core::v1 as core, customer::v1 as customer, party_relationships::v1 as wire,
};

#[derive(Debug, Default, Clone, Copy)]
pub struct PartyRelationshipCapabilityPlanner;

impl TransactionalAggregatePlanner for PartyRelationshipCapabilityPlanner {
    fn target(
        &self,
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
    ) -> Result<AggregateTarget, SdkError> {
        ensure_definition(definition, request)?;
        let (party_relationship_id, presence) = match definition.capability_id.as_str() {
            CREATE_CAPABILITY => {
                let command: wire::CreatePartyRelationshipRequest =
                    support::decode_request_with_data_class(
                        request,
                        MODULE_ID,
                        CREATE_REQUEST_SCHEMA,
                        DataClass::Personal,
                    )?;
                (
                    party_relationship_id_from_ref(
                        command.party_relationship_ref,
                        "party_relationship.party_relationship_ref",
                    )?,
                    AggregatePresence::MustBeAbsent,
                )
            }
            UPDATE_CAPABILITY => {
                let command: wire::UpdatePartyRelationshipRequest =
                    support::decode_request_with_data_class(
                        request,
                        MODULE_ID,
                        UPDATE_REQUEST_SCHEMA,
                        DataClass::Personal,
                    )?;
                (
                    party_relationship_id_from_ref(
                        command.party_relationship_ref,
                        "party_relationship.party_relationship_ref",
                    )?,
                    AggregatePresence::MustExist,
                )
            }
            _ => return Err(unsupported_capability()),
        };

        Ok(AggregateTarget {
            reference: support::record_ref(
                RECORD_TYPE,
                party_relationship_id.as_str(),
                "party_relationship.party_relationship_ref.party_relationship_id",
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
            CREATE_CAPABILITY => plan_create(definition, request, current),
            UPDATE_CAPABILITY => plan_update(definition, request, current),
            _ => Err(unsupported_capability()),
        }
    }
}

fn plan_create(
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
    current: Option<&RecordSnapshot>,
) -> Result<CapabilityBatchExecutionPlan, SdkError> {
    if current.is_some() {
        return Err(invalid_plan());
    }

    let command: wire::CreatePartyRelationshipRequest = support::decode_request_with_data_class(
        request,
        MODULE_ID,
        CREATE_REQUEST_SCHEMA,
        DataClass::Personal,
    )?;
    let party_relationship = PartyRelationship::create(CreatePartyRelationship {
        party_relationship_id: party_relationship_id_from_ref(
            command.party_relationship_ref,
            "party_relationship.party_relationship_ref",
        )?,
        from_party_ref: party_reference_from_ref(
            command.from_party_ref,
            "party_relationship.from_party_ref",
        )?,
        to_party_ref: party_reference_from_ref(
            command.to_party_ref,
            "party_relationship.to_party_ref",
        )?,
        relationship_type: relationship_type_from_wire(command.relationship_type)?,
        valid_from_unix_nanos: optional_time(command.valid_from),
        valid_until_unix_nanos: optional_time(command.valid_until),
        occurred_at_unix_nanos: request.context.execution.request_started_at_unix_nanos,
    })?;

    let aggregate = support::record_ref(
        RECORD_TYPE,
        party_relationship.party_relationship_id().as_str(),
        "party_relationship.party_relationship_ref.party_relationship_id",
    )?;
    let public_relationship = party_relationship_to_wire(&party_relationship);
    let output = support::protobuf_payload(
        MODULE_ID,
        CREATE_RESPONSE_SCHEMA,
        DataClass::Personal,
        &wire::CreatePartyRelationshipResponse {
            party_relationship: Some(public_relationship.clone()),
        },
    )?;
    let event = support::event_evidence_with_data_class(
        request,
        aggregate.clone(),
        MODULE_ID,
        EventSpec {
            event_type: CREATED_EVENT_TYPE,
            event_schema_id: CREATED_EVENT_SCHEMA,
            aggregate_version: party_relationship.version(),
            previous_version: None,
        },
        DataClass::Personal,
        &wire::PartyRelationshipCreatedEvent {
            party_relationship: Some(public_relationship),
        },
    )?;

    mutation_plan(
        definition,
        request,
        aggregate.clone(),
        RecordMutation::Create {
            reference: aggregate,
            payload: persisted_payload(&party_relationship)?,
        },
        event,
        output,
    )
}

fn plan_update(
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
    current: Option<&RecordSnapshot>,
) -> Result<CapabilityBatchExecutionPlan, SdkError> {
    let current = current.ok_or_else(invalid_plan)?;
    let command: wire::UpdatePartyRelationshipRequest = support::decode_request_with_data_class(
        request,
        MODULE_ID,
        UPDATE_REQUEST_SCHEMA,
        DataClass::Personal,
    )?;
    let requested_id = party_relationship_id_from_ref(
        command.party_relationship_ref,
        "party_relationship.party_relationship_ref",
    )?;
    if requested_id.as_str() != current.reference.record_id.as_str() {
        return Err(invalid_plan());
    }

    let mut party_relationship = party_relationship_from_snapshot(current)?;
    party_relationship.apply_update(UpdatePartyRelationship {
        expected_version: command.expected_version,
        status: status_from_wire(command.status)?,
        valid_from_unix_nanos: optional_time(command.valid_from),
        valid_until_unix_nanos: optional_time(command.valid_until),
        occurred_at_unix_nanos: request.context.execution.request_started_at_unix_nanos,
    })?;

    let aggregate = current.reference.clone();
    let public_relationship = party_relationship_to_wire(&party_relationship);
    let output = support::protobuf_payload(
        MODULE_ID,
        UPDATE_RESPONSE_SCHEMA,
        DataClass::Personal,
        &wire::UpdatePartyRelationshipResponse {
            party_relationship: Some(public_relationship.clone()),
        },
    )?;
    let event = support::event_evidence_with_data_class(
        request,
        aggregate.clone(),
        MODULE_ID,
        EventSpec {
            event_type: UPDATED_EVENT_TYPE,
            event_schema_id: UPDATED_EVENT_SCHEMA,
            aggregate_version: party_relationship.version(),
            previous_version: Some(current.version),
        },
        DataClass::Personal,
        &wire::PartyRelationshipUpdatedEvent {
            party_relationship: Some(public_relationship),
        },
    )?;

    mutation_plan(
        definition,
        request,
        aggregate.clone(),
        RecordMutation::Update {
            reference: aggregate,
            expected_version: current.version,
            payload: persisted_payload(&party_relationship)?,
        },
        event,
        output,
    )
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

pub fn party_relationship_to_wire(
    party_relationship: &PartyRelationship,
) -> wire::PartyRelationship {
    wire::PartyRelationship {
        party_relationship_ref: Some(customer::PartyRelationshipRef {
            party_relationship_id: party_relationship
                .party_relationship_id()
                .as_str()
                .to_owned(),
        }),
        from_party_ref: Some(customer::PartyRef {
            party_id: party_relationship.from_party_ref().as_str().to_owned(),
        }),
        to_party_ref: Some(customer::PartyRef {
            party_id: party_relationship.to_party_ref().as_str().to_owned(),
        }),
        relationship_type: Some(relationship_type_to_wire(
            party_relationship.relationship_type(),
        )),
        status: match party_relationship.status() {
            PartyRelationshipStatus::Active => wire::PartyRelationshipStatus::Active as i32,
            PartyRelationshipStatus::Inactive => wire::PartyRelationshipStatus::Inactive as i32,
        },
        valid_from: party_relationship
            .valid_from_unix_nanos()
            .map(|unix_nanos| core::UnixTime { unix_nanos }),
        valid_until: party_relationship
            .valid_until_unix_nanos()
            .map(|unix_nanos| core::UnixTime { unix_nanos }),
        resource_version: Some(customer::CustomerResourceVersion {
            version: party_relationship.version(),
            created_at: Some(core::UnixTime {
                unix_nanos: party_relationship.created_at_unix_nanos(),
            }),
            updated_at: Some(core::UnixTime {
                unix_nanos: party_relationship.updated_at_unix_nanos(),
            }),
        }),
    }
}

pub fn persisted_contract() -> PersistedPayloadContract<'static> {
    PersistedPayloadContract {
        owner: MODULE_ID,
        schema_id: PARTY_RELATIONSHIP_STATE_SCHEMA_ID,
        schema_version: PARTY_RELATIONSHIP_STATE_SCHEMA_VERSION,
        descriptor_hash: party_relationship_state_descriptor_hash(),
        maximum_size_bytes: PARTY_RELATIONSHIP_STATE_MAXIMUM_BYTES,
        retention_policy_id: PARTY_RELATIONSHIP_STATE_RETENTION_POLICY_ID,
    }
}

pub fn persisted_payload(
    party_relationship: &PartyRelationship,
) -> Result<crm_module_sdk::TypedPayload, SdkError> {
    support::persisted_json_payload_with_data_class(
        persisted_contract(),
        DataClass::Personal,
        encode_party_relationship_state(party_relationship)?,
    )
}

pub fn party_relationship_from_snapshot(
    snapshot: &RecordSnapshot,
) -> Result<PartyRelationship, SdkError> {
    let party_relationship =
        decode_party_relationship_state(support::persisted_json_bytes_with_data_class(
            snapshot,
            persisted_contract(),
            DataClass::Personal,
        )?)?;
    if party_relationship.party_relationship_id().as_str() != snapshot.reference.record_id.as_str()
        || party_relationship.version() != snapshot.version
    {
        return Err(support::stored_data_error(
            "PARTY_RELATIONSHIPS_PERSISTED_RELATIONSHIP_IDENTITY_INVALID",
        ));
    }
    Ok(party_relationship)
}

pub fn referenced_party_ids_from_create(
    request: &CapabilityRequest,
) -> Result<Vec<PartyReference>, SdkError> {
    let command: wire::CreatePartyRelationshipRequest = support::decode_request_with_data_class(
        request,
        MODULE_ID,
        CREATE_REQUEST_SCHEMA,
        DataClass::Personal,
    )?;
    Ok(vec![
        party_reference_from_ref(command.from_party_ref, "party_relationship.from_party_ref")?,
        party_reference_from_ref(command.to_party_ref, "party_relationship.to_party_ref")?,
    ])
}

fn relationship_type_to_wire(value: &RelationshipType) -> wire::PartyRelationshipType {
    wire::PartyRelationshipType {
        code: value.code().to_owned(),
        directionality: match value.directionality() {
            RelationshipDirectionality::Directional => {
                wire::PartyRelationshipDirectionality::Directional as i32
            }
            RelationshipDirectionality::Reciprocal => {
                wire::PartyRelationshipDirectionality::Reciprocal as i32
            }
        },
        from_role: value.from_role().to_owned(),
        to_role: value.to_role().to_owned(),
    }
}

fn relationship_type_from_wire(
    value: Option<wire::PartyRelationshipType>,
) -> Result<RelationshipType, SdkError> {
    let value = value.ok_or_else(|| {
        SdkError::invalid_argument(
            "party_relationship.relationship_type",
            "Party Relationship type is required",
        )
    })?;
    RelationshipType::try_new(
        value.code,
        directionality_from_wire(value.directionality)?,
        value.from_role,
        value.to_role,
    )
}

fn directionality_from_wire(value: i32) -> Result<RelationshipDirectionality, SdkError> {
    match wire::PartyRelationshipDirectionality::try_from(value) {
        Ok(wire::PartyRelationshipDirectionality::Directional) => {
            Ok(RelationshipDirectionality::Directional)
        }
        Ok(wire::PartyRelationshipDirectionality::Reciprocal) => {
            Ok(RelationshipDirectionality::Reciprocal)
        }
        Ok(wire::PartyRelationshipDirectionality::Unspecified) | Err(_) => {
            Err(SdkError::invalid_argument(
                "party_relationship.relationship_type.directionality",
                "Party Relationship directionality must be DIRECTIONAL or RECIPROCAL",
            ))
        }
    }
}

fn status_from_wire(value: i32) -> Result<PartyRelationshipStatus, SdkError> {
    match wire::PartyRelationshipStatus::try_from(value) {
        Ok(wire::PartyRelationshipStatus::Active) => Ok(PartyRelationshipStatus::Active),
        Ok(wire::PartyRelationshipStatus::Inactive) => Ok(PartyRelationshipStatus::Inactive),
        Ok(wire::PartyRelationshipStatus::Unspecified) | Err(_) => Err(SdkError::invalid_argument(
            "party_relationship.status",
            "Party Relationship status must be ACTIVE or INACTIVE",
        )),
    }
}

fn party_relationship_id_from_ref(
    party_relationship_ref: Option<customer::PartyRelationshipRef>,
    field: &'static str,
) -> Result<PartyRelationshipId, SdkError> {
    let party_relationship_ref = party_relationship_ref.ok_or_else(|| {
        SdkError::invalid_argument(field, "Party Relationship reference is required")
    })?;
    PartyRelationshipId::try_new(party_relationship_ref.party_relationship_id)
}

fn party_reference_from_ref(
    party_ref: Option<customer::PartyRef>,
    field: &'static str,
) -> Result<PartyReference, SdkError> {
    let party_ref = party_ref
        .ok_or_else(|| SdkError::invalid_argument(field, "Party reference is required"))?;
    PartyReference::try_new(party_ref.party_id)
}

fn optional_time(value: Option<core::UnixTime>) -> Option<i64> {
    value.map(|value| value.unix_nanos)
}

fn ensure_definition(
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
) -> Result<(), SdkError> {
    if !MUTATION_CAPABILITY_IDS.contains(&definition.capability_id.as_str())
        || definition.owner_module_id.as_str() != MODULE_ID
        || definition.capability_id.as_str() != request.context.execution.capability_id.as_str()
    {
        return Err(invalid_plan());
    }
    Ok(())
}

fn invalid_plan() -> SdkError {
    SdkError::new(
        "PARTY_RELATIONSHIPS_CAPABILITY_PLAN_INVALID",
        crm_module_sdk::ErrorCategory::Internal,
        false,
        "The Party Relationship capability could not be planned safely.",
    )
}

fn unsupported_capability() -> SdkError {
    SdkError::new(
        "PARTY_RELATIONSHIPS_CAPABILITY_UNSUPPORTED",
        crm_module_sdk::ErrorCategory::Internal,
        false,
        "The Party Relationship capability is not configured.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wire_mapping_preserves_endpoints_semantics_and_version_metadata() {
        let value = PartyRelationship::create(CreatePartyRelationship {
            party_relationship_id: PartyRelationshipId::try_new("relationship-wire-1").unwrap(),
            from_party_ref: PartyReference::try_new("party-acme").unwrap(),
            to_party_ref: PartyReference::try_new("party-ada").unwrap(),
            relationship_type: RelationshipType::employment(),
            valid_from_unix_nanos: Some(10),
            valid_until_unix_nanos: Some(1_000),
            occurred_at_unix_nanos: 10,
        })
        .unwrap();

        let wire = party_relationship_to_wire(&value);
        assert_eq!(
            wire.party_relationship_ref.unwrap().party_relationship_id,
            "relationship-wire-1"
        );
        assert_eq!(wire.from_party_ref.unwrap().party_id, "party-acme");
        assert_eq!(wire.to_party_ref.unwrap().party_id, "party-ada");
        let relationship_type = wire.relationship_type.unwrap();
        assert_eq!(relationship_type.code, "employment");
        assert_eq!(
            relationship_type.directionality,
            wire::PartyRelationshipDirectionality::Directional as i32
        );
        assert_eq!(wire.resource_version.unwrap().version, 1);
    }
}
