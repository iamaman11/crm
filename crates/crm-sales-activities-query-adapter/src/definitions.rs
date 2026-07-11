use crm_capability_plan_support as support;
use crm_capability_runtime::{CapabilityDefinition, CapabilityRisk};
use crm_module_sdk::{CapabilityId, CapabilityVersion, DataClass, ModuleId, SdkError};

pub const SALES_MODULE_ID: &str = "crm.sales";
pub const SALES_RECORD_TYPE: &str = "sales.deal";
pub const ACTIVITIES_MODULE_ID: &str = "crm.activities";
pub const ACTIVITIES_RECORD_TYPE: &str = "activities.task";

pub const SALES_GET_CAPABILITY: &str = "sales.deal.get";
pub const SALES_LIST_CAPABILITY: &str = "sales.deal.list";
pub const ACTIVITIES_GET_CAPABILITY: &str = "activities.task.get";
pub const ACTIVITIES_LIST_CAPABILITY: &str = "activities.task.list";

pub const SALES_GET_REQUEST_SCHEMA: &str = "crm.sales.v1.GetDealRequest";
pub const SALES_GET_RESPONSE_SCHEMA: &str = "crm.sales.v1.GetDealResponse";
pub const SALES_LIST_REQUEST_SCHEMA: &str = "crm.sales.v1.ListDealsRequest";
pub const SALES_LIST_RESPONSE_SCHEMA: &str = "crm.sales.v1.ListDealsResponse";
pub const ACTIVITIES_GET_REQUEST_SCHEMA: &str = "crm.activities.v1.GetTaskRequest";
pub const ACTIVITIES_GET_RESPONSE_SCHEMA: &str = "crm.activities.v1.GetTaskResponse";
pub const ACTIVITIES_LIST_REQUEST_SCHEMA: &str = "crm.activities.v1.ListTasksRequest";
pub const ACTIVITIES_LIST_RESPONSE_SCHEMA: &str = "crm.activities.v1.ListTasksResponse";

pub const PRODUCTION_QUERY_CAPABILITY_IDS: [&str; 4] = [
    SALES_GET_CAPABILITY,
    SALES_LIST_CAPABILITY,
    ACTIVITIES_GET_CAPABILITY,
    ACTIVITIES_LIST_CAPABILITY,
];

pub fn query_capability_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {
    PRODUCTION_QUERY_CAPABILITY_IDS
        .iter()
        .map(|capability_id| query_capability_definition(capability_id))
        .collect()
}

pub fn query_capability_definition(capability_id: &str) -> Result<CapabilityDefinition, SdkError> {
    let (owner, input_schema, output_schema) = match capability_id {
        SALES_GET_CAPABILITY => (
            SALES_MODULE_ID,
            SALES_GET_REQUEST_SCHEMA,
            SALES_GET_RESPONSE_SCHEMA,
        ),
        SALES_LIST_CAPABILITY => (
            SALES_MODULE_ID,
            SALES_LIST_REQUEST_SCHEMA,
            SALES_LIST_RESPONSE_SCHEMA,
        ),
        ACTIVITIES_GET_CAPABILITY => (
            ACTIVITIES_MODULE_ID,
            ACTIVITIES_GET_REQUEST_SCHEMA,
            ACTIVITIES_GET_RESPONSE_SCHEMA,
        ),
        ACTIVITIES_LIST_CAPABILITY => (
            ACTIVITIES_MODULE_ID,
            ACTIVITIES_LIST_REQUEST_SCHEMA,
            ACTIVITIES_LIST_RESPONSE_SCHEMA,
        ),
        _ => {
            return Err(SdkError::new(
                "QUERY_CAPABILITY_UNSUPPORTED",
                crm_module_sdk::ErrorCategory::Internal,
                false,
                "The query capability is not configured.",
            ));
        }
    };

    Ok(CapabilityDefinition {
        capability_id: support::configured_identifier(CapabilityId::try_new(capability_id))?,
        capability_version: support::configured_identifier(CapabilityVersion::try_new(
            support::CONTRACT_VERSION,
        ))?,
        owner_module_id: support::configured_identifier(ModuleId::try_new(owner))?,
        input_contract: support::protobuf_contract(
            owner,
            input_schema,
            vec![DataClass::Confidential],
        )?,
        output_contract: Some(support::protobuf_contract(
            owner,
            output_schema,
            vec![DataClass::Confidential],
        )?),
        risk: CapabilityRisk::Low,
        mutation: false,
        requires_idempotency: false,
        requires_approval: false,
        authorization_policy_id: capability_id.to_owned(),
        rate_limit_policy_id: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn publishes_exactly_four_read_only_query_coordinates() {
        let definitions = query_capability_definitions().unwrap();
        assert_eq!(definitions.len(), 4);
        assert_eq!(
            definitions
                .iter()
                .map(|definition| definition.capability_id.as_str())
                .collect::<Vec<_>>(),
            PRODUCTION_QUERY_CAPABILITY_IDS
        );
        assert!(definitions.iter().all(|definition| !definition.mutation));
        assert!(
            definitions
                .iter()
                .all(|definition| !definition.requires_idempotency && !definition.requires_approval)
        );
    }
}
