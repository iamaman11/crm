#![forbid(unsafe_code)]

//! Permission-aware query adapter for governed Customer Enrichment definitions.

use crm_capability_plan_support as support;
use crm_capability_runtime::{CapabilityDefinition, CapabilityRisk};
use crm_core_data::{PostgresDataStore, RecordGetQuery};
use crm_customer_enrichment::{MAPPING_VERSION_RECORD_TYPE, PROVIDER_PROFILE_VERSION_RECORD_TYPE};
use crm_customer_enrichment_capability_adapter::{
    MODULE_ID, mapping_from_snapshot, mapping_to_wire, provider_profile_from_snapshot,
    provider_profile_to_wire,
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
pub const GET_MAPPING_CAPABILITY: &str = "customer_enrichment.mapping.get";
pub const GET_MAPPING_REQUEST_SCHEMA: &str = "crm.customer_enrichment.v1.GetMappingVersionRequest";
pub const GET_MAPPING_RESPONSE_SCHEMA: &str =
    "crm.customer_enrichment.v1.GetMappingVersionResponse";
pub const QUERY_CAPABILITY_IDS: &[&str] =
    &[GET_PROVIDER_PROFILE_CAPABILITY, GET_MAPPING_CAPABILITY];

#[derive(Clone)]
pub struct CustomerEnrichmentQueryAdapter {
    store: PostgresDataStore,
    visibility: Arc<dyn QueryVisibilityAuthorizer>,
}

impl CustomerEnrichmentQueryAdapter {
    pub fn new(store: PostgresDataStore, visibility: Arc<dyn QueryVisibilityAuthorizer>) -> Self {
        Self { store, visibility }
    }

    async fn execute_get_provider_profile(
        &self,
        request: &QueryRequest,
    ) -> Result<TypedPayload, SdkError> {
        let command: wire::GetProviderProfileVersionRequest =
            decode_input(request, GET_PROVIDER_PROFILE_REQUEST_SCHEMA)?;
        let record_id = provider_profile_record_id(command.provider_profile_version_ref)?;
        let snapshot = self
            .get_visible_snapshot(
                request,
                provider_profile_record_type()?,
                record_id,
                provider_profile_not_found,
            )
            .await?;
        let visibility = self
            .visibility
            .authorize_visibility(request, &snapshot.reference)
            .await?;
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

    async fn execute_get_mapping(&self, request: &QueryRequest) -> Result<TypedPayload, SdkError> {
        let command: wire::GetMappingVersionRequest =
            decode_input(request, GET_MAPPING_REQUEST_SCHEMA)?;
        let record_id = mapping_record_id(command.mapping_version_ref)?;
        let snapshot = self
            .get_snapshot(
                request,
                mapping_record_type()?,
                record_id,
                mapping_not_found,
            )
            .await?;
        let mapping = mapping_from_snapshot(&snapshot)?;
        let profile_reference = support::record_ref(
            PROVIDER_PROFILE_VERSION_RECORD_TYPE,
            mapping.provider_profile_version_id().as_str(),
            "customer_enrichment.mapping.definition.provider_profile_version_ref.provider_profile_version_id",
        )?;
        let visibility = self
            .visibility
            .authorize_visibility(request, &profile_reference)
            .await?;
        if !visibility.resource_visible {
            return Err(mapping_not_found());
        }
        let mut output = mapping_to_wire(&mapping);
        if !visibility.allows_field("definition") {
            output.definition = None;
        }
        support::protobuf_payload(
            MODULE_ID,
            GET_MAPPING_RESPONSE_SCHEMA,
            DataClass::Confidential,
            &wire::GetMappingVersionResponse {
                mapping_version: Some(output),
            },
        )
    }

    async fn get_visible_snapshot(
        &self,
        request: &QueryRequest,
        record_type: RecordType,
        record_id: RecordId,
        not_found: fn() -> SdkError,
    ) -> Result<crm_module_sdk::RecordSnapshot, SdkError> {
        let snapshot = self
            .get_snapshot(request, record_type, record_id, not_found)
            .await?;
        let visibility = self
            .visibility
            .authorize_visibility(request, &snapshot.reference)
            .await?;
        if visibility.resource_visible {
            Ok(snapshot)
        } else {
            Err(not_found())
        }
    }

    async fn get_snapshot(
        &self,
        request: &QueryRequest,
        record_type: RecordType,
        record_id: RecordId,
        not_found: fn() -> SdkError,
    ) -> Result<crm_module_sdk::RecordSnapshot, SdkError> {
        self.store
            .get_record_for_query(&RecordGetQuery {
                tenant_id: request.context.tenant_id.clone(),
                owner_module_id: module_id()?,
                record_type,
                record_id,
            })
            .await?
            .ok_or_else(not_found)
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
            match definition.capability_id.as_str() {
                GET_PROVIDER_PROFILE_CAPABILITY => {
                    let command: wire::GetProviderProfileVersionRequest =
                        decode_input(request, GET_PROVIDER_PROFILE_REQUEST_SCHEMA)?;
                    provider_profile_record_id(command.provider_profile_version_ref).map(|_| ())
                }
                GET_MAPPING_CAPABILITY => {
                    let command: wire::GetMappingVersionRequest =
                        decode_input(request, GET_MAPPING_REQUEST_SCHEMA)?;
                    mapping_record_id(command.mapping_version_ref).map(|_| ())
                }
                _ => Err(unsupported_query()),
            }
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
            let output = match definition.capability_id.as_str() {
                GET_PROVIDER_PROFILE_CAPABILITY => {
                    self.execute_get_provider_profile(&request).await?
                }
                GET_MAPPING_CAPABILITY => self.execute_get_mapping(&request).await?,
                _ => return Err(unsupported_query()),
            };
            Ok(QueryExecutionResult { output })
        })
    }
}

pub fn query_capability_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {
    Ok(vec![
        provider_profile_query_capability_definition()?,
        mapping_query_capability_definition()?,
    ])
}

pub fn query_capability_definition() -> Result<CapabilityDefinition, SdkError> {
    provider_profile_query_capability_definition()
}

pub fn provider_profile_query_capability_definition() -> Result<CapabilityDefinition, SdkError> {
    query_definition(
        GET_PROVIDER_PROFILE_CAPABILITY,
        GET_PROVIDER_PROFILE_REQUEST_SCHEMA,
        GET_PROVIDER_PROFILE_RESPONSE_SCHEMA,
    )
}

pub fn mapping_query_capability_definition() -> Result<CapabilityDefinition, SdkError> {
    query_definition(
        GET_MAPPING_CAPABILITY,
        GET_MAPPING_REQUEST_SCHEMA,
        GET_MAPPING_RESPONSE_SCHEMA,
    )
}

fn query_definition(
    capability_id: &'static str,
    request_schema: &'static str,
    response_schema: &'static str,
) -> Result<CapabilityDefinition, SdkError> {
    Ok(CapabilityDefinition {
        capability_id: configured(CapabilityId::try_new(capability_id))?,
        capability_version: configured(CapabilityVersion::try_new(support::CONTRACT_VERSION))?,
        owner_module_id: configured(ModuleId::try_new(MODULE_ID))?,
        input_contract: support::protobuf_contract(
            MODULE_ID,
            request_schema,
            vec![DataClass::Confidential],
        )?,
        output_contract: Some(support::protobuf_contract(
            MODULE_ID,
            response_schema,
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

fn decode_input<T: Message + Default>(
    request: &QueryRequest,
    expected_schema: &'static str,
) -> Result<T, SdkError> {
    let payload = &request.input;
    if payload.owner.as_str() != MODULE_ID
        || payload.schema_id.as_str() != expected_schema
        || payload.schema_version.as_str() != support::CONTRACT_VERSION
        || payload.descriptor_hash != support::message_descriptor_hash(expected_schema)
        || payload.data_class != DataClass::Confidential
        || payload.encoding != PayloadEncoding::Protobuf
        || payload.maximum_size_bytes != support::MAX_PROTOBUF_BYTES
        || payload.validate().is_err()
    {
        return Err(SdkError::new(
            "CUSTOMER_ENRICHMENT_QUERY_CONTRACT_MISMATCH",
            ErrorCategory::InvalidArgument,
            false,
            "The Customer Enrichment query input does not match the required contract.",
        ));
    }
    T::decode(payload.bytes.as_slice()).map_err(|_| {
        SdkError::new(
            "CUSTOMER_ENRICHMENT_QUERY_PROTOBUF_INVALID",
            ErrorCategory::InvalidArgument,
            false,
            "The Customer Enrichment query input is not valid Protobuf.",
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

fn mapping_record_id(value: Option<wire::MappingVersionRef>) -> Result<RecordId, SdkError> {
    let value = value.ok_or_else(|| {
        SdkError::invalid_argument(
            "customer_enrichment.mapping_version_ref",
            "Mapping version reference is required",
        )
    })?;
    RecordId::try_new(value.mapping_version_id).map_err(|error| {
        SdkError::invalid_argument(
            "customer_enrichment.mapping_version_ref.mapping_version_id",
            error.to_string(),
        )
    })
}

fn ensure_definition(definition: &CapabilityDefinition) -> Result<(), SdkError> {
    if definition.owner_module_id.as_str() != MODULE_ID
        || !QUERY_CAPABILITY_IDS.contains(&definition.capability_id.as_str())
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

fn mapping_record_type() -> Result<RecordType, SdkError> {
    configured(RecordType::try_new(MAPPING_VERSION_RECORD_TYPE))
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

fn mapping_not_found() -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_MAPPING_NOT_FOUND",
        ErrorCategory::NotFound,
        false,
        "The requested mapping version was not found.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn definition_query_catalog_is_exact() {
        let definitions = query_capability_definitions().unwrap();
        assert_eq!(definitions.len(), 2);
        let ids = definitions
            .iter()
            .map(|definition| definition.capability_id.as_str())
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(ids, QUERY_CAPABILITY_IDS.iter().copied().collect());
        for definition in definitions {
            assert_eq!(definition.owner_module_id.as_str(), MODULE_ID);
            assert_eq!(definition.capability_version.as_str(), "1.0.0");
            assert!(!definition.mutation);
            assert!(!definition.requires_idempotency);
            assert!(!definition.requires_approval);
            assert_eq!(definition.risk, CapabilityRisk::Low);
        }
    }
}
