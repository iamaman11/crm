#![forbid(unsafe_code)]

use crm_module_sdk::{Clock, ModuleExecutionContext, SdkError};

/// Architecture marker for `crm-sales`.
pub const CRATE_NAME: &str = "crm-sales";

/// Minimal host-bound proof that a business module consumes governed SDK ports
/// rather than infrastructure clients. Domain behavior is added in the first
/// vertical slice after the capability execution pipeline exists.
pub fn observed_at(context: &ModuleExecutionContext, clock: &dyn Clock) -> Result<i64, SdkError> {
    context.validate()?;
    Ok(clock.now_unix_nanos())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crm_module_sdk::testing::FixedClock;
    use crm_module_sdk::{
        ActorId, BusinessTransactionId, CapabilityId, CapabilityVersion, CausationId,
        CorrelationId, ExecutionContext, IdempotencyKey, ModuleId, RequestId, SchemaVersion,
        TenantId, TraceId,
    };

    #[test]
    fn uses_injected_clock_with_governed_context() {
        let context = ModuleExecutionContext {
            module_id: ModuleId::try_new("crm.sales").unwrap(),
            execution: ExecutionContext {
                tenant_id: TenantId::try_new("tenant-a").unwrap(),
                actor_id: ActorId::try_new("actor-a").unwrap(),
                request_id: RequestId::try_new("request-a").unwrap(),
                correlation_id: CorrelationId::try_new("correlation-a").unwrap(),
                causation_id: CausationId::try_new("causation-a").unwrap(),
                trace_id: TraceId::try_new("trace-a").unwrap(),
                capability_id: CapabilityId::try_new("sales.observe").unwrap(),
                capability_version: CapabilityVersion::try_new("1.0.0").unwrap(),
                idempotency_key: IdempotencyKey::try_new("idempotency-a").unwrap(),
                business_transaction_id: BusinessTransactionId::try_new("transaction-a").unwrap(),
                schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
                request_started_at_unix_nanos: 1,
            },
        };
        let clock = FixedClock::new(42);

        assert_eq!(observed_at(&context, &clock).unwrap(), 42);
    }
}
