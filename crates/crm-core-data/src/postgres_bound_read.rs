use crate::postgres::PostgresDataStore;
use crm_module_sdk::{ErrorCategory, SdkError, TenantId};
use sqlx::{Postgres, Transaction};

impl PostgresDataStore {
    /// Begins a tenant-bound read-only transaction for approved infrastructure
    /// compositions that must combine multiple authoritative reads atomically.
    ///
    /// Business modules must continue to use governed SDK ports. This method is
    /// intentionally limited to infrastructure/composition crates that already
    /// own PostgreSQL transaction semantics.
    pub async fn begin_bound_read_transaction(
        &self,
        tenant_id: &TenantId,
    ) -> Result<Transaction<'_, Postgres>, SdkError> {
        let mut transaction = self.pool().begin().await.map_err(database_unavailable)?;
        sqlx::query("SET TRANSACTION READ ONLY")
            .execute(&mut *transaction)
            .await
            .map_err(database_unavailable)?;
        sqlx::query("SELECT set_config('app.tenant_id', $1, true)")
            .bind(tenant_id.as_str())
            .execute(&mut *transaction)
            .await
            .map_err(database_unavailable)?;
        Ok(transaction)
    }
}

fn database_unavailable(error: sqlx::Error) -> SdkError {
    SdkError::new(
        "DATA_BOUND_READ_UNAVAILABLE",
        ErrorCategory::Unavailable,
        true,
        "The data service is temporarily unavailable.",
    )
    .with_internal_reference(error.to_string())
}
