#![forbid(unsafe_code)]

#[allow(dead_code)]
#[path = "lib.rs"]
mod case_create;
mod control_release;
mod legal_hold;
mod restriction;

pub use case_create::{
    CREATE_PRIVACY_CASE_CAPABILITY, CREATE_PRIVACY_CASE_REQUEST_SCHEMA,
    CREATE_PRIVACY_CASE_RESPONSE_SCHEMA, CustomerPrivacyCaseCreateCapabilityPlanner,
    CustomerPrivacyCasePreviousReferencePlanner, PRIVACY_CASE_CREATED_EVENT_SCHEMA,
    PRIVACY_CASE_CREATED_EVENT_TYPE, capability_definition, deterministic_privacy_case_id,
    previous_case_id_from_request, previous_case_not_found, privacy_case_from_create_request,
    privacy_case_ref_from_id, validate_previous_case_snapshot,
};
pub use control_release::*;
pub use legal_hold::*;
pub use restriction::*;

use crm_capability_runtime::CapabilityDefinition;
use crm_module_sdk::SdkError;

pub const IMPLEMENTED_MUTATION_CAPABILITY_IDS: &[&str] = &[
    CREATE_PRIVACY_CASE_CAPABILITY,
    PLACE_PROCESSING_RESTRICTION_CAPABILITY,
    RELEASE_PROCESSING_RESTRICTION_CAPABILITY,
    PLACE_CUSTOMER_DATA_LEGAL_HOLD_CAPABILITY,
    RELEASE_CUSTOMER_DATA_LEGAL_HOLD_CAPABILITY,
];

/// Preserves the accepted case-create contribution for legacy callers while
/// later production boundaries add governed Customer Privacy controls explicitly.
pub fn capability_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {
    Ok(vec![capability_definition()?])
}

pub fn implemented_capability_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {
    Ok(vec![
        capability_definition()?,
        place_processing_restriction_capability_definition()?,
        release_processing_restriction_capability_definition()?,
        place_customer_data_legal_hold_capability_definition()?,
        release_customer_data_legal_hold_capability_definition()?,
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn implemented_inventory_contains_case_create_and_complete_control_mutations() {
        let definitions = implemented_capability_definitions().unwrap();
        assert_eq!(definitions.len(), 5);
        assert_eq!(
            definitions
                .iter()
                .map(|definition| definition.capability_id.as_str())
                .collect::<Vec<_>>(),
            IMPLEMENTED_MUTATION_CAPABILITY_IDS
        );
        assert_eq!(capability_definitions().unwrap().len(), 1);
    }
}
