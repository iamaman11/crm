#![forbid(unsafe_code)]

//! Stable application boundary for Customer Privacy commands, queries, internal discovery and planning.
//!
//! Existing capability-specific crates remain behavior-owning transitional
//! implementation details. New Customer Privacy behavior enters through this
//! package rather than adding another command/query crate or a generic-runtime
//! dependency.

mod approval;
mod discovery;
mod planning;
mod reads {
    #[cfg(test)]
    use crm_customer_privacy::{ACTION_PLAN_GET_COORDINATE, OWNER_OUTCOMES_LIST_COORDINATE};

    include!("reads.rs");
}

pub use approval::*;
pub use discovery::*;
pub use planning::*;
pub use reads::*;

use crm_capability_runtime::CapabilityDefinition;
use crm_customer_privacy_cancel_capability_adapter::capability_definitions as cancel_definitions;
use crm_customer_privacy_capability_adapter::capability_definitions as case_create_definitions;
pub use crm_customer_privacy_capability_adapter::{
    PLACE_PROCESSING_RESTRICTION_CAPABILITY,
    place_processing_restriction_capability_definition,
};
use crm_customer_privacy_query_adapter::query_capability_definitions as case_query_definitions;
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
pub fn mutation_capability_definitions_with_restrictions(
) -> Result<Vec<CapabilityDefinition>, SdkError> {
    let mut definitions = mutation_capability_definitions()?;
    definitions.insert(1, place_processing_restriction_capability_definition()?);
    Ok(definitions)
}

pub fn query_capability_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {
    let mut definitions = case_query_definitions()?;
    definitions.extend(plan_read_query_capability_definitions()?);
    Ok(definitions)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inventories_preserve_legacy_five_and_enable_exact_step_four_six() {
        let legacy = mutation_capability_definitions().unwrap();
        let step_four = mutation_capability_definitions_with_restrictions().unwrap();
        let queries = query_capability_definitions().unwrap();
        assert_eq!(legacy.len(), 5);
        assert_eq!(step_four.len(), 6);
        assert_eq!(queries.len(), 4);
        assert!(!legacy.iter().any(|definition| {
            definition.capability_id.as_str() == PLACE_PROCESSING_RESTRICTION_CAPABILITY
        }));
        for capability in [
            APPROVE_PRIVACY_CASE_CAPABILITY,
            PLACE_PROCESSING_RESTRICTION_CAPABILITY,
        ] {
            assert!(step_four.iter().any(|definition| {
                definition.capability_id.as_str() == capability
            }));
        }
    }
}
