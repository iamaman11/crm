#![forbid(unsafe_code)]

//! Transaction-scoped proof for the authoritative Identity Resolution canonical
//! topology. This owner-side composition reuses the accepted merge-lineage state,
//! canonical redirect relationship and Party reference boundaries; it does not own a
//! second topology, mutate Party values or expose a capability-specific transport.

use crm_identity_resolution::{
    MergeOperation, MergeOperationId, MergeOperationStatus, PartyReference,
};
use crm_identity_resolution_capability_adapter::{
    CANONICAL_REDIRECT_PARTY_RECORD_TYPE, CANONICAL_REDIRECT_RELATIONSHIP_TYPE,
    MERGE_OPERATION_RECORD_TYPE, MODULE_ID, PARTY_MERGE_RELATIONSHIP_TYPE,
    merge_operation_from_snapshot,
};
use crm_module_sdk::{
    DataClass, ErrorCategory, ModuleId, PayloadEncoding, RecordId, RecordRef, RecordSnapshot,
    RecordType, RetentionPolicyId, SchemaId, SchemaVersion, SdkError, TenantId, TypedPayload,
};
use crm_parties_capability_adapter::RECORD_TYPE as PARTY_RECORD_TYPE;
use crm_party_reference_composition::require_party_reference_in_transaction;
use sqlx::{Postgres, Row, Transaction, postgres::PgRow};
use std::collections::BTreeSet;

const MAXIMUM_CANONICAL_REDIRECT_HOPS: usize = 64;
const MAXIMUM_MERGE_OPERATIONS_PER_PARTY: i64 = 1_000;

/// Exact authoritative proof captured while the tenant Identity Resolution topology
/// lock is held inside the caller's business transaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalPartyTopologyProof {
    pub requested_party: PartyReference,
    pub canonical_party: PartyReference,
    pub generation: u64,
    pub party_path: Vec<PartyReference>,
    pub merge_operation_path: Vec<MergeOperationId>,
}

/// Proves that `requested_party` currently resolves to `claimed_canonical_party` at
/// exactly `claimed_generation`. The proof is race-safe because it acquires the same
/// tenant topology advisory lock used by merge/unmerge before reading generation,
/// Party records, redirect edges and active merge-operation lineage. All reads occur
/// through the caller's already-bound PostgreSQL transaction and FORCE RLS context.
pub async fn prove_canonical_party_in_transaction(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: &TenantId,
    requested_party: &PartyReference,
    claimed_canonical_party: &PartyReference,
    claimed_generation: u64,
) -> Result<CanonicalPartyTopologyProof, SdkError> {
    if claimed_generation == 0 {
        return Err(SdkError::invalid_argument(
            "identity_resolution.generation",
            "Identity Resolution generation must be positive.",
        ));
    }

    acquire_topology_lock(transaction, tenant_id).await?;
    let actual_generation = current_generation(transaction, tenant_id).await?;
    if actual_generation != claimed_generation {
        return Err(stale_generation());
    }

    require_party(transaction, tenant_id, requested_party).await?;
    require_party(transaction, tenant_id, claimed_canonical_party).await?;

    let mut current = requested_party.clone();
    let mut party_path = vec![current.clone()];
    let mut merge_operation_path = Vec::new();
    let mut visited = BTreeSet::from([current.clone()]);

    for _ in 0..MAXIMUM_CANONICAL_REDIRECT_HOPS {
        let Some(next) = immediate_redirect_target(transaction, tenant_id, &current).await? else {
            if current != *claimed_canonical_party {
                return Err(canonical_party_mismatch());
            }
            return Ok(CanonicalPartyTopologyProof {
                requested_party: requested_party.clone(),
                canonical_party: current,
                generation: actual_generation,
                party_path,
                merge_operation_path,
            });
        };

        if !visited.insert(next.clone()) {
            return Err(canonical_redirect_invalid(
                "canonical redirect topology contains a cycle",
            ));
        }
        require_party(transaction, tenant_id, &next).await?;
        let operation = active_operation_for_edge(transaction, tenant_id, &current, &next).await?;
        merge_operation_path.push(operation.operation_id().clone());
        current = next;
        party_path.push(current.clone());
    }

    Err(canonical_redirect_invalid(
        "canonical redirect topology exceeds the supported hop bound",
    ))
}

async fn acquire_topology_lock(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: &TenantId,
) -> Result<(), SdkError> {
    sqlx::query("SELECT crm.lock_identity_resolution_topology($1)")
        .bind(tenant_id.as_str())
        .execute(&mut **transaction)
        .await
        .map_err(topology_store_unavailable)?;
    Ok(())
}

async fn current_generation(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: &TenantId,
) -> Result<u64, SdkError> {
    let generation: i64 =
        sqlx::query_scalar("SELECT crm.current_identity_resolution_generation($1)")
            .bind(tenant_id.as_str())
            .fetch_one(&mut **transaction)
            .await
            .map_err(topology_store_unavailable)?;
    u64::try_from(generation).map_err(|_| {
        canonical_redirect_invalid("authoritative topology generation is not positive")
    })
}

async fn require_party(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: &TenantId,
    party: &PartyReference,
) -> Result<(), SdkError> {
    let party_id = RecordId::try_new(party.as_str()).map_err(configuration_error)?;
    require_party_reference_in_transaction(transaction, tenant_id, &party_id).await?;
    Ok(())
}

async fn immediate_redirect_target(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: &TenantId,
    source: &PartyReference,
) -> Result<Option<PartyReference>, SdkError> {
    let rows = sqlx::query(
        r#"
        SELECT target_record_id
        FROM crm.relationships
        WHERE tenant_id = $1
          AND owner_module_id = $2
          AND relationship_type = $3
          AND source_record_type = $4
          AND source_record_id = $5
          AND target_record_type = $4
        ORDER BY target_record_id ASC
        LIMIT 2
        FOR SHARE
        "#,
    )
    .bind(tenant_id.as_str())
    .bind(MODULE_ID)
    .bind(CANONICAL_REDIRECT_RELATIONSHIP_TYPE)
    .bind(CANONICAL_REDIRECT_PARTY_RECORD_TYPE)
    .bind(source.as_str())
    .fetch_all(&mut **transaction)
    .await
    .map_err(topology_store_unavailable)?;

    if rows.len() > 1 {
        return Err(canonical_redirect_invalid(
            "more than one active canonical redirect exists for one source Party",
        ));
    }
    rows.first()
        .map(|row| {
            row.try_get::<String, _>("target_record_id")
                .map_err(topology_store_unavailable)
                .and_then(PartyReference::try_new)
        })
        .transpose()
}

async fn active_operation_for_edge(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: &TenantId,
    source: &PartyReference,
    target: &PartyReference,
) -> Result<MergeOperation, SdkError> {
    let rows = sqlx::query(
        r#"
        SELECT
          operation.record_id,
          operation.version,
          operation.owner_module_id,
          operation.schema_id,
          operation.schema_version,
          operation.descriptor_hash,
          operation.data_class,
          operation.payload_encoding,
          operation.maximum_payload_size,
          operation.retention_policy_id,
          operation.payload_bytes
        FROM crm.relationships AS relation
        JOIN crm.records AS operation
          ON operation.tenant_id = relation.tenant_id
         AND operation.record_type = relation.target_record_type
         AND operation.record_id = relation.target_record_id
        WHERE relation.tenant_id = $1
          AND relation.owner_module_id = $2
          AND relation.relationship_type = $3
          AND relation.source_record_type = $4
          AND relation.source_record_id = $5
          AND relation.target_record_type = $6
          AND operation.owner_module_id = $2
          AND operation.record_type = $6
          AND operation.deleted_at IS NULL
        ORDER BY operation.record_id ASC
        LIMIT $7
        FOR SHARE OF relation, operation
        "#,
    )
    .bind(tenant_id.as_str())
    .bind(MODULE_ID)
    .bind(PARTY_MERGE_RELATIONSHIP_TYPE)
    .bind(PARTY_RECORD_TYPE)
    .bind(source.as_str())
    .bind(MERGE_OPERATION_RECORD_TYPE)
    .bind(MAXIMUM_MERGE_OPERATIONS_PER_PARTY + 1)
    .fetch_all(&mut **transaction)
    .await
    .map_err(topology_store_unavailable)?;

    if i64::try_from(rows.len()).unwrap_or(i64::MAX) > MAXIMUM_MERGE_OPERATIONS_PER_PARTY {
        return Err(canonical_redirect_invalid(
            "merge-operation lineage exceeds the supported per-Party bound",
        ));
    }

    let mut matching = None;
    for row in rows {
        let operation = merge_operation_from_snapshot(&decode_operation_snapshot(row)?)
            .map_err(merge_operation_state_invalid)?;
        if operation.status() != MergeOperationStatus::Active
            || operation.source_party_ref() != source
            || operation.survivor_party_ref() != target
        {
            continue;
        }
        if matching.is_some() {
            return Err(canonical_redirect_invalid(
                "more than one active merge operation matches one canonical redirect edge",
            ));
        }
        matching = Some(operation);
    }
    matching.ok_or_else(|| {
        canonical_redirect_invalid(
            "canonical redirect edge has no matching active merge operation",
        )
    })
}

fn decode_operation_snapshot(row: PgRow) -> Result<RecordSnapshot, SdkError> {
    let record_id: String = row.try_get("record_id").map_err(operation_row_invalid)?;
    let version: i64 = row.try_get("version").map_err(operation_row_invalid)?;
    let owner_module_id: String = row
        .try_get("owner_module_id")
        .map_err(operation_row_invalid)?;
    let schema_id: String = row.try_get("schema_id").map_err(operation_row_invalid)?;
    let schema_version: String = row
        .try_get("schema_version")
        .map_err(operation_row_invalid)?;
    let descriptor_hash: Vec<u8> = row
        .try_get("descriptor_hash")
        .map_err(operation_row_invalid)?;
    let data_class: String = row.try_get("data_class").map_err(operation_row_invalid)?;
    let payload_encoding: String = row
        .try_get("payload_encoding")
        .map_err(operation_row_invalid)?;
    let maximum_payload_size: i64 = row
        .try_get("maximum_payload_size")
        .map_err(operation_row_invalid)?;
    let retention_policy_id: String = row
        .try_get("retention_policy_id")
        .map_err(operation_row_invalid)?;
    let payload_bytes: Vec<u8> = row
        .try_get("payload_bytes")
        .map_err(operation_row_invalid)?;

    if version <= 0 || data_class != "personal" || payload_encoding != "json" {
        return Err(operation_row_invalid(
            "merge-operation row version, data class or encoding is invalid",
        ));
    }
    let descriptor_hash: [u8; 32] = descriptor_hash.try_into().map_err(|_| {
        operation_row_invalid("merge-operation descriptor hash must contain exactly 32 bytes")
    })?;
    let maximum_size_bytes = u64::try_from(maximum_payload_size)
        .map_err(|_| operation_row_invalid("merge-operation maximum payload size is negative"))?;

    Ok(RecordSnapshot {
        reference: RecordRef {
            record_type: RecordType::try_new(MERGE_OPERATION_RECORD_TYPE)
                .map_err(configuration_error)?,
            record_id: RecordId::try_new(record_id).map_err(operation_row_invalid)?,
        },
        version,
        payload: TypedPayload {
            owner: ModuleId::try_new(owner_module_id).map_err(operation_row_invalid)?,
            schema_id: SchemaId::try_new(schema_id).map_err(operation_row_invalid)?,
            schema_version: SchemaVersion::try_new(schema_version)
                .map_err(operation_row_invalid)?,
            descriptor_hash,
            data_class: DataClass::Personal,
            encoding: PayloadEncoding::Json,
            maximum_size_bytes,
            retention_policy_id: RetentionPolicyId::try_new(retention_policy_id)
                .map_err(operation_row_invalid)?,
            bytes: payload_bytes,
        },
    })
}

fn stale_generation() -> SdkError {
    SdkError::new(
        "IDENTITY_RESOLUTION_TOPOLOGY_GENERATION_STALE",
        ErrorCategory::Conflict,
        true,
        "The Identity Resolution topology changed before the subject proof was committed.",
    )
}

fn canonical_party_mismatch() -> SdkError {
    SdkError::new(
        "IDENTITY_RESOLUTION_CANONICAL_PARTY_MISMATCH",
        ErrorCategory::InvalidArgument,
        false,
        "The submitted Party does not resolve to the specified canonical Party.",
    )
}

fn canonical_redirect_invalid(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "IDENTITY_RESOLUTION_CANONICAL_REDIRECT_INVALID",
        ErrorCategory::Internal,
        false,
        "The canonical Party redirect topology is temporarily unavailable.",
    )
    .with_internal_reference(reference.into())
}

fn topology_store_unavailable(reference: impl std::fmt::Display) -> SdkError {
    SdkError::new(
        "IDENTITY_RESOLUTION_TOPOLOGY_STORE_UNAVAILABLE",
        ErrorCategory::Unavailable,
        true,
        "The canonical Party topology could not be verified atomically.",
    )
    .with_internal_reference(reference.to_string())
}

fn merge_operation_state_invalid(error: SdkError) -> SdkError {
    canonical_redirect_invalid(format!(
        "active merge-operation state failed strict rehydration: {}",
        error.code
    ))
}

fn operation_row_invalid(reference: impl std::fmt::Display) -> SdkError {
    canonical_redirect_invalid(format!("merge-operation row is invalid: {reference}"))
}

fn configuration_error(error: crm_module_sdk::IdentifierError) -> SdkError {
    SdkError::new(
        "IDENTITY_RESOLUTION_TOPOLOGY_CONFIGURATION_INVALID",
        ErrorCategory::Internal,
        false,
        "The Identity Resolution topology proof is not configured safely.",
    )
    .with_internal_reference(error.to_string())
}
