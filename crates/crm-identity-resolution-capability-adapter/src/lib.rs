#![forbid(unsafe_code)]

//! Governed mutation adapter for authoritative Identity Resolution candidate cases.
//!
//! The pure owner remains free of SQL, transport types and direct Party storage
//! access. Same-tenant Party existence and exact authoritative source-version
//! checks are composed outside this crate before execution.

mod merge_owner_planner;
mod planner;

pub use merge_owner_planner::*;
pub use planner::*;

use crm_capability_plan_support as support;
use crm_capability_runtime::{CapabilityDefinition, CapabilityRisk};
use crm_module_sdk::{
    CapabilityId, CapabilityVersion, DataClass, ErrorCategory, ModuleId, SdkError,
};

pub const MODULE_ID: &str = "crm.identity-resolution";
pub const RECORD_TYPE: &str = "identity_resolution.candidate_case";

pub const REGISTER_CAPABILITY: &str = "identity_resolution.candidate.register";
pub const REFRESH_CAPABILITY: &str = "identity_resolution.candidate.evidence.refresh";
pub const DISMISS_CAPABILITY: &str = "identity_resolution.candidate.dismiss";
pub const CONFIRM_CAPABILITY: &str = "identity_resolution.candidate.confirm_duplicate";

pub const REGISTER_REQUEST_SCHEMA: &str =
    "crm.identity_resolution.v1.RegisterDuplicateCandidateRequest";
pub const REGISTER_RESPONSE_SCHEMA: &str =
    "crm.identity_resolution.v1.RegisterDuplicateCandidateResponse";
pub const REFRESH_REQUEST_SCHEMA: &str =
    "crm.identity_resolution.v1.RefreshDuplicateCandidateEvidenceRequest";
pub const REFRESH_RESPONSE_SCHEMA: &str =
    "crm.identity_resolution.v1.RefreshDuplicateCandidateEvidenceResponse";
pub const DISMISS_REQUEST_SCHEMA: &str =
    "crm.identity_resolution.v1.DismissDuplicateCandidateRequest";
pub const DISMISS_RESPONSE_SCHEMA: &str =
    "crm.identity_resolution.v1.DismissDuplicateCandidateResponse";
pub const CONFIRM_REQUEST_SCHEMA: &str =
    "crm.identity_resolution.v1.ConfirmDuplicateCandidateRequest";
pub const CONFIRM_RESPONSE_SCHEMA: &str =
    "crm.identity_resolution.v1.ConfirmDuplicateCandidateResponse";

pub const REGISTERED_EVENT_TYPE: &str = "identity_resolution.candidate.registered";
pub const REGISTERED_EVENT_SCHEMA: &str =
    "crm.identity_resolution.v1.DuplicateCandidateRegisteredEvent";
pub const REFRESHED_EVENT_TYPE: &str = "identity_resolution.candidate.evidence_refreshed";
pub const REFRESHED_EVENT_SCHEMA: &str =
    "crm.identity_resolution.v1.DuplicateCandidateEvidenceRefreshedEvent";
pub const DISMISSED_EVENT_TYPE: &str = "identity_resolution.candidate.dismissed";
pub const DISMISSED_EVENT_SCHEMA: &str =
    "crm.identity_resolution.v1.DuplicateCandidateDismissedEvent";
pub const CONFIRMED_EVENT_TYPE: &str = "identity_resolution.candidate.confirmed_duplicate";
pub const CONFIRMED_EVENT_SCHEMA: &str =
    "crm.identity_resolution.v1.DuplicateCandidateConfirmedEvent";

pub const MUTATION_CAPABILITY_IDS: [&str; 4] = [
    REGISTER_CAPABILITY,
    REFRESH_CAPABILITY,
    DISMISS_CAPABILITY,
    CONFIRM_CAPABILITY,
];

pub fn capability_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {
    MUTATION_CAPABILITY_IDS
        .into_iter()
        .map(capability_definition)
        .collect()
}

pub fn capability_definition(capability_id: &str) -> Result<CapabilityDefinition, SdkError> {
    let (input_schema, output_schema, risk) = match capability_id {
        REGISTER_CAPABILITY => (
            REGISTER_REQUEST_SCHEMA,
            REGISTER_RESPONSE_SCHEMA,
            CapabilityRisk::Medium,
        ),
        REFRESH_CAPABILITY => (
            REFRESH_REQUEST_SCHEMA,
            REFRESH_RESPONSE_SCHEMA,
            CapabilityRisk::Medium,
        ),
        DISMISS_CAPABILITY => (
            DISMISS_REQUEST_SCHEMA,
            DISMISS_RESPONSE_SCHEMA,
            CapabilityRisk::High,
        ),
        CONFIRM_CAPABILITY => (
            CONFIRM_REQUEST_SCHEMA,
            CONFIRM_RESPONSE_SCHEMA,
            CapabilityRisk::High,
        ),
        _ => {
            return Err(configuration_error(
                "IDENTITY_RESOLUTION_CAPABILITY_UNSUPPORTED",
                "The Identity Resolution mutation capability is unsupported.",
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
            "IDENTITY_RESOLUTION_CONFIGURATION_INVALID",
            "The Identity Resolution capability configuration is invalid.",
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
    fn publishes_exact_mutation_coordinates_as_personal_idempotent_mutations() {
        let definitions = capability_definitions().unwrap();
        assert_eq!(definitions.len(), 4);
        assert_eq!(definitions[0].capability_id.as_str(), REGISTER_CAPABILITY);
        assert_eq!(definitions[1].capability_id.as_str(), REFRESH_CAPABILITY);
        assert_eq!(definitions[2].capability_id.as_str(), DISMISS_CAPABILITY);
        assert_eq!(definitions[3].capability_id.as_str(), CONFIRM_CAPABILITY);
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
    fn rejects_unknown_identity_resolution_mutation_coordinate() {
        let error = capability_definition("identity_resolution.candidate.merge").unwrap_err();
        assert_eq!(error.code, "IDENTITY_RESOLUTION_CAPABILITY_UNSUPPORTED");
    }
}
