#![forbid(unsafe_code)]

//! Stable application boundary for Customer Privacy commands, queries, internal discovery, planning,
//! retention adjudication and governed access/export assembly.
//!
//! Existing capability-specific crates remain behavior-owning transitional
//! implementation details. New Customer Privacy behavior enters through this
//! package rather than adding another command/query crate or a generic-runtime
//! dependency.

mod access_export;
mod approval;
mod discovery;
mod execution;
mod planning;
mod retention;
mod reads {
    #[cfg(test)]
    use crm_customer_privacy::{ACTION_PLAN_GET_COORDINATE, OWNER_OUTCOMES_LIST_COORDINATE};

    include!("reads.rs");
}

pub use access_export::*;
pub use approval::*;
pub use discovery::*;
pub use execution::*;
pub use planning::*;
pub use reads::*;
pub use retention::*;

use crm_capability_runtime::CapabilityDefinition;
use crm_customer_privacy_cancel_capability_adapter::capability_definitions as cancel_definitions;
use crm_customer_privacy_capability_adapter::capability_definitions as case_create_definitions;
pub use crm_customer_privacy_capability_adapter::{
    PLACE_CUSTOMER_DATA_LEGAL_HOLD_CAPABILITY, PLACE_PROCESSING_RESTRICTION_CAPABILITY,
    RELEASE_CUSTOMER_DATA_LEGAL_HOLD_CAPABILITY, RELEASE_PROCESSING_RESTRICTION_CAPABILITY,
    place_customer_data_legal_hold_capability_definition,
    place_processing_restriction_capability_definition,
    release_customer_data_legal_hold_capability_definition,
    release_processing_restriction_capability_definition,
};
use crm_customer_privacy_query_adapter::{
    control_query_capability_definitions, query_capability_definitions as case_query_definitions,
};
use crm_customer_privacy_subject_capability_adapter::capability_definitions as subject_definitions;
use crm_customer_privacy_submit_capability_adapter::capability_definitions as submit_definitions;
use crm_module_sdk::SdkError;

/// Accepted pre-step-four inventory retained for legacy production composition.
pub fn mutation_capability_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {
    let mut definitions = case_create_definitions()?;
    definitions.extend(submit_definitions()?);
    definitions.extend(subject_definitions()?);
    definitions.push(approval_capability_definition()?);
    definitions.extend(cancel_definitions()?);
    Ok(definitions)
}

/// Exact production inventory once immediate restriction placement is enabled.
pub fn mutation_capability_definitions_with_restrictions()
-> Result<Vec<CapabilityDefinition>, SdkError> {
    let mut definitions = mutation_capability_definitions()?;
    definitions.insert(1, place_processing_restriction_capability_definition()?);
    Ok(definitions)
}

/// Exact step-six production inventory with both deny directives.
pub fn mutation_capability_definitions_with_restrictions_and_legal_holds()
-> Result<Vec<CapabilityDefinition>, SdkError> {
    let mut definitions = mutation_capability_definitions_with_restrictions()?;
    definitions.insert(2, place_customer_data_legal_hold_capability_definition()?);
    Ok(definitions)
}

/// Frozen Phase 8A public mutation inventory with complete control release lifecycle.
pub fn mutation_capability_definitions_with_complete_control_lifecycle()
-> Result<Vec<CapabilityDefinition>, SdkError> {
    let mut definitions = mutation_capability_definitions_with_restrictions_and_legal_holds()?;
    definitions.insert(2, release_processing_restriction_capability_definition()?);
    definitions.insert(4, release_customer_data_legal_hold_capability_definition()?);
    Ok(definitions)
}

/// Accepted four-query inventory retained for legacy production composition.
pub fn query_capability_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {
    let mut definitions = case_query_definitions()?;
    definitions.extend(plan_read_query_capability_definitions()?);
    Ok(definitions)
}

/// Frozen Phase 8A inventory with the three permission-aware control reads.
pub fn query_capability_definitions_with_complete_control_lifecycle()
-> Result<Vec<CapabilityDefinition>, SdkError> {
    let mut definitions = case_query_definitions()?;
    definitions.extend(control_query_capability_definitions()?);
    definitions.extend(plan_read_query_capability_definitions()?);
    Ok(definitions)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inventories_preserve_legacy_and_complete_frozen_control_lifecycle() {
        let legacy = mutation_capability_definitions().unwrap();
        let step_four = mutation_capability_definitions_with_restrictions().unwrap();
        let step_six = mutation_capability_definitions_with_restrictions_and_legal_holds().unwrap();
        let complete = mutation_capability_definitions_with_complete_control_lifecycle().unwrap();
        let legacy_queries = query_capability_definitions().unwrap();
        let complete_queries =
            query_capability_definitions_with_complete_control_lifecycle().unwrap();
        assert_eq!(legacy.len(), 5);
        assert_eq!(step_four.len(), 6);
        assert_eq!(step_six.len(), 7);
        assert_eq!(complete.len(), 9);
        assert_eq!(legacy_queries.len(), 4);
        assert_eq!(complete_queries.len(), 7);
        assert_eq!(
            complete
                .iter()
                .map(|definition| definition.capability_id.as_str())
                .collect::<Vec<_>>(),
            vec![
                "customer_privacy.case.create",
                PLACE_PROCESSING_RESTRICTION_CAPABILITY,
                RELEASE_PROCESSING_RESTRICTION_CAPABILITY,
                PLACE_CUSTOMER_DATA_LEGAL_HOLD_CAPABILITY,
                RELEASE_CUSTOMER_DATA_LEGAL_HOLD_CAPABILITY,
                "customer_privacy.case.submit",
                "customer_privacy.case.subject.verify",
                "customer_privacy.case.approve",
                "customer_privacy.case.cancel",
            ]
        );
        assert!(!legacy.iter().any(|definition| {
            matches!(
                definition.capability_id.as_str(),
                PLACE_PROCESSING_RESTRICTION_CAPABILITY
                    | RELEASE_PROCESSING_RESTRICTION_CAPABILITY
                    | PLACE_CUSTOMER_DATA_LEGAL_HOLD_CAPABILITY
                    | RELEASE_CUSTOMER_DATA_LEGAL_HOLD_CAPABILITY
            )
        }));
    }
}
