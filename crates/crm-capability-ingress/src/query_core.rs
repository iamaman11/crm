use crate::{
    AuthenticationError, ContextResolutionError, QueryCallEnvelope, QueryContextResolver,
    RequestAuthenticator, SafeTransportError,
};
use crm_module_sdk::{CorrelationId, ErrorCategory, RequestId, TraceId};
use crm_query_runtime::{
    QueryExecutionResult, QueryGateway, QueryGatewayError, query_gateway_error_to_sdk,
};
use std::error::Error;
use std::fmt;
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryExecutionReceipt {
    pub request_id: RequestId,
    pub correlation_id: CorrelationId,
    pub trace_id: TraceId,
    pub result: QueryExecutionResult,
}

#[derive(Debug)]
pub enum QueryIngressError {
    Authentication(AuthenticationError),
    Context(ContextResolutionError),
    Gateway(QueryGatewayError),
    DeadlineExceeded,
}

impl fmt::Display for QueryIngressError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Authentication(error) => write!(formatter, "authentication failed: {error}"),
            Self::Context(error) => write!(formatter, "query context failed: {error}"),
            Self::Gateway(error) => write!(formatter, "query gateway failed: {error}"),
            Self::DeadlineExceeded => formatter.write_str("query request deadline exceeded"),
        }
    }
}

impl Error for QueryIngressError {}

impl From<QueryIngressError> for SafeTransportError {
    fn from(error: QueryIngressError) -> Self {
        match error {
            QueryIngressError::Authentication(error) => Self {
                code: error.code().to_owned(),
                category: ErrorCategory::Authentication,
                retryable: error.retryable(),
                safe_message: "Authentication failed.".to_owned(),
                retry_after_millis: None,
            },
            QueryIngressError::Context(error) => {
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
                    safe_message: query_context_safe_message(&error).to_owned(),
                    retry_after_millis: None,
                }
            }
            QueryIngressError::Gateway(error) => {
                let error = query_gateway_error_to_sdk(error);
                Self {
                    code: error.code,
                    category: error.category,
                    retryable: error.retryable,
                    safe_message: error.safe_message,
                    retry_after_millis: None,
                }
            }
            QueryIngressError::DeadlineExceeded => Self {
                code: "QUERY_DEADLINE_EXCEEDED".to_owned(),
                category: ErrorCategory::Unavailable,
                retryable: true,
                safe_message: "The query request exceeded its deadline.".to_owned(),
                retry_after_millis: None,
            },
        }
    }
}

fn query_context_safe_message(error: &ContextResolutionError) -> &'static str {
    match error {
        ContextResolutionError::TenantForbidden => {
            "You are not permitted to access the requested tenant."
        }
        ContextResolutionError::TenantRequired | ContextResolutionError::TenantInvalid => {
            "A valid tenant is required."
        }
        ContextResolutionError::InvalidTimeout | ContextResolutionError::TimeoutTooLarge => {
            "The requested timeout budget is invalid."
        }
        ContextResolutionError::InvalidIdentifier(_) => {
            "The query execution context metadata is invalid."
        }
        ContextResolutionError::ClockInvalid
        | ContextResolutionError::IdentityGenerationUnavailable
        | ContextResolutionError::InvalidServerConfiguration => {
            "The query execution context is temporarily unavailable."
        }
        ContextResolutionError::IdempotencyKeyRequired => {
            "The query execution context is invalid."
        }
    }
}

#[derive(Clone)]
pub struct QueryIngress {
    authenticator: Arc<dyn RequestAuthenticator>,
    context_resolver: QueryContextResolver,
    gateway: Arc<QueryGateway>,
}

impl fmt::Debug for QueryIngress {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("QueryIngress")
            .field("authenticator", &"dyn RequestAuthenticator")
            .field("context_resolver", &self.context_resolver)
            .field("gateway", &self.gateway)
            .finish()
    }
}

impl QueryIngress {
    pub fn new(
        authenticator: Arc<dyn RequestAuthenticator>,
        context_resolver: QueryContextResolver,
        gateway: Arc<QueryGateway>,
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
        envelope: QueryCallEnvelope,
    ) -> Result<QueryExecutionReceipt, QueryIngressError> {
        let principal = self
            .authenticator
            .authenticate(authorization_value)
            .await
            .map_err(QueryIngressError::Authentication)?;
        let resolved = self
            .context_resolver
            .resolve(&principal, envelope)
            .map_err(QueryIngressError::Context)?;
        let request_id = resolved.request.context.request_id.clone();
        let correlation_id = resolved.request.context.correlation_id.clone();
        let trace_id = resolved.request.context.trace_id.clone();
        let duration = Duration::from_millis(resolved.timeout.duration_millis);
        let result = tokio::time::timeout(duration, self.gateway.execute(resolved.request))
            .await
            .map_err(|_| QueryIngressError::DeadlineExceeded)?
            .map_err(QueryIngressError::Gateway)?;
        Ok(QueryExecutionReceipt {
            request_id,
            correlation_id,
            trace_id,
            result,
        })
    }
}
