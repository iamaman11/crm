use crate::bootstrap_visibility as base;
use crm_capability_runtime::CapabilityDefinition;
use crm_customer_enrichment_suggestion_query_adapter::{
    GET_SUGGESTION_CAPABILITY, LIST_SUGGESTIONS_BY_PARTY_CAPABILITY,
};
use crm_module_sdk::{CapabilityId, SdkError};

pub(crate) use base::BootstrapVisibilityResource;

#[derive(Debug, Clone)]
pub(crate) struct BootstrapVisibilityRegistry {
    base: base::BootstrapVisibilityRegistry,
}

impl BootstrapVisibilityRegistry {
    pub fn resources_for(
        &self,
        definition: &CapabilityDefinition,
    ) -> Result<Vec<BootstrapVisibilityResource>, SdkError> {
        if definition.capability_id.as_str() == LIST_SUGGESTIONS_BY_PARTY_CAPABILITY {
            let mut equivalent = definition.clone();
            equivalent.capability_id =
                CapabilityId::try_new(GET_SUGGESTION_CAPABILITY).map_err(|error| {
                    SdkError::invalid_argument(
                        "customer_enrichment.suggestion.list.bootstrap_visibility",
                        error.to_string(),
                    )
                })?;
            return self.base.resources_for(&equivalent);
        }
        self.base.resources_for(definition)
    }
}

pub(crate) fn build_bootstrap_visibility_registry(
) -> Result<BootstrapVisibilityRegistry, SdkError> {
    Ok(BootstrapVisibilityRegistry {
        base: base::build_bootstrap_visibility_registry()?,
    })
}
