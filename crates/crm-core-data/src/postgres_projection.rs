use crate::postgres::PostgresDataStore;
use crate::postgres_event_delivery::EventDeliveryQuery;
use crm_core_events::{
    EventHistoryCursor, EventHistoryPage, EventHistoryRequest, ProjectionApplyResult,
    ProjectionCheckpoint, ProjectionEventApplication,
};
use crm_module_sdk::{ErrorCategory, EventId, SdkError, TenantId};
use sqlx::Row;

impl PostgresDataStore {
    pub async fn list_event_history(
        &self,
        request: &EventHistoryRequest,
    ) -> Result<EventHistoryPage, SdkError> {
        let page_size = request
            .effective_page_size()
            .map_err(projection_request_invalid)?;
        let query_limit = i64::from(page_size) + 1;
        let event_types = request
            .event_types
            .iter()
            .map(|event_type| event_type.as_str().to_owned())
            .collect::<Vec<_>>();
        let after_time = request
            .after
            .as_ref()
            .map(|cursor| cursor.occurred_at_unix_nanos);
        let after_event_id = request
            .after
            .as_ref()
            .map(|cursor| cursor.event_id.as_str().to_owned());

        let mut transaction = self
            .pool()
            .begin()
            .await
            .map_err(projection_database_error)?;
        bind_projection_tenant(&mut transaction, &request.tenant_id).await?;
        sqlx::query("SET TRANSACTION READ ONLY")
            .execute(&mut *transaction)
            .await
            .map_err(projection_database_error)?;
        let rows = sqlx::query(
            r#"
            SELECT
              event_id,
              ((EXTRACT(EPOCH FROM occurred_at) * 1000000)::bigint * 1000)
                AS occurred_at_unix_nanos
            FROM crm.outbox_events
            WHERE tenant_id = $1
              AND event_type = ANY($2::text[])
              AND (
                $3::bigint IS NULL
                OR ((EXTRACT(EPOCH FROM occurred_at) * 1000000)::bigint * 1000) > $3
                OR (
                  ((EXTRACT(EPOCH FROM occurred_at) * 1000000)::bigint * 1000) = $3
                  AND event_id > $4
                )
              )
            ORDER BY occurred_at ASC, event_id ASC
            LIMIT $5
            "#,
        )
        .bind(request.tenant_id.as_str())
        .bind(&event_types)
        .bind(after_time)
        .bind(after_event_id.as_deref().unwrap_or(""))
        .bind(query_limit)
        .fetch_all(&mut *transaction)
        .await
        .map_err(projection_database_error)?;
        transaction
            .commit()
            .await
            .map_err(projection_database_error)?;

        let has_more = rows.len() > page_size as usize;
        let selected = rows
            .into_iter()
            .take(page_size as usize)
            .collect::<Vec<_>>();
        let mut deliveries = Vec::with_capacity(selected.len());
        let mut last_cursor = None;
        for row in selected {
            let event_id = EventId::try_new(
                row.try_get::<String, _>("event_id")
                    .map_err(|error| projection_stored_value_invalid(error.to_string()))?,
            )
            .map_err(|error| projection_stored_value_invalid(error.to_string()))?;
            let occurred_at_unix_nanos: i64 = row
                .try_get("occurred_at_unix_nanos")
                .map_err(|error| projection_stored_value_invalid(error.to_string()))?;
            let delivery = self
                .get_event_delivery(&EventDeliveryQuery {
                    tenant_id: request.tenant_id.clone(),
                    event_id: event_id.clone(),
                    consumer_module_id: request.consumer_module_id.clone(),
                })
                .await?
                .ok_or_else(|| {
                    projection_stored_value_invalid(
                        "event disappeared while reconstructing immutable projection history",
                    )
                })?;
            last_cursor = Some(EventHistoryCursor {
                occurred_at_unix_nanos,
                event_id,
            });
            deliveries.push(delivery);
        }

        Ok(EventHistoryPage {
            deliveries,
            next_cursor: if has_more { last_cursor } else { None },
        })
    }

    pub async fn apply_projection_event(
        &self,
        application: &ProjectionEventApplication,
    ) -> Result<ProjectionApplyResult, SdkError> {
        application.validate().map_err(projection_request_invalid)?;
        let tenant_id = &application.delivery.tenant_id;
        let occurred_at = application.delivery.occurred_at_unix_nanos;
        let event_id = application.delivery.event_id.as_str();
        let mut transaction = self
            .pool()
            .begin()
            .await
            .map_err(projection_database_error)?;
        bind_projection_tenant(&mut transaction, tenant_id).await?;

        sqlx::query(
            r#"
            INSERT INTO crm.projection_checkpoints (
              tenant_id, projection_id, last_occurred_at, last_event_id,
              applied_event_count, status
            )
            VALUES (
              $1, $2,
              TIMESTAMPTZ 'epoch' + ($3::bigint / 1000) * INTERVAL '1 microsecond',
              $4, 0, 'active'
            )
            ON CONFLICT (tenant_id, projection_id) DO NOTHING
            "#,
        )
        .bind(tenant_id.as_str())
        .bind(&application.projection_id)
        .bind(occurred_at)
        .bind(event_id)
        .execute(&mut *transaction)
        .await
        .map_err(projection_database_error)?;

        let checkpoint = sqlx::query(
            r#"
            SELECT
              ((EXTRACT(EPOCH FROM last_occurred_at) * 1000000)::bigint * 1000)
                AS last_occurred_at_unix_nanos,
              last_event_id,
              status
            FROM crm.projection_checkpoints
            WHERE tenant_id = $1 AND projection_id = $2
            FOR UPDATE
            "#,
        )
        .bind(tenant_id.as_str())
        .bind(&application.projection_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(projection_database_error)?;
        let checkpoint_time: i64 = checkpoint
            .try_get("last_occurred_at_unix_nanos")
            .map_err(|error| projection_stored_value_invalid(error.to_string()))?;
        let checkpoint_event_id: String = checkpoint
            .try_get("last_event_id")
            .map_err(|error| projection_stored_value_invalid(error.to_string()))?;
        let checkpoint_status: String = checkpoint
            .try_get("status")
            .map_err(|error| projection_stored_value_invalid(error.to_string()))?;
        if checkpoint_status != "active" {
            return Err(projection_conflict(
                "The projection checkpoint is failed and must be reset or repaired before applying more events.",
            ));
        }

        let already_applied: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS (
              SELECT 1
              FROM crm.projection_applied_events
              WHERE tenant_id = $1 AND projection_id = $2 AND event_id = $3
            )
            "#,
        )
        .bind(tenant_id.as_str())
        .bind(&application.projection_id)
        .bind(event_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(projection_database_error)?;
        if already_applied {
            transaction
                .commit()
                .await
                .map_err(projection_database_error)?;
            return Ok(ProjectionApplyResult {
                replayed: true,
                documents_written: 0,
            });
        }

        if (occurred_at, event_id) < (checkpoint_time, checkpoint_event_id.as_str()) {
            return Err(projection_conflict(
                "The projection event is older than the current checkpoint.",
            ));
        }

        sqlx::query(
            r#"
            INSERT INTO crm.projection_applied_events (
              tenant_id, projection_id, event_id, occurred_at
            )
            VALUES (
              $1, $2, $3,
              TIMESTAMPTZ 'epoch' + ($4::bigint / 1000) * INTERVAL '1 microsecond'
            )
            "#,
        )
        .bind(tenant_id.as_str())
        .bind(&application.projection_id)
        .bind(event_id)
        .bind(occurred_at)
        .execute(&mut *transaction)
        .await
        .map_err(projection_database_error)?;

        let mut documents_written = 0_u32;
        for write in &application.writes {
            let result = sqlx::query(
                r#"
                INSERT INTO crm.projection_documents (
                  tenant_id, projection_id, resource_type, resource_id,
                  source_event_id, source_version, document
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7)
                ON CONFLICT (tenant_id, projection_id, resource_type, resource_id)
                DO UPDATE SET
                  source_event_id = EXCLUDED.source_event_id,
                  source_version = EXCLUDED.source_version,
                  document = EXCLUDED.document,
                  updated_at = clock_timestamp()
                WHERE EXCLUDED.source_version >= crm.projection_documents.source_version
                "#,
            )
            .bind(tenant_id.as_str())
            .bind(&application.projection_id)
            .bind(&write.resource_type)
            .bind(&write.resource_id)
            .bind(event_id)
            .bind(write.source_version)
            .bind(&write.document)
            .execute(&mut *transaction)
            .await
            .map_err(projection_database_error)?;
            documents_written = documents_written
                .checked_add(u32::try_from(result.rows_affected()).unwrap_or(u32::MAX))
                .ok_or_else(|| {
                    projection_stored_value_invalid("projection write count overflow")
                })?;
        }

        sqlx::query(
            r#"
            UPDATE crm.projection_checkpoints
               SET last_occurred_at = TIMESTAMPTZ 'epoch'
                     + ($3::bigint / 1000) * INTERVAL '1 microsecond',
                   last_event_id = $4,
                   applied_event_count = applied_event_count + 1,
                   status = 'active',
                   failure_event_id = NULL,
                   failure_code = NULL,
                   updated_at = clock_timestamp()
             WHERE tenant_id = $1 AND projection_id = $2
            "#,
        )
        .bind(tenant_id.as_str())
        .bind(&application.projection_id)
        .bind(occurred_at)
        .bind(event_id)
        .execute(&mut *transaction)
        .await
        .map_err(projection_database_error)?;
        transaction
            .commit()
            .await
            .map_err(projection_database_error)?;

        Ok(ProjectionApplyResult {
            replayed: false,
            documents_written,
        })
    }

    pub async fn projection_checkpoint(
        &self,
        tenant_id: &TenantId,
        projection_id: &str,
    ) -> Result<Option<ProjectionCheckpoint>, SdkError> {
        validate_projection_id(projection_id)?;
        let mut transaction = self
            .pool()
            .begin()
            .await
            .map_err(projection_database_error)?;
        bind_projection_tenant(&mut transaction, tenant_id).await?;
        sqlx::query("SET TRANSACTION READ ONLY")
            .execute(&mut *transaction)
            .await
            .map_err(projection_database_error)?;
        let row = sqlx::query(
            r#"
            SELECT
              ((EXTRACT(EPOCH FROM last_occurred_at) * 1000000)::bigint * 1000)
                AS last_occurred_at_unix_nanos,
              last_event_id,
              applied_event_count
            FROM crm.projection_checkpoints
            WHERE tenant_id = $1 AND projection_id = $2
            "#,
        )
        .bind(tenant_id.as_str())
        .bind(projection_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(projection_database_error)?;
        transaction
            .commit()
            .await
            .map_err(projection_database_error)?;
        row.map(|row| decode_checkpoint(tenant_id, projection_id, row))
            .transpose()
    }

    pub async fn reset_projection(
        &self,
        tenant_id: &TenantId,
        projection_id: &str,
    ) -> Result<(), SdkError> {
        validate_projection_id(projection_id)?;
        let mut transaction = self
            .pool()
            .begin()
            .await
            .map_err(projection_database_error)?;
        bind_projection_tenant(&mut transaction, tenant_id).await?;
        sqlx::query(
            "DELETE FROM crm.projection_checkpoints WHERE tenant_id = $1 AND projection_id = $2",
        )
        .bind(tenant_id.as_str())
        .bind(projection_id)
        .execute(&mut *transaction)
        .await
        .map_err(projection_database_error)?;
        transaction
            .commit()
            .await
            .map_err(projection_database_error)?;
        Ok(())
    }

    pub async fn projection_document(
        &self,
        tenant_id: &TenantId,
        projection_id: &str,
        resource_type: &str,
        resource_id: &str,
    ) -> Result<Option<serde_json::Value>, SdkError> {
        validate_projection_id(projection_id)?;
        if resource_type.is_empty()
            || resource_type.len() > 180
            || resource_id.is_empty()
            || resource_id.len() > 360
        {
            return Err(projection_request_invalid(
                "projection resource identity is invalid",
            ));
        }
        let mut transaction = self
            .pool()
            .begin()
            .await
            .map_err(projection_database_error)?;
        bind_projection_tenant(&mut transaction, tenant_id).await?;
        sqlx::query("SET TRANSACTION READ ONLY")
            .execute(&mut *transaction)
            .await
            .map_err(projection_database_error)?;
        let document = sqlx::query_scalar::<_, serde_json::Value>(
            r#"
            SELECT document
            FROM crm.projection_documents
            WHERE tenant_id = $1
              AND projection_id = $2
              AND resource_type = $3
              AND resource_id = $4
            "#,
        )
        .bind(tenant_id.as_str())
        .bind(projection_id)
        .bind(resource_type)
        .bind(resource_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(projection_database_error)?;
        transaction
            .commit()
            .await
            .map_err(projection_database_error)?;
        Ok(document)
    }

    pub async fn projection_documents(
        &self,
        tenant_id: &TenantId,
        projection_id: &str,
        resource_type: &str,
    ) -> Result<Vec<serde_json::Value>, SdkError> {
        validate_projection_id(projection_id)?;
        if resource_type.is_empty() || resource_type.len() > 180 {
            return Err(projection_request_invalid(
                "projection resource type is invalid",
            ));
        }
        let mut transaction = self
            .pool()
            .begin()
            .await
            .map_err(projection_database_error)?;
        bind_projection_tenant(&mut transaction, tenant_id).await?;
        sqlx::query("SET TRANSACTION READ ONLY")
            .execute(&mut *transaction)
            .await
            .map_err(projection_database_error)?;
        let rows = sqlx::query_scalar::<_, serde_json::Value>(
            r#"
            SELECT document
            FROM crm.projection_documents
            WHERE tenant_id = $1
              AND projection_id = $2
              AND resource_type = $3
            ORDER BY resource_id ASC
            "#,
        )
        .bind(tenant_id.as_str())
        .bind(projection_id)
        .bind(resource_type)
        .fetch_all(&mut *transaction)
        .await
        .map_err(projection_database_error)?;
        transaction
            .commit()
            .await
            .map_err(projection_database_error)?;
        Ok(rows)
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
        .map_err(projection_database_error)?;
    Ok(())
}

fn decode_checkpoint(
    tenant_id: &TenantId,
    projection_id: &str,
    row: sqlx::postgres::PgRow,
) -> Result<ProjectionCheckpoint, SdkError> {
    let event_id = EventId::try_new(
        row.try_get::<String, _>("last_event_id")
            .map_err(|error| projection_stored_value_invalid(error.to_string()))?,
    )
    .map_err(|error| projection_stored_value_invalid(error.to_string()))?;
    let applied_event_count: i64 = row
        .try_get("applied_event_count")
        .map_err(|error| projection_stored_value_invalid(error.to_string()))?;
    Ok(ProjectionCheckpoint {
        tenant_id: tenant_id.clone(),
        projection_id: projection_id.to_owned(),
        cursor: EventHistoryCursor {
            occurred_at_unix_nanos: row
                .try_get("last_occurred_at_unix_nanos")
                .map_err(|error| projection_stored_value_invalid(error.to_string()))?,
            event_id,
        },
        applied_event_count: u64::try_from(applied_event_count)
            .map_err(|_| projection_stored_value_invalid("negative projection event count"))?,
    })
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

fn projection_conflict(message: &'static str) -> SdkError {
    SdkError::new(
        "PROJECTION_CHECKPOINT_CONFLICT",
        ErrorCategory::Conflict,
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

fn projection_database_error(error: sqlx::Error) -> SdkError {
    SdkError::new(
        "PROJECTION_STORAGE_UNAVAILABLE",
        ErrorCategory::Unavailable,
        true,
        "The projection service is temporarily unavailable.",
    )
    .with_internal_reference(error.to_string())
}
