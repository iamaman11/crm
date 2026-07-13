#![forbid(unsafe_code)]

mod base_adapter;

pub use base_adapter::{
    AUTHORIZE_CAPABILITY, AUTHORIZE_REQUEST_SCHEMA, AUTHORIZE_RESPONSE_SCHEMA, GET_CAPABILITY,
    GET_REQUEST_SCHEMA, GET_RESPONSE_SCHEMA, LIST_CAPABILITY, LIST_REQUEST_SCHEMA,
    LIST_RESPONSE_SCHEMA, QUERY_CAPABILITY_IDS, query_capability_definition,
    query_capability_definitions,
};

use crm_capability_plan_support as support;
use crm_capability_runtime::CapabilityDefinition;
use crm_consents::{
    CommunicationAuthorizationReason as DomainAuthorizationReason, CommunicationChannel,
    ConsentAuthorization, ContactPointReference, EvaluateCommunicationAuthorization,
    PartyReference, PurposeCode, evaluate_communication_authorization,
};
use crm_consents_capability_adapter::{
    MODULE_ID, PARTY_AUTHORIZATION_RELATIONSHIP_TYPE, PARTY_AUTHORIZATION_SOURCE_RECORD_TYPE,
    RECORD_TYPE, consent_authorization_from_snapshot,
};
use crm_core_data::{
    MAXIMUM_RELATED_RECORD_QUERY_PAGE_SIZE, PostgresDataStore, RelatedRecordListQuery,
};
use crm_module_sdk::{
    DataClass, ErrorCategory, ModuleId, PayloadEncoding, PortFuture, RecordId, RecordRef,
    RecordType, RelationshipType, SdkError, TypedPayload,
};
use crm_proto_contracts::crm::{consents::v1 as wire, core::v1 as core, customer::v1 as customer};
use crm_query_runtime::{
    CursorCodec, QueryExecutionResult, QueryExecutor, QueryRequest, QuerySemanticValidator,
    QueryVisibilityAuthorizer,
};
use prost::Message;
use std::fmt;
use std::sync::Arc;

/// Consent query adapter that delegates permission-aware get/list behavior to
/// the proven implementation and resolves communication authorization through
/// an authoritative Party -> Consent relationship access path.
#[derive(Clone)]
pub struct ConsentQueryAdapter {
    base_adapter: base_adapter::ConsentQueryAdapter,
    store: PostgresDataStore,
}

impl fmt::Debug for ConsentQueryAdapter {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ConsentQueryAdapter")
            .field("base_adapter", &self.base_adapter)
            .field("store", &self.store)
            .finish()
    }
}

impl ConsentQueryAdapter {
    pub fn new(
        store: PostgresDataStore,
        cursor_codec: CursorCodec,
        visibility: Arc<dyn QueryVisibilityAuthorizer>,
    ) -> Result<Self, SdkError> {
        let base_adapter =
            base_adapter::ConsentQueryAdapter::new(store.clone(), cursor_codec, visibility)?;
        Ok(Self {
            base_adapter,
            store,
        })
    }

    async fn execute_authorize(&self, request: &QueryRequest) -> Result<TypedPayload, SdkError> {
        let command = decode_authorize_input(request)?;
        let evaluation = evaluate_command(request, command)?;
        let decision = match self
            .load_authorization_candidates(request, &evaluation)
            .await
        {
            Ok(authorizations) => {
                evaluate_communication_authorization(&evaluation, authorizations.iter())?
            }
            Err(error) if error.code.as_str() == "DATA_QUERY_UNAVAILABLE" => {
                return authorization_payload(
                    &evaluation,
                    false,
                    wire::CommunicationAuthorizationReason::DataUnavailable,
                    Vec::new(),
                );
            }
            Err(error) => return Err(error),
        };

        let reason = match decision.reason {
            DomainAuthorizationReason::ActiveGrant => {
                wire::CommunicationAuthorizationReason::ActiveGrant
            }
            DomainAuthorizationReason::ActiveDeny => {
                wire::CommunicationAuthorizationReason::ActiveDeny
            }
            DomainAuthorizationReason::Withdrawn => {
                wire::CommunicationAuthorizationReason::Withdrawn
            }
            DomainAuthorizationReason::NoApplicableGrant => {
                wire::CommunicationAuthorizationReason::NoApplicableGrant
            }
        };
        authorization_payload(
            &evaluation,
            decision.allowed,
            reason,
            decision
                .determining_authorization_ids
                .into_iter()
                .map(|authorization_id| wire::ConsentAuthorizationRef {
                    authorization_id: authorization_id.as_str().to_owned(),
                })
                .collect(),
        )
    }

    async fn load_authorization_candidates(
        &self,
        request: &QueryRequest,
        evaluation: &EvaluateCommunicationAuthorization,
    ) -> Result<Vec<ConsentAuthorization>, SdkError> {
        let mut after_record_id = None;
        let mut authorizations = Vec::new();
        loop {
            let page = self
                .store
                .list_related_records_for_query(&RelatedRecordListQuery {
                    tenant_id: request.context.tenant_id.clone(),
                    relationship_owner_module_id: configured_module_id(MODULE_ID)?,
                    relationship_type: configured_relationship_type(
                        PARTY_AUTHORIZATION_RELATIONSHIP_TYPE,
                    )?,
                    source: RecordRef {
                        record_type: configured_record_type(
                            PARTY_AUTHORIZATION_SOURCE_RECORD_TYPE,
                        )?,
                        record_id: RecordId::try_new(evaluation.party_ref.as_str())
                            .map_err(config_error)?,
                    },
                    target_owner_module_id: configured_module_id(MODULE_ID)?,
                    target_record_type: configured_record_type(RECORD_TYPE)?,
                    page_size: MAXIMUM_RELATED_RECORD_QUERY_PAGE_SIZE,
                    after_record_id: after_record_id.clone(),
                })
                .await?;

            authorizations.extend(
                page.records
                    .iter()
                    .map(consent_authorization_from_snapshot)
                    .collect::<Result<Vec<_>, _>>()?,
            );
            after_record_id = page.next_record_id;
            if after_record_id.is_none() {
                return Ok(authorizations);
            }
        }
    }
}

impl QuerySemanticValidator for ConsentQueryAdapter {
    fn validate<'a>(
        &'a self,
        definition: &'a CapabilityDefinition,
        request: &'a QueryRequest,
    ) -> PortFuture<'a, Result<(), SdkError>> {
        self.base_adapter.validate(definition, request)
    }
}

impl QueryExecutor for ConsentQueryAdapter {
    fn execute<'a>(
        &'a self,
        definition: &'a CapabilityDefinition,
        request: QueryRequest,
    ) -> PortFuture<'a, Result<QueryExecutionResult, SdkError>> {
        if definition.capability_id.as_str() != AUTHORIZE_CAPABILITY {
            return self.base_adapter.execute(definition, request);
        }
        Box::pin(async move {
            let output = self.execute_authorize(&request).await?;
            Ok(QueryExecutionResult { output })
        })
    }
}

fn decode_authorize_input(
    request: &QueryRequest,
) -> Result<wire::AuthorizeCommunicationRequest, SdkError> {
    let payload = &request.input;
    if payload.owner.as_str() != MODULE_ID
        || payload.schema_id.as_str() != AUTHORIZE_REQUEST_SCHEMA
        || payload.schema_version.as_str() != support::CONTRACT_VERSION
        || payload.descriptor_hash != support::message_descriptor_hash(AUTHORIZE_REQUEST_SCHEMA)
        || payload.data_class != DataClass::Personal
        || payload.encoding != PayloadEncoding::Protobuf
        || payload.maximum_size_bytes > support::MAX_PROTOBUF_BYTES
        || payload.validate().is_err()
    {
        return Err(SdkError::new(
            "CONSENTS_QUERY_INPUT_CONTRACT_MISMATCH",
            ErrorCategory::InvalidArgument,
            false,
            "The Consent Authorization query input does not match the required contract.",
        ));
    }
    wire::AuthorizeCommunicationRequest::decode(payload.bytes.as_slice()).map_err(|_| {
        SdkError::new(
            "CONSENTS_QUERY_INPUT_PROTOBUF_INVALID",
            ErrorCategory::InvalidArgument,
            false,
            "The Consent Authorization query input is not valid Protobuf.",
        )
    })
}

fn evaluate_command(
    request: &QueryRequest,
    command: wire::AuthorizeCommunicationRequest,
) -> Result<EvaluateCommunicationAuthorization, SdkError> {
    let party_ref = command.party_ref.ok_or_else(|| {
        SdkError::invalid_argument(
            "communication_authorization.party_ref",
            "Party reference is required",
        )
    })?;
    Ok(EvaluateCommunicationAuthorization {
        party_ref: PartyReference::try_new(party_ref.party_id)?,
        contact_point_ref: command
            .contact_point_ref
            .map(|value| ContactPointReference::try_new(value.contact_point_id))
            .transpose()?,
        purpose: PurposeCode::try_new(command.purpose)?,
        channel: required_channel(command.channel)?,
        evaluation_time_unix_nanos: request.context.request_started_at_unix_nanos,
    })
}

fn required_channel(value: i32) -> Result<CommunicationChannel, SdkError> {
    match wire::CommunicationChannel::try_from(value) {
        Ok(wire::CommunicationChannel::Email) => Ok(CommunicationChannel::Email),
        Ok(wire::CommunicationChannel::Phone) => Ok(CommunicationChannel::Phone),
        Ok(wire::CommunicationChannel::Sms) => Ok(CommunicationChannel::Sms),
        Ok(wire::CommunicationChannel::Postal) => Ok(CommunicationChannel::Postal),
        Ok(wire::CommunicationChannel::Messaging) => Ok(CommunicationChannel::Messaging),
        Ok(wire::CommunicationChannel::Push) => Ok(CommunicationChannel::Push),
        Ok(wire::CommunicationChannel::Unspecified) | Err(_) => Err(SdkError::invalid_argument(
            "communication_authorization.channel",
            "Communication channel is required",
        )),
    }
}

fn authorization_payload(
    evaluation: &EvaluateCommunicationAuthorization,
    allowed: bool,
    reason: wire::CommunicationAuthorizationReason,
    determining_authorizations: Vec<wire::ConsentAuthorizationRef>,
) -> Result<TypedPayload, SdkError> {
    support::protobuf_payload(
        MODULE_ID,
        AUTHORIZE_RESPONSE_SCHEMA,
        DataClass::Personal,
        &wire::AuthorizeCommunicationResponse {
            decision: Some(wire::CommunicationAuthorizationDecision {
                allowed,
                reason: reason as i32,
                party_ref: Some(customer::PartyRef {
                    party_id: evaluation.party_ref.as_str().to_owned(),
                }),
                purpose: evaluation.purpose.as_str().to_owned(),
                channel: channel_wire_value(evaluation.channel),
                contact_point_ref: evaluation.contact_point_ref.as_ref().map(|value| {
                    customer::ContactPointRef {
                        contact_point_id: value.as_str().to_owned(),
                    }
                }),
                evaluated_at: Some(core::UnixTime {
                    unix_nanos: evaluation.evaluation_time_unix_nanos,
                }),
                determining_authorizations,
            }),
        },
    )
}

fn channel_wire_value(value: CommunicationChannel) -> i32 {
    match value {
        CommunicationChannel::Email => wire::CommunicationChannel::Email as i32,
        CommunicationChannel::Phone => wire::CommunicationChannel::Phone as i32,
        CommunicationChannel::Sms => wire::CommunicationChannel::Sms as i32,
        CommunicationChannel::Postal => wire::CommunicationChannel::Postal as i32,
        CommunicationChannel::Messaging => wire::CommunicationChannel::Messaging as i32,
        CommunicationChannel::Push => wire::CommunicationChannel::Push as i32,
    }
}

fn configured_module_id(value: &str) -> Result<ModuleId, SdkError> {
    ModuleId::try_new(value).map_err(config_error)
}

fn configured_record_type(value: &str) -> Result<RecordType, SdkError> {
    RecordType::try_new(value).map_err(config_error)
}

fn configured_relationship_type(value: &str) -> Result<RelationshipType, SdkError> {
    RelationshipType::try_new(value).map_err(config_error)
}

fn config_error(error: crm_module_sdk::IdentifierError) -> SdkError {
    SdkError::new(
        "CONSENTS_QUERY_CONFIGURATION_INVALID",
        ErrorCategory::Internal,
        false,
        "The Consent Authorization query adapter is not configured safely.",
    )
    .with_internal_reference(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authoritative_access_path_coordinates_are_stable() {
        assert_eq!(
            configured_relationship_type(PARTY_AUTHORIZATION_RELATIONSHIP_TYPE)
                .unwrap()
                .as_str(),
            "consents.authorization.party"
        );
        assert_eq!(
            configured_record_type(PARTY_AUTHORIZATION_SOURCE_RECORD_TYPE)
                .unwrap()
                .as_str(),
            "parties.party"
        );
    }
}
