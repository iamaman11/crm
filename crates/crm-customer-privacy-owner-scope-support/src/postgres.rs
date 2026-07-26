use crm_core_data::BoundReadTransaction;
use crm_identity_resolution_capability_adapter::{
    CANONICAL_REDIRECT_PARTY_RECORD_TYPE, CANONICAL_REDIRECT_RELATIONSHIP_TYPE,
    MODULE_ID as IDENTITY_RESOLUTION_MODULE_ID,
};
use crm_module_sdk::{RecordId, TenantId};
use crm_parties::MODULE_ID as PARTIES_MODULE_ID;
use crm_parties_capability_adapter::RECORD_TYPE as PARTY_RECORD_TYPE;

#[derive(Debug)]
pub enum CanonicalPartyClaimError {
    Database(sqlx::Error),
    GenerationNotPositive,
    StaleGeneration,
    PartyNotVisible,
    ActiveRedirect,
}

pub async fn prove_canonical_party_claim(
    transaction: &mut BoundReadTransaction<'_>,
    tenant_id: &TenantId,
    canonical_party_id: &RecordId,
    claimed_generation: u64,
) -> Result<(), CanonicalPartyClaimError> {
    sqlx::query("SELECT crm.lock_identity_resolution_topology($1)")
        .bind(tenant_id.as_str())
        .execute(&mut ***transaction)
        .await
        .map_err(CanonicalPartyClaimError::Database)?;

    let actual_generation: i64 =
        sqlx::query_scalar("SELECT crm.current_identity_resolution_generation($1)")
            .bind(tenant_id.as_str())
            .fetch_one(&mut ***transaction)
            .await
            .map_err(CanonicalPartyClaimError::Database)?;
    let actual_generation = u64::try_from(actual_generation)
        .map_err(|_| CanonicalPartyClaimError::GenerationNotPositive)?;
    if actual_generation != claimed_generation {
        return Err(CanonicalPartyClaimError::StaleGeneration);
    }

    let party_exists: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS (
          SELECT 1
          FROM crm.records
          WHERE tenant_id = $1
            AND owner_module_id = $2
            AND record_type = $3
            AND record_id = $4
            AND deleted_at IS NULL
        )
        "#,
    )
    .bind(tenant_id.as_str())
    .bind(PARTIES_MODULE_ID)
    .bind(PARTY_RECORD_TYPE)
    .bind(canonical_party_id.as_str())
    .fetch_one(&mut ***transaction)
    .await
    .map_err(CanonicalPartyClaimError::Database)?;
    if !party_exists {
        return Err(CanonicalPartyClaimError::PartyNotVisible);
    }

    let outgoing_redirects: i64 = sqlx::query_scalar(
        r#"
        SELECT count(*)::bigint
        FROM crm.relationships
        WHERE tenant_id = $1
          AND owner_module_id = $2
          AND relationship_type = $3
          AND source_record_type = $4
          AND source_record_id = $5
          AND target_record_type = $4
        "#,
    )
    .bind(tenant_id.as_str())
    .bind(IDENTITY_RESOLUTION_MODULE_ID)
    .bind(CANONICAL_REDIRECT_RELATIONSHIP_TYPE)
    .bind(CANONICAL_REDIRECT_PARTY_RECORD_TYPE)
    .bind(canonical_party_id.as_str())
    .fetch_one(&mut ***transaction)
    .await
    .map_err(CanonicalPartyClaimError::Database)?;
    if outgoing_redirects != 0 {
        return Err(CanonicalPartyClaimError::ActiveRedirect);
    }

    Ok(())
}
