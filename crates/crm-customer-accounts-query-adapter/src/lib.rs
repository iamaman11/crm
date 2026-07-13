#![forbid(unsafe_code)]

//! Governed public query adapter boundary for the Account owner domain.
//!
//! Query execution and persistence remain adapters. The Account owner module
//! exposes domain invariants only and does not depend on cursor, authorization
//! or PostgreSQL concerns.

use crm_capability_plan_support as support;
use crm_capability_runtime::{CapabilityDefinition, CapabilityRisk};
use crm_module_sdk::{CapabilityId, CapabilityVersion, DataClass, ErrorCategory, ModuleId, SdkError};

pub const MODULE_ID: &str = "crm.customer-accounts";
pub const RECORD_TYPE: &str = "accounts.account";

pub const GET_CAPABILITY: &str = "accounts.account.get";
pub const LIST_CAPABILITY: &str = "accounts.account.list";

pub const GET_REQUEST_SCHEMA: &str = "crm.accounts.v1.GetAccountRequest";
pub const GET_RESPONSE_SCHEMA: &str = "crm.accounts.v1.GetAccountResponse";
pub const LIST_REQUEST_SCHEMA: &str = "crm.accounts.v1.ListAccountsRequest";
pub const LIST_RESPONSE_SCHEMA: &str = "crm.accounts.v1.ListAccountsResponse";

pub const QUERY_CAPABILITY_IDS: [&str; 2] = [GET_CAPABILITY, LIST_CAPABILITY];

pub fn query_capability_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {
    QUERY_CAPABILITY_IDS
        .into_iter()
        .map(query_capability_definition)
        .collect()
}

pub fn query_capability_definition(capability_id: &str) -> Result<CapabilityDefinition, SdkError> {
    let (input_schema, output_schema) = match capability_id {
        GET_CAPABILITY => (GET_REQUEST_SCHEMA, GET_RESPONSE_SCHEMA),
        LIST_CAPABILITY => (LIST_REQUEST_SCHEMA, LIST_RESPONSE_SCHEMA),
        _ => {
            return Err(configuration_error(
                "CUSTOMER_ACCOUNTS_QUERY_CAPABILITY_UNSUPPORTED",
                "The Account query capability is unsupported.",
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
        risk: CapabilityRisk::Low,
        mutation: false,
        requires_idempotency: false,
        requires_approval: false,
        authorization_policy_id: capability_id.to_owned(),
        rate_limit_policy_id: None,
    })
}

/// Production query adapter boundary. The concrete `QuerySemanticValidator` and
/// `QueryExecutor` implementation is added together with persisted Account JSON
/// decoding and signed cursor semantics in the next 8A.3a step.
#[derive(Debug, Default, Clone, Copy)]
pub struct AccountQueryAdapter;

fn configured<T>(value: Result<T, crm_module_sdk::IdentifierError>) -> Result<T, SdkError> {
    value.map_err(|error| {
        configuration_error(
            "CUSTOMER_ACCOUNTS_QUERY_CONFIGURATION_INVALID",
            "The Account query configuration is invalid.",
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
    fn publishes_exact_get_and_list_coordinates_as_personal_queries() {
        let definitions = query_capability_definitions().unwrap();
        assert_eq!(definitions.len(), 2);
        assert_eq!(definitions[0].capability_id.as_str(), GET_CAPABILITY);
        assert_eq!(definitions[1].capability_id.as_str(), LIST_CAPABILITY);
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
                    .expect("Account query output contract")
                    .allowed_data_classes,
                vec![DataClass::Personal]
            );
            assert!(!definition.mutation);
            assert!(!definition.requires_idempotency);
            assert!(!definition.requires_approval);
        }
    }

    #[test]
    fn rejects_unknown_account_query_coordinate() {
        let error = query_capability_definition("accounts.account.search").unwrap_err();
        assert_eq!(
            error.code,
            "CUSTOMER_ACCOUNTS_QUERY_CAPABILITY_UNSUPPORTED"
        );
    }
}
