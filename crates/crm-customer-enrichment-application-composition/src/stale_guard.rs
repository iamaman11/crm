use crm_capability_runtime::CapabilityRequest;
use crm_core_data::TransactionalAggregateGuard;
use crm_customer_enrichment::{
    SUGGESTION_RECORD_TYPE, Suggestion, decode_suggestion_state, derive_suggestion_supersession,
};
use crm_customer_enrichment_capability_adapter::MODULE_ID;
use crm_module_sdk::{ErrorCategory, PortFuture, SdkError};
use sqlx::{Postgres, Row, Transaction};

#[derive(Debug, Clone)]
pub(crate) struct PostgresSuggestionMutationGuard {
    suggestion: Suggestion,
}

impl PostgresSuggestionMutationGuard {
    pub(crate) fn new(suggestion: Suggestion) -> Self {
        Self { suggestion }
    }
}

impl TransactionalAggregateGuard for PostgresSuggestionMutationGuard {
    fn check<'a>(
        &'a self,
        transaction: &'a mut Transaction<'_, Postgres>,
        request: &'a CapabilityRequest,
    ) -> PortFuture<'a, Result<(), SdkError>> {
        Box::pin(async move {
            sqlx::query("SELECT pg_advisory_xact_lock($1)")
                .bind(self.suggestion.mutation_lock_key())
                .execute(&mut **transaction)
                .await
                .map_err(store_unavailable)?;

            let target_field = match self.suggestion.target().target_field {
                crm_customer_enrichment::TargetField::PartyDisplayName => "party_display_name",
            };
            let rows = sqlx::query(
                r#"
                SELECT record_id, payload_bytes
                FROM crm.records
                WHERE tenant_id = $1
                  AND owner_module_id = $2
                  AND record_type = $3
                  AND deleted_at IS NULL
                  AND convert_from(payload_bytes, 'UTF8')::jsonb ->> 'provider_profile_version_id' = $4
                  AND convert_from(payload_bytes, 'UTF8')::jsonb ->> 'mapping_version_id' = $5
                  AND convert_from(payload_bytes, 'UTF8')::jsonb -> 'target' ->> 'resource_id' = $6
                  AND convert_from(payload_bytes, 'UTF8')::jsonb -> 'target' ->> 'target_field' = $7
                ORDER BY record_id ASC
                FOR SHARE
                "#,
            )
            .bind(request.context.execution.tenant_id.as_str())
            .bind(MODULE_ID)
            .bind(SUGGESTION_RECORD_TYPE)
            .bind(self.suggestion.provider_profile_version_id().as_str())
            .bind(self.suggestion.mapping_version_id().as_str())
            .bind(&self.suggestion.target().resource_id)
            .bind(target_field)
            .fetch_all(&mut **transaction)
            .await
            .map_err(store_unavailable)?;

            let mut suggestions = Vec::with_capacity(rows.len());
            for row in rows {
                let record_id: String = row.try_get("record_id").map_err(store_unavailable)?;
                let payload: Vec<u8> = row.try_get("payload_bytes").map_err(store_unavailable)?;
                let suggestion = decode_suggestion_state(&payload).map_err(state_invalid)?;
                if record_id != suggestion.suggestion_id().as_str() {
                    return Err(state_invalid(
                        "suggestion record identity differs from canonical payload identity",
                    ));
                }
                suggestions.push(suggestion);
            }
            if !suggestions
                .iter()
                .any(|candidate| candidate.suggestion_id() == self.suggestion.suggestion_id())
            {
                return Err(state_invalid(
                    "authoritative suggestion disappeared inside the mutation transaction",
                ));
            }
            let supersession = derive_suggestion_supersession(suggestions.iter());
            if let Some(successor) = supersession.get(self.suggestion.suggestion_id()) {
                return Err(SdkError::new(
                    "CUSTOMER_ENRICHMENT_SUGGESTION_SUPERSEDED",
                    ErrorCategory::Conflict,
                    false,
                    "A superseded suggestion cannot be newly reviewed or applied.",
                )
                .with_internal_reference(format!(
                    "suggestion={};successor={}",
                    self.suggestion.suggestion_id().as_str(),
                    successor.as_str()
                )));
            }
            Ok(())
        })
    }
}

fn store_unavailable(reference: impl ToString) -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_STALE_GUARD_STORE_UNAVAILABLE",
        ErrorCategory::Unavailable,
        true,
        "Suggestion freshness could not be verified atomically.",
    )
    .with_internal_reference(reference.to_string())
}

fn state_invalid(reference: impl ToString) -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_STALE_GUARD_STATE_INVALID",
        ErrorCategory::Internal,
        false,
        "Stored suggestion freshness evidence is invalid.",
    )
    .with_internal_reference(reference.to_string())
}
