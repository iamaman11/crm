use crm_capability_runtime::CapabilityRequest;
use crm_contact_points_capability_adapter::{
    CREATE_CAPABILITY, MODULE_ID, referenced_party_id_from_create,
};
use crm_core_data::{
    CustomerSubjectOperationClass, TransactionalAggregateGuard,
    TransactionalCustomerSubjectPolicyPort,
    postgres_sqlx::{Postgres, Transaction},
};
use crm_identity_resolution_topology_composition::require_current_canonical_party_in_transaction;
use crm_module_sdk::{ErrorCategory, PortFuture, SdkError};
use std::fmt;
use std::sync::Arc;

/// Final deny-only policy guard for the bounded protected-owner boundary
/// `contact-points.contact-point.create@1.0.0`.
///
/// The Contact Point request already carries exactly one Party reference. This
/// guard proves that Party is currently canonical, then invokes the authoritative
/// Customer Privacy policy. The policy acquires the shared tenant + subject lock
/// and reloads live restriction state before the owner plan is persisted.
#[derive(Clone)]
pub struct ContactPointCreateCustomerSubjectGuard {
    policy: Arc<dyn TransactionalCustomerSubjectPolicyPort>,
}

impl ContactPointCreateCustomerSubjectGuard {
    pub fn new(policy: Arc<dyn TransactionalCustomerSubjectPolicyPort>) -> Self {
        Self { policy }
    }
}

impl fmt::Debug for ContactPointCreateCustomerSubjectGuard {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ContactPointCreateCustomerSubjectGuard")
            .field("policy", &"dyn TransactionalCustomerSubjectPolicyPort")
            .finish()
    }
}

impl TransactionalAggregateGuard for ContactPointCreateCustomerSubjectGuard {
    fn check<'a>(
        &'a self,
        transaction: &'a mut Transaction<'_, Postgres>,
        request: &'a CapabilityRequest,
    ) -> PortFuture<'a, Result<(), SdkError>> {
        Box::pin(async move {
            ensure_create_coordinate(request)?;
            let canonical_party_id = referenced_party_id_from_create(request)?;
            require_current_canonical_party_in_transaction(
                transaction,
                &request.context.execution.tenant_id,
                &canonical_party_id,
            )
            .await
            .map_err(canonical_party_unavailable)?;
            self.policy
                .lock_and_enforce(
                    transaction,
                    request,
                    &canonical_party_id,
                    CustomerSubjectOperationClass::Processing,
                )
                .await
        })
    }
}

fn ensure_create_coordinate(request: &CapabilityRequest) -> Result<(), SdkError> {
    if request.context.module_id.as_str() != MODULE_ID
        || request.context.execution.capability_id.as_str() != CREATE_CAPABILITY
        || request.context.execution.capability_version.as_str() != "1.0.0"
    {
        return Err(SdkError::new(
            "CONTACT_POINTS_PRIVACY_GUARD_INVALID",
            ErrorCategory::Internal,
            false,
            "The Contact Point privacy guard is not configured safely.",
        ));
    }
    Ok(())
}

fn canonical_party_unavailable(error: SdkError) -> SdkError {
    SdkError::new(
        "CONTACT_POINTS_CANONICAL_PARTY_UNAVAILABLE",
        error.category,
        error.retryable,
        "The canonical Party for the Contact Point could not be verified.",
    )
    .with_internal_reference(error.code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guard_is_shareable_transaction_guard() {
        fn require_guard<T: TransactionalAggregateGuard + Send + Sync>() {}
        require_guard::<ContactPointCreateCustomerSubjectGuard>();
    }
}
