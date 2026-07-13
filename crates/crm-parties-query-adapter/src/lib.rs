#![forbid(unsafe_code)]

use crm_capability_plan_support as support;
use crm_capability_runtime::{CapabilityDefinition, CapabilityRisk};
use crm_core_data::{PostgresDataStore, RecordGetQuery};
use crm_module_sdk::{
    CapabilityId, CapabilityVersion, DataClass, ErrorCategory, ModuleId, PayloadEncoding, PortFuture,
    RecordId, RecordType, SdkError, TypedPayload,
};
use crm_parties::decode_party_state;
use crm_parties_capability_adapter::{MODULE_ID, RECORD_TYPE, party_to_wire, persisted_contract};
use crm_proto_contracts::crm::parties::v1 as wire;
use crm_query_runtime::{
    QueryExecutionResult, QueryExecutor, QueryRequest, QuerySemanticValidator,
    QueryVisibilityAuthorizer,
};
use prost::Message;
use std::sync::Arc;

pub const GET_CAPABILITY: &str = "parties.party.get";
pub const GET_REQUEST_SCHEMA: &str = "crm.parties.v1.GetPartyRequest";
pub const GET_RESPONSE_SCHEMA: &str = "crm.parties.v1.GetPartyResponse";

#[derive(Clone)]
pub struct PartyQueryAdapter {
    store: PostgresDataStore,
    visibility: Arc<dyn QueryVisibilityAuthorizer>,
}

impl std::fmt::Debug for PartyQueryAdapter {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PartyQueryAdapter")
            .field("store", &self.store)
            .field("visibility", &"dyn QueryVisibilityAuthorizer")
            .finish()
    }
}

impl PartyQueryAdapter {
    pub fn new(store: PostgresDataStore, visibility: Arc<dyn QueryVisibilityAuthorizer>) -> Self {
        Self { store, visibility }
    }
}

pub fn query_capability_definition() -> Result<CapabilityDefinition, SdkError> {
    Ok(CapabilityDefinition {
        capability_id: support::configured_identifier(CapabilityId::try_new(GET_CAPABILITY))?,
        capability_version: support::configured_identifier(CapabilityVersion::try_new(
            support::CONTRACT_VERSION,
        ))?,
        owner_module_id: support::configured_identifier(ModuleId::try_new(MODULE_ID))?,
        input_contract: support::protobuf_contract(
            MODULE_ID,
            GET_REQUEST_SCHEMA,
            vec![DataClass::Personal],
        )?,
        output_contract: Some(support::protobuf_contract(
            MODULE_ID,
            GET_RESPONSE_SCHEMA,
            vec![DataClass::Personal],
        )?),
        risk: CapabilityRisk::Low,
        mutation: false,
        requires_idempotency: false,
        requires_approval: false,
        authorization_policy_id: GET_CAPABILITY.to_owned(),
        rate_limit_policy_id: None,
    })
}

impl QuerySemanticValidator for PartyQueryAdapter {
    fn validate<'a>(
        &'a self,
        definition: &'a CapabilityDefinition,
        request: &'a QueryRequest,
    ) -> PortFuture<'a, Result<(), SdkError>> {
        Box::pin(async move {
            ensure_definition(definition)?;
            let command: wire::GetPartyRequest = decode_input(request)?;
            let party_ref = command
                .party_ref
                .ok_or_else(|| SdkError::invalid_argument("party.party_ref", "Party reference is required"))?;
            validate_record_id(&party_ref.party_id)?;
            Ok(())
        })
    }
}

impl QueryExecutor for PartyQueryAdapter {
    fn execute<'a>(
        &'a self,
        definition: &'a CapabilityDefinition,
        request: QueryRequest,
    ) -> PortFuture<'a, Result<QueryExecutionResult, SdkError>> {
        Box::pin(async move {
            ensure_definition(definition)?;
            let command: wire::GetPartyRequest = decode_input(&request)?;
            let party_ref = command
                .party_ref
                .ok_or_else(|| SdkError::invalid_argument("party.party_ref", "Party reference is required"))?;
            let record_id = validate_record_id(&party_ref.party_id)?;
            let snapshot = self
                .store
                .get_record_for_query(&RecordGetQuery {
                    tenant_id: request.context.tenant_id.clone(),
                    owner_module_id: ModuleId::try_new(MODULE_ID).map_err(config_error)?,
                    record_type: RecordType::try_new(RECORD_TYPE).map_err(config_error)?,
                    record_id,
                })
                .await?
                .ok_or_else(resource_not_found)?;
            let visibility = self
                .visibility
                .authorize_visibility(&request, &snapshot.reference)
                .await?;
            if !visibility.resource_visible {
                return Err(resource_not_found());
            }

            let contract = persisted_contract();
            let party = decode_party_state(support::persisted_json_bytes(&snapshot, contract)?)?;
            if party.party_id().as_str() != snapshot.reference.record_id.as_str()
                || party.version() != snapshot.version
            {
                return Err(support::stored_data_error(
                    "PARTIES_PERSISTED_PARTY_IDENTITY_INVALID",
                ));
            }

            let output = support::protobuf_payload(
                MODULE_ID,
                GET_RESPONSE_SCHEMA,
                DataClass::Personal,
                &wire::GetPartyResponse {
                    party: Some(party_to_wire(&party)),
                },
            )?;
            Ok(QueryExecutionResult { output })
        })
    }
}

fn ensure_definition(definition: &CapabilityDefinition) -> Result<(), SdkError> {
    if definition.capability_id.as_str() != GET_CAPABILITY
        || definition.owner_module_id.as_str() != MODULE_ID
        || definition.mutation
    {
        return Err(unsupported_query());
    }
    Ok(())
}

fn decode_input(request: &QueryRequest) -> Result<wire::GetPartyRequest, SdkError> {
    let payload = &request.input;
    if payload.owner.as_str() != MODULE_ID
        || payload.schema_id.as_str() != GET_REQUEST_SCHEMA
        || payload.schema_version.as_str() != support::CONTRACT_VERSION
        || payload.descriptor_hash != support::message_descriptor_hash(GET_REQUEST_SCHEMA)
        || payload.data_class != DataClass::Personal
        || payload.encoding != PayloadEncoding::Protobuf
        || payload.maximum_size_bytes > support::MAX_PROTOBUF_BYTES
        || payload.validate().is_err()
    {
        return Err(SdkError::new(
            "PARTIES_QUERY_INPUT_CONTRACT_MISMATCH",
            ErrorCategory::InvalidArgument,
            false,
            "The Party query input does not match the required contract.",
        ));
    }
    wire::GetPartyRequest::decode(payload.bytes.as_slice()).map_err(|_| {
        SdkError::new(
            "PARTIES_QUERY_INPUT_PROTOBUF_INVALID",
            ErrorCategory::InvalidArgument,
            false,
            "The Party query input is not valid Protobuf.",
        )
    })
}

fn validate_record_id(value: &str) -> Result<RecordId, SdkError> {
    RecordId::try_new(value.to_owned())
        .map_err(|error| SdkError::invalid_argument("party.party_ref.party_id", error.to_string()))
}

fn resource_not_found() -> SdkError {
    SdkError::new(
        "QUERY_RESOURCE_NOT_FOUND",
        ErrorCategory::NotFound,
        false,
        "The requested resource was not found.",
    )
}

fn unsupported_query() -> SdkError {
    SdkError::new(
        "PARTIES_QUERY_CAPABILITY_UNSUPPORTED",
        ErrorCategory::Internal,
        false,
        "The Party query capability is not configured.",
    )
}

fn config_error(error: crm_module_sdk::IdentifierError) -> SdkError {
    SdkError::new(
        "PARTIES_QUERY_CONFIGURATION_INVALID",
        ErrorCategory::Internal,
        false,
        "The Party query adapter is not configured safely.",
    )
    .with_internal_reference(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_query_is_personal_read_only_and_not_idempotency_bound() {
        let definition = query_capability_definition().unwrap();
        assert_eq!(definition.owner_module_id.as_str(), MODULE_ID);
        assert!(!definition.mutation);
        assert!(!definition.requires_idempotency);
        assert_eq!(definition.input_contract.data_classes, vec![DataClass::Personal]);
    }
}
