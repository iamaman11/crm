use crm_module_sdk::{
    DataClass, DomainEvent, ErrorCategory, ModuleExecutionContext, ModuleId, PayloadEncoding,
    RecordId, RecordRef, RecordSnapshot, RecordType, RetentionPolicyId, SchemaId, SchemaVersion,
    SdkError, TypedPayload,
};
use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Postgres, Row, Transaction};
use std::error::Error;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdempotencyEvidence {
    pub scope: String,
    pub key: String,
    pub request_hash: [u8; 32],
    pub expires_at_unix_nanos: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditEvidence {
    pub audit_sequence: i64,
    pub audit_record_id: String,
    pub canonicalization_profile: String,
    pub previous_hash: [u8; 32],
    pub record_hash: [u8; 32],
    pub canonical_envelope: Vec<u8>,
    pub occurred_at_unix_nanos: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordCreatePlan {
    pub context: ModuleExecutionContext,
    pub record: RecordRef,
    pub record_payload: TypedPayload,
    pub event_id: String,
    pub event: DomainEvent,
    pub idempotency: IdempotencyEvidence,
    pub audit: AuditEvidence,
}

impl RecordCreatePlan {
    pub fn validate(&self) -> Result<(), DataError> {
        self.context.validate().map_err(DataError::Sdk)?;
        self.record_payload.validate().map_err(DataError::Sdk)?;
        self.event.payload.validate().map_err(DataError::Sdk)?;

        if self.event.aggregate != self.record {
            return Err(DataError::InvalidPlan(
                "event aggregate must match the created record".to_owned(),
            ));
        }
        if self.event_id.is_empty()
            || self.idempotency.scope.is_empty()
            || self.idempotency.key.is_empty()
            || self.audit.audit_record_id.is_empty()
            || self.audit.canonicalization_profile.is_empty()
        {
            return Err(DataError::InvalidPlan(
                "event, idempotency and audit identifiers must not be empty".to_owned(),
            ));
        }
        if self.audit.audit_sequence <= 0 {
            return Err(DataError::InvalidPlan(
                "audit sequence must be positive".to_owned(),
            ));
        }
        if self.idempotency.request_hash.iter().all(|byte| *byte == 0)
            || self.audit.record_hash.iter().all(|byte| *byte == 0)
        {
            return Err(DataError::InvalidPlan(
                "request and audit record hashes must not be all zeroes".to_owned(),
            ));
        }
        if self.context.module_id != self.record_payload.owner {
            return Err(DataError::InvalidPlan(
                "record payload owner must match the executing module".to_owned(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaultInjection {
    None,
    OmitIdempotency,
    OmitOutbox,
    OmitAudit,
    OmitCompletionMarker,
}

#[derive(Debug)]
pub enum DataError {
    Database(sqlx::Error),
    Sdk(SdkError),
    InvalidPlan(String),
    InvalidStoredValue(String),
}

impl fmt::Display for DataError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Database(error) => write!(formatter, "database operation failed: {error}"),
            Self::Sdk(error) => write!(formatter, "SDK validation failed: {error}"),
            Self::InvalidPlan(message) => write!(formatter, "invalid mutation plan: {message}"),
            Self::InvalidStoredValue(message) => {
                write!(formatter, "invalid value stored in PostgreSQL: {message}")
            }
        }
    }
}

impl Error for DataError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Database(error) => Some(error),
            Self::Sdk(error) => Some(error),
            Self::InvalidPlan(_) | Self::InvalidStoredValue(_) => None,
        }
    }
}

impl From<sqlx::Error> for DataError {
    fn from(value: sqlx::Error) -> Self {
        Self::Database(value)
    }
}

#[derive(Debug, Clone)]
pub struct PostgresDataStore {
    pool: PgPool,
}

impl PostgresDataStore {
    pub async fn connect(database_url: &str, maximum_connections: u32) -> Result<Self, DataError> {
        let pool = PgPoolOptions::new()
            .max_connections(maximum_connections)
            .connect(database_url)
            .await?;
        Ok(Self { pool })
    }

    pub const fn from_pool(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub async fn create_record(
        &self,
        plan: &RecordCreatePlan,
    ) -> Result<RecordSnapshot, DataError> {
        self.create_record_with_fault(plan, FaultInjection::None)
            .await
    }

    #[doc(hidden)]
    pub async fn create_record_with_fault(
        &self,
        plan: &RecordCreatePlan,
        fault: FaultInjection,
    ) -> Result<RecordSnapshot, DataError> {
        plan.validate()?;
        let mut transaction = self.pool.begin().await?;
        bind_execution_context(&mut transaction, &plan.context).await?;

        let snapshot = insert_record(&mut transaction, plan).await?;
        if fault != FaultInjection::OmitIdempotency {
            insert_idempotency(&mut transaction, plan).await?;
        }
        if fault != FaultInjection::OmitOutbox {
            insert_outbox_event(&mut transaction, plan).await?;
        }
        if fault != FaultInjection::OmitAudit {
            insert_audit_record(&mut transaction, plan).await?;
        }
        if fault != FaultInjection::OmitCompletionMarker {
            insert_completion_marker(&mut transaction, plan).await?;
        }

        transaction.commit().await?;
        Ok(snapshot)
    }

    pub async fn get_record(
        &self,
        context: &ModuleExecutionContext,
        reference: &RecordRef,
    ) -> Result<Option<RecordSnapshot>, DataError> {
        context.validate().map_err(DataError::Sdk)?;
        let mut transaction = self.pool.begin().await?;
        bind_execution_context(&mut transaction, context).await?;

        let row = sqlx::query(
            r#"
            SELECT
              version,
              owner_module_id,
              schema_id,
              schema_version,
              descriptor_hash,
              data_class,
              payload_encoding,
              maximum_payload_size,
              retention_policy_id,
              payload_bytes
            FROM crm.records
            WHERE tenant_id = $1
              AND record_type = $2
              AND record_id = $3
              AND deleted_at IS NULL
            "#,
        )
        .bind(context.execution.tenant_id.as_str())
        .bind(reference.record_type.as_str())
        .bind(reference.record_id.as_str())
        .fetch_optional(&mut *transaction)
        .await?;

        transaction.commit().await?;
        row.map(|row| decode_record(reference.clone(), row))
            .transpose()
    }
}

async fn bind_execution_context(
    transaction: &mut Transaction<'_, Postgres>,
    context: &ModuleExecutionContext,
) -> Result<(), DataError> {
    sqlx::query(
        r#"
        SELECT
          set_config('app.tenant_id', $1, true),
          set_config('app.actor_id', $2, true),
          set_config('app.request_id', $3, true),
          set_config('app.capability_id', $4, true),
          set_config('app.capability_version', $5, true),
          set_config('app.business_transaction_id', $6, true)
        "#,
    )
    .bind(context.execution.tenant_id.as_str())
    .bind(context.execution.actor_id.as_str())
    .bind(context.execution.request_id.as_str())
    .bind(context.execution.capability_id.as_str())
    .bind(context.execution.capability_version.as_str())
    .bind(context.execution.business_transaction_id.as_str())
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

async fn insert_record(
    transaction: &mut Transaction<'_, Postgres>,
    plan: &RecordCreatePlan,
) -> Result<RecordSnapshot, DataError> {
    let maximum_size = i64::try_from(plan.record_payload.maximum_size_bytes)
        .map_err(|_| DataError::InvalidPlan("record payload size exceeds i64".to_owned()))?;
    sqlx::query(
        r#"
        INSERT INTO crm.records (
          tenant_id,
          record_type,
          record_id,
          version,
          owner_module_id,
          schema_id,
          schema_version,
          descriptor_hash,
          data_class,
          payload_encoding,
          maximum_payload_size,
          retention_policy_id,
          payload_bytes,
          last_business_transaction_id
        )
        VALUES ($1, $2, $3, 1, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
        "#,
    )
    .bind(plan.context.execution.tenant_id.as_str())
    .bind(plan.record.record_type.as_str())
    .bind(plan.record.record_id.as_str())
    .bind(plan.record_payload.owner.as_str())
    .bind(plan.record_payload.schema_id.as_str())
    .bind(plan.record_payload.schema_version.as_str())
    .bind(plan.record_payload.descriptor_hash.as_slice())
    .bind(data_class_name(plan.record_payload.data_class))
    .bind(payload_encoding_name(plan.record_payload.encoding))
    .bind(maximum_size)
    .bind(plan.record_payload.retention_policy_id.as_str())
    .bind(plan.record_payload.bytes.as_slice())
    .bind(plan.context.execution.business_transaction_id.as_str())
    .execute(&mut **transaction)
    .await?;

    Ok(RecordSnapshot {
        reference: plan.record.clone(),
        version: 1,
        payload: plan.record_payload.clone(),
    })
}

async fn insert_idempotency(
    transaction: &mut Transaction<'_, Postgres>,
    plan: &RecordCreatePlan,
) -> Result<(), DataError> {
    sqlx::query(
        r#"
        INSERT INTO crm.idempotency_records (
          tenant_id,
          idempotency_scope,
          idempotency_key,
          request_hash,
          status,
          business_transaction_id,
          expires_at
        )
        VALUES (
          $1, $2, $3, $4, 'completed', $5,
          TIMESTAMPTZ 'epoch' + ($6::bigint / 1000) * INTERVAL '1 microsecond'
        )
        "#,
    )
    .bind(plan.context.execution.tenant_id.as_str())
    .bind(&plan.idempotency.scope)
    .bind(&plan.idempotency.key)
    .bind(plan.idempotency.request_hash.as_slice())
    .bind(plan.context.execution.business_transaction_id.as_str())
    .bind(plan.idempotency.expires_at_unix_nanos)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

async fn insert_outbox_event(
    transaction: &mut Transaction<'_, Postgres>,
    plan: &RecordCreatePlan,
) -> Result<(), DataError> {
    let maximum_size = i64::try_from(plan.event.payload.maximum_size_bytes)
        .map_err(|_| DataError::InvalidPlan("event payload size exceeds i64".to_owned()))?;
    sqlx::query(
        r#"
        INSERT INTO crm.outbox_events (
          tenant_id,
          event_id,
          business_transaction_id,
          aggregate_type,
          aggregate_id,
          aggregate_version,
          event_sequence,
          event_type,
          deduplication_key,
          schema_id,
          schema_version,
          descriptor_hash,
          data_class,
          payload_encoding,
          maximum_payload_size,
          retention_policy_id,
          payload_bytes,
          occurred_at
        )
        VALUES (
          $1, $2, $3, $4, $5, 1, 1, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15,
          TIMESTAMPTZ 'epoch' + ($16::bigint / 1000) * INTERVAL '1 microsecond'
        )
        "#,
    )
    .bind(plan.context.execution.tenant_id.as_str())
    .bind(&plan.event_id)
    .bind(plan.context.execution.business_transaction_id.as_str())
    .bind(plan.event.aggregate.record_type.as_str())
    .bind(plan.event.aggregate.record_id.as_str())
    .bind(plan.event.event_type.as_str())
    .bind(&plan.event.deduplication_key)
    .bind(plan.event.payload.schema_id.as_str())
    .bind(plan.event.payload.schema_version.as_str())
    .bind(plan.event.payload.descriptor_hash.as_slice())
    .bind(data_class_name(plan.event.payload.data_class))
    .bind(payload_encoding_name(plan.event.payload.encoding))
    .bind(maximum_size)
    .bind(plan.event.payload.retention_policy_id.as_str())
    .bind(plan.event.payload.bytes.as_slice())
    .bind(plan.audit.occurred_at_unix_nanos)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

async fn insert_audit_record(
    transaction: &mut Transaction<'_, Postgres>,
    plan: &RecordCreatePlan,
) -> Result<(), DataError> {
    sqlx::query(
        r#"
        INSERT INTO crm.audit_records (
          tenant_id,
          audit_sequence,
          audit_record_id,
          business_transaction_id,
          actor_id,
          capability_id,
          capability_version,
          canonicalization_profile,
          previous_hash,
          record_hash,
          canonical_envelope,
          occurred_at
        )
        VALUES (
          $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11,
          TIMESTAMPTZ 'epoch' + ($12::bigint / 1000) * INTERVAL '1 microsecond'
        )
        "#,
    )
    .bind(plan.context.execution.tenant_id.as_str())
    .bind(plan.audit.audit_sequence)
    .bind(&plan.audit.audit_record_id)
    .bind(plan.context.execution.business_transaction_id.as_str())
    .bind(plan.context.execution.actor_id.as_str())
    .bind(plan.context.execution.capability_id.as_str())
    .bind(plan.context.execution.capability_version.as_str())
    .bind(&plan.audit.canonicalization_profile)
    .bind(plan.audit.previous_hash.as_slice())
    .bind(plan.audit.record_hash.as_slice())
    .bind(plan.audit.canonical_envelope.as_slice())
    .bind(plan.audit.occurred_at_unix_nanos)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

async fn insert_completion_marker(
    transaction: &mut Transaction<'_, Postgres>,
    plan: &RecordCreatePlan,
) -> Result<(), DataError> {
    sqlx::query(
        r#"
        INSERT INTO crm.business_transactions (
          tenant_id,
          business_transaction_id,
          actor_id,
          request_id,
          capability_id,
          capability_version,
          expected_outbox_events,
          expected_audit_records,
          expected_idempotency_records
        )
        VALUES ($1, $2, $3, $4, $5, $6, 1, 1, 1)
        "#,
    )
    .bind(plan.context.execution.tenant_id.as_str())
    .bind(plan.context.execution.business_transaction_id.as_str())
    .bind(plan.context.execution.actor_id.as_str())
    .bind(plan.context.execution.request_id.as_str())
    .bind(plan.context.execution.capability_id.as_str())
    .bind(plan.context.execution.capability_version.as_str())
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

fn decode_record(
    reference: RecordRef,
    row: sqlx::postgres::PgRow,
) -> Result<RecordSnapshot, DataError> {
    let owner = ModuleId::try_new(row.try_get::<String, _>("owner_module_id")?)
        .map_err(|error| DataError::InvalidStoredValue(error.to_string()))?;
    let schema_id = SchemaId::try_new(row.try_get::<String, _>("schema_id")?)
        .map_err(|error| DataError::InvalidStoredValue(error.to_string()))?;
    let schema_version = SchemaVersion::try_new(row.try_get::<String, _>("schema_version")?)
        .map_err(|error| DataError::InvalidStoredValue(error.to_string()))?;
    let retention_policy_id =
        RetentionPolicyId::try_new(row.try_get::<String, _>("retention_policy_id")?)
            .map_err(|error| DataError::InvalidStoredValue(error.to_string()))?;
    let descriptor_hash: Vec<u8> = row.try_get("descriptor_hash")?;
    let descriptor_hash: [u8; 32] = descriptor_hash
        .try_into()
        .map_err(|_| DataError::InvalidStoredValue("descriptor hash is not 32 bytes".to_owned()))?;
    let maximum_payload_size: i64 = row.try_get("maximum_payload_size")?;
    let maximum_payload_size = u64::try_from(maximum_payload_size)
        .map_err(|_| DataError::InvalidStoredValue("negative maximum payload size".to_owned()))?;

    Ok(RecordSnapshot {
        reference,
        version: row.try_get("version")?,
        payload: TypedPayload {
            owner,
            schema_id,
            schema_version,
            descriptor_hash,
            data_class: parse_data_class(row.try_get("data_class")?)?,
            encoding: parse_payload_encoding(row.try_get("payload_encoding")?)?,
            maximum_size_bytes: maximum_payload_size,
            retention_policy_id,
            bytes: row.try_get("payload_bytes")?,
        },
    })
}

const fn data_class_name(value: DataClass) -> &'static str {
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

fn parse_data_class(value: String) -> Result<DataClass, DataError> {
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
        _ => Err(DataError::InvalidStoredValue(format!(
            "unknown data class {value}"
        ))),
    }
}

const fn payload_encoding_name(value: PayloadEncoding) -> &'static str {
    match value {
        PayloadEncoding::Protobuf => "protobuf",
        PayloadEncoding::Json => "json",
        PayloadEncoding::Utf8Text => "utf8_text",
        PayloadEncoding::Binary => "binary",
    }
}

fn parse_payload_encoding(value: String) -> Result<PayloadEncoding, DataError> {
    match value.as_str() {
        "protobuf" => Ok(PayloadEncoding::Protobuf),
        "json" => Ok(PayloadEncoding::Json),
        "utf8_text" => Ok(PayloadEncoding::Utf8Text),
        "binary" => Ok(PayloadEncoding::Binary),
        _ => Err(DataError::InvalidStoredValue(format!(
            "unknown payload encoding {value}"
        ))),
    }
}

pub fn database_error_to_sdk(error: DataError) -> SdkError {
    match error {
        DataError::Sdk(error) => error,
        DataError::InvalidPlan(message) | DataError::InvalidStoredValue(message) => SdkError::new(
            "DATA_INVALID",
            ErrorCategory::InvalidArgument,
            false,
            message,
        ),
        DataError::Database(error) => SdkError::new(
            "DATA_UNAVAILABLE",
            ErrorCategory::Unavailable,
            true,
            "The data service is temporarily unavailable.",
        )
        .with_internal_reference(error.to_string()),
    }
}
