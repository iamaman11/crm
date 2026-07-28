use crm_capability_runtime::CapabilityRequest;
use crm_module_sdk::{PortFuture, RecordId, SdkError};
use sqlx::{Postgres, Transaction};

/// Classifies the protected customer-subject boundary without embedding any
/// Customer Privacy aggregate or policy implementation in the platform core.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CustomerSubjectOperationClass {
    Processing,
    Communication,
}

/// Transaction-scoped final policy boundary for customer-subject operations.
///
/// Implementations must acquire the platform-wide tenant + canonical Party
/// subject lock and evaluate authoritative live policy state in the supplied
/// PostgreSQL transaction before returning `Ok(())`. An unavailable, stale,
/// corrupt or cross-tenant decision must return a bounded error rather than
/// being interpreted as allow.
///
/// Owner packages depend only on this stable port. Customer Privacy remains the
/// policy owner and supplies the production implementation separately.
pub trait TransactionalCustomerSubjectPolicyPort: Send + Sync {
    fn lock_and_enforce<'a>(
        &'a self,
        transaction: &'a mut Transaction<'_, Postgres>,
        request: &'a CapabilityRequest,
        canonical_party_id: &'a RecordId,
        operation_class: CustomerSubjectOperationClass,
    ) -> PortFuture<'a, Result<(), SdkError>>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn operation_classes_are_explicit_and_non_granting() {
        assert_ne!(
            CustomerSubjectOperationClass::Processing,
            CustomerSubjectOperationClass::Communication
        );
        assert_eq!(
            format!("{:?}", CustomerSubjectOperationClass::Processing),
            "Processing"
        );
        assert_eq!(
            format!("{:?}", CustomerSubjectOperationClass::Communication),
            "Communication"
        );
    }
}
