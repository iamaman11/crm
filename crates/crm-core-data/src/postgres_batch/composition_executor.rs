impl PostgresDataStore {
    /// Execute multiple independently owner-bound mutation fragments in one
    /// PostgreSQL transaction and one governed business transaction.
    ///
    /// Each record/relationship mutation is applied under the fragment owner's
    /// module identity while the authenticated execution, capability,
    /// idempotency, audit chain and completion marker remain those of the single
    /// public composed operation.
    pub async fn execute_composed_batch(
        &self,
        plan: &ComposedBatchMutationPlan,
    ) -> Result<BatchMutationResult, BatchError> {
        plan.validate()?;
        let mut transaction = self.pool().begin().await?;
        bind_execution_context(&mut transaction, &plan.context).await?;

        if let Some(result) = load_composed_batch_replay(&mut transaction, plan).await? {
            transaction.commit().await?;
            return Ok(result);
        }

        insert_idempotency_claim(&mut transaction, &plan.context, &plan.idempotency).await?;
        let result = apply_composed_batch(&mut transaction, plan).await?;
        complete_composed_batch_idempotency(&mut transaction, plan, &result).await?;
        insert_composed_completion_marker(&mut transaction, plan).await?;

        transaction.commit().await?;
        Ok(result)
    }
}

async fn apply_composed_batch(
    transaction: &mut Transaction<'_, Postgres>,
    plan: &ComposedBatchMutationPlan,
) -> Result<BatchMutationResult, BatchError> {
    let record_capacity = plan.record_mutations().count();
    let relationship_capacity = plan.relationship_mutations().count();
    let mut records = Vec::with_capacity(record_capacity);
    let mut linked_relationships = Vec::with_capacity(relationship_capacity);
    let mut unlinked_relationships = Vec::with_capacity(relationship_capacity);

    for fragment in &plan.owner_fragments {
        let owner_context = owner_execution_context(&plan.context, &fragment.owner_module_id);
        for mutation in &fragment.records {
            records.push(apply_record_mutation(transaction, &owner_context, mutation).await?);
        }
        for mutation in &fragment.relationships {
            match mutation {
                RelationshipMutation::Link {
                    relationship,
                    payload,
                } => {
                    link_relationship(transaction, &owner_context, relationship, payload).await?;
                    linked_relationships.push(relationship.clone());
                }
                RelationshipMutation::Unlink { relationship } => {
                    unlink_relationship(transaction, &owner_context, relationship).await?;
                    unlinked_relationships.push(relationship.clone());
                }
            }
        }
        for event in &fragment.events {
            insert_outbox_event(transaction, &owner_context, event).await?;
        }
    }

    let materialized = materialize_audit_chain(transaction, &plan.context, &plan.audits)
        .await
        .map_err(audit_materialization_to_batch_error)?;
    for audit in &materialized {
        insert_audit_record(transaction, &plan.context, audit).await?;
    }

    Ok(BatchMutationResult {
        records,
        linked_relationships,
        unlinked_relationships,
        replayed: false,
    })
}

fn owner_execution_context(
    context: &ModuleExecutionContext,
    owner_module_id: &ModuleId,
) -> ModuleExecutionContext {
    ModuleExecutionContext {
        module_id: owner_module_id.clone(),
        execution: context.execution.clone(),
    }
}

async fn load_composed_batch_replay(
    transaction: &mut Transaction<'_, Postgres>,
    plan: &ComposedBatchMutationPlan,
) -> Result<Option<BatchMutationResult>, BatchError> {
    let response = load_idempotency_response(transaction, &plan.context, &plan.idempotency).await?;
    let Some(response) = response else {
        return Ok(None);
    };
    validate_response_metadata(
        &response,
        BATCH_RESULT_SCHEMA_ID,
        BATCH_RESULT_SCHEMA_VERSION,
        batch_result_descriptor_hash(),
    )?;
    let mut result: BatchMutationResult = serde_json::from_slice(&response.payload).map_err(|error| {
        BatchError::InvalidStoredValue(format!("composed batch response JSON is invalid: {error}"))
    })?;
    result.replayed = true;
    Ok(Some(result))
}

async fn complete_composed_batch_idempotency(
    transaction: &mut Transaction<'_, Postgres>,
    plan: &ComposedBatchMutationPlan,
    result: &BatchMutationResult,
) -> Result<(), BatchError> {
    let response = serde_json::to_vec(result).map_err(|error| {
        BatchError::InvalidPlan(format!("composed batch response serialization failed: {error}"))
    })?;
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
    .bind(BATCH_RESULT_SCHEMA_ID)
    .bind(BATCH_RESULT_SCHEMA_VERSION)
    .bind(batch_result_descriptor_hash().as_slice())
    .bind(response)
    .bind(plan.context.execution.business_transaction_id.as_str())
    .execute(&mut **transaction)
    .await?;
    if update.rows_affected() != 1 {
        return Err(BatchError::Conflict(
            "idempotency claim disappeared before composed batch completion".to_owned(),
        ));
    }
    Ok(())
}

async fn insert_composed_completion_marker(
    transaction: &mut Transaction<'_, Postgres>,
    plan: &ComposedBatchMutationPlan,
) -> Result<(), BatchError> {
    let expected_events = i32::try_from(plan.event_evidence().count())
        .map_err(|_| BatchError::InvalidPlan("too many events in one composed batch".to_owned()))?;
    let expected_audits = i32::try_from(plan.audits.len())
        .map_err(|_| BatchError::InvalidPlan("too many audits in one composed batch".to_owned()))?;
    sqlx::query(
        r#"
        INSERT INTO crm.business_transactions (
          tenant_id, business_transaction_id, actor_id, request_id,
          correlation_id, trace_id,
          capability_id, capability_version,
          expected_outbox_events, expected_audit_records, expected_idempotency_records
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, 1)
        "#,
    )
    .bind(plan.context.execution.tenant_id.as_str())
    .bind(plan.context.execution.business_transaction_id.as_str())
    .bind(plan.context.execution.actor_id.as_str())
    .bind(plan.context.execution.request_id.as_str())
    .bind(plan.context.execution.correlation_id.as_str())
    .bind(plan.context.execution.trace_id.as_str())
    .bind(plan.context.execution.capability_id.as_str())
    .bind(plan.context.execution.capability_version.as_str())
    .bind(expected_events)
    .bind(expected_audits)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}
