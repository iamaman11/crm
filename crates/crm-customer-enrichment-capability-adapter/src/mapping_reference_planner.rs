use crate::CustomerEnrichmentMappingCapabilityPlanner;
use crm_capability_runtime::{CapabilityDefinition, CapabilityRequest};
use crm_core_data::{AggregateTarget, CapabilityBatchExecutionPlan, TransactionalAggregatePlanner};
use crm_module_sdk::{RecordSnapshot, SdkError};

/// Compatibility router for mapping publication.
///
/// The immutable provider profile is validated before execution. The mapping itself is the
/// authoritative aggregate target and the record created by the atomic publication plan.
#[derive(Debug, Default, Clone, Copy)]
pub struct CustomerEnrichmentMappingReferencePlanner;

impl TransactionalAggregatePlanner for CustomerEnrichmentMappingReferencePlanner {
    fn target(
        &self,
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
    ) -> Result<AggregateTarget, SdkError> {
        CustomerEnrichmentMappingCapabilityPlanner.target(definition, request)
    }

    fn plan(
        &self,
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
        current: Option<&RecordSnapshot>,
    ) -> Result<CapabilityBatchExecutionPlan, SdkError> {
        CustomerEnrichmentMappingCapabilityPlanner.plan(definition, request, current)
    }
}
