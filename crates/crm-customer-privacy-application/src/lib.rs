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
mod restriction;
mod reads {
    #[cfg(test)]
    use crm_customer_privacy::{ACTION_PLAN_GET_COORDINATE, OWNER_OUTCOMES_LIST_COORDINATE};

    include!("reads.rs");
}

pub use approval::*;
pub use discovery::*;
pub use planning::*;
pub use reads::*;
pub use restriction::*;

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
    definitions.push(approval_capability_definition()?);
    definitions.extend(cancel_definitions()?);
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
    fn inventory_promotes_five_mutations_and_preserves_four_queries() {
        let mutations = mutation_capability_definitions().unwrap();
        let queries = query_capability_definitions().unwrap();
        assert_eq!(mutations.len(), 5);
        assert_eq!(queries.len(), 4);
        assert!(mutations.iter().any(|definition| {
            definition.capability_id.as_str() == APPROVE_PRIVACY_CASE_CAPABILITY
        }));
    }
}
