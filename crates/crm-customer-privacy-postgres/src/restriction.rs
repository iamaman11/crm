use crm_core_data::{CustomerSubjectOperationClass, PostgresDataStore};
use crm_core_data::postgres_sqlx as sqlx;
use crm_customer_privacy::{
    MODULE_ID, PRIVACY_CASE_RECORD_TYPE, PrivacyCaseKind, PrivacyCaseStatus,
    ProcessingRestriction, ProcessingRestrictionScope, ProcessingRestrictionState,
    RestrictedChannel,
};
use crm_customer_privacy_application::{
    RestrictionPlacementCommit, RestrictionPlacementInvocation,
    RestrictionPlacementPersistencePort,
};
use crm_customer_privacy_persistence_adapter::privacy_case_from_snapshot;
use crm_module_sdk::{ErrorCategory, PortFuture, RecordId, SdkError, TenantId};
use crm_party_reference_composition::lock_customer_privacy_subject_in_transaction;
use sqlx::{Postgres, Row, Transaction};
use std::collections::BTreeSet;

#[derive(Debug, Clone)]
pub struct PostgresProcessingRestrictionStore {
    store: PostgresDataStore,
}

impl PostgresProcessingRestrictionStore {
    pub fn new(store: PostgresDataStore) -> Self {
        Self { store }
    }

    pub fn store(&self) -> &PostgresDataStore {
        &self.store
    }

    pub fn lock_and_enforce<'a>(
        &'a self,
        transaction: &'a mut Transaction<'_, Postgres>,
        tenant_id: &'a TenantId,
        canonical_party_id: &'a RecordId,
        operation_class: CustomerSubjectOperationClass,
    ) -> PortFuture<'a, Result<(), SdkError>> {
        Box::pin(async move {
            lock_customer_privacy_subject_in_transaction(
                transaction,
                tenant_id,
                canonical_party_id,
            )
            .await?;
            ensure_customer_privacy_active(transaction, tenant_id).await?;

            let row = sqlx::query(
                r#"
SELECT restriction_id,
       state,
       scopes,
       channels,
       starts_at_unix_nanos,
       expires_at_unix_nanos,
       policy_version,
       placed_at_unix_nanos,
       (starts_at_unix_nanos <=
          (extract(epoch FROM clock_timestamp()) * 1000000000)::bigint) AS effective_now
FROM customer_privacy.processing_restrictions
WHERE tenant_id = $1
  AND canonical_party_id = $2
FOR SHARE
                "#,
            )
            .bind(tenant_id.as_str())
            .bind(canonical_party_id.as_str())
            .fetch_optional(transaction.as_mut())
            .await
            .map_err(policy_dependency_error)?;

            let Some(row) = row else {
                return Ok(());
            };
            let state: String = row.try_get("state").map_err(policy_row_error)?;
            let scopes: Vec<String> = row.try_get("scopes").map_err(policy_row_error)?;
            let channels: Vec<String> = row.try_get("channels").map_err(policy_row_error)?;
            let starts_at: i64 = row
                .try_get("starts_at_unix_nanos")
                .map_err(policy_row_error)?;
            let expires_at: Option<i64> = row
                .try_get("expires_at_unix_nanos")
                .map_err(policy_row_error)?;
            let policy_version: String = row
                .try_get("policy_version")
                .map_err(policy_row_error)?;
            let placed_at: i64 = row
                .try_get("placed_at_unix_nanos")
                .map_err(policy_row_error)?;
            let effective_now: bool = row.try_get("effective_now").map_err(policy_row_error)?;
            if state != "active"
                || scopes != ["all_processing"]
                || !channels.is_empty()
                || starts_at <= 0
                || starts_at > placed_at
                || expires_at.is_some()
                || policy_version.trim().is_empty()
            {
                return Err(policy_corrupt());
            }
            if !effective_now {
                return Err(policy_stale());
            }

            Err(SdkError::new(
                "CUSTOMER_PRIVACY_SUBJECT_OPERATION_RESTRICTED",
                ErrorCategory::Conflict,
                false,
                "The customer-subject operation is denied by an active processing restriction.",
            )
            .with_internal_reference(format!(
                "operation_class={};restriction_id={}",
                operation_class.label(),
                row.try_get::<String, _>("restriction_id")
                    .map_err(policy_row_error)?
            )))
        })
    }
}

impl RestrictionPlacementPersistencePort for PostgresProcessingRestrictionStore {
    fn place<'a>(
        &'a self,
        invocation: &'a RestrictionPlacementInvocation,
    ) -> PortFuture<'a, Result<RestrictionPlacementCommit, SdkError>> {
        Box::pin(async move {
            let mut transaction = self.store.pool().begin().await.map_err(storage_error)?;
            bind_invocation(&mut transaction, invocation).await?;

            let case_row = sqlx::query(
                r#"
SELECT snapshot
FROM crm.records
WHERE tenant_id = $1
  AND record_type = $2
  AND record_id = $3
FOR UPDATE
                "#,
            )
            .bind(invocation.tenant_id.as_str())
            .bind(PRIVACY_CASE_RECORD_TYPE)
            .bind(invocation.privacy_case_id.as_str())
            .fetch_optional(transaction.as_mut())
            .await
            .map_err(storage_error)?
            .ok_or_else(case_unavailable)?;
            let snapshot = case_row
                .try_get("snapshot")
                .map_err(|error| corrupt_case(error.to_string()))?;
            let privacy_case = privacy_case_from_snapshot(&snapshot)?;
            if privacy_case.tenant_id() != &invocation.tenant_id
                || privacy_case.case_id() != &invocation.privacy_case_id
                || privacy_case.kind() != PrivacyCaseKind::RestrictProcessing
                || !placement_status_allowed(privacy_case.status())
            {
                return Err(case_unavailable());
            }
            let subject = privacy_case.subject_binding().ok_or_else(case_unavailable)?;
            lock_customer_privacy_subject_in_transaction(
                &mut transaction,
                &invocation.tenant_id,
                &subject.canonical_party_id,
            )
            .await?;
            ensure_customer_privacy_active(&mut transaction, &invocation.tenant_id).await?;

            if let Some(commit) = replay_commit(
                &mut transaction,
                invocation,
                &subject.canonical_party_id,
            )
            .await?
            {
                transaction.commit().await.map_err(storage_error)?;
                return Ok(commit);
            }

            let existing_subject = sqlx::query_scalar::<_, String>(
                r#"
SELECT restriction_id
FROM customer_privacy.processing_restrictions
WHERE tenant_id = $1
  AND canonical_party_id = $2
FOR SHARE
                "#,
            )
            .bind(invocation.tenant_id.as_str())
            .bind(subject.canonical_party_id.as_str())
            .fetch_optional(transaction.as_mut())
            .await
            .map_err(storage_error)?;
            if existing_subject.is_some() {
                return Err(restriction_conflict());
            }

            let restriction = build_restriction(invocation, &subject.canonical_party_id)?;
            sqlx::query(
                r#"
INSERT INTO customer_privacy.processing_restrictions (
  tenant_id,
  restriction_id,
  privacy_case_id,
  canonical_party_id,
  state,
  scopes,
  channels,
  starts_at_unix_nanos,
  expires_at_unix_nanos,
  reason,
  legal_basis,
  policy_version,
  placed_at_unix_nanos,
  placed_by_actor_id,
  request_id,
  correlation_id,
  trace_id,
  idempotency_key,
  restriction_version
)
VALUES ($1, $2, $3, $4, 'active', $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, 1)
                "#,
            )
            .bind(invocation.tenant_id.as_str())
            .bind(invocation.restriction_id.as_str())
            .bind(invocation.privacy_case_id.as_str())
            .bind(subject.canonical_party_id.as_str())
            .bind(vec!["all_processing"])
            .bind(Vec::<String>::new())
            .bind(invocation.starts_at_unix_nanos)
            .bind(invocation.expires_at_unix_nanos)
            .bind(invocation.reason.trim())
            .bind(invocation.legal_basis.trim())
            .bind(invocation.policy_version.trim())
            .bind(invocation.proposed_placed_at_unix_nanos)
            .bind(invocation.actor_id.as_str())
            .bind(invocation.request_id.as_str())
            .bind(invocation.correlation_id.as_str())
            .bind(invocation.trace_id.as_str())
            .bind(invocation.idempotency_key.trim())
            .execute(transaction.as_mut())
            .await
            .map_err(insert_error)?;

            sqlx::query(
                r#"
INSERT INTO customer_privacy.processing_restriction_events (
  tenant_id,
  restriction_id,
  event_sequence,
  event_type,
  privacy_case_id,
  canonical_party_id,
  policy_version,
  request_id,
  actor_id,
  recorded_at_unix_nanos
)
VALUES ($1, $2, 1, 'customer_privacy.restriction.placed', $3, $4, $5, $6, $7, $8)
                "#,
            )
            .bind(invocation.tenant_id.as_str())
            .bind(invocation.restriction_id.as_str())
            .bind(invocation.privacy_case_id.as_str())
            .bind(subject.canonical_party_id.as_str())
            .bind(invocation.policy_version.trim())
            .bind(invocation.request_id.as_str())
            .bind(invocation.actor_id.as_str())
            .bind(invocation.proposed_placed_at_unix_nanos)
            .execute(transaction.as_mut())
            .await
            .map_err(storage_error)?;

            sqlx::query(
                r#"
INSERT INTO customer_privacy.processing_restriction_idempotency (
  tenant_id,
  idempotency_key,
  restriction_id,
  privacy_case_id,
  canonical_party_id,
  request_id,
  policy_version,
  committed_at_unix_nanos
)
VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                "#,
            )
            .bind(invocation.tenant_id.as_str())
            .bind(invocation.idempotency_key.trim())
            .bind(invocation.restriction_id.as_str())
            .bind(invocation.privacy_case_id.as_str())
            .bind(subject.canonical_party_id.as_str())
            .bind(invocation.request_id.as_str())
            .bind(invocation.policy_version.trim())
            .bind(invocation.proposed_placed_at_unix_nanos)
            .execute(transaction.as_mut())
            .await
            .map_err(insert_error)?;

            transaction.commit().await.map_err(storage_error)?;
            Ok(RestrictionPlacementCommit {
                restriction,
                replayed: false,
            })
        })
    }
}

async fn replay_commit(
    transaction: &mut Transaction<'_, Postgres>,
    invocation: &RestrictionPlacementInvocation,
    canonical_party_id: &RecordId,
) -> Result<Option<RestrictionPlacementCommit>, SdkError> {
    let row = sqlx::query(
        r#"
SELECT r.restriction_id,
       r.privacy_case_id,
       r.canonical_party_id,
       r.scopes,
       r.channels,
       r.starts_at_unix_nanos,
       r.expires_at_unix_nanos,
       r.reason,
       r.legal_basis,
       r.policy_version,
       r.placed_at_unix_nanos,
       r.placed_by_actor_id,
       r.restriction_version,
       i.request_id
FROM customer_privacy.processing_restriction_idempotency i
JOIN customer_privacy.processing_restrictions r
  ON r.tenant_id = i.tenant_id
 AND r.restriction_id = i.restriction_id
WHERE i.tenant_id = $1
  AND i.idempotency_key = $2
FOR SHARE OF r, i
        "#,
    )
    .bind(invocation.tenant_id.as_str())
    .bind(invocation.idempotency_key.trim())
    .fetch_optional(transaction.as_mut())
    .await
    .map_err(storage_error)?;
    let Some(row) = row else {
        return Ok(None);
    };

    let matches = row.try_get::<String, _>("restriction_id").map_err(policy_row_error)?
        == invocation.restriction_id.as_str()
        && row.try_get::<String, _>("privacy_case_id").map_err(policy_row_error)?
            == invocation.privacy_case_id.as_str()
        && row.try_get::<String, _>("canonical_party_id").map_err(policy_row_error)?
            == canonical_party_id.as_str()
        && row.try_get::<Vec<String>, _>("scopes").map_err(policy_row_error)?
            == ["all_processing"]
        && row.try_get::<Vec<String>, _>("channels").map_err(policy_row_error)?.is_empty()
        && row.try_get::<i64, _>("starts_at_unix_nanos").map_err(policy_row_error)?
            == invocation.starts_at_unix_nanos
        && row.try_get::<Option<i64>, _>("expires_at_unix_nanos").map_err(policy_row_error)?
            == invocation.expires_at_unix_nanos
        && row.try_get::<String, _>("reason").map_err(policy_row_error)?
            == invocation.reason.trim()
        && row.try_get::<String, _>("legal_basis").map_err(policy_row_error)?
            == invocation.legal_basis.trim()
        && row.try_get::<String, _>("policy_version").map_err(policy_row_error)?
            == invocation.policy_version.trim()
        && row.try_get::<i64, _>("placed_at_unix_nanos").map_err(policy_row_error)?
            == invocation.proposed_placed_at_unix_nanos
        && row.try_get::<String, _>("placed_by_actor_id").map_err(policy_row_error)?
            == invocation.actor_id.as_str()
        && row.try_get::<String, _>("request_id").map_err(policy_row_error)?
            == invocation.request_id.as_str()
        && row.try_get::<i64, _>("restriction_version").map_err(policy_row_error)? == 1;
    if !matches {
        return Err(restriction_conflict());
    }
    Ok(Some(RestrictionPlacementCommit {
        restriction: build_restriction(invocation, canonical_party_id)?,
        replayed: true,
    }))
}

fn build_restriction(
    invocation: &RestrictionPlacementInvocation,
    canonical_party_id: &RecordId,
) -> Result<ProcessingRestriction, SdkError> {
    ProcessingRestriction::new(
        invocation.restriction_id.clone(),
        invocation.tenant_id.clone(),
        invocation.privacy_case_id.clone(),
        canonical_party_id.clone(),
        ProcessingRestrictionState::Active,
        BTreeSet::from([ProcessingRestrictionScope::AllProcessing]),
        BTreeSet::<RestrictedChannel>::new(),
        invocation.starts_at_unix_nanos,
        invocation.expires_at_unix_nanos,
        invocation.reason.clone(),
        invocation.legal_basis.clone(),
        invocation.policy_version.clone(),
        invocation.proposed_placed_at_unix_nanos,
        invocation.actor_id.clone(),
    )
}

fn placement_status_allowed(status: PrivacyCaseStatus) -> bool {
    matches!(
        status,
        PrivacyCaseStatus::SubjectVerified
            | PrivacyCaseStatus::Scoping
            | PrivacyCaseStatus::Scoped
            | PrivacyCaseStatus::Planned
            | PrivacyCaseStatus::AwaitingApproval
            | PrivacyCaseStatus::Executing
            | PrivacyCaseStatus::Converging
            | PrivacyCaseStatus::FailedRetryable(_)
    )
}

async fn ensure_customer_privacy_active(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: &TenantId,
) -> Result<(), SdkError> {
    let status = sqlx::query_scalar::<_, String>(
        r#"
SELECT status
FROM crm.module_installations
WHERE tenant_id = $1
  AND module_id = $2
FOR SHARE
        "#,
    )
    .bind(tenant_id.as_str())
    .bind(MODULE_ID)
    .fetch_optional(transaction.as_mut())
    .await
    .map_err(policy_dependency_error)?;
    if status.as_deref() != Some("active") {
        return Err(SdkError::new(
            "CUSTOMER_PRIVACY_SUBJECT_POLICY_UNAVAILABLE",
            ErrorCategory::Conflict,
            true,
            "Customer Privacy policy is unavailable for the tenant.",
        ));
    }
    Ok(())
}

async fn bind_invocation(
    transaction: &mut Transaction<'_, Postgres>,
    invocation: &RestrictionPlacementInvocation,
) -> Result<(), SdkError> {
    sqlx::query(
        r#"
SELECT
  set_config('app.tenant_id', $1, true),
  set_config('app.actor_id', $2, true),
  set_config('app.request_id', $3, true),
  set_config('app.correlation_id', $4, true),
  set_config('app.trace_id', $5, true)
        "#,
    )
    .bind(invocation.tenant_id.as_str())
    .bind(invocation.actor_id.as_str())
    .bind(invocation.request_id.as_str())
    .bind(invocation.correlation_id.as_str())
    .bind(invocation.trace_id.as_str())
    .execute(transaction.as_mut())
    .await
    .map_err(storage_error)?;
    Ok(())
}

fn case_unavailable() -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_RESTRICTION_CASE_UNAVAILABLE",
        ErrorCategory::NotFound,
        false,
        "The Customer Privacy case is unavailable.",
    )
}

fn corrupt_case(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_RESTRICTION_CASE_CORRUPT",
        ErrorCategory::Internal,
        false,
        "The Customer Privacy case evidence is invalid.",
    )
    .with_internal_reference(reference.into())
}

fn restriction_conflict() -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_RESTRICTION_PLACEMENT_CONFLICT",
        ErrorCategory::Conflict,
        false,
        "The processing restriction placement conflicts with durable evidence.",
    )
}

fn policy_corrupt() -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_SUBJECT_POLICY_CORRUPT",
        ErrorCategory::Internal,
        false,
        "Customer Privacy policy evidence is invalid.",
    )
}

fn policy_stale() -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_SUBJECT_POLICY_STALE",
        ErrorCategory::Conflict,
        true,
        "Customer Privacy policy evidence is not effective yet.",
    )
}

fn policy_row_error(error: sqlx::Error) -> SdkError {
    policy_corrupt().with_internal_reference(error.to_string())
}

fn policy_dependency_error(error: sqlx::Error) -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_SUBJECT_POLICY_UNAVAILABLE",
        ErrorCategory::Internal,
        true,
        "Customer Privacy policy could not be evaluated.",
    )
    .with_internal_reference(error.to_string())
}

fn insert_error(error: sqlx::Error) -> SdkError {
    if let sqlx::Error::Database(database_error) = &error {
        if database_error.is_unique_violation() {
            return restriction_conflict().with_internal_reference(error.to_string());
        }
    }
    storage_error(error)
}

fn storage_error(error: sqlx::Error) -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_RESTRICTION_PERSISTENCE_FAILED",
        ErrorCategory::Internal,
        true,
        "Customer Privacy restriction persistence failed.",
    )
    .with_internal_reference(error.to_string())
}
