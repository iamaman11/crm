use crate::{
    CustomerEnrichmentMappingCapabilityPlanner, MODULE_ID, PROVIDER_PROFILE_VERSION_RECORD_TYPE,
    PUBLISH_MAPPING_REQUEST_SCHEMA, mapping_from_definition, provider_profile_from_snapshot,
};
use crm_capability_plan_support as support;
use crm_capability_runtime::{CapabilityDefinition, CapabilityRequest};
use crm_core_data::{
    AggregatePresence, AggregateTarget, CapabilityBatchExecutionPlan, TransactionalAggregatePlanner,
};
use crm_module_sdk::{DataClass, ErrorCategory, RecordSnapshot, SdkError};
use crm_proto_contracts::crm::customer_enrichment::v1 as wire;

#[derive(Debug, Default, Clone, Copy)]
pub struct CustomerEnrichmentMappingReferencePlanner;

impl TransactionalAggregatePlanner for CustomerEnrichmentMappingReferencePlanner {
    fn target(
        &self,
        _definition: &CapabilityDefinition,
        request: &CapabilityRequest,
    ) -> Result<AggregateTarget, SdkError> {
        let mapping = decode_mapping(request)?;
        Ok(AggregateTarget {
            reference: support::record_ref(
                PROVIDER_PROFILE_VERSION_RECORD_TYPE,
                mapping.provider_profile_version_id().as_str(),
                "customer_enrichment.mapping.definition.provider_profile_version_ref.provider_profile_version_id",
            )?,
            presence: AggregatePresence::MustExist,
        })
    }

    fn plan(
        &self,
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
        current: Option<&RecordSnapshot>,
    ) -> Result<CapabilityBatchExecutionPlan, SdkError> {
        let mapping = decode_mapping(request)?;
        let profile = provider_profile_from_snapshot(
            current.ok_or_else(|| invalid_reference("provider profile snapshot is missing"))?,
        )?;
        if profile.version_id() != mapping.provider_profile_version_id() {
            return Err(invalid_reference(
                "locked provider-profile identity differs from the mapping reference",
            ));
        }
        if !profile
            .supported_target_fields()
            .contains(&mapping.target_field())
        {
            return Err(SdkError::new(
                "CUSTOMER_ENRICHMENT_MAPPING_TARGET_FIELD_UNSUPPORTED",
                ErrorCategory::InvalidArgument,
                false,
                "The referenced provider profile does not support the mapping target field.",
            ));
        }
        CustomerEnrichmentMappingCapabilityPlanner.plan(definition, request, None)
    }
}

fn decode_mapping(
    request: &CapabilityRequest,
) -> Result<crm_customer_enrichment::MappingVersion, SdkError> {
    let command: wire::PublishMappingVersionRequest = support::decode_request_with_data_class(
        request,
        MODULE_ID,
        PUBLISH_MAPPING_REQUEST_SCHEMA,
        DataClass::Confidential,
    )?;
    mapping_from_definition(command.definition)
}

fn invalid_reference(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_MAPPING_PROVIDER_PROFILE_INVALID",
        ErrorCategory::InvalidArgument,
        false,
        "The referenced provider-profile version is invalid or unavailable.",
    )
    .with_internal_reference(reference.into())
}
