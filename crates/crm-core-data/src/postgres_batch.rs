use crate::postgres::{AuditEvidence, FaultInjection, IdempotencyEvidence, PostgresDataStore};
use crm_module_sdk::{
    DataClass, DomainEvent, ErrorCategory, ModuleExecutionContext, ModuleId, PayloadEncoding,
    RecordRef, RecordSnapshot, RelationshipRef, RetentionPolicyId, SchemaId, SchemaVersion,
    SdkError, TypedPayload,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{Postgres, Row, Transaction};
use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

const BATCH_RESULT_SCHEMA_ID: &str = "crm.core.data.batch_mutation_result";
const BATCH_RESULT_SCHEMA_VERSION: &str = "1.0.0";
const BATCH_RESULT_SCHEMA_DESCRIPTOR: &[u8] =
    b"crm.core.data.batch_mutation_result/v1:records,linked_relationships,unlinked_relationships,replayed";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum RecordMutation {
    Create {
        reference: RecordRef,
        payload: TypedPayload,
    },
    Update {
        reference: RecordRef,
        expected_version: i64,
        payload: TypedPayload,
    },
}

impl RecordMutation {
    fn reference(&self) -> &RecordRef {
        match self {
            Self::Create { reference, .. } | Self::Update { reference, .. } => reference,
        }
    }

    fn payload(&self) -> &TypedPayload {
        match self {
            Self::Create { payload, .. } | Self::Update { payload, .. } => payload,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum RelationshipMutation {
    Link {
        relationship: RelationshipRef,
        payload: TypedPayload,
    },
    Unlink {
        relationship: RelationshipRef,
    },
}

impl RelationshipMutation {
    fn relationship(&self) -> &RelationshipRef {
        match self {
            Self::Link { relationship, .. } | Self::Unlink { relationship } => relationship,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventEvidence {
    pub event_id: String,
    pub event: DomainEvent,
    pub aggregate_version: i64,
    pub event_sequence: i64,
    pub occurred_at_unix_nanos: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchMutationPlan {
    pub context: ModuleExecutionContext,
    pub records: Vec<RecordMutation>,
    pub relationships: Vec<RelationshipMutation>,
    pub events: Vec<EventEvidence>,
    pub idempotency: IdempotencyEvidence,
    pub audits: Vec<AuditEvidence>,
}

impl BatchMutationPlan {
    pub fn validate(&self) -> Result<(), BatchError> {
        self.context.validate().map_err(BatchError::Sdk)?;
        if self.records.is_empty() && self.relationships.is_empty() {
            return Err(BatchError::InvalidPlan(
                "at least one record or relationship mutation is required".to_owned(),
            ));
        }
        if self.events.is_empty() || self.audits.is_empty() {
            return Err(BatchError::InvalidPlan(
                "every batch requires at least one outbox event and audit record".to_owned(),
            ));
        }
        if self.idempotency.scope.is_empty() || self.idempotency.key.is_empty() {
            return Err(BatchError::InvalidPlan(
                "idempotency scope and key must not be empty".to_owned(),
            ));
        }
        if self.idempotency.key != self.context.execution.idempotency_key.as_str() {
            return Err(BatchError::InvalidPlan(
                "idempotency evidence key must match the execution context".to_owned(),
            ));
        }
        if self.idempotency.request_hash.iter().all(|byte| *byte == 0) {
            return Err(BatchError::InvalidPlan(
                "idempotency request hash must not be all zeroes".to_owned(),
            ));
        }
        if self.idempotency.expires_at_unix_nanos
            <= self.context.execution.request_started_at_unix_nanos
        {
            return Err(BatchError::InvalidPlan(
                "idempotency expiry must be later than request start".to_owned(),
            ));
        }

        let mut record_keys = BTreeSet::new();
        for mutation in &self.records {
            mutation.payload().validate().map_err(BatchError::Sdk)?;
            if mutation.payload().owner != self.context.module_id {
                return Err(BatchError::InvalidPlan(format!(
                    "record {} payload owner does not match executing module",
                    mutation.reference().record_id
                )));
            }
            if matches!(
                mutation,
                RecordMutation::Update {
                    expected_version,
                    ..
                } if *expected_version <= 0
            ) {
                return Err(BatchError::InvalidPlan(
                    "record update expected_version must be positive".to_owned(),
                ));
            }
            let key = format!(
                "{}:{}",
                mutation.reference().record_type,
                mutation.reference().record_id
            );
            if !record_keys.insert(key.clone()) {
                return Err(BatchError::InvalidPlan(format!(
                    "record {key} is mutated more than once in one batch"
                )));
            }
        }

        let mut relationship_keys = BTreeSet::new();
        for mutation in &self.relationships {
            if let RelationshipMutation::Link { payload, .. } = mutation {
                payload.validate().map_err(BatchError::Sdk)?;
                if payload.owner != self.context.module_id {
                    return Err(BatchError::InvalidPlan(
                        "relationship payload owner does not match executing module".to_owned(),
                    ));
                }
            }
            let relationship = mutation.relationship();
            let key = relationship_key(relationship);
            if !relationship_keys.insert(key.clone()) {
                return Err(BatchError::InvalidPlan(format!(
                    "relationship {key} is mutated more than once in one batch"
                )));
            }
        }

        let mut event_ids = BTreeSet::new();
        let mut deduplication_keys = BTreeSet::new();
        for evidence in &self.events {
            evidence.event.payload.validate().map_err(BatchError::Sdk)?;
            if evidence.event.payload.owner != self.context.module_id {
                return Err(BatchError::InvalidPlan(
                    "event payload owner does not match executing module".to_owned(),
                ));
            }
            if evidence.event_id.is_empty()
                || evidence.event.deduplication_key.is_empty()
                || evidence.aggregate_version <= 0
                || evidence.event_sequence <= 0
            {
                return Err(BatchError::InvalidPlan(
                    "event identifiers and versions must be positive and non-empty".to_owned(),
                ));
            }
            if !event_ids.insert(evidence.event_id.clone()) {
                return Err(BatchError::InvalidPlan(format!(
                    "duplicate event id {}",
                    evidence.event_id
                )));
            }
            let deduplication_key = format!(
                "{}:{}",
                evidence.event.event_type, evidence.event.deduplication_key
            );
            if !deduplication_keys.insert(deduplication_key.clone()) {
                return Err(BatchError::InvalidPlan(format!(
                    "duplicate event deduplication identity {deduplication_key}"
                )));
            }
        }

        let mut audit_ids = BTreeSet::new();
        for (index, audit) in self.audits.iter().enumerate() {
            if audit.audit_record_id.is_empty()
                || audit.canonicalization_profile.is_empty()
                || audit.audit_sequence <= 0
                || audit.record_hash.iter().all(|byte| *byte == 0)
            {
                return Err(BatchError::InvalidPlan(
                    "audit identifiers, sequence and hash must be valid".to_owned(),
                ));
            }
            if !audit_ids.insert(audit.audit_record_id.clone()) {
                return Err(BatchError::InvalidPlan(format!(
                    "duplicate audit record id {}",
                    audit.audit_record_id
                )));
            }
            if let Some(previous) = index
                .checked_sub(1)
                .and_then(|value| self.audits.get(value))
            {
                if audit.audit_sequence != previous.audit_sequence + 1 {
                    return Err(BatchError::InvalidPlan(
                        "audit records in a batch must use contiguous sequences".to_owned(),
                    ));
                }
                if audit.previous_hash != previous.record_hash {
                    return Err(BatchError::InvalidPlan(
                        "audit records in a batch must form a continuous hash chain".to_owned(),
                    ));
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BatchMutationResult {
    pub records: Vec<RecordSnapshot>,
    pub linked_relationships: Vec<RelationshipRef>,
    pub unlinked_relationships: Vec<RelationshipRef>,
    pub replayed: bool,
}

#[derive(Debug)]
pub enum BatchError {
    Database(sqlx::Error),
    Sdk(SdkError),
    InvalidPlan(String),
    Conflict(String),
    IdempotencyKeyReused,
    IdempotencyInProgress,
    InvalidStoredValue(String),
}

impl fmt::Display for BatchError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Database(error) => write!(formatter, "database operation failed: {error}"),
            Self::Sdk(error) => write!(formatter, "SDK validation failed: {error}"),
            Self::InvalidPlan(message) => write!(formatter, "invalid batch plan: {message}"),
            Self::Conflict(message) => write!(formatter, "mutation conflict: {message}"),
            Self::IdempotencyKeyReused => formatter
                .write_str("idempotency key was previously used for a different semantic request"),
            Self::IdempotencyInProgress => {
                formatter.write_str("idempotent request is already in progress")
            }
            Self::InvalidStoredValue(message) => {
                write!(formatter, "invalid stored idempotency response: {message}")
            }
        }
    }
}

impl Error for BatchError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Database(error) => Some(error),
            Self::Sdk(error) => Some(error),
            Self::InvalidPlan(_)
            | Self::Conflict(_)
            | Self::IdempotencyKeyReused
            | Self::IdempotencyInProgress
            | Self::InvalidStoredValue(_) => None,
        }
    }
}

impl From<sqlx::Error> for BatchError {
    fn from(value: sqlx::Error) -> Self {
        Self::Database(value)
    }
}

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
    if stored_hash.as_slice() != plan.idempotency.request_hash {
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
    if schema_id.as_deref() != Some(BATCH_RESULT_SCHEMA_ID)
        || schema_version.as_deref() != Some(BATCH_RESULT_SCHEMA_VERSION)
        || descriptor_hash.as_deref() != Some(batch_result_descriptor_hash().as_slice())
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
    audit: &AuditEvidence,
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

async fn complete_idempotency(
    transaction: &mut Transaction<'_, Postgres>,
    plan: &BatchMutationPlan,
    result: &BatchMutationResult,
) -> Result<(), BatchError> {
    let response = serde_json::to_vec(result).map_err(|error| {
        BatchError::InvalidPlan(format!("batch response serialization failed: {error}"))
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
            "idempotency claim disappeared before completion".to_owned(),
        ));
    }
    Ok(())
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

fn checked_size(value: u64, label: &str) -> Result<i64, BatchError> {
    i64::try_from(value).map_err(|_| BatchError::InvalidPlan(format!("{label} size exceeds i64")))
}

fn relationship_key(relationship: &RelationshipRef) -> String {
    format!(
        "{}:{}:{}->{}:{}",
        relationship.relationship_type,
        relationship.source.record_type,
        relationship.source.record_id,
        relationship.target.record_type,
        relationship.target.record_id
    )
}

fn batch_result_descriptor_hash() -> [u8; 32] {
    Sha256::digest(BATCH_RESULT_SCHEMA_DESCRIPTOR).into()
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

const fn payload_encoding_name(value: PayloadEncoding) -> &'static str {
    match value {
        PayloadEncoding::Protobuf => "protobuf",
        PayloadEncoding::Json => "json",
        PayloadEncoding::Utf8Text => "utf8_text",
        PayloadEncoding::Binary => "binary",
    }
}

pub fn batch_error_to_sdk(error: BatchError) -> SdkError {
    match error {
        BatchError::Sdk(error) => error,
        BatchError::InvalidPlan(message) | BatchError::InvalidStoredValue(message) => {
            SdkError::new(
                "DATA_INVALID",
                ErrorCategory::InvalidArgument,
                false,
                message,
            )
        }
        BatchError::Conflict(message) | BatchError::IdempotencyKeyReused => SdkError::new(
            "DATA_CONFLICT",
            ErrorCategory::Conflict,
            false,
            match error {
                BatchError::IdempotencyKeyReused => {
                    "The idempotency key was used for a different request.".to_owned()
                }
                _ => message,
            },
        ),
        BatchError::IdempotencyInProgress => SdkError::new(
            "DATA_IDEMPOTENCY_IN_PROGRESS",
            ErrorCategory::Conflict,
            true,
            "The same request is already being processed.",
        ),
        BatchError::Database(error) => SdkError::new(
            "DATA_UNAVAILABLE",
            ErrorCategory::Unavailable,
            true,
            "The data service is temporarily unavailable.",
        )
        .with_internal_reference(error.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crm_module_sdk::{
        ActorId, BusinessTransactionId, CapabilityId, CapabilityVersion, CausationId,
        CorrelationId, EventType, ExecutionContext, IdempotencyKey, RecordId, RecordType,
        RequestId, StateKey, TenantId, TraceId,
    };

    fn context() -> ModuleExecutionContext {
        ModuleExecutionContext {
            module_id: ModuleId::try_new("crm.sales").unwrap(),
            execution: ExecutionContext {
                tenant_id: TenantId::try_new("tenant-a").unwrap(),
                actor_id: ActorId::try_new("actor-a").unwrap(),
                request_id: RequestId::try_new("request-a").unwrap(),
                correlation_id: CorrelationId::try_new("correlation-a").unwrap(),
                causation_id: CausationId::try_new("causation-a").unwrap(),
                trace_id: TraceId::try_new("trace-a").unwrap(),
                capability_id: CapabilityId::try_new("sales.batch").unwrap(),
                capability_version: CapabilityVersion::try_new("1.0.0").unwrap(),
                idempotency_key: IdempotencyKey::try_new("idem-a").unwrap(),
                business_transaction_id: BusinessTransactionId::try_new("tx-a").unwrap(),
                schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
                request_started_at_unix_nanos: 1,
            },
        }
    }

    fn payload() -> TypedPayload {
        TypedPayload {
            owner: ModuleId::try_new("crm.sales").unwrap(),
            schema_id: SchemaId::try_new("sales.deal.v1").unwrap(),
            schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
            descriptor_hash: [1; 32],
            data_class: DataClass::Internal,
            encoding: PayloadEncoding::Protobuf,
            maximum_size_bytes: 16,
            retention_policy_id: RetentionPolicyId::try_new("standard").unwrap(),
            bytes: vec![1],
        }
    }

    #[test]
    fn rejects_duplicate_record_mutations() {
        let reference = RecordRef {
            record_type: RecordType::try_new("sales.deal").unwrap(),
            record_id: RecordId::try_new("deal-1").unwrap(),
        };
        let plan = BatchMutationPlan {
            context: context(),
            records: vec![
                RecordMutation::Create {
                    reference: reference.clone(),
                    payload: payload(),
                },
                RecordMutation::Update {
                    reference: reference.clone(),
                    expected_version: 1,
                    payload: payload(),
                },
            ],
            relationships: Vec::new(),
            events: vec![EventEvidence {
                event_id: "event-1".to_owned(),
                event: DomainEvent {
                    event_type: EventType::try_new("sales.deal.created").unwrap(),
                    aggregate: reference,
                    expected_aggregate_version: None,
                    deduplication_key: "deal-1-created".to_owned(),
                    payload: payload(),
                },
                aggregate_version: 1,
                event_sequence: 1,
                occurred_at_unix_nanos: 2,
            }],
            idempotency: IdempotencyEvidence {
                scope: "sales.batch@1.0.0".to_owned(),
                key: "idem-a".to_owned(),
                request_hash: [2; 32],
                expires_at_unix_nanos: 10,
            },
            audits: vec![AuditEvidence {
                audit_sequence: 1,
                audit_record_id: "audit-1".to_owned(),
                canonicalization_profile: "crm.cjson/v1".to_owned(),
                previous_hash: [0; 32],
                record_hash: [3; 32],
                canonical_envelope: vec![1],
                occurred_at_unix_nanos: 2,
            }],
        };
        assert!(matches!(plan.validate(), Err(BatchError::InvalidPlan(_))));
        let _ = StateKey::try_new("type-import-guard").unwrap();
    }
}
