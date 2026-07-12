use crate::PostgresDataStore;
use crm_metadata_runtime::{
    ActivationResult, MetadataBundleDraft, MetadataDocument, MetadataError, MetadataId,
    MetadataImpactReport, MetadataKey, MetadataKind, MetadataRevisionId, PublishResult,
    RollbackResult, TenantMetadataCatalog, TenantMetadataSnapshot,
};
use crm_module_sdk::{ModuleExecutionContext, SdkError, TenantId};
use sqlx::{Postgres, Row, Transaction};
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

const METADATA_ACTIVATION_LOCK_NAMESPACE: i64 = 0x4352_4d45_5441_4441;
const MAX_TRANSITION_ID_BYTES: usize = 512;

#[derive(Debug, Clone)]
pub struct PostgresMetadataStore {
    store: PostgresDataStore,
}

impl PostgresMetadataStore {
    pub const fn new(store: PostgresDataStore) -> Self {
        Self { store }
    }

    pub async fn publish(
        &self,
        context: &ModuleExecutionContext,
        draft: &MetadataBundleDraft,
        occurred_at_unix_nanos: i64,
    ) -> Result<PublishResult, MetadataPersistenceError> {
        validate_operation(context, occurred_at_unix_nanos)?;
        let revision_id = draft.revision_id();
        let document_count = i32::try_from(draft.documents().len()).map_err(|_| {
            MetadataPersistenceError::InvalidInput(
                "metadata document count exceeds PostgreSQL integer range".to_owned(),
            )
        })?;

        let mut transaction = self.store.pool().begin().await?;
        bind_execution_context(&mut transaction, context).await?;

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
        .fetch_optional(&mut *transaction)
        .await?
        .is_some();

        if inserted {
            insert_documents(&mut transaction, context, &revision_id, draft).await?;
            let state = load_state(&mut transaction, &context.execution.tenant_id, false).await?;
            insert_transition(
                &mut transaction,
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
            let existing =
                load_bundle(&mut transaction, &context.execution.tenant_id, &revision_id)
                    .await?
                    .ok_or_else(|| {
                        MetadataPersistenceError::InvalidStoredValue(
                            "metadata revision header exists without a readable bundle".to_owned(),
                        )
                    })?;
            if existing.documents() != draft.documents() {
                return Err(MetadataPersistenceError::RevisionIdentityCollision(
                    revision_id,
                ));
            }
        }

        transaction.commit().await?;
        Ok(PublishResult {
            revision_id,
            already_published: !inserted,
        })
    }

    pub async fn revision(
        &self,
        context: &ModuleExecutionContext,
        revision_id: &MetadataRevisionId,
    ) -> Result<Option<MetadataBundleDraft>, MetadataPersistenceError> {
        context.validate().map_err(MetadataPersistenceError::Sdk)?;
        let mut transaction = self.store.pool().begin().await?;
        bind_execution_context(&mut transaction, context).await?;
        let revision =
            load_bundle(&mut transaction, &context.execution.tenant_id, revision_id).await?;
        transaction.commit().await?;
        Ok(revision)
    }

    pub async fn tenant_state(
        &self,
        context: &ModuleExecutionContext,
    ) -> Result<TenantMetadataSnapshot, MetadataPersistenceError> {
        context.validate().map_err(MetadataPersistenceError::Sdk)?;
        let mut transaction = self.store.pool().begin().await?;
        bind_execution_context(&mut transaction, context).await?;
        let state = load_state(&mut transaction, &context.execution.tenant_id, false).await?;
        transaction.commit().await?;
        Ok(state)
    }

    pub async fn impact_for(
        &self,
        context: &ModuleExecutionContext,
        candidate_revision: &MetadataRevisionId,
    ) -> Result<MetadataImpactReport, MetadataPersistenceError> {
        context.validate().map_err(MetadataPersistenceError::Sdk)?;
        let mut transaction = self.store.pool().begin().await?;
        bind_execution_context(&mut transaction, context).await?;
        let state = load_state(&mut transaction, &context.execution.tenant_id, false).await?;
        let impact = load_impact(
            &mut transaction,
            &context.execution.tenant_id,
            state.active_revision.as_ref(),
            candidate_revision,
        )
        .await?;
        transaction.commit().await?;
        Ok(impact)
    }

    pub async fn activate(
        &self,
        context: &ModuleExecutionContext,
        candidate_revision: &MetadataRevisionId,
        expected_generation: u64,
        allow_breaking_changes: bool,
        occurred_at_unix_nanos: i64,
    ) -> Result<ActivationResult, MetadataPersistenceError> {
        validate_operation(context, occurred_at_unix_nanos)?;
        let mut transaction = self.store.pool().begin().await?;
        bind_execution_context(&mut transaction, context).await?;
        lock_activation(&mut transaction, &context.execution.tenant_id).await?;

        let state = load_state(&mut transaction, &context.execution.tenant_id, true).await?;
        require_generation(expected_generation, state.generation)?;

        let impact = load_impact(
            &mut transaction,
            &context.execution.tenant_id,
            state.active_revision.as_ref(),
            candidate_revision,
        )
        .await?;
        let previous_revision = state.active_revision.clone();
        if previous_revision.as_ref() == Some(candidate_revision) {
            transaction.commit().await?;
            return Ok(ActivationResult {
                generation: state.generation,
                active_revision: candidate_revision.clone(),
                previous_revision,
                already_active: true,
                impact,
            });
        }
        if impact.has_breaking_changes() && !allow_breaking_changes {
            return Err(
                MetadataPersistenceError::BreakingChangeConfirmationRequired(
                    candidate_revision.clone(),
                ),
            );
        }

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
                &mut transaction,
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
            &mut transaction,
            context,
            next_generation,
            candidate_revision,
            next_depth,
        )
        .await?;
        insert_transition(
            &mut transaction,
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

        transaction.commit().await?;
        Ok(ActivationResult {
            generation: next_generation,
            active_revision: candidate_revision.clone(),
            previous_revision,
            already_active: false,
            impact,
        })
    }

    pub async fn rollback(
        &self,
        context: &ModuleExecutionContext,
        expected_generation: u64,
        occurred_at_unix_nanos: i64,
    ) -> Result<RollbackResult, MetadataPersistenceError> {
        validate_operation(context, occurred_at_unix_nanos)?;
        let mut transaction = self.store.pool().begin().await?;
        bind_execution_context(&mut transaction, context).await?;
        lock_activation(&mut transaction, &context.execution.tenant_id).await?;

        let state = load_state(&mut transaction, &context.execution.tenant_id, true).await?;
        require_generation(expected_generation, state.generation)?;
        let replaced_revision = state
            .active_revision
            .clone()
            .ok_or(MetadataPersistenceError::RollbackUnavailable)?;
        if state.rollback_depth == 0 {
            return Err(MetadataPersistenceError::RollbackUnavailable);
        }

        let target_revision = load_rollback_target(
            &mut transaction,
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

        delete_rollback_stack_entry(&mut transaction, context, state.rollback_depth).await?;
        upsert_activation_head(
            &mut transaction,
            context,
            next_generation,
            &target_revision,
            next_depth,
        )
        .await?;
        insert_transition(
            &mut transaction,
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

        transaction.commit().await?;
        Ok(RollbackResult {
            generation: next_generation,
            active_revision: target_revision,
            replaced_revision,
        })
    }

    pub async fn transitions(
        &self,
        context: &ModuleExecutionContext,
    ) -> Result<Vec<MetadataTransitionEvidence>, MetadataPersistenceError> {
        context.validate().map_err(MetadataPersistenceError::Sdk)?;
        let mut transaction = self.store.pool().begin().await?;
        bind_execution_context(&mut transaction, context).await?;
        let rows = sqlx::query(
            r#"
            SELECT
              transition_id,
              action,
              generation,
              rollback_depth,
              from_revision_id,
              to_revision_id,
              actor_id,
              request_id,
              capability_id,
              capability_version,
              business_transaction_id,
              (extract(epoch FROM occurred_at) * 1000000000)::bigint AS occurred_at_unix_nanos
            FROM crm.metadata_transitions
            WHERE tenant_id = $1
            ORDER BY occurred_at, transition_id
            "#,
        )
        .bind(context.execution.tenant_id.as_str())
        .fetch_all(&mut *transaction)
        .await?;
        transaction.commit().await?;

        rows.into_iter().map(decode_transition).collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetadataTransitionAction {
    Publish,
    Activate,
    Rollback,
}

impl MetadataTransitionAction {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Publish => "publish",
            Self::Activate => "activate",
            Self::Rollback => "rollback",
        }
    }

    fn parse(value: &str) -> Result<Self, MetadataPersistenceError> {
        match value {
            "publish" => Ok(Self::Publish),
            "activate" => Ok(Self::Activate),
            "rollback" => Ok(Self::Rollback),
            _ => Err(MetadataPersistenceError::InvalidStoredValue(format!(
                "unknown metadata transition action `{value}`"
            ))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetadataTransitionEvidence {
    pub transition_id: String,
    pub action: MetadataTransitionAction,
    pub generation: u64,
    pub rollback_depth: usize,
    pub from_revision: Option<MetadataRevisionId>,
    pub to_revision: MetadataRevisionId,
    pub actor_id: String,
    pub request_id: String,
    pub capability_id: String,
    pub capability_version: String,
    pub business_transaction_id: String,
    pub occurred_at_unix_nanos: i64,
}

#[derive(Debug)]
pub enum MetadataPersistenceError {
    Database(sqlx::Error),
    Sdk(SdkError),
    Runtime(MetadataError),
    InvalidInput(String),
    InvalidStoredValue(String),
    RevisionNotFound(MetadataRevisionId),
    RevisionIdentityCollision(MetadataRevisionId),
    GenerationConflict { expected: u64, actual: u64 },
    BreakingChangeConfirmationRequired(MetadataRevisionId),
    RollbackUnavailable,
}

impl fmt::Display for MetadataPersistenceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Database(error) => {
                write!(formatter, "metadata database operation failed: {error}")
            }
            Self::Sdk(error) => write!(formatter, "metadata execution context is invalid: {error}"),
            Self::Runtime(error) => {
                write!(formatter, "metadata runtime validation failed: {error}")
            }
            Self::InvalidInput(message) => {
                write!(formatter, "invalid metadata persistence input: {message}")
            }
            Self::InvalidStoredValue(message) => {
                write!(
                    formatter,
                    "invalid metadata value stored in PostgreSQL: {message}"
                )
            }
            Self::RevisionNotFound(revision_id) => {
                write!(formatter, "metadata revision {revision_id} was not found")
            }
            Self::RevisionIdentityCollision(revision_id) => write!(
                formatter,
                "metadata revision identity collision detected for {revision_id}"
            ),
            Self::GenerationConflict { expected, actual } => write!(
                formatter,
                "metadata activation generation conflict: expected {expected}, actual {actual}"
            ),
            Self::BreakingChangeConfirmationRequired(revision_id) => write!(
                formatter,
                "breaking metadata changes require explicit confirmation before activating {revision_id}"
            ),
            Self::RollbackUnavailable => {
                formatter.write_str("no previous metadata revision is available for rollback")
            }
        }
    }
}

impl Error for MetadataPersistenceError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Database(error) => Some(error),
            Self::Sdk(error) => Some(error),
            Self::Runtime(error) => Some(error),
            Self::InvalidInput(_)
            | Self::InvalidStoredValue(_)
            | Self::RevisionNotFound(_)
            | Self::RevisionIdentityCollision(_)
            | Self::GenerationConflict { .. }
            | Self::BreakingChangeConfirmationRequired(_)
            | Self::RollbackUnavailable => None,
        }
    }
}

impl From<sqlx::Error> for MetadataPersistenceError {
    fn from(value: sqlx::Error) -> Self {
        Self::Database(value)
    }
}

impl From<MetadataError> for MetadataPersistenceError {
    fn from(value: MetadataError) -> Self {
        Self::Runtime(value)
    }
}

fn validate_operation(
    context: &ModuleExecutionContext,
    occurred_at_unix_nanos: i64,
) -> Result<(), MetadataPersistenceError> {
    context.validate().map_err(MetadataPersistenceError::Sdk)?;
    if occurred_at_unix_nanos <= 0 {
        return Err(MetadataPersistenceError::InvalidInput(
            "metadata transition occurrence time must be positive".to_owned(),
        ));
    }
    Ok(())
}

async fn bind_execution_context(
    transaction: &mut Transaction<'_, Postgres>,
    context: &ModuleExecutionContext,
) -> Result<(), MetadataPersistenceError> {
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

async fn lock_activation(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: &TenantId,
) -> Result<(), MetadataPersistenceError> {
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, $2))")
        .bind(tenant_id.as_str())
        .bind(METADATA_ACTIVATION_LOCK_NAMESPACE)
        .fetch_one(&mut **transaction)
        .await?;
    Ok(())
}

async fn insert_documents(
    transaction: &mut Transaction<'_, Postgres>,
    context: &ModuleExecutionContext,
    revision_id: &MetadataRevisionId,
    draft: &MetadataBundleDraft,
) -> Result<(), MetadataPersistenceError> {
    for document in draft.documents().values() {
        sqlx::query(
            r#"
            INSERT INTO crm.metadata_revision_documents (
              tenant_id,
              revision_id,
              metadata_kind,
              metadata_id,
              schema_version,
              canonical_content
            )
            VALUES ($1, $2, $3, $4, $5, $6)
            "#,
        )
        .bind(context.execution.tenant_id.as_str())
        .bind(revision_id.as_bytes().as_slice())
        .bind(kind_name(document.key().kind()))
        .bind(document.key().id().as_str())
        .bind(document.schema_version())
        .bind(document.canonical_content())
        .execute(&mut **transaction)
        .await?;

        for dependency in document.dependencies() {
            sqlx::query(
                r#"
                INSERT INTO crm.metadata_revision_dependencies (
                  tenant_id,
                  revision_id,
                  metadata_kind,
                  metadata_id,
                  dependency_kind,
                  dependency_id
                )
                VALUES ($1, $2, $3, $4, $5, $6)
                "#,
            )
            .bind(context.execution.tenant_id.as_str())
            .bind(revision_id.as_bytes().as_slice())
            .bind(kind_name(document.key().kind()))
            .bind(document.key().id().as_str())
            .bind(kind_name(dependency.kind()))
            .bind(dependency.id().as_str())
            .execute(&mut **transaction)
            .await?;
        }
    }
    Ok(())
}

async fn load_bundle(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: &TenantId,
    revision_id: &MetadataRevisionId,
) -> Result<Option<MetadataBundleDraft>, MetadataPersistenceError> {
    let header = sqlx::query(
        r#"
        SELECT document_count
        FROM crm.metadata_revisions_v2
        WHERE tenant_id = $1 AND revision_id = $2
        "#,
    )
    .bind(tenant_id.as_str())
    .bind(revision_id.as_bytes().as_slice())
    .fetch_optional(&mut **transaction)
    .await?;
    let Some(header) = header else {
        return Ok(None);
    };
    let document_count: i32 = header.try_get("document_count")?;
    let expected_document_count = usize::try_from(document_count).map_err(|_| {
        MetadataPersistenceError::InvalidStoredValue(
            "metadata document count is negative or exceeds usize".to_owned(),
        )
    })?;

    let dependency_rows = sqlx::query(
        r#"
        SELECT
          metadata_kind,
          metadata_id,
          dependency_kind,
          dependency_id
        FROM crm.metadata_revision_dependencies
        WHERE tenant_id = $1 AND revision_id = $2
        ORDER BY metadata_kind, metadata_id, dependency_kind, dependency_id
        "#,
    )
    .bind(tenant_id.as_str())
    .bind(revision_id.as_bytes().as_slice())
    .fetch_all(&mut **transaction)
    .await?;

    let mut dependencies = BTreeMap::<MetadataKey, BTreeSet<MetadataKey>>::new();
    for row in dependency_rows {
        let key = metadata_key_from_stored(
            row.try_get::<String, _>("metadata_kind")?,
            row.try_get::<String, _>("metadata_id")?,
        )?;
        let dependency = metadata_key_from_stored(
            row.try_get::<String, _>("dependency_kind")?,
            row.try_get::<String, _>("dependency_id")?,
        )?;
        dependencies.entry(key).or_default().insert(dependency);
    }

    let rows = sqlx::query(
        r#"
        SELECT metadata_kind, metadata_id, schema_version, canonical_content
        FROM crm.metadata_revision_documents
        WHERE tenant_id = $1 AND revision_id = $2
        ORDER BY metadata_kind, metadata_id
        "#,
    )
    .bind(tenant_id.as_str())
    .bind(revision_id.as_bytes().as_slice())
    .fetch_all(&mut **transaction)
    .await?;

    if rows.len() != expected_document_count {
        return Err(MetadataPersistenceError::InvalidStoredValue(format!(
            "metadata revision {revision_id} declares {expected_document_count} documents but stores {}",
            rows.len()
        )));
    }

    let mut documents = Vec::with_capacity(rows.len());
    for row in rows {
        let key = metadata_key_from_stored(
            row.try_get::<String, _>("metadata_kind")?,
            row.try_get::<String, _>("metadata_id")?,
        )?;
        let document_dependencies = dependencies.remove(&key).unwrap_or_default();
        documents.push(MetadataDocument::new(
            key,
            row.try_get::<String, _>("schema_version")?,
            row.try_get::<Vec<u8>, _>("canonical_content")?,
            document_dependencies,
        )?);
    }
    if !dependencies.is_empty() {
        return Err(MetadataPersistenceError::InvalidStoredValue(
            "metadata dependency rows reference documents missing from the revision".to_owned(),
        ));
    }

    let draft = MetadataBundleDraft::new(documents)?;
    if draft.revision_id() != *revision_id {
        return Err(MetadataPersistenceError::InvalidStoredValue(format!(
            "metadata revision identity mismatch: requested {revision_id}, reconstructed {}",
            draft.revision_id()
        )));
    }
    Ok(Some(draft))
}

async fn load_state(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: &TenantId,
    for_update: bool,
) -> Result<TenantMetadataSnapshot, MetadataPersistenceError> {
    let query = if for_update {
        r#"
        SELECT generation, active_revision_id, rollback_depth
        FROM crm.metadata_activation_heads
        WHERE tenant_id = $1
        FOR UPDATE
        "#
    } else {
        r#"
        SELECT generation, active_revision_id, rollback_depth
        FROM crm.metadata_activation_heads
        WHERE tenant_id = $1
        "#
    };
    let row = sqlx::query(query)
        .bind(tenant_id.as_str())
        .fetch_optional(&mut **transaction)
        .await?;
    let Some(row) = row else {
        return Ok(TenantMetadataSnapshot {
            generation: 0,
            active_revision: None,
            rollback_depth: 0,
        });
    };

    let generation = stored_u64(row.try_get::<i64, _>("generation")?, "generation")?;
    let rollback_depth = stored_usize(row.try_get::<i64, _>("rollback_depth")?, "rollback depth")?;
    let active_revision = row
        .try_get::<Option<Vec<u8>>, _>("active_revision_id")?
        .map(revision_id_from_bytes)
        .transpose()?;
    Ok(TenantMetadataSnapshot {
        generation,
        active_revision,
        rollback_depth,
    })
}

async fn load_impact(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: &TenantId,
    current_revision: Option<&MetadataRevisionId>,
    candidate_revision: &MetadataRevisionId,
) -> Result<MetadataImpactReport, MetadataPersistenceError> {
    let candidate_bundle = load_bundle(transaction, tenant_id, candidate_revision)
        .await?
        .ok_or_else(|| MetadataPersistenceError::RevisionNotFound(candidate_revision.clone()))?;
    let current_bundle = match current_revision {
        Some(revision_id) => Some((
            revision_id.clone(),
            load_bundle(transaction, tenant_id, revision_id)
                .await?
                .ok_or_else(|| {
                    MetadataPersistenceError::InvalidStoredValue(format!(
                        "active metadata revision {revision_id} is missing"
                    ))
                })?,
        )),
        None => None,
    };
    impact_from_runtime(
        tenant_id,
        current_bundle,
        candidate_revision,
        candidate_bundle,
    )
}

fn impact_from_runtime(
    tenant_id: &TenantId,
    current: Option<(MetadataRevisionId, MetadataBundleDraft)>,
    candidate_revision: &MetadataRevisionId,
    candidate_bundle: MetadataBundleDraft,
) -> Result<MetadataImpactReport, MetadataPersistenceError> {
    let mut catalog = TenantMetadataCatalog::new();
    let candidate_publish = catalog.publish(tenant_id.clone(), candidate_bundle, 0);
    if candidate_publish.revision_id != *candidate_revision {
        return Err(MetadataPersistenceError::InvalidStoredValue(
            "candidate revision identity changed while loading impact analysis".to_owned(),
        ));
    }
    if let Some((current_revision, current_bundle)) = current {
        let current_publish = catalog.publish(tenant_id.clone(), current_bundle, 0);
        if current_publish.revision_id != current_revision {
            return Err(MetadataPersistenceError::InvalidStoredValue(
                "active revision identity changed while loading impact analysis".to_owned(),
            ));
        }
        catalog.activate(tenant_id.clone(), &current_revision, 0, false)?;
    }
    catalog
        .impact_for(tenant_id, candidate_revision)
        .map_err(MetadataPersistenceError::Runtime)
}

fn require_generation(expected: u64, actual: u64) -> Result<(), MetadataPersistenceError> {
    if expected != actual {
        return Err(MetadataPersistenceError::GenerationConflict { expected, actual });
    }
    Ok(())
}

async fn insert_rollback_stack_entry(
    transaction: &mut Transaction<'_, Postgres>,
    context: &ModuleExecutionContext,
    depth: usize,
    revision_id: &MetadataRevisionId,
    pushed_generation: u64,
) -> Result<(), MetadataPersistenceError> {
    sqlx::query(
        r#"
        INSERT INTO crm.metadata_rollback_stack (
          tenant_id,
          depth,
          revision_id,
          pushed_generation,
          last_business_transaction_id
        )
        VALUES ($1, $2, $3, $4, $5)
        "#,
    )
    .bind(context.execution.tenant_id.as_str())
    .bind(database_i64(depth, "rollback depth")?)
    .bind(revision_id.as_bytes().as_slice())
    .bind(database_i64(pushed_generation, "pushed generation")?)
    .bind(context.execution.business_transaction_id.as_str())
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

async fn load_rollback_target(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: &TenantId,
    depth: usize,
) -> Result<Option<MetadataRevisionId>, MetadataPersistenceError> {
    let row = sqlx::query(
        r#"
        SELECT revision_id
        FROM crm.metadata_rollback_stack
        WHERE tenant_id = $1 AND depth = $2
        FOR UPDATE
        "#,
    )
    .bind(tenant_id.as_str())
    .bind(database_i64(depth, "rollback depth")?)
    .fetch_optional(&mut **transaction)
    .await?;
    row.map(|row| revision_id_from_bytes(row.try_get::<Vec<u8>, _>("revision_id")?))
        .transpose()
}

async fn delete_rollback_stack_entry(
    transaction: &mut Transaction<'_, Postgres>,
    context: &ModuleExecutionContext,
    depth: usize,
) -> Result<(), MetadataPersistenceError> {
    let result = sqlx::query(
        r#"
        DELETE FROM crm.metadata_rollback_stack
        WHERE tenant_id = $1 AND depth = $2
        "#,
    )
    .bind(context.execution.tenant_id.as_str())
    .bind(database_i64(depth, "rollback depth")?)
    .execute(&mut **transaction)
    .await?;
    if result.rows_affected() != 1 {
        return Err(MetadataPersistenceError::InvalidStoredValue(
            "metadata rollback stack entry disappeared during rollback".to_owned(),
        ));
    }
    Ok(())
}

async fn upsert_activation_head(
    transaction: &mut Transaction<'_, Postgres>,
    context: &ModuleExecutionContext,
    generation: u64,
    active_revision: &MetadataRevisionId,
    rollback_depth: usize,
) -> Result<(), MetadataPersistenceError> {
    sqlx::query(
        r#"
        INSERT INTO crm.metadata_activation_heads (
          tenant_id,
          generation,
          active_revision_id,
          rollback_depth,
          last_business_transaction_id
        )
        VALUES ($1, $2, $3, $4, $5)
        ON CONFLICT (tenant_id) DO UPDATE
        SET generation = EXCLUDED.generation,
            active_revision_id = EXCLUDED.active_revision_id,
            rollback_depth = EXCLUDED.rollback_depth,
            last_business_transaction_id = EXCLUDED.last_business_transaction_id,
            updated_at = clock_timestamp()
        "#,
    )
    .bind(context.execution.tenant_id.as_str())
    .bind(database_i64(generation, "generation")?)
    .bind(active_revision.as_bytes().as_slice())
    .bind(database_i64(rollback_depth, "rollback depth")?)
    .bind(context.execution.business_transaction_id.as_str())
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

struct MetadataTransitionWrite<'a> {
    action: MetadataTransitionAction,
    generation: u64,
    rollback_depth: usize,
    from_revision: Option<&'a MetadataRevisionId>,
    to_revision: &'a MetadataRevisionId,
    occurred_at_unix_nanos: i64,
}

async fn insert_transition(
    transaction: &mut Transaction<'_, Postgres>,
    context: &ModuleExecutionContext,
    transition: MetadataTransitionWrite<'_>,
) -> Result<(), MetadataPersistenceError> {
    let transition_id = transition_id(
        context,
        transition.action,
        transition.generation,
        transition.to_revision,
    )?;
    sqlx::query(
        r#"
        INSERT INTO crm.metadata_transitions (
          tenant_id,
          transition_id,
          action,
          generation,
          rollback_depth,
          from_revision_id,
          to_revision_id,
          actor_id,
          request_id,
          capability_id,
          capability_version,
          business_transaction_id,
          occurred_at
        )
        VALUES (
          $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12,
          TIMESTAMPTZ 'epoch' + ($13::bigint / 1000) * INTERVAL '1 microsecond'
        )
        "#,
    )
    .bind(context.execution.tenant_id.as_str())
    .bind(transition_id)
    .bind(transition.action.as_str())
    .bind(database_i64(transition.generation, "generation")?)
    .bind(database_i64(transition.rollback_depth, "rollback depth")?)
    .bind(
        transition
            .from_revision
            .map(|revision| revision.as_bytes().as_slice()),
    )
    .bind(transition.to_revision.as_bytes().as_slice())
    .bind(context.execution.actor_id.as_str())
    .bind(context.execution.request_id.as_str())
    .bind(context.execution.capability_id.as_str())
    .bind(context.execution.capability_version.as_str())
    .bind(context.execution.business_transaction_id.as_str())
    .bind(transition.occurred_at_unix_nanos)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

fn transition_id(
    context: &ModuleExecutionContext,
    action: MetadataTransitionAction,
    generation: u64,
    revision_id: &MetadataRevisionId,
) -> Result<String, MetadataPersistenceError> {
    let value = format!(
        "metadata:{}:{}:{generation}:{revision_id}",
        context.execution.business_transaction_id,
        action.as_str()
    );
    if value.len() > MAX_TRANSITION_ID_BYTES {
        return Err(MetadataPersistenceError::InvalidInput(format!(
            "metadata transition id exceeds {MAX_TRANSITION_ID_BYTES} bytes"
        )));
    }
    Ok(value)
}

fn decode_transition(
    row: sqlx::postgres::PgRow,
) -> Result<MetadataTransitionEvidence, MetadataPersistenceError> {
    Ok(MetadataTransitionEvidence {
        transition_id: row.try_get("transition_id")?,
        action: MetadataTransitionAction::parse(&row.try_get::<String, _>("action")?)?,
        generation: stored_u64(row.try_get::<i64, _>("generation")?, "generation")?,
        rollback_depth: stored_usize(row.try_get::<i64, _>("rollback_depth")?, "rollback depth")?,
        from_revision: row
            .try_get::<Option<Vec<u8>>, _>("from_revision_id")?
            .map(revision_id_from_bytes)
            .transpose()?,
        to_revision: revision_id_from_bytes(row.try_get("to_revision_id")?)?,
        actor_id: row.try_get("actor_id")?,
        request_id: row.try_get("request_id")?,
        capability_id: row.try_get("capability_id")?,
        capability_version: row.try_get("capability_version")?,
        business_transaction_id: row.try_get("business_transaction_id")?,
        occurred_at_unix_nanos: row.try_get("occurred_at_unix_nanos")?,
    })
}

fn metadata_key_from_stored(
    kind: String,
    id: String,
) -> Result<MetadataKey, MetadataPersistenceError> {
    let kind = parse_kind(&kind)?;
    let id = MetadataId::try_new(id).map_err(MetadataPersistenceError::Runtime)?;
    Ok(MetadataKey::new(kind, id))
}

const fn kind_name(kind: MetadataKind) -> &'static str {
    match kind {
        MetadataKind::Object => "object",
        MetadataKind::Field => "field",
        MetadataKind::Relationship => "relationship",
        MetadataKind::Layout => "layout",
        MetadataKind::View => "view",
        MetadataKind::Pipeline => "pipeline",
        MetadataKind::Permission => "permission",
        MetadataKind::Workflow => "workflow",
    }
}

fn parse_kind(value: &str) -> Result<MetadataKind, MetadataPersistenceError> {
    match value {
        "object" => Ok(MetadataKind::Object),
        "field" => Ok(MetadataKind::Field),
        "relationship" => Ok(MetadataKind::Relationship),
        "layout" => Ok(MetadataKind::Layout),
        "view" => Ok(MetadataKind::View),
        "pipeline" => Ok(MetadataKind::Pipeline),
        "permission" => Ok(MetadataKind::Permission),
        "workflow" => Ok(MetadataKind::Workflow),
        _ => Err(MetadataPersistenceError::InvalidStoredValue(format!(
            "unknown metadata kind `{value}`"
        ))),
    }
}

fn revision_id_from_bytes(bytes: Vec<u8>) -> Result<MetadataRevisionId, MetadataPersistenceError> {
    let bytes: [u8; 32] = bytes.try_into().map_err(|_| {
        MetadataPersistenceError::InvalidStoredValue(
            "metadata revision id must contain exactly 32 bytes".to_owned(),
        )
    })?;
    Ok(MetadataRevisionId::from_bytes(bytes))
}

fn stored_u64(value: i64, field: &str) -> Result<u64, MetadataPersistenceError> {
    u64::try_from(value).map_err(|_| {
        MetadataPersistenceError::InvalidStoredValue(format!("metadata {field} is negative"))
    })
}

fn stored_usize(value: i64, field: &str) -> Result<usize, MetadataPersistenceError> {
    usize::try_from(value).map_err(|_| {
        MetadataPersistenceError::InvalidStoredValue(format!(
            "metadata {field} is negative or exceeds usize"
        ))
    })
}

fn database_i64<T>(value: T, field: &str) -> Result<i64, MetadataPersistenceError>
where
    i64: TryFrom<T>,
{
    i64::try_from(value).map_err(|_| {
        MetadataPersistenceError::InvalidInput(format!(
            "metadata {field} exceeds PostgreSQL bigint range"
        ))
    })
}
