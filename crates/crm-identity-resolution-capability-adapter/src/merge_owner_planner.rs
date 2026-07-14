use crate::MODULE_ID;
use crm_capability_plan_support::{self as support, EventSpec, PersistedPayloadContract};
use crm_capability_runtime::CapabilityRequest;
use crm_core_data::{OwnerMutationFragment, RecordMutation, RelationshipMutation};
use crm_identity_resolution::{
    MergeLineageStatus, MergePartyKind, PARTY_MERGE_LINEAGE_STATE_MAXIMUM_BYTES,
    PARTY_MERGE_LINEAGE_STATE_RETENTION_POLICY_ID, PARTY_MERGE_LINEAGE_STATE_SCHEMA_ID,
    PARTY_MERGE_LINEAGE_STATE_SCHEMA_VERSION, PartyMergeLineage, SurvivorshipSource,
    decode_party_merge_lineage_state, encode_party_merge_lineage_state,
    party_merge_lineage_state_descriptor_hash,
};
use crm_module_sdk::{
    DataClass, RecordId, RecordRef, RecordSnapshot, RecordType, RelationshipRef, RelationshipType,
    SdkError,
};
use crm_proto_contracts::crm::{
    core::v1 as core, customer::v1 as customer, identity_resolution::v1 as wire,
};

pub const MERGE_LINEAGE_RECORD_TYPE: &str = "identity_resolution.merge_lineage";
pub const PARTY_MERGE_LINEAGE_RELATIONSHIP_TYPE: &str = "identity_resolution.merge_lineage.party";
pub const PARTY_MERGE_LINEAGE_SOURCE_RECORD_TYPE: &str = "parties.party";

pub const MERGE_APPLIED_EVENT_TYPE: &str = "identity_resolution.merge.applied";
pub const MERGE_APPLIED_EVENT_SCHEMA: &str = "crm.identity_resolution.v1.PartyMergeAppliedEvent";
pub const MERGE_UNMERGED_EVENT_TYPE: &str = "identity_resolution.merge.unmerged";
pub const MERGE_UNMERGED_EVENT_SCHEMA: &str = "crm.identity_resolution.v1.PartyMergeUnmergedEvent";

const PARTY_LINK_SCHEMA_ID: &str = "crm.identity_resolution.merge_lineage.party-link";
const PARTY_LINK_SCHEMA_VERSION: &str = "1.0.0";
const PARTY_LINK_MAXIMUM_BYTES: u64 = 1_024;
const PARTY_LINK_DESCRIPTOR_HASH: [u8; 32] = [
    109, 245, 152, 107, 117, 103, 54, 25, 21, 128, 179, 21, 53, 99, 129, 125, 79, 196, 84, 233,
    194, 245, 94, 61, 134, 153, 164, 9, 151, 33, 140, 142,
];

pub fn plan_identity_merge_owner_fragment(
    request: &CapabilityRequest,
    lineage: &PartyMergeLineage,
) -> Result<OwnerMutationFragment, SdkError> {
    if lineage.status() != MergeLineageStatus::Active || lineage.version() != 1 {
        return Err(invalid_owner_plan(
            "IDENTITY_RESOLUTION_MERGE_LINEAGE_CREATE_STATE_INVALID",
            "new merge lineage must be active at version 1",
        ));
    }
    let aggregate = merge_lineage_record_ref(lineage)?;
    let event = support::event_evidence_with_data_class(
        request,
        aggregate.clone(),
        MODULE_ID,
        EventSpec {
            event_type: MERGE_APPLIED_EVENT_TYPE,
            event_schema_id: MERGE_APPLIED_EVENT_SCHEMA,
            aggregate_version: lineage.version(),
            previous_version: None,
        },
        DataClass::Personal,
        &wire::PartyMergeAppliedEvent {
            merge_lineage: Some(party_merge_lineage_to_wire(lineage)),
        },
    )?;

    let mut relationships = Vec::with_capacity(2);
    for party_ref in [lineage.pair().left(), lineage.pair().right()] {
        relationships.push(RelationshipMutation::Link {
            relationship: RelationshipRef {
                relationship_type: configured_relationship_type()?,
                source: RecordRef {
                    record_type: configured_record_type(PARTY_MERGE_LINEAGE_SOURCE_RECORD_TYPE)?,
                    record_id: RecordId::try_new(party_ref.as_str()).map_err(config_error)?,
                },
                target: aggregate.clone(),
            },
            payload: party_link_payload()?,
        });
    }

    Ok(OwnerMutationFragment {
        owner_module_id: configured_module_id()?,
        records: vec![RecordMutation::Create {
            reference: aggregate,
            payload: merge_lineage_persisted_payload(lineage)?,
        }],
        relationships,
        events: vec![event],
    })
}

pub fn plan_identity_unmerge_owner_fragment(
    request: &CapabilityRequest,
    current: &PartyMergeLineage,
    resulting: &PartyMergeLineage,
) -> Result<OwnerMutationFragment, SdkError> {
    validate_unmerge_transition(current, resulting)?;
    let aggregate = merge_lineage_record_ref(resulting)?;
    let event = support::event_evidence_with_data_class(
        request,
        aggregate.clone(),
        MODULE_ID,
        EventSpec {
            event_type: MERGE_UNMERGED_EVENT_TYPE,
            event_schema_id: MERGE_UNMERGED_EVENT_SCHEMA,
            aggregate_version: resulting.version(),
            previous_version: Some(current.version()),
        },
        DataClass::Personal,
        &wire::PartyMergeUnmergedEvent {
            merge_lineage: Some(party_merge_lineage_to_wire(resulting)),
        },
    )?;

    Ok(OwnerMutationFragment {
        owner_module_id: configured_module_id()?,
        records: vec![RecordMutation::Update {
            reference: aggregate,
            expected_version: current.version(),
            payload: merge_lineage_persisted_payload(resulting)?,
        }],
        relationships: Vec::new(),
        events: vec![event],
    })
}

pub fn merge_lineage_persisted_contract() -> PersistedPayloadContract<'static> {
    PersistedPayloadContract {
        owner: MODULE_ID,
        schema_id: PARTY_MERGE_LINEAGE_STATE_SCHEMA_ID,
        schema_version: PARTY_MERGE_LINEAGE_STATE_SCHEMA_VERSION,
        descriptor_hash: party_merge_lineage_state_descriptor_hash(),
        maximum_size_bytes: PARTY_MERGE_LINEAGE_STATE_MAXIMUM_BYTES,
        retention_policy_id: PARTY_MERGE_LINEAGE_STATE_RETENTION_POLICY_ID,
    }
}

pub fn merge_lineage_from_snapshot(
    snapshot: &RecordSnapshot,
) -> Result<PartyMergeLineage, SdkError> {
    let lineage = decode_party_merge_lineage_state(support::persisted_json_bytes_with_data_class(
        snapshot,
        merge_lineage_persisted_contract(),
        DataClass::Personal,
    )?)?;
    if lineage.merge_id().as_str() != snapshot.reference.record_id.as_str()
        || lineage.version() != snapshot.version
    {
        return Err(support::stored_data_error(
            "IDENTITY_RESOLUTION_MERGE_LINEAGE_IDENTITY_INVALID",
        ));
    }
    Ok(lineage)
}

pub fn party_merge_lineage_to_wire(lineage: &PartyMergeLineage) -> wire::PartyMergeLineage {
    wire::PartyMergeLineage {
        merge_ref: Some(wire::PartyMergeLineageRef {
            merge_id: lineage.merge_id().as_str().to_owned(),
        }),
        candidate_case_ref: Some(wire::DuplicateCandidateCaseRef {
            case_id: lineage.candidate_case_id().as_str().to_owned(),
        }),
        candidate_case_version: lineage.candidate_case_version(),
        left_party_ref: Some(customer::PartyRef {
            party_id: lineage.pair().left().as_str().to_owned(),
        }),
        right_party_ref: Some(customer::PartyRef {
            party_id: lineage.pair().right().as_str().to_owned(),
        }),
        survivor_party_ref: Some(customer::PartyRef {
            party_id: lineage.survivor_party_ref().as_str().to_owned(),
        }),
        absorbed_party_ref: Some(customer::PartyRef {
            party_id: lineage.absorbed_party_ref().as_str().to_owned(),
        }),
        party_kind: match lineage.party_kind() {
            MergePartyKind::Person => wire::MergePartyKind::Person as i32,
            MergePartyKind::Organization => wire::MergePartyKind::Organization as i32,
        },
        survivor_pre_merge_version: lineage.survivor_pre_merge_version(),
        absorbed_pre_merge_version: lineage.absorbed_pre_merge_version(),
        survivor_post_merge_version: lineage.survivor_post_merge_version(),
        absorbed_post_merge_version: lineage.absorbed_post_merge_version(),
        display_name_survivorship: Some(wire::DisplayNameSurvivorship {
            chosen_source: match lineage.display_name_survivorship().chosen_source() {
                SurvivorshipSource::Survivor => wire::SurvivorshipSource::Survivor as i32,
                SurvivorshipSource::Absorbed => wire::SurvivorshipSource::Absorbed as i32,
            },
            survivor_value: lineage
                .display_name_survivorship()
                .survivor_value()
                .to_owned(),
            absorbed_value: lineage
                .display_name_survivorship()
                .absorbed_value()
                .to_owned(),
            chosen_value: lineage
                .display_name_survivorship()
                .chosen_value()
                .to_owned(),
        }),
        merge_actor_ref: lineage.merge_actor_ref().as_str().to_owned(),
        merged_at: Some(core::UnixTime {
            unix_nanos: lineage.merged_at_unix_nanos(),
        }),
        status: match lineage.status() {
            MergeLineageStatus::Active => wire::PartyMergeLineageStatus::Active as i32,
            MergeLineageStatus::Unmerged => wire::PartyMergeLineageStatus::Unmerged as i32,
        },
        unmerge_decision: lineage.unmerge_decision().map(|decision| {
            wire::PartyMergeUnmergeDecision {
                actor_ref: decision.actor_ref().as_str().to_owned(),
                reason: decision.reason().as_str().to_owned(),
                survivor_pre_unmerge_version: decision.survivor_pre_unmerge_version(),
                absorbed_pre_unmerge_version: decision.absorbed_pre_unmerge_version(),
                survivor_post_unmerge_version: decision.survivor_post_unmerge_version(),
                absorbed_post_unmerge_version: decision.absorbed_post_unmerge_version(),
                occurred_at: Some(core::UnixTime {
                    unix_nanos: decision.occurred_at_unix_nanos(),
                }),
            }
        }),
        resource_version: Some(customer::CustomerResourceVersion {
            version: lineage.version(),
        }),
    }
}

fn merge_lineage_persisted_payload(
    lineage: &PartyMergeLineage,
) -> Result<crm_module_sdk::TypedPayload, SdkError> {
    support::persisted_json_payload_with_data_class(
        merge_lineage_persisted_contract(),
        DataClass::Personal,
        encode_party_merge_lineage_state(lineage)?,
    )
}

fn merge_lineage_record_ref(lineage: &PartyMergeLineage) -> Result<RecordRef, SdkError> {
    support::record_ref(
        MERGE_LINEAGE_RECORD_TYPE,
        lineage.merge_id().as_str(),
        "identity_resolution.merge.merge_ref.merge_id",
    )
}

fn party_link_payload() -> Result<crm_module_sdk::TypedPayload, SdkError> {
    support::persisted_json_payload_with_data_class(
        PersistedPayloadContract {
            owner: MODULE_ID,
            schema_id: PARTY_LINK_SCHEMA_ID,
            schema_version: PARTY_LINK_SCHEMA_VERSION,
            descriptor_hash: PARTY_LINK_DESCRIPTOR_HASH,
            maximum_size_bytes: PARTY_LINK_MAXIMUM_BYTES,
            retention_policy_id: PARTY_MERGE_LINEAGE_STATE_RETENTION_POLICY_ID,
        },
        DataClass::Personal,
        b"{}".to_vec(),
    )
}

fn validate_unmerge_transition(
    current: &PartyMergeLineage,
    resulting: &PartyMergeLineage,
) -> Result<(), SdkError> {
    if current.status() != MergeLineageStatus::Active
        || resulting.status() != MergeLineageStatus::Unmerged
        || resulting.version() != current.version() + 1
        || current.merge_id() != resulting.merge_id()
        || current.candidate_case_id() != resulting.candidate_case_id()
        || current.candidate_case_version() != resulting.candidate_case_version()
        || current.pair() != resulting.pair()
        || current.survivor_party_ref() != resulting.survivor_party_ref()
        || current.absorbed_party_ref() != resulting.absorbed_party_ref()
        || current.party_kind() != resulting.party_kind()
        || current.survivor_pre_merge_version() != resulting.survivor_pre_merge_version()
        || current.absorbed_pre_merge_version() != resulting.absorbed_pre_merge_version()
        || current.survivor_post_merge_version() != resulting.survivor_post_merge_version()
        || current.absorbed_post_merge_version() != resulting.absorbed_post_merge_version()
        || current.display_name_survivorship() != resulting.display_name_survivorship()
        || current.merge_actor_ref() != resulting.merge_actor_ref()
        || current.merged_at_unix_nanos() != resulting.merged_at_unix_nanos()
    {
        return Err(invalid_owner_plan(
            "IDENTITY_RESOLUTION_MERGE_UNMERGE_TRANSITION_INVALID",
            "the requested lineage update is not an exact unmerge transition",
        ));
    }
    Ok(())
}

fn configured_module_id() -> Result<crm_module_sdk::ModuleId, SdkError> {
    crm_module_sdk::ModuleId::try_new(MODULE_ID).map_err(config_error)
}

fn configured_relationship_type() -> Result<RelationshipType, SdkError> {
    RelationshipType::try_new(PARTY_MERGE_LINEAGE_RELATIONSHIP_TYPE).map_err(config_error)
}

fn configured_record_type(value: &str) -> Result<RecordType, SdkError> {
    RecordType::try_new(value).map_err(config_error)
}

fn config_error(error: crm_module_sdk::IdentifierError) -> SdkError {
    SdkError::new(
        "IDENTITY_RESOLUTION_MERGE_CONFIGURATION_INVALID",
        crm_module_sdk::ErrorCategory::Internal,
        false,
        "The Identity Resolution merge owner fragment is not configured safely.",
    )
    .with_internal_reference(error.to_string())
}

fn invalid_owner_plan(code: &'static str, message: &'static str) -> SdkError {
    SdkError::new(
        code,
        crm_module_sdk::ErrorCategory::Internal,
        false,
        message,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crm_identity_resolution::{
        CanonicalPartyPair, CreatePartyMergeLineage, DisplayNameSurvivorship, MergeActorReference,
        PartyReference,
    };

    fn lineage() -> PartyMergeLineage {
        let survivor = PartyReference::try_new("party-a").unwrap();
        let absorbed = PartyReference::try_new("party-b").unwrap();
        let pair = CanonicalPartyPair::try_new(survivor.clone(), absorbed.clone()).unwrap();
        PartyMergeLineage::create(CreatePartyMergeLineage {
            candidate_case_id: crm_identity_resolution::DuplicateCandidateCaseId::for_pair(&pair)
                .unwrap(),
            candidate_case_version: 2,
            survivor_party_ref: survivor,
            absorbed_party_ref: absorbed,
            party_kind: MergePartyKind::Person,
            survivor_pre_merge_version: 3,
            absorbed_pre_merge_version: 4,
            survivor_post_merge_version: 4,
            absorbed_post_merge_version: 5,
            display_name_survivorship: DisplayNameSurvivorship::try_new(
                SurvivorshipSource::Absorbed,
                "Alpha",
                "Beta",
            )
            .unwrap(),
            merge_actor_ref: MergeActorReference::try_new("reviewer-1").unwrap(),
            occurred_at_unix_nanos: 100,
        })
        .unwrap()
    }

    #[test]
    fn merge_lineage_wire_and_persistence_coordinates_are_stable() {
        let value = lineage();
        let public = party_merge_lineage_to_wire(&value);
        assert_eq!(
            public.merge_ref.unwrap().merge_id,
            value.merge_id().as_str()
        );
        let contract = merge_lineage_persisted_contract();
        assert_eq!(contract.owner, MODULE_ID);
        assert_eq!(contract.schema_id, PARTY_MERGE_LINEAGE_STATE_SCHEMA_ID);
        assert_eq!(
            contract.schema_version,
            PARTY_MERGE_LINEAGE_STATE_SCHEMA_VERSION
        );
        assert_eq!(
            contract.descriptor_hash,
            party_merge_lineage_state_descriptor_hash()
        );
    }

    #[test]
    fn invalid_unmerge_transition_is_rejected() {
        let current = lineage();
        let resulting = lineage();
        assert_eq!(
            validate_unmerge_transition(&current, &resulting)
                .unwrap_err()
                .code,
            "IDENTITY_RESOLUTION_MERGE_UNMERGE_TRANSITION_INVALID"
        );
    }
}
