use crm_core_data::RecordQueryContinuation;
use crm_customer_data_operations_execution_composition::{
    PartyExportSelectionSource, PartyExportSelectionSourceCandidate,
    PartyExportSelectionSourceContinuation, PartyExportSelectionSourceKind,
    PartyExportSelectionSourcePage, PartyExportSelectionSourceRequest,
};
use crm_module_sdk::{ErrorCategory, PortFuture, SdkError};
use crm_parties_query_adapter::{
    LIST_CAPABILITY as PARTY_LIST_CAPABILITY, PartyExportSelectionKind, PartyQueryAdapter,
    export_selection_query_request, query_capability_definition,
};
use crm_query_runtime::QueryAuthorizer;
use std::sync::Arc;

#[derive(Clone)]
pub struct GovernedPartyExportSelectionSource {
    adapter: Arc<PartyQueryAdapter>,
    authorizer: Arc<dyn QueryAuthorizer>,
}

impl std::fmt::Debug for GovernedPartyExportSelectionSource {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("GovernedPartyExportSelectionSource")
            .field("adapter", &"PartyQueryAdapter")
            .field("authorizer", &"dyn QueryAuthorizer")
            .finish()
    }
}

impl GovernedPartyExportSelectionSource {
    pub fn new(adapter: Arc<PartyQueryAdapter>, authorizer: Arc<dyn QueryAuthorizer>) -> Self {
        Self {
            adapter,
            authorizer,
        }
    }
}

impl PartyExportSelectionSource for GovernedPartyExportSelectionSource {
    fn list_page<'a>(
        &'a self,
        source_request: PartyExportSelectionSourceRequest<'a>,
    ) -> PortFuture<'a, Result<PartyExportSelectionSourcePage, SdkError>> {
        Box::pin(async move {
            let party_kind = match source_request.kind {
                None => None,
                Some(PartyExportSelectionSourceKind::Person) => {
                    Some(PartyExportSelectionKind::Person)
                }
                Some(PartyExportSelectionSourceKind::Organization) => {
                    Some(PartyExportSelectionKind::Organization)
                }
            };
            let request = export_selection_query_request(
                source_request.tenant_id,
                source_request.actor_id,
                source_request.job_id,
                party_kind,
                source_request.request_started_at_unix_nanos,
            )?;
            let definition = query_capability_definition(PARTY_LIST_CAPABILITY)?;
            let authorization = self.authorizer.authorize(&definition, &request).await?;
            if !authorization.allowed {
                return Err(SdkError::new(
                    "CUSTOMER_DATA_EXPORT_SELECTION_SOURCE_PERMISSION_DENIED",
                    ErrorCategory::Authorization,
                    false,
                    "The export selection worker is not authorized to read Party candidates.",
                )
                .with_internal_reference(format!(
                    "decision_id={} reason_code={} policy_version={}",
                    authorization.decision_id,
                    authorization.reason_code,
                    authorization.policy_version
                )));
            }

            let after = source_request
                .after
                .map(|continuation| RecordQueryContinuation {
                    sort_value: continuation.sort_value,
                    record_id: continuation.record_id,
                });
            let page = self
                .adapter
                .list_for_export_selection(
                    &request,
                    source_request.selection_cutoff_unix_nanos,
                    party_kind,
                    source_request.page_size,
                    after,
                )
                .await?;
            Ok(PartyExportSelectionSourcePage {
                candidates: page
                    .candidates
                    .into_iter()
                    .map(|candidate| PartyExportSelectionSourceCandidate {
                        party_id: candidate.party_id,
                        resource_version: candidate.resource_version,
                    })
                    .collect(),
                next: page
                    .next
                    .map(|continuation| PartyExportSelectionSourceContinuation {
                        sort_value: continuation.sort_value,
                        record_id: continuation.record_id,
                    }),
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_kind_contract_is_closed_and_exact() {
        let person = PartyExportSelectionSourceKind::Person;
        let organization = PartyExportSelectionSourceKind::Organization;
        assert_ne!(person, organization);
    }
}
