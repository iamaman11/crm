use crm_capability_runtime::{CapabilityRequest, TransactionalCapabilityExecutor};
use crm_core_data::{
    PostgresDataStore, PostgresTransactionalAggregateExecutor, TransactionalAggregateGuard,
};
use crm_customer_privacy::MODULE_ID;
use crm_customer_privacy_capability_adapter::{
    CustomerPrivacyLegalHoldPlaceCapabilityPlanner, PLACE_CUSTOMER_DATA_LEGAL_HOLD_CAPABILITY,
    customer_data_legal_hold_canonical_party_id_from_request,
};
use crm_identity_resolution_topology_composition::require_current_canonical_party_in_transaction;
use crm_module_sdk::{ErrorCategory, PortFuture, SdkError};
use crm_party_reference_composition::lock_customer_subject_in_transaction;
use sqlx::{Postgres, Transaction};
use std::sync::Arc;

/// Final placement guard for customer-data legal holds.
///
/// The guard proves current canonical Party identity and acquires the shared
/// tenant + subject lock before the immutable hold is persisted by the generic
/// aggregate executor in the same transaction.
#[derive(Debug, Default, Clone, Copy)]
pub struct PostgresCustomerPrivacyLegalHoldPlacementGuard;

impl TransactionalAggregateGuard for PostgresCustomerPrivacyLegalHoldPlacementGuard {
    fn check<'a>(
        &'a self,
        transaction: &'a mut Transaction<'_, Postgres>,
        request: &'a CapabilityRequest,
    ) -> PortFuture<'a, Result<(), SdkError>> {
        Box::pin(async move {
            ensure_place_coordinate(request)?;
            let canonical_party_id =
                customer_data_legal_hold_canonical_party_id_from_request(request)?;
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

pub fn postgres_legal_hold_place_executor(
    store: PostgresDataStore,
) -> Arc<dyn TransactionalCapabilityExecutor> {
    Arc::new(PostgresTransactionalAggregateExecutor::guarded(
        store,
        Arc::new(CustomerPrivacyLegalHoldPlaceCapabilityPlanner),
        Arc::new(PostgresCustomerPrivacyLegalHoldPlacementGuard),
    ))
}

fn ensure_place_coordinate(request: &CapabilityRequest) -> Result<(), SdkError> {
    if request.context.module_id.as_str() != MODULE_ID
        || request.context.execution.capability_id.as_str()
            != PLACE_CUSTOMER_DATA_LEGAL_HOLD_CAPABILITY
        || request.context.execution.capability_version.as_str() != "1.0.0"
    {
        return Err(SdkError::new(
            "CUSTOMER_PRIVACY_LEGAL_HOLD_PLACEMENT_GUARD_INVALID",
            ErrorCategory::Internal,
            false,
            "The customer-data legal-hold placement guard is not configured safely.",
        ));
    }
    Ok(())
}

fn canonical_party_unavailable(error: SdkError) -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_LEGAL_HOLD_CANONICAL_PARTY_UNAVAILABLE",
        error.category,
        error.retryable,
        "The canonical Party for the customer-data legal hold could not be verified.",
    )
    .with_internal_reference(error.code)
}

fn subject_lock_unavailable(error: SdkError) -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_LEGAL_HOLD_SUBJECT_LOCK_UNAVAILABLE",
        ErrorCategory::Unavailable,
        true,
        "The customer-data legal-hold subject could not be locked safely.",
    )
    .with_internal_reference(error.code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placement_guard_is_shareable_transaction_guard() {
        fn require_guard<T: TransactionalAggregateGuard + Send + Sync>() {}
        require_guard::<PostgresCustomerPrivacyLegalHoldPlacementGuard>();
    }
}
