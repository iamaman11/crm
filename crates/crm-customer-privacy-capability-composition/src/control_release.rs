use crm_capability_runtime::{CapabilityRequest, TransactionalCapabilityExecutor};
use crm_core_data::{
    PostgresDataStore, PostgresTransactionalAggregateExecutor, TransactionalAggregateGuard,
};
use crm_customer_privacy::{
    LEGAL_HOLD_RECORD_TYPE, MODULE_ID, RESTRICTION_RECORD_TYPE, decode_legal_hold_state,
    decode_processing_restriction_state,
};
use crm_customer_privacy_capability_adapter::{
    CustomerPrivacyLegalHoldReleaseCapabilityPlanner,
    CustomerPrivacyRestrictionReleaseCapabilityPlanner,
    RELEASE_CUSTOMER_DATA_LEGAL_HOLD_CAPABILITY, RELEASE_PROCESSING_RESTRICTION_CAPABILITY,
    legal_hold_ref_from_release_request, processing_restriction_ref_from_release_request,
};
use crm_module_sdk::{ErrorCategory, PortFuture, SdkError};
use crm_party_reference_composition::lock_customer_subject_in_transaction;
use sqlx::{Postgres, Row, Transaction};
use std::sync::Arc;

#[derive(Debug, Default, Clone, Copy)]
pub struct PostgresCustomerPrivacyRestrictionReleaseGuard;

impl TransactionalAggregateGuard for PostgresCustomerPrivacyRestrictionReleaseGuard {
    fn check<'a>(
        &'a self,
        transaction: &'a mut Transaction<'_, Postgres>,
        request: &'a CapabilityRequest,
    ) -> PortFuture<'a, Result<(), SdkError>> {
        Box::pin(async move {
            ensure_release_coordinate(request, RELEASE_PROCESSING_RESTRICTION_CAPABILITY)?;
            let reference = processing_restriction_ref_from_release_request(request)?;
            let payload = load_control_payload(
                transaction,
                request,
                RESTRICTION_RECORD_TYPE,
                reference.record_id.as_str(),
            )
            .await?;
            let restriction =
                decode_processing_restriction_state(&payload).map_err(control_state_unavailable)?;
            if restriction.restriction_id() != &reference.record_id
                || restriction.tenant_id() != &request.context.execution.tenant_id
            {
                return Err(control_not_found());
            }
            lock_customer_subject_in_transaction(
                transaction,
                &request.context.execution.tenant_id,
                restriction.canonical_party_id(),
            )
            .await
            .map_err(subject_lock_unavailable)
        })
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct PostgresCustomerPrivacyLegalHoldReleaseGuard;

impl TransactionalAggregateGuard for PostgresCustomerPrivacyLegalHoldReleaseGuard {
    fn check<'a>(
        &'a self,
        transaction: &'a mut Transaction<'_, Postgres>,
        request: &'a CapabilityRequest,
    ) -> PortFuture<'a, Result<(), SdkError>> {
        Box::pin(async move {
            ensure_release_coordinate(request, RELEASE_CUSTOMER_DATA_LEGAL_HOLD_CAPABILITY)?;
            let reference = legal_hold_ref_from_release_request(request)?;
            let payload = load_control_payload(
                transaction,
                request,
                LEGAL_HOLD_RECORD_TYPE,
                reference.record_id.as_str(),
            )
            .await?;
            let hold = decode_legal_hold_state(&payload).map_err(control_state_unavailable)?;
            if hold.hold_id() != &reference.record_id
                || hold.tenant_id() != &request.context.execution.tenant_id
            {
                return Err(control_not_found());
            }
            lock_customer_subject_in_transaction(
                transaction,
                &request.context.execution.tenant_id,
                hold.canonical_party_id(),
            )
            .await
            .map_err(subject_lock_unavailable)
        })
    }
}

pub fn postgres_restriction_release_executor(
    store: PostgresDataStore,
) -> Arc<dyn TransactionalCapabilityExecutor> {
    Arc::new(PostgresTransactionalAggregateExecutor::guarded(
        store,
        Arc::new(CustomerPrivacyRestrictionReleaseCapabilityPlanner),
        Arc::new(PostgresCustomerPrivacyRestrictionReleaseGuard),
    ))
}

pub fn postgres_legal_hold_release_executor(
    store: PostgresDataStore,
) -> Arc<dyn TransactionalCapabilityExecutor> {
    Arc::new(PostgresTransactionalAggregateExecutor::guarded(
        store,
        Arc::new(CustomerPrivacyLegalHoldReleaseCapabilityPlanner),
        Arc::new(PostgresCustomerPrivacyLegalHoldReleaseGuard),
    ))
}

async fn load_control_payload(
    transaction: &mut Transaction<'_, Postgres>,
    request: &CapabilityRequest,
    record_type: &'static str,
    record_id: &str,
) -> Result<Vec<u8>, SdkError> {
    let row = sqlx::query(
        r#"
        SELECT payload_bytes
        FROM crm.records
        WHERE tenant_id = $1
          AND owner_module_id = $2
          AND record_type = $3
          AND record_id = $4
          AND deleted_at IS NULL
        "#,
    )
    .bind(request.context.execution.tenant_id.as_str())
    .bind(MODULE_ID)
    .bind(record_type)
    .bind(record_id)
    .fetch_optional(&mut **transaction)
    .await
    .map_err(control_store_unavailable)?;

    row.map(|row| row.try_get("payload_bytes"))
        .transpose()
        .map_err(control_store_unavailable)?
        .ok_or_else(control_not_found)
}

fn ensure_release_coordinate(
    request: &CapabilityRequest,
    capability_id: &'static str,
) -> Result<(), SdkError> {
    if request.context.module_id.as_str() != MODULE_ID
        || request.context.execution.capability_id.as_str() != capability_id
        || request.context.execution.capability_version.as_str() != "1.0.0"
    {
        return Err(SdkError::new(
            "CUSTOMER_PRIVACY_CONTROL_RELEASE_GUARD_INVALID",
            ErrorCategory::Internal,
            false,
            "The Customer Privacy control release guard is not configured safely.",
        ));
    }
    Ok(())
}

fn control_not_found() -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_CONTROL_NOT_FOUND",
        ErrorCategory::NotFound,
        false,
        "The requested Customer Privacy control was not found.",
    )
}

fn control_state_unavailable(error: SdkError) -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_CONTROL_STATE_UNAVAILABLE",
        ErrorCategory::Unavailable,
        true,
        "The requested Customer Privacy control is temporarily unavailable.",
    )
    .with_internal_reference(error.code)
}

fn control_store_unavailable(error: sqlx::Error) -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_CONTROL_STORE_UNAVAILABLE",
        ErrorCategory::Unavailable,
        true,
        "The requested Customer Privacy control is temporarily unavailable.",
    )
    .with_internal_reference(error.to_string())
}

fn subject_lock_unavailable(error: SdkError) -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_CONTROL_SUBJECT_LOCK_UNAVAILABLE",
        ErrorCategory::Unavailable,
        true,
        "The Customer Privacy control subject could not be locked safely.",
    )
    .with_internal_reference(error.code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_guards_are_shareable_transaction_guards() {
        fn require_guard<T: TransactionalAggregateGuard + Send + Sync>() {}
        require_guard::<PostgresCustomerPrivacyRestrictionReleaseGuard>();
        require_guard::<PostgresCustomerPrivacyLegalHoldReleaseGuard>();
    }
}
