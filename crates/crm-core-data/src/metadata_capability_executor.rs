use crate::audit::{
    AuditIntent, AuditMaterializationError, materialize_audit_chain,
};
use crate::capability_executor::{
    capability_batch_error_to_sdk, capability_idempotency_scope,
};
use crate::metadata_store::{
    MetadataPersistenceError, MetadataTransitionAction, MetadataTransitionWrite,
    delete_rollback_stack_entry, insert_documents, insert_rollback_stack_entry,
    insert_transition, load_bundle, load_impact, load_rollback_target, load_state,
    lock_activation, require_generation, upsert_activation_head,
};
use crate::postgres_batch::{
    BatchMutationPlan, bind_execution_context, capability_idempotency,
    complete_capability_idempotency, insert_audit_record, insert_completion_marker,
    insert_idempotency_claim, load_capability_replay,
};
use crate::{BatchError, PostgresDataStore};
use crm_capability_runtime::{
    CapabilityDefinition, CapabilityExecutionResult, CapabilityRequest,
    TransactionalCapabilityExecutor,
};
use crm_metadata_api_adapter::{
    ACTIVATE_REQUEST_SCHEMA, ACTIVATE_RESPONSE_SCHEMA, ACTIVATE_REVISION_CAPABILITY,
    METADATA_MODULE_ID, METADATA_MUTATION_CAPABILITY_IDS, PUBLISH_BUNDLE_CAPABILITY,
    PUBLISH_REQUEST_SCHEMA, PUBLISH_RESPONSE_SCHEMA, ROLLBACK_REQUEST_SCHEMA,
    ROLLBACK_RESPONSE_SCHEMA, ROLLBACK_REVISION_CAPABILITY, activation_state_to_wire,
    decode_request, impact_to_wire, metadata_capability_definition, parse_revision_id,
    protobuf_payload, publish_bundle_from_wire,
};
use crm_metadata_runtime::{
    MetadataBundleDraft, MetadataImpactReport, MetadataRevisionId, TenantMetadataSnapshot,
};
use crm_module_sdk::{
    DataClass, ErrorCategory, PortFuture, ResourceRef, SdkError, TypedPayload,
};
use crm_proto_contracts::crm::metadata::v1 as wire;
use sha2::{Digest, Sha256};
use sqlx::{Postgres, Transaction};
use std::collections::BTreeMap;

const METADATA_AUDIT_ID_DOMAIN: &[u8] = b"crm.metadata.audit-id.sha256/v1";

#[derive(Debug, Clone)]
pub struct PostgresMetadataCapabilityExecutor {
    store: PostgresDataStore,
}

impl PostgresMetadataCapabilityExecutor {
    pub const fn new(store: PostgresDataStore) -> Self {
        Self { store }
    }
}

impl TransactionalCapabilityExecutor for PostgresMetadataCapabilityExecutor {
    fn execute<'a>(
        &'a self,
        definition: &'a CapabilityDefinition,
        request: CapabilityRequest,
    ) -> PortFuture<'a, Result<CapabilityExecutionResult, SdkError>> {
        Box::pin(async move {
            let command = decode_metadata_command(definition, &request)?;
            let idempotency = capability_idempotency(
                &request,
                capability_idempotency_scope(definition),
            )
            .map_err(capability_batch_error_to_sdk)?;

            let mut transaction = self
                .store
                .pool()
                .begin()
                .await
                .map_err(|error| capability_batch_error_to_sdk(BatchError::Database(error)))?;
            bind_execution_context(&mut transaction, &request.context)
                .await
                .map_err(capability_batch_error_to_sdk)?;

            if let Some(result) = load_capability_replay(
                &mut transaction,
                &request.context,
                &idempotency,
            )
            .await
            .map_err(capability_batch_error_to_sdk)?
            {
                transaction
                    .commit()
                    .await
                    .map_err(|error| capability_batch_error_to_sdk(BatchError::Database(error)))?;
                return Ok(result);
            }

            insert_idempotency_claim(&mut transaction, &request.context, &idempotency)
                .await
                .map_err(capability_batch_error_to_sdk)?;

            let applied = apply_metadata_command(&mut transaction, &request, command)
                .await
                .map_err(metadata_persistence_error_to_sdk)?;
            let audit = metadata_audit_intent(&request, &applied)?;
            let plan = BatchMutationPlan {
                context: request.context.clone(),
                records: Vec::new(),
                relationships: Vec::new(),
                events: Vec::new(),
                idempotency,
                audits: vec![audit],
            };

            let materialized = materialize_audit_chain(
                &mut transaction,
                &plan.context,
                &plan.audits,
            )
            .await
            .map_err(audit_materialization_to_batch_error)
            .map_err(capability_batch_error_to_sdk)?;
            for audit in &materialized {
                insert_audit_record(&mut transaction, &plan.context, audit)
                    .await
                    .map_err(capability_batch_error_to_sdk)?;
            }

            let result = CapabilityExecutionResult {
                output: Some(applied.output),
                affected_resources: applied.affected_resources,
                replayed: false,
            };
            complete_capability_idempotency(&mut transaction, &plan, &result)
                .await
                .map_err(capability_batch_error_to_sdk)?;
            insert_completion_marker(&mut transaction, &plan)
                .await
                .map_err(capability_batch_error_to_sdk)?;
            transaction
                .commit()
                .await
                .map_err(|error| capability_batch_error_to_sdk(BatchError::Database(error)))?;
            Ok(result)
        })
    }
}

#[derive(Debug)]
enum MetadataCommand {
    Publish(MetadataBundleDraft),
    Activate {
        revision_id: MetadataRevisionId,
        expected_generation: u64,
        confirm_breaking_changes: bool,
    },
    Rollback {
        expected_generation: u64,
    },
}

fn decode_metadata_command(
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
) -> Result<MetadataCommand, SdkError> {
    let capability_id = definition.capability_id.as_str();
    if !METADATA_MUTATION_CAPABILITY_IDS.contains(&capability_id) {
        return Err(metadata_configuration_error(
            "METADATA_MUTATION_ROUTE_UNSUPPORTED",
            "The metadata mutation route is unsupported.",
        ));
    }
    let expected = metadata_capability_definition(capability_id)?;
    if definition != &expected
        || request.context.module_id != expected.owner_module_id
        || request.context.execution.capability_id != expected.capability_id
        || request.context.execution.capability_version != expected.capability_version
        || !expected.input_contract.matches(&request.input)
    {
        return Err(metadata_configuration_error(
            "METADATA_MUTATION_BINDING_MISMATCH",
            "The metadata mutation binding is invalid.",
        ));
    }

    match capability_id {
        PUBLISH_BUNDLE_CAPABILITY => {
            let command: wire::PublishMetadataBundleRequest =
                decode_request(request, METADATA_MODULE_ID, PUBLISH_REQUEST_SCHEMA)?;
            Ok(MetadataCommand::Publish(publish_bundle_from_wire(command)?))
        }
        ACTIVATE_REVISION_CAPABILITY => {
            let command: wire::ActivateMetadataRevisionRequest =
                decode_request(request, METADATA_MODULE_ID, ACTIVATE_REQUEST_SCHEMA)?;
            Ok(MetadataCommand::Activate {
                revision_id: parse_revision_id(&command.revision_id, "revision_id")?,
                expected_generation: command.expected_generation,
                confirm_breaking_changes: command.confirm_breaking_changes,
            })
        }
        ROLLBACK_REVISION_CAPABILITY => {
            let command: wire::RollbackMetadataRevisionRequest =
                decode_request(request, METADATA_MODULE_ID, ROLLBACK_REQUEST_SCHEMA)?;
            Ok(MetadataCommand::Rollback {
                expected_generation: command.expected_generation,
            })
        }
        _ => Err(metadata_configuration_error(
            "METADATA_MUTATION_ROUTE_UNSUPPORTED",
            "The metadata mutation route is unsupported.",
        )),
    }
}

struct AppliedMetadataMutation {
    output: TypedPayload,
    affected_resources: Vec<ResourceRef>,
    audit_resource_type: &'static str,
    audit_resource_id: String,
    audit_version: i64,
}

async fn apply_metadata_command(
    transaction: &mut Transaction<'_, Postgres>,
    request: &CapabilityRequest,
    command: MetadataCommand,
) -> Result<AppliedMetadataMutation, MetadataPersistenceError> {
    let occurred_at_unix_nanos = request.context.execution.request_started_at_unix_nanos;
    if occurred_at_unix_nanos <= 0 {
        return Err(MetadataPersistenceError::InvalidInput(
            "metadata mutation occurrence time must be positive".to_owned(),
        ));
    }

    match command {
        MetadataCommand::Publish(draft) => {
            apply_publish(transaction, request, &draft, occurred_at_unix_nanos).await
        }
        MetadataCommand::Activate {
            revision_id,
            expected_generation,
            confirm_breaking_changes,
        } => {
            apply_activate(
                transaction,
                request,
                &revision_id,
                expected_generation,
                confirm_breaking_changes,
                occurred_at_unix_nanos,
            )
            .await
        }
        MetadataCommand::Rollback {
            expected_generation,
        } => {
            apply_rollback(
                transaction,
                request,
                expected_generation,
                occurred_at_unix_nanos,
            )
            .await
        }
    }
}

async fn apply_publish(
    transaction: &mut Transaction<'_, Postgres>,
    request: &CapabilityRequest,
    draft: &MetadataBundleDraft,
    occurred_at_unix_nanos: i64,
) -> Result<AppliedMetadataMutation, MetadataPersistenceError> {
    let revision_id = draft.revision_id();
    let document_count = i32::try_from(draft.documents().len()).map_err(|_| {
        MetadataPersistenceError::InvalidInput(
            "metadata document count exceeds PostgreSQL integer range".to_owned(),
        )
    })?;
    let context = &request.context;
    let inserted = sqlx::query(
        r#"
        INSERT INTO crm.metadata_revisions_v2 (
          tenant_id,
          revision_id,
          document_count,
          published_by_actor_id,
          business_transaction_id,
          published_at
        )
        VALUES (
          $1, $2, $3, $4, $5,
          TIMESTAMPTZ 'epoch' + ($6::bigint / 1000) * INTERVAL '1 microsecond'
        )
        ON CONFLICT (tenant_id, revision_id) DO NOTHING
        RETURNING revision_id
        "#,
    )
    .bind(context.execution.tenant_id.as_str())
    .bind(revision_id.as_bytes().as_slice())
    .bind(document_count)
    .bind(context.execution.actor_id.as_str())
    .bind(context.execution.business_transaction_id.as_str())
    .bind(occurred_at_unix_nanos)
    .fetch_optional(&mut **transaction)
    .await?
    .is_some();

    if inserted {
        insert_documents(transaction, context, &revision_id, draft).await?;
        let state = load_state(transaction, &context.execution.tenant_id, false).await?;
        insert_transition(
            transaction,
            context,
            MetadataTransitionWrite {
                action: MetadataTransitionAction::Publish,
                generation: state.generation,
                rollback_depth: state.rollback_depth,
                from_revision: None,
                to_revision: &revision_id,
                occurred_at_unix_nanos,
            },
        )
        .await?;
    } else {
        let existing = load_bundle(transaction, &context.execution.tenant_id, &revision_id)
            .await?
            .ok_or_else(|| {
                MetadataPersistenceError::InvalidStoredValue(
                    "metadata revision header exists without a readable bundle".to_owned(),
                )
            })?;
        if existing.documents() != draft.documents() {
            return Err(MetadataPersistenceError::RevisionIdentityCollision(revision_id));
        }
    }

    let state = load_state(transaction, &context.execution.tenant_id, false).await?;
    let output = protobuf_payload(
        METADATA_MODULE_ID,
        PUBLISH_RESPONSE_SCHEMA,
        DataClass::Confidential,
        &wire::PublishMetadataBundleResponse {
            revision_id: revision_id.to_hex(),
            newly_published: inserted,
        },
    )
    .map_err(MetadataPersistenceError::Sdk)?;
    Ok(AppliedMetadataMutation {
        output,
        affected_resources: vec![revision_resource(&revision_id)],
        audit_resource_type: "metadata.revision",
        audit_resource_id: revision_id.to_hex(),
        audit_version: generation_i64(state.generation)?,
    })
}

async fn apply_activate(
    transaction: &mut Transaction<'_, Postgres>,
    request: &CapabilityRequest,
    candidate_revision: &MetadataRevisionId,
    expected_generation: u64,
    confirm_breaking_changes: bool,
    occurred_at_unix_nanos: i64,
) -> Result<AppliedMetadataMutation, MetadataPersistenceError> {
    let context = &request.context;
    lock_activation(transaction, &context.execution.tenant_id).await?;
    let state = load_state(transaction, &context.execution.tenant_id, true).await?;
    require_generation(expected_generation, state.generation)?;
    let impact = load_impact(
        transaction,
        &context.execution.tenant_id,
        state.active_revision.as_ref(),
        candidate_revision,
    )
    .await?;

    let next_state = if state.active_revision.as_ref() == Some(candidate_revision) {
        state
    } else {
        if impact.has_breaking_changes() && !confirm_breaking_changes {
            return Err(MetadataPersistenceError::BreakingChangeConfirmationRequired(
                candidate_revision.clone(),
            ));
        }
        let previous_revision = state.active_revision.clone();
        let next_generation = state.generation.checked_add(1).ok_or_else(|| {
            MetadataPersistenceError::InvalidStoredValue(
                "metadata activation generation overflowed u64".to_owned(),
            )
        })?;
        let next_depth = if let Some(previous) = previous_revision.as_ref() {
            let depth = state.rollback_depth.checked_add(1).ok_or_else(|| {
                MetadataPersistenceError::InvalidStoredValue(
                    "metadata rollback depth overflowed usize".to_owned(),
                )
            })?;
            insert_rollback_stack_entry(
                transaction,
                context,
                depth,
                previous,
                next_generation,
            )
            .await?;
            depth
        } else {
            state.rollback_depth
        };
        upsert_activation_head(
            transaction,
            context,
            next_generation,
            candidate_revision,
            next_depth,
        )
        .await?;
        insert_transition(
            transaction,
            context,
            MetadataTransitionWrite {
                action: MetadataTransitionAction::Activate,
                generation: next_generation,
                rollback_depth: next_depth,
                from_revision: previous_revision.as_ref(),
                to_revision: candidate_revision,
                occurred_at_unix_nanos,
            },
        )
        .await?;
        TenantMetadataSnapshot {
            generation: next_generation,
            active_revision: Some(candidate_revision.clone()),
            rollback_depth: next_depth,
        }
    };

    activation_applied(candidate_revision, next_state, impact)
}

fn activation_applied(
    candidate_revision: &MetadataRevisionId,
    state: TenantMetadataSnapshot,
    impact: MetadataImpactReport,
) -> Result<AppliedMetadataMutation, MetadataPersistenceError> {
    let output = protobuf_payload(
        METADATA_MODULE_ID,
        ACTIVATE_RESPONSE_SCHEMA,
        DataClass::Confidential,
        &wire::ActivateMetadataRevisionResponse {
            state: Some(activation_state_to_wire(&state)),
            impact: Some(impact_to_wire(&impact)),
        },
    )
    .map_err(MetadataPersistenceError::Sdk)?;
    Ok(AppliedMetadataMutation {
        output,
        affected_resources: vec![
            activation_resource(state.generation)?,
            revision_resource(candidate_revision),
        ],
        audit_resource_type: "metadata.activation",
        audit_resource_id: candidate_revision.to_hex(),
        audit_version: generation_i64(state.generation)?,
    })
}

async fn apply_rollback(
    transaction: &mut Transaction<'_, Postgres>,
    request: &CapabilityRequest,
    expected_generation: u64,
    occurred_at_unix_nanos: i64,
) -> Result<AppliedMetadataMutation, MetadataPersistenceError> {
    let context = &request.context;
    lock_activation(transaction, &context.execution.tenant_id).await?;
    let state = load_state(transaction, &context.execution.tenant_id, true).await?;
    require_generation(expected_generation, state.generation)?;
    let replaced_revision = state
        .active_revision
        .clone()
        .ok_or(MetadataPersistenceError::RollbackUnavailable)?;
    if state.rollback_depth == 0 {
        return Err(MetadataPersistenceError::RollbackUnavailable);
    }
    let target_revision = load_rollback_target(
        transaction,
        &context.execution.tenant_id,
        state.rollback_depth,
    )
    .await?
    .ok_or_else(|| {
        MetadataPersistenceError::InvalidStoredValue(
            "metadata rollback depth has no matching stack entry".to_owned(),
        )
    })?;
    let next_generation = state.generation.checked_add(1).ok_or_else(|| {
        MetadataPersistenceError::InvalidStoredValue(
            "metadata activation generation overflowed u64".to_owned(),
        )
    })?;
    let next_depth = state.rollback_depth - 1;
    delete_rollback_stack_entry(transaction, context, state.rollback_depth).await?;
    upsert_activation_head(
        transaction,
        context,
        next_generation,
        &target_revision,
        next_depth,
    )
    .await?;
    insert_transition(
        transaction,
        context,
        MetadataTransitionWrite {
            action: MetadataTransitionAction::Rollback,
            generation: next_generation,
            rollback_depth: next_depth,
            from_revision: Some(&replaced_revision),
            to_revision: &target_revision,
            occurred_at_unix_nanos,
        },
    )
    .await?;

    let next_state = TenantMetadataSnapshot {
        generation: next_generation,
        active_revision: Some(target_revision.clone()),
        rollback_depth: next_depth,
    };
    let output = protobuf_payload(
        METADATA_MODULE_ID,
        ROLLBACK_RESPONSE_SCHEMA,
        DataClass::Confidential,
        &wire::RollbackMetadataRevisionResponse {
            state: Some(activation_state_to_wire(&next_state)),
        },
    )
    .map_err(MetadataPersistenceError::Sdk)?;
    Ok(AppliedMetadataMutation {
        output,
        affected_resources: vec![
            activation_resource(next_generation)?,
            revision_resource(&target_revision),
        ],
        audit_resource_type: "metadata.activation",
        audit_resource_id: target_revision.to_hex(),
        audit_version: generation_i64(next_generation)?,
    })
}

fn revision_resource(revision_id: &MetadataRevisionId) -> ResourceRef {
    ResourceRef {
        resource_type: "metadata.revision".to_owned(),
        resource_id: revision_id.to_hex(),
        version: None,
    }
}

fn activation_resource(generation: u64) -> Result<ResourceRef, MetadataPersistenceError> {
    Ok(ResourceRef {
        resource_type: "metadata.activation".to_owned(),
        resource_id: "active".to_owned(),
        version: Some(generation_i64(generation)?),
    })
}

fn generation_i64(generation: u64) -> Result<i64, MetadataPersistenceError> {
    i64::try_from(generation).map_err(|_| {
        MetadataPersistenceError::InvalidStoredValue(
            "metadata generation exceeds PostgreSQL integer range".to_owned(),
        )
    })
}

fn metadata_audit_intent(
    request: &CapabilityRequest,
    applied: &AppliedMetadataMutation,
) -> Result<AuditIntent, SdkError> {
    let mut envelope = BTreeMap::new();
    envelope.insert(
        "actor_id",
        request.context.execution.actor_id.as_str().to_owned(),
    );
    envelope.insert("aggregate_id", applied.audit_resource_id.clone());
    envelope.insert(
        "aggregate_type",
        applied.audit_resource_type.to_owned(),
    );
    envelope.insert("aggregate_version", applied.audit_version.to_string());
    envelope.insert(
        "capability_id",
        request.context.execution.capability_id.as_str().to_owned(),
    );
    envelope.insert(
        "capability_version",
        request
            .context
            .execution
            .capability_version
            .as_str()
            .to_owned(),
    );
    envelope.insert(
        "operation",
        request.context.execution.capability_id.as_str().to_owned(),
    );
    envelope.insert("request_hash", hex(&request.input_hash));
    envelope.insert("result_hash", sha256_hex(&applied.output.bytes));
    envelope.insert(
        "tenant_id",
        request.context.execution.tenant_id.as_str().to_owned(),
    );
    envelope.insert(
        "transaction_id",
        request
            .context
            .execution
            .business_transaction_id
            .as_str()
            .to_owned(),
    );
    let canonical_envelope = serde_json::to_vec(&envelope).map_err(|error| {
        SdkError::new(
            "METADATA_AUDIT_ENVELOPE_SERIALIZATION_FAILED",
            ErrorCategory::Internal,
            false,
            "The metadata audit evidence could not be produced.",
        )
        .with_internal_reference(error.to_string())
    })?;

    Ok(AuditIntent {
        audit_record_id: metadata_audit_record_id(request, &applied.audit_resource_id),
        canonicalization_profile: "crm.cjson/v1".to_owned(),
        canonical_envelope,
        occurred_at_unix_nanos: request.context.execution.request_started_at_unix_nanos,
    })
}

fn metadata_audit_record_id(request: &CapabilityRequest, resource_id: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(METADATA_AUDIT_ID_DOMAIN);
    hasher.update(request.context.execution.tenant_id.as_str().as_bytes());
    hasher.update(
        request
            .context
            .execution
            .business_transaction_id
            .as_str()
            .as_bytes(),
    );
    hasher.update(request.context.execution.capability_id.as_str().as_bytes());
    hasher.update(resource_id.as_bytes());
    format!("metadata-audit-{}", hex(&hasher.finalize()))
}

fn sha256_hex(value: &[u8]) -> String {
    hex(&Sha256::digest(value))
}

fn hex(value: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(value.len() * 2);
    for byte in value {
        output.push(DIGITS[usize::from(byte >> 4)] as char);
        output.push(DIGITS[usize::from(byte & 0x0f)] as char);
    }
    output
}

fn audit_materialization_to_batch_error(error: AuditMaterializationError) -> BatchError {
    match error {
        AuditMaterializationError::Database(error) => BatchError::Database(error),
        AuditMaterializationError::InvalidIntent(message) => BatchError::InvalidPlan(message),
        AuditMaterializationError::InvalidStoredValue(message) => {
            BatchError::InvalidStoredValue(message)
        }
    }
}

fn metadata_persistence_error_to_sdk(error: MetadataPersistenceError) -> SdkError {
    match error {
        MetadataPersistenceError::Sdk(error) => error,
        MetadataPersistenceError::RevisionNotFound(revision_id) => SdkError::new(
            "METADATA_REVISION_NOT_FOUND",
            ErrorCategory::NotFound,
            false,
            "The requested metadata revision does not exist.",
        )
        .with_internal_reference(revision_id.to_hex()),
        MetadataPersistenceError::GenerationConflict { expected, actual } => SdkError::new(
            "METADATA_GENERATION_CONFLICT",
            ErrorCategory::Conflict,
            false,
            "The metadata activation state changed before the request completed.",
        )
        .with_internal_reference(format!("expected={expected};actual={actual}")),
        MetadataPersistenceError::BreakingChangeConfirmationRequired(revision_id) => SdkError::new(
            "METADATA_BREAKING_CHANGE_CONFIRMATION_REQUIRED",
            ErrorCategory::Conflict,
            false,
            "Breaking metadata changes require explicit confirmation.",
        )
        .with_internal_reference(revision_id.to_hex()),
        MetadataPersistenceError::RollbackUnavailable => SdkError::new(
            "METADATA_ROLLBACK_UNAVAILABLE",
            ErrorCategory::Conflict,
            false,
            "No previous metadata revision is available for rollback.",
        ),
        MetadataPersistenceError::Database(error) => SdkError::new(
            "METADATA_STORAGE_UNAVAILABLE",
            ErrorCategory::Unavailable,
            true,
            "The metadata change could not be persisted at this time.",
        )
        .with_internal_reference(error.to_string()),
        MetadataPersistenceError::Runtime(error) => SdkError::new(
            "METADATA_MUTATION_INVALID",
            ErrorCategory::InvalidArgument,
            false,
            "The metadata change is invalid.",
        )
        .with_internal_reference(error.to_string()),
        MetadataPersistenceError::InvalidInput(message) => SdkError::new(
            "METADATA_MUTATION_INVALID",
            ErrorCategory::InvalidArgument,
            false,
            "The metadata change is invalid.",
        )
        .with_internal_reference(message),
        MetadataPersistenceError::RevisionIdentityCollision(revision_id) => SdkError::new(
            "METADATA_REVISION_IDENTITY_COLLISION",
            ErrorCategory::Internal,
            false,
            "Stored metadata failed identity validation.",
        )
        .with_internal_reference(revision_id.to_hex()),
        MetadataPersistenceError::InvalidStoredValue(message) => SdkError::new(
            "METADATA_STORED_VALUE_INVALID",
            ErrorCategory::Internal,
            false,
            "Stored metadata failed integrity validation.",
        )
        .with_internal_reference(message),
    }
}

fn metadata_configuration_error(code: &'static str, safe_message: &'static str) -> SdkError {
    SdkError::new(code, ErrorCategory::Internal, false, safe_message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crm_metadata_schema::METADATA_DEFINITION_SCHEMA_VERSION;
    use crm_module_sdk::{
        ActorId, BusinessTransactionId, CapabilityId, CapabilityVersion, CausationId,
        CorrelationId, ExecutionContext, IdempotencyKey, ModuleExecutionContext, ModuleId,
        RequestId, SchemaVersion, TenantId, TraceId,
    };

    fn request(capability_id: &str, input: TypedPayload) -> CapabilityRequest {
        CapabilityRequest {
            context: ModuleExecutionContext {
                module_id: ModuleId::try_new(METADATA_MODULE_ID).unwrap(),
                execution: ExecutionContext {
                    tenant_id: TenantId::try_new("tenant-a").unwrap(),
                    actor_id: ActorId::try_new("actor-a").unwrap(),
                    request_id: RequestId::try_new("request-a").unwrap(),
                    correlation_id: CorrelationId::try_new("correlation-a").unwrap(),
                    causation_id: CausationId::try_new("causation-a").unwrap(),
                    trace_id: TraceId::try_new("trace-a").unwrap(),
                    capability_id: CapabilityId::try_new(capability_id).unwrap(),
                    capability_version: CapabilityVersion::try_new("1.0.0").unwrap(),
                    idempotency_key: IdempotencyKey::try_new("idem-a").unwrap(),
                    business_transaction_id: BusinessTransactionId::try_new("tx-a").unwrap(),
                    schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
                    request_started_at_unix_nanos: 1_000_000_000,
                },
            },
            input_hash: [7; 32],
            input,
            approval: None,
        }
    }

    #[test]
    fn strict_publish_command_is_decoded_before_any_database_work() {
        let definition = metadata_capability_definition(PUBLISH_BUNDLE_CAPABILITY).unwrap();
        let input = protobuf_payload(
            METADATA_MODULE_ID,
            PUBLISH_REQUEST_SCHEMA,
            DataClass::Confidential,
            &wire::PublishMetadataBundleRequest {
                definitions: vec![wire::MetadataDefinitionInput {
                    schema_version: METADATA_DEFINITION_SCHEMA_VERSION.to_owned(),
                    definition_json: br#"{"kind":"object","definition":{"id":"crm.sales.deal","owner_module_id":"crm.sales","label":"Deal","plural_label":"Deals","description":null,"tags":[]}}"#.to_vec(),
                }],
            },
        )
        .unwrap();
        let command = decode_metadata_command(
            &definition,
            &request(PUBLISH_BUNDLE_CAPABILITY, input),
        )
        .unwrap();

        assert!(matches!(command, MetadataCommand::Publish(_)));
    }

    #[test]
    fn mutation_executor_rejects_query_coordinates() {
        let definition = metadata_capability_definition("metadata.activation.get").unwrap();
        let input = protobuf_payload(
            METADATA_MODULE_ID,
            "crm.metadata.v1.GetMetadataActivationRequest",
            DataClass::Confidential,
            &wire::GetMetadataActivationRequest {},
        )
        .unwrap();
        let error = decode_metadata_command(
            &definition,
            &request("metadata.activation.get", input),
        )
        .unwrap_err();

        assert_eq!(error.code, "METADATA_MUTATION_ROUTE_UNSUPPORTED");
    }
}
