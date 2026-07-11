use crate::PostgresDataStore;
use crm_core_events::{
    EventHistoryPage, EventHistoryRequest, ProjectionApplyResult, ProjectionCheckpoint,
    ProjectionEventApplication, ProjectionFailure, ProjectionStore, ProjectionStoreFuture,
};
use crm_module_sdk::{ErrorCategory, SdkError, TenantId};
use sqlx::Row;

impl ProjectionStore for PostgresDataStore {
    fn projection_checkpoint(
        &self,
        tenant_id: TenantId,
        projection_id: String,
    ) -> ProjectionStoreFuture<'_, Option<ProjectionCheckpoint>> {
        Box::pin(async move {
            self.ensure_projection_active_or_absent(&tenant_id, &projection_id)
                .await?;
            PostgresDataStore::projection_checkpoint(self, &tenant_id, &projection_id).await
        })
    }

    fn list_event_history(
        &self,
        request: EventHistoryRequest,
    ) -> ProjectionStoreFuture<'_, EventHistoryPage> {
        Box::pin(async move { PostgresDataStore::list_event_history(self, &request).await })
    }

    fn apply_projection_event(
        &self,
        application: ProjectionEventApplication,
    ) -> ProjectionStoreFuture<'_, ProjectionApplyResult> {
        Box::pin(async move { PostgresDataStore::apply_projection_event(self, &application).await })
    }

    fn mark_projection_failed(&self, failure: ProjectionFailure) -> ProjectionStoreFuture<'_, ()> {
        Box::pin(async move { self.record_projection_failure(&failure).await })
    }

    fn reset_projection(
        &self,
        tenant_id: TenantId,
        projection_id: String,
    ) -> ProjectionStoreFuture<'_, ()> {
        Box::pin(async move {
            PostgresDataStore::reset_projection(self, &tenant_id, &projection_id).await
        })
    }
}

impl PostgresDataStore {
    async fn ensure_projection_active_or_absent(
        &self,
        tenant_id: &TenantId,
        projection_id: &str,
    ) -> Result<(), SdkError> {
        validate_projection_id(projection_id)?;
        let mut transaction = self
            .pool()
            .begin()
            .await
            .map_err(projection_storage_error)?;
        bind_projection_tenant(&mut transaction, tenant_id).await?;
        sqlx::query("SET TRANSACTION READ ONLY")
            .execute(&mut *transaction)
            .await
            .map_err(projection_storage_error)?;
        let row = sqlx::query(
            r#"
            SELECT status, failure_event_id, failure_code
            FROM crm.projection_checkpoints
            WHERE tenant_id = $1 AND projection_id = $2
            "#,
        )
        .bind(tenant_id.as_str())
        .bind(projection_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(projection_storage_error)?;
        transaction
            .commit()
            .await
            .map_err(projection_storage_error)?;

        let Some(row) = row else {
            return Ok(());
        };
        let status: String = row
            .try_get("status")
            .map_err(|error| projection_stored_value_invalid(error.to_string()))?;
        match status.as_str() {
            "active" => Ok(()),
            "failed" => {
                let failure_event_id: Option<String> = row
                    .try_get("failure_event_id")
                    .map_err(|error| projection_stored_value_invalid(error.to_string()))?;
                let failure_code: Option<String> = row
                    .try_get("failure_code")
                    .map_err(|error| projection_stored_value_invalid(error.to_string()))?;
                Err(SdkError::new(
                    "PROJECTION_CHECKPOINT_FAILED",
                    ErrorCategory::Conflict,
                    false,
                    "The projection checkpoint is failed and must be reset or repaired before processing can continue.",
                )
                .with_internal_reference(format!(
                    "projection_id={projection_id};failure_event_id={};failure_code={}",
                    failure_event_id.as_deref().unwrap_or("missing"),
                    failure_code.as_deref().unwrap_or("missing")
                )))
            }
            other => Err(projection_stored_value_invalid(format!(
                "unsupported projection checkpoint status: {other}"
            ))),
        }
    }

    async fn record_projection_failure(&self, failure: &ProjectionFailure) -> Result<(), SdkError> {
        failure.validate().map_err(projection_request_invalid)?;
        let mut transaction = self
            .pool()
            .begin()
            .await
            .map_err(projection_storage_error)?;
        bind_projection_tenant(&mut transaction, &failure.tenant_id).await?;
        sqlx::query(
            r#"
            INSERT INTO crm.projection_checkpoints (
              tenant_id, projection_id, last_occurred_at, last_event_id,
              applied_event_count, status, failure_event_id, failure_code
            )
            VALUES (
              $1, $2,
              TIMESTAMPTZ 'epoch' + ($3::bigint / 1000) * INTERVAL '1 microsecond',
              $4, 0, 'failed', $4, $5
            )
            ON CONFLICT (tenant_id, projection_id) DO UPDATE SET
              status = 'failed',
              failure_event_id = EXCLUDED.failure_event_id,
              failure_code = EXCLUDED.failure_code,
              updated_at = clock_timestamp()
            "#,
        )
        .bind(failure.tenant_id.as_str())
        .bind(&failure.projection_id)
        .bind(failure.occurred_at_unix_nanos)
        .bind(failure.event_id.as_str())
        .bind(&failure.failure_code)
        .execute(&mut *transaction)
        .await
        .map_err(projection_storage_error)?;
        transaction
            .commit()
            .await
            .map_err(projection_storage_error)?;
        Ok(())
    }
}

async fn bind_projection_tenant(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    tenant_id: &TenantId,
) -> Result<(), SdkError> {
    sqlx::query("SELECT set_config('app.tenant_id', $1, true)")
        .bind(tenant_id.as_str())
        .execute(&mut **transaction)
        .await
        .map_err(projection_storage_error)?;
    Ok(())
}

fn validate_projection_id(projection_id: &str) -> Result<(), SdkError> {
    if projection_id.is_empty() || projection_id.len() > 180 {
        return Err(projection_request_invalid("projection id is invalid"));
    }
    Ok(())
}

fn projection_request_invalid(message: &'static str) -> SdkError {
    SdkError::new(
        "PROJECTION_REQUEST_INVALID",
        ErrorCategory::InvalidArgument,
        false,
        message,
    )
}

fn projection_stored_value_invalid(internal: impl Into<String>) -> SdkError {
    SdkError::new(
        "PROJECTION_STORED_VALUE_INVALID",
        ErrorCategory::Unavailable,
        true,
        "The projection service is temporarily unavailable.",
    )
    .with_internal_reference(internal)
}

fn projection_storage_error(error: sqlx::Error) -> SdkError {
    SdkError::new(
        "PROJECTION_STORAGE_UNAVAILABLE",
        ErrorCategory::Unavailable,
        true,
        "The projection service is temporarily unavailable.",
    )
    .with_internal_reference(error.to_string())
}
