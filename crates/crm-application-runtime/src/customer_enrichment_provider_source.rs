use crm_core_data::{PostgresDataStore, RecordGetQuery};
use crm_customer_enrichment::{
    ENRICHMENT_REQUEST_RECORD_TYPE, EnrichmentRequestStatus, PROVIDER_PROFILE_VERSION_RECORD_TYPE,
    PartySnapshot, TargetField,
};
use crm_customer_enrichment_capability_adapter::{
    MODULE_ID, enrichment_request_from_snapshot, provider_profile_from_snapshot,
};
use crm_customer_enrichment_provider_process_composition::{
    ProviderDispatchSourceDisposition, ProviderDispatchSourcePort, ProviderDispatchSourceSnapshot,
};
use crm_module_sdk::{
    ActorId, ErrorCategory, ModuleId, PortFuture, RecordId, RecordType, SdkError, TenantId,
};
use crm_parties_query_adapter::{
    GET_CAPABILITY as PARTY_GET_CAPABILITY, PartyQueryAdapter, export_execution_query_request,
    query_capability_definition as party_query_definition,
};
use crm_proto_contracts::crm::parties::v1 as party_wire;
use crm_query_runtime::{QueryAuthorizer, QueryExecutor, QuerySemanticValidator};
use prost::Message;
use std::fmt;
use std::sync::Arc;

#[derive(Clone)]
pub struct GovernedCustomerEnrichmentProviderSource {
    store: PostgresDataStore,
    party_queries: Arc<PartyQueryAdapter>,
    query_authorizer: Arc<dyn QueryAuthorizer>,
}

impl GovernedCustomerEnrichmentProviderSource {
    pub fn new(
        store: PostgresDataStore,
        party_queries: Arc<PartyQueryAdapter>,
        query_authorizer: Arc<dyn QueryAuthorizer>,
    ) -> Self {
        Self {
            store,
            party_queries,
            query_authorizer,
        }
    }
}

impl fmt::Debug for GovernedCustomerEnrichmentProviderSource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GovernedCustomerEnrichmentProviderSource")
            .field("store", &self.store)
            .field("party_queries", &"PartyQueryAdapter")
            .field("query_authorizer", &"dyn QueryAuthorizer")
            .finish()
    }
}

impl ProviderDispatchSourcePort for GovernedCustomerEnrichmentProviderSource {
    fn load<'a>(
        &'a self,
        tenant_id: TenantId,
        request_id: RecordId,
        worker_actor_id: ActorId,
        now_unix_ms: u64,
    ) -> PortFuture<'a, Result<ProviderDispatchSourceDisposition, SdkError>> {
        Box::pin(async move {
            if now_unix_ms == 0 {
                return Err(source_configuration_invalid(
                    "provider source clock must be after the Unix epoch",
                ));
            }
            let request_snapshot = self
                .store
                .get_record_for_query(&RecordGetQuery {
                    tenant_id: tenant_id.clone(),
                    owner_module_id: module_id()?,
                    record_type: record_type(ENRICHMENT_REQUEST_RECORD_TYPE)?,
                    record_id: request_id.clone(),
                })
                .await?
                .ok_or_else(request_unavailable)?;
            let request = enrichment_request_from_snapshot(&request_snapshot)?;
            if request.request_id().as_str() != request_id.as_str()
                || request.tenant_id() != &tenant_id
            {
                return Err(source_snapshot_invalid(
                    "request snapshot identity does not match the event lookup",
                ));
            }
            if !request_status_requires_provider(request.status()) {
                return Ok(ProviderDispatchSourceDisposition::Skip);
            }
            if request.target().target_field != TargetField::PartyDisplayName {
                return Err(source_snapshot_invalid(
                    "provider source supports only the exact Party display-name target",
                ));
            }

            let profile_record_id =
                RecordId::try_new(request.provider_profile_version_id().as_str().to_owned())
                    .map_err(source_identifier_invalid)?;
            let profile_snapshot = self
                .store
                .get_record_for_query(&RecordGetQuery {
                    tenant_id: tenant_id.clone(),
                    owner_module_id: module_id()?,
                    record_type: record_type(PROVIDER_PROFILE_VERSION_RECORD_TYPE)?,
                    record_id: profile_record_id,
                })
                .await?
                .ok_or_else(provider_profile_unavailable)?;
            let provider_profile = provider_profile_from_snapshot(&profile_snapshot)?;
            if provider_profile.version_id() != request.provider_profile_version_id() {
                return Err(source_snapshot_invalid(
                    "provider profile snapshot identity does not match the request",
                ));
            }

            let party_id = RecordId::try_new(request.target().resource_id.clone())
                .map_err(source_identifier_invalid)?;
            let request_started_at_unix_nanos = now_unix_ms
                .checked_mul(1_000_000)
                .and_then(|value| i64::try_from(value).ok())
                .ok_or_else(|| {
                    source_configuration_invalid(
                        "provider source clock exceeds the supported range",
                    )
                })?;
            let query_identity = format!("provider-source-{}", request.request_id().as_str());
            let query = export_execution_query_request(
                &tenant_id,
                &worker_actor_id,
                &query_identity,
                &party_id,
                request_started_at_unix_nanos,
            )?;
            let definition = party_query_definition(PARTY_GET_CAPABILITY)?;
            let authorization = self.query_authorizer.authorize(&definition, &query).await?;
            if !authorization.allowed {
                return Err(SdkError::new(
                    "CUSTOMER_ENRICHMENT_PROVIDER_PARTY_PERMISSION_DENIED",
                    ErrorCategory::Authorization,
                    false,
                    "The provider worker is not authorized to inspect the target Party.",
                )
                .with_internal_reference(format!(
                    "decision_id={};reason_code={};policy_version={}",
                    authorization.decision_id,
                    authorization.reason_code,
                    authorization.policy_version
                )));
            }
            self.party_queries.validate(&definition, &query).await?;
            let result = self.party_queries.execute(&definition, query).await?;
            let response = party_wire::GetPartyResponse::decode(result.output.bytes.as_slice())
                .map_err(|error| source_snapshot_invalid(error.to_string()))?;
            let party = response.party.ok_or_else(party_unavailable)?;
            let returned_party_id = party
                .party_ref
                .ok_or_else(|| source_snapshot_invalid("Party response reference is missing"))?
                .party_id;
            let returned_party_id =
                RecordId::try_new(returned_party_id).map_err(source_identifier_invalid)?;
            let resource_version = party
                .resource_version
                .ok_or_else(|| source_snapshot_invalid("Party resource version is missing"))?
                .version;
            let expected_resource_version = i64::try_from(request.target().resource_version)
                .map_err(|_| {
                    source_snapshot_invalid("target Party resource version exceeds wire range")
                })?;
            if returned_party_id != party_id {
                return Err(source_snapshot_invalid(
                    "Party response identity does not match the request target",
                ));
            }
            if resource_version != expected_resource_version {
                return Err(SdkError::new(
                    "CUSTOMER_ENRICHMENT_PROVIDER_PARTY_VERSION_CHANGED",
                    ErrorCategory::Conflict,
                    false,
                    "The target Party changed before provider dispatch.",
                ));
            }
            if party.display_name.is_empty() {
                return Err(SdkError::new(
                    "CUSTOMER_ENRICHMENT_PROVIDER_PARTY_FIELD_NOT_VISIBLE",
                    ErrorCategory::Authorization,
                    false,
                    "The provider worker cannot read the target Party display name.",
                ));
            }
            let observed_at_unix_ms = i64::try_from(now_unix_ms).map_err(|_| {
                source_configuration_invalid(
                    "provider source clock exceeds the Party snapshot range",
                )
            })?;
            Ok(ProviderDispatchSourceDisposition::Ready(Box::new(
                ProviderDispatchSourceSnapshot {
                    request,
                    provider_profile,
                    party_snapshot: PartySnapshot {
                        party_id,
                        display_name: party.display_name,
                        resource_version,
                        observed_at_unix_ms,
                    },
                },
            )))
        })
    }
}

const fn request_status_requires_provider(status: EnrichmentRequestStatus) -> bool {
    matches!(
        status,
        EnrichmentRequestStatus::Created
            | EnrichmentRequestStatus::Queued
            | EnrichmentRequestStatus::Dispatched
            | EnrichmentRequestStatus::FailedRetryable
    )
}

fn module_id() -> Result<ModuleId, SdkError> {
    ModuleId::try_new(MODULE_ID).map_err(source_identifier_invalid)
}

fn record_type(value: &str) -> Result<RecordType, SdkError> {
    RecordType::try_new(value).map_err(source_identifier_invalid)
}

fn source_identifier_invalid(error: crm_module_sdk::IdentifierError) -> SdkError {
    source_configuration_invalid(error.to_string())
}

fn source_configuration_invalid(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_PROVIDER_SOURCE_CONFIGURATION_INVALID",
        ErrorCategory::Internal,
        false,
        "The Customer Enrichment provider source is not configured safely.",
    )
    .with_internal_reference(reference.into())
}

fn source_snapshot_invalid(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_PROVIDER_SOURCE_SNAPSHOT_INVALID",
        ErrorCategory::Internal,
        false,
        "Stored provider source evidence is invalid.",
    )
    .with_internal_reference(reference.into())
}

fn request_unavailable() -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_PROVIDER_REQUEST_UNAVAILABLE",
        ErrorCategory::Unavailable,
        true,
        "The enrichment request is temporarily unavailable to the provider worker.",
    )
}

fn provider_profile_unavailable() -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_PROVIDER_PROFILE_UNAVAILABLE",
        ErrorCategory::Unavailable,
        true,
        "The exact provider profile is temporarily unavailable to the provider worker.",
    )
}

fn party_unavailable() -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_PROVIDER_PARTY_UNAVAILABLE",
        ErrorCategory::NotFound,
        false,
        "The target Party is unavailable to the provider worker.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_pre_response_states_require_provider_execution() {
        for status in [
            EnrichmentRequestStatus::Created,
            EnrichmentRequestStatus::Queued,
            EnrichmentRequestStatus::Dispatched,
            EnrichmentRequestStatus::FailedRetryable,
        ] {
            assert!(request_status_requires_provider(status));
        }
        for status in [
            EnrichmentRequestStatus::ResponseRecorded,
            EnrichmentRequestStatus::SuggestionsMaterialized,
            EnrichmentRequestStatus::Completed,
            EnrichmentRequestStatus::FailedTerminal,
            EnrichmentRequestStatus::Cancelled,
            EnrichmentRequestStatus::Expired,
        ] {
            assert!(!request_status_requires_provider(status));
        }
    }

    #[test]
    fn source_is_thread_safe() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<GovernedCustomerEnrichmentProviderSource>();
    }
}
