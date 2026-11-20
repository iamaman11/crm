use crm_capability_runtime::{CapabilityRequest, TransactionalCapabilityExecutor};
use crm_core_data::{
    PostgresDataStore, PostgresTransactionalAggregateExecutor, TransactionalAggregateGuard,
};
use crm_customer_privacy::MODULE_ID;
use crm_customer_privacy_capability_adapter::{
    CustomerPrivacyRestrictionPlaceCapabilityPlanner, PLACE_PROCESSING_RESTRICTION_CAPABILITY,
    processing_restriction_canonical_party_id_from_request,
};
use crm_identity_resolution_topology_composition::require_current_canonical_party_in_transaction;
use crm_module_sdk::{ErrorCategory, PortFuture, SdkError};
use crm_party_reference_composition::lock_customer_subject_in_transaction;
use sqlx::{Postgres, Transaction};
use std::sync::Arc;

/// Final placement guard for immediate deny-only processing restrictions.
///
/// The guard proves that the submitted Party is the current authoritative
/// canonical Party, then acquires the platform-wide tenant + subject lock. The
/// restriction aggregate is persisted only after both proofs succeed inside the
/// same transaction used by the shared aggregate executor.
#[derive(Debug, Default, Clone, Copy)]
pub struct PostgresCustomerPrivacyRestrictionPlacementGuard;

impl TransactionalAggregateGuard for PostgresCustomerPrivacyRestrictionPlacementGuard {
    fn check<'a>(
        &'a self,
        transaction: &'a mut Transaction<'_, Postgres>,
        request: &'a CapabilityRequest,
    ) -> PortFuture<'a, Result<(), SdkError>> {
        Box::pin(async move {
            ensure_place_coordinate(request)?;
            let canonical_party_id =
                processing_restriction_canonical_party_id_from_request(request)?;
            require_current_canonical_party_in_transaction(
                transaction,
                &request.context.execution.tenant_id,
                &canonical_party_id,
            )
            .await
            .map_err(canonical_party_unavailable)?;
            lock_customer_subject_in_transaction(
                transaction,
                &request.context.execution.tenant_id,
                &canonical_party_id,
            )
            .await
            .map_err(subject_lock_unavailable)
        })
    }
}

pub fn postgres_restriction_place_executor(
    store: PostgresDataStore,
) -> Arc<dyn TransactionalCapabilityExecutor> {
    Arc::new(PostgresTransactionalAggregateExecutor::guarded(
        store,
        Arc::new(CustomerPrivacyRestrictionPlaceCapabilityPlanner),
        Arc::new(PostgresCustomerPrivacyRestrictionPlacementGuard),
    ))
}

fn ensure_place_coordinate(request: &CapabilityRequest) -> Result<(), SdkError> {
    if request.context.module_id.as_str() != MODULE_ID
        || request.context.execution.capability_id.as_str()
            != PLACE_PROCESSING_RESTRICTION_CAPABILITY
        || request.context.execution.capability_version.as_str() != "1.0.0"
    {
        return Err(SdkError::new(
            "CUSTOMER_PRIVACY_RESTRICTION_PLACEMENT_GUARD_INVALID",
            ErrorCategory::Internal,
            false,
            "The processing restriction placement guard is not configured safely.",
        ));
    }
    Ok(())
}

fn canonical_party_unavailable(error: SdkError) -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_RESTRICTION_CANONICAL_PARTY_UNAVAILABLE",
        error.category,
        error.retryable,
        "The canonical Party for the processing restriction could not be verified.",
    )
    .with_internal_reference(error.code)
}

fn subject_lock_unavailable(error: SdkError) -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_RESTRICTION_SUBJECT_LOCK_UNAVAILABLE",
        ErrorCategory::Unavailable,
        true,
        "The processing restriction subject could not be locked safely.",
    )
    .with_internal_reference(error.code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placement_guard_is_shareable_transaction_guard() {
        fn require_guard<T: TransactionalAggregateGuard + Send + Sync>() {}
        require_guard::<PostgresCustomerPrivacyRestrictionPlacementGuard>();
    }
}
