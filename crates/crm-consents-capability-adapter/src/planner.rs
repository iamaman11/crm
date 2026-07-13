use crate::{CREATE_CAPABILITY, MODULE_ID};
use crm_capability_plan_support::{self as support, PersistedPayloadContract};
use crm_capability_runtime::{CapabilityDefinition, CapabilityRequest};
use crm_consents::CONSENT_AUTHORIZATION_STATE_RETENTION_POLICY_ID;
use crm_core_data::{
    AggregateTarget, CapabilityBatchExecutionPlan, RelationshipMutation,
    TransactionalAggregatePlanner,
};
use crm_module_sdk::{
    DataClass, RecordId, RecordRef, RecordSnapshot, RecordType, RelationshipRef, RelationshipType,
    SdkError,
};

#[path = "owner_planner.rs"]
mod owner_planner;

pub use owner_planner::{
    CreateConsentReferenceScope, consent_authorization_from_snapshot,
    consent_authorization_to_wire, persisted_contract, persisted_payload,
    referenced_scope_from_create,
};

pub const PARTY_AUTHORIZATION_RELATIONSHIP_TYPE: &str = "consents.authorization.party";
pub const PARTY_AUTHORIZATION_SOURCE_RECORD_TYPE: &str = "parties.party";

const PARTY_LINK_SCHEMA_ID: &str = "crm.consents.authorization.party-link";
const PARTY_LINK_SCHEMA_VERSION: &str = "1.0.0";
const PARTY_LINK_MAXIMUM_BYTES: u64 = 1_024;
const PARTY_LINK_DESCRIPTOR_HASH: [u8; 32] = [
    16, 148, 140, 202, 69, 17, 65, 34, 240, 245, 72, 186, 248, 226, 88, 171, 222, 205, 83, 102,
    214, 200, 170, 146, 101, 39, 122, 153, 77, 84, 18, 59,
];

/// Governed Consent planner that preserves the pure owner plan and atomically
/// adds one authoritative Party -> Consent access-path relationship on create.
///
/// Cross-owner existence, tenant ownership, Contact Point ownership and channel
/// compatibility are still validated by application composition before this
/// plan reaches the transactional executor. The relationship does not own
/// Consent semantics; it only prevents authorization reads from requiring a
/// tenant-wide scan or an eventually consistent projection.
#[derive(Debug, Default, Clone, Copy)]
pub struct ConsentCapabilityPlanner;

impl TransactionalAggregatePlanner for ConsentCapabilityPlanner {
    fn target(
        &self,
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
    ) -> Result<AggregateTarget, SdkError> {
        owner_planner::ConsentCapabilityPlanner.target(definition, request)
    }

    fn plan(
        &self,
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
        current: Option<&RecordSnapshot>,
    ) -> Result<CapabilityBatchExecutionPlan, SdkError> {
        let mut plan =
            owner_planner::ConsentCapabilityPlanner.plan(definition, request, current)?;
        if definition.capability_id.as_str() == CREATE_CAPABILITY {
            let scope = referenced_scope_from_create(request)?;
            let target = owner_planner::ConsentCapabilityPlanner
                .target(definition, request)?
                .reference;
            plan.batch.relationships.push(RelationshipMutation::Link {
                relationship: RelationshipRef {
                    relationship_type: configured_relationship_type()?,
                    source: RecordRef {
                        record_type: configured_record_type(
                            PARTY_AUTHORIZATION_SOURCE_RECORD_TYPE,
                        )?,
                        record_id: RecordId::try_new(scope.party_ref.as_str())
                            .map_err(config_error)?,
                    },
                    target,
                },
                payload: party_link_payload()?,
            });
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
            retention_policy_id: CONSENT_AUTHORIZATION_STATE_RETENTION_POLICY_ID,
        },
        DataClass::Personal,
        b"{}".to_vec(),
    )
}

fn configured_relationship_type() -> Result<RelationshipType, SdkError> {
    RelationshipType::try_new(PARTY_AUTHORIZATION_RELATIONSHIP_TYPE).map_err(config_error)
}

fn configured_record_type(value: &str) -> Result<RecordType, SdkError> {
    RecordType::try_new(value).map_err(config_error)
}

fn config_error(error: crm_module_sdk::IdentifierError) -> SdkError {
    SdkError::new(
        "CONSENTS_RELATIONSHIP_CONFIGURATION_INVALID",
        crm_module_sdk::ErrorCategory::Internal,
        false,
        "The Consent relationship access path is not configured safely.",
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
            PARTY_AUTHORIZATION_RELATIONSHIP_TYPE
        );
        assert_eq!(
            configured_record_type(PARTY_AUTHORIZATION_SOURCE_RECORD_TYPE)
                .unwrap()
                .as_str(),
            PARTY_AUTHORIZATION_SOURCE_RECORD_TYPE
        );
    }
}
