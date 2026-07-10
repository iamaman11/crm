impl PostgresDataStore {
    pub async fn execute_batch(
        &self,
        plan: &BatchMutationPlan,
    ) -> Result<BatchMutationResult, BatchError> {
        self.execute_batch_with_fault(plan, FaultInjection::None)
            .await
    }

    #[doc(hidden)]
    pub async fn execute_batch_with_fault(
        &self,
        plan: &BatchMutationPlan,
        fault: FaultInjection,
    ) -> Result<BatchMutationResult, BatchError> {
        plan.validate()?;
        let mut transaction = self.pool().begin().await?;
        bind_execution_context(&mut transaction, &plan.context).await?;

        if let Some(result) = load_replay(&mut transaction, plan).await? {
            transaction.commit().await?;
            return Ok(result);
        }

        if fault != FaultInjection::OmitIdempotency {
            insert_idempotency_claim(&mut transaction, plan).await?;
        }

        let mut records = Vec::with_capacity(plan.records.len());
        for mutation in &plan.records {
            records.push(apply_record_mutation(&mut transaction, &plan.context, mutation).await?);
        }

        let mut linked_relationships = Vec::new();
        let mut unlinked_relationships = Vec::new();
        for mutation in &plan.relationships {
            match mutation {
                RelationshipMutation::Link {
                    relationship,
                    payload,
                } => {
                    link_relationship(&mut transaction, &plan.context, relationship, payload)
                        .await?;
                    linked_relationships.push(relationship.clone());
                }
                RelationshipMutation::Unlink { relationship } => {
                    unlink_relationship(&mut transaction, &plan.context, relationship).await?;
                    unlinked_relationships.push(relationship.clone());
                }
            }
        }

        if fault != FaultInjection::OmitOutbox {
            for event in &plan.events {
                insert_outbox_event(&mut transaction, &plan.context, event).await?;
            }
        }
        if fault != FaultInjection::OmitAudit {
            for audit in &plan.audits {
                insert_audit_record(&mut transaction, &plan.context, audit).await?;
            }
        }

        let result = BatchMutationResult {
            records,
            linked_relationships,
            unlinked_relationships,
            replayed: false,
        };
        if fault != FaultInjection::OmitIdempotency {
            complete_idempotency(&mut transaction, plan, &result).await?;
        }
        if fault != FaultInjection::OmitCompletionMarker {
            insert_completion_marker(&mut transaction, plan).await?;
        }

        transaction.commit().await?;
        Ok(result)
    }
}

async fn bind_execution_context(
    transaction: &mut Transaction<'_, Postgres>,
    context: &ModuleExecutionContext,
) -> Result<(), BatchError> {
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

async fn load_replay(
    transaction: &mut Transaction<'_, Postgres>,
    plan: &BatchMutationPlan,
) -> Result<Option<BatchMutationResult>, BatchError> {
    let row = sqlx::query(
        r#"
        SELECT
          request_hash,
          status,
          response_schema_id,
          response_schema_version,
          response_descriptor_hash,
          response_payload_encoding,
          response_payload
        FROM crm.idempotency_records
        WHERE tenant_id = $1
          AND idempotency_scope = $2
          AND idempotency_key = $3
        "#,
    )
    .bind(plan.context.execution.tenant_id.as_str())
    .bind(&plan.idempotency.scope)
    .bind(&plan.idempotency.key)
    .fetch_optional(&mut **transaction)
    .await?;

    let Some(row) = row else {
        return Ok(None);
    };
    let stored_hash: Vec<u8> = row.try_get("request_hash")?;
    if stored_hash.as_slice() != plan.idempotency.request_hash.as_slice() {
        return Err(BatchError::IdempotencyKeyReused);
    }
    let status: String = row.try_get("status")?;
    if status != "completed" {
        return Err(BatchError::IdempotencyInProgress);
    }

    let schema_id: Option<String> = row.try_get("response_schema_id")?;
    let schema_version: Option<String> = row.try_get("response_schema_version")?;
    let descriptor_hash: Option<Vec<u8>> = row.try_get("response_descriptor_hash")?;
    let encoding: Option<String> = row.try_get("response_payload_encoding")?;
    let payload: Option<Vec<u8>> = row.try_get("response_payload")?;
    let expected_descriptor_hash = batch_result_descriptor_hash();
    if schema_id.as_deref() != Some(BATCH_RESULT_SCHEMA_ID)
        || schema_version.as_deref() != Some(BATCH_RESULT_SCHEMA_VERSION)
        || descriptor_hash.as_deref() != Some(expected_descriptor_hash.as_slice())
        || encoding.as_deref() != Some("json")
    {
        return Err(BatchError::InvalidStoredValue(
            "response schema metadata does not match the batch result contract".to_owned(),
        ));
    }
    let payload = payload.ok_or_else(|| {
        BatchError::InvalidStoredValue("completed response payload is missing".to_owned())
    })?;
    let mut result: BatchMutationResult = serde_json::from_slice(&payload).map_err(|error| {
        BatchError::InvalidStoredValue(format!("response JSON is invalid: {error}"))
    })?;
    result.replayed = true;
    Ok(Some(result))
}

async fn insert_idempotency_claim(
    transaction: &mut Transaction<'_, Postgres>,
    plan: &BatchMutationPlan,
) -> Result<(), BatchError> {
    let result = sqlx::query(
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
          $1, $2, $3, $4, 'in_progress', $5,
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
    .await;

    match result {
        Ok(_) => Ok(()),
        Err(sqlx::Error::Database(error)) if error.is_unique_violation() => {
            Err(BatchError::IdempotencyInProgress)
        }
        Err(error) => Err(BatchError::Database(error)),
    }
}
