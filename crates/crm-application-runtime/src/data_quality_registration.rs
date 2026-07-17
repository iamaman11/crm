use crm_capability_runtime::{CapabilityDefinition, CapabilityRequest};
use crm_core_data::{AggregateTarget, CapabilityBatchExecutionPlan, TransactionalAggregatePlanner};
use crm_data_quality_capability_adapter::{
    ACKNOWLEDGE_FINDING_CAPABILITY, ASSIGN_FINDING_CAPABILITY,
    DataQualityFindingStewardshipPlanner, DataQualityRuleSetCapabilityPlanner,
    PUBLISH_PARTY_RULE_SET_CAPABILITY, WAIVE_FINDING_CAPABILITY,
};
use crm_module_sdk::{ErrorCategory, RecordSnapshot, SdkError};

#[derive(Debug, Default, Clone, Copy)]
pub struct DataQualityAggregatePlanner;

impl TransactionalAggregatePlanner for DataQualityAggregatePlanner {
    fn target(
        &self,
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
    ) -> Result<AggregateTarget, SdkError> {
        match definition.capability_id.as_str() {
            PUBLISH_PARTY_RULE_SET_CAPABILITY => {
                DataQualityRuleSetCapabilityPlanner.target(definition, request)
            }
            ASSIGN_FINDING_CAPABILITY
            | ACKNOWLEDGE_FINDING_CAPABILITY
            | WAIVE_FINDING_CAPABILITY => {
                DataQualityFindingStewardshipPlanner.target(definition, request)
            }
            _ => Err(unsupported_capability(definition)),
        }
    }

    fn plan(
        &self,
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
        current: Option<&RecordSnapshot>,
    ) -> Result<CapabilityBatchExecutionPlan, SdkError> {
        match definition.capability_id.as_str() {
            PUBLISH_PARTY_RULE_SET_CAPABILITY => {
                DataQualityRuleSetCapabilityPlanner.plan(definition, request, current)
            }
            ASSIGN_FINDING_CAPABILITY
            | ACKNOWLEDGE_FINDING_CAPABILITY
            | WAIVE_FINDING_CAPABILITY => {
                DataQualityFindingStewardshipPlanner.plan(definition, request, current)
            }
            _ => Err(unsupported_capability(definition)),
        }
    }
}

fn unsupported_capability(definition: &CapabilityDefinition) -> SdkError {
    SdkError::new(
        "DATA_QUALITY_AGGREGATE_CAPABILITY_UNSUPPORTED",
        ErrorCategory::Internal,
        false,
        "The Data Quality aggregate capability is not configured.",
    )
    .with_internal_reference(format!(
        "{}@{}",
        definition.capability_id, definition.capability_version
    ))
}
