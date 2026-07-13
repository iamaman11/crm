#![forbid(unsafe_code)]

//! Governed public mutation adapter for the Contact Point owner domain.
//!
//! The pure aggregate remains inside `crm-contact-points`; this crate binds
//! exact published mutation contracts to the transactional planning path.

mod planner;

pub use planner::*;

use crm_capability_plan_support as support;
use crm_capability_runtime::{CapabilityDefinition, CapabilityRisk};
use crm_module_sdk::{
    CapabilityId, CapabilityVersion, DataClass, ErrorCategory, ModuleId, SdkError,
};

pub const MODULE_ID: &str = "crm.contact-points";
pub const RECORD_TYPE: &str = "contact-points.contact_point";

pub const CREATE_CAPABILITY: &str = "contact-points.contact-point.create";
pub const UPDATE_CAPABILITY: &str = "contact-points.contact-point.update";
pub const VERIFY_CAPABILITY: &str = "contact-points.contact-point.verify";

pub const CREATE_REQUEST_SCHEMA: &str = "crm.contact_points.v1.CreateContactPointRequest";
pub const CREATE_RESPONSE_SCHEMA: &str = "crm.contact_points.v1.CreateContactPointResponse";
pub const UPDATE_REQUEST_SCHEMA: &str = "crm.contact_points.v1.UpdateContactPointRequest";
pub const UPDATE_RESPONSE_SCHEMA: &str = "crm.contact_points.v1.UpdateContactPointResponse";
pub const VERIFY_REQUEST_SCHEMA: &str = "crm.contact_points.v1.VerifyContactPointRequest";
pub const VERIFY_RESPONSE_SCHEMA: &str = "crm.contact_points.v1.VerifyContactPointResponse";

pub const CREATED_EVENT_TYPE: &str = "contact-points.contact-point.created";
pub const CREATED_EVENT_SCHEMA: &str = "crm.contact_points.v1.ContactPointCreatedEvent";
pub const UPDATED_EVENT_TYPE: &str = "contact-points.contact-point.updated";
pub const UPDATED_EVENT_SCHEMA: &str = "crm.contact_points.v1.ContactPointUpdatedEvent";
pub const VERIFIED_EVENT_TYPE: &str = "contact-points.contact-point.verified";
pub const VERIFIED_EVENT_SCHEMA: &str = "crm.contact_points.v1.ContactPointVerifiedEvent";

pub const MUTATION_CAPABILITY_IDS: [&str; 3] =
    [CREATE_CAPABILITY, UPDATE_CAPABILITY, VERIFY_CAPABILITY];

pub fn capability_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {
    MUTATION_CAPABILITY_IDS
        .into_iter()
        .map(capability_definition)
        .collect()
}

pub fn capability_definition(capability_id: &str) -> Result<CapabilityDefinition, SdkError> {
    let (input_schema, output_schema) = match capability_id {
        CREATE_CAPABILITY => (CREATE_REQUEST_SCHEMA, CREATE_RESPONSE_SCHEMA),
        UPDATE_CAPABILITY => (UPDATE_REQUEST_SCHEMA, UPDATE_RESPONSE_SCHEMA),
        VERIFY_CAPABILITY => (VERIFY_REQUEST_SCHEMA, VERIFY_RESPONSE_SCHEMA),
        _ => {
            return Err(configuration_error(
                "CONTACT_POINTS_CAPABILITY_UNSUPPORTED",
                "The Contact Point mutation capability is unsupported.",
            ));
        }
    };

    Ok(CapabilityDefinition {
        capability_id: configured(CapabilityId::try_new(capability_id))?,
        capability_version: configured(CapabilityVersion::try_new(support::CONTRACT_VERSION))?,
        owner_module_id: configured(ModuleId::try_new(MODULE_ID))?,
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

fn configured<T>(value: Result<T, crm_module_sdk::IdentifierError>) -> Result<T, SdkError> {
    value.map_err(|error| {
        configuration_error(
            "CONTACT_POINTS_CONFIGURATION_INVALID",
            "The Contact Point capability configuration is invalid.",
        )
        .with_internal_reference(error.to_string())
    })
}

fn configuration_error(code: &'static str, safe_message: &'static str) -> SdkError {
    SdkError::new(code, ErrorCategory::Internal, false, safe_message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn publishes_exact_create_update_and_verify_coordinates_as_personal_mutations() {
        let definitions = capability_definitions().unwrap();
        assert_eq!(definitions.len(), 3);
        assert_eq!(
            definitions
                .iter()
                .map(|definition| definition.capability_id.as_str())
                .collect::<Vec<_>>(),
            MUTATION_CAPABILITY_IDS
        );
        for definition in definitions {
            assert_eq!(definition.owner_module_id.as_str(), MODULE_ID);
            assert_eq!(
                definition.capability_version.as_str(),
                support::CONTRACT_VERSION
            );
            assert_eq!(
                definition.input_contract.allowed_data_classes,
                vec![DataClass::Personal]
            );
            assert_eq!(
                definition
                    .output_contract
                    .as_ref()
                    .expect("Contact Point output contract")
                    .allowed_data_classes,
                vec![DataClass::Personal]
            );
            assert!(definition.mutation);
            assert!(definition.requires_idempotency);
            assert!(!definition.requires_approval);
        }
    }

    #[test]
    fn rejects_unknown_contact_point_mutation_coordinate() {
        let error = capability_definition("contact-points.contact-point.delete").unwrap_err();
        assert_eq!(error.code, "CONTACT_POINTS_CAPABILITY_UNSUPPORTED");
    }
}
