use crate::PostgresDataStore;
use crm_core_events::{ModuleActivationReader, ModuleActivationState};
use crm_module_sdk::{
    DataClass, ErrorCategory, ModuleExecutionContext, ModuleId, ModuleStateEntry, ModuleStateStore,
    PayloadEncoding, PortFuture, PortResult, PutModuleStateRequest, RetentionPolicyId, SchemaId,
    SchemaVersion, SdkError, StateKey, TenantId, TypedPayload,
};
use sqlx::Row;

/// Durable host implementation for module-private operational state and lifecycle reads.
///
/// Business modules still see only the `ModuleStateStore` SDK port. PostgreSQL, RLS and
/// installation tables remain host-runtime implementation details.
#[derive(Debug, Clone)]
pub struct PostgresModuleRuntimeStore {
    store: PostgresDataStore,
}

impl PostgresModuleRuntimeStore {
    pub fn new(store: PostgresDataStore) -> Self {
        Self { store }
    }
}

impl ModuleActivationReader for PostgresModuleRuntimeStore {
    fn activation_state<'a>(
        &'a self,
        tenant_id: &'a TenantId,
        module_id: &'a ModuleId,
    ) -> PortFuture<'a, PortResult<ModuleActivationState>> {
        Box::pin(async move {
            let mut transaction = self
                .store
                .pool()
                .begin()
                .await
                .map_err(state_database_error)?;
            bind_tenant_read_context(&mut transaction, tenant_id).await?;
            let row = sqlx::query(
                r#"
                SELECT status
                FROM crm.module_installations
                WHERE tenant_id = $1 AND module_id = $2
                "#,
            )
            .bind(tenant_id.as_str())
            .bind(module_id.as_str())
            .fetch_optional(&mut *transaction)
            .await
            .map_err(state_database_error)?;
            transaction.commit().await.map_err(state_database_error)?;

            Ok(match row {
                None => ModuleActivationState::Missing,
                Some(row)
                    if row
                        .try_get::<String, _>("status")
                        .map_err(state_database_error)?
                        == "active" =>
                {
                    ModuleActivationState::Active
                }
                Some(_) => ModuleActivationState::Inactive,
            })
        })
    }
}

impl ModuleStateStore for PostgresModuleRuntimeStore {
    fn get<'a>(
        &'a self,
        context: &'a ModuleExecutionContext,
        key: StateKey,
    ) -> PortFuture<'a, PortResult<Option<ModuleStateEntry>>> {
        Box::pin(async move {
            context.validate()?;
            let mut transaction = self
                .store
                .pool()
                .begin()
                .await
                .map_err(state_database_error)?;
            crate::postgres_batch::bind_execution_context(&mut transaction, context)
                .await
                .map_err(crate::postgres_batch::batch_error_to_sdk)?;
            let row = sqlx::query(
                r#"
                SELECT
                  version,
                  schema_id,
                  schema_version,
                  descriptor_hash,
                  data_class,
                  payload_encoding,
                  maximum_payload_size,
                  retention_policy_id,
                  payload_bytes
                FROM crm.module_state
                WHERE tenant_id = $1
                  AND module_id = $2
                  AND state_key = $3
                "#,
            )
            .bind(context.execution.tenant_id.as_str())
            .bind(context.module_id.as_str())
            .bind(key.as_str())
            .fetch_optional(&mut *transaction)
            .await
            .map_err(state_database_error)?;
            transaction.commit().await.map_err(state_database_error)?;
            row.map(|row| decode_state_entry(context, key, row))
                .transpose()
        })
    }

    fn put<'a>(
        &'a self,
        context: &'a ModuleExecutionContext,
        request: PutModuleStateRequest,
    ) -> PortFuture<'a, PortResult<ModuleStateEntry>> {
        Box::pin(async move {
            context.validate()?;
            request.value.validate()?;
            if request.value.owner != context.module_id {
                return Err(SdkError::new(
                    "SDK_STATE_OWNER_MISMATCH",
                    ErrorCategory::Authorization,
                    false,
                    "A module may write only payloads it owns.",
                ));
            }
            if request.expected_version.is_some_and(|version| version <= 0) {
                return Err(SdkError::invalid_argument(
                    "module_state.expected_version",
                    "expected version must be greater than zero",
                ));
            }
            let maximum_size = i64::try_from(request.value.maximum_size_bytes).map_err(|_| {
                SdkError::invalid_argument(
                    "module_state.maximum_size_bytes",
                    "maximum payload size exceeds the PostgreSQL representation",
                )
            })?;

            let mut transaction = self
                .store
                .pool()
                .begin()
                .await
                .map_err(state_database_error)?;
            crate::postgres_batch::bind_execution_context(&mut transaction, context)
                .await
                .map_err(crate::postgres_batch::batch_error_to_sdk)?;

            let version = match request.expected_version {
                None => sqlx::query(
                    r#"
                    INSERT INTO crm.module_state (
                      tenant_id, module_id, state_key, version,
                      schema_id, schema_version, descriptor_hash, data_class,
                      payload_encoding, maximum_payload_size, retention_policy_id,
                      payload_bytes, last_business_transaction_id
                    )
                    VALUES ($1, $2, $3, 1, $4, $5, $6, $7, $8, $9, $10, $11, $12)
                    ON CONFLICT (tenant_id, module_id, state_key) DO NOTHING
                    RETURNING version
                    "#,
                )
                .bind(context.execution.tenant_id.as_str())
                .bind(context.module_id.as_str())
                .bind(request.key.as_str())
                .bind(request.value.schema_id.as_str())
                .bind(request.value.schema_version.as_str())
                .bind(request.value.descriptor_hash.as_slice())
                .bind(data_class_name(request.value.data_class))
                .bind(payload_encoding_name(request.value.encoding))
                .bind(maximum_size)
                .bind(request.value.retention_policy_id.as_str())
                .bind(request.value.bytes.as_slice())
                .bind(context.execution.business_transaction_id.as_str())
                .fetch_optional(&mut *transaction)
                .await
                .map_err(state_database_error)?
                .map(|row| row.try_get::<i64, _>("version"))
                .transpose()
                .map_err(state_database_error)?
                .ok_or_else(|| version_conflict("module state entry already exists"))?,
                Some(expected_version) => sqlx::query(
                    r#"
                    UPDATE crm.module_state
                       SET version = version + 1,
                           schema_id = $5,
                           schema_version = $6,
                           descriptor_hash = $7,
                           data_class = $8,
                           payload_encoding = $9,
                           maximum_payload_size = $10,
                           retention_policy_id = $11,
                           payload_bytes = $12,
                           last_business_transaction_id = $13,
                           updated_at = clock_timestamp()
                     WHERE tenant_id = $1
                       AND module_id = $2
                       AND state_key = $3
                       AND version = $4
                    RETURNING version
                    "#,
                )
                .bind(context.execution.tenant_id.as_str())
                .bind(context.module_id.as_str())
                .bind(request.key.as_str())
                .bind(expected_version)
                .bind(request.value.schema_id.as_str())
                .bind(request.value.schema_version.as_str())
                .bind(request.value.descriptor_hash.as_slice())
                .bind(data_class_name(request.value.data_class))
                .bind(payload_encoding_name(request.value.encoding))
                .bind(maximum_size)
                .bind(request.value.retention_policy_id.as_str())
                .bind(request.value.bytes.as_slice())
                .bind(context.execution.business_transaction_id.as_str())
                .fetch_optional(&mut *transaction)
                .await
                .map_err(state_database_error)?
                .map(|row| row.try_get::<i64, _>("version"))
                .transpose()
                .map_err(state_database_error)?
                .ok_or_else(|| {
                    version_conflict(format!(
                        "module state expected version {expected_version} does not match"
                    ))
                })?,
            };

            transaction.commit().await.map_err(state_database_error)?;
            Ok(ModuleStateEntry {
                key: request.key,
                version,
                value: request.value,
            })
        })
    }

    fn delete<'a>(
        &'a self,
        context: &'a ModuleExecutionContext,
        key: StateKey,
        expected_version: Option<i64>,
    ) -> PortFuture<'a, PortResult<()>> {
        Box::pin(async move {
            context.validate()?;
            let mut transaction = self
                .store
                .pool()
                .begin()
                .await
                .map_err(state_database_error)?;
            crate::postgres_batch::bind_execution_context(&mut transaction, context)
                .await
                .map_err(crate::postgres_batch::batch_error_to_sdk)?;

            let row = sqlx::query(
                r#"
                SELECT version
                FROM crm.module_state
                WHERE tenant_id = $1 AND module_id = $2 AND state_key = $3
                FOR UPDATE
                "#,
            )
            .bind(context.execution.tenant_id.as_str())
            .bind(context.module_id.as_str())
            .bind(key.as_str())
            .fetch_optional(&mut *transaction)
            .await
            .map_err(state_database_error)?;
            let Some(row) = row else {
                transaction.commit().await.map_err(state_database_error)?;
                return Ok(());
            };
            let current_version: i64 = row.try_get("version").map_err(state_database_error)?;
            if expected_version.is_some_and(|expected| expected != current_version) {
                return Err(version_conflict(format!(
                    "expected version {}, found {current_version}",
                    expected_version.expect("checked as Some")
                )));
            }

            sqlx::query(
                r#"
                UPDATE crm.module_state
                   SET last_business_transaction_id = $4,
                       updated_at = clock_timestamp()
                 WHERE tenant_id = $1 AND module_id = $2 AND state_key = $3
                "#,
            )
            .bind(context.execution.tenant_id.as_str())
            .bind(context.module_id.as_str())
            .bind(key.as_str())
            .bind(context.execution.business_transaction_id.as_str())
            .execute(&mut *transaction)
            .await
            .map_err(state_database_error)?;
            sqlx::query(
                "DELETE FROM crm.module_state WHERE tenant_id = $1 AND module_id = $2 AND state_key = $3",
            )
            .bind(context.execution.tenant_id.as_str())
            .bind(context.module_id.as_str())
            .bind(key.as_str())
            .execute(&mut *transaction)
            .await
            .map_err(state_database_error)?;
            transaction.commit().await.map_err(state_database_error)?;
            Ok(())
        })
    }
}

async fn bind_tenant_read_context(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    tenant_id: &TenantId,
) -> Result<(), SdkError> {
    sqlx::query("SELECT set_config('app.tenant_id', $1, true)")
        .bind(tenant_id.as_str())
        .execute(&mut **transaction)
        .await
        .map_err(state_database_error)?;
    Ok(())
}

fn decode_state_entry(
    context: &ModuleExecutionContext,
    key: StateKey,
    row: sqlx::postgres::PgRow,
) -> Result<ModuleStateEntry, SdkError> {
    let descriptor_hash: Vec<u8> = row
        .try_get("descriptor_hash")
        .map_err(state_database_error)?;
    let descriptor_hash: [u8; 32] = descriptor_hash
        .try_into()
        .map_err(|_| state_corrupt("module state descriptor hash is not 32 bytes"))?;
    let maximum_size: i64 = row
        .try_get("maximum_payload_size")
        .map_err(state_database_error)?;
    let maximum_size_bytes = u64::try_from(maximum_size)
        .map_err(|_| state_corrupt("module state maximum payload size is negative"))?;
    let payload = TypedPayload {
        owner: context.module_id.clone(),
        schema_id: SchemaId::try_new(
            row.try_get::<String, _>("schema_id")
                .map_err(state_database_error)?,
        )
        .map_err(|_| state_corrupt("module state schema id is invalid"))?,
        schema_version: SchemaVersion::try_new(
            row.try_get::<String, _>("schema_version")
                .map_err(state_database_error)?,
        )
        .map_err(|_| state_corrupt("module state schema version is invalid"))?,
        descriptor_hash,
        data_class: parse_data_class(
            row.try_get::<String, _>("data_class")
                .map_err(state_database_error)?,
        )?,
        encoding: parse_payload_encoding(
            row.try_get::<String, _>("payload_encoding")
                .map_err(state_database_error)?,
        )?,
        maximum_size_bytes,
        retention_policy_id: RetentionPolicyId::try_new(
            row.try_get::<String, _>("retention_policy_id")
                .map_err(state_database_error)?,
        )
        .map_err(|_| state_corrupt("module state retention policy id is invalid"))?,
        bytes: row.try_get("payload_bytes").map_err(state_database_error)?,
    };
    payload.validate()?;
    Ok(ModuleStateEntry {
        key,
        version: row.try_get("version").map_err(state_database_error)?,
        value: payload,
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
        _ => Err(state_corrupt("module state data class is unknown")),
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

fn parse_payload_encoding(value: String) -> Result<PayloadEncoding, SdkError> {
    match value.as_str() {
        "protobuf" => Ok(PayloadEncoding::Protobuf),
        "json" => Ok(PayloadEncoding::Json),
        "utf8_text" => Ok(PayloadEncoding::Utf8Text),
        "binary" => Ok(PayloadEncoding::Binary),
        _ => Err(state_corrupt("module state payload encoding is unknown")),
    }
}

fn version_conflict(message: impl Into<String>) -> SdkError {
    SdkError::new(
        "SDK_VERSION_CONFLICT",
        ErrorCategory::Conflict,
        false,
        message,
    )
}

fn state_corrupt(message: impl Into<String>) -> SdkError {
    SdkError::new(
        "SDK_STATE_CORRUPT",
        ErrorCategory::Internal,
        false,
        "The module state is invalid.",
    )
    .with_internal_reference(message.into())
}

fn state_database_error(error: sqlx::Error) -> SdkError {
    SdkError::new(
        "SDK_STATE_UNAVAILABLE",
        ErrorCategory::Unavailable,
        true,
        "The module state service is temporarily unavailable.",
    )
    .with_internal_reference(error.to_string())
}
