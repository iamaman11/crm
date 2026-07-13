#![forbid(unsafe_code)]

//! Governed public mutation adapter for the Account owner domain.
//!
//! The pure Account aggregate remains inside `crm-customer-accounts`; this crate
//! binds exact published mutation contracts to the transactional planning path.

mod planner;

pub use planner::*;

use crm_capability_plan_support as support;
use crm_capability_runtime::{CapabilityDefinition, CapabilityRisk};
use crm_module_sdk::{CapabilityId, CapabilityVersion, DataClass, ErrorCategory, ModuleId, SdkError};

pub const MODULE_ID: &str = "crm.customer-accounts";
pub const RECORD_TYPE: &str = "accounts.account";

pub const CREATE_CAPABILITY: &str = "accounts.account.create";
pub const UPDATE_CAPABILITY: &str = "accounts.account.update";

pub const CREATE_REQUEST_SCHEMA: &str = "crm.accounts.v1.CreateAccountRequest";
pub const CREATE_RESPONSE_SCHEMA: &str = "crm.accounts.v1.CreateAccountResponse";
pub const UPDATE_REQUEST_SCHEMA: &str = "crm.accounts.v1.UpdateAccountRequest";
pub const UPDATE_RESPONSE_SCHEMA: &str = "crm.accounts.v1.UpdateAccountResponse";

pub const CREATED_EVENT_TYPE: &str = "accounts.account.created";
pub const CREATED_EVENT_SCHEMA: &str = "crm.accounts.v1.AccountCreatedEvent";
pub const UPDATED_EVENT_TYPE: &str = "accounts.account.updated";
pub const UPDATED_EVENT_SCHEMA: &str = "crm.accounts.v1.AccountUpdatedEvent";

pub const MUTATION_CAPABILITY_IDS: [&str; 2] = [CREATE_CAPABILITY, UPDATE_CAPABILITY];

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
        _ => {
            return Err(configuration_error(
                "CUSTOMER_ACCOUNTS_CAPABILITY_UNSUPPORTED",
                "The Account mutation capability is unsupported.",
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
            "CUSTOMER_ACCOUNTS_CONFIGURATION_INVALID",
            "The Account capability configuration is invalid.",
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
    fn publishes_exact_create_and_update_coordinates_as_personal_mutations() {
        let definitions = capability_definitions().unwrap();
        assert_eq!(definitions.len(), 2);
        assert_eq!(definitions[0].capability_id.as_str(), CREATE_CAPABILITY);
        assert_eq!(definitions[1].capability_id.as_str(), UPDATE_CAPABILITY);
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
                    .expect("Account output contract")
                    .allowed_data_classes,
                vec![DataClass::Personal]
            );
            assert!(definition.mutation);
            assert!(definition.requires_idempotency);
            assert!(!definition.requires_approval);
        }
    }

    #[test]
    fn rejects_unknown_account_mutation_coordinate() {
        let error = capability_definition("accounts.account.delete").unwrap_err();
        assert_eq!(error.code, "CUSTOMER_ACCOUNTS_CAPABILITY_UNSUPPORTED");
    }
}
