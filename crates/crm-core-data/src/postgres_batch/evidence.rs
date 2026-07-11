async fn insert_outbox_event(
    transaction: &mut Transaction<'_, Postgres>,
    context: &ModuleExecutionContext,
    evidence: &EventEvidence,
) -> Result<(), BatchError> {
    let maximum_size = checked_size(evidence.event.payload.maximum_size_bytes, "event payload")?;
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

async fn insert_audit_record(
    transaction: &mut Transaction<'_, Postgres>,
    context: &ModuleExecutionContext,
    audit: &MaterializedAuditRecord,
) -> Result<(), BatchError> {
    sqlx::query(
        r#"
        INSERT INTO crm.audit_records (
          tenant_id, audit_sequence, audit_record_id, business_transaction_id,
          actor_id, capability_id, capability_version, canonicalization_profile,
          previous_hash, record_hash, canonical_envelope, occurred_at
        )
        VALUES (
          $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11,
          TIMESTAMPTZ 'epoch' + ($12::bigint / 1000) * INTERVAL '1 microsecond'
        )
        "#,
    )
    .bind(context.execution.tenant_id.as_str())
    .bind(audit.audit_sequence)
    .bind(&audit.audit_record_id)
    .bind(context.execution.business_transaction_id.as_str())
    .bind(context.execution.actor_id.as_str())
    .bind(context.execution.capability_id.as_str())
    .bind(context.execution.capability_version.as_str())
    .bind(&audit.canonicalization_profile)
    .bind(audit.previous_hash.as_slice())
    .bind(audit.record_hash.as_slice())
    .bind(audit.canonical_envelope.as_slice())
    .bind(audit.occurred_at_unix_nanos)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

async fn complete_batch_idempotency(
    transaction: &mut Transaction<'_, Postgres>,
    plan: &BatchMutationPlan,
    result: &BatchMutationResult,
) -> Result<(), BatchError> {
    let response = serde_json::to_vec(result).map_err(|error| {
        BatchError::InvalidPlan(format!("batch response serialization failed: {error}"))
    })?;
    let response_descriptor_hash = batch_result_descriptor_hash();
    complete_idempotency_response(
        transaction,
        plan,
        BATCH_RESULT_SCHEMA_ID,
        BATCH_RESULT_SCHEMA_VERSION,
        response_descriptor_hash,
        response,
    )
    .await
}

async fn complete_idempotency_response(
    transaction: &mut Transaction<'_, Postgres>,
    plan: &BatchMutationPlan,
    schema_id: &str,
    schema_version: &str,
    descriptor_hash: [u8; 32],
    response: Vec<u8>,
) -> Result<(), BatchError> {
    let update = sqlx::query(
        r#"
        UPDATE crm.idempotency_records
           SET status = 'completed',
               response_schema_id = $4,
               response_schema_version = $5,
               response_descriptor_hash = $6,
               response_payload_encoding = 'json',
               response_payload = $7,
               updated_at = clock_timestamp()
         WHERE tenant_id = $1
           AND idempotency_scope = $2
           AND idempotency_key = $3
           AND business_transaction_id = $8
           AND status = 'in_progress'
        "#,
    )
    .bind(plan.context.execution.tenant_id.as_str())
    .bind(&plan.idempotency.scope)
    .bind(&plan.idempotency.key)
    .bind(schema_id)
    .bind(schema_version)
    .bind(descriptor_hash.as_slice())
    .bind(response)
    .bind(plan.context.execution.business_transaction_id.as_str())
    .execute(&mut **transaction)
    .await?;
    if update.rows_affected() != 1 {
        return Err(BatchError::Conflict(
            "idempotency claim disappeared before completion".to_owned(),
        ));
    }
    Ok(())
}

async fn complete_capability_idempotency(
    transaction: &mut Transaction<'_, Postgres>,
    plan: &BatchMutationPlan,
    result: &CapabilityExecutionResult,
) -> Result<(), BatchError> {
    let response = serde_json::to_vec(result).map_err(|error| {
        BatchError::InvalidPlan(format!(
            "capability response serialization failed: {error}"
        ))
    })?;
    let response_descriptor_hash = capability_result_descriptor_hash();
    complete_idempotency_response(
        transaction,
        plan,
        CAPABILITY_RESULT_SCHEMA_ID,
        CAPABILITY_RESULT_SCHEMA_VERSION,
        response_descriptor_hash,
        response,
    )
    .await
}

async fn insert_completion_marker(
    transaction: &mut Transaction<'_, Postgres>,
    plan: &BatchMutationPlan,
) -> Result<(), BatchError> {
    let expected_events = i32::try_from(plan.events.len())
        .map_err(|_| BatchError::InvalidPlan("too many events in one batch".to_owned()))?;
    let expected_audits = i32::try_from(plan.audits.len())
        .map_err(|_| BatchError::InvalidPlan("too many audits in one batch".to_owned()))?;
    sqlx::query(
        r#"
        INSERT INTO crm.business_transactions (
          tenant_id, business_transaction_id, actor_id, request_id,
          capability_id, capability_version,
          expected_outbox_events, expected_audit_records, expected_idempotency_records
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 1)
        "#,
    )
    .bind(plan.context.execution.tenant_id.as_str())
    .bind(plan.context.execution.business_transaction_id.as_str())
    .bind(plan.context.execution.actor_id.as_str())
    .bind(plan.context.execution.request_id.as_str())
    .bind(plan.context.execution.capability_id.as_str())
    .bind(plan.context.execution.capability_version.as_str())
    .bind(expected_events)
    .bind(expected_audits)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}
