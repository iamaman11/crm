#![forbid(unsafe_code)]

//! Stable application boundary for Customer Privacy commands and queries.
//!
//! Existing capability-specific crates remain behavior-owning transitional
//! implementation details. New Customer Privacy behavior must enter through this
//! package rather than adding another command/query crate or a generic-runtime
//! dependency.

use crm_capability_runtime::CapabilityDefinition;
use crm_customer_privacy_cancel_capability_adapter::capability_definitions as cancel_definitions;
use crm_customer_privacy_capability_adapter::capability_definitions as create_definitions;
use crm_customer_privacy_query_adapter::query_capability_definitions as case_query_definitions;
use crm_customer_privacy_subject_capability_adapter::capability_definitions as subject_definitions;
use crm_customer_privacy_submit_capability_adapter::capability_definitions as submit_definitions;
use crm_module_sdk::SdkError;

pub fn mutation_capability_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {
    let mut definitions = create_definitions()?;
    definitions.extend(submit_definitions()?);
    definitions.extend(subject_definitions()?);
    definitions.extend(cancel_definitions()?);
    Ok(definitions)
}

pub fn query_capability_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {
    case_query_definitions()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inventory_preserves_four_mutations_and_two_queries() {
        let mutations = mutation_capability_definitions().unwrap();
        let queries = query_capability_definitions().unwrap();
        assert_eq!(mutations.len(), 4);
        assert_eq!(queries.len(), 2);
    }
}
