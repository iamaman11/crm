#![forbid(unsafe_code)]

//! Governed minimized Party reads for deterministic Data Quality staging.
//!
//! This composition performs a separate live top-level Party GET authorization,
//! then executes the normal Party query adapter. Tenant/RLS and live resource and
//! field visibility therefore remain owned by the Party query boundary.

mod staging_command;
mod staging_context;
mod staging_execute;
mod staging_request;
pub mod staging_sink;
pub mod worker;
mod worker_context;

pub use staging_sink::*;
pub use worker::*;
pub use worker_context::{EVALUATION_WORKER_ACTOR_ID, EVALUATION_WORKER_CAPABILITY_VERSION};

use crm_capability_runtime::CapabilityDefinition;
use crm_module_sdk::{ActorId, ErrorCategory, PortFuture, RecordId, SdkError, TenantId};
use crm_parties_query_adapter::{
    GET_CAPABILITY as PARTY_GET_CAPABILITY, PartyQueryAdapter, export_execution_query_request,
    query_capability_definition,
};
use crm_proto_contracts::crm::parties::v1 as wire;
use crm_query_runtime::{QueryAuthorizer, QueryExecutor, QuerySemanticValidator};
use prost::Message;
use std::fmt;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartyQualitySourceKind {
    Person,
    Organization,
}

#[derive(Debug, Clone, Copy)]
pub struct PartyQualitySourceRequest<'a> {
    pub tenant_id: &'a TenantId,
    pub actor_id: &'a ActorId,
    pub request_identity: &'a str,
    pub party_id: &'a RecordId,
    pub request_started_at_unix_nanos: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartyQualitySourceSnapshot {
    pub party_id: RecordId,
    pub kind: PartyQualitySourceKind,
    pub display_name: String,
    pub resource_version: i64,
}

pub trait PartyQualitySource: Send + Sync {
    fn get<'a>(
        &'a self,
        request: PartyQualitySourceRequest<'a>,
    ) -> PortFuture<'a, Result<PartyQualitySourceSnapshot, SdkError>>;
}

#[derive(Clone)]
pub struct GovernedPartyQualitySource {
    adapter: Arc<PartyQueryAdapter>,
    query_authorizer: Arc<dyn QueryAuthorizer>,
}

impl GovernedPartyQualitySource {
    pub fn new(
        adapter: Arc<PartyQueryAdapter>,
        query_authorizer: Arc<dyn QueryAuthorizer>,
    ) -> Self {
        Self {
            adapter,
            query_authorizer,
        }
    }
}

impl fmt::Debug for GovernedPartyQualitySource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GovernedPartyQualitySource")
            .field("adapter", &"PartyQueryAdapter")
            .field("query_authorizer", &"dyn QueryAuthorizer")
            .finish()
    }
}

impl PartyQualitySource for GovernedPartyQualitySource {
    fn get<'a>(
        &'a self,
        source_request: PartyQualitySourceRequest<'a>,
    ) -> PortFuture<'a, Result<PartyQualitySourceSnapshot, SdkError>> {
        Box::pin(async move {
            let request = export_execution_query_request(
                source_request.tenant_id,
                source_request.actor_id,
                source_request.request_identity,
                source_request.party_id,
                source_request.request_started_at_unix_nanos,
            )?;
            let definition = query_capability_definition(PARTY_GET_CAPABILITY)?;
            authorize_party_get(self.query_authorizer.as_ref(), &definition, &request).await?;
            self.adapter.validate(&definition, &request).await?;
            let result = self.adapter.execute(&definition, request).await?;
            let response = wire::GetPartyResponse::decode(result.output.bytes.as_slice())
                .map_err(|error| source_contract_error(error.to_string()))?;
            let party = response.party.ok_or_else(source_not_found)?;
            let party_ref = party.party_ref.ok_or_else(source_not_found)?;
            if party_ref.party_id != source_request.party_id.as_str() {
                return Err(source_contract_error(
                    "Party query returned a different resource identity",
                ));
            }
            let kind = match wire::PartyKind::try_from(party.kind) {
                Ok(wire::PartyKind::Person) => PartyQualitySourceKind::Person,
                Ok(wire::PartyKind::Organization) => PartyQualitySourceKind::Organization,
                Ok(wire::PartyKind::Unspecified) | Err(_) => return Err(source_not_found()),
            };
            if party.display_name.is_empty() {
                return Err(source_not_found());
            }
            let resource_version = party
                .resource_version
                .ok_or_else(|| source_contract_error("Party resource version is missing"))?
                .version;
            if resource_version <= 0 {
                return Err(source_contract_error(
                    "Party resource version must be positive",
                ));
            }
            Ok(PartyQualitySourceSnapshot {
                party_id: source_request.party_id.clone(),
                kind,
                display_name: party.display_name,
                resource_version,
            })
        })
    }
}

async fn authorize_party_get(
    authorizer: &dyn QueryAuthorizer,
    definition: &CapabilityDefinition,
    request: &crm_query_runtime::QueryRequest,
) -> Result<(), SdkError> {
    let decision = authorizer.authorize(definition, request).await?;
    if decision.allowed {
        return Ok(());
    }
    Err(SdkError::new(
        "DATA_QUALITY_PARTY_SOURCE_PERMISSION_DENIED",
        ErrorCategory::Authorization,
        false,
        "The Data Quality worker is not authorized to read the requested Party.",
    )
    .with_internal_reference(format!(
        "decision_id={} reason_code={} policy_version={}",
        decision.decision_id, decision.reason_code, decision.policy_version
    )))
}

fn source_not_found() -> SdkError {
    SdkError::new(
        "QUERY_RESOURCE_NOT_FOUND",
        ErrorCategory::NotFound,
        false,
        "The requested resource was not found.",
    )
}

fn source_contract_error(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "DATA_QUALITY_PARTY_SOURCE_CONTRACT_INVALID",
        ErrorCategory::Internal,
        false,
        "The governed Party quality source returned invalid evidence.",
    )
    .with_internal_reference(reference.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_send_sync<T: Send + Sync>() {}

    #[test]
    fn minimized_source_contract_is_closed_and_thread_safe() {
        assert_ne!(
            PartyQualitySourceKind::Person,
            PartyQualitySourceKind::Organization
        );
        assert_send_sync::<GovernedPartyQualitySource>();
        assert_send_sync::<PartyEvaluationStageWorker>();
    }
}
