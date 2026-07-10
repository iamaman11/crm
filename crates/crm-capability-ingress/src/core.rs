use crate::{
    AuthenticationError, CapabilityCallEnvelope, ContextResolutionError, ExecutionContextResolver,
    RequestAuthenticator,
};
use crm_capability_runtime::{
    CapabilityExecutionResult, CapabilityGateway, GatewayError, gateway_error_to_sdk,
};
use crm_module_sdk::{CorrelationId, ErrorCategory, RequestId, SdkError, TraceId};
use std::error::Error;
use std::fmt;
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionReceipt {
    pub request_id: RequestId,
    pub correlation_id: CorrelationId,
    pub trace_id: TraceId,
    pub result: CapabilityExecutionResult,
}

#[derive(Debug)]
pub enum IngressError {
    Authentication(AuthenticationError),
    Context(ContextResolutionError),
    Gateway(GatewayError),
    DeadlineExceeded,
}

impl fmt::Display for IngressError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Authentication(error) => write!(formatter, "authentication failed: {error}"),
            Self::Context(error) => write!(formatter, "execution context failed: {error}"),
            Self::Gateway(error) => write!(formatter, "capability gateway failed: {error}"),
            Self::DeadlineExceeded => formatter.write_str("capability request deadline exceeded"),
        }
    }
}

impl Error for IngressError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SafeTransportError {
    pub code: String,
    pub category: ErrorCategory,
    pub retryable: bool,
    pub safe_message: String,
    pub retry_after_millis: Option<u64>,
}

impl From<IngressError> for SafeTransportError {
    fn from(error: IngressError) -> Self {
        match error {
            IngressError::Authentication(error) => Self {
                code: error.code().to_owned(),
                category: ErrorCategory::Authentication,
                retryable: error.retryable(),
                safe_message: "Authentication failed.".to_owned(),
                retry_after_millis: None,
            },
            IngressError::Context(error) => {
                let category = if matches!(error, ContextResolutionError::TenantForbidden) {
                    ErrorCategory::Authorization
                } else if error.retryable() {
                    ErrorCategory::Unavailable
                } else {
                    ErrorCategory::InvalidArgument
                };
                Self {
                    code: error.code().to_owned(),
                    category,
                    retryable: error.retryable(),
                    safe_message: context_safe_message(&error).to_owned(),
                    retry_after_millis: None,
                }
            }
            IngressError::Gateway(GatewayError::RateLimited {
                retry_after_millis, ..
            }) => Self {
                code: "CAPABILITY_RATE_LIMITED".to_owned(),
                category: ErrorCategory::RateLimit,
                retryable: true,
                safe_message: "The capability rate limit was exceeded.".to_owned(),
                retry_after_millis,
            },
            IngressError::Gateway(error) => Self::from_sdk(gateway_error_to_sdk(error)),
            IngressError::DeadlineExceeded => Self {
                code: "CAPABILITY_DEADLINE_EXCEEDED".to_owned(),
                category: ErrorCategory::Unavailable,
                retryable: true,
                safe_message: "The capability request exceeded its deadline.".to_owned(),
                retry_after_millis: None,
            },
        }
    }
}

impl SafeTransportError {
    fn from_sdk(error: SdkError) -> Self {
        Self {
            code: error.code,
            category: error.category,
            retryable: error.retryable,
            safe_message: error.safe_message,
            retry_after_millis: None,
        }
    }
}

fn context_safe_message(error: &ContextResolutionError) -> &'static str {
    match error {
        ContextResolutionError::TenantForbidden => {
            "You are not permitted to access the requested tenant."
        }
        ContextResolutionError::TenantRequired | ContextResolutionError::TenantInvalid => {
            "A valid tenant is required."
        }
        ContextResolutionError::IdempotencyKeyRequired => {
            "An idempotency key is required for this request."
        }
        ContextResolutionError::InvalidTimeout | ContextResolutionError::TimeoutTooLarge => {
            "The requested timeout budget is invalid."
        }
        ContextResolutionError::InvalidIdentifier(_) => {
            "The execution context metadata is invalid."
        }
        ContextResolutionError::ClockInvalid
        | ContextResolutionError::IdentityGenerationUnavailable
        | ContextResolutionError::InvalidServerConfiguration => {
            "The execution context is temporarily unavailable."
        }
    }
}

#[derive(Clone)]
pub struct CapabilityIngress {
    authenticator: Arc<dyn RequestAuthenticator>,
    context_resolver: ExecutionContextResolver,
    gateway: Arc<CapabilityGateway>,
}

impl fmt::Debug for CapabilityIngress {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CapabilityIngress")
            .field("authenticator", &"dyn RequestAuthenticator")
            .field("context_resolver", &self.context_resolver)
            .field("gateway", &self.gateway)
            .finish()
    }
}

impl CapabilityIngress {
    pub fn new(
        authenticator: Arc<dyn RequestAuthenticator>,
        context_resolver: ExecutionContextResolver,
        gateway: Arc<CapabilityGateway>,
    ) -> Self {
        Self {
            authenticator,
            context_resolver,
            gateway,
        }
    }

    pub async fn execute(
        &self,
        authorization_value: &str,
        envelope: CapabilityCallEnvelope,
    ) -> Result<ExecutionReceipt, IngressError> {
        let principal = self
            .authenticator
            .authenticate(authorization_value)
            .await
            .map_err(IngressError::Authentication)?;
        let resolved = self
            .context_resolver
            .resolve(&principal, envelope)
            .map_err(IngressError::Context)?;
        let request_id = resolved.request.context.execution.request_id.clone();
        let correlation_id = resolved.request.context.execution.correlation_id.clone();
        let trace_id = resolved.request.context.execution.trace_id.clone();
        let duration = Duration::from_millis(resolved.timeout.duration_millis);
        let result = tokio::time::timeout(duration, self.gateway.execute(resolved.request))
            .await
            .map_err(|_| IngressError::DeadlineExceeded)?
            .map_err(IngressError::Gateway)?;
        Ok(ExecutionReceipt {
            request_id,
            correlation_id,
            trace_id,
            result,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AccessTokenGrant, AccessTokenStore, BearerTokenAuthenticator, CapabilityRoute,
        IngressMetadata, TimeoutPolicy,
    };
    use crm_capability_runtime::{
        ApprovalEvidence, AuthorizationDecision, CapabilityApprovalVerifier, CapabilityAuthorizer,
        CapabilityDefinition, CapabilityRateLimiter, CapabilityRegistryPort, CapabilityRequest,
        CapabilityRisk, CapabilitySemanticValidator, PayloadContract, RateLimitDecision,
        TransactionalCapabilityExecutor,
    };
    use crm_module_sdk::testing::{DeterministicRandom, FixedClock};
    use crm_module_sdk::{
        ActorId, CapabilityId, CapabilityVersion, Clock, DataClass, ModuleId, PayloadEncoding,
        PortFuture, RetentionPolicyId, SchemaId, SchemaVersion, SdkError, TenantId, TypedPayload,
    };
    use std::collections::BTreeSet;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct Registry(CapabilityDefinition);
    impl CapabilityRegistryPort for Registry {
        fn resolve<'a>(
            &'a self,
            _capability_id: &'a CapabilityId,
            _capability_version: &'a CapabilityVersion,
        ) -> PortFuture<'a, Result<Option<CapabilityDefinition>, SdkError>> {
            Box::pin(async move { Ok(Some(self.0.clone())) })
        }
    }

    struct Validator;
    impl CapabilitySemanticValidator for Validator {
        fn validate<'a>(
            &'a self,
            _definition: &'a CapabilityDefinition,
            _request: &'a CapabilityRequest,
        ) -> PortFuture<'a, Result<(), SdkError>> {
            Box::pin(async { Ok(()) })
        }
    }

    struct RateLimiter;
    impl CapabilityRateLimiter for RateLimiter {
        fn check<'a>(
            &'a self,
            _definition: &'a CapabilityDefinition,
            _request: &'a CapabilityRequest,
        ) -> PortFuture<'a, Result<RateLimitDecision, SdkError>> {
            Box::pin(async {
                Ok(RateLimitDecision {
                    allowed: true,
                    decision_id: "rate-1".to_owned(),
                    retry_after_millis: None,
                })
            })
        }
    }

    struct ApprovalVerifier;
    impl CapabilityApprovalVerifier for ApprovalVerifier {
        fn verify<'a>(
            &'a self,
            _definition: &'a CapabilityDefinition,
            _request: &'a CapabilityRequest,
            _approval: &'a ApprovalEvidence,
        ) -> PortFuture<'a, Result<(), SdkError>> {
            Box::pin(async { Ok(()) })
        }
    }

    struct Authorizer;
    impl CapabilityAuthorizer for Authorizer {
        fn authorize<'a>(
            &'a self,
            _definition: &'a CapabilityDefinition,
            _request: &'a CapabilityRequest,
        ) -> PortFuture<'a, Result<AuthorizationDecision, SdkError>> {
            Box::pin(async {
                Ok(AuthorizationDecision {
                    allowed: true,
                    decision_id: "authz-1".to_owned(),
                    reason_code: "allowed".to_owned(),
                    policy_version: "1".to_owned(),
                })
            })
        }
    }

    struct Executor(Arc<AtomicUsize>);
    impl TransactionalCapabilityExecutor for Executor {
        fn execute<'a>(
            &'a self,
            _definition: &'a CapabilityDefinition,
            _request: CapabilityRequest,
        ) -> PortFuture<'a, Result<CapabilityExecutionResult, SdkError>> {
            Box::pin(async move {
                self.0.fetch_add(1, Ordering::SeqCst);
                Ok(CapabilityExecutionResult {
                    output: None,
                    affected_resources: Vec::new(),
                    replayed: false,
                })
            })
        }
    }

    #[tokio::test]
    async fn authentication_and_tenant_resolution_precede_gateway_execution() {
        let clock: Arc<dyn Clock> = Arc::new(FixedClock::new(100));
        let store = AccessTokenStore::default();
        let token = b"0123456789abcdef0123456789abcdef";
        store
            .issue(
                token,
                AccessTokenGrant {
                    actor_id: ActorId::try_new("actor-1").unwrap(),
                    tenant_ids: BTreeSet::from([TenantId::try_new("tenant-1").unwrap()]),
                    authentication_id: "session-1".to_owned(),
                    expires_at_unix_nanos: 1_000,
                },
            )
            .unwrap();
        let executor_calls = Arc::new(AtomicUsize::new(0));
        let definition = definition();
        let gateway = Arc::new(CapabilityGateway::new(
            Arc::new(Registry(definition)),
            Arc::new(Validator),
            Arc::new(RateLimiter),
            Arc::new(ApprovalVerifier),
            Arc::new(Authorizer),
            Arc::new(Executor(Arc::clone(&executor_calls))),
            Arc::clone(&clock),
        ));
        let ingress = CapabilityIngress::new(
            Arc::new(BearerTokenAuthenticator::new(store, Arc::clone(&clock))),
            ExecutionContextResolver::new(
                clock,
                Arc::new(DeterministicRandom::from_bytes(0_u8..64)),
                TimeoutPolicy {
                    default_millis: 500,
                    maximum_millis: 1_000,
                },
            )
            .unwrap(),
            gateway,
        );

        let denied = ingress
            .execute(
                "Bearer invalid-invalid-invalid-invalid",
                envelope("tenant-1"),
            )
            .await
            .unwrap_err();
        assert!(matches!(denied, IngressError::Authentication(_)));
        assert_eq!(executor_calls.load(Ordering::SeqCst), 0);

        let forbidden = ingress
            .execute(
                "Bearer 0123456789abcdef0123456789abcdef",
                envelope("tenant-2"),
            )
            .await
            .unwrap_err();
        assert!(matches!(forbidden, IngressError::Context(_)));
        assert_eq!(executor_calls.load(Ordering::SeqCst), 0);

        ingress
            .execute(
                "Bearer 0123456789abcdef0123456789abcdef",
                envelope("tenant-1"),
            )
            .await
            .unwrap();
        assert_eq!(executor_calls.load(Ordering::SeqCst), 1);
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
            rate_limit_policy_id: None,
        }
    }

    fn envelope(tenant: &str) -> CapabilityCallEnvelope {
        CapabilityCallEnvelope {
            route: CapabilityRoute {
                owner_module_id: ModuleId::try_new("crm.sales").unwrap(),
                capability_id: CapabilityId::try_new("crm.sales.deal.create").unwrap(),
                capability_version: CapabilityVersion::try_new("1.0.0").unwrap(),
                schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
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
            approval: None,
            metadata: IngressMetadata {
                tenant_id: Some(tenant.to_owned()),
                idempotency_key: Some("idem-1".to_owned()),
                ..IngressMetadata::default()
            },
        }
    }
}
