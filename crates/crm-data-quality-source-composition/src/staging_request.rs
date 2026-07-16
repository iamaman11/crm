use crm_capability_runtime::{CapabilityDefinition, CapabilityRequest};

pub(crate) struct PreparedStageRequest {
    pub definition: CapabilityDefinition,
    pub request: CapabilityRequest,
}
