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
    PARTY_STATE_SCHEMA_VERSION, Party, PartyId, PartyKind, encode_party_state,
    party_state_descriptor_hash,
};
use crm_proto_contracts::crm::{core::v1 as core, customer::v1 as customer, parties::v1 as wire};

pub const MODULE_ID: &str = "crm.parties";
pub const RECORD_TYPE: &str = "parties.party";
pub const CREATE_CAPABILITY: &str = "parties.party.create";

pub const CREATE_REQUEST_SCHEMA: &str = "crm.parties.v1.CreatePartyRequest";
pub const CREATE_RESPONSE_SCHEMA: &str = "crm.parties.v1.CreatePartyResponse";
pub const CREATED_EVENT_SCHEMA: &str = "crm.parties.v1.PartyCreatedEvent";

#[derive(Debug, Default, Clone, Copy)]
pub struct PartyCapabilityPlanner;

pub fn capability_definition(capability_id: &str) -> Result<CapabilityDefinition, SdkError> {
    if capability_id != CREATE_CAPABILITY {
        return Err(unsupported_capability());
    }

    Ok(CapabilityDefinition {
        capability_id: support::configured_identifier(CapabilityId::try_new(CREATE_CAPABILITY))?,
        capability_version: support::configured_identifier(CapabilityVersion::try_new(
            support::CONTRACT_VERSION,
        ))?,
        owner_module_id: support::configured_identifier(ModuleId::try_new(MODULE_ID))?,
        input_contract: support::protobuf_contract(
            MODULE_ID,
            CREATE_REQUEST_SCHEMA,
            vec![DataClass::Personal],
        )?,
        output_contract: Some(support::protobuf_contract(
            MODULE_ID,
            CREATE_RESPONSE_SCHEMA,
            vec![DataClass::Personal],
        )?),
        risk: CapabilityRisk::Medium,
        mutation: true,
        requires_idempotency: true,
        requires_approval: false,
        authorization_policy_id: CREATE_CAPABILITY.to_owned(),
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
        let command: wire::CreatePartyRequest =
            support::decode_request(request, MODULE_ID, CREATE_REQUEST_SCHEMA)?;
        let party_ref = command
            .party_ref
            .ok_or_else(|| SdkError::invalid_argument("party.party_ref", "Party reference is required"))?;
        let party_id = PartyId::try_new(party_ref.party_id)?;

        Ok(AggregateTarget {
            reference: support::record_ref(RECORD_TYPE, party_id.as_str(), "party.party_ref.party_id")?,
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

        let command: wire::CreatePartyRequest =
            support::decode_request(request, MODULE_ID, CREATE_REQUEST_SCHEMA)?;
        let party_ref = command
            .party_ref
            .ok_or_else(|| SdkError::invalid_argument("party.party_ref", "Party reference is required"))?;
        let party = Party::create(CreateParty {
            party_id: PartyId::try_new(party_ref.party_id)?,
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
        let event = support::event_evidence(
            request,
            aggregate.clone(),
            MODULE_ID,
            EventSpec {
                event_type: "parties.party.created",
                event_schema_id: CREATED_EVENT_SCHEMA,
                aggregate_version: party.version(),
                previous_version: None,
            },
            &wire::PartyCreatedEvent {
                party: Some(public_party),
            },
        )?;
        let audit = support::audit_intent(
            request,
            &aggregate,
            party.version(),
            definition.capability_id.as_str(),
            &output.bytes,
        )?;

        Ok(CapabilityBatchExecutionPlan {
            batch: BatchMutationPlan {
                context: request.context.clone(),
                records: vec![RecordMutation::Create {
                    reference: aggregate,
                    payload: persisted_payload(&party)?,
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
    support::persisted_json_payload(persisted_contract(), encode_party_state(party)?)
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
    if definition.capability_id.as_str() != CREATE_CAPABILITY
        || definition.owner_module_id.as_str() != MODULE_ID
        || request.context.execution.capability_id.as_str() != CREATE_CAPABILITY
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
    fn create_capability_is_personal_mutation_with_idempotency() {
        let definition = capability_definition(CREATE_CAPABILITY).unwrap();
        assert_eq!(definition.owner_module_id.as_str(), MODULE_ID);
        assert!(definition.mutation);
        assert!(definition.requires_idempotency);
        assert_eq!(definition.input_contract.data_classes, vec![DataClass::Personal]);
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
        assert_eq!(wire.kind, crm_proto_contracts::crm::parties::v1::PartyKind::Organization as i32);
        assert_eq!(wire.resource_version.unwrap().version, 1);
    }
}
