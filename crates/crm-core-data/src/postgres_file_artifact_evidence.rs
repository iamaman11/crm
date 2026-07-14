use crate::{BatchError, EventEvidence};
use crm_module_sdk::{DataClass, ModuleExecutionContext, PayloadEncoding};
use sqlx::{Postgres, Transaction};

pub(crate) async fn insert_file_artifact_outbox_event(
    transaction: &mut Transaction<'_, Postgres>,
    context: &ModuleExecutionContext,
    evidence: &EventEvidence,
) -> Result<(), BatchError> {
    let maximum_size = i64::try_from(evidence.event.payload.maximum_size_bytes).map_err(|_| {
        BatchError::InvalidPlan("file artifact event maximum payload size is too large".to_owned())
    })?;
    sqlx::query(
        r#"
        INSERT INTO crm.outbox_events (
          tenant_id, event_id, business_transaction_id,
          aggregate_type, aggregate_id, aggregate_version, event_sequence,
          event_type, deduplication_key, schema_id, schema_version, descriptor_hash,
          data_class, payload_encoding, maximum_payload_size, retention_policy_id,
          payload_bytes, occurred_at
        )
        VALUES (
          $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17,
          TIMESTAMPTZ 'epoch' + ($18::bigint / 1000) * INTERVAL '1 microsecond'
        )
        "#,
    )
    .bind(context.execution.tenant_id.as_str())
    .bind(&evidence.event_id)
    .bind(context.execution.business_transaction_id.as_str())
    .bind(evidence.event.aggregate.record_type.as_str())
    .bind(evidence.event.aggregate.record_id.as_str())
    .bind(evidence.aggregate_version)
    .bind(evidence.event_sequence)
    .bind(evidence.event.event_type.as_str())
    .bind(&evidence.event.deduplication_key)
    .bind(evidence.event.payload.schema_id.as_str())
    .bind(evidence.event.payload.schema_version.as_str())
    .bind(evidence.event.payload.descriptor_hash.as_slice())
    .bind(data_class_name(evidence.event.payload.data_class))
    .bind(payload_encoding_name(evidence.event.payload.encoding))
    .bind(maximum_size)
    .bind(evidence.event.payload.retention_policy_id.as_str())
    .bind(evidence.event.payload.bytes.as_slice())
    .bind(evidence.occurred_at_unix_nanos)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

fn data_class_name(value: DataClass) -> &'static str {
    match value {
        DataClass::Public => "public",
        DataClass::Internal => "internal",
        DataClass::Confidential => "confidential",
        DataClass::Restricted => "restricted",
        DataClass::Personal => "personal",
        DataClass::SensitivePersonal => "sensitive_personal",
        DataClass::Biometric => "biometric",
        DataClass::Financial => "financial",
        DataClass::Credential => "credential",
    }
}

fn payload_encoding_name(value: PayloadEncoding) -> &'static str {
    match value {
        PayloadEncoding::Protobuf => "protobuf",
        PayloadEncoding::Json => "json",
        PayloadEncoding::Utf8Text => "utf8_text",
        PayloadEncoding::Binary => "binary",
    }
}
