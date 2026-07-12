use crate::PostgresDataStore;
use crm_module_sdk::{
    ErrorCategory, ModuleId, RecordId, RecordRef, RecordType, SdkError, TenantId,
};
use crm_search_runtime::{
    SearchCandidate, SearchCandidateCursor, SearchCandidatePage, SearchCandidateRequest,
    SearchCandidateStore,
};
use serde_json::{Map, Value};
use sqlx::Row;
use std::collections::BTreeMap;

const MAXIMUM_SEARCH_CANDIDATE_PAGE_SIZE: u32 = 500;

impl SearchCandidateStore for PostgresDataStore {
    fn search_candidates<'a>(
        &'a self,
        request: SearchCandidateRequest,
    ) -> crm_module_sdk::PortFuture<'a, Result<SearchCandidatePage, SdkError>> {
        Box::pin(async move { self.search_projection_candidates(&request).await })
    }
}

impl PostgresDataStore {
    pub async fn register_search_generation(
        &self,
        tenant_id: &TenantId,
        index_id: &str,
        generation_id: &str,
        projection_id: &str,
        schema_version: &str,
    ) -> Result<(), SdkError> {
        validate_coordinate(index_id, 180)?;
        validate_coordinate(generation_id, 180)?;
        validate_coordinate(projection_id, 180)?;
        validate_coordinate(schema_version, 120)?;
        let mut transaction = self.pool().begin().await.map_err(search_storage_error)?;
        bind_search_tenant(&mut transaction, tenant_id).await?;
        sqlx::query(
            r#"
            INSERT INTO crm.search_index_generations (
              tenant_id, index_id, generation_id, projection_id, schema_version, status
            )
            VALUES ($1, $2, $3, $4, $5, 'building')
            ON CONFLICT (tenant_id, index_id, generation_id) DO UPDATE SET
              projection_id = EXCLUDED.projection_id,
              schema_version = EXCLUDED.schema_version,
              status = CASE
                WHEN crm.search_index_generations.status = 'active' THEN 'active'
                ELSE 'building'
              END
            "#,
        )
        .bind(tenant_id.as_str())
        .bind(index_id)
        .bind(generation_id)
        .bind(projection_id)
        .bind(schema_version)
        .execute(&mut *transaction)
        .await
        .map_err(search_storage_error)?;
        transaction.commit().await.map_err(search_storage_error)?;
        Ok(())
    }

    pub async fn activate_search_generation(
        &self,
        tenant_id: &TenantId,
        index_id: &str,
        generation_id: &str,
    ) -> Result<(), SdkError> {
        validate_coordinate(index_id, 180)?;
        validate_coordinate(generation_id, 180)?;
        let mut transaction = self.pool().begin().await.map_err(search_storage_error)?;
        bind_search_tenant(&mut transaction, tenant_id).await?;
        sqlx::query(
            r#"
            UPDATE crm.search_index_generations
            SET status = 'retired'
            WHERE tenant_id = $1 AND index_id = $2 AND status = 'active'
            "#,
        )
        .bind(tenant_id.as_str())
        .bind(index_id)
        .execute(&mut *transaction)
        .await
        .map_err(search_storage_error)?;
        let activated = sqlx::query(
            r#"
            UPDATE crm.search_index_generations
            SET status = 'active', activated_at = clock_timestamp()
            WHERE tenant_id = $1 AND index_id = $2 AND generation_id = $3
            "#,
        )
        .bind(tenant_id.as_str())
        .bind(index_id)
        .bind(generation_id)
        .execute(&mut *transaction)
        .await
        .map_err(search_storage_error)?;
        if activated.rows_affected() != 1 {
            return Err(SdkError::new(
                "SEARCH_GENERATION_NOT_FOUND",
                ErrorCategory::NotFound,
                false,
                "The search index generation was not found.",
            ));
        }
        transaction.commit().await.map_err(search_storage_error)?;
        Ok(())
    }

    async fn search_projection_candidates(
        &self,
        request: &SearchCandidateRequest,
    ) -> Result<SearchCandidatePage, SdkError> {
        if request.page_size == 0 || request.page_size > MAXIMUM_SEARCH_CANDIDATE_PAGE_SIZE {
            return Err(search_request_invalid(
                "search candidate page size is invalid",
            ));
        }
        if request.normalized_text.is_empty() {
            return Err(search_request_invalid("search text is empty"));
        }
        let mut transaction = self.pool().begin().await.map_err(search_storage_error)?;
        bind_search_tenant(&mut transaction, &request.tenant_id).await?;
        sqlx::query("SET TRANSACTION READ ONLY")
            .execute(&mut *transaction)
            .await
            .map_err(search_storage_error)?;

        let active = sqlx::query(
            r#"
            SELECT projection_id
            FROM crm.search_index_generations
            WHERE tenant_id = $1 AND index_id = $2 AND status = 'active'
            "#,
        )
        .bind(request.tenant_id.as_str())
        .bind(request.index_id.as_str())
        .fetch_optional(&mut *transaction)
        .await
        .map_err(search_storage_error)?;
        let Some(active) = active else {
            transaction.commit().await.map_err(search_storage_error)?;
            return Err(SdkError::new(
                "SEARCH_INDEX_NOT_ACTIVE",
                ErrorCategory::Unavailable,
                true,
                "Search is temporarily unavailable.",
            ));
        };
        let projection_id: String = active
            .try_get("projection_id")
            .map_err(|error| search_stored_value_invalid(error.to_string()))?;
        let resource_types = request
            .resource_types
            .iter()
            .map(|value| value.as_str().to_owned())
            .collect::<Vec<_>>();
        let after_rank = request.after.as_ref().map(|cursor| cursor.rank_micros);
        let after_resource_type = request
            .after
            .as_ref()
            .map(|cursor| cursor.resource_type.as_str().to_owned());
        let after_resource_id = request
            .after
            .as_ref()
            .map(|cursor| cursor.resource_id.as_str().to_owned());
        let limit = i64::from(request.page_size) + 1;

        let rows = sqlx::query(
            r#"
            WITH ranked AS (
              SELECT
                resource_type,
                resource_id,
                source_version,
                document,
                GREATEST(
                  1::bigint,
                  ROUND(
                    ts_rank_cd(
                      to_tsvector('simple', COALESCE(document ->> 'search_text', '')),
                      websearch_to_tsquery('simple', $3)
                    ) * 1000000.0
                  )::bigint
                ) AS rank_micros
              FROM crm.projection_documents
              WHERE tenant_id = $1
                AND projection_id = $2
                AND (cardinality($4::text[]) = 0 OR resource_type = ANY($4::text[]))
                AND to_tsvector('simple', COALESCE(document ->> 'search_text', ''))
                    @@ websearch_to_tsquery('simple', $3)
            )
            SELECT resource_type, resource_id, source_version, document, rank_micros
            FROM ranked
            WHERE $5::bigint IS NULL
               OR rank_micros < $5
               OR (
                    rank_micros = $5
                    AND (
                      resource_type > $6
                      OR (resource_type = $6 AND resource_id > $7)
                    )
                  )
            ORDER BY rank_micros DESC, resource_type ASC, resource_id ASC
            LIMIT $8
            "#,
        )
        .bind(request.tenant_id.as_str())
        .bind(&projection_id)
        .bind(&request.normalized_text)
        .bind(&resource_types)
        .bind(after_rank)
        .bind(after_resource_type)
        .bind(after_resource_id)
        .bind(limit)
        .fetch_all(&mut *transaction)
        .await
        .map_err(search_storage_error)?;
        transaction.commit().await.map_err(search_storage_error)?;

        let has_more = rows.len() > request.page_size as usize;
        let mut candidates = rows
            .into_iter()
            .take(request.page_size as usize)
            .map(decode_search_candidate)
            .collect::<Result<Vec<_>, _>>()?;
        let next_after = if has_more {
            candidates.last().map(|candidate| SearchCandidateCursor {
                rank_micros: candidate.rank_micros,
                resource_type: candidate.resource.record_type.clone(),
                resource_id: candidate.resource.record_id.clone(),
            })
        } else {
            None
        };
        candidates.shrink_to_fit();
        Ok(SearchCandidatePage {
            candidates,
            next_after,
        })
    }
}

fn decode_search_candidate(row: sqlx::postgres::PgRow) -> Result<SearchCandidate, SdkError> {
    let resource_type: String = row
        .try_get("resource_type")
        .map_err(|error| search_stored_value_invalid(error.to_string()))?;
    let resource_id: String = row
        .try_get("resource_id")
        .map_err(|error| search_stored_value_invalid(error.to_string()))?;
    let source_version: i64 = row
        .try_get("source_version")
        .map_err(|error| search_stored_value_invalid(error.to_string()))?;
    let rank_micros: i64 = row
        .try_get("rank_micros")
        .map_err(|error| search_stored_value_invalid(error.to_string()))?;
    let document: Value = row
        .try_get("document")
        .map_err(|error| search_stored_value_invalid(error.to_string()))?;
    let object = document.as_object().ok_or_else(|| {
        search_stored_value_invalid("search projection document is not an object")
    })?;
    let owner_module_id = string_field(object, "owner_module_id")?;
    Ok(SearchCandidate {
        owner_module_id: ModuleId::try_new(owner_module_id)
            .map_err(|error| search_stored_value_invalid(error.to_string()))?,
        resource: RecordRef {
            record_type: RecordType::try_new(resource_type)
                .map_err(|error| search_stored_value_invalid(error.to_string()))?,
            record_id: RecordId::try_new(resource_id)
                .map_err(|error| search_stored_value_invalid(error.to_string()))?,
        },
        source_version,
        rank_micros,
        searchable_fields: string_map_field(object, "searchable_fields")?,
        display_fields: string_map_field(object, "display_fields")?,
    })
}

fn string_field<'a>(object: &'a Map<String, Value>, field: &str) -> Result<&'a str, SdkError> {
    object
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| search_stored_value_invalid(format!("missing string field: {field}")))
}

fn string_map_field(
    object: &Map<String, Value>,
    field: &str,
) -> Result<BTreeMap<String, String>, SdkError> {
    let values = object
        .get(field)
        .and_then(Value::as_object)
        .ok_or_else(|| search_stored_value_invalid(format!("missing object field: {field}")))?;
    values
        .iter()
        .map(|(name, value)| {
            let value = value.as_str().ok_or_else(|| {
                search_stored_value_invalid(format!(
                    "non-string search field value: {field}.{name}"
                ))
            })?;
            Ok((name.clone(), value.to_owned()))
        })
        .collect()
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

fn validate_coordinate(value: &str, maximum: usize) -> Result<(), SdkError> {
    if value.is_empty() || value.len() > maximum || value.chars().any(char::is_control) {
        return Err(search_request_invalid("search coordinate is invalid"));
    }
    Ok(())
}

fn search_request_invalid(message: &'static str) -> SdkError {
    SdkError::new(
        "SEARCH_REQUEST_INVALID",
        ErrorCategory::InvalidArgument,
        false,
        message,
    )
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
