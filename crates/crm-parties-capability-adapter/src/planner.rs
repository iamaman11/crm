use crm_capability_plan_support::{self as support, EventSpec, PersistedPayloadContract};
use crm_capability_runtime::{CapabilityDefinition, CapabilityRequest, CapabilityRisk};
use crm_core_data::{
    AggregatePresence, AggregateTarget, BatchMutationPlan, CapabilityBatchExecutionPlan,
    RecordMutation, TransactionalAggregatePlanner,
};
use crm_module_sdk::{
    CapabilityId, CapabilityVersion, DataClass, ModuleId, RecordSnapshot, SdkError,
};
use crm_parties::{
    CreateParty, PARTY_STATE_MAXIMUM_BYTES, PARTY_STATE_RETENTION_POLICY_ID, PARTY_STATE_SCHEMA_ID,
    PARTY_STATE_SCHEMA_VERSION, Party, PartyId, PartyKind, UpdateParty, decode_party_state,
    encode_party_state, party_state_descriptor_hash,
};
use crm_proto_contracts::crm::{core::v1 as core, customer::v1 as customer, parties::v1 as wire};

pub const MODULE_ID: &str = "crm.parties";
pub const RECORD_TYPE: &str = "parties.party";
pub const CREATE_CAPABILITY: &str = "parties.party.create";
pub const UPDATE_CAPABILITY: &str = "parties.party.update";

pub const CREATE_REQUEST_SCHEMA: &str = "crm.parties.v1.CreatePartyRequest";
pub const CREATE_RESPONSE_SCHEMA: &str = "crm.parties.v1.CreatePartyResponse";
pub const UPDATE_REQUEST_SCHEMA: &str = "crm.parties.v1.UpdatePartyRequest";
pub const UPDATE_RESPONSE_SCHEMA: &str = "crm.parties.v1.UpdatePartyResponse";
pub const CREATED_EVENT_SCHEMA: &str = "crm.parties.v1.PartyCreatedEvent";
pub const UPDATED_EVENT_SCHEMA: &str = "crm.parties.v1.PartyUpdatedEvent";

pub const PARTY_MUTATION_CAPABILITY_IDS: [&str; 2] = [CREATE_CAPABILITY, UPDATE_CAPABILITY];

#[derive(Debug, Default, Clone, Copy)]
pub struct PartyCapabilityPlanner;

pub fn capability_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {
    PARTY_MUTATION_CAPABILITY_IDS
        .iter()
        .map(|capability_id| capability_definition(capability_id))
        .collect()
}

pub fn capability_definition(capability_id: &str) -> Result<CapabilityDefinition, SdkError> {
    let (input_schema, output_schema) = match capability_id {
        CREATE_CAPABILITY => (CREATE_REQUEST_SCHEMA, CREATE_RESPONSE_SCHEMA),
        UPDATE_CAPABILITY => (UPDATE_REQUEST_SCHEMA, UPDATE_RESPONSE_SCHEMA),
        _ => return Err(unsupported_capability()),
    };

    Ok(CapabilityDefinition {
        capability_id: support::configured_identifier(CapabilityId::try_new(capability_id))?,
        capability_version: support::configured_identifier(CapabilityVersion::try_new(
            support::CONTRACT_VERSION,
        ))?,
        owner_module_id: support::configured_identifier(ModuleId::try_new(MODULE_ID))?,
        input_contract: support::protobuf_contract(
            MODULE_ID,
            input_schema,
            vec![DataClass::Personal],
        )?,
        output_contract: Some(support::protobuf_contract(
            MODULE_ID,
            output_schema,
            vec![DataClass::Personal],
        )?),
        risk: CapabilityRisk::Medium,
        mutation: true,
        requires_idempotency: true,
        requires_approval: false,
        authorization_policy_id: capability_id.to_owned(),
        rate_limit_policy_id: None,
    })
}

impl TransactionalAggregatePlanner for PartyCapabilityPlanner {
    fn target(
        &self,
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
    ) -> Result<AggregateTarget, SdkError> {
        ensure_definition(definition, request)?;
        let (party_id, presence) = match definition.capability_id.as_str() {
            CREATE_CAPABILITY => {
                let command: wire::CreatePartyRequest = support::decode_request_with_data_class(
                    request,
                    MODULE_ID,
                    CREATE_REQUEST_SCHEMA,
                    DataClass::Personal,
                )?;
                (
                    party_id_from_ref(command.party_ref, "party.party_ref")?,
                    AggregatePresence::MustBeAbsent,
                )
            }
            UPDATE_CAPABILITY => {
                let command: wire::UpdatePartyRequest = support::decode_request_with_data_class(
                    request,
                    MODULE_ID,
                    UPDATE_REQUEST_SCHEMA,
                    DataClass::Personal,
                )?;
                (
                    party_id_from_ref(command.party_ref, "party.party_ref")?,
                    AggregatePresence::MustExist,
                )
            }
            _ => return Err(unsupported_capability()),
        };

        Ok(AggregateTarget {
            reference: support::record_ref(
                RECORD_TYPE,
                party_id.as_str(),
                "party.party_ref.party_id",
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

    let command: wire::CreatePartyRequest = support::decode_request_with_data_class(
        request,
        MODULE_ID,
        CREATE_REQUEST_SCHEMA,
        DataClass::Personal,
    )?;
    let party = Party::create(CreateParty {
        party_id: party_id_from_ref(command.party_ref, "party.party_ref")?,
        kind: party_kind_from_wire(command.kind)?,
        display_name: command.display_name,
        occurred_at_unix_nanos: request.context.execution.request_started_at_unix_nanos,
    })?;

    let aggregate = support::record_ref(
        RECORD_TYPE,
        party.party_id().as_str(),
        "party.party_ref.party_id",
    )?;
    let public_party = party_to_wire(&party);
    let output = support::protobuf_payload(
        MODULE_ID,
        CREATE_RESPONSE_SCHEMA,
        DataClass::Personal,
        &wire::CreatePartyResponse {
            party: Some(public_party.clone()),
        },
    )?;
    let event = support::event_evidence_with_data_class(
        request,
        aggregate.clone(),
        MODULE_ID,
        EventSpec {
            event_type: "parties.party.created",
            event_schema_id: CREATED_EVENT_SCHEMA,
            aggregate_version: party.version(),
            previous_version: None,
        },
        DataClass::Personal,
        &wire::PartyCreatedEvent {
            party: Some(public_party),
        },
    )?;

    mutation_plan(
        definition,
        request,
        aggregate.clone(),
        RecordMutation::Create {
            reference: aggregate,
            payload: persisted_payload(&party)?,
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
    let command: wire::UpdatePartyRequest = support::decode_request_with_data_class(
        request,
        MODULE_ID,
        UPDATE_REQUEST_SCHEMA,
        DataClass::Personal,
    )?;
    let requested_party_id = party_id_from_ref(command.party_ref, "party.party_ref")?;
    if requested_party_id.as_str() != current.reference.record_id.as_str() {
        return Err(invalid_plan());
    }

    let mut party = party_from_snapshot(current)?;
    party.apply_update(UpdateParty {
        expected_version: command.expected_version,
        display_name: command.display_name,
        occurred_at_unix_nanos: request.context.execution.request_started_at_unix_nanos,
    })?;

    let aggregate = current.reference.clone();
    let public_party = party_to_wire(&party);
    let output = support::protobuf_payload(
        MODULE_ID,
        UPDATE_RESPONSE_SCHEMA,
        DataClass::Personal,
        &wire::UpdatePartyResponse {
            party: Some(public_party.clone()),
        },
    )?;
    let event = support::event_evidence_with_data_class(
        request,
        aggregate.clone(),
        MODULE_ID,
        EventSpec {
            event_type: "parties.party.updated",
            event_schema_id: UPDATED_EVENT_SCHEMA,
            aggregate_version: party.version(),
            previous_version: Some(current.version),
        },
        DataClass::Personal,
        &wire::PartyUpdatedEvent {
            party: Some(public_party),
            changed_fields: vec!["display_name".to_owned()],
        },
    )?;

    mutation_plan(
        definition,
        request,
        aggregate.clone(),
        RecordMutation::Update {
            reference: aggregate,
            expected_version: current.version,
            payload: persisted_payload(&party)?,
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

pub fn party_to_wire(party: &Party) -> wire::Party {
    wire::Party {
        party_ref: Some(customer::PartyRef {
            party_id: party.party_id().as_str().to_owned(),
        }),
        kind: match party.kind() {
            PartyKind::Person => wire::PartyKind::Person as i32,
            PartyKind::Organization => wire::PartyKind::Organization as i32,
        },
        display_name: party.display_name().to_owned(),
        resource_version: Some(customer::CustomerResourceVersion {
            version: party.version(),
            created_at: Some(core::UnixTime {
                unix_nanos: party.created_at_unix_nanos(),
            }),
            updated_at: Some(core::UnixTime {
                unix_nanos: party.updated_at_unix_nanos(),
            }),
        }),
    }
}

pub fn persisted_contract() -> PersistedPayloadContract<'static> {
    PersistedPayloadContract {
        owner: MODULE_ID,
        schema_id: PARTY_STATE_SCHEMA_ID,
        schema_version: PARTY_STATE_SCHEMA_VERSION,
        descriptor_hash: party_state_descriptor_hash(),
        maximum_size_bytes: PARTY_STATE_MAXIMUM_BYTES,
        retention_policy_id: PARTY_STATE_RETENTION_POLICY_ID,
    }
}

pub fn persisted_payload(party: &Party) -> Result<crm_module_sdk::TypedPayload, SdkError> {
    support::persisted_json_payload_with_data_class(
        persisted_contract(),
        DataClass::Personal,
        encode_party_state(party)?,
    )
}

pub fn party_from_snapshot(snapshot: &RecordSnapshot) -> Result<Party, SdkError> {
    let v1_contract = persisted_contract();
    let v2_contract = crate::merge_planner::party_v2_persisted_contract();
    let contract = if snapshot.payload.schema_version == v1_contract.schema_version
        && snapshot.payload.descriptor_hash == v1_contract.descriptor_hash
    {
        v1_contract
    } else if snapshot.payload.schema_version == v2_contract.schema_version
        && snapshot.payload.descriptor_hash == v2_contract.descriptor_hash
    {
        v2_contract
    } else {
        return Err(support::stored_data_error(
            "PARTIES_PERSISTED_PARTY_CONTRACT_INVALID",
        ));
    };
    let party = decode_party_state(support::persisted_json_bytes_with_data_class(
        snapshot,
        contract,
        DataClass::Personal,
    )?)?;
    if party.party_id().as_str() != snapshot.reference.record_id.as_str()
        || party.version() != snapshot.version
    {
        return Err(support::stored_data_error(
            "PARTIES_PERSISTED_PARTY_IDENTITY_INVALID",
        ));
    }
    Ok(party)
}

fn party_id_from_ref(
    party_ref: Option<customer::PartyRef>,
    field: &'static str,
) -> Result<PartyId, SdkError> {
    let party_ref = party_ref
        .ok_or_else(|| SdkError::invalid_argument(field, "Party reference is required"))?;
    PartyId::try_new(party_ref.party_id)
}

fn party_kind_from_wire(value: i32) -> Result<PartyKind, SdkError> {
    match wire::PartyKind::try_from(value) {
        Ok(wire::PartyKind::Person) => Ok(PartyKind::Person),
        Ok(wire::PartyKind::Organization) => Ok(PartyKind::Organization),
        Ok(wire::PartyKind::Unspecified) | Err(_) => Err(SdkError::invalid_argument(
            "party.kind",
            "Party kind must be PERSON or ORGANIZATION",
        )),
    }
}

fn ensure_definition(
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
) -> Result<(), SdkError> {
    if !PARTY_MUTATION_CAPABILITY_IDS.contains(&definition.capability_id.as_str())
        || definition.owner_module_id.as_str() != MODULE_ID
        || definition.capability_id.as_str() != request.context.execution.capability_id.as_str()
    {
        return Err(invalid_plan());
    }
    Ok(())
}

fn invalid_plan() -> SdkError {
    SdkError::new(
        "PARTIES_CAPABILITY_PLAN_INVALID",
        crm_module_sdk::ErrorCategory::Internal,
        false,
        "The Party capability could not be planned safely.",
    )
}

fn unsupported_capability() -> SdkError {
    SdkError::new(
        "PARTIES_CAPABILITY_UNSUPPORTED",
        crm_module_sdk::ErrorCategory::Internal,
        false,
        "The Party capability is not configured.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn publishes_create_and_update_as_personal_idempotent_mutations() {
        let definitions = capability_definitions().unwrap();
        assert_eq!(definitions.len(), 2);
        assert_eq!(
            definitions
                .iter()
                .map(|definition| definition.capability_id.as_str())
                .collect::<Vec<_>>(),
            PARTY_MUTATION_CAPABILITY_IDS
        );
        assert!(definitions.iter().all(|definition| definition.mutation));
        assert!(
            definitions
                .iter()
                .all(|definition| definition.requires_idempotency)
        );
        assert!(definitions.iter().all(|definition| {
            definition.input_contract.allowed_data_classes == vec![DataClass::Personal]
        }));
    }

    #[test]
    fn wire_mapping_preserves_typed_kind_and_version_metadata() {
        let party = Party::create(CreateParty {
            party_id: PartyId::try_new("party-wire-1").unwrap(),
            kind: PartyKind::Organization,
            display_name: "Northwind Holdings".to_owned(),
            occurred_at_unix_nanos: 42,
        })
        .unwrap();

        let wire = party_to_wire(&party);
        assert_eq!(wire.party_ref.unwrap().party_id, "party-wire-1");
        assert_eq!(
            wire.kind,
            crm_proto_contracts::crm::parties::v1::PartyKind::Organization as i32
        );
        assert_eq!(wire.resource_version.unwrap().version, 1);
    }
}
