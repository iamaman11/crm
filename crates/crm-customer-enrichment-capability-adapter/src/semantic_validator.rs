use crate::{
    IMPLEMENTED_MUTATION_CAPABILITY_IDS, MODULE_ID, PROVIDER_PROFILE_VERSION_RECORD_TYPE,
    PUBLISH_MAPPING_CAPABILITY, mapping_from_definition, provider_profile_from_snapshot,
};
use crm_capability_plan_support as support;
use crm_capability_runtime::{
    CapabilityDefinition, CapabilityRequest, CapabilitySemanticValidator,
};
use crm_core_data::{PostgresDataStore, RecordGetQuery};
use crm_module_sdk::{
    DataClass, ErrorCategory, ModuleId, PortFuture, RecordId, RecordType, SdkError,
};
use crm_proto_contracts::crm::customer_enrichment::v1 as wire;

#[derive(Debug, Clone)]
pub struct CustomerEnrichmentCapabilitySemanticValidator {
    store: PostgresDataStore,
}

impl CustomerEnrichmentCapabilitySemanticValidator {
    pub fn new(store: PostgresDataStore) -> Self {
        Self { store }
    }
}

impl CapabilitySemanticValidator for CustomerEnrichmentCapabilitySemanticValidator {
    fn validate<'a>(
        &'a self,
        definition: &'a CapabilityDefinition,
        request: &'a CapabilityRequest,
    ) -> PortFuture<'a, Result<(), SdkError>> {
        if !IMPLEMENTED_MUTATION_CAPABILITY_IDS.contains(&definition.capability_id.as_str()) {
            return Box::pin(async { Err(unsupported_capability()) });
        }
        if definition.capability_id.as_str() != PUBLISH_MAPPING_CAPABILITY {
            return Box::pin(async { Ok(()) });
        }
        let mapping = decode_mapping(request);
        Box::pin(async move {
            let mapping = mapping?;
            let provider_profile_id = RecordId::try_new(
                mapping.provider_profile_version_id().as_str().to_owned(),
            )
            .map_err(configuration_error)?;
            let snapshot = self
                .store
                .get_record_for_query(&RecordGetQuery {
                    tenant_id: request.context.execution.tenant_id.clone(),
                    owner_module_id: module_id()?,
                    record_type: provider_profile_record_type()?,
                    record_id: provider_profile_id,
                })
                .await?
                .ok_or_else(reference_unavailable)?;
            let provider_profile = provider_profile_from_snapshot(&snapshot)?;
            if provider_profile
                .supported_target_fields()
                .contains(&mapping.target_field())
            {
                Ok(())
            } else {
                Err(target_field_unsupported())
            }
        })
    }
}

fn decode_mapping(request: &CapabilityRequest) -> Result<crm_customer_enrichment::MappingVersion, SdkError> {
    let command: wire::PublishMappingVersionRequest = support::decode_request_with_data_class(
        request,
        MODULE_ID,
        crate::PUBLISH_MAPPING_REQUEST_SCHEMA,
        DataClass::Confidential,
    )?;
    mapping_from_definition(command.definition)
}

fn module_id() -> Result<ModuleId, SdkError> {
    ModuleId::try_new(MODULE_ID).map_err(configuration_error)
}

fn provider_profile_record_type() -> Result<RecordType, SdkError> {
    RecordType::try_new(PROVIDER_PROFILE_VERSION_RECORD_TYPE).map_err(configuration_error)
}

fn reference_unavailable() -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_MAPPING_PROVIDER_PROFILE_UNAVAILABLE",
        ErrorCategory::InvalidArgument,
        false,
        "The referenced provider-profile version is unavailable.",
    )
}

fn target_field_unsupported() -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_MAPPING_TARGET_FIELD_UNSUPPORTED",
        ErrorCategory::InvalidArgument,
        false,
        "The referenced provider profile does not support the mapping target field.",
    )
}

fn unsupported_capability() -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_COMPOSITION_CAPABILITY_UNSUPPORTED",
        ErrorCategory::Internal,
        false,
        "The Customer Enrichment mutation capability is not configured for this composition boundary.",
    )
}

fn configuration_error(error: impl std::fmt::Display) -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_COMPOSITION_CONFIGURATION_INVALID",
        ErrorCategory::Internal,
        false,
        "The Customer Enrichment composition configuration is invalid.",
    )
    .with_internal_reference(error.to_string())
}
