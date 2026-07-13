#![forbid(unsafe_code)]

//! Governed mutation adapter for the authoritative Consent owner domain.
//!
//! Cross-owner Party and Contact Point integrity remains in application
//! composition. This crate only binds exact published contracts to the pure
//! `crm-consents` aggregate and transactional mutation planning path.

mod planner;

pub use planner::*;

use crm_capability_plan_support as support;
use crm_capability_runtime::{CapabilityDefinition, CapabilityRisk};
use crm_module_sdk::{
    CapabilityId, CapabilityVersion, DataClass, ErrorCategory, ModuleId, SdkError,
};

pub const MODULE_ID: &str = "crm.consents";
pub const RECORD_TYPE: &str = "consents.authorization";

pub const CREATE_CAPABILITY: &str = "consents.authorization.create";
pub const WITHDRAW_CAPABILITY: &str = "consents.authorization.withdraw";

pub const CREATE_REQUEST_SCHEMA: &str = "crm.consents.v1.CreateConsentAuthorizationRequest";
pub const CREATE_RESPONSE_SCHEMA: &str = "crm.consents.v1.CreateConsentAuthorizationResponse";
pub const WITHDRAW_REQUEST_SCHEMA: &str = "crm.consents.v1.WithdrawConsentAuthorizationRequest";
pub const WITHDRAW_RESPONSE_SCHEMA: &str =
    "crm.consents.v1.WithdrawConsentAuthorizationResponse";

pub const CREATED_EVENT_TYPE: &str = "consents.authorization.created";
pub const CREATED_EVENT_SCHEMA: &str = "crm.consents.v1.ConsentAuthorizationCreatedEvent";
pub const WITHDRAWN_EVENT_TYPE: &str = "consents.authorization.withdrawn";
pub const WITHDRAWN_EVENT_SCHEMA: &str = "crm.consents.v1.ConsentAuthorizationWithdrawnEvent";

pub const MUTATION_CAPABILITY_IDS: [&str; 2] = [CREATE_CAPABILITY, WITHDRAW_CAPABILITY];

pub fn capability_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {
    MUTATION_CAPABILITY_IDS
        .into_iter()
        .map(capability_definition)
        .collect()
}

pub fn capability_definition(capability_id: &str) -> Result<CapabilityDefinition, SdkError> {
    let (input_schema, output_schema, risk) = match capability_id {
        CREATE_CAPABILITY => (
            CREATE_REQUEST_SCHEMA,
            CREATE_RESPONSE_SCHEMA,
            CapabilityRisk::Medium,
        ),
        WITHDRAW_CAPABILITY => (
            WITHDRAW_REQUEST_SCHEMA,
            WITHDRAW_RESPONSE_SCHEMA,
            CapabilityRisk::High,
        ),
        _ => {
            return Err(configuration_error(
                "CONSENTS_CAPABILITY_UNSUPPORTED",
                "The Consent mutation capability is unsupported.",
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
        risk,
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
            "CONSENTS_CONFIGURATION_INVALID",
            "The Consent capability configuration is invalid.",
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
    fn publishes_exact_create_and_withdraw_coordinates_as_personal_mutations() {
        let definitions = capability_definitions().unwrap();
        assert_eq!(definitions.len(), 2);
        assert_eq!(definitions[0].capability_id.as_str(), CREATE_CAPABILITY);
        assert_eq!(definitions[1].capability_id.as_str(), WITHDRAW_CAPABILITY);
        assert_eq!(definitions[0].risk, CapabilityRisk::Medium);
        assert_eq!(definitions[1].risk, CapabilityRisk::High);
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
            assert!(definition.mutation);
            assert!(definition.requires_idempotency);
            assert!(!definition.requires_approval);
        }
    }

    #[test]
    fn rejects_unknown_consent_mutation_coordinate() {
        let error = capability_definition("consents.authorization.delete").unwrap_err();
        assert_eq!(error.code, "CONSENTS_CAPABILITY_UNSUPPORTED");
    }
}
