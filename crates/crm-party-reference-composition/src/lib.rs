#![forbid(unsafe_code)]

//! Shared application-side Party reference composition.
//!
//! Owner modules consume stable Party references but never read Party storage. The
//! ordinary reader conceals missing and cross-tenant references behind one bounded
//! result before final live authorization. Transactional owner compositions use the
//! same Party identity boundary with `FOR SHARE` under the already-bound tenant RLS
//! context, and may acquire the shared tenant + canonical Party subject lock before
//! committing protected state.

use crm_core_data::{PostgresDataStore, RecordGetQuery};
use crm_module_sdk::{
    ErrorCategory, ModuleId, PortFuture, RecordId, RecordType, SdkError, TenantId,
};
use crm_parties_capability_adapter::{MODULE_ID as PARTIES_MODULE_ID, RECORD_TYPE};
use sqlx::{Postgres, Row, Transaction};

pub trait PartyReferenceReader: Send + Sync {
    fn exists<'a>(
        &'a self,
        tenant_id: &'a TenantId,
        party_id: &'a str,
    ) -> PortFuture<'a, Result<bool, SdkError>>;
}

#[derive(Debug, Clone)]
pub struct PostgresPartyReferenceReader {
    store: PostgresDataStore,
}

impl PostgresPartyReferenceReader {
    pub fn new(store: PostgresDataStore) -> Self {
        Self { store }
    }
}

impl PartyReferenceReader for PostgresPartyReferenceReader {
    fn exists<'a>(
        &'a self,
        tenant_id: &'a TenantId,
        party_id: &'a str,
    ) -> PortFuture<'a, Result<bool, SdkError>> {
        Box::pin(async move {
            let owner_module_id =
                ModuleId::try_new(PARTIES_MODULE_ID).map_err(configuration_error)?;
            let record_type = RecordType::try_new(RECORD_TYPE).map_err(configuration_error)?;
            let record_id = RecordId::try_new(party_id).map_err(configuration_error)?;
            Ok(self
                .store
                .get_record_for_query(&RecordGetQuery {
                    tenant_id: tenant_id.clone(),
                    owner_module_id,
                    record_type,
                    record_id,
                })
                .await?
                .is_some())
        })
    }
}

/// Locks and proves one authoritative Party row inside the caller's PostgreSQL
/// business transaction. Tenant RLS has already been bound by the shared aggregate
/// executor; therefore a missing and a cross-tenant Party are deliberately
/// indistinguishable. The returned version belongs to the exact locked snapshot.
pub async fn require_party_reference_in_transaction(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: &TenantId,
    party_id: &RecordId,
) -> Result<i64, SdkError> {
    let row = sqlx::query(
        r#"
        SELECT record_id, version
        FROM crm.records
        WHERE tenant_id = $1
          AND owner_module_id = $2
          AND record_type = $3
          AND record_id = $4
          AND deleted_at IS NULL
        FOR SHARE
        "#,
    )
    .bind(tenant_id.as_str())
    .bind(PARTIES_MODULE_ID)
    .bind(RECORD_TYPE)
    .bind(party_id.as_str())
    .fetch_optional(&mut **transaction)
    .await
    .map_err(party_reference_store_unavailable)?
    .ok_or_else(party_reference_unavailable)?;

    let stored_id: String = row
        .try_get("record_id")
        .map_err(party_reference_state_invalid)?;
    let version: i64 = row
        .try_get("version")
        .map_err(party_reference_state_invalid)?;
    if stored_id != party_id.as_str() || version <= 0 {
        return Err(party_reference_state_invalid(
            "locked Party identity or version is invalid",
        ));
    }
    Ok(version)
}

/// Acquires the platform-wide final subject lock for one tenant and authoritative
/// canonical Party. The SQL function is the single lock-key implementation reused by
/// Customer Privacy and every protected owner boundary; capability-specific crates do
/// not reproduce or reinterpret its hashing scheme.
pub async fn lock_customer_subject_in_transaction(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: &TenantId,
    canonical_party_id: &RecordId,
) -> Result<(), SdkError> {
    sqlx::query("SELECT crm.lock_customer_subject($1, $2)")
        .bind(tenant_id.as_str())
        .bind(canonical_party_id.as_str())
        .execute(&mut **transaction)
        .await
        .map_err(customer_subject_lock_unavailable)?;
    Ok(())
}

fn party_reference_unavailable() -> SdkError {
    SdkError::new(
        "PARTY_REFERENCE_UNAVAILABLE",
        ErrorCategory::NotFound,
        false,
        "The referenced Party is unavailable.",
    )
}

fn party_reference_store_unavailable(reference: impl std::fmt::Display) -> SdkError {
    SdkError::new(
        "PARTY_REFERENCE_STORE_UNAVAILABLE",
        ErrorCategory::Unavailable,
        true,
        "The referenced Party could not be verified atomically.",
    )
    .with_internal_reference(reference.to_string())
}

fn party_reference_state_invalid(reference: impl std::fmt::Display) -> SdkError {
    SdkError::new(
        "PARTY_REFERENCE_STATE_INVALID",
        ErrorCategory::Internal,
        false,
        "The referenced Party state is invalid.",
    )
    .with_internal_reference(reference.to_string())
}

fn customer_subject_lock_unavailable(reference: impl std::fmt::Display) -> SdkError {
    SdkError::new(
        "CUSTOMER_SUBJECT_LOCK_UNAVAILABLE",
        ErrorCategory::Unavailable,
        true,
        "The customer subject could not be locked safely.",
    )
    .with_internal_reference(reference.to_string())
}

fn configuration_error(error: crm_module_sdk::IdentifierError) -> SdkError {
    SdkError::new(
        "PARTY_REFERENCE_CONFIGURATION_INVALID",
        ErrorCategory::Internal,
        false,
        "The Party reference validation boundary is not configured safely.",
    )
    .with_internal_reference(error.to_string())
}

pub const CRATE_NAME: &str = "crm-party-reference-composition";
