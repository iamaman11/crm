use crate::{CanonicalPartyTopologyProof, prove_canonical_party_in_transaction};
use crm_identity_resolution::PartyReference;
use crm_module_sdk::{ErrorCategory, RecordId, SdkError, TenantId};
use sqlx::{Postgres, Transaction};

/// Proves that one Party is the current canonical Party for itself inside the
/// caller-owned transaction.
///
/// The generation is read before the canonical proof acquires the authoritative
/// topology lock. A concurrent topology change therefore cannot be accepted:
/// the proof reloads the generation after locking and fails closed when it no
/// longer matches the observed value.
pub async fn require_current_canonical_party_in_transaction(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: &TenantId,
    canonical_party_id: &RecordId,
) -> Result<CanonicalPartyTopologyProof, SdkError> {
    let observed_generation: i64 =
        sqlx::query_scalar("SELECT crm.current_identity_resolution_generation($1)")
            .bind(tenant_id.as_str())
            .fetch_one(&mut **transaction)
            .await
            .map_err(topology_generation_unavailable)?;
    let observed_generation = u64::try_from(observed_generation).map_err(|_| {
        topology_generation_invalid("authoritative topology generation is negative")
    })?;
    if observed_generation == 0 {
        return Err(topology_generation_invalid(
            "authoritative topology generation is zero",
        ));
    }

    let canonical_party =
        PartyReference::try_new(canonical_party_id.as_str()).map_err(|error| {
            topology_generation_invalid(format!(
                "canonical Party identity is invalid: {}",
                error.code
            ))
        })?;
    prove_canonical_party_in_transaction(
        transaction,
        tenant_id,
        &canonical_party,
        &canonical_party,
        observed_generation,
    )
    .await
}

fn topology_generation_unavailable(error: impl std::fmt::Display) -> SdkError {
    SdkError::new(
        "IDENTITY_RESOLUTION_TOPOLOGY_GENERATION_UNAVAILABLE",
        ErrorCategory::Unavailable,
        true,
        "The current canonical Party topology could not be verified.",
    )
    .with_internal_reference(error.to_string())
}

fn topology_generation_invalid(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "IDENTITY_RESOLUTION_TOPOLOGY_GENERATION_INVALID",
        ErrorCategory::Internal,
        false,
        "The current canonical Party topology is invalid.",
    )
    .with_internal_reference(reference)
}
