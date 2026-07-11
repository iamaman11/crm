use crate::postgres::PostgresDataStore;
use crate::postgres_batch::{BatchError, parse_data_class, parse_payload_encoding};
use crm_module_sdk::{
    ErrorCategory, ModuleId, RecordId, RecordRef, RecordSnapshot, RecordType, RetentionPolicyId,
    SchemaId, SchemaVersion, SdkError, TenantId, TypedPayload,
};
use sqlx::{Postgres, Row, Transaction, postgres::PgRow};

pub const MAXIMUM_RECORD_QUERY_PAGE_SIZE: u32 = 1_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordQuerySort {
    CreatedAtAscending,
    UpdatedAtDescending,
}

impl RecordQuerySort {
    pub const fn id(self) -> &'static str {
        match self {
            Self::CreatedAtAscending => "created_at_asc_record_id_asc",
            Self::UpdatedAtDescending => "updated_at_desc_record_id_asc",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordQueryContinuation {
    pub sort_value: String,
    pub record_id: RecordId,
}

impl RecordQueryContinuation {
    pub fn validate(&self) -> Result<(), SdkError> {
        if self.sort_value.is_empty()
            || self.sort_value.len() > 32
            || self.sort_value.parse::<i64>().is_err()
        {
            return Err(invalid_query(
                "DATA_QUERY_CONTINUATION_INVALID",
                "The record query continuation is invalid.",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordGetQuery {
    pub tenant_id: TenantId,
    pub owner_module_id: ModuleId,
    pub record_type: RecordType,
    pub record_id: RecordId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordListQuery {
    pub tenant_id: TenantId,
    pub owner_module_id: ModuleId,
    pub record_type: RecordType,
    pub page_size: u32,
    pub sort: RecordQuerySort,
    pub after: Option<RecordQueryContinuation>,
}

impl RecordListQuery {
    pub fn validate(&self) -> Result<(), SdkError> {
        if self.page_size == 0 || self.page_size > MAXIMUM_RECORD_QUERY_PAGE_SIZE {
            return Err(invalid_query(
                "DATA_QUERY_PAGE_SIZE_INVALID",
                "The record query page size is invalid.",
            ));
        }
        if let Some(after) = &self.after {
            after.validate()?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordQueryPage {
    pub records: Vec<RecordSnapshot>,
    pub next: Option<RecordQueryContinuation>,
}

impl PostgresDataStore {
    pub async fn get_record_for_query(
        &self,
        query: &RecordGetQuery,
    ) -> Result<Option<RecordSnapshot>, SdkError> {
        let mut transaction = self
            .pool()
            .begin()
            .await
            .map_err(database_unavailable)?;
        bind_read_context(&mut transaction, &query.tenant_id).await?;
        let row = sqlx::query(
            r#"
            SELECT
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
              payload_bytes
            FROM crm.records
            WHERE tenant_id = $1
              AND owner_module_id = $2
              AND record_type = $3
              AND record_id = $4
              AND deleted_at IS NULL
            "#,
        )
        .bind(query.tenant_id.as_str())
        .bind(query.owner_module_id.as_str())
        .bind(query.record_type.as_str())
        .bind(query.record_id.as_str())
        .fetch_optional(&mut *transaction)
        .await
        .map_err(database_unavailable)?;
        transaction.commit().await.map_err(database_unavailable)?;

        row.map(|row| decode_query_record(&query.tenant_id, &query.record_type, row))
            .transpose()
    }

    pub async fn list_records_for_query(
        &self,
        query: &RecordListQuery,
    ) -> Result<RecordQueryPage, SdkError> {
        query.validate()?;
        let fetch_limit = i64::from(query.page_size) + 1;
        let after_sort = query.after.as_ref().map(|value| value.sort_value.as_str());
        let after_record_id = query
            .after
            .as_ref()
            .map(|value| value.record_id.as_str())
            .unwrap_or("");
        let mut transaction = self
            .pool()
            .begin()
            .await
            .map_err(database_unavailable)?;
        bind_read_context(&mut transaction, &query.tenant_id).await?;

        let rows = match query.sort {
            RecordQuerySort::CreatedAtAscending => {
                sqlx::query(
                    r#"
                    SELECT
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
                      ((EXTRACT(EPOCH FROM created_at) * 1000000)::bigint)::text AS sort_value
                    FROM crm.records
                    WHERE tenant_id = $1
                      AND owner_module_id = $2
                      AND record_type = $3
                      AND deleted_at IS NULL
                      AND (
                        $4::text IS NULL
                        OR created_at > TIMESTAMPTZ 'epoch' + ($4::bigint * INTERVAL '1 microsecond')
                        OR (
                          created_at = TIMESTAMPTZ 'epoch' + ($4::bigint * INTERVAL '1 microsecond')
                          AND record_id > $5
                        )
                      )
                    ORDER BY created_at ASC, record_id ASC
                    LIMIT $6
                    "#,
                )
                .bind(query.tenant_id.as_str())
                .bind(query.owner_module_id.as_str())
                .bind(query.record_type.as_str())
                .bind(after_sort)
                .bind(after_record_id)
                .bind(fetch_limit)
                .fetch_all(&mut *transaction)
                .await
            }
            RecordQuerySort::UpdatedAtDescending => {
                sqlx::query(
                    r#"
                    SELECT
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
                      ((EXTRACT(EPOCH FROM updated_at) * 1000000)::bigint)::text AS sort_value
                    FROM crm.records
                    WHERE tenant_id = $1
                      AND owner_module_id = $2
                      AND record_type = $3
                      AND deleted_at IS NULL
                      AND (
                        $4::text IS NULL
                        OR updated_at < TIMESTAMPTZ 'epoch' + ($4::bigint * INTERVAL '1 microsecond')
                        OR (
                          updated_at = TIMESTAMPTZ 'epoch' + ($4::bigint * INTERVAL '1 microsecond')
                          AND record_id > $5
                        )
                      )
                    ORDER BY updated_at DESC, record_id ASC
                    LIMIT $6
                    "#,
                )
                .bind(query.tenant_id.as_str())
                .bind(query.owner_module_id.as_str())
                .bind(query.record_type.as_str())
                .bind(after_sort)
                .bind(after_record_id)
                .bind(fetch_limit)
                .fetch_all(&mut *transaction)
                .await
            }
        }
        .map_err(database_unavailable)?;
        transaction.commit().await.map_err(database_unavailable)?;

        let has_more = rows.len() > query.page_size as usize;
        let mut decoded = rows
            .into_iter()
            .map(|row| {
                let sort_value: String = row
                    .try_get("sort_value")
                    .map_err(|error| stored_value_invalid(error.to_string()))?;
                let snapshot = decode_query_record(&query.tenant_id, &query.record_type, row)?;
                Ok((snapshot, sort_value))
            })
            .collect::<Result<Vec<_>, SdkError>>()?;

        if has_more {
            decoded.pop();
        }
        let next = if has_more {
            decoded
                .last()
                .map(|(snapshot, sort_value)| RecordQueryContinuation {
                    sort_value: sort_value.clone(),
                    record_id: snapshot.reference.record_id.clone(),
                })
        } else {
            None
        };
        let records = decoded
            .into_iter()
            .map(|(snapshot, _)| snapshot)
            .collect();
        Ok(RecordQueryPage { records, next })
    }
}

async fn bind_read_context(
    transaction: &mut Transaction<'_, Postgres>,
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

fn decode_query_record(
    tenant_id: &TenantId,
    record_type: &RecordType,
    row: PgRow,
) -> Result<RecordSnapshot, SdkError> {
    let descriptor_hash: Vec<u8> = row
        .try_get("descriptor_hash")
        .map_err(|error| stored_value_invalid(error.to_string()))?;
    let descriptor_hash = descriptor_hash.try_into().map_err(|_| {
        stored_value_invalid("record descriptor hash must contain exactly 32 bytes".to_owned())
    })?;
    let maximum_payload_size: i64 = row
        .try_get("maximum_payload_size")
        .map_err(|error| stored_value_invalid(error.to_string()))?;
    let maximum_size_bytes = u64::try_from(maximum_payload_size).map_err(|_| {
        stored_value_invalid("record maximum payload size must be non-negative".to_owned())
    })?;
    let record_id = RecordId::try_new(
        row.try_get::<String, _>("record_id")
            .map_err(|error| stored_value_invalid(error.to_string()))?,
    )
    .map_err(|error| stored_value_invalid(error.to_string()))?;
    let owner = ModuleId::try_new(
        row.try_get::<String, _>("owner_module_id")
            .map_err(|error| stored_value_invalid(error.to_string()))?,
    )
    .map_err(|error| stored_value_invalid(error.to_string()))?;
    let schema_id = SchemaId::try_new(
        row.try_get::<String, _>("schema_id")
            .map_err(|error| stored_value_invalid(error.to_string()))?,
    )
    .map_err(|error| stored_value_invalid(error.to_string()))?;
    let schema_version = SchemaVersion::try_new(
        row.try_get::<String, _>("schema_version")
            .map_err(|error| stored_value_invalid(error.to_string()))?,
    )
    .map_err(|error| stored_value_invalid(error.to_string()))?;
    let retention_policy_id = RetentionPolicyId::try_new(
        row.try_get::<String, _>("retention_policy_id")
            .map_err(|error| stored_value_invalid(error.to_string()))?,
    )
    .map_err(|error| stored_value_invalid(error.to_string()))?;
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
    let payload = TypedPayload {
        owner,
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
    payload.validate().map_err(|error| {
        stored_value_invalid(format!("stored record payload is invalid: {}", error.code))
    })?;
    Ok(RecordSnapshot {
        reference: RecordRef {
            record_type: record_type.clone(),
            record_id,
        },
        version: row
            .try_get("version")
            .map_err(|error| stored_value_invalid(error.to_string()))?,
        payload,
    })
}

fn batch_decode_error(error: BatchError) -> SdkError {
    stored_value_invalid(error.to_string())
}

fn invalid_query(code: &'static str, safe_message: &'static str) -> SdkError {
    SdkError::new(code, ErrorCategory::InvalidArgument, false, safe_message)
}

fn stored_value_invalid(internal: String) -> SdkError {
    SdkError::new(
        "DATA_QUERY_STORED_VALUE_INVALID",
        ErrorCategory::Unavailable,
        true,
        "The data service is temporarily unavailable.",
    )
    .with_internal_reference(internal)
}

fn database_unavailable(error: sqlx::Error) -> SdkError {
    SdkError::new(
        "DATA_QUERY_UNAVAILABLE",
        ErrorCategory::Unavailable,
        true,
        "The data service is temporarily unavailable.",
    )
    .with_internal_reference(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_list_query_rejects_zero_and_excessive_page_sizes() {
        let base = RecordListQuery {
            tenant_id: TenantId::try_new("tenant-a").unwrap(),
            owner_module_id: ModuleId::try_new("crm.sales").unwrap(),
            record_type: RecordType::try_new("sales.deal").unwrap(),
            page_size: 1,
            sort: RecordQuerySort::CreatedAtAscending,
            after: None,
        };
        let mut invalid = base.clone();
        invalid.page_size = 0;
        assert_eq!(
            invalid.validate().unwrap_err().code,
            "DATA_QUERY_PAGE_SIZE_INVALID"
        );
        let mut invalid = base;
        invalid.page_size = MAXIMUM_RECORD_QUERY_PAGE_SIZE + 1;
        assert_eq!(
            invalid.validate().unwrap_err().code,
            "DATA_QUERY_PAGE_SIZE_INVALID"
        );
    }

    #[test]
    fn record_query_continuation_requires_canonical_epoch_microseconds() {
        let valid = RecordQueryContinuation {
            sort_value: "1700000000000000".to_owned(),
            record_id: RecordId::try_new("deal-1").unwrap(),
        };
        valid.validate().unwrap();
        let invalid = RecordQueryContinuation {
            sort_value: "2026-07-11T12:00:00Z".to_owned(),
            record_id: RecordId::try_new("deal-1").unwrap(),
        };
        assert_eq!(
            invalid.validate().unwrap_err().code,
            "DATA_QUERY_CONTINUATION_INVALID"
        );
    }

    #[test]
    fn record_query_sort_ids_are_stable_contract_values() {
        assert_eq!(
            RecordQuerySort::CreatedAtAscending.id(),
            "created_at_asc_record_id_asc"
        );
        assert_eq!(
            RecordQuerySort::UpdatedAtDescending.id(),
            "updated_at_desc_record_id_asc"
        );
    }
}
