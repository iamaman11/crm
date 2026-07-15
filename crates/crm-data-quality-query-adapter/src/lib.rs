#![forbid(unsafe_code)]

use crm_capability_plan_support as support;
use crm_capability_runtime::{CapabilityDefinition, CapabilityRisk};
use crm_core_data::{PostgresDataStore, RecordGetQuery};
use crm_data_quality::PARTY_RULE_SET_VERSION_RECORD_TYPE;
use crm_data_quality_capability_adapter::{
    MODULE_ID, party_rule_set_from_snapshot, party_rule_set_to_wire,
};
use crm_module_sdk::{
    CapabilityId, CapabilityVersion, DataClass, ErrorCategory, ModuleId, PayloadEncoding,
    PortFuture, RecordId, RecordType, SdkError, TypedPayload,
};
use crm_proto_contracts::crm::data_quality::v1 as wire;
use crm_query_runtime::{
    QueryExecutionResult, QueryExecutor, QueryRequest, QuerySemanticValidator,
    QueryVisibilityAuthorizer,
};
use prost::Message;
use std::sync::Arc;

pub const GET_PARTY_RULE_SET_CAPABILITY: &str = "data_quality.party.rule_set.get";
pub const GET_PARTY_RULE_SET_REQUEST_SCHEMA: &str =
    "crm.data_quality.v1.GetPartyRuleSetVersionRequest";
pub const GET_PARTY_RULE_SET_RESPONSE_SCHEMA: &str =
    "crm.data_quality.v1.GetPartyRuleSetVersionResponse";

#[derive(Clone)]
pub struct DataQualityQueryAdapter {
    store: PostgresDataStore,
    visibility: Arc<dyn QueryVisibilityAuthorizer>,
}

impl std::fmt::Debug for DataQualityQueryAdapter {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DataQualityQueryAdapter")
            .field("store", &self.store)
            .field("visibility", &"dyn QueryVisibilityAuthorizer")
            .finish()
    }
}

impl DataQualityQueryAdapter {
    pub fn new(store: PostgresDataStore, visibility: Arc<dyn QueryVisibilityAuthorizer>) -> Self {
        Self { store, visibility }
    }

    async fn execute_get_party_rule_set(
        &self,
        request: &QueryRequest,
    ) -> Result<TypedPayload, SdkError> {
        let command: wire::GetPartyRuleSetVersionRequest = decode_input(request)?;
        let version_ref = required_rule_set_ref(command.rule_set_version_ref)?;
        let snapshot = self
            .store
            .get_record_for_query(&RecordGetQuery {
                tenant_id: request.context.tenant_id.clone(),
                owner_module_id: module_id()?,
                record_type: rule_set_record_type()?,
                record_id: RecordId::try_new(version_ref.rule_set_version_id).map_err(|_| {
                    SdkError::invalid_argument(
                        "data_quality.party_rule_set.rule_set_version_ref.rule_set_version_id",
                        "Party rule-set version reference is invalid",
                    )
                })?,
            })
            .await?
            .ok_or_else(resource_not_found)?;
        let visibility = self
            .visibility
            .authorize_visibility(request, &snapshot.reference)
            .await?;
        if !visibility.resource_visible {
            return Err(resource_not_found());
        }

        let rule_set = party_rule_set_from_snapshot(&snapshot)?;
        let mut output = party_rule_set_to_wire(&rule_set);
        if !visibility.allows_field("definition") {
            output.definition = None;
        }

        support::protobuf_payload(
            MODULE_ID,
            GET_PARTY_RULE_SET_RESPONSE_SCHEMA,
            DataClass::Confidential,
            &wire::GetPartyRuleSetVersionResponse {
                rule_set_version: Some(output),
            },
        )
    }
}

pub fn query_capability_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {
    Ok(vec![query_capability_definition()?])
}

pub fn query_capability_definition() -> Result<CapabilityDefinition, SdkError> {
    Ok(CapabilityDefinition {
        capability_id: support::configured_identifier(CapabilityId::try_new(
            GET_PARTY_RULE_SET_CAPABILITY,
        ))?,
        capability_version: support::configured_identifier(CapabilityVersion::try_new(
            support::CONTRACT_VERSION,
        ))?,
        owner_module_id: support::configured_identifier(ModuleId::try_new(MODULE_ID))?,
        input_contract: support::protobuf_contract(
            MODULE_ID,
            GET_PARTY_RULE_SET_REQUEST_SCHEMA,
            vec![DataClass::Confidential],
        )?,
        output_contract: Some(support::protobuf_contract(
            MODULE_ID,
            GET_PARTY_RULE_SET_RESPONSE_SCHEMA,
            vec![DataClass::Confidential],
        )?),
        risk: CapabilityRisk::Low,
        mutation: false,
        requires_idempotency: false,
        requires_approval: false,
        authorization_policy_id: GET_PARTY_RULE_SET_CAPABILITY.to_owned(),
        rate_limit_policy_id: None,
    })
}

impl QuerySemanticValidator for DataQualityQueryAdapter {
    fn validate<'a>(
        &'a self,
        definition: &'a CapabilityDefinition,
        request: &'a QueryRequest,
    ) -> PortFuture<'a, Result<(), SdkError>> {
        Box::pin(async move {
            ensure_definition(definition)?;
            let command: wire::GetPartyRuleSetVersionRequest = decode_input(request)?;
            let version_ref = required_rule_set_ref(command.rule_set_version_ref)?;
            RecordId::try_new(version_ref.rule_set_version_id).map_err(|_| {
                SdkError::invalid_argument(
                    "data_quality.party_rule_set.rule_set_version_ref.rule_set_version_id",
                    "Party rule-set version reference is invalid",
                )
            })?;
            Ok(())
        })
    }
}

impl QueryExecutor for DataQualityQueryAdapter {
    fn execute<'a>(
        &'a self,
        definition: &'a CapabilityDefinition,
        request: QueryRequest,
    ) -> PortFuture<'a, Result<QueryExecutionResult, SdkError>> {
        Box::pin(async move {
            ensure_definition(definition)?;
            Ok(QueryExecutionResult {
                output: self.execute_get_party_rule_set(&request).await?,
            })
        })
    }
}

fn decode_input<T: Message + Default>(request: &QueryRequest) -> Result<T, SdkError> {
    let payload = &request.input;
    if payload.owner.as_str() != MODULE_ID
        || payload.schema_id.as_str() != GET_PARTY_RULE_SET_REQUEST_SCHEMA
        || payload.schema_version.as_str() != support::CONTRACT_VERSION
        || payload.descriptor_hash
            != support::message_descriptor_hash(GET_PARTY_RULE_SET_REQUEST_SCHEMA)
        || payload.data_class != DataClass::Confidential
        || payload.encoding != PayloadEncoding::Protobuf
        || payload.maximum_size_bytes != support::MAX_PROTOBUF_BYTES
        || payload.validate().is_err()
    {
        return Err(SdkError::new(
            "DATA_QUALITY_QUERY_INPUT_CONTRACT_MISMATCH",
            ErrorCategory::InvalidArgument,
            false,
            "The Data Quality query input does not match the required contract.",
        ));
    }
    T::decode(payload.bytes.as_slice()).map_err(|_| {
        SdkError::new(
            "DATA_QUALITY_QUERY_INPUT_PROTOBUF_INVALID",
            ErrorCategory::InvalidArgument,
            false,
            "The Data Quality query input is not valid Protobuf.",
        )
    })
}

fn required_rule_set_ref(
    value: Option<wire::PartyRuleSetVersionRef>,
) -> Result<wire::PartyRuleSetVersionRef, SdkError> {
    value.ok_or_else(|| {
        SdkError::invalid_argument(
            "data_quality.party_rule_set.rule_set_version_ref",
            "Party rule-set version reference is required",
        )
    })
}

fn ensure_definition(definition: &CapabilityDefinition) -> Result<(), SdkError> {
    if definition.capability_id.as_str() != GET_PARTY_RULE_SET_CAPABILITY
        || definition.owner_module_id.as_str() != MODULE_ID
    {
        return Err(unsupported_query());
    }
    Ok(())
}

fn module_id() -> Result<ModuleId, SdkError> {
    ModuleId::try_new(MODULE_ID).map_err(|_| configuration_error())
}

fn rule_set_record_type() -> Result<RecordType, SdkError> {
    RecordType::try_new(PARTY_RULE_SET_VERSION_RECORD_TYPE).map_err(|_| configuration_error())
}

fn resource_not_found() -> SdkError {
    SdkError::new(
        "DATA_QUALITY_PARTY_RULE_SET_NOT_FOUND",
        ErrorCategory::NotFound,
        false,
        "The requested Party rule-set version was not found.",
    )
}

fn unsupported_query() -> SdkError {
    SdkError::new(
        "DATA_QUALITY_QUERY_UNSUPPORTED",
        ErrorCategory::InvalidArgument,
        false,
        "The requested Data Quality query is not supported.",
    )
}

fn configuration_error() -> SdkError {
    SdkError::new(
        "DATA_QUALITY_QUERY_CONFIGURATION_INVALID",
        ErrorCategory::Internal,
        false,
        "The Data Quality query configuration is invalid.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rule_set_get_is_exact_confidential_read_only_surface() {
        let definition = query_capability_definition().unwrap();
        assert_eq!(
            definition.capability_id.as_str(),
            GET_PARTY_RULE_SET_CAPABILITY
        );
        assert_eq!(definition.owner_module_id.as_str(), MODULE_ID);
        assert_eq!(
            definition.input_contract.allowed_data_classes,
            vec![DataClass::Confidential]
        );
        assert_eq!(definition.risk, CapabilityRisk::Low);
        assert!(!definition.mutation);
        assert!(!definition.requires_idempotency);
        assert!(!definition.requires_approval);
    }
}
