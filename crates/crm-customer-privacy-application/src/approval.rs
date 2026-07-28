use crm_capability_plan_support as support;
use crm_capability_runtime::{CapabilityDefinition, CapabilityRisk};
use crm_customer_privacy::MODULE_ID;
use crm_module_sdk::{CapabilityId, CapabilityVersion, DataClass, ModuleId, SdkError};

pub const APPROVE_PRIVACY_CASE_CAPABILITY: &str = "customer_privacy.case.approve";
pub const APPROVE_PRIVACY_CASE_REQUEST_SCHEMA: &str =
    "crm.customer_privacy.v1.ApprovePrivacyCaseRequest";
pub const APPROVE_PRIVACY_CASE_RESPONSE_SCHEMA: &str =
    "crm.customer_privacy.v1.ApprovePrivacyCaseResponse";

pub fn approval_capability_definition() -> Result<CapabilityDefinition, SdkError> {
    Ok(CapabilityDefinition {
        capability_id: configured(CapabilityId::try_new(APPROVE_PRIVACY_CASE_CAPABILITY))?,
        capability_version: configured(CapabilityVersion::try_new(support::CONTRACT_VERSION))?,
        owner_module_id: configured(ModuleId::try_new(MODULE_ID))?,
        input_contract: support::protobuf_contract(
            MODULE_ID,
            APPROVE_PRIVACY_CASE_REQUEST_SCHEMA,
            vec![DataClass::Confidential],
        )?,
        output_contract: Some(support::protobuf_contract(
            MODULE_ID,
            APPROVE_PRIVACY_CASE_RESPONSE_SCHEMA,
            vec![DataClass::Confidential],
        )?),
        risk: CapabilityRisk::High,
        mutation: true,
        requires_idempotency: true,
        requires_approval: false,
        authorization_policy_id: APPROVE_PRIVACY_CASE_CAPABILITY.to_owned(),
        rate_limit_policy_id: None,
    })
}

fn configured<T>(value: Result<T, crm_module_sdk::IdentifierError>) -> Result<T, SdkError> {
    value.map_err(|error| {
        crm_module_sdk::SdkError::new(
            "CUSTOMER_PRIVACY_CASE_APPROVAL_CONFIGURATION_INVALID",
            crm_module_sdk::ErrorCategory::Internal,
            false,
            "The privacy case approval capability is not configured safely.",
        )
        .with_internal_reference(error.to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn approval_definition_is_exact_high_risk_idempotent_mutation() {
        let definition = approval_capability_definition().unwrap();
        assert_eq!(
            definition.capability_id.as_str(),
            APPROVE_PRIVACY_CASE_CAPABILITY
        );
        assert_eq!(definition.capability_version.as_str(), "1.0.0");
        assert_eq!(definition.owner_module_id.as_str(), MODULE_ID);
        assert!(definition.mutation);
        assert!(definition.requires_idempotency);
        assert!(!definition.requires_approval);
        assert_eq!(definition.risk, CapabilityRisk::High);
    }
}
