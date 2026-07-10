async fn apply_record_mutation(
    transaction: &mut Transaction<'_, Postgres>,
    context: &ModuleExecutionContext,
    mutation: &RecordMutation,
) -> Result<RecordSnapshot, BatchError> {
    match mutation {
        RecordMutation::Create { reference, payload } => {
            insert_record(transaction, context, reference, payload).await
        }
        RecordMutation::Update {
            reference,
            expected_version,
            payload,
        } => update_record(transaction, context, reference, *expected_version, payload).await,
    }
}

async fn insert_record(
    transaction: &mut Transaction<'_, Postgres>,
    context: &ModuleExecutionContext,
    reference: &RecordRef,
    payload: &TypedPayload,
) -> Result<RecordSnapshot, BatchError> {
    let maximum_size = checked_size(payload.maximum_size_bytes, "record payload")?;
    sqlx::query(
        r#"
        INSERT INTO crm.records (
          tenant_id, record_type, record_id, version, owner_module_id,
          schema_id, schema_version, descriptor_hash, data_class, payload_encoding,
          maximum_payload_size, retention_policy_id, payload_bytes,
          last_business_transaction_id
        )
        VALUES ($1, $2, $3, 1, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
        "#,
    )
    .bind(context.execution.tenant_id.as_str())
    .bind(reference.record_type.as_str())
    .bind(reference.record_id.as_str())
    .bind(payload.owner.as_str())
    .bind(payload.schema_id.as_str())
    .bind(payload.schema_version.as_str())
    .bind(payload.descriptor_hash.as_slice())
    .bind(data_class_name(payload.data_class))
    .bind(payload_encoding_name(payload.encoding))
    .bind(maximum_size)
    .bind(payload.retention_policy_id.as_str())
    .bind(payload.bytes.as_slice())
    .bind(context.execution.business_transaction_id.as_str())
    .execute(&mut **transaction)
    .await?;
    Ok(RecordSnapshot {
        reference: reference.clone(),
        version: 1,
        payload: payload.clone(),
    })
}

async fn update_record(
    transaction: &mut Transaction<'_, Postgres>,
    context: &ModuleExecutionContext,
    reference: &RecordRef,
    expected_version: i64,
    payload: &TypedPayload,
) -> Result<RecordSnapshot, BatchError> {
    let maximum_size = checked_size(payload.maximum_size_bytes, "record payload")?;
    let row = sqlx::query(
        r#"
        UPDATE crm.records
           SET version = version + 1,
               schema_id = $4,
               schema_version = $5,
               descriptor_hash = $6,
               data_class = $7,
               payload_encoding = $8,
               maximum_payload_size = $9,
               retention_policy_id = $10,
               payload_bytes = $11,
               last_business_transaction_id = $12,
               updated_at = clock_timestamp()
         WHERE tenant_id = $1
           AND record_type = $2
           AND record_id = $3
           AND owner_module_id = $13
           AND version = $14
           AND deleted_at IS NULL
        RETURNING version
        "#,
    )
    .bind(context.execution.tenant_id.as_str())
    .bind(reference.record_type.as_str())
    .bind(reference.record_id.as_str())
    .bind(payload.schema_id.as_str())
    .bind(payload.schema_version.as_str())
    .bind(payload.descriptor_hash.as_slice())
    .bind(data_class_name(payload.data_class))
    .bind(payload_encoding_name(payload.encoding))
    .bind(maximum_size)
    .bind(payload.retention_policy_id.as_str())
    .bind(payload.bytes.as_slice())
    .bind(context.execution.business_transaction_id.as_str())
    .bind(context.module_id.as_str())
    .bind(expected_version)
    .fetch_optional(&mut **transaction)
    .await?;
    let row = row.ok_or_else(|| {
        BatchError::Conflict(format!(
            "record {}:{} does not exist, is not owned by the module, or version {} is stale",
            reference.record_type, reference.record_id, expected_version
        ))
    })?;
    Ok(RecordSnapshot {
        reference: reference.clone(),
        version: row.try_get("version")?,
        payload: payload.clone(),
    })
}

async fn link_relationship(
    transaction: &mut Transaction<'_, Postgres>,
    context: &ModuleExecutionContext,
    relationship: &RelationshipRef,
    payload: &TypedPayload,
) -> Result<(), BatchError> {
    let maximum_size = checked_size(payload.maximum_size_bytes, "relationship payload")?;
    sqlx::query(
        r#"
        INSERT INTO crm.relationships (
          tenant_id, relationship_type,
          source_record_type, source_record_id,
          target_record_type, target_record_id,
          version, owner_module_id, schema_id, schema_version, descriptor_hash,
          data_class, payload_encoding, maximum_payload_size, retention_policy_id,
          payload_bytes, last_business_transaction_id
        )
        VALUES ($1, $2, $3, $4, $5, $6, 1, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)
        "#,
    )
    .bind(context.execution.tenant_id.as_str())
    .bind(relationship.relationship_type.as_str())
    .bind(relationship.source.record_type.as_str())
    .bind(relationship.source.record_id.as_str())
    .bind(relationship.target.record_type.as_str())
    .bind(relationship.target.record_id.as_str())
    .bind(payload.owner.as_str())
    .bind(payload.schema_id.as_str())
    .bind(payload.schema_version.as_str())
    .bind(payload.descriptor_hash.as_slice())
    .bind(data_class_name(payload.data_class))
    .bind(payload_encoding_name(payload.encoding))
    .bind(maximum_size)
    .bind(payload.retention_policy_id.as_str())
    .bind(payload.bytes.as_slice())
    .bind(context.execution.business_transaction_id.as_str())
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

async fn unlink_relationship(
    transaction: &mut Transaction<'_, Postgres>,
    context: &ModuleExecutionContext,
    relationship: &RelationshipRef,
) -> Result<(), BatchError> {
    let result = sqlx::query(
        r#"
        DELETE FROM crm.relationships
        WHERE tenant_id = $1
          AND relationship_type = $2
          AND source_record_type = $3
          AND source_record_id = $4
          AND target_record_type = $5
          AND target_record_id = $6
          AND owner_module_id = $7
        "#,
    )
    .bind(context.execution.tenant_id.as_str())
    .bind(relationship.relationship_type.as_str())
    .bind(relationship.source.record_type.as_str())
    .bind(relationship.source.record_id.as_str())
    .bind(relationship.target.record_type.as_str())
    .bind(relationship.target.record_id.as_str())
    .bind(context.module_id.as_str())
    .execute(&mut **transaction)
    .await?;
    if result.rows_affected() != 1 {
        return Err(BatchError::Conflict(format!(
            "relationship {} was not found or is not owned by the module",
            relationship_key(relationship)
        )));
    }
    Ok(())
}
