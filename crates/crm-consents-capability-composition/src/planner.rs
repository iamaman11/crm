use crm_capability_plan_support::{self as support, PersistedPayloadContract};
use crm_capability_runtime::{CapabilityDefinition, CapabilityRequest};
use crm_consents::CONSENT_AUTHORIZATION_STATE_RETENTION_POLICY_ID;
use crm_consents_capability_adapter::{
    CREATE_CAPABILITY, ConsentCapabilityPlanner, MODULE_ID, referenced_scope_from_create,
};
use crm_core_data::{
    AggregateTarget, CapabilityBatchExecutionPlan, RelationshipMutation,
    TransactionalAggregatePlanner,
};
use crm_module_sdk::{
    DataClass, RecordRef, RecordSnapshot, RecordType, RelationshipRef, RelationshipType, SdkError,
};
use crm_parties_capability_adapter::RECORD_TYPE as PARTY_RECORD_TYPE;
use sha2::{Digest, Sha256};

pub const PARTY_AUTHORIZATION_RELATIONSHIP_TYPE: &str = "consents.authorization.party";
const PARTY_LINK_SCHEMA_ID: &str = "crm.consents.authorization.party-link";
const PARTY_LINK_SCHEMA_VERSION: &str = "1.0.0";
const PARTY_LINK_MAXIMUM_BYTES: u64 = 1_024;
const PARTY_LINK_DESCRIPTOR: &[u8] = b"crm.consents.authorization.party-link/v1:{}";

/// Pure composition planner that decorates an owner-authored Consent create plan
/// with one authoritative Party -> Consent relationship in the same PostgreSQL batch.
///
/// The owner aggregate remains the source of truth for Consent semantics. The
/// relationship is only an authoritative access path that narrows decision reads to
/// one Party without an eventual-consistency projection or a tenant-wide scan.
#[derive(Debug, Default, Clone, Copy)]
pub struct ConsentCapabilityCompositionPlanner;

impl TransactionalAggregatePlanner for ConsentCapabilityCompositionPlanner {
    fn target(
        &self,
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
    ) -> Result<AggregateTarget, SdkError> {
        ConsentCapabilityPlanner.target(definition, request)
    }

    fn plan(
        &self,
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
        current: Option<&RecordSnapshot>,
    ) -> Result<CapabilityBatchExecutionPlan, SdkError> {
        let mut plan = ConsentCapabilityPlanner.plan(definition, request, current)?;
        if definition.capability_id.as_str() == CREATE_CAPABILITY {
            let scope = referenced_scope_from_create(request)?;
            let target = ConsentCapabilityPlanner.target(definition, request)?.reference;
            plan.batch.relationships.push(RelationshipMutation::Link {
                relationship: RelationshipRef {
                    relationship_type: configured_relationship_type()?,
                    source: RecordRef {
                        record_type: configured_record_type(PARTY_RECORD_TYPE)?,
                        record_id: crm_module_sdk::RecordId::try_new(scope.party_ref.as_str())
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
            descriptor_hash: Sha256::digest(PARTY_LINK_DESCRIPTOR).into(),
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
        "The Consent relationship composition is not configured safely.",
    )
    .with_internal_reference(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn party_link_contract_is_personal_json_and_nonzero_hashed() {
        let payload = party_link_payload().unwrap();
        assert_eq!(payload.owner.as_str(), MODULE_ID);
        assert_eq!(payload.schema_id.as_str(), PARTY_LINK_SCHEMA_ID);
        assert_eq!(payload.data_class, DataClass::Personal);
        assert_eq!(payload.encoding, crm_module_sdk::PayloadEncoding::Json);
        assert_eq!(payload.bytes, b"{}");
        assert_ne!(payload.descriptor_hash, [0; 32]);
    }

    #[test]
    fn relationship_coordinate_is_stable() {
        assert_eq!(
            configured_relationship_type().unwrap().as_str(),
            PARTY_AUTHORIZATION_RELATIONSHIP_TYPE
        );
        assert_eq!(
            configured_record_type(PARTY_RECORD_TYPE).unwrap().as_str(),
            PARTY_RECORD_TYPE
        );
    }
}
