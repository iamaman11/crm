use crm_customer_data_operations_execution_composition::{
    PartyExportExecutionSource, PartyExportExecutionSourceKind, PartyExportExecutionSourceRequest,
    PartyExportExecutionSourceResult,
};
use crm_module_sdk::{ErrorCategory, PortFuture, SdkError};
use crm_parties::PartyKind;
use crm_parties_query_adapter::{
    GET_CAPABILITY as PARTY_GET_CAPABILITY, PartyExportExecutionRead, PartyQueryAdapter,
    export_execution_query_request, query_capability_definition,
};
use crm_query_runtime::QueryAuthorizer;
use std::sync::Arc;

#[derive(Clone)]
pub struct GovernedPartyExportExecutionSource {
    adapter: Arc<PartyQueryAdapter>,
    authorizer: Arc<dyn QueryAuthorizer>,
}

impl std::fmt::Debug for GovernedPartyExportExecutionSource {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("GovernedPartyExportExecutionSource")
            .field("adapter", &"PartyQueryAdapter")
            .field("authorizer", &"dyn QueryAuthorizer")
            .finish()
    }
}

impl GovernedPartyExportExecutionSource {
    pub fn new(adapter: Arc<PartyQueryAdapter>, authorizer: Arc<dyn QueryAuthorizer>) -> Self {
        Self {
            adapter,
            authorizer,
        }
    }
}

impl PartyExportExecutionSource for GovernedPartyExportExecutionSource {
    fn get<'a>(
        &'a self,
        source_request: PartyExportExecutionSourceRequest<'a>,
    ) -> PortFuture<'a, Result<PartyExportExecutionSourceResult, SdkError>> {
        Box::pin(async move {
            let request = export_execution_query_request(
                source_request.tenant_id,
                source_request.actor_id,
                source_request.job_id,
                source_request.party_id,
                source_request.request_started_at_unix_nanos,
            )?;
            let definition = query_capability_definition(PARTY_GET_CAPABILITY)?;
            let authorization = self.authorizer.authorize(&definition, &request).await?;
            if !authorization.allowed {
                return Err(SdkError::new(
                    "CUSTOMER_DATA_EXPORT_EXECUTION_SOURCE_PERMISSION_DENIED",
                    ErrorCategory::Authorization,
                    false,
                    "The export execution worker is not authorized to read the selected Party.",
                )
                .with_internal_reference(format!(
                    "decision_id={} reason_code={} policy_version={}",
                    authorization.decision_id,
                    authorization.reason_code,
                    authorization.policy_version
                )));
            }

            let read = self
                .adapter
                .get_for_export_execution(
                    &request,
                    source_request.party_id,
                    source_request.expected_resource_version,
                )
                .await?;
            Ok(match read {
                PartyExportExecutionRead::NotVisible => {
                    PartyExportExecutionSourceResult::NotVisible
                }
                PartyExportExecutionRead::VersionChanged => {
                    PartyExportExecutionSourceResult::VersionChanged
                }
                PartyExportExecutionRead::Unavailable => {
                    PartyExportExecutionSourceResult::Unavailable
                }
                PartyExportExecutionRead::Visible {
                    party_id,
                    kind,
                    display_name,
                    resource_version,
                    allowed_fields,
                } => PartyExportExecutionSourceResult::Visible {
                    party_id,
                    kind: allowed_fields.contains("kind").then_some(match kind {
                        PartyKind::Person => PartyExportExecutionSourceKind::Person,
                        PartyKind::Organization => PartyExportExecutionSourceKind::Organization,
                    }),
                    display_name: allowed_fields
                        .contains("display_name")
                        .then_some(display_name),
                    resource_version,
                },
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_send_sync<T: Send + Sync>() {}

    #[test]
    fn governed_execution_source_is_thread_safe() {
        assert_send_sync::<GovernedPartyExportExecutionSource>();
    }
}
