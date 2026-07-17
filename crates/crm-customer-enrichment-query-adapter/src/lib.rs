#![forbid(unsafe_code)]

//! Permission-aware query adapter for governed Customer Enrichment definitions.

use crm_capability_plan_support as support;
use crm_capability_runtime::{CapabilityDefinition, CapabilityRisk};
use crm_core_data::{PostgresDataStore, RecordGetQuery};
use crm_customer_enrichment::PROVIDER_PROFILE_VERSION_RECORD_TYPE;
use crm_customer_enrichment_capability_adapter::{
    provider_profile_from_snapshot, provider_profile_to_wire, MODULE_ID,
};
use crm_module_sdk::{
    CapabilityId, CapabilityVersion, DataClass, ErrorCategory, ModuleId, PayloadEncoding,
    PortFuture, RecordId, RecordType, SdkError, TypedPayload,
};
use crm_proto_contracts::crm::customer_enrichment::v1 as wire;
use crm_query_runtime::{
    QueryExecutionResult, QueryExecutor, QueryRequest, QuerySemanticValidator,
    QueryVisibilityAuthorizer,
};
use prost::Message;
use std::sync::Arc;

pub const GET_PROVIDER_PROFILE_CAPABILITY: &str = "customer_enrichment.provider_profile.get";
pub const GET_PROVIDER_PROFILE_REQUEST_SCHEMA: &str =
    "crm.customer_enrichment.v1.GetProviderProfileVersionRequest";
pub const GET_PROVIDER_PROFILE_RESPONSE_SCHEMA: &str =
    "crm.customer_enrichment.v1.GetProviderProfileVersionResponse";
pub const QUERY_CAPABILITY_IDS: &[&str] = &[GET_PROVIDER_PROFILE_CAPABILITY];

#[derive(Clone)]
pub struct CustomerEnrichmentQueryAdapter {
    store: PostgresDataStore,
    visibility: Arc<dyn QueryVisibilityAuthorizer>,
}

impl CustomerEnrichmentQueryAdapter {
    pub fn new(
        store: PostgresDataStore,
        visibility: Arc<dyn QueryVisibilityAuthorizer>,
    ) -> Self {
        Self { store, visibility }
    }

    async fn execute_get_provider_profile(
        &self,
        request: &QueryRequest,
    ) -> Result<TypedPayload, SdkError> {
        let command: wire::GetProviderProfileVersionRequest = decode_input(request)?;
        let record_id = provider_profile_record_id(command.provider_profile_version_ref)?;
        let snapshot = self
            .store
            .get_record_for_query(&RecordGetQuery {
                tenant_id: request.context.tenant_id.clone(),
                owner_module_id: module_id()?,
                record_type: provider_profile_record_type()?,
                record_id,
            })
            .await?
            .ok_or_else(provider_profile_not_found)?;
        let visibility = self
            .visibility
            .authorize_visibility(request, &snapshot.reference)
            .await?;
        if !visibility.resource_visible {
            return Err(provider_profile_not_found());
        }

        let profile = provider_profile_from_snapshot(&snapshot)?;
        let mut output = provider_profile_to_wire(&profile);
        if !visibility.allows_field("definition") {
            output.definition = None;
        }
        support::protobuf_payload(
            MODULE_ID,
            GET_PROVIDER_PROFILE_RESPONSE_SCHEMA,
            DataClass::Confidential,
            &wire::GetProviderProfileVersionResponse {
                provider_profile_version: Some(output),
            },
        )
    }
}

impl std::fmt::Debug for CustomerEnrichmentQueryAdapter {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CustomerEnrichmentQueryAdapter")
            .field("store", &self.store)
            .field("visibility", &"dyn QueryVisibilityAuthorizer")
            .finish()
    }
}

impl QuerySemanticValidator for CustomerEnrichmentQueryAdapter {
    fn validate<'a>(
        &'a self,
        definition: &'a CapabilityDefinition,
        request: &'a QueryRequest,
    ) -> PortFuture<'a, Result<(), SdkError>> {
        Box::pin(async move {
            ensure_definition(definition)?;
            let command: wire::GetProviderProfileVersionRequest = decode_input(request)?;
            provider_profile_record_id(command.provider_profile_version_ref).map(|_| ())
        })
    }
}

impl QueryExecutor for CustomerEnrichmentQueryAdapter {
    fn execute<'a>(
        &'a self,
        definition: &'a CapabilityDefinition,
        request: QueryRequest,
    ) -> PortFuture<'a, Result<QueryExecutionResult, SdkError>> {
        Box::pin(async move {
            ensure_definition(definition)?;
            Ok(QueryExecutionResult {
                output: self.execute_get_provider_profile(&request).await?,
            })
        })
    }
}

pub fn query_capability_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {
    Ok(vec![provider_profile_query_capability_definition()?])
}

pub fn query_capability_definition() -> Result<CapabilityDefinition, SdkError> {
    provider_profile_query_capability_definition()
}

pub fn provider_profile_query_capability_definition() -> Result<CapabilityDefinition, SdkError> {
    Ok(CapabilityDefinition {
        capability_id: configured(CapabilityId::try_new(GET_PROVIDER_PROFILE_CAPABILITY))?,
        capability_version: configured(CapabilityVersion::try_new(support::CONTRACT_VERSION))?,
        owner_module_id: configured(ModuleId::try_new(MODULE_ID))?,
        input_contract: support::protobuf_contract(
            MODULE_ID,
            GET_PROVIDER_PROFILE_REQUEST_SCHEMA,
            vec![DataClass::Confidential],
        )?,
        output_contract: Some(support::protobuf_contract(
            MODULE_ID,
            GET_PROVIDER_PROFILE_RESPONSE_SCHEMA,
            vec![DataClass::Confidential],
        )?),
        risk: CapabilityRisk::Low,
        mutation: false,
        requires_idempotency: false,
        requires_approval: false,
        authorization_policy_id: GET_PROVIDER_PROFILE_CAPABILITY.to_owned(),
        rate_limit_policy_id: None,
    })
}

fn decode_input<T: Message + Default>(request: &QueryRequest) -> Result<T, SdkError> {
    let payload = &request.input;
    if payload.owner.as_str() != MODULE_ID
        || payload.schema_id.as_str() != GET_PROVIDER_PROFILE_REQUEST_SCHEMA
        || payload.schema_version.as_str() != support::CONTRACT_VERSION
        || payload.descriptor_hash
            != support::message_descriptor_hash(GET_PROVIDER_PROFILE_REQUEST_SCHEMA)
        || payload.data_class != DataClass::Confidential
        || payload.encoding != PayloadEncoding::Protobuf
        || payload.maximum_size_bytes != support::MAX_PROTOBUF_BYTES
        || payload.validate().is_err()
    {
        return Err(SdkError::new(
            "CUSTOMER_ENRICHMENT_PROVIDER_PROFILE_QUERY_CONTRACT_MISMATCH",
            ErrorCategory::InvalidArgument,
            false,
            "The provider-profile query input does not match the required contract.",
        ));
    }
    T::decode(payload.bytes.as_slice()).map_err(|_| {
        SdkError::new(
            "CUSTOMER_ENRICHMENT_PROVIDER_PROFILE_QUERY_PROTOBUF_INVALID",
            ErrorCategory::InvalidArgument,
            false,
            "The provider-profile query input is not valid Protobuf.",
        )
    })
}

fn provider_profile_record_id(
    value: Option<wire::ProviderProfileVersionRef>,
) -> Result<RecordId, SdkError> {
    let value = value.ok_or_else(|| {
        SdkError::invalid_argument(
            "customer_enrichment.provider_profile_version_ref",
            "Provider-profile version reference is required",
        )
    })?;
    RecordId::try_new(value.provider_profile_version_id).map_err(|error| {
        SdkError::invalid_argument(
            "customer_enrichment.provider_profile_version_ref.provider_profile_version_id",
            error.to_string(),
        )
    })
}

fn ensure_definition(definition: &CapabilityDefinition) -> Result<(), SdkError> {
    if definition.owner_module_id.as_str() != MODULE_ID
        || definition.capability_id.as_str() != GET_PROVIDER_PROFILE_CAPABILITY
        || definition.capability_version.as_str() != support::CONTRACT_VERSION
        || definition.mutation
    {
        return Err(unsupported_query());
    }
    Ok(())
}

fn module_id() -> Result<ModuleId, SdkError> {
    configured(ModuleId::try_new(MODULE_ID))
}

fn provider_profile_record_type() -> Result<RecordType, SdkError> {
    configured(RecordType::try_new(PROVIDER_PROFILE_VERSION_RECORD_TYPE))
}

fn configured<T>(value: Result<T, crm_module_sdk::IdentifierError>) -> Result<T, SdkError> {
    value.map_err(|error| configuration_error().with_internal_reference(error.to_string()))
}

fn unsupported_query() -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_QUERY_UNSUPPORTED",
        ErrorCategory::InvalidArgument,
        false,
        "The requested Customer Enrichment query is not supported.",
    )
}

fn configuration_error() -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_QUERY_CONFIGURATION_INVALID",
        ErrorCategory::Internal,
        false,
        "The Customer Enrichment query configuration is invalid.",
    )
}

fn provider_profile_not_found() -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_PROVIDER_PROFILE_NOT_FOUND",
        ErrorCategory::NotFound,
        false,
        "The requested provider-profile version was not found.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_profile_query_definition_is_exact() {
        let definitions = query_capability_definitions().unwrap();
        assert_eq!(definitions.len(), 1);
        let definition = &definitions[0];
        assert_eq!(definition.owner_module_id.as_str(), MODULE_ID);
        assert_eq!(
            definition.capability_id.as_str(),
            GET_PROVIDER_PROFILE_CAPABILITY
        );
        assert_eq!(definition.capability_version.as_str(), "1.0.0");
        assert!(!definition.mutation);
        assert!(!definition.requires_idempotency);
        assert!(!definition.requires_approval);
        assert_eq!(definition.risk, CapabilityRisk::Low);
        assert_eq!(QUERY_CAPABILITY_IDS, &[GET_PROVIDER_PROFILE_CAPABILITY]);
    }
}
