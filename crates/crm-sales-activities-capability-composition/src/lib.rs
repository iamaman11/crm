#![forbid(unsafe_code)]

//! Production composition for the first independent Sales and Activities vertical slice.
//!
//! This crate owns no business state. It composes published mutation/query
//! contracts, owner-module planners and the optional Sales→Activities event
//! integration over governed platform adapters. Transport and persistence
//! implementations remain outside business owner modules.

mod link_event_processor;
mod phase6_projections;
mod phase7_search;
mod phase7_search_worker;
mod query_catalog;
mod query_router;

pub use link_event_processor::*;
pub use phase6_projections::*;
pub use phase7_search::*;
pub use phase7_search_worker::*;
pub use query_catalog::*;
pub use query_router::*;

use crm_activities_capability_adapter::{
    ActivitiesTaskCapabilityPlanner, COMPLETE_CAPABILITY as ACTIVITIES_COMPLETE_CAPABILITY,
    CREATE_CAPABILITY as ACTIVITIES_CREATE_CAPABILITY, MODULE_ID as ACTIVITIES_MODULE_ID,
    REMINDER_CAPABILITY as ACTIVITIES_REMINDER_CAPABILITY,
    UPDATE_CAPABILITY as ACTIVITIES_UPDATE_CAPABILITY,
    capability_definition as activities_capability_definition,
};
use crm_capability_adapters::CapabilityCatalog;
use crm_capability_runtime::{CapabilityDefinition, CapabilityRequest};
use crm_core_data::{AggregateTarget, CapabilityBatchExecutionPlan, TransactionalAggregatePlanner};
use crm_module_sdk::{ErrorCategory, RecordSnapshot, SdkError};
use crm_sales_capability_adapter::{
    ADVANCE_CAPABILITY as SALES_ADVANCE_CAPABILITY, CREATE_CAPABILITY as SALES_CREATE_CAPABILITY,
    MODULE_ID as SALES_MODULE_ID, SalesDealCapabilityPlanner,
    UPDATE_CAPABILITY as SALES_UPDATE_CAPABILITY,
    capability_definition as sales_capability_definition,
};

pub const PRODUCTION_MUTATION_CAPABILITY_IDS: [&str; 7] = [
    SALES_CREATE_CAPABILITY,
    SALES_UPDATE_CAPABILITY,
    SALES_ADVANCE_CAPABILITY,
    ACTIVITIES_CREATE_CAPABILITY,
    ACTIVITIES_UPDATE_CAPABILITY,
    ACTIVITIES_COMPLETE_CAPABILITY,
    ACTIVITIES_REMINDER_CAPABILITY,
];

pub fn capability_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {
    PRODUCTION_MUTATION_CAPABILITY_IDS
        .iter()
        .map(|capability_id| expected_definition(capability_id))
        .collect()
}

pub fn capability_catalog() -> Result<CapabilityCatalog, SdkError> {
    CapabilityCatalog::new(capability_definitions()?).map_err(|error| {
        SdkError::new(
            "CAPABILITY_CATALOG_CONFIGURATION_INVALID",
            ErrorCategory::Internal,
            false,
            "The capability catalog configuration is invalid.",
        )
        .with_internal_reference(error.to_string())
    })
}

#[derive(Debug, Default, Clone, Copy)]
pub struct SalesActivitiesCapabilityPlannerRouter;

impl TransactionalAggregatePlanner for SalesActivitiesCapabilityPlannerRouter {
    fn target(
        &self,
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
    ) -> Result<AggregateTarget, SdkError> {
        match validated_route(definition, request)? {
            PlannerRoute::Sales => SalesDealCapabilityPlanner.target(definition, request),
            PlannerRoute::Activities => ActivitiesTaskCapabilityPlanner.target(definition, request),
        }
    }

    fn plan(
        &self,
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
        current: Option<&RecordSnapshot>,
    ) -> Result<CapabilityBatchExecutionPlan, SdkError> {
        match validated_route(definition, request)? {
            PlannerRoute::Sales => SalesDealCapabilityPlanner.plan(definition, request, current),
            PlannerRoute::Activities => {
                ActivitiesTaskCapabilityPlanner.plan(definition, request, current)
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PlannerRoute {
    Sales,
    Activities,
}

fn validated_route(
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
) -> Result<PlannerRoute, SdkError> {
    let expected = expected_definition(definition.capability_id.as_str())?;

    if definition.capability_version != expected.capability_version {
        return Err(configuration_error(
            "CAPABILITY_PLANNER_VERSION_MISMATCH",
            "The capability planner version binding is invalid.",
        ));
    }
    if definition.owner_module_id != expected.owner_module_id {
        return Err(configuration_error(
            "CAPABILITY_PLANNER_OWNER_MISMATCH",
            "The capability planner owner binding is invalid.",
        ));
    }
    if request.context.execution.capability_id != expected.capability_id
        || request.context.execution.capability_version != expected.capability_version
        || request.context.module_id != expected.owner_module_id
    {
        return Err(configuration_error(
            "CAPABILITY_PLANNER_REQUEST_BINDING_MISMATCH",
            "The capability planner request binding is invalid.",
        ));
    }

    match expected.owner_module_id.as_str() {
        SALES_MODULE_ID => Ok(PlannerRoute::Sales),
        ACTIVITIES_MODULE_ID => Ok(PlannerRoute::Activities),
        _ => Err(configuration_error(
            "CAPABILITY_PLANNER_ROUTE_UNSUPPORTED",
            "The capability planner route is unsupported.",
        )),
    }
}

fn expected_definition(capability_id: &str) -> Result<CapabilityDefinition, SdkError> {
    match capability_id {
        SALES_CREATE_CAPABILITY | SALES_UPDATE_CAPABILITY | SALES_ADVANCE_CAPABILITY => {
            sales_capability_definition(capability_id)
        }
        ACTIVITIES_CREATE_CAPABILITY
        | ACTIVITIES_UPDATE_CAPABILITY
        | ACTIVITIES_COMPLETE_CAPABILITY
        | ACTIVITIES_REMINDER_CAPABILITY => activities_capability_definition(capability_id),
        _ => Err(configuration_error(
            "CAPABILITY_PLANNER_ROUTE_UNSUPPORTED",
            "The capability planner route is unsupported.",
        )),
    }
}

fn configuration_error(code: &'static str, safe_message: &'static str) -> SdkError {
    SdkError::new(code, ErrorCategory::Internal, false, safe_message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crm_module_sdk::{
        ActorId, BusinessTransactionId, CapabilityId, CapabilityVersion, CausationId,
        CorrelationId, DataClass, ExecutionContext, IdempotencyKey, ModuleExecutionContext,
        ModuleId, PayloadEncoding, RequestId, RetentionPolicyId, SchemaId, SchemaVersion, TenantId,
        TraceId, TypedPayload,
    };

    #[test]
    fn catalog_contains_exactly_the_seven_phase6_mutations_in_stable_order() {
        let definitions = capability_definitions().unwrap();
        assert_eq!(definitions.len(), PRODUCTION_MUTATION_CAPABILITY_IDS.len());
        assert_eq!(
            definitions
                .iter()
                .map(|definition| definition.capability_id.as_str())
                .collect::<Vec<_>>(),
            PRODUCTION_MUTATION_CAPABILITY_IDS
        );
        assert_eq!(capability_catalog().unwrap().len(), 7);
    }

    #[test]
    fn router_rejects_unknown_coordinate_before_payload_decoding() {
        let mut definition = sales_capability_definition(SALES_CREATE_CAPABILITY).unwrap();
        definition.capability_id = CapabilityId::try_new("sales.deal.unknown").unwrap();
        let request = request_for(&definition);
        let error = SalesActivitiesCapabilityPlannerRouter
            .target(&definition, &request)
            .unwrap_err();
        assert_eq!(error.code, "CAPABILITY_PLANNER_ROUTE_UNSUPPORTED");
    }

    #[test]
    fn router_rejects_version_mismatch_before_payload_decoding() {
        let mut definition = sales_capability_definition(SALES_CREATE_CAPABILITY).unwrap();
        definition.capability_version = CapabilityVersion::try_new("2.0.0").unwrap();
        let request = request_for(&definition);
        let error = SalesActivitiesCapabilityPlannerRouter
            .target(&definition, &request)
            .unwrap_err();
        assert_eq!(error.code, "CAPABILITY_PLANNER_VERSION_MISMATCH");
    }

    #[test]
    fn router_rejects_owner_mismatch_before_payload_decoding() {
        let mut definition = sales_capability_definition(SALES_CREATE_CAPABILITY).unwrap();
        definition.owner_module_id = ModuleId::try_new(ACTIVITIES_MODULE_ID).unwrap();
        let request = request_for(&definition);
        let error = SalesActivitiesCapabilityPlannerRouter
            .target(&definition, &request)
            .unwrap_err();
        assert_eq!(error.code, "CAPABILITY_PLANNER_OWNER_MISMATCH");
    }

    #[test]
    fn router_rejects_request_coordinate_mismatch_before_payload_decoding() {
        let definition = sales_capability_definition(SALES_CREATE_CAPABILITY).unwrap();
        let mut request = request_for(&definition);
        request.context.module_id = ModuleId::try_new(ACTIVITIES_MODULE_ID).unwrap();
        let error = SalesActivitiesCapabilityPlannerRouter
            .target(&definition, &request)
            .unwrap_err();
        assert_eq!(error.code, "CAPABILITY_PLANNER_REQUEST_BINDING_MISMATCH");
    }

    fn request_for(definition: &CapabilityDefinition) -> CapabilityRequest {
        CapabilityRequest {
            context: ModuleExecutionContext {
                module_id: definition.owner_module_id.clone(),
                execution: ExecutionContext {
                    tenant_id: TenantId::try_new("tenant-a").unwrap(),
                    actor_id: ActorId::try_new("actor-a").unwrap(),
                    request_id: RequestId::try_new("request-a").unwrap(),
                    correlation_id: CorrelationId::try_new("correlation-a").unwrap(),
                    causation_id: CausationId::try_new("causation-a").unwrap(),
                    trace_id: TraceId::try_new("trace-a").unwrap(),
                    capability_id: definition.capability_id.clone(),
                    capability_version: definition.capability_version.clone(),
                    idempotency_key: IdempotencyKey::try_new("idem-a").unwrap(),
                    business_transaction_id: BusinessTransactionId::try_new("tx-a").unwrap(),
                    schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
                    request_started_at_unix_nanos: 1,
                },
            },
            input: TypedPayload {
                owner: definition.input_contract.owner.clone(),
                schema_id: SchemaId::try_new(definition.input_contract.schema_id.as_str()).unwrap(),
                schema_version: definition.input_contract.schema_version.clone(),
                descriptor_hash: definition.input_contract.descriptor_hash,
                data_class: DataClass::Confidential,
                encoding: PayloadEncoding::Protobuf,
                maximum_size_bytes: definition.input_contract.maximum_size_bytes,
                retention_policy_id: RetentionPolicyId::try_new("standard").unwrap(),
                bytes: Vec::new(),
            },
            input_hash: [1; 32],
            approval: None,
        }
    }
}
