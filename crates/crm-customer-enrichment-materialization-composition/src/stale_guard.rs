use crm_capability_runtime::CapabilityRequest;
use crm_core_data::TransactionalAggregateGuard;
use crm_customer_enrichment::{
    APPLICATION_ATTEMPT_RECORD_TYPE, MappingVersion, ProviderProfileVersion, TargetField,
    TargetSnapshot, suggestion_mutation_lock_key,
};
use crm_customer_enrichment_capability_adapter::MODULE_ID;
use crm_module_sdk::{ErrorCategory, PortFuture, SdkError};
use crm_proto_contracts::crm::customer_enrichment::v1 as wire;
use sqlx::{Postgres, Transaction};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct MutationCoordinate {
    lock_key: i64,
    provider_profile_version_id: String,
    mapping_version_id: String,
    resource_id: String,
    target_field: &'static str,
}

#[derive(Debug, Clone)]
pub(crate) struct PostgresSuggestionMaterializationGuard {
    coordinates: Vec<MutationCoordinate>,
}

impl PostgresSuggestionMaterializationGuard {
    pub(crate) fn new(
        profile: &ProviderProfileVersion,
        mapping: &MappingVersion,
        command: &wire::MaterializeSuggestionsRequest,
    ) -> Result<Self, SdkError> {
        let mut coordinates = command
            .candidates
            .iter()
            .map(|candidate| {
                let target = candidate.target.as_ref().ok_or_else(|| {
                    SdkError::invalid_argument(
                        "customer_enrichment.candidates.target",
                        "Suggestion target snapshot is required",
                    )
                })?;
                let party = target.party_ref.as_ref().ok_or_else(|| {
                    SdkError::invalid_argument(
                        "customer_enrichment.candidates.target.party_ref",
                        "Party reference is required",
                    )
                })?;
                let target_field = match wire::EnrichmentTargetField::try_from(target.target_field)
                {
                    Ok(wire::EnrichmentTargetField::PartyDisplayName) => {
                        TargetField::PartyDisplayName
                    }
                    Ok(wire::EnrichmentTargetField::Unspecified) | Err(_) => {
                        return Err(SdkError::invalid_argument(
                            "customer_enrichment.candidates.target.target_field",
                            "Suggestion target field must be specified",
                        ));
                    }
                };
                let resource_version =
                    u64::try_from(target.party_resource_version).map_err(|_| {
                        SdkError::invalid_argument(
                            "customer_enrichment.candidates.target.party_resource_version",
                            "Party resource version must not be negative",
                        )
                    })?;
                let target_snapshot = TargetSnapshot::try_new(
                    party.party_id.clone(),
                    resource_version,
                    target_field,
                )?;
                Ok(MutationCoordinate {
                    lock_key: suggestion_mutation_lock_key(
                        profile.version_id(),
                        mapping.version_id(),
                        &target_snapshot,
                    ),
                    provider_profile_version_id: profile.version_id().as_str().to_owned(),
                    mapping_version_id: mapping.version_id().as_str().to_owned(),
                    resource_id: target_snapshot.resource_id,
                    target_field: match target_field {
                        TargetField::PartyDisplayName => "party_display_name",
                    },
                })
            })
            .collect::<Result<Vec<_>, SdkError>>()?;
        coordinates.sort();
        coordinates.dedup();
        Ok(Self { coordinates })
    }
}

impl TransactionalAggregateGuard for PostgresSuggestionMaterializationGuard {
    fn check<'a>(
        &'a self,
        transaction: &'a mut Transaction<'_, Postgres>,
        request: &'a CapabilityRequest,
    ) -> PortFuture<'a, Result<(), SdkError>> {
        Box::pin(async move {
            for coordinate in &self.coordinates {
                sqlx::query("SELECT pg_advisory_xact_lock($1)")
                    .bind(coordinate.lock_key)
                    .execute(&mut **transaction)
                    .await
                    .map_err(store_unavailable)?;
            }
            for coordinate in &self.coordinates {
                let pending: bool = sqlx::query_scalar(
                    r#"
                    SELECT EXISTS (
                      SELECT 1
                      FROM crm.records AS suggestion
                      JOIN crm.records AS attempt
                        ON attempt.tenant_id = suggestion.tenant_id
                       AND attempt.owner_module_id = $2
                       AND attempt.record_type = $7
                       AND attempt.deleted_at IS NULL
                       AND convert_from(attempt.payload_bytes, 'UTF8')::jsonb ->> 'suggestion_id' = suggestion.record_id
                      WHERE suggestion.tenant_id = $1
                        AND suggestion.owner_module_id = $2
                        AND suggestion.record_type = 'customer_enrichment.suggestion'
                        AND suggestion.deleted_at IS NULL
                        AND convert_from(suggestion.payload_bytes, 'UTF8')::jsonb ->> 'provider_profile_version_id' = $3
                        AND convert_from(suggestion.payload_bytes, 'UTF8')::jsonb ->> 'mapping_version_id' = $4
                        AND convert_from(suggestion.payload_bytes, 'UTF8')::jsonb -> 'target' ->> 'resource_id' = $5
                        AND convert_from(suggestion.payload_bytes, 'UTF8')::jsonb -> 'target' ->> 'target_field' = $6
                        AND convert_from(attempt.payload_bytes, 'UTF8')::jsonb -> 'recorded_outcome' = 'null'::jsonb
                    )
                    "#,
                )
                .bind(request.context.execution.tenant_id.as_str())
                .bind(MODULE_ID)
                .bind(&coordinate.provider_profile_version_id)
                .bind(&coordinate.mapping_version_id)
                .bind(&coordinate.resource_id)
                .bind(coordinate.target_field)
                .bind(APPLICATION_ATTEMPT_RECORD_TYPE)
                .fetch_one(&mut **transaction)
                .await
                .map_err(store_unavailable)?;
                if pending {
                    return Err(SdkError::new(
                        "CUSTOMER_ENRICHMENT_APPLICATION_IN_PROGRESS",
                        ErrorCategory::Conflict,
                        true,
                        "Suggestion materialization is deferred while an exact application attempt is pending.",
                    ));
                }
            }
            Ok(())
        })
    }
}

fn store_unavailable(reference: impl ToString) -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_MATERIALIZATION_GUARD_STORE_UNAVAILABLE",
        ErrorCategory::Unavailable,
        true,
        "Suggestion materialization guards could not be evaluated.",
    )
    .with_internal_reference(reference.to_string())
}
