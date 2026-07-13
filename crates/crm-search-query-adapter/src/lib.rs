#![forbid(unsafe_code)]

//! Governed public query adapter for permission-aware search.
//!
//! This crate owns no index persistence and no authorization policy. It binds
//! the versioned public search contract to `crm-search-runtime`, while the
//! runtime repeats live resource/field visibility for every index candidate.

use crm_capability_plan_support as support;
use crm_capability_runtime::{CapabilityDefinition, CapabilityRisk};
use crm_module_sdk::{
    CapabilityId, CapabilityVersion, DataClass, ErrorCategory, ModuleId, PayloadEncoding,
    PortFuture, RecordType, SdkError,
};
use crm_proto_contracts::crm::search::v1 as search_proto;
use crm_query_runtime::{
    CursorCodec, QueryExecutionResult, QueryExecutor, QueryRequest, QuerySemanticValidator,
    QueryVisibilityAuthorizer,
};
use crm_search_runtime::{
    MAXIMUM_SEARCH_PAGE_SIZE, PermissionAwareSearch, SearchCandidateStore, SearchIndexId,
    SearchRequest,
};
use prost::Message;
use std::collections::BTreeSet;
use std::fmt;
use std::sync::Arc;

pub const SEARCH_MODULE_ID: &str = "crm.search";
pub const SEARCH_QUERY_CAPABILITY: &str = "search.global.query";
pub const SEARCH_REQUEST_SCHEMA: &str = "crm.search.v1.SearchRequest";
pub const SEARCH_RESPONSE_SCHEMA: &str = "crm.search.v1.SearchResponse";

pub fn search_query_capability_definition() -> Result<CapabilityDefinition, SdkError> {
    Ok(CapabilityDefinition {
        capability_id: configured(CapabilityId::try_new(SEARCH_QUERY_CAPABILITY))?,
        capability_version: configured(CapabilityVersion::try_new(support::CONTRACT_VERSION))?,
        owner_module_id: configured(ModuleId::try_new(SEARCH_MODULE_ID))?,
        input_contract: support::protobuf_contract(
            SEARCH_MODULE_ID,
            SEARCH_REQUEST_SCHEMA,
            vec![DataClass::Personal],
        )?,
        output_contract: Some(support::protobuf_contract(
            SEARCH_MODULE_ID,
            SEARCH_RESPONSE_SCHEMA,
            vec![DataClass::Personal],
        )?),
        risk: CapabilityRisk::Low,
        mutation: false,
        requires_idempotency: false,
        requires_approval: false,
        authorization_policy_id: SEARCH_QUERY_CAPABILITY.to_owned(),
        rate_limit_policy_id: None,
    })
}

#[derive(Clone)]
pub struct SearchQueryAdapter {
    index_id: SearchIndexId,
    search: PermissionAwareSearch,
}

impl fmt::Debug for SearchQueryAdapter {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SearchQueryAdapter")
            .field("index_id", &self.index_id)
            .field("search", &self.search)
            .finish()
    }
}

impl SearchQueryAdapter {
    pub fn new(
        index_id: SearchIndexId,
        store: Arc<dyn SearchCandidateStore>,
        visibility: Arc<dyn QueryVisibilityAuthorizer>,
        cursor_codec: CursorCodec,
    ) -> Result<Self, SdkError> {
        Ok(Self {
            index_id,
            search: PermissionAwareSearch::new(store, visibility, cursor_codec)?,
        })
    }
}

impl QuerySemanticValidator for SearchQueryAdapter {
    fn validate<'a>(
        &'a self,
        definition: &'a CapabilityDefinition,
        request: &'a QueryRequest,
    ) -> PortFuture<'a, Result<(), SdkError>> {
        Box::pin(async move {
            validate_definition(definition)?;
            let command = decode_input(request)?;
            let search_request = runtime_request(self.index_id.clone(), command)?;
            search_request.validate()?;
            if search_request.page_size < 0
                || u32::try_from(search_request.page_size)
                    .ok()
                    .is_some_and(|size| size > MAXIMUM_SEARCH_PAGE_SIZE)
            {
                return Err(SdkError::invalid_argument(
                    "search.page_size",
                    "search page size is invalid",
                ));
            }
            Ok(())
        })
    }
}

impl QueryExecutor for SearchQueryAdapter {
    fn execute<'a>(
        &'a self,
        definition: &'a CapabilityDefinition,
        request: QueryRequest,
    ) -> PortFuture<'a, Result<QueryExecutionResult, SdkError>> {
        Box::pin(async move {
            validate_definition(definition)?;
            let command = decode_input(&request)?;
            let page = self
                .search
                .search(&request, runtime_request(self.index_id.clone(), command)?)
                .await?;
            let output = support::protobuf_payload(
                SEARCH_MODULE_ID,
                SEARCH_RESPONSE_SCHEMA,
                DataClass::Personal,
                &search_proto::SearchResponse {
                    hits: page
                        .hits
                        .into_iter()
                        .map(|hit| search_proto::SearchHit {
                            owner_module_id: hit.owner_module_id.as_str().to_owned(),
                            resource_type: hit.resource.record_type.as_str().to_owned(),
                            resource_id: hit.resource.record_id.as_str().to_owned(),
                            source_version: hit.source_version,
                            rank_micros: hit.rank_micros,
                            fields: hit.fields.into_iter().collect(),
                            matched_fields: hit.matched_fields.into_iter().collect(),
                        })
                        .collect(),
                    next_cursor: page.next_cursor.unwrap_or_default(),
                },
            )?;
            Ok(QueryExecutionResult { output })
        })
    }
}

fn runtime_request(
    index_id: SearchIndexId,
    command: search_proto::SearchRequest,
) -> Result<SearchRequest, SdkError> {
    let resource_types = command
        .resource_types
        .into_iter()
        .map(|value| {
            RecordType::try_new(value).map_err(|error| {
                SdkError::invalid_argument("search.resource_types", error.to_string())
            })
        })
        .collect::<Result<BTreeSet<_>, _>>()?;
    Ok(SearchRequest {
        index_id,
        text: command.text,
        resource_types,
        page_size: command.page_size,
        cursor: (!command.cursor.is_empty()).then_some(command.cursor),
    })
}

fn decode_input(request: &QueryRequest) -> Result<search_proto::SearchRequest, SdkError> {
    let payload = &request.input;
    if payload.owner.as_str() != SEARCH_MODULE_ID
        || payload.schema_id.as_str() != SEARCH_REQUEST_SCHEMA
        || payload.schema_version.as_str() != support::CONTRACT_VERSION
        || payload.descriptor_hash != support::message_descriptor_hash(SEARCH_REQUEST_SCHEMA)
        || payload.data_class != DataClass::Personal
        || payload.encoding != PayloadEncoding::Protobuf
        || payload.maximum_size_bytes > support::MAX_PROTOBUF_BYTES
        || payload.validate().is_err()
    {
        return Err(SdkError::new(
            "SEARCH_QUERY_INPUT_CONTRACT_MISMATCH",
            ErrorCategory::InvalidArgument,
            false,
            "The search query input does not match the required contract.",
        ));
    }
    search_proto::SearchRequest::decode(payload.bytes.as_slice()).map_err(|_| {
        SdkError::new(
            "SEARCH_QUERY_INPUT_PROTOBUF_INVALID",
            ErrorCategory::InvalidArgument,
            false,
            "The search query input is not valid Protobuf.",
        )
    })
}

fn validate_definition(definition: &CapabilityDefinition) -> Result<(), SdkError> {
    let expected = search_query_capability_definition()?;
    if definition.capability_id != expected.capability_id
        || definition.capability_version != expected.capability_version
        || definition.owner_module_id != expected.owner_module_id
    {
        return Err(SdkError::new(
            "SEARCH_QUERY_DEFINITION_MISMATCH",
            ErrorCategory::Internal,
            false,
            "The search query capability binding is invalid.",
        ));
    }
    Ok(())
}

fn configured<T>(value: Result<T, crm_module_sdk::IdentifierError>) -> Result<T, SdkError> {
    value.map_err(|error| {
        SdkError::new(
            "SEARCH_QUERY_CONFIGURATION_INVALID",
            ErrorCategory::Internal,
            false,
            "The search query configuration is invalid.",
        )
        .with_internal_reference(error.to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn publishes_one_stable_read_only_personal_search_capability() {
        let definition = search_query_capability_definition().unwrap();
        assert_eq!(definition.owner_module_id.as_str(), SEARCH_MODULE_ID);
        assert_eq!(definition.capability_id.as_str(), SEARCH_QUERY_CAPABILITY);
        assert_eq!(
            definition.capability_version.as_str(),
            support::CONTRACT_VERSION
        );
        assert_eq!(
            definition.input_contract.allowed_data_classes,
            vec![DataClass::Personal]
        );
        assert_eq!(
            definition
                .output_contract
                .as_ref()
                .expect("search output contract")
                .allowed_data_classes,
            vec![DataClass::Personal]
        );
        assert!(!definition.mutation);
        assert!(!definition.requires_idempotency);
        assert!(!definition.requires_approval);
    }

    #[test]
    fn runtime_request_deduplicates_resource_type_filters() {
        let request = runtime_request(
            SearchIndexId::try_new("crm.global-search").unwrap(),
            search_proto::SearchRequest {
                text: "Acme".to_owned(),
                resource_types: vec!["sales.deal".to_owned(), "sales.deal".to_owned()],
                page_size: 25,
                cursor: String::new(),
            },
        )
        .unwrap();
        assert_eq!(request.resource_types.len(), 1);
        assert!(request.cursor.is_none());
    }
}
