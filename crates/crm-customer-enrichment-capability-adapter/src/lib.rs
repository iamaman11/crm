#![forbid(unsafe_code)]

//! Governed mutation adapters for `crm.customer-enrichment`.
//!
//! Immutable definition publication uses the shared transactional record/idempotency/outbox/audit
//! runtime. Provider network I/O, credentials, Party reads and owner mutation remain outside this
//! crate.

mod mapping_planner;
mod mapping_reference_planner;
mod mapping_snapshot;
mod provider_profile_planner;
mod provider_profile_snapshot;
mod request_planner;
mod request_reference_planner;
mod semantic_validator;

pub use mapping_planner::{
    CustomerEnrichmentMappingCapabilityPlanner, mapping_from_definition,
    mapping_persisted_contract, mapping_persisted_payload, mapping_record_ref, mapping_to_wire,
    provider_profile_version_id_from_external,
};
pub use mapping_reference_planner::CustomerEnrichmentMappingReferencePlanner;
pub use mapping_snapshot::mapping_from_snapshot;
pub use provider_profile_planner::{
    provider_profile_from_definition, provider_profile_persisted_contract,
    provider_profile_persisted_payload, provider_profile_record_ref, provider_profile_to_wire,
};
pub use provider_profile_snapshot::provider_profile_from_snapshot;
pub use request_planner::*;
pub use request_reference_planner::CustomerEnrichmentRequestReferencePlanner;
pub use semantic_validator::CustomerEnrichmentCapabilitySemanticValidator;

use crm_capability_plan_support as support;
use crm_capability_runtime::{CapabilityDefinition, CapabilityRequest, CapabilityRisk};
use crm_core_data::{AggregateTarget, CapabilityBatchExecutionPlan, TransactionalAggregatePlanner};
use crm_module_sdk::{
    CapabilityId, CapabilityVersion, DataClass, ErrorCategory, ModuleId, RecordSnapshot, SdkError,
};

pub const MODULE_ID: &str = crm_customer_enrichment::MODULE_ID;
pub const PROVIDER_PROFILE_VERSION_RECORD_TYPE: &str =
    crm_customer_enrichment::PROVIDER_PROFILE_VERSION_RECORD_TYPE;
pub const MAPPING_VERSION_RECORD_TYPE: &str = crm_customer_enrichment::MAPPING_VERSION_RECORD_TYPE;

pub const PUBLISH_PROVIDER_PROFILE_CAPABILITY: &str =
    "customer_enrichment.provider_profile.publish";
pub const PUBLISH_PROVIDER_PROFILE_REQUEST_SCHEMA: &str =
    "crm.customer_enrichment.v1.PublishProviderProfileVersionRequest";
pub const PUBLISH_PROVIDER_PROFILE_RESPONSE_SCHEMA: &str =
    "crm.customer_enrichment.v1.PublishProviderProfileVersionResponse";
pub const PROVIDER_PROFILE_PUBLISHED_EVENT_TYPE: &str =
    "customer_enrichment.provider_profile.published";
pub const PROVIDER_PROFILE_PUBLISHED_EVENT_SCHEMA: &str =
    "crm.customer_enrichment.v1.ProviderProfileVersionPublishedEvent";

pub const PUBLISH_MAPPING_CAPABILITY: &str = "customer_enrichment.mapping.publish";
pub const PUBLISH_MAPPING_REQUEST_SCHEMA: &str =
    "crm.customer_enrichment.v1.PublishMappingVersionRequest";
pub const PUBLISH_MAPPING_RESPONSE_SCHEMA: &str =
    "crm.customer_enrichment.v1.PublishMappingVersionResponse";
pub const MAPPING_PUBLISHED_EVENT_TYPE: &str = "customer_enrichment.mapping.published";
pub const MAPPING_PUBLISHED_EVENT_SCHEMA: &str =
    "crm.customer_enrichment.v1.MappingVersionPublishedEvent";

/// Exact mutation coordinates currently safe for direct production registration.
///
/// Request creation has a module-owned planner and definition factory, but it is intentionally
/// excluded until the separate production composition executor performs governed Party and policy
/// pre-authorization.
pub const IMPLEMENTED_MUTATION_CAPABILITY_IDS: &[&str] = &[
    PUBLISH_PROVIDER_PROFILE_CAPABILITY,
    PUBLISH_MAPPING_CAPABILITY,
];

/// Module-owned planner router. The public type name is retained so production composition does
/// not gain a capability-specific switch as additional enrichment coordinates are implemented.
#[derive(Debug, Default, Clone, Copy)]
pub struct CustomerEnrichmentProviderProfileCapabilityPlanner;

impl TransactionalAggregatePlanner for CustomerEnrichmentProviderProfileCapabilityPlanner {
    fn target(
        &self,
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
    ) -> Result<AggregateTarget, SdkError> {
        match definition.capability_id.as_str() {
            PUBLISH_PROVIDER_PROFILE_CAPABILITY => {
                provider_profile_planner::CustomerEnrichmentProviderProfileCapabilityPlanner
                    .target(definition, request)
            }
            PUBLISH_MAPPING_CAPABILITY => {
                CustomerEnrichmentMappingReferencePlanner.target(definition, request)
            }
            CREATE_ENRICHMENT_REQUEST_CAPABILITY => {
                CustomerEnrichmentRequestReferencePlanner.target(definition, request)
            }
            _ => Err(unsupported_capability()),
        }
    }

    fn plan(
        &self,
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
        current: Option<&RecordSnapshot>,
    ) -> Result<CapabilityBatchExecutionPlan, SdkError> {
        match definition.capability_id.as_str() {
            PUBLISH_PROVIDER_PROFILE_CAPABILITY => {
                provider_profile_planner::CustomerEnrichmentProviderProfileCapabilityPlanner
                    .plan(definition, request, current)
            }
            PUBLISH_MAPPING_CAPABILITY => {
                CustomerEnrichmentMappingReferencePlanner.plan(definition, request, current)
            }
            CREATE_ENRICHMENT_REQUEST_CAPABILITY => {
                CustomerEnrichmentRequestReferencePlanner.plan(definition, request, current)
            }
            _ => Err(unsupported_capability()),
        }
    }
}

pub fn capability_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {
    Ok(vec![
        provider_profile_capability_definition()?,
        mapping_capability_definition()?,
    ])
}

pub fn capability_definition() -> Result<CapabilityDefinition, SdkError> {
    provider_profile_capability_definition()
}

pub fn provider_profile_capability_definition() -> Result<CapabilityDefinition, SdkError> {
    mutation_definition(
        PUBLISH_PROVIDER_PROFILE_CAPABILITY,
        PUBLISH_PROVIDER_PROFILE_REQUEST_SCHEMA,
        PUBLISH_PROVIDER_PROFILE_RESPONSE_SCHEMA,
        DataClass::Confidential,
    )
}

pub fn mapping_capability_definition() -> Result<CapabilityDefinition, SdkError> {
    mutation_definition(
        PUBLISH_MAPPING_CAPABILITY,
        PUBLISH_MAPPING_REQUEST_SCHEMA,
        PUBLISH_MAPPING_RESPONSE_SCHEMA,
        DataClass::Confidential,
    )
}

pub fn request_create_capability_definition() -> Result<CapabilityDefinition, SdkError> {
    mutation_definition(
        CREATE_ENRICHMENT_REQUEST_CAPABILITY,
        CREATE_ENRICHMENT_REQUEST_REQUEST_SCHEMA,
        CREATE_ENRICHMENT_REQUEST_RESPONSE_SCHEMA,
        DataClass::Personal,
    )
}

fn mutation_definition(
    capability_id: &'static str,
    request_schema: &'static str,
    response_schema: &'static str,
    data_class: DataClass,
) -> Result<CapabilityDefinition, SdkError> {
    Ok(CapabilityDefinition {
        capability_id: configured(CapabilityId::try_new(capability_id))?,
        capability_version: configured(CapabilityVersion::try_new(support::CONTRACT_VERSION))?,
        owner_module_id: configured(ModuleId::try_new(MODULE_ID))?,
        input_contract: support::protobuf_contract(MODULE_ID, request_schema, vec![data_class])?,
        output_contract: Some(support::protobuf_contract(
            MODULE_ID,
            response_schema,
            vec![data_class],
        )?),
        risk: CapabilityRisk::Medium,
        mutation: true,
        requires_idempotency: true,
        requires_approval: false,
        authorization_policy_id: capability_id.to_owned(),
        rate_limit_policy_id: None,
    })
}

fn configured<T>(value: Result<T, crm_module_sdk::IdentifierError>) -> Result<T, SdkError> {
    value.map_err(|error| configuration_error().with_internal_reference(error.to_string()))
}

fn configuration_error() -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_CONFIGURATION_INVALID",
        ErrorCategory::Internal,
        false,
        "The Customer Enrichment capability configuration is invalid.",
    )
}

fn unsupported_capability() -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_CAPABILITY_UNSUPPORTED",
        ErrorCategory::Internal,
        false,
        "The Customer Enrichment mutation capability is not supported.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn implemented_mutation_catalog_is_exact() {
        let definitions = capability_definitions().unwrap();
        assert_eq!(definitions.len(), 2);
        let ids = definitions
            .iter()
            .map(|definition| definition.capability_id.as_str())
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(
            ids,
            IMPLEMENTED_MUTATION_CAPABILITY_IDS
                .iter()
                .copied()
                .collect()
        );
        for definition in definitions {
            assert_eq!(definition.owner_module_id.as_str(), MODULE_ID);
            assert_eq!(definition.capability_version.as_str(), "1.0.0");
            assert!(definition.mutation);
            assert!(definition.requires_idempotency);
            assert!(!definition.requires_approval);
            assert_eq!(definition.risk, CapabilityRisk::Medium);
        }
    }

    #[test]
    fn request_create_definition_is_personal_but_not_directly_registered() {
        let definition = request_create_capability_definition().unwrap();
        assert_eq!(
            definition.capability_id.as_str(),
            CREATE_ENRICHMENT_REQUEST_CAPABILITY
        );
        assert_eq!(
            definition.input_contract.allowed_data_classes,
            vec![DataClass::Personal]
        );
        assert!(
            !IMPLEMENTED_MUTATION_CAPABILITY_IDS.contains(&CREATE_ENRICHMENT_REQUEST_CAPABILITY)
        );
    }
}
