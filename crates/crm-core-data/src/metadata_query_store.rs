use crate::PostgresDataStore;
use crm_metadata_query_adapter::MetadataQueryStore;
use crm_metadata_runtime::{
    MetadataBundleDraft, MetadataDocument, MetadataError, MetadataId, MetadataImpactReport,
    MetadataKey, MetadataKind, MetadataRevisionId, TenantMetadataCatalog, TenantMetadataSnapshot,
};
use crm_module_sdk::{ErrorCategory, PortFuture, SdkError, TenantId};
use sqlx::{Postgres, Row, Transaction};
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

/// Tenant-only PostgreSQL read adapter for governed metadata queries.
///
/// This adapter deliberately accepts only the tenant authority established by
/// `QueryGateway`. It never requires or invents mutation-only idempotency or
/// business-transaction identifiers, and every transaction binds `app.tenant_id`
/// before reading FORCE-RLS protected metadata tables.
#[derive(Debug, Clone)]
pub struct PostgresMetadataQueryStore {
    store: PostgresDataStore,
}

impl PostgresMetadataQueryStore {
    pub const fn new(store: PostgresDataStore) -> Self {
        Self { store }
    }

    async fn load_revision(
        &self,
        tenant_id: &TenantId,
        revision_id: &MetadataRevisionId,
    ) -> Result<Option<MetadataBundleDraft>, MetadataQueryPersistenceError> {
        let mut transaction = self.store.pool().begin().await?;
        bind_tenant_scope(&mut transaction, tenant_id).await?;
        let revision = load_bundle(&mut transaction, tenant_id, revision_id).await?;
        transaction.commit().await?;
        Ok(revision)
    }

    async fn load_tenant_state(
        &self,
        tenant_id: &TenantId,
    ) -> Result<TenantMetadataSnapshot, MetadataQueryPersistenceError> {
        let mut transaction = self.store.pool().begin().await?;
        bind_tenant_scope(&mut transaction, tenant_id).await?;
        let state = load_state(&mut transaction, tenant_id).await?;
        transaction.commit().await?;
        Ok(state)
    }

    async fn load_impact(
        &self,
        tenant_id: &TenantId,
        candidate_revision: &MetadataRevisionId,
    ) -> Result<MetadataImpactReport, MetadataQueryPersistenceError> {
        let mut transaction = self.store.pool().begin().await?;
        bind_tenant_scope(&mut transaction, tenant_id).await?;
        let state = load_state(&mut transaction, tenant_id).await?;
        let candidate_bundle = load_bundle(&mut transaction, tenant_id, candidate_revision)
            .await?
            .ok_or_else(|| {
                MetadataQueryPersistenceError::RevisionNotFound(candidate_revision.clone())
            })?;
        let current_bundle = match state.active_revision.as_ref() {
            Some(revision_id) => Some((
                revision_id.clone(),
                load_bundle(&mut transaction, tenant_id, revision_id)
                    .await?
                    .ok_or_else(|| {
                        MetadataQueryPersistenceError::InvalidStoredValue(format!(
                            "active metadata revision {revision_id} is missing"
                        ))
                    })?,
            )),
            None => None,
        };
        transaction.commit().await?;
        impact_from_runtime(
            tenant_id,
            current_bundle,
            candidate_revision,
            candidate_bundle,
        )
    }
}

impl MetadataQueryStore for PostgresMetadataQueryStore {
    fn impact_for<'a>(
        &'a self,
        tenant_id: &'a TenantId,
        candidate_revision: &'a MetadataRevisionId,
    ) -> PortFuture<'a, Result<MetadataImpactReport, SdkError>> {
        Box::pin(async move {
            self.load_impact(tenant_id, candidate_revision)
                .await
                .map_err(metadata_query_error_to_sdk)
        })
    }

    fn revision<'a>(
        &'a self,
        tenant_id: &'a TenantId,
        revision_id: &'a MetadataRevisionId,
    ) -> PortFuture<'a, Result<Option<MetadataBundleDraft>, SdkError>> {
        Box::pin(async move {
            self.load_revision(tenant_id, revision_id)
                .await
                .map_err(metadata_query_error_to_sdk)
        })
    }

    fn tenant_state<'a>(
        &'a self,
        tenant_id: &'a TenantId,
    ) -> PortFuture<'a, Result<TenantMetadataSnapshot, SdkError>> {
        Box::pin(async move {
            self.load_tenant_state(tenant_id)
                .await
                .map_err(metadata_query_error_to_sdk)
        })
    }
}

async fn bind_tenant_scope(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: &TenantId,
) -> Result<(), MetadataQueryPersistenceError> {
    sqlx::query("SELECT set_config('app.tenant_id', $1, true)")
        .bind(tenant_id.as_str())
        .execute(&mut **transaction)
        .await?;
    Ok(())
}

async fn load_bundle(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: &TenantId,
    revision_id: &MetadataRevisionId,
) -> Result<Option<MetadataBundleDraft>, MetadataQueryPersistenceError> {
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
        MetadataQueryPersistenceError::InvalidStoredValue(
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
        return Err(MetadataQueryPersistenceError::InvalidStoredValue(format!(
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
        return Err(MetadataQueryPersistenceError::InvalidStoredValue(
            "metadata dependency rows reference documents missing from the revision".to_owned(),
        ));
    }

    let draft = MetadataBundleDraft::new(documents)?;
    if draft.revision_id() != *revision_id {
        return Err(MetadataQueryPersistenceError::InvalidStoredValue(format!(
            "metadata revision identity mismatch: requested {revision_id}, reconstructed {}",
            draft.revision_id()
        )));
    }
    Ok(Some(draft))
}

async fn load_state(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: &TenantId,
) -> Result<TenantMetadataSnapshot, MetadataQueryPersistenceError> {
    let row = sqlx::query(
        r#"
        SELECT generation, active_revision_id, rollback_depth
        FROM crm.metadata_activation_heads
        WHERE tenant_id = $1
        "#,
    )
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

fn impact_from_runtime(
    tenant_id: &TenantId,
    current: Option<(MetadataRevisionId, MetadataBundleDraft)>,
    candidate_revision: &MetadataRevisionId,
    candidate_bundle: MetadataBundleDraft,
) -> Result<MetadataImpactReport, MetadataQueryPersistenceError> {
    let mut catalog = TenantMetadataCatalog::new();
    let candidate_publish = catalog.publish(tenant_id.clone(), candidate_bundle, 0);
    if candidate_publish.revision_id != *candidate_revision {
        return Err(MetadataQueryPersistenceError::InvalidStoredValue(
            "candidate revision identity changed while loading impact analysis".to_owned(),
        ));
    }
    if let Some((current_revision, current_bundle)) = current {
        let current_publish = catalog.publish(tenant_id.clone(), current_bundle, 0);
        if current_publish.revision_id != current_revision {
            return Err(MetadataQueryPersistenceError::InvalidStoredValue(
                "active revision identity changed while loading impact analysis".to_owned(),
            ));
        }
        catalog.activate(tenant_id.clone(), &current_revision, 0, false)?;
    }
    catalog
        .impact_for(tenant_id, candidate_revision)
        .map_err(MetadataQueryPersistenceError::Runtime)
}

fn metadata_key_from_stored(
    kind: String,
    id: String,
) -> Result<MetadataKey, MetadataQueryPersistenceError> {
    let kind = parse_kind(&kind)?;
    let id = MetadataId::try_new(id).map_err(MetadataQueryPersistenceError::Runtime)?;
    Ok(MetadataKey::new(kind, id))
}

fn parse_kind(value: &str) -> Result<MetadataKind, MetadataQueryPersistenceError> {
    match value {
        "object" => Ok(MetadataKind::Object),
        "field" => Ok(MetadataKind::Field),
        "relationship" => Ok(MetadataKind::Relationship),
        "layout" => Ok(MetadataKind::Layout),
        "view" => Ok(MetadataKind::View),
        "pipeline" => Ok(MetadataKind::Pipeline),
        "permission" => Ok(MetadataKind::Permission),
        "workflow" => Ok(MetadataKind::Workflow),
        _ => Err(MetadataQueryPersistenceError::InvalidStoredValue(format!(
            "unknown metadata kind `{value}`"
        ))),
    }
}

fn revision_id_from_bytes(
    bytes: Vec<u8>,
) -> Result<MetadataRevisionId, MetadataQueryPersistenceError> {
    let bytes: [u8; 32] = bytes.try_into().map_err(|_| {
        MetadataQueryPersistenceError::InvalidStoredValue(
            "metadata revision id must contain exactly 32 bytes".to_owned(),
        )
    })?;
    Ok(MetadataRevisionId::from_bytes(bytes))
}

fn stored_u64(value: i64, field: &str) -> Result<u64, MetadataQueryPersistenceError> {
    u64::try_from(value).map_err(|_| {
        MetadataQueryPersistenceError::InvalidStoredValue(format!(
            "metadata {field} is negative"
        ))
    })
}

fn stored_usize(value: i64, field: &str) -> Result<usize, MetadataQueryPersistenceError> {
    usize::try_from(value).map_err(|_| {
        MetadataQueryPersistenceError::InvalidStoredValue(format!(
            "metadata {field} is negative or exceeds usize"
        ))
    })
}

#[derive(Debug)]
enum MetadataQueryPersistenceError {
    Database(sqlx::Error),
    Runtime(MetadataError),
    InvalidStoredValue(String),
    RevisionNotFound(MetadataRevisionId),
}

impl fmt::Display for MetadataQueryPersistenceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Database(error) => write!(formatter, "metadata query database operation failed: {error}"),
            Self::Runtime(error) => write!(formatter, "metadata query runtime validation failed: {error}"),
            Self::InvalidStoredValue(message) => {
                write!(formatter, "invalid metadata value stored in PostgreSQL: {message}")
            }
            Self::RevisionNotFound(revision_id) => {
                write!(formatter, "metadata revision {revision_id} was not found")
            }
        }
    }
}

impl Error for MetadataQueryPersistenceError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Database(error) => Some(error),
            Self::Runtime(error) => Some(error),
            Self::InvalidStoredValue(_) | Self::RevisionNotFound(_) => None,
        }
    }
}

impl From<sqlx::Error> for MetadataQueryPersistenceError {
    fn from(value: sqlx::Error) -> Self {
        Self::Database(value)
    }
}

impl From<MetadataError> for MetadataQueryPersistenceError {
    fn from(value: MetadataError) -> Self {
        Self::Runtime(value)
    }
}

fn metadata_query_error_to_sdk(error: MetadataQueryPersistenceError) -> SdkError {
    match error {
        MetadataQueryPersistenceError::RevisionNotFound(revision_id) => SdkError::new(
            "METADATA_REVISION_NOT_FOUND",
            ErrorCategory::NotFound,
            false,
            "The requested metadata revision does not exist.",
        )
        .with_internal_reference(revision_id.to_hex()),
        MetadataQueryPersistenceError::Database(error) => SdkError::new(
            "METADATA_QUERY_STORE_UNAVAILABLE",
            ErrorCategory::Dependency,
            true,
            "The metadata query store is temporarily unavailable.",
        )
        .with_internal_reference(error.to_string()),
        MetadataQueryPersistenceError::Runtime(error) => SdkError::new(
            "METADATA_QUERY_STORED_CONTRACT_INVALID",
            ErrorCategory::Internal,
            false,
            "Stored metadata failed integrity validation.",
        )
        .with_internal_reference(error.to_string()),
        MetadataQueryPersistenceError::InvalidStoredValue(message) => SdkError::new(
            "METADATA_QUERY_STORED_VALUE_INVALID",
            ErrorCategory::Internal,
            false,
            "Stored metadata failed integrity validation.",
        )
        .with_internal_reference(message),
    }
}
