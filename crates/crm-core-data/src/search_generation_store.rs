use crate::PostgresDataStore;
use crm_module_sdk::{ErrorCategory, SdkError, TenantId};
use crm_search_runtime::{
    SearchGenerationStatus, SearchGenerationStore, SearchIndexGeneration, SearchIndexId,
};
use sqlx::Row;

impl SearchGenerationStore for PostgresDataStore {
    fn active_generation<'a>(
        &'a self,
        tenant_id: TenantId,
        index_id: SearchIndexId,
    ) -> crm_module_sdk::PortFuture<'a, Result<Option<SearchIndexGeneration>, SdkError>> {
        Box::pin(async move {
            let mut transaction = self.pool().begin().await.map_err(search_storage_error)?;
            bind_search_tenant(&mut transaction, &tenant_id).await?;
            sqlx::query("SET TRANSACTION READ ONLY")
                .execute(&mut *transaction)
                .await
                .map_err(search_storage_error)?;
            let row = sqlx::query(
                r#"
                SELECT generation_id, projection_id, schema_version
                FROM crm.search_index_generations
                WHERE tenant_id = $1 AND index_id = $2 AND status = 'active'
                "#,
            )
            .bind(tenant_id.as_str())
            .bind(index_id.as_str())
            .fetch_optional(&mut *transaction)
            .await
            .map_err(search_storage_error)?;
            transaction.commit().await.map_err(search_storage_error)?;

            row.map(|row| {
                let generation_id: String = row
                    .try_get("generation_id")
                    .map_err(|error| search_stored_value_invalid(error.to_string()))?;
                let projection_id: String = row
                    .try_get("projection_id")
                    .map_err(|error| search_stored_value_invalid(error.to_string()))?;
                let schema_version: String = row
                    .try_get("schema_version")
                    .map_err(|error| search_stored_value_invalid(error.to_string()))?;
                Ok(SearchIndexGeneration {
                    tenant_id,
                    index_id,
                    generation_id,
                    projection_id,
                    schema_version,
                    status: SearchGenerationStatus::Active,
                })
            })
            .transpose()
        })
    }

    fn register_building_generation<'a>(
        &'a self,
        generation: SearchIndexGeneration,
    ) -> crm_module_sdk::PortFuture<'a, Result<(), SdkError>> {
        Box::pin(async move {
            if generation.status != SearchGenerationStatus::Building {
                return Err(SdkError::new(
                    "SEARCH_GENERATION_STATUS_INVALID",
                    ErrorCategory::InvalidArgument,
                    false,
                    "The search generation status is invalid.",
                ));
            }
            self.register_search_generation(
                &generation.tenant_id,
                generation.index_id.as_str(),
                &generation.generation_id,
                &generation.projection_id,
                &generation.schema_version,
            )
            .await
        })
    }

    fn activate_generation<'a>(
        &'a self,
        tenant_id: TenantId,
        index_id: SearchIndexId,
        generation_id: String,
    ) -> crm_module_sdk::PortFuture<'a, Result<(), SdkError>> {
        Box::pin(async move {
            self.activate_search_generation(&tenant_id, index_id.as_str(), &generation_id)
                .await
        })
    }
}

async fn bind_search_tenant(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    tenant_id: &TenantId,
) -> Result<(), SdkError> {
    sqlx::query("SELECT set_config('app.tenant_id', $1, true)")
        .bind(tenant_id.as_str())
        .execute(&mut **transaction)
        .await
        .map_err(search_storage_error)?;
    Ok(())
}

fn search_stored_value_invalid(internal: impl Into<String>) -> SdkError {
    SdkError::new(
        "SEARCH_STORED_VALUE_INVALID",
        ErrorCategory::Unavailable,
        true,
        "Search is temporarily unavailable.",
    )
    .with_internal_reference(internal)
}

fn search_storage_error(error: sqlx::Error) -> SdkError {
    SdkError::new(
        "SEARCH_STORAGE_UNAVAILABLE",
        ErrorCategory::Unavailable,
        true,
        "Search is temporarily unavailable.",
    )
    .with_internal_reference(error.to_string())
}
