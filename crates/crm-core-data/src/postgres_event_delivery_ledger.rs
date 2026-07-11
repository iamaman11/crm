use crate::postgres::PostgresDataStore;
use crate::postgres_event_delivery::EventDeliveryQuery;
use crm_module_sdk::{ErrorCategory, EventDelivery, SdkError, TenantId};
use sqlx::Row;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaimedEventDelivery {
    pub delivery: Box<EventDelivery>,
    pub attempt_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventDeliveryClaim {
    InactiveConsumer,
    MissingSourceEvent,
    NotReady,
    Claimed(ClaimedEventDelivery),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventDeliveryCompletion {
    Applied,
    Ignored,
}

impl EventDeliveryCompletion {
    const fn database_status(self) -> &'static str {
        match self {
            Self::Applied => "applied",
            Self::Ignored => "ignored",
        }
    }
}

impl PostgresDataStore {
    /// Claims one consumer-scoped delivery with a lease.
    ///
    /// Source events are immutable, so reconstructing the envelope before the
    /// ledger transaction is safe. The claim itself is serialized by a single
    /// conditional UPDATE and can recover an expired processing lease.
    pub async fn claim_event_delivery(
        &self,
        query: &EventDeliveryQuery,
        worker_id: &str,
        now_unix_nanos: i64,
        lease_expires_at_unix_nanos: i64,
    ) -> Result<EventDeliveryClaim, SdkError> {
        validate_worker_id(worker_id)?;
        if now_unix_nanos < 0 || lease_expires_at_unix_nanos <= now_unix_nanos {
            return Err(delivery_invalid(
                "EVENT_DELIVERY_LEASE_INVALID",
                "The event delivery lease is invalid.",
            ));
        }

        let Some(delivery) = self.get_event_delivery(query).await? else {
            return Ok(EventDeliveryClaim::MissingSourceEvent);
        };
        if !self
            .is_module_active(&query.tenant_id, &query.consumer_module_id)
            .await?
        {
            return Ok(EventDeliveryClaim::InactiveConsumer);
        }

        let mut transaction = self.pool().begin().await.map_err(database_unavailable)?;
        bind_tenant(&mut transaction, &query.tenant_id).await?;
        sqlx::query(
            r#"
            INSERT INTO crm.event_deliveries (
              tenant_id, consumer_module_id, event_id, delivery_id,
              status, attempt_count, next_attempt_at
            )
            VALUES (
              $1, $2, $3, $4, 'pending', 0,
              TIMESTAMPTZ 'epoch' + ($5::bigint / 1000) * INTERVAL '1 microsecond'
            )
            ON CONFLICT (tenant_id, consumer_module_id, event_id) DO NOTHING
            "#,
        )
        .bind(query.tenant_id.as_str())
        .bind(query.consumer_module_id.as_str())
        .bind(query.event_id.as_str())
        .bind(delivery.delivery_id.as_str())
        .bind(now_unix_nanos)
        .execute(&mut *transaction)
        .await
        .map_err(database_unavailable)?;

        let claimed = sqlx::query(
            r#"
            UPDATE crm.event_deliveries
               SET status = 'processing',
                   attempt_count = attempt_count + 1,
                   lease_owner = $5,
                   lease_expires_at = TIMESTAMPTZ 'epoch'
                     + ($6::bigint / 1000) * INTERVAL '1 microsecond',
                   last_error_code = NULL,
                   updated_at = clock_timestamp()
             WHERE tenant_id = $1
               AND consumer_module_id = $2
               AND event_id = $3
               AND delivery_id = $4
               AND (
                 (
                   status IN ('pending', 'retry')
                   AND next_attempt_at <= TIMESTAMPTZ 'epoch'
                     + ($7::bigint / 1000) * INTERVAL '1 microsecond'
                 )
                 OR
                 (
                   status = 'processing'
                   AND lease_expires_at <= TIMESTAMPTZ 'epoch'
                     + ($7::bigint / 1000) * INTERVAL '1 microsecond'
                 )
               )
            RETURNING attempt_count
            "#,
        )
        .bind(query.tenant_id.as_str())
        .bind(query.consumer_module_id.as_str())
        .bind(query.event_id.as_str())
        .bind(delivery.delivery_id.as_str())
        .bind(worker_id)
        .bind(lease_expires_at_unix_nanos)
        .bind(now_unix_nanos)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(database_unavailable)?;
        transaction.commit().await.map_err(database_unavailable)?;

        let Some(row) = claimed else {
            return Ok(EventDeliveryClaim::NotReady);
        };
        let attempt_count: i32 = row
            .try_get("attempt_count")
            .map_err(|error| stored_value_invalid(error.to_string()))?;
        let attempt_count = u32::try_from(attempt_count)
            .map_err(|_| stored_value_invalid("negative delivery attempt count"))?;
        Ok(EventDeliveryClaim::Claimed(ClaimedEventDelivery {
            delivery: Box::new(delivery),
            attempt_count,
        }))
    }

    pub async fn complete_event_delivery(
        &self,
        tenant_id: &TenantId,
        delivery_id: &str,
        worker_id: &str,
        completion: EventDeliveryCompletion,
    ) -> Result<(), SdkError> {
        validate_worker_id(worker_id)?;
        let mut transaction = self.pool().begin().await.map_err(database_unavailable)?;
        bind_tenant(&mut transaction, tenant_id).await?;
        let update = sqlx::query(
            r#"
            UPDATE crm.event_deliveries
               SET status = $4,
                   lease_owner = NULL,
                   lease_expires_at = NULL,
                   completed_at = clock_timestamp(),
                   last_error_code = NULL,
                   updated_at = clock_timestamp()
             WHERE tenant_id = $1
               AND delivery_id = $2
               AND status = 'processing'
               AND lease_owner = $3
            "#,
        )
        .bind(tenant_id.as_str())
        .bind(delivery_id)
        .bind(worker_id)
        .bind(completion.database_status())
        .execute(&mut *transaction)
        .await
        .map_err(database_unavailable)?;
        if update.rows_affected() != 1 {
            return Err(delivery_conflict(
                "The event delivery lease is no longer owned by this worker.",
            ));
        }
        transaction.commit().await.map_err(database_unavailable)?;
        Ok(())
    }

    pub async fn retry_event_delivery(
        &self,
        tenant_id: &TenantId,
        delivery_id: &str,
        worker_id: &str,
        error_code: &str,
        next_attempt_at_unix_nanos: i64,
    ) -> Result<(), SdkError> {
        validate_worker_id(worker_id)?;
        validate_error_code(error_code)?;
        if next_attempt_at_unix_nanos < 0 {
            return Err(delivery_invalid(
                "EVENT_DELIVERY_RETRY_INVALID",
                "The event delivery retry request is invalid.",
            ));
        }
        let mut transaction = self.pool().begin().await.map_err(database_unavailable)?;
        bind_tenant(&mut transaction, tenant_id).await?;
        let update = sqlx::query(
            r#"
            UPDATE crm.event_deliveries
               SET status = 'retry',
                   next_attempt_at = TIMESTAMPTZ 'epoch'
                     + ($5::bigint / 1000) * INTERVAL '1 microsecond',
                   lease_owner = NULL,
                   lease_expires_at = NULL,
                   last_error_code = $4,
                   updated_at = clock_timestamp()
             WHERE tenant_id = $1
               AND delivery_id = $2
               AND status = 'processing'
               AND lease_owner = $3
            "#,
        )
        .bind(tenant_id.as_str())
        .bind(delivery_id)
        .bind(worker_id)
        .bind(error_code)
        .bind(next_attempt_at_unix_nanos)
        .execute(&mut *transaction)
        .await
        .map_err(database_unavailable)?;
        if update.rows_affected() != 1 {
            return Err(delivery_conflict(
                "The event delivery lease is no longer owned by this worker.",
            ));
        }
        transaction.commit().await.map_err(database_unavailable)?;
        Ok(())
    }

    pub async fn dead_letter_event_delivery(
        &self,
        tenant_id: &TenantId,
        delivery_id: &str,
        worker_id: &str,
        error_code: &str,
    ) -> Result<(), SdkError> {
        validate_worker_id(worker_id)?;
        validate_error_code(error_code)?;
        let mut transaction = self.pool().begin().await.map_err(database_unavailable)?;
        bind_tenant(&mut transaction, tenant_id).await?;
        let update = sqlx::query(
            r#"
            UPDATE crm.event_deliveries
               SET status = 'dead_letter',
                   lease_owner = NULL,
                   lease_expires_at = NULL,
                   last_error_code = $4,
                   updated_at = clock_timestamp()
             WHERE tenant_id = $1
               AND delivery_id = $2
               AND status = 'processing'
               AND lease_owner = $3
            "#,
        )
        .bind(tenant_id.as_str())
        .bind(delivery_id)
        .bind(worker_id)
        .bind(error_code)
        .execute(&mut *transaction)
        .await
        .map_err(database_unavailable)?;
        if update.rows_affected() != 1 {
            return Err(delivery_conflict(
                "The event delivery lease is no longer owned by this worker.",
            ));
        }
        transaction.commit().await.map_err(database_unavailable)?;
        Ok(())
    }
}

async fn bind_tenant(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    tenant_id: &TenantId,
) -> Result<(), SdkError> {
    sqlx::query("SELECT set_config('app.tenant_id', $1, true)")
        .bind(tenant_id.as_str())
        .execute(&mut **transaction)
        .await
        .map_err(database_unavailable)?;
    Ok(())
}

fn validate_worker_id(worker_id: &str) -> Result<(), SdkError> {
    if worker_id.is_empty() || worker_id.len() > 180 {
        return Err(delivery_invalid(
            "EVENT_DELIVERY_WORKER_INVALID",
            "The event delivery worker identity is invalid.",
        ));
    }
    Ok(())
}

fn validate_error_code(error_code: &str) -> Result<(), SdkError> {
    if error_code.is_empty() || error_code.len() > 180 {
        return Err(delivery_invalid(
            "EVENT_DELIVERY_ERROR_CODE_INVALID",
            "The event delivery error code is invalid.",
        ));
    }
    Ok(())
}

fn delivery_invalid(code: &'static str, safe_message: &'static str) -> SdkError {
    SdkError::new(code, ErrorCategory::InvalidArgument, false, safe_message)
}

fn delivery_conflict(safe_message: &'static str) -> SdkError {
    SdkError::new(
        "EVENT_DELIVERY_LEASE_CONFLICT",
        ErrorCategory::Conflict,
        true,
        safe_message,
    )
}

fn stored_value_invalid(internal: impl Into<String>) -> SdkError {
    SdkError::new(
        "EVENT_DELIVERY_STORED_VALUE_INVALID",
        ErrorCategory::Unavailable,
        true,
        "The event delivery service is temporarily unavailable.",
    )
    .with_internal_reference(internal)
}

fn database_unavailable(error: sqlx::Error) -> SdkError {
    SdkError::new(
        "EVENT_DELIVERY_UNAVAILABLE",
        ErrorCategory::Unavailable,
        true,
        "The event delivery service is temporarily unavailable.",
    )
    .with_internal_reference(error.to_string())
}
