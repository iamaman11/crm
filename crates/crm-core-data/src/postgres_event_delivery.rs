use crate::PostgresDataStore;
use crm_core_events::{EventDeliveryLookup, EventDeliveryReader};
use crm_module_sdk::{
    ActorId, CorrelationId, DataClass, DeliveryId, ErrorCategory, EventDelivery, EventId,
    EventType, EventVersion, ModuleId, PayloadEncoding, PortFuture, PortResult, RecordId,
    RecordRef, RecordType, RetentionPolicyId, SchemaId, SchemaVersion, SdkError, TraceId,
    TypedPayload,
};
use sqlx::Row;

/// FORCE-RLS reader that reconstructs one immutable SDK `EventDelivery` from
/// authoritative transactional outbox evidence.
#[derive(Debug, Clone)]
pub struct PostgresEventDeliveryReader {
    store: PostgresDataStore,
}

impl PostgresEventDeliveryReader {
    pub fn new(store: PostgresDataStore) -> Self {
        Self { store }
    }
}

impl EventDeliveryReader for PostgresEventDeliveryReader {
    fn load<'a>(
        &'a self,
        lookup: &'a EventDeliveryLookup,
    ) -> PortFuture<'a, PortResult<Option<EventDelivery>>> {
        Box::pin(async move {
            let mut transaction = self
                .store
                .pool()
                .begin()
                .await
                .map_err(event_database_error)?;
            sqlx::query("SET TRANSACTION READ ONLY")
                .execute(&mut *transaction)
                .await
                .map_err(event_database_error)?;
            sqlx::query("SELECT set_config('app.tenant_id', $1, true)")
                .bind(lookup.tenant_id.as_str())
                .execute(&mut *transaction)
                .await
                .map_err(event_database_error)?;

            let row = sqlx::query(
                r#"
                SELECT
                  event_id,
                  source_module_id,
                  source_actor_id,
                  event_type,
                  event_version,
                  aggregate_type,
                  aggregate_id,
                  aggregate_version,
                  (EXTRACT(EPOCH FROM occurred_at) * 1000000000)::bigint AS occurred_at_unix_nanos,
                  correlation_id,
                  trace_id,
                  schema_id,
                  schema_version,
                  descriptor_hash,
                  data_class,
                  payload_encoding,
                  maximum_payload_size,
                  retention_policy_id,
                  payload_bytes
                FROM crm.outbox_events
                WHERE tenant_id = $1 AND event_id = $2
                "#,
            )
            .bind(lookup.tenant_id.as_str())
            .bind(lookup.event_id.as_str())
            .fetch_optional(&mut *transaction)
            .await
            .map_err(event_database_error)?;
            transaction.commit().await.map_err(event_database_error)?;

            row.map(|row| decode_delivery(lookup, row)).transpose()
        })
    }
}

fn decode_delivery(
    lookup: &EventDeliveryLookup,
    row: sqlx::postgres::PgRow,
) -> Result<EventDelivery, SdkError> {
    let source_module_id = ModuleId::try_new(
        row.try_get::<String, _>("source_module_id")
            .map_err(event_database_error)?,
    )
    .map_err(|_| event_stored_value_invalid("source module id is invalid"))?;
    let event_version = EventVersion::try_new(
        row.try_get::<String, _>("event_version")
            .map_err(event_database_error)?,
    )
    .map_err(|_| event_stored_value_invalid("event version is invalid"))?;
    let descriptor_hash: Vec<u8> = row
        .try_get("descriptor_hash")
        .map_err(event_database_error)?;
    let descriptor_hash: [u8; 32] = descriptor_hash
        .try_into()
        .map_err(|_| event_stored_value_invalid("descriptor hash is not 32 bytes"))?;
    let maximum_size: i64 = row
        .try_get("maximum_payload_size")
        .map_err(event_database_error)?;
    let maximum_size_bytes = u64::try_from(maximum_size)
        .map_err(|_| event_stored_value_invalid("maximum payload size is negative"))?;

    let delivery = EventDelivery {
        delivery_id: DeliveryId::try_new(lookup.delivery_id.as_str().to_owned())?,
        event_id: EventId::try_new(
            row.try_get::<String, _>("event_id")
                .map_err(event_database_error)?,
        )?,
        tenant_id: lookup.tenant_id.clone(),
        source_module_id: source_module_id.clone(),
        consumer_module_id: lookup.consumer_module_id.clone(),
        source_actor_id: ActorId::try_new(
            row.try_get::<String, _>("source_actor_id")
                .map_err(event_database_error)?,
        )
        .map_err(|_| event_stored_value_invalid("source actor id is invalid"))?,
        event_type: EventType::try_new(
            row.try_get::<String, _>("event_type")
                .map_err(event_database_error)?,
        )
        .map_err(|_| event_stored_value_invalid("event type is invalid"))?,
        event_version: event_version.clone(),
        aggregate: RecordRef {
            record_type: RecordType::try_new(
                row.try_get::<String, _>("aggregate_type")
                    .map_err(event_database_error)?,
            )
            .map_err(|_| event_stored_value_invalid("aggregate type is invalid"))?,
            record_id: RecordId::try_new(
                row.try_get::<String, _>("aggregate_id")
                    .map_err(event_database_error)?,
            )
            .map_err(|_| event_stored_value_invalid("aggregate id is invalid"))?,
        },
        aggregate_version: row
            .try_get("aggregate_version")
            .map_err(event_database_error)?,
        occurred_at_unix_nanos: row
            .try_get("occurred_at_unix_nanos")
            .map_err(event_database_error)?,
        correlation_id: CorrelationId::try_new(
            row.try_get::<String, _>("correlation_id")
                .map_err(event_database_error)?,
        )
        .map_err(|_| event_stored_value_invalid("correlation id is invalid"))?,
        trace_id: TraceId::try_new(
            row.try_get::<String, _>("trace_id")
                .map_err(event_database_error)?,
        )
        .map_err(|_| event_stored_value_invalid("trace id is invalid"))?,
        payload: TypedPayload {
            owner: source_module_id,
            schema_id: SchemaId::try_new(
                row.try_get::<String, _>("schema_id")
                    .map_err(event_database_error)?,
            )
            .map_err(|_| event_stored_value_invalid("schema id is invalid"))?,
            schema_version: SchemaVersion::try_new(
                row.try_get::<String, _>("schema_version")
                    .map_err(event_database_error)?,
            )
            .map_err(|_| event_stored_value_invalid("schema version is invalid"))?,
            descriptor_hash,
            data_class: parse_data_class(
                row.try_get::<String, _>("data_class")
                    .map_err(event_database_error)?,
            )?,
            encoding: parse_payload_encoding(
                row.try_get::<String, _>("payload_encoding")
                    .map_err(event_database_error)?,
            )?,
            maximum_size_bytes,
            retention_policy_id: RetentionPolicyId::try_new(
                row.try_get::<String, _>("retention_policy_id")
                    .map_err(event_database_error)?,
            )
            .map_err(|_| event_stored_value_invalid("retention policy id is invalid"))?,
            bytes: row.try_get("payload_bytes").map_err(event_database_error)?,
        },
    };
    delivery.validate()?;
    if delivery.event_version.as_str() != event_version.as_str() {
        return Err(event_stored_value_invalid(
            "event version changed during decode",
        ));
    }
    Ok(delivery)
}

fn parse_data_class(value: String) -> Result<DataClass, SdkError> {
    match value.as_str() {
        "public" => Ok(DataClass::Public),
        "internal" => Ok(DataClass::Internal),
        "confidential" => Ok(DataClass::Confidential),
        "restricted" => Ok(DataClass::Restricted),
        "personal" => Ok(DataClass::Personal),
        "sensitive_personal" => Ok(DataClass::SensitivePersonal),
        "biometric" => Ok(DataClass::Biometric),
        "financial" => Ok(DataClass::Financial),
        "credential" => Ok(DataClass::Credential),
        _ => Err(event_stored_value_invalid("event data class is unknown")),
    }
}

fn parse_payload_encoding(value: String) -> Result<PayloadEncoding, SdkError> {
    match value.as_str() {
        "protobuf" => Ok(PayloadEncoding::Protobuf),
        "json" => Ok(PayloadEncoding::Json),
        "utf8_text" => Ok(PayloadEncoding::Utf8Text),
        "binary" => Ok(PayloadEncoding::Binary),
        _ => Err(event_stored_value_invalid(
            "event payload encoding is unknown",
        )),
    }
}

fn event_stored_value_invalid(message: impl Into<String>) -> SdkError {
    SdkError::new(
        "EVENT_DELIVERY_STORED_VALUE_INVALID",
        ErrorCategory::Internal,
        false,
        "The stored source event is invalid.",
    )
    .with_internal_reference(message.into())
}

fn event_database_error(error: sqlx::Error) -> SdkError {
    SdkError::new(
        "EVENT_DELIVERY_UNAVAILABLE",
        ErrorCategory::Unavailable,
        true,
        "The event delivery service is temporarily unavailable.",
    )
    .with_internal_reference(error.to_string())
}
