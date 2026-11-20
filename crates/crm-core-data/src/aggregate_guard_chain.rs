use crate::TransactionalAggregateGuard;
use crm_capability_runtime::CapabilityRequest;
use crm_module_sdk::{ErrorCategory, PortFuture, SdkError};
use sqlx::{Postgres, Transaction};
use std::fmt;
use std::sync::Arc;

/// Deterministically composes multiple final transaction guards without adding
/// capability-specific branching to the generic aggregate executor.
#[derive(Clone)]
pub struct TransactionalAggregateGuardChain {
    guards: Vec<Arc<dyn TransactionalAggregateGuard>>,
}

impl fmt::Debug for TransactionalAggregateGuardChain {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TransactionalAggregateGuardChain")
            .field("guard_count", &self.guards.len())
            .finish()
    }
}

impl TransactionalAggregateGuardChain {
    pub fn new(
        guards: impl IntoIterator<Item = Arc<dyn TransactionalAggregateGuard>>,
    ) -> Result<Self, SdkError> {
        let guards = guards.into_iter().collect::<Vec<_>>();
        if guards.is_empty() {
            return Err(SdkError::new(
                "TRANSACTIONAL_GUARD_CHAIN_EMPTY",
                ErrorCategory::Internal,
                false,
                "The transaction guard chain is not configured safely.",
            ));
        }
        Ok(Self { guards })
    }

    pub fn len(&self) -> usize {
        self.guards.len()
    }

    pub fn is_empty(&self) -> bool {
        self.guards.is_empty()
    }
}

impl TransactionalAggregateGuard for TransactionalAggregateGuardChain {
    fn check<'a>(
        &'a self,
        transaction: &'a mut Transaction<'_, Postgres>,
        request: &'a CapabilityRequest,
    ) -> PortFuture<'a, Result<(), SdkError>> {
        Box::pin(async move {
            for guard in &self.guards {
                guard.check(transaction, request).await?;
            }
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_chain_is_rejected() {
        let guards: Vec<Arc<dyn TransactionalAggregateGuard>> = Vec::new();
        let error = TransactionalAggregateGuardChain::new(guards).unwrap_err();
        assert_eq!(error.code, "TRANSACTIONAL_GUARD_CHAIN_EMPTY");
        assert!(!error.retryable);
    }
}
