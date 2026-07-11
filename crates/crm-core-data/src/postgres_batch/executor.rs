const CAPABILITY_RESULT_SCHEMA_ID: &str = "crm.core.data.capability_execution_result";
const CAPABILITY_RESULT_SCHEMA_VERSION: &str = "1.0.0";
const CAPABILITY_RESULT_SCHEMA_DESCRIPTOR: &[u8] =
    b"crm.core.data.capability_execution_result/v1:output,affected_resources,replayed";

impl PostgresDataStore {
    pub async fn execute_batch(
        &self,
        plan: &BatchMutationPlan,
    ) -> Result<BatchMutationResult, BatchError> {
        self.execute_batch_with_fault(plan, FaultInjection::None).await
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

        if let Some(result) = load_batch_replay(&mut transaction, plan).await? {
            transaction.commit().await?;
            return Ok(result);
        }

        if fault != FaultInjection::OmitIdempotency {
            insert_idempotency_claim(
                &mut transaction,
                &plan.context,
                &plan.idempotency,
            )
            .await?;
        }

        let result = apply_planned_batch(&mut transaction, plan, fault).await?;
        if fault != FaultInjection::OmitIdempotency {
            complete_batch_idempotency(&mut transaction, plan, &result).await?;
        }
        if fault != FaultInjection::OmitCompletionMarker {
            insert_completion_marker(&mut transaction, plan).await?;
        }

        transaction.commit().await?;
        Ok(result)
    }

    pub(crate) async fn execute_transactional_aggregate(
        &self,
        definition: &CapabilityDefinition,
        request: CapabilityRequest,
        target: AggregateTarget,
        planner: &dyn TransactionalAggregatePlanner,
    ) -> Result<CapabilityExecutionResult, BatchError> {
        let scope = capability_idempotency_scope(definition);
        let idempotency = capability_idempotency(&request, scope)?;
        let mut transaction = self.pool().begin().await?;
        bind_execution_context(&mut transaction, &request.context).await?;

        if let Some(result) = load_capability_replay(
            &mut transaction,
            &request.context,
            &idempotency,
        )
        .await?
        {
            transaction.commit().await?;
            return Ok(result);
        }

        insert_idempotency_claim(&mut transaction, &request.context, &idempotency).await?;
        let current = load_record_for_update(
            &mut transaction,
            &request.context,
            &target.reference,
        )
        .await?;
        validate_target_presence(&target, current.as_ref())?;

        let execution_plan = planner
            .plan(definition, &request, current.as_ref())
            .map_err(BatchError::Sdk)?;
        validate_execution_plan(definition, &request, &execution_plan)
            .map_err(BatchError::Sdk)?;
        validate_target_mutation(&target, current.as_ref(), &execution_plan)?;

        let batch_result = apply_planned_batch(
            &mut transaction,
            &execution_plan.batch,
            FaultInjection::None,
        )
        .await?;
        let result = CapabilityExecutionResult {
            output: execution_plan.output,
            affected_resources: affected_resources(&batch_result),
            replayed: false,
        };
        complete_capability_idempotency(
            &mut transaction,
            &execution_plan.batch,
            &result,
        )
        .await?;
        insert_completion_marker(&mut transaction, &execution_plan.batch).await?;
        transaction.commit().await?;
        Ok(result)
    }
}

fn capability_idempotency(
    request: &CapabilityRequest,
    scope: String,
) -> Result<IdempotencyEvidence, BatchError> {
    const DEFAULT_IDEMPOTENCY_TTL_NANOS: i64 = 86_400_000_000_000;
    let expires_at_unix_nanos = request
        .context
        .execution
        .request_started_at_unix_nanos
        .checked_add(DEFAULT_IDEMPOTENCY_TTL_NANOS)
        .ok_or_else(|| BatchError::InvalidPlan("idempotency expiry overflowed i64".to_owned()))?;
    Ok(IdempotencyEvidence {
        scope,
        key: request.context.execution.idempotency_key.to_string(),
        request_hash: request.input_hash,
        expires_at_unix_nanos,
    })
}

fn validate_target_presence(
    target: &AggregateTarget,
    current: Option<&RecordSnapshot>,
) -> Result<(), BatchError> {
    match (target.presence, current) {
        (AggregatePresence::MustBeAbsent, None) | (AggregatePresence::MustExist, Some(_)) => Ok(()),
        (AggregatePresence::MustBeAbsent, Some(_)) => Err(BatchError::Sdk(SdkError::new(
            "CAPABILITY_AGGREGATE_ALREADY_EXISTS",
            ErrorCategory::Conflict,
            false,
            "The requested aggregate already exists.",
        ))),
        (AggregatePresence::MustExist, None) => Err(BatchError::Sdk(SdkError::new(
            "CAPABILITY_AGGREGATE_NOT_FOUND",
            ErrorCategory::NotFound,
            false,
            "The requested aggregate was not found.",
        ))),
    }
}

fn validate_target_mutation(
    target: &AggregateTarget,
    current: Option<&RecordSnapshot>,
    plan: &CapabilityBatchExecutionPlan,
) -> Result<(), BatchError> {
    let mut matching = plan.batch.records.iter().filter(|mutation| match mutation {
        RecordMutation::Create { reference, .. } | RecordMutation::Update { reference, .. } => {
            reference == &target.reference
        }
    });
    let mutation = matching.next().ok_or_else(|| {
        BatchError::InvalidPlan("aggregate plan does not mutate its resolved target".to_owned())
    })?;
    if matching.next().is_some() {
        return Err(BatchError::InvalidPlan(
            "aggregate plan mutates its resolved target more than once".to_owned(),
        ));
    }

    match (target.presence, current, mutation) {
        (AggregatePresence::MustBeAbsent, None, RecordMutation::Create { .. }) => Ok(()),
        (
            AggregatePresence::MustExist,
            Some(snapshot),
            RecordMutation::Update {
                expected_version, ..
            },
        ) if *expected_version == snapshot.version => Ok(()),
        _ => Err(BatchError::InvalidPlan(
            "aggregate target presence, locked version and planned mutation do not agree"
                .to_owned(),
        )),
    }
}

async fn apply_planned_batch(
    transaction: &mut Transaction<'_, Postgres>,
    plan: &BatchMutationPlan,
    fault: FaultInjection,
) -> Result<BatchMutationResult, BatchError> {
    let mut records = Vec::with_capacity(plan.records.len());
    for mutation in &plan.records {
        records.push(apply_record_mutation(transaction, &plan.context, mutation).await?);
    }

    let mut linked_relationships = Vec::new();
    let mut unlinked_relationships = Vec::new();
    for mutation in &plan.relationships {
        match mutation {
            RelationshipMutation::Link {
                relationship,
                payload,
            } => {
                link_relationship(transaction, &plan.context, relationship, payload).await?;
                linked_relationships.push(relationship.clone());
            }
            RelationshipMutation::Unlink { relationship } => {
                unlink_relationship(transaction, &plan.context, relationship).await?;
                unlinked_relationships.push(relationship.clone());
            }
        }
    }

    if fault != FaultInjection::OmitOutbox {
        for event in &plan.events {
            insert_outbox_event(transaction, &plan.context, event).await?;
        }
    }
    if fault != FaultInjection::OmitAudit {
        let materialized = materialize_audit_chain(transaction, &plan.context, &plan.audits)
            .await
            .map_err(audit_materialization_to_batch_error)?;
        for audit in &materialized {
            insert_audit_record(transaction, &plan.context, audit).await?;
        }
    }

    Ok(BatchMutationResult {
        records,
        linked_relationships,
        unlinked_relationships,
        replayed: false,
    })
}

pub(crate) async fn bind_execution_context(
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

async fn load_batch_replay(
    transaction: &mut Transaction<'_, Postgres>,
    plan: &BatchMutationPlan,
) -> Result<Option<BatchMutationResult>, BatchError> {
    let response = load_idempotency_response(
        transaction,
        &plan.context,
        &plan.idempotency,
    )
    .await?;
    let Some(response) = response else {
        return Ok(None);
    };
    let expected_descriptor_hash = batch_result_descriptor_hash();
    validate_response_metadata(
        &response,
        BATCH_RESULT_SCHEMA_ID,
        BATCH_RESULT_SCHEMA_VERSION,
        expected_descriptor_hash,
    )?;
    let mut result: BatchMutationResult = serde_json::from_slice(&response.payload).map_err(|error| {
        BatchError::InvalidStoredValue(format!("response JSON is invalid: {error}"))
    })?;
    result.replayed = true;
    Ok(Some(result))
}

async fn load_capability_replay(
    transaction: &mut Transaction<'_, Postgres>,
    context: &ModuleExecutionContext,
    idempotency: &IdempotencyEvidence,
) -> Result<Option<CapabilityExecutionResult>, BatchError> {
    let response = load_idempotency_response(transaction, context, idempotency).await?;
    let Some(response) = response else {
        return Ok(None);
    };
    let expected_descriptor_hash = capability_result_descriptor_hash();
    validate_response_metadata(
        &response,
        CAPABILITY_RESULT_SCHEMA_ID,
        CAPABILITY_RESULT_SCHEMA_VERSION,
        expected_descriptor_hash,
    )?;
    let mut result: CapabilityExecutionResult =
        serde_json::from_slice(&response.payload).map_err(|error| {
            BatchError::InvalidStoredValue(format!("capability response JSON is invalid: {error}"))
        })?;
    result.replayed = true;
    Ok(Some(result))
}

struct StoredIdempotencyResponse {
    schema_id: Option<String>,
    schema_version: Option<String>,
    descriptor_hash: Option<Vec<u8>>,
    encoding: Option<String>,
    payload: Vec<u8>,
}

async fn load_idempotency_response(
    transaction: &mut Transaction<'_, Postgres>,
    context: &ModuleExecutionContext,
    idempotency: &IdempotencyEvidence,
) -> Result<Option<StoredIdempotencyResponse>, BatchError> {
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
    .bind(context.execution.tenant_id.as_str())
    .bind(&idempotency.scope)
    .bind(&idempotency.key)
    .fetch_optional(&mut **transaction)
    .await?;

    let Some(row) = row else {
        return Ok(None);
    };
    let stored_hash: Vec<u8> = row.try_get("request_hash")?;
    if stored_hash.as_slice() != idempotency.request_hash.as_slice() {
        return Err(BatchError::IdempotencyKeyReused);
    }
    let status: String = row.try_get("status")?;
    if status != "completed" {
        return Err(BatchError::IdempotencyInProgress);
    }
    let payload: Option<Vec<u8>> = row.try_get("response_payload")?;
    Ok(Some(StoredIdempotencyResponse {
        schema_id: row.try_get("response_schema_id")?,
        schema_version: row.try_get("response_schema_version")?,
        descriptor_hash: row.try_get("response_descriptor_hash")?,
        encoding: row.try_get("response_payload_encoding")?,
        payload: payload.ok_or_else(|| {
            BatchError::InvalidStoredValue("completed response payload is missing".to_owned())
        })?,
    }))
}

fn validate_response_metadata(
    response: &StoredIdempotencyResponse,
    schema_id: &str,
    schema_version: &str,
    descriptor_hash: [u8; 32],
) -> Result<(), BatchError> {
    if response.schema_id.as_deref() != Some(schema_id)
        || response.schema_version.as_deref() != Some(schema_version)
        || response.descriptor_hash.as_deref() != Some(descriptor_hash.as_slice())
        || response.encoding.as_deref() != Some("json")
    {
        return Err(BatchError::InvalidStoredValue(
            "response schema metadata does not match the expected result contract".to_owned(),
        ));
    }
    Ok(())
}

async fn insert_idempotency_claim(
    transaction: &mut Transaction<'_, Postgres>,
    context: &ModuleExecutionContext,
    idempotency: &IdempotencyEvidence,
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
    .bind(context.execution.tenant_id.as_str())
    .bind(&idempotency.scope)
    .bind(&idempotency.key)
    .bind(idempotency.request_hash.as_slice())
    .bind(context.execution.business_transaction_id.as_str())
    .bind(idempotency.expires_at_unix_nanos)
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

fn capability_result_descriptor_hash() -> [u8; 32] {
    Sha256::digest(CAPABILITY_RESULT_SCHEMA_DESCRIPTOR).into()
}
