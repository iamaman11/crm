use crate::{MODULE_ID, REGISTER_CAPABILITY};
use crm_capability_plan_support::{self as support, PersistedPayloadContract};
use crm_capability_runtime::{CapabilityDefinition, CapabilityRequest};
use crm_core_data::{
    AggregateTarget, CapabilityBatchExecutionPlan, RelationshipMutation,
    TransactionalAggregatePlanner,
};
use crm_identity_resolution::DUPLICATE_CANDIDATE_CASE_STATE_RETENTION_POLICY_ID;
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

const PARTY_LINK_SCHEMA_ID: &str = "crm.identity_resolution.candidate.party-link";
const PARTY_LINK_SCHEMA_VERSION: &str = "1.0.0";
const PARTY_LINK_MAXIMUM_BYTES: u64 = 1_024;
const PARTY_LINK_DESCRIPTOR_HASH: [u8; 32] = [
    37, 183, 202, 14, 177, 222, 72, 169, 159, 53, 204, 23, 148, 41, 31, 224, 90, 184, 197, 78, 56,
    196, 177, 36, 3, 152, 108, 209, 14, 197, 73, 111,
];

/// Governed Identity Resolution planner that preserves the pure owner plan and
/// atomically adds two Party -> candidate-case access-path relationships when a
/// canonical candidate pair is first registered.
///
/// These relationships are not identity truth and do not imply a merge. They
/// provide an authoritative, indexed access path from either Party to the one
/// candidate-case record while the owner record remains the only source of
/// candidate evidence and reviewer decision state.
#[derive(Debug, Default, Clone, Copy)]
pub struct IdentityResolutionCapabilityPlanner;

impl TransactionalAggregatePlanner for IdentityResolutionCapabilityPlanner {
    fn target(
        &self,
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
    ) -> Result<AggregateTarget, SdkError> {
        owner_planner::IdentityResolutionCapabilityPlanner.target(definition, request)
    }

    fn plan(
        &self,
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
        current: Option<&RecordSnapshot>,
    ) -> Result<CapabilityBatchExecutionPlan, SdkError> {
        let mut plan = owner_planner::IdentityResolutionCapabilityPlanner
            .plan(definition, request, current)?;
        if definition.capability_id.as_str() == REGISTER_CAPABILITY {
            let scope = evidence_reference_scope_from_request(REGISTER_CAPABILITY, request)?
                .ok_or_else(invalid_registration_scope)?;
            let target = owner_planner::IdentityResolutionCapabilityPlanner
                .target(definition, request)?
                .reference;
            for expectation in scope.parties {
                plan.batch.relationships.push(RelationshipMutation::Link {
                    relationship: RelationshipRef {
                        relationship_type: configured_relationship_type()?,
                        source: RecordRef {
                            record_type: configured_record_type(
                                PARTY_CANDIDATE_SOURCE_RECORD_TYPE,
                            )?,
                            record_id: RecordId::try_new(expectation.party_ref.as_str())
                                .map_err(config_error)?,
                        },
                        target: target.clone(),
                    },
                    payload: party_link_payload()?,
                });
            }
        }
        Ok(plan)
    }
}

fn party_link_payload() -> Result<crm_module_sdk::TypedPayload, SdkError> {
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

fn configured_relationship_type() -> Result<RelationshipType, SdkError> {
    RelationshipType::try_new(PARTY_CANDIDATE_RELATIONSHIP_TYPE).map_err(config_error)
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
    fn party_link_contract_is_personal_json_and_stably_hashed() {
        let payload = party_link_payload().unwrap();
        assert_eq!(payload.owner.as_str(), MODULE_ID);
        assert_eq!(payload.schema_id.as_str(), PARTY_LINK_SCHEMA_ID);
        assert_eq!(payload.data_class, DataClass::Personal);
        assert_eq!(payload.encoding, crm_module_sdk::PayloadEncoding::Json);
        assert_eq!(payload.bytes, b"{}");
        assert_eq!(payload.descriptor_hash, PARTY_LINK_DESCRIPTOR_HASH);
    }

    #[test]
    fn authoritative_party_access_path_coordinates_are_stable() {
        assert_eq!(
            configured_relationship_type().unwrap().as_str(),
            PARTY_CANDIDATE_RELATIONSHIP_TYPE
        );
        assert_eq!(
            configured_record_type(PARTY_CANDIDATE_SOURCE_RECORD_TYPE)
                .unwrap()
                .as_str(),
            PARTY_CANDIDATE_SOURCE_RECORD_TYPE
        );
    }
}
