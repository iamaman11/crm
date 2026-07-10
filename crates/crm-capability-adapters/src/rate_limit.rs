use crm_capability_runtime::{
    CapabilityDefinition, CapabilityRateLimiter, CapabilityRequest, RateLimitDecision,
};
use crm_module_sdk::{
    ActorId, CapabilityId, CapabilityVersion, Clock, ErrorCategory, PortFuture, SdkError, TenantId,
};
use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::sync::{Arc, Mutex, RwLock};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixedWindowPolicy {
    pub policy_id: String,
    pub maximum_requests: u64,
    pub window_nanos: i64,
}

impl FixedWindowPolicy {
    fn validate(&self) -> Result<(), RateLimitStoreError> {
        if self.policy_id.is_empty() || self.maximum_requests == 0 || self.window_nanos <= 0 {
            return Err(RateLimitStoreError::InvalidPolicy);
        }
        Ok(())
    }
}

#[derive(Debug)]
pub enum RateLimitStoreError {
    InvalidPolicy,
    Poisoned,
}

impl fmt::Display for RateLimitStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPolicy => formatter.write_str("rate-limit policy is invalid"),
            Self::Poisoned => formatter.write_str("rate-limit policy store lock is poisoned"),
        }
    }
}

impl Error for RateLimitStoreError {}

#[derive(Debug, Default)]
struct PolicyState {
    revision: u64,
    policies: BTreeMap<String, FixedWindowPolicy>,
}

#[derive(Debug, Clone, Default)]
pub struct RateLimitPolicyStore {
    state: Arc<RwLock<PolicyState>>,
}

impl RateLimitPolicyStore {
    pub fn upsert(&self, policy: FixedWindowPolicy) -> Result<u64, RateLimitStoreError> {
        policy.validate()?;
        let mut state = self
            .state
            .write()
            .map_err(|_| RateLimitStoreError::Poisoned)?;
        state.revision = state.revision.saturating_add(1);
        state.policies.insert(policy.policy_id.clone(), policy);
        Ok(state.revision)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct RateLimitKey {
    tenant_id: TenantId,
    actor_id: ActorId,
    policy_id: String,
    capability_id: CapabilityId,
    capability_version: CapabilityVersion,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct WindowCounter {
    window_start_unix_nanos: i64,
    count: u64,
}

#[derive(Clone)]
pub struct FixedWindowRateLimiter {
    policies: RateLimitPolicyStore,
    counters: Arc<Mutex<BTreeMap<RateLimitKey, WindowCounter>>>,
    clock: Arc<dyn Clock>,
}

impl fmt::Debug for FixedWindowRateLimiter {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FixedWindowRateLimiter")
            .field("policies", &self.policies)
            .field("clock", &"dyn Clock")
            .finish_non_exhaustive()
    }
}

impl FixedWindowRateLimiter {
    pub fn new(policies: RateLimitPolicyStore, clock: Arc<dyn Clock>) -> Self {
        Self {
            policies,
            counters: Arc::new(Mutex::new(BTreeMap::new())),
            clock,
        }
    }
}

impl CapabilityRateLimiter for FixedWindowRateLimiter {
    fn check<'a>(
        &'a self,
        definition: &'a CapabilityDefinition,
        request: &'a CapabilityRequest,
    ) -> PortFuture<'a, Result<RateLimitDecision, SdkError>> {
        Box::pin(async move {
            let policy_id = definition.rate_limit_policy_id.as_ref().ok_or_else(|| {
                SdkError::new(
                    "RATE_LIMIT_POLICY_BINDING_MISSING",
                    ErrorCategory::Internal,
                    false,
                    "The rate-limit configuration is invalid.",
                )
            })?;
            let policy_state = self
                .policies
                .state
                .read()
                .map_err(|_| unavailable_error())?;
            let policy = policy_state
                .policies
                .get(policy_id)
                .cloned()
                .ok_or_else(|| {
                    SdkError::new(
                        "RATE_LIMIT_POLICY_NOT_FOUND",
                        ErrorCategory::Dependency,
                        false,
                        "The rate-limit policy is unavailable.",
                    )
                    .with_internal_reference(policy_id)
                })?;
            let policy_revision = policy_state.revision;
            drop(policy_state);

            let now = self.clock.now_unix_nanos();
            if now < 0 {
                return Err(SdkError::new(
                    "RATE_LIMIT_CLOCK_INVALID",
                    ErrorCategory::Internal,
                    false,
                    "The rate-limit clock is invalid.",
                ));
            }
            let window_start = now.div_euclid(policy.window_nanos) * policy.window_nanos;
            let key = RateLimitKey {
                tenant_id: request.context.execution.tenant_id.clone(),
                actor_id: request.context.execution.actor_id.clone(),
                policy_id: policy.policy_id.clone(),
                capability_id: definition.capability_id.clone(),
                capability_version: definition.capability_version.clone(),
            };
            let mut counters = self.counters.lock().map_err(|_| unavailable_error())?;
            let counter = counters.entry(key.clone()).or_insert(WindowCounter {
                window_start_unix_nanos: window_start,
                count: 0,
            });
            if counter.window_start_unix_nanos != window_start {
                *counter = WindowCounter {
                    window_start_unix_nanos: window_start,
                    count: 0,
                };
            }
            let allowed = counter.count < policy.maximum_requests;
            if allowed {
                counter.count = counter.count.saturating_add(1);
            }
            let retry_after_millis = (!allowed).then(|| {
                let remaining_nanos = window_start
                    .saturating_add(policy.window_nanos)
                    .saturating_sub(now);
                u64::try_from(remaining_nanos)
                    .unwrap_or(u64::MAX)
                    .saturating_add(999_999)
                    / 1_000_000
            });
            Ok(RateLimitDecision {
                allowed,
                decision_id: format!(
                    "rate:{}:{}:{}:{}:{}",
                    policy_revision, key.tenant_id, key.actor_id, window_start, counter.count
                ),
                retry_after_millis,
            })
        })
    }
}

fn unavailable_error() -> SdkError {
    SdkError::new(
        "RATE_LIMIT_STORE_UNAVAILABLE",
        ErrorCategory::Unavailable,
        true,
        "Rate limiting is temporarily unavailable.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crm_capability_runtime::{CapabilityRisk, PayloadContract};
    use crm_module_sdk::testing::FixedClock;
    use crm_module_sdk::{
        BusinessTransactionId, CausationId, CorrelationId, DataClass, ExecutionContext,
        IdempotencyKey, ModuleExecutionContext, ModuleId, PayloadEncoding, RequestId,
        RetentionPolicyId, SchemaId, SchemaVersion, TraceId, TypedPayload,
    };

    #[tokio::test]
    async fn enforces_limit_and_resets_at_next_window() {
        let clock = Arc::new(FixedClock::new(5));
        let policies = RateLimitPolicyStore::default();
        policies
            .upsert(FixedWindowPolicy {
                policy_id: "write-small".to_owned(),
                maximum_requests: 2,
                window_nanos: 10,
            })
            .unwrap();
        let limiter = FixedWindowRateLimiter::new(policies, clock.clone());
        let definition = definition();
        let request = request("tenant-1");

        assert!(limiter.check(&definition, &request).await.unwrap().allowed);
        assert!(limiter.check(&definition, &request).await.unwrap().allowed);
        let denied = limiter.check(&definition, &request).await.unwrap();
        assert!(!denied.allowed);
        assert_eq!(denied.retry_after_millis, Some(1));

        clock.set(10);
        assert!(limiter.check(&definition, &request).await.unwrap().allowed);
    }

    #[tokio::test]
    async fn counters_are_tenant_scoped() {
        let clock = Arc::new(FixedClock::new(5));
        let policies = RateLimitPolicyStore::default();
        policies
            .upsert(FixedWindowPolicy {
                policy_id: "write-small".to_owned(),
                maximum_requests: 1,
                window_nanos: 100,
            })
            .unwrap();
        let limiter = FixedWindowRateLimiter::new(policies, clock);
        let definition = definition();

        assert!(
            limiter
                .check(&definition, &request("tenant-1"))
                .await
                .unwrap()
                .allowed
        );
        assert!(
            limiter
                .check(&definition, &request("tenant-2"))
                .await
                .unwrap()
                .allowed
        );
    }

    fn definition() -> CapabilityDefinition {
        CapabilityDefinition {
            capability_id: CapabilityId::try_new("crm.sales.deal.create").unwrap(),
            capability_version: CapabilityVersion::try_new("1.0.0").unwrap(),
            owner_module_id: ModuleId::try_new("crm.sales").unwrap(),
            input_contract: PayloadContract {
                owner: ModuleId::try_new("crm.sales").unwrap(),
                schema_id: SchemaId::try_new("crm.sales.deal.create").unwrap(),
                schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
                descriptor_hash: [1; 32],
                allowed_data_classes: vec![DataClass::Internal],
                allowed_encodings: vec![PayloadEncoding::Json],
                maximum_size_bytes: 4096,
            },
            output_contract: None,
            risk: CapabilityRisk::Medium,
            mutation: true,
            requires_idempotency: true,
            requires_approval: false,
            authorization_policy_id: "sales.deal.create".to_owned(),
            rate_limit_policy_id: Some("write-small".to_owned()),
        }
    }

    fn request(tenant: &str) -> CapabilityRequest {
        CapabilityRequest {
            context: ModuleExecutionContext {
                module_id: ModuleId::try_new("crm.sales").unwrap(),
                execution: ExecutionContext {
                    tenant_id: TenantId::try_new(tenant).unwrap(),
                    actor_id: ActorId::try_new("actor-1").unwrap(),
                    request_id: RequestId::try_new("request-1").unwrap(),
                    correlation_id: CorrelationId::try_new("correlation-1").unwrap(),
                    causation_id: CausationId::try_new("causation-1").unwrap(),
                    trace_id: TraceId::try_new("trace-1").unwrap(),
                    capability_id: CapabilityId::try_new("crm.sales.deal.create").unwrap(),
                    capability_version: CapabilityVersion::try_new("1.0.0").unwrap(),
                    idempotency_key: IdempotencyKey::try_new("idem-1").unwrap(),
                    business_transaction_id: BusinessTransactionId::try_new("txn-1").unwrap(),
                    schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
                    request_started_at_unix_nanos: 5,
                },
            },
            input: TypedPayload {
                owner: ModuleId::try_new("crm.sales").unwrap(),
                schema_id: SchemaId::try_new("crm.sales.deal.create").unwrap(),
                schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
                descriptor_hash: [1; 32],
                data_class: DataClass::Internal,
                encoding: PayloadEncoding::Json,
                maximum_size_bytes: 4096,
                retention_policy_id: RetentionPolicyId::try_new("standard").unwrap(),
                bytes: b"{}".to_vec(),
            },
            input_hash: [2; 32],
            approval: None,
        }
    }
}
