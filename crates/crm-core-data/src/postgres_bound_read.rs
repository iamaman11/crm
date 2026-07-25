use crate::postgres::PostgresDataStore;
use crm_module_sdk::{ErrorCategory, SdkError, TenantId};
use sqlx::{Postgres, Transaction};
use std::ops::{Deref, DerefMut};

/// Tenant-bound PostgreSQL transaction with one repeatable READ ONLY snapshot.
///
/// Construction is restricted by architecture policy to approved infrastructure
/// adapters. Dereferencing exposes the existing SQLx transaction surface only
/// after the database has enforced repeatable-read, read-only mode and tenant-local
/// RLS context.
pub struct BoundReadTransaction<'a> {
    inner: Transaction<'a, Postgres>,
}

impl std::fmt::Debug for BoundReadTransaction<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("BoundReadTransaction")
            .field("isolation", &"REPEATABLE READ")
            .field("mode", &"READ ONLY")
            .finish_non_exhaustive()
    }
}

impl<'a> Deref for BoundReadTransaction<'a> {
    type Target = Transaction<'a, Postgres>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<'a> DerefMut for BoundReadTransaction<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl BoundReadTransaction<'_> {
    pub async fn commit(self) -> Result<(), sqlx::Error> {
        self.inner.commit().await
    }
}

impl PostgresDataStore {
    /// Begins a tenant-bound repeatable read-only transaction for an
    /// architecture-approved infrastructure adapter that must combine
    /// authoritative reads atomically without row-locking statements.
    pub async fn begin_bound_read_transaction(
        &self,
        tenant_id: &TenantId,
    ) -> Result<BoundReadTransaction<'_>, SdkError> {
        let mut transaction = self.pool().begin().await.map_err(database_unavailable)?;
        sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY")
            .execute(&mut *transaction)
            .await
            .map_err(database_unavailable)?;
        sqlx::query("SELECT set_config('app.tenant_id', $1, true)")
            .bind(tenant_id.as_str())
            .execute(&mut *transaction)
            .await
            .map_err(database_unavailable)?;
        Ok(BoundReadTransaction { inner: transaction })
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
