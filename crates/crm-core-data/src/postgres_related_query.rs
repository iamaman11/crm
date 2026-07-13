use crate::postgres::PostgresDataStore;
use crate::postgres_batch::{BatchError, parse_data_class, parse_payload_encoding};
use crm_module_sdk::{
    ErrorCategory, ModuleId, RecordId, RecordRef, RecordSnapshot, RecordType, RelationshipType,
    RetentionPolicyId, SchemaId, SchemaVersion, SdkError, TenantId, TypedPayload,
};
use sqlx::{Postgres, Row, Transaction, postgres::PgRow};

pub const MAXIMUM_RELATED_RECORD_QUERY_PAGE_SIZE: u32 = 1_000;

/// Authoritative record lookup through a transactionally maintained core relationship.
///
/// This query never reads rebuildable projections. Both the relationship row and target
/// record are tenant-scoped authoritative storage, and RLS is bound before the read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelatedRecordListQuery {
    pub tenant_id: TenantId,
    pub relationship_owner_module_id: ModuleId,
    pub relationship_type: RelationshipType,
    pub source: RecordRef,
    pub target_owner_module_id: ModuleId,
    pub target_record_type: RecordType,
    pub page_size: u32,
    pub after_record_id: Option<RecordId>,
}

impl RelatedRecordListQuery {
    pub fn validate(&self) -> Result<(), SdkError> {
        if self.page_size == 0 || self.page_size > MAXIMUM_RELATED_RECORD_QUERY_PAGE_SIZE {
            return Err(invalid_query(
                "DATA_RELATED_QUERY_PAGE_SIZE_INVALID",
                "The related-record query page size is invalid.",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelatedRecordQueryPage {
    pub records: Vec<RecordSnapshot>,
    pub next_record_id: Option<RecordId>,
}

impl PostgresDataStore {
    pub async fn list_related_records_for_query(
        &self,
        query: &RelatedRecordListQuery,
    ) -> Result<RelatedRecordQueryPage, SdkError> {
        query.validate()?;
        let fetch_limit = i64::from(query.page_size) + 1;
        let after_record_id = query.after_record_id.as_ref().map(RecordId::as_str);
        let mut transaction = self.pool().begin().await.map_err(database_unavailable)?;
        bind_read_context(&mut transaction, &query.tenant_id).await?;

        let rows = sqlx::query(
            r#"
            SELECT
              target.record_id,
              target.version,
              target.owner_module_id,
              target.schema_id,
              target.schema_version,
              target.descriptor_hash,
              target.data_class,
              target.payload_encoding,
              target.maximum_payload_size,
              target.retention_policy_id,
              target.payload_bytes
            FROM crm.relationships AS relation
            JOIN crm.records AS target
              ON target.tenant_id = relation.tenant_id
             AND target.record_type = relation.target_record_type
             AND target.record_id = relation.target_record_id
            WHERE relation.tenant_id = $1
              AND relation.owner_module_id = $2
              AND relation.relationship_type = $3
              AND relation.source_record_type = $4
              AND relation.source_record_id = $5
              AND relation.target_record_type = $6
              AND target.owner_module_id = $7
              AND target.record_type = $6
              AND target.deleted_at IS NULL
              AND ($8::text IS NULL OR target.record_id > $8)
            ORDER BY target.record_id ASC
            LIMIT $9
            "#,
        )
        .bind(query.tenant_id.as_str())
        .bind(query.relationship_owner_module_id.as_str())
        .bind(query.relationship_type.as_str())
        .bind(query.source.record_type.as_str())
        .bind(query.source.record_id.as_str())
        .bind(query.target_record_type.as_str())
        .bind(query.target_owner_module_id.as_str())
        .bind(after_record_id)
        .bind(fetch_limit)
        .fetch_all(&mut *transaction)
        .await
        .map_err(database_unavailable)?;
        transaction.commit().await.map_err(database_unavailable)?;

        let has_more = rows.len() > query.page_size as usize;
        let mut records = rows
            .into_iter()
            .map(|row| decode_related_record(&query.target_record_type, row))
            .collect::<Result<Vec<_>, SdkError>>()?;
        if has_more {
            records.pop();
        }
        let next_record_id = has_more
            .then(|| {
                records
                    .last()
                    .map(|record| record.reference.record_id.clone())
            })
            .flatten();
        Ok(RelatedRecordQueryPage {
            records,
            next_record_id,
        })
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

fn decode_related_record(record_type: &RecordType, row: PgRow) -> Result<RecordSnapshot, SdkError> {
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
    fn related_query_rejects_zero_and_oversized_pages() {
        let mut query = RelatedRecordListQuery {
            tenant_id: TenantId::try_new("tenant-a").unwrap(),
            relationship_owner_module_id: ModuleId::try_new("crm.test").unwrap(),
            relationship_type: RelationshipType::try_new("test.related").unwrap(),
            source: RecordRef {
                record_type: RecordType::try_new("test.source").unwrap(),
                record_id: RecordId::try_new("source-1").unwrap(),
            },
            target_owner_module_id: ModuleId::try_new("crm.target").unwrap(),
            target_record_type: RecordType::try_new("test.target").unwrap(),
            page_size: 0,
            after_record_id: None,
        };
        assert_eq!(
            query.validate().unwrap_err().code,
            "DATA_RELATED_QUERY_PAGE_SIZE_INVALID"
        );
        query.page_size = MAXIMUM_RELATED_RECORD_QUERY_PAGE_SIZE + 1;
        assert_eq!(
            query.validate().unwrap_err().code,
            "DATA_RELATED_QUERY_PAGE_SIZE_INVALID"
        );
        query.page_size = MAXIMUM_RELATED_RECORD_QUERY_PAGE_SIZE;
        query.validate().unwrap();
    }
}
