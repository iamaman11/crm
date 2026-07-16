#![forbid(unsafe_code)]

use crm_capability_plan_support as support;
use crm_capability_runtime::{CapabilityDefinition, CapabilityRisk};
use crm_core_data::{PostgresDataStore, RecordGetQuery};
use crm_data_quality::{
    PARTY_COMPLETENESS_PROFILE_VERSION_RECORD_TYPE, PARTY_RULE_SET_VERSION_RECORD_TYPE,
};
use crm_data_quality_capability_adapter::{
    MODULE_ID, completeness_profile_rule_set_version_id_from_snapshot,
    party_completeness_profile_from_immutable_snapshot, party_completeness_profile_to_wire,
    party_rule_set_from_snapshot, party_rule_set_to_wire,
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

pub const GET_PARTY_COMPLETENESS_PROFILE_CAPABILITY: &str =
    "data_quality.party.completeness_profile.get";
pub const GET_PARTY_COMPLETENESS_PROFILE_REQUEST_SCHEMA: &str =
    "crm.data_quality.v1.GetPartyCompletenessProfileVersionRequest";
pub const GET_PARTY_COMPLETENESS_PROFILE_RESPONSE_SCHEMA: &str =
    "crm.data_quality.v1.GetPartyCompletenessProfileVersionResponse";

pub const QUERY_CAPABILITY_IDS: &[&str] = &[
    GET_PARTY_RULE_SET_CAPABILITY,
    GET_PARTY_COMPLETENESS_PROFILE_CAPABILITY,
];

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
        let command: wire::GetPartyRuleSetVersionRequest = decode_input(
            request,
            GET_PARTY_RULE_SET_REQUEST_SCHEMA,
            "DATA_QUALITY_RULE_SET_QUERY_INPUT",
        )?;
        let version_ref = required_rule_set_ref(command.rule_set_version_ref)?;
        let snapshot = self
            .load_snapshot(
                request,
                PARTY_RULE_SET_VERSION_RECORD_TYPE,
                version_ref.rule_set_version_id,
                rule_set_not_found,
            )
            .await?;
        let visibility = self
            .visibility
            .authorize_visibility(request, &snapshot.reference)
            .await?;
        if !visibility.resource_visible {
            return Err(rule_set_not_found());
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

    async fn execute_get_party_completeness_profile(
        &self,
        request: &QueryRequest,
    ) -> Result<TypedPayload, SdkError> {
        let command: wire::GetPartyCompletenessProfileVersionRequest = decode_input(
            request,
            GET_PARTY_COMPLETENESS_PROFILE_REQUEST_SCHEMA,
            "DATA_QUALITY_COMPLETENESS_PROFILE_QUERY_INPUT",
        )?;
        let version_ref =
            required_completeness_profile_ref(command.completeness_profile_version_ref)?;
        let snapshot = self
            .load_snapshot(
                request,
                PARTY_COMPLETENESS_PROFILE_VERSION_RECORD_TYPE,
                version_ref.completeness_profile_version_id,
                completeness_profile_not_found,
            )
            .await?;
        let visibility = self
            .visibility
            .authorize_visibility(request, &snapshot.reference)
            .await?;
        if !visibility.resource_visible {
            return Err(completeness_profile_not_found());
        }

        let rule_set_version_id =
            completeness_profile_rule_set_version_id_from_snapshot(&snapshot)?;
        let rule_set_snapshot = self
            .load_snapshot(
                request,
                PARTY_RULE_SET_VERSION_RECORD_TYPE,
                rule_set_version_id,
                persisted_reference_missing,
            )
            .await?;
        let rule_set = party_rule_set_from_snapshot(&rule_set_snapshot)?;
        let profile = party_completeness_profile_from_immutable_snapshot(&snapshot, &rule_set)?;
        let mut output = party_completeness_profile_to_wire(&profile);
        if !visibility.allows_field("definition") {
            output.definition = None;
        }

        support::protobuf_payload(
            MODULE_ID,
            GET_PARTY_COMPLETENESS_PROFILE_RESPONSE_SCHEMA,
            DataClass::Confidential,
            &wire::GetPartyCompletenessProfileVersionResponse {
                completeness_profile_version: Some(output),
            },
        )
    }

    async fn load_snapshot(
        &self,
        request: &QueryRequest,
        record_type: &'static str,
        record_id: String,
        missing: fn() -> SdkError,
    ) -> Result<crm_module_sdk::RecordSnapshot, SdkError> {
        let record_id = RecordId::try_new(record_id).map_err(|_| missing())?;
        self.store
            .get_record_for_query(&RecordGetQuery {
                tenant_id: request.context.tenant_id.clone(),
                owner_module_id: module_id()?,
                record_type: RecordType::try_new(record_type).map_err(|_| configuration_error())?,
                record_id,
            })
            .await?
            .ok_or_else(missing)
    }
}

pub fn query_capability_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {
    Ok(vec![
        rule_set_query_capability_definition()?,
        completeness_profile_query_capability_definition()?,
    ])
}

pub fn query_capability_definition() -> Result<CapabilityDefinition, SdkError> {
    rule_set_query_capability_definition()
}

pub fn rule_set_query_capability_definition() -> Result<CapabilityDefinition, SdkError> {
    query_definition(
        GET_PARTY_RULE_SET_CAPABILITY,
        GET_PARTY_RULE_SET_REQUEST_SCHEMA,
        GET_PARTY_RULE_SET_RESPONSE_SCHEMA,
    )
}

pub fn completeness_profile_query_capability_definition() -> Result<CapabilityDefinition, SdkError>
{
    query_definition(
        GET_PARTY_COMPLETENESS_PROFILE_CAPABILITY,
        GET_PARTY_COMPLETENESS_PROFILE_REQUEST_SCHEMA,
        GET_PARTY_COMPLETENESS_PROFILE_RESPONSE_SCHEMA,
    )
}

fn query_definition(
    capability_id: &'static str,
    input_schema: &'static str,
    output_schema: &'static str,
) -> Result<CapabilityDefinition, SdkError> {
    Ok(CapabilityDefinition {
        capability_id: support::configured_identifier(CapabilityId::try_new(capability_id))?,
        capability_version: support::configured_identifier(CapabilityVersion::try_new(
            support::CONTRACT_VERSION,
        ))?,
        owner_module_id: support::configured_identifier(ModuleId::try_new(MODULE_ID))?,
        input_contract: support::protobuf_contract(
            MODULE_ID,
            input_schema,
            vec![DataClass::Confidential],
        )?,
        output_contract: Some(support::protobuf_contract(
            MODULE_ID,
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

impl QuerySemanticValidator for DataQualityQueryAdapter {
    fn validate<'a>(
        &'a self,
        definition: &'a CapabilityDefinition,
        request: &'a QueryRequest,
    ) -> PortFuture<'a, Result<(), SdkError>> {
        Box::pin(async move {
            ensure_definition(definition)?;
            match definition.capability_id.as_str() {
                GET_PARTY_RULE_SET_CAPABILITY => {
                    let command: wire::GetPartyRuleSetVersionRequest = decode_input(
                        request,
                        GET_PARTY_RULE_SET_REQUEST_SCHEMA,
                        "DATA_QUALITY_RULE_SET_QUERY_INPUT",
                    )?;
                    validate_record_id(
                        required_rule_set_ref(command.rule_set_version_ref)?.rule_set_version_id,
                        "data_quality.party_rule_set.rule_set_version_ref.rule_set_version_id",
                    )
                }
                GET_PARTY_COMPLETENESS_PROFILE_CAPABILITY => {
                    let command: wire::GetPartyCompletenessProfileVersionRequest = decode_input(
                        request,
                        GET_PARTY_COMPLETENESS_PROFILE_REQUEST_SCHEMA,
                        "DATA_QUALITY_COMPLETENESS_PROFILE_QUERY_INPUT",
                    )?;
                    validate_record_id(
                        required_completeness_profile_ref(
                            command.completeness_profile_version_ref,
                        )?
                        .completeness_profile_version_id,
                        "data_quality.party_completeness_profile.completeness_profile_version_ref.completeness_profile_version_id",
                    )
                }
                _ => Err(unsupported_query()),
            }
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
            let output = match definition.capability_id.as_str() {
                GET_PARTY_RULE_SET_CAPABILITY => {
                    self.execute_get_party_rule_set(&request).await?
                }
                GET_PARTY_COMPLETENESS_PROFILE_CAPABILITY => {
                    self.execute_get_party_completeness_profile(&request).await?
                }
                _ => return Err(unsupported_query()),
            };
            Ok(QueryExecutionResult { output })
        })
    }
}

fn decode_input<T: Message + Default>(
    request: &QueryRequest,
    schema_id: &'static str,
    code_prefix: &'static str,
) -> Result<T, SdkError> {
    let payload = &request.input;
    if payload.owner.as_str() != MODULE_ID
        || payload.schema_id.as_str() != schema_id
        || payload.schema_version.as_str() != support::CONTRACT_VERSION
        || payload.descriptor_hash != support::message_descriptor_hash(schema_id)
        || payload.data_class != DataClass::Confidential
        || payload.encoding != PayloadEncoding::Protobuf
        || payload.maximum_size_bytes != support::MAX_PROTOBUF_BYTES
        || payload.validate().is_err()
    {
        return Err(SdkError::new(
            format!("{code_prefix}_CONTRACT_MISMATCH"),
            ErrorCategory::InvalidArgument,
            false,
            "The Data Quality query input does not match the required contract.",
        ));
    }
    T::decode(payload.bytes.as_slice()).map_err(|_| {
        SdkError::new(
            format!("{code_prefix}_PROTOBUF_INVALID"),
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

fn required_completeness_profile_ref(
    value: Option<wire::PartyCompletenessProfileVersionRef>,
) -> Result<wire::PartyCompletenessProfileVersionRef, SdkError> {
    value.ok_or_else(|| {
        SdkError::invalid_argument(
            "data_quality.party_completeness_profile.completeness_profile_version_ref",
            "Party completeness-profile version reference is required",
        )
    })
}

fn validate_record_id(value: String, field: &'static str) -> Result<(), SdkError> {
    RecordId::try_new(value)
        .map(|_| ())
        .map_err(|_| SdkError::invalid_argument(field, "Data Quality version reference is invalid"))
}

fn ensure_definition(definition: &CapabilityDefinition) -> Result<(), SdkError> {
    if !QUERY_CAPABILITY_IDS.contains(&definition.capability_id.as_str())
        || definition.owner_module_id.as_str() != MODULE_ID
    {
        return Err(unsupported_query());
    }
    Ok(())
}

fn module_id() -> Result<ModuleId, SdkError> {
    ModuleId::try_new(MODULE_ID).map_err(|_| configuration_error())
}

fn rule_set_not_found() -> SdkError {
    SdkError::new(
        "DATA_QUALITY_PARTY_RULE_SET_NOT_FOUND",
        ErrorCategory::NotFound,
        false,
        "The requested Party rule-set version was not found.",
    )
}

fn completeness_profile_not_found() -> SdkError {
    SdkError::new(
        "DATA_QUALITY_PARTY_COMPLETENESS_PROFILE_NOT_FOUND",
        ErrorCategory::NotFound,
        false,
        "The requested Party completeness-profile version was not found.",
    )
}

fn persisted_reference_missing() -> SdkError {
    SdkError::new(
        "DATA_QUALITY_PERSISTED_STATE_INVALID",
        ErrorCategory::Internal,
        false,
        "The persisted Data Quality state is invalid.",
    )
    .with_internal_reference("Party completeness profile references a missing rule-set version")
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
    fn immutable_definition_queries_are_exact_confidential_read_only_surfaces() {
        let definitions = query_capability_definitions().unwrap();
        assert_eq!(definitions.len(), 2);
        for (definition, expected_capability) in definitions.iter().zip(QUERY_CAPABILITY_IDS) {
            assert_eq!(definition.capability_id.as_str(), *expected_capability);
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
}
