use crate::{
    CANDIDATE_MUTATION_CAPABILITY_IDS, MERGE_CAPABILITY, MERGE_MUTATION_CAPABILITY_IDS, MODULE_ID,
    REGISTER_CAPABILITY, UNMERGE_CAPABILITY,
};
use crm_capability_plan_support::{self as support, PersistedPayloadContract};
use crm_capability_runtime::{CapabilityDefinition, CapabilityRequest};
use crm_core_data::{
    AggregateTarget, CapabilityBatchExecutionPlan, RelationshipMutation,
    TransactionalAggregatePlanner,
};
use crm_identity_resolution::{
    DUPLICATE_CANDIDATE_CASE_STATE_RETENTION_POLICY_ID, MERGE_OPERATION_STATE_RETENTION_POLICY_ID,
};
use crm_module_sdk::{
    DataClass, RecordId, RecordRef, RecordSnapshot, RecordType, RelationshipRef, RelationshipType,
    SdkError,
};

#[path = "owner_planner.rs"]
mod owner_planner;

pub use owner_planner::{
    EvidenceReferenceScope, PartyVersionExpectation, duplicate_candidate_case_from_snapshot,
    duplicate_candidate_case_to_wire, evidence_reference_scope_from_request, persisted_contract,
    persisted_payload,
};

pub const PARTY_CANDIDATE_RELATIONSHIP_TYPE: &str = "identity_resolution.candidate.party";
pub const PARTY_CANDIDATE_SOURCE_RECORD_TYPE: &str = "parties.party";
pub const PARTY_MERGE_RELATIONSHIP_TYPE: &str = "identity_resolution.merge.party";
pub const PARTY_MERGE_SOURCE_RECORD_TYPE: &str = "parties.party";
pub const CANONICAL_REDIRECT_RELATIONSHIP_TYPE: &str = "identity_resolution.canonical_redirect";
pub const CANONICAL_REDIRECT_PARTY_RECORD_TYPE: &str = "parties.party";

const PARTY_LINK_SCHEMA_ID: &str = "crm.identity_resolution.candidate.party-link";
const PARTY_LINK_SCHEMA_VERSION: &str = "1.0.0";
const PARTY_LINK_MAXIMUM_BYTES: u64 = 1_024;
const PARTY_LINK_DESCRIPTOR_HASH: [u8; 32] = [
    37, 183, 202, 14, 177, 222, 72, 169, 159, 53, 204, 23, 148, 41, 31, 224, 90, 184, 197, 78, 56,
    196, 177, 36, 3, 152, 108, 209, 14, 197, 73, 111,
];
const MERGE_PARTY_LINK_SCHEMA_ID: &str = "crm.identity_resolution.merge.party-link";
const MERGE_PARTY_LINK_SCHEMA_VERSION: &str = "1.0.0";
const MERGE_PARTY_LINK_MAXIMUM_BYTES: u64 = 1_024;
const MERGE_PARTY_LINK_DESCRIPTOR_HASH: [u8; 32] = [
    63, 48, 239, 6, 128, 145, 231, 99, 49, 72, 104, 141, 41, 175, 121, 83, 80, 180, 43, 52, 220,
    16, 147, 23, 147, 87, 83, 7, 38, 143, 192, 15,
];
const CANONICAL_REDIRECT_SCHEMA_ID: &str = "crm.identity_resolution.canonical_redirect";
const CANONICAL_REDIRECT_SCHEMA_VERSION: &str = "1.0.0";
const CANONICAL_REDIRECT_MAXIMUM_BYTES: u64 = 1_024;
const CANONICAL_REDIRECT_DESCRIPTOR_HASH: [u8; 32] = [
    116, 222, 36, 232, 8, 51, 80, 134, 40, 101, 85, 41, 108, 205, 203, 215, 131, 29, 31, 241, 92,
    245, 63, 130, 194, 181, 243, 199, 181, 87, 229, 104,
];

/// Governed Identity Resolution planner that routes candidate-case and merge-lineage
/// aggregates without combining their owner record types.
///
/// Candidate registration atomically creates two Party -> candidate-case access links.
/// Merge execution atomically creates two Party -> merge-operation lineage access links
/// plus one source Party -> survivor Party canonical redirect. Unmerge removes exactly
/// that current redirect while preserving immutable lineage access links and history.
#[derive(Debug, Default, Clone, Copy)]
pub struct IdentityResolutionCapabilityPlanner;

impl TransactionalAggregatePlanner for IdentityResolutionCapabilityPlanner {
    fn target(
        &self,
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
    ) -> Result<AggregateTarget, SdkError> {
        if MERGE_MUTATION_CAPABILITY_IDS.contains(&definition.capability_id.as_str()) {
            crate::MergeLineageCapabilityPlanner.target(definition, request)
        } else if CANDIDATE_MUTATION_CAPABILITY_IDS.contains(&definition.capability_id.as_str()) {
            owner_planner::IdentityResolutionCapabilityPlanner.target(definition, request)
        } else {
            Err(unsupported_capability())
        }
    }

    fn plan(
        &self,
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
        current: Option<&RecordSnapshot>,
    ) -> Result<CapabilityBatchExecutionPlan, SdkError> {
        let mut plan = if MERGE_MUTATION_CAPABILITY_IDS.contains(&definition.capability_id.as_str())
        {
            crate::MergeLineageCapabilityPlanner.plan(definition, request, current)?
        } else if CANDIDATE_MUTATION_CAPABILITY_IDS.contains(&definition.capability_id.as_str()) {
            owner_planner::IdentityResolutionCapabilityPlanner.plan(definition, request, current)?
        } else {
            return Err(unsupported_capability());
        };

        match definition.capability_id.as_str() {
            REGISTER_CAPABILITY => add_candidate_party_links(&mut plan, request, definition)?,
            MERGE_CAPABILITY => {
                add_merge_party_links(&mut plan, request, definition)?;
                add_canonical_redirect_link(&mut plan, request)?;
            }
            UNMERGE_CAPABILITY => remove_canonical_redirect_link(&mut plan, current)?,
            _ => {}
        }
        Ok(plan)
    }
}

fn add_candidate_party_links(
    plan: &mut CapabilityBatchExecutionPlan,
    request: &CapabilityRequest,
    definition: &CapabilityDefinition,
) -> Result<(), SdkError> {
    let scope = evidence_reference_scope_from_request(REGISTER_CAPABILITY, request)?
        .ok_or_else(invalid_registration_scope)?;
    let target = owner_planner::IdentityResolutionCapabilityPlanner
        .target(definition, request)?
        .reference;
    for expectation in scope.parties {
        plan.batch.relationships.push(RelationshipMutation::Link {
            relationship: RelationshipRef {
                relationship_type: configured_relationship_type(PARTY_CANDIDATE_RELATIONSHIP_TYPE)?,
                source: RecordRef {
                    record_type: configured_record_type(PARTY_CANDIDATE_SOURCE_RECORD_TYPE)?,
                    record_id: RecordId::try_new(expectation.party_ref.as_str())
                        .map_err(config_error)?,
                },
                target: target.clone(),
            },
            payload: candidate_party_link_payload()?,
        });
    }
    Ok(())
}

fn add_merge_party_links(
    plan: &mut CapabilityBatchExecutionPlan,
    request: &CapabilityRequest,
    definition: &CapabilityDefinition,
) -> Result<(), SdkError> {
    let scope = crate::merge_reference_scope_from_request(request)?;
    let target = crate::MergeLineageCapabilityPlanner
        .target(definition, request)?
        .reference;
    for party_ref in [scope.source.party_ref, scope.survivor.party_ref] {
        plan.batch.relationships.push(RelationshipMutation::Link {
            relationship: RelationshipRef {
                relationship_type: configured_relationship_type(PARTY_MERGE_RELATIONSHIP_TYPE)?,
                source: party_record_ref(&party_ref)?,
                target: target.clone(),
            },
            payload: merge_party_link_payload()?,
        });
    }
    Ok(())
}

fn add_canonical_redirect_link(
    plan: &mut CapabilityBatchExecutionPlan,
    request: &CapabilityRequest,
) -> Result<(), SdkError> {
    let scope = crate::merge_reference_scope_from_request(request)?;
    plan.batch.relationships.push(RelationshipMutation::Link {
        relationship: RelationshipRef {
            relationship_type: configured_relationship_type(CANONICAL_REDIRECT_RELATIONSHIP_TYPE)?,
            source: party_record_ref(&scope.source.party_ref)?,
            target: party_record_ref(&scope.survivor.party_ref)?,
        },
        payload: canonical_redirect_payload()?,
    });
    Ok(())
}

fn remove_canonical_redirect_link(
    plan: &mut CapabilityBatchExecutionPlan,
    current: Option<&RecordSnapshot>,
) -> Result<(), SdkError> {
    let operation = crate::merge_operation_from_snapshot(current.ok_or_else(invalid_merge_state)?)?;
    plan.batch.relationships.push(RelationshipMutation::Unlink {
        relationship: RelationshipRef {
            relationship_type: configured_relationship_type(CANONICAL_REDIRECT_RELATIONSHIP_TYPE)?,
            source: party_record_ref(operation.source_party_ref())?,
            target: party_record_ref(operation.survivor_party_ref())?,
        },
    });
    Ok(())
}

fn party_record_ref(
    party_ref: &crm_identity_resolution::PartyReference,
) -> Result<RecordRef, SdkError> {
    Ok(RecordRef {
        record_type: configured_record_type(CANONICAL_REDIRECT_PARTY_RECORD_TYPE)?,
        record_id: RecordId::try_new(party_ref.as_str()).map_err(config_error)?,
    })
}

fn candidate_party_link_payload() -> Result<crm_module_sdk::TypedPayload, SdkError> {
    support::persisted_json_payload_with_data_class(
        PersistedPayloadContract {
            owner: MODULE_ID,
            schema_id: PARTY_LINK_SCHEMA_ID,
            schema_version: PARTY_LINK_SCHEMA_VERSION,
            descriptor_hash: PARTY_LINK_DESCRIPTOR_HASH,
            maximum_size_bytes: PARTY_LINK_MAXIMUM_BYTES,
            retention_policy_id: DUPLICATE_CANDIDATE_CASE_STATE_RETENTION_POLICY_ID,
        },
        DataClass::Personal,
        b"{}".to_vec(),
    )
}

fn merge_party_link_payload() -> Result<crm_module_sdk::TypedPayload, SdkError> {
    support::persisted_json_payload_with_data_class(
        PersistedPayloadContract {
            owner: MODULE_ID,
            schema_id: MERGE_PARTY_LINK_SCHEMA_ID,
            schema_version: MERGE_PARTY_LINK_SCHEMA_VERSION,
            descriptor_hash: MERGE_PARTY_LINK_DESCRIPTOR_HASH,
            maximum_size_bytes: MERGE_PARTY_LINK_MAXIMUM_BYTES,
            retention_policy_id: MERGE_OPERATION_STATE_RETENTION_POLICY_ID,
        },
        DataClass::Personal,
        b"{}".to_vec(),
    )
}

fn canonical_redirect_payload() -> Result<crm_module_sdk::TypedPayload, SdkError> {
    support::persisted_json_payload_with_data_class(
        PersistedPayloadContract {
            owner: MODULE_ID,
            schema_id: CANONICAL_REDIRECT_SCHEMA_ID,
            schema_version: CANONICAL_REDIRECT_SCHEMA_VERSION,
            descriptor_hash: CANONICAL_REDIRECT_DESCRIPTOR_HASH,
            maximum_size_bytes: CANONICAL_REDIRECT_MAXIMUM_BYTES,
            retention_policy_id: MERGE_OPERATION_STATE_RETENTION_POLICY_ID,
        },
        DataClass::Personal,
        b"{}".to_vec(),
    )
}

fn configured_relationship_type(value: &str) -> Result<RelationshipType, SdkError> {
    RelationshipType::try_new(value).map_err(config_error)
}

fn configured_record_type(value: &str) -> Result<RecordType, SdkError> {
    RecordType::try_new(value).map_err(config_error)
}

fn invalid_registration_scope() -> SdkError {
    SdkError::new(
        "IDENTITY_RESOLUTION_REGISTRATION_SCOPE_INVALID",
        crm_module_sdk::ErrorCategory::Internal,
        false,
        "The Identity Resolution registration scope is invalid.",
    )
}

fn invalid_merge_state() -> SdkError {
    SdkError::new(
        "IDENTITY_RESOLUTION_MERGE_STATE_INVALID",
        crm_module_sdk::ErrorCategory::Internal,
        false,
        "The Identity Resolution merge operation state is unavailable.",
    )
}

fn unsupported_capability() -> SdkError {
    SdkError::new(
        "IDENTITY_RESOLUTION_CAPABILITY_UNSUPPORTED",
        crm_module_sdk::ErrorCategory::Internal,
        false,
        "The Identity Resolution mutation capability is unsupported.",
    )
}

fn config_error(error: crm_module_sdk::IdentifierError) -> SdkError {
    SdkError::new(
        "IDENTITY_RESOLUTION_RELATIONSHIP_CONFIGURATION_INVALID",
        crm_module_sdk::ErrorCategory::Internal,
        false,
        "The Identity Resolution relationship access path is not configured safely.",
    )
    .with_internal_reference(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candidate_merge_and_redirect_link_contracts_are_personal_json_and_distinct() {
        let candidate = candidate_party_link_payload().unwrap();
        let merge = merge_party_link_payload().unwrap();
        let redirect = canonical_redirect_payload().unwrap();
        for payload in [&candidate, &merge, &redirect] {
            assert_eq!(payload.owner.as_str(), MODULE_ID);
            assert_eq!(payload.data_class, DataClass::Personal);
            assert_eq!(payload.encoding, crm_module_sdk::PayloadEncoding::Json);
            assert_eq!(payload.bytes, b"{}");
        }
        assert_ne!(candidate.schema_id, merge.schema_id);
        assert_ne!(candidate.schema_id, redirect.schema_id);
        assert_ne!(merge.schema_id, redirect.schema_id);
    }

    #[test]
    fn authoritative_party_access_and_redirect_coordinates_are_stable() {
        assert_eq!(
            configured_relationship_type(PARTY_CANDIDATE_RELATIONSHIP_TYPE)
                .unwrap()
                .as_str(),
            PARTY_CANDIDATE_RELATIONSHIP_TYPE
        );
        assert_eq!(
            configured_relationship_type(PARTY_MERGE_RELATIONSHIP_TYPE)
                .unwrap()
                .as_str(),
            PARTY_MERGE_RELATIONSHIP_TYPE
        );
        assert_eq!(
            configured_relationship_type(CANONICAL_REDIRECT_RELATIONSHIP_TYPE)
                .unwrap()
                .as_str(),
            CANONICAL_REDIRECT_RELATIONSHIP_TYPE
        );
        assert_eq!(
            configured_record_type(PARTY_MERGE_SOURCE_RECORD_TYPE)
                .unwrap()
                .as_str(),
            PARTY_MERGE_SOURCE_RECORD_TYPE
        );
    }
}
