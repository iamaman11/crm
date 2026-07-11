use crate::postgres::PostgresDataStore;
use crate::postgres_batch::{BatchError, parse_data_class, parse_payload_encoding};
use crm_module_sdk::{
    ActorId, CorrelationId, DeliveryId, ErrorCategory, EventDelivery, EventId, EventType,
    EventVersion, ModuleId, RecordId, RecordRef, RecordType, RetentionPolicyId, SchemaId,
    SchemaVersion, SdkError, TenantId, TraceId, TypedPayload,
};
use sha2::{Digest, Sha256};
use sqlx::Row;
use std::fmt::Write as _;

const DELIVERY_ID_PROFILE: &[u8] = b"crm.event-delivery-id/v1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventDeliveryQuery {
    pub tenant_id: TenantId,
    pub event_id: EventId,
    pub consumer_module_id: ModuleId,
}

impl PostgresDataStore {
    /// Reconstructs one immutable source event as a consumer-scoped delivery.
    ///
    /// The authoritative outbox remains source-owned. Consumer identity is bound
    /// only when the delivery envelope is materialized, producing a stable
    /// consumer-scoped delivery id suitable for retries and idempotency.
    pub async fn get_event_delivery(
        &self,
        query: &EventDeliveryQuery,
    ) -> Result<Option<EventDelivery>, SdkError> {
        let mut transaction = self.pool().begin().await.map_err(database_unavailable)?;
        bind_read_tenant(&mut transaction, &query.tenant_id).await?;
        let row = sqlx::query(
            r#"
            SELECT
              e.event_id,
              e.aggregate_type,
              e.aggregate_id,
              e.aggregate_version,
              e.event_type,
              e.schema_id,
              e.schema_version,
              e.descriptor_hash,
              e.data_class,
              e.payload_encoding,
              e.maximum_payload_size,
              e.retention_policy_id,
              e.payload_bytes,
              ((EXTRACT(EPOCH FROM e.occurred_at) * 1000000)::bigint * 1000)
                AS occurred_at_unix_nanos,
              bt.actor_id AS source_actor_id,
              bt.correlation_id,
              bt.trace_id,
              c.owner_module_id AS source_module_id
            FROM crm.outbox_events e
            JOIN crm.business_transactions bt
              ON bt.tenant_id = e.tenant_id
             AND bt.business_transaction_id = e.business_transaction_id
            JOIN crm.capability_registry c
              ON c.capability_id = bt.capability_id
             AND c.capability_version = bt.capability_version
            WHERE e.tenant_id = $1
              AND e.event_id = $2
            "#,
        )
        .bind(query.tenant_id.as_str())
        .bind(query.event_id.as_str())
        .fetch_optional(&mut *transaction)
        .await
        .map_err(database_unavailable)?;
        transaction.commit().await.map_err(database_unavailable)?;

        row.map(|row| decode_event_delivery(query, row)).transpose()
    }

    /// Returns true only for an installed consumer in the exact `active` state.
    /// Missing, installed-only, suspended, upgrading, rollback, uninstalling and
    /// failed installations are all non-runnable.
    pub async fn is_module_active(
        &self,
        tenant_id: &TenantId,
        module_id: &ModuleId,
    ) -> Result<bool, SdkError> {
        let mut transaction = self.pool().begin().await.map_err(database_unavailable)?;
        bind_read_tenant(&mut transaction, tenant_id).await?;
        let status = sqlx::query_scalar::<_, String>(
            r#"
            SELECT status
            FROM crm.module_installations
            WHERE tenant_id = $1
              AND module_id = $2
            "#,
        )
        .bind(tenant_id.as_str())
        .bind(module_id.as_str())
        .fetch_optional(&mut *transaction)
        .await
        .map_err(database_unavailable)?;
        transaction.commit().await.map_err(database_unavailable)?;
        Ok(status.as_deref() == Some("active"))
    }
}

async fn bind_read_tenant(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    tenant_id: &TenantId,
) -> Result<(), SdkError> {
    sqlx::query("SET TRANSACTION READ ONLY")
        .execute(&mut **transaction)
        .await
        .map_err(database_unavailable)?;
    sqlx::query("SELECT set_config('app.tenant_id', $1, true)")
        .bind(tenant_id.as_str())
        .execute(&mut **transaction)
        .await
        .map_err(database_unavailable)?;
    Ok(())
}

fn decode_event_delivery(
    query: &EventDeliveryQuery,
    row: sqlx::postgres::PgRow,
) -> Result<EventDelivery, SdkError> {
    let source_module_id = parse_identifier(
        "source module",
        row.try_get::<String, _>("source_module_id"),
        ModuleId::try_new,
    )?;
    let event_id = parse_identifier(
        "event id",
        row.try_get::<String, _>("event_id"),
        EventId::try_new,
    )?;
    if event_id != query.event_id {
        return Err(stored_value_invalid("event identity changed during read"));
    }
    let aggregate_type = parse_identifier(
        "aggregate type",
        row.try_get::<String, _>("aggregate_type"),
        RecordType::try_new,
    )?;
    let aggregate_id = parse_identifier(
        "aggregate id",
        row.try_get::<String, _>("aggregate_id"),
        RecordId::try_new,
    )?;
    let source_actor_id = parse_identifier(
        "source actor",
        row.try_get::<String, _>("source_actor_id"),
        ActorId::try_new,
    )?;
    let event_type = parse_identifier(
        "event type",
        row.try_get::<String, _>("event_type"),
        EventType::try_new,
    )?;
    let schema_id = parse_identifier(
        "schema id",
        row.try_get::<String, _>("schema_id"),
        SchemaId::try_new,
    )?;
    let schema_version = parse_identifier(
        "schema version",
        row.try_get::<String, _>("schema_version"),
        SchemaVersion::try_new,
    )?;
    let event_version = EventVersion::try_new(schema_version.as_str())
        .map_err(|error| stored_value_invalid(error.to_string()))?;
    let correlation_id = parse_identifier(
        "correlation id",
        row.try_get::<String, _>("correlation_id"),
        CorrelationId::try_new,
    )?;
    let trace_id = parse_identifier(
        "trace id",
        row.try_get::<String, _>("trace_id"),
        TraceId::try_new,
    )?;
    let retention_policy_id = parse_identifier(
        "retention policy",
        row.try_get::<String, _>("retention_policy_id"),
        RetentionPolicyId::try_new,
    )?;
    let descriptor_hash: Vec<u8> = row
        .try_get("descriptor_hash")
        .map_err(|error| stored_value_invalid(error.to_string()))?;
    let descriptor_hash: [u8; 32] = descriptor_hash
        .try_into()
        .map_err(|_| stored_value_invalid("event descriptor hash is not 32 bytes"))?;
    let maximum_payload_size: i64 = row
        .try_get("maximum_payload_size")
        .map_err(|error| stored_value_invalid(error.to_string()))?;
    let maximum_size_bytes = u64::try_from(maximum_payload_size)
        .map_err(|_| stored_value_invalid("event maximum payload size is negative"))?;
    let data_class = parse_data_class(
        row.try_get("data_class")
            .map_err(|error| stored_value_invalid(error.to_string()))?,
    )
    .map_err(batch_decode_error)?;
    let encoding = parse_payload_encoding(
        row.try_get("payload_encoding")
            .map_err(|error| stored_value_invalid(error.to_string()))?,
    )
    .map_err(batch_decode_error)?;
    let aggregate_version: i64 = row
        .try_get("aggregate_version")
        .map_err(|error| stored_value_invalid(error.to_string()))?;
    let occurred_at_unix_nanos: i64 = row
        .try_get("occurred_at_unix_nanos")
        .map_err(|error| stored_value_invalid(error.to_string()))?;

    let payload = TypedPayload {
        owner: source_module_id.clone(),
        schema_id,
        schema_version,
        descriptor_hash,
        data_class,
        encoding,
        maximum_size_bytes,
        retention_policy_id,
        bytes: row
            .try_get("payload_bytes")
            .map_err(|error| stored_value_invalid(error.to_string()))?,
    };
    let delivery = EventDelivery {
        delivery_id: deterministic_delivery_id(
            &query.tenant_id,
            &query.consumer_module_id,
            &event_id,
        )?,
        event_id,
        tenant_id: query.tenant_id.clone(),
        source_module_id,
        consumer_module_id: query.consumer_module_id.clone(),
        source_actor_id,
        event_type,
        event_version,
        aggregate: RecordRef {
            record_type: aggregate_type,
            record_id: aggregate_id,
        },
        aggregate_version,
        occurred_at_unix_nanos,
        correlation_id,
        trace_id,
        payload,
    };
    delivery
        .validate()
        .map_err(|error| stored_value_invalid(error.to_string()))?;
    Ok(delivery)
}

fn deterministic_delivery_id(
    tenant_id: &TenantId,
    consumer_module_id: &ModuleId,
    event_id: &EventId,
) -> Result<DeliveryId, SdkError> {
    let mut hasher = Sha256::new();
    hash_field(&mut hasher, DELIVERY_ID_PROFILE);
    hash_field(&mut hasher, tenant_id.as_str().as_bytes());
    hash_field(&mut hasher, consumer_module_id.as_str().as_bytes());
    hash_field(&mut hasher, event_id.as_str().as_bytes());
    let digest = hasher.finalize();
    let mut value = String::with_capacity("delivery-".len() + digest.len() * 2);
    value.push_str("delivery-");
    for byte in digest {
        write!(&mut value, "{byte:02x}")
            .map_err(|error| stored_value_invalid(error.to_string()))?;
    }
    DeliveryId::try_new(value).map_err(|error| stored_value_invalid(error.to_string()))
}

fn hash_field(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_be_bytes());
    hasher.update(bytes);
}

fn parse_identifier<T, E>(
    label: &str,
    value: Result<String, sqlx::Error>,
    parser: impl FnOnce(String) -> Result<T, E>,
) -> Result<T, SdkError>
where
    E: std::fmt::Display,
{
    let value = value.map_err(|error| stored_value_invalid(error.to_string()))?;
    parser(value).map_err(|error| stored_value_invalid(format!("invalid {label}: {error}")))
}

fn batch_decode_error(error: BatchError) -> SdkError {
    stored_value_invalid(error.to_string())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delivery_identity_is_consumer_scoped_and_stable() {
        let tenant = TenantId::try_new("tenant-a").unwrap();
        let event = EventId::try_new("event-1").unwrap();
        let first_consumer = ModuleId::try_new("crm.link-a").unwrap();
        let second_consumer = ModuleId::try_new("crm.link-b").unwrap();

        let first = deterministic_delivery_id(&tenant, &first_consumer, &event).unwrap();
        let replay = deterministic_delivery_id(&tenant, &first_consumer, &event).unwrap();
        let other = deterministic_delivery_id(&tenant, &second_consumer, &event).unwrap();

        assert_eq!(first, replay);
        assert_ne!(first, other);
        assert!(first.as_str().starts_with("delivery-"));
    }
}
