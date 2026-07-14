use crate::planner::{MODULE_ID, RECORD_TYPE, party_to_wire};
use crm_capability_plan_support::{self as support, EventSpec, PersistedPayloadContract};
use crm_capability_runtime::CapabilityRequest;
use crm_core_data::{EventEvidence, RecordMutation};
use crm_module_sdk::{DataClass, ErrorCategory, SdkError};
use crm_parties::{
    ApplyMergeDisplayName, MergeLineageReference, MarkPartyMerged, PARTY_STATE_MAXIMUM_BYTES,
    PARTY_STATE_RETENTION_POLICY_ID, PARTY_STATE_SCHEMA_ID, PARTY_STATE_V2_SCHEMA_VERSION, Party,
    PartyKind, ReactivatePartyFromMerge, encode_party_state_v2, party_state_v2_descriptor_hash,
};
use crm_proto_contracts::crm::{customer::v1 as customer, parties::v1 as wire};

pub const MERGE_REDIRECT_APPLIED_EVENT_TYPE: &str = "parties.party.merge_redirect_applied";
pub const MERGE_REDIRECT_APPLIED_EVENT_SCHEMA: &str =
    "crm.parties.v1.PartyMergeRedirectAppliedEvent";
pub const MERGE_REDIRECT_REMOVED_EVENT_TYPE: &str = "parties.party.merge_redirect_removed";
pub const MERGE_REDIRECT_REMOVED_EVENT_SCHEMA: &str =
    "crm.parties.v1.PartyMergeRedirectRemovedEvent";
pub const MERGE_SURVIVORSHIP_UPDATED_EVENT_TYPE: &str =
    "parties.party.merge_survivorship_updated";
pub const MERGE_SURVIVORSHIP_UPDATED_EVENT_SCHEMA: &str =
    "crm.parties.v1.PartyMergeSurvivorshipUpdatedEvent";
pub const MERGE_SURVIVORSHIP_RESTORED_EVENT_TYPE: &str =
    "parties.party.merge_survivorship_restored";
pub const MERGE_SURVIVORSHIP_RESTORED_EVENT_SCHEMA: &str =
    "crm.parties.v1.PartyMergeSurvivorshipRestoredEvent";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartyDisplayNameSurvivorship {
    Survivor,
    Absorbed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartyOwnedMutationFragment {
    pub records: Vec<RecordMutation>,
    pub events: Vec<EventEvidence>,
    pub resulting_survivor: Party,
    pub resulting_absorbed: Party,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanPartyMergeOwnerFragment<'a> {
    pub survivor: &'a Party,
    pub absorbed: &'a Party,
    pub expected_survivor_version: i64,
    pub expected_absorbed_version: i64,
    pub merge_lineage_ref: MergeLineageReference,
    pub display_name_survivorship: PartyDisplayNameSurvivorship,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanPartyUnmergeOwnerFragment<'a> {
    pub survivor: &'a Party,
    pub absorbed: &'a Party,
    pub expected_survivor_version: i64,
    pub expected_absorbed_version: i64,
    pub merge_lineage_ref: MergeLineageReference,
    pub restore_survivor_display_name: Option<String>,
}

pub fn plan_party_merge_owner_fragment(
    request: &CapabilityRequest,
    input: PlanPartyMergeOwnerFragment<'_>,
) -> Result<PartyOwnedMutationFragment, SdkError> {
    validate_pair(input.survivor, input.absorbed)?;
    let occurred_at = request.context.execution.request_started_at_unix_nanos;
    let mut survivor = input.survivor.clone();
    let mut absorbed = input.absorbed.clone();
    let original_survivor_version = survivor.version();
    let original_absorbed_version = absorbed.version();

    let survivor_changed = match input.display_name_survivorship {
        PartyDisplayNameSurvivorship::Survivor => {
            require_expected_version(&survivor, input.expected_survivor_version)?;
            require_active(&survivor)?;
            false
        }
        PartyDisplayNameSurvivorship::Absorbed => {
            require_active(&survivor)?;
            if survivor.display_name() == absorbed.display_name() {
                require_expected_version(&survivor, input.expected_survivor_version)?;
                false
            } else {
                survivor.apply_merge_display_name(ApplyMergeDisplayName {
                    expected_version: input.expected_survivor_version,
                    display_name: absorbed.display_name().to_owned(),
                    occurred_at_unix_nanos: occurred_at,
                })?;
                true
            }
        }
    };

    absorbed.mark_merged(MarkPartyMerged {
        expected_version: input.expected_absorbed_version,
        survivor_party_id: survivor.party_id().clone(),
        merge_lineage_ref: input.merge_lineage_ref.clone(),
        occurred_at_unix_nanos: occurred_at,
    })?;

    let mut records = Vec::with_capacity(if survivor_changed { 2 } else { 1 });
    let mut events = Vec::with_capacity(if survivor_changed { 2 } else { 1 });

    if survivor_changed {
        let aggregate = party_record_ref(&survivor)?;
        records.push(RecordMutation::Update {
            reference: aggregate.clone(),
            expected_version: original_survivor_version,
            payload: v2_persisted_payload(&survivor)?,
        });
        events.push(support::event_evidence_with_data_class(
            request,
            aggregate,
            MODULE_ID,
            EventSpec {
                event_type: MERGE_SURVIVORSHIP_UPDATED_EVENT_TYPE,
                event_schema_id: MERGE_SURVIVORSHIP_UPDATED_EVENT_SCHEMA,
                aggregate_version: survivor.version(),
                previous_version: Some(original_survivor_version),
            },
            DataClass::Personal,
            &wire::PartyMergeSurvivorshipUpdatedEvent {
                party: Some(party_to_wire(&survivor)),
                merge_lineage_id: input.merge_lineage_ref.as_str().to_owned(),
                changed_fields: vec!["display_name".to_owned()],
            },
        )?);
    }

    let absorbed_aggregate = party_record_ref(&absorbed)?;
    records.push(RecordMutation::Update {
        reference: absorbed_aggregate.clone(),
        expected_version: original_absorbed_version,
        payload: v2_persisted_payload(&absorbed)?,
    });
    events.push(support::event_evidence_with_data_class(
        request,
        absorbed_aggregate,
        MODULE_ID,
        EventSpec {
            event_type: MERGE_REDIRECT_APPLIED_EVENT_TYPE,
            event_schema_id: MERGE_REDIRECT_APPLIED_EVENT_SCHEMA,
            aggregate_version: absorbed.version(),
            previous_version: Some(original_absorbed_version),
        },
        DataClass::Personal,
        &wire::PartyMergeRedirectAppliedEvent {
            absorbed_party_ref: Some(customer::PartyRef {
                party_id: absorbed.party_id().as_str().to_owned(),
            }),
            survivor_party_ref: Some(customer::PartyRef {
                party_id: survivor.party_id().as_str().to_owned(),
            }),
            merge_lineage_id: input.merge_lineage_ref.as_str().to_owned(),
            absorbed_party_version: absorbed.version(),
        },
    )?);

    Ok(PartyOwnedMutationFragment {
        records,
        events,
        resulting_survivor: survivor,
        resulting_absorbed: absorbed,
    })
}

pub fn plan_party_unmerge_owner_fragment(
    request: &CapabilityRequest,
    input: PlanPartyUnmergeOwnerFragment<'_>,
) -> Result<PartyOwnedMutationFragment, SdkError> {
    validate_pair(input.survivor, input.absorbed)?;
    let occurred_at = request.context.execution.request_started_at_unix_nanos;
    let mut survivor = input.survivor.clone();
    let mut absorbed = input.absorbed.clone();
    let original_survivor_version = survivor.version();
    let original_absorbed_version = absorbed.version();

    let survivor_changed = if let Some(display_name) = input.restore_survivor_display_name {
        survivor.apply_merge_display_name(ApplyMergeDisplayName {
            expected_version: input.expected_survivor_version,
            display_name,
            occurred_at_unix_nanos: occurred_at,
        })?;
        true
    } else {
        require_expected_version(&survivor, input.expected_survivor_version)?;
        require_active(&survivor)?;
        false
    };

    absorbed.reactivate_from_merge(ReactivatePartyFromMerge {
        expected_version: input.expected_absorbed_version,
        expected_survivor_party_id: survivor.party_id().clone(),
        expected_merge_lineage_ref: input.merge_lineage_ref.clone(),
        occurred_at_unix_nanos: occurred_at,
    })?;

    let mut records = Vec::with_capacity(if survivor_changed { 2 } else { 1 });
    let mut events = Vec::with_capacity(if survivor_changed { 2 } else { 1 });

    if survivor_changed {
        let aggregate = party_record_ref(&survivor)?;
        records.push(RecordMutation::Update {
            reference: aggregate.clone(),
            expected_version: original_survivor_version,
            payload: v2_persisted_payload(&survivor)?,
        });
        events.push(support::event_evidence_with_data_class(
            request,
            aggregate,
            MODULE_ID,
            EventSpec {
                event_type: MERGE_SURVIVORSHIP_RESTORED_EVENT_TYPE,
                event_schema_id: MERGE_SURVIVORSHIP_RESTORED_EVENT_SCHEMA,
                aggregate_version: survivor.version(),
                previous_version: Some(original_survivor_version),
            },
            DataClass::Personal,
            &wire::PartyMergeSurvivorshipRestoredEvent {
                party: Some(party_to_wire(&survivor)),
                merge_lineage_id: input.merge_lineage_ref.as_str().to_owned(),
                changed_fields: vec!["display_name".to_owned()],
            },
        )?);
    }

    let absorbed_aggregate = party_record_ref(&absorbed)?;
    records.push(RecordMutation::Update {
        reference: absorbed_aggregate.clone(),
        expected_version: original_absorbed_version,
        payload: v2_persisted_payload(&absorbed)?,
    });
    events.push(support::event_evidence_with_data_class(
        request,
        absorbed_aggregate,
        MODULE_ID,
        EventSpec {
            event_type: MERGE_REDIRECT_REMOVED_EVENT_TYPE,
            event_schema_id: MERGE_REDIRECT_REMOVED_EVENT_SCHEMA,
            aggregate_version: absorbed.version(),
            previous_version: Some(original_absorbed_version),
        },
        DataClass::Personal,
        &wire::PartyMergeRedirectRemovedEvent {
            absorbed_party_ref: Some(customer::PartyRef {
                party_id: absorbed.party_id().as_str().to_owned(),
            }),
            survivor_party_ref: Some(customer::PartyRef {
                party_id: survivor.party_id().as_str().to_owned(),
            }),
            merge_lineage_id: input.merge_lineage_ref.as_str().to_owned(),
            absorbed_party_version: absorbed.version(),
        },
    )?);

    Ok(PartyOwnedMutationFragment {
        records,
        events,
        resulting_survivor: survivor,
        resulting_absorbed: absorbed,
    })
}

pub fn party_v2_persisted_contract() -> PersistedPayloadContract<'static> {
    PersistedPayloadContract {
        owner: MODULE_ID,
        schema_id: PARTY_STATE_SCHEMA_ID,
        schema_version: PARTY_STATE_V2_SCHEMA_VERSION,
        descriptor_hash: party_state_v2_descriptor_hash(),
        maximum_size_bytes: PARTY_STATE_MAXIMUM_BYTES,
        retention_policy_id: PARTY_STATE_RETENTION_POLICY_ID,
    }
}

fn v2_persisted_payload(party: &Party) -> Result<crm_module_sdk::TypedPayload, SdkError> {
    support::persisted_json_payload_with_data_class(
        party_v2_persisted_contract(),
        DataClass::Personal,
        encode_party_state_v2(party)?,
    )
}

fn party_record_ref(party: &Party) -> Result<crm_module_sdk::RecordRef, SdkError> {
    support::record_ref(
        RECORD_TYPE,
        party.party_id().as_str(),
        "party.party_ref.party_id",
    )
}

fn validate_pair(survivor: &Party, absorbed: &Party) -> Result<(), SdkError> {
    if survivor.party_id() == absorbed.party_id() {
        return Err(invalid_owner_plan(
            "PARTIES_MERGE_SELF_INVALID",
            "a Party cannot be merged into itself",
        ));
    }
    if survivor.kind() != absorbed.kind() {
        return Err(invalid_owner_plan(
            "PARTIES_MERGE_KIND_MISMATCH",
            "Party merge requires matching Party kinds",
        ));
    }
    require_active(survivor)?;
    require_active(absorbed)
}

fn require_expected_version(party: &Party, expected_version: i64) -> Result<(), SdkError> {
    if party.version() != expected_version {
        return Err(SdkError::new(
            "PARTIES_PARTY_VERSION_CONFLICT",
            ErrorCategory::Conflict,
            false,
            format!(
                "expected Party version {expected_version}, found {}",
                party.version()
            ),
        ));
    }
    Ok(())
}

fn require_active(party: &Party) -> Result<(), SdkError> {
    if !party.is_active() {
        return Err(SdkError::new(
            "PARTIES_PARTY_MERGED_READ_ONLY",
            ErrorCategory::Conflict,
            false,
            "a merged Party is not eligible for this owner transition",
        ));
    }
    Ok(())
}

fn invalid_owner_plan(code: &'static str, message: &'static str) -> SdkError {
    SdkError::new(code, ErrorCategory::InvalidArgument, false, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crm_parties::{CreateParty, PartyId, PartyLifecycle};

    fn party(id: &str, kind: PartyKind, name: &str) -> Party {
        Party::create(CreateParty {
            party_id: PartyId::try_new(id).unwrap(),
            kind,
            display_name: name.to_owned(),
            occurred_at_unix_nanos: 10,
        })
        .unwrap()
    }

    fn merge_ref() -> MergeLineageReference {
        MergeLineageReference::try_new(
            "idrm-0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        )
        .unwrap()
    }

    #[test]
    fn merge_state_plan_is_directional_and_preserves_absorbed_identity() {
        let survivor = party("party-a", PartyKind::Person, "Alpha Person");
        let absorbed = party("party-b", PartyKind::Person, "Beta Person");
        let (survivor, absorbed, survivor_changed) = plan_merge_state(
            &survivor,
            &absorbed,
            1,
            1,
            merge_ref(),
            PartyDisplayNameSurvivorship::Absorbed,
            20,
        )
        .unwrap();
        assert!(survivor_changed);
        assert_eq!(survivor.display_name(), "Beta Person");
        assert_eq!(survivor.version(), 2);
        assert_eq!(absorbed.party_id().as_str(), "party-b");
        assert_eq!(absorbed.version(), 2);
        assert!(matches!(absorbed.lifecycle(), PartyLifecycle::Merged { .. }));
    }

    #[test]
    fn unmerge_state_plan_rejects_later_survivor_version() {
        let survivor = party("party-a", PartyKind::Organization, "Alpha");
        let absorbed = party("party-b", PartyKind::Organization, "Beta");
        let (mut survivor, absorbed, _) = plan_merge_state(
            &survivor,
            &absorbed,
            1,
            1,
            merge_ref(),
            PartyDisplayNameSurvivorship::Absorbed,
            20,
        )
        .unwrap();
        survivor
            .apply_update(crm_parties::UpdateParty {
                expected_version: 2,
                display_name: "Later Edit".to_owned(),
                occurred_at_unix_nanos: 30,
            })
            .unwrap();
        let error = plan_unmerge_state(
            &survivor,
            &absorbed,
            2,
            2,
            merge_ref(),
            Some("Alpha".to_owned()),
            40,
        )
        .unwrap_err();
        assert_eq!(error.code, "PARTIES_PARTY_VERSION_CONFLICT");
    }

    #[test]
    fn cross_kind_merge_is_rejected_before_any_state_change() {
        let survivor = party("party-a", PartyKind::Person, "Alpha");
        let absorbed = party("party-b", PartyKind::Organization, "Beta");
        assert_eq!(
            plan_merge_state(
                &survivor,
                &absorbed,
                1,
                1,
                merge_ref(),
                PartyDisplayNameSurvivorship::Survivor,
                20,
            )
            .unwrap_err()
            .code,
            "PARTIES_MERGE_KIND_MISMATCH"
        );
        assert_eq!(survivor.version(), 1);
        assert_eq!(absorbed.version(), 1);
    }

    fn plan_merge_state(
        survivor: &Party,
        absorbed: &Party,
        expected_survivor_version: i64,
        expected_absorbed_version: i64,
        merge_lineage_ref: MergeLineageReference,
        display_name_survivorship: PartyDisplayNameSurvivorship,
        occurred_at_unix_nanos: i64,
    ) -> Result<(Party, Party, bool), SdkError> {
        validate_pair(survivor, absorbed)?;
        let mut survivor = survivor.clone();
        let mut absorbed = absorbed.clone();
        let survivor_changed = match display_name_survivorship {
            PartyDisplayNameSurvivorship::Survivor => {
                require_expected_version(&survivor, expected_survivor_version)?;
                false
            }
            PartyDisplayNameSurvivorship::Absorbed => {
                if survivor.display_name() == absorbed.display_name() {
                    require_expected_version(&survivor, expected_survivor_version)?;
                    false
                } else {
                    survivor.apply_merge_display_name(ApplyMergeDisplayName {
                        expected_version: expected_survivor_version,
                        display_name: absorbed.display_name().to_owned(),
                        occurred_at_unix_nanos,
                    })?;
                    true
                }
            }
        };
        absorbed.mark_merged(MarkPartyMerged {
            expected_version: expected_absorbed_version,
            survivor_party_id: survivor.party_id().clone(),
            merge_lineage_ref,
            occurred_at_unix_nanos,
        })?;
        Ok((survivor, absorbed, survivor_changed))
    }

    fn plan_unmerge_state(
        survivor: &Party,
        absorbed: &Party,
        expected_survivor_version: i64,
        expected_absorbed_version: i64,
        merge_lineage_ref: MergeLineageReference,
        restore_survivor_display_name: Option<String>,
        occurred_at_unix_nanos: i64,
    ) -> Result<(Party, Party, bool), SdkError> {
        validate_pair(survivor, absorbed)?;
        let mut survivor = survivor.clone();
        let mut absorbed = absorbed.clone();
        let survivor_changed = if let Some(display_name) = restore_survivor_display_name {
            survivor.apply_merge_display_name(ApplyMergeDisplayName {
                expected_version: expected_survivor_version,
                display_name,
                occurred_at_unix_nanos,
            })?;
            true
        } else {
            require_expected_version(&survivor, expected_survivor_version)?;
            false
        };
        absorbed.reactivate_from_merge(ReactivatePartyFromMerge {
            expected_version: expected_absorbed_version,
            expected_survivor_party_id: survivor.party_id().clone(),
            expected_merge_lineage_ref: merge_lineage_ref,
            occurred_at_unix_nanos,
        })?;
        Ok((survivor, absorbed, survivor_changed))
    }
}
