use crate::{
    AuthenticatedPrincipal, CapabilityRoute, ContextResolutionError, TimeoutBudget, TimeoutPolicy,
    semantic_input_hash,
};
use crm_module_sdk::{
    CorrelationId, Clock, RandomSource, RequestId, TenantId, TraceId, TypedPayload,
};
use crm_query_runtime::{QueryExecutionContext, QueryRequest};
use std::fmt;
use std::sync::Arc;

const GENERATED_QUERY_ID_BYTES: usize = 16;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct QueryIngressMetadata {
    pub tenant_id: Option<String>,
    pub request_id: Option<String>,
    pub correlation_id: Option<String>,
    pub trace_id: Option<String>,
    pub timeout_millis: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryCallEnvelope {
    pub route: CapabilityRoute,
    pub input: TypedPayload,
    pub metadata: QueryIngressMetadata,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedQueryCall {
    pub request: QueryRequest,
    pub timeout: TimeoutBudget,
    pub authentication_id: String,
}

#[derive(Clone)]
pub struct QueryContextResolver {
    clock: Arc<dyn Clock>,
    random: Arc<dyn RandomSource>,
    timeout_policy: TimeoutPolicy,
}

impl fmt::Debug for QueryContextResolver {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("QueryContextResolver")
            .field("clock", &"dyn Clock")
            .field("random", &"dyn RandomSource")
            .field("timeout_policy", &self.timeout_policy)
            .finish()
    }
}

impl QueryContextResolver {
    pub fn new(
        clock: Arc<dyn Clock>,
        random: Arc<dyn RandomSource>,
        timeout_policy: TimeoutPolicy,
    ) -> Result<Self, ContextResolutionError> {
        Ok(Self {
            clock,
            random,
            timeout_policy: timeout_policy.validate()?,
        })
    }

    pub fn resolve(
        &self,
        principal: &AuthenticatedPrincipal,
        envelope: QueryCallEnvelope,
    ) -> Result<ResolvedQueryCall, ContextResolutionError> {
        let started_at = self.clock.now_unix_nanos();
        if started_at < 0 {
            return Err(ContextResolutionError::ClockInvalid);
        }
        let tenant_value = envelope
            .metadata
            .tenant_id
            .as_deref()
            .ok_or(ContextResolutionError::TenantRequired)?;
        let tenant_id =
            TenantId::try_new(tenant_value).map_err(|_| ContextResolutionError::TenantInvalid)?;
        if !principal.permits_tenant(&tenant_id) {
            return Err(ContextResolutionError::TenantForbidden);
        }

        let request_id = optional_or_generated(
            envelope.metadata.request_id.as_deref(),
            || self.generate_identifier("query-request"),
            RequestId::try_new,
            "request_id",
        )?;
        let correlation_id = optional_or_default(
            envelope.metadata.correlation_id.as_deref(),
            request_id.as_str(),
            CorrelationId::try_new,
            "correlation_id",
        )?;
        let trace_id = optional_or_generated(
            envelope.metadata.trace_id.as_deref(),
            || self.generate_identifier("query-trace"),
            TraceId::try_new,
            "trace_id",
        )?;
        let timeout = self.resolve_timeout(started_at, envelope.metadata.timeout_millis)?;
        let input_hash = semantic_input_hash(&envelope.input);

        Ok(ResolvedQueryCall {
            request: QueryRequest {
                owner_module_id: envelope.route.owner_module_id,
                context: QueryExecutionContext {
                    tenant_id,
                    actor_id: principal.actor_id.clone(),
                    request_id,
                    correlation_id,
                    trace_id,
                    capability_id: envelope.route.capability_id,
                    capability_version: envelope.route.capability_version,
                    schema_version: envelope.route.schema_version,
                    request_started_at_unix_nanos: started_at,
                },
                input: envelope.input,
                input_hash,
            },
            timeout,
            authentication_id: principal.authentication_id.clone(),
        })
    }

    fn resolve_timeout(
        &self,
        started_at: i64,
        requested_millis: Option<u64>,
    ) -> Result<TimeoutBudget, ContextResolutionError> {
        let duration_millis = requested_millis.unwrap_or(self.timeout_policy.default_millis);
        if duration_millis == 0 {
            return Err(ContextResolutionError::InvalidTimeout);
        }
        if duration_millis > self.timeout_policy.maximum_millis {
            return Err(ContextResolutionError::TimeoutTooLarge);
        }
        let nanos = duration_millis
            .checked_mul(1_000_000)
            .and_then(|value| i64::try_from(value).ok())
            .ok_or(ContextResolutionError::InvalidTimeout)?;
        let deadline_unix_nanos = started_at
            .checked_add(nanos)
            .ok_or(ContextResolutionError::InvalidTimeout)?;
        Ok(TimeoutBudget {
            duration_millis,
            deadline_unix_nanos,
        })
    }

    fn generate_identifier(&self, prefix: &str) -> Result<String, ContextResolutionError> {
        let mut bytes = [0_u8; GENERATED_QUERY_ID_BYTES];
        self.random
            .fill_bytes(&mut bytes)
            .map_err(|_| ContextResolutionError::IdentityGenerationUnavailable)?;
        let mut value = String::with_capacity(prefix.len() + 1 + GENERATED_QUERY_ID_BYTES * 2);
        value.push_str(prefix);
        value.push('-');
        for byte in bytes {
            use std::fmt::Write as _;
            write!(&mut value, "{byte:02x}")
                .map_err(|_| ContextResolutionError::IdentityGenerationUnavailable)?;
        }
        Ok(value)
    }
}

fn parse_id<T, E>(
    value: &str,
    parser: impl FnOnce(String) -> Result<T, E>,
    field: &'static str,
) -> Result<T, ContextResolutionError> {
    parser(value.to_owned()).map_err(|_| ContextResolutionError::InvalidIdentifier(field))
}

fn optional_or_default<T, E>(
    value: Option<&str>,
    default: &str,
    parser: impl FnOnce(String) -> Result<T, E>,
    field: &'static str,
) -> Result<T, ContextResolutionError> {
    parse_id(value.unwrap_or(default), parser, field)
}

fn optional_or_generated<T, E>(
    value: Option<&str>,
    generator: impl FnOnce() -> Result<String, ContextResolutionError>,
    parser: impl FnOnce(String) -> Result<T, E>,
    field: &'static str,
) -> Result<T, ContextResolutionError> {
    let value = match value {
        Some(value) => value.to_owned(),
        None => generator()?,
    };
    parser(value).map_err(|_| ContextResolutionError::InvalidIdentifier(field))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crm_module_sdk::testing::{DeterministicRandom, FixedClock};
    use crm_module_sdk::{
        ActorId, CapabilityId, CapabilityVersion, DataClass, ModuleId, PayloadEncoding,
        RetentionPolicyId, SchemaId, SchemaVersion,
    };
    use std::collections::BTreeSet;

    #[test]
    fn resolves_query_without_mutation_only_identity_fields() {
        let clock: Arc<dyn Clock> = Arc::new(FixedClock::new(100));
        let resolver = QueryContextResolver::new(
            clock,
            Arc::new(DeterministicRandom::from_bytes(0_u8..64)),
            TimeoutPolicy {
                default_millis: 500,
                maximum_millis: 1_000,
            },
        )
        .unwrap();
        let principal = AuthenticatedPrincipal {
            actor_id: ActorId::try_new("actor-1").unwrap(),
            tenant_ids: BTreeSet::from([TenantId::try_new("tenant-1").unwrap()]),
            authentication_id: "session-1".to_owned(),
        };
        let resolved = resolver
            .resolve(
                &principal,
                QueryCallEnvelope {
                    route: CapabilityRoute {
                        owner_module_id: ModuleId::try_new("crm.sales").unwrap(),
                        capability_id: CapabilityId::try_new("sales.deal.get").unwrap(),
                        capability_version: CapabilityVersion::try_new("1.0.0").unwrap(),
                        schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
                    },
                    input: TypedPayload {
                        owner: ModuleId::try_new("crm.sales").unwrap(),
                        schema_id: SchemaId::try_new("crm.sales.v1.GetDealRequest").unwrap(),
                        schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
                        descriptor_hash: [1; 32],
                        data_class: DataClass::Confidential,
                        encoding: PayloadEncoding::Protobuf,
                        maximum_size_bytes: 1_024,
                        retention_policy_id: RetentionPolicyId::try_new("standard").unwrap(),
                        bytes: vec![1],
                    },
                    metadata: QueryIngressMetadata {
                        tenant_id: Some("tenant-1".to_owned()),
                        ..QueryIngressMetadata::default()
                    },
                },
            )
            .unwrap();
        assert_eq!(resolved.request.context.tenant_id.as_str(), "tenant-1");
        assert_eq!(resolved.request.context.actor_id.as_str(), "actor-1");
        assert_eq!(resolved.request.context.capability_id.as_str(), "sales.deal.get");
        assert_eq!(resolved.timeout.duration_millis, 500);
    }
}
