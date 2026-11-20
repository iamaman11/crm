use crate::AuthenticatedPrincipal;
use crm_capability_runtime::{ApprovalEvidence, CapabilityRequest};
use crm_module_sdk::{
    BusinessTransactionId, CapabilityId, CapabilityVersion, CausationId, Clock, CorrelationId,
    DataClass, ExecutionContext, IdempotencyKey, ModuleExecutionContext, ModuleId, PayloadEncoding,
    RandomSource, RequestId, SchemaVersion, TenantId, TraceId, TypedPayload,
};
use sha2::{Digest, Sha256};
use std::error::Error;
use std::fmt;
use std::sync::Arc;

const SEMANTIC_HASH_PROFILE: &[u8] = b"crm.capability-input/v1";
const GENERATED_ID_BYTES: usize = 16;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityRoute {
    pub owner_module_id: ModuleId,
    pub capability_id: CapabilityId,
    pub capability_version: CapabilityVersion,
    pub schema_version: SchemaVersion,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct IngressMetadata {
    pub tenant_id: Option<String>,
    pub request_id: Option<String>,
    pub correlation_id: Option<String>,
    pub causation_id: Option<String>,
    pub trace_id: Option<String>,
    pub idempotency_key: Option<String>,
    pub business_transaction_id: Option<String>,
    pub timeout_millis: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityCallEnvelope {
    pub route: CapabilityRoute,
    pub input: TypedPayload,
    pub approval: Option<ApprovalEvidence>,
    pub metadata: IngressMetadata,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimeoutPolicy {
    pub default_millis: u64,
    pub maximum_millis: u64,
}

impl TimeoutPolicy {
    pub fn validate(self) -> Result<Self, ContextResolutionError> {
        if self.default_millis == 0
            || self.maximum_millis == 0
            || self.default_millis > self.maximum_millis
        {
            return Err(ContextResolutionError::InvalidServerConfiguration);
        }
        Ok(self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimeoutBudget {
    pub duration_millis: u64,
    pub deadline_unix_nanos: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedCapabilityCall {
    pub request: CapabilityRequest,
    pub timeout: TimeoutBudget,
    pub authentication_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContextResolutionError {
    TenantRequired,
    TenantInvalid,
    TenantForbidden,
    IdempotencyKeyRequired,
    InvalidIdentifier(&'static str),
    InvalidTimeout,
    TimeoutTooLarge,
    ClockInvalid,
    IdentityGenerationUnavailable,
    InvalidServerConfiguration,
}

impl ContextResolutionError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::TenantRequired => "TENANT_REQUIRED",
            Self::TenantInvalid => "TENANT_INVALID",
            Self::TenantForbidden => "TENANT_FORBIDDEN",
            Self::IdempotencyKeyRequired => "IDEMPOTENCY_KEY_REQUIRED",
            Self::InvalidIdentifier(_) => "EXECUTION_CONTEXT_INVALID",
            Self::InvalidTimeout => "TIMEOUT_INVALID",
            Self::TimeoutTooLarge => "TIMEOUT_EXCEEDS_LIMIT",
            Self::ClockInvalid | Self::IdentityGenerationUnavailable => {
                "EXECUTION_CONTEXT_UNAVAILABLE"
            }
            Self::InvalidServerConfiguration => "EXECUTION_CONTEXT_CONFIGURATION_INVALID",
        }
    }

    pub fn retryable(&self) -> bool {
        matches!(
            self,
            Self::ClockInvalid | Self::IdentityGenerationUnavailable
        )
    }
}

impl fmt::Display for ContextResolutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::TenantRequired => "tenant identity is required",
            Self::TenantInvalid => "tenant identity is invalid",
            Self::TenantForbidden => "the authenticated actor cannot access this tenant",
            Self::IdempotencyKeyRequired => "an idempotency key is required",
            Self::InvalidIdentifier(_) => "execution context metadata is invalid",
            Self::InvalidTimeout => "timeout budget is invalid",
            Self::TimeoutTooLarge => "timeout budget exceeds the server limit",
            Self::ClockInvalid => "the request clock is invalid",
            Self::IdentityGenerationUnavailable => "request identity generation is unavailable",
            Self::InvalidServerConfiguration => "the execution-context configuration is invalid",
        })
    }
}

impl Error for ContextResolutionError {}

#[derive(Clone)]
pub struct ExecutionContextResolver {
    clock: Arc<dyn Clock>,
    random: Arc<dyn RandomSource>,
    timeout_policy: TimeoutPolicy,
}

impl fmt::Debug for ExecutionContextResolver {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ExecutionContextResolver")
            .field("clock", &"dyn Clock")
            .field("random", &"dyn RandomSource")
            .field("timeout_policy", &self.timeout_policy)
            .finish()
    }
}

impl ExecutionContextResolver {
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
        envelope: CapabilityCallEnvelope,
    ) -> Result<ResolvedCapabilityCall, ContextResolutionError> {
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
        let idempotency_key = envelope
            .metadata
            .idempotency_key
            .as_deref()
            .ok_or(ContextResolutionError::IdempotencyKeyRequired)
            .and_then(|value| parse_id(value, IdempotencyKey::try_new, "idempotency_key"))?;
        let request_id = optional_or_generated(
            envelope.metadata.request_id.as_deref(),
            || self.generate_identifier("request"),
            RequestId::try_new,
            "request_id",
        )?;
        let correlation_id = optional_or_default(
            envelope.metadata.correlation_id.as_deref(),
            request_id.as_str(),
            CorrelationId::try_new,
            "correlation_id",
        )?;
        let causation_id = optional_or_default(
            envelope.metadata.causation_id.as_deref(),
            request_id.as_str(),
            CausationId::try_new,
            "causation_id",
        )?;
        let trace_id = optional_or_generated(
            envelope.metadata.trace_id.as_deref(),
            || self.generate_identifier("trace"),
            TraceId::try_new,
            "trace_id",
        )?;
        let business_transaction_id = optional_or_generated(
            envelope.metadata.business_transaction_id.as_deref(),
            || self.generate_identifier("business-transaction"),
            BusinessTransactionId::try_new,
            "business_transaction_id",
        )?;
        let timeout = self.resolve_timeout(started_at, envelope.metadata.timeout_millis)?;
        let input_hash = semantic_input_hash(&envelope.input);
        let context = ModuleExecutionContext {
            module_id: envelope.route.owner_module_id,
            execution: ExecutionContext {
                tenant_id,
                actor_id: principal.actor_id.clone(),
                request_id,
                correlation_id,
                causation_id,
                trace_id,
                capability_id: envelope.route.capability_id,
                capability_version: envelope.route.capability_version,
                idempotency_key,
                business_transaction_id,
                schema_version: envelope.route.schema_version,
                request_started_at_unix_nanos: started_at,
            },
        };
        Ok(ResolvedCapabilityCall {
            request: CapabilityRequest {
                context,
                input: envelope.input,
                input_hash,
                approval: envelope.approval,
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
        let mut bytes = [0_u8; GENERATED_ID_BYTES];
        self.random
            .fill_bytes(&mut bytes)
            .map_err(|_| ContextResolutionError::IdentityGenerationUnavailable)?;
        let mut value = String::with_capacity(prefix.len() + 1 + GENERATED_ID_BYTES * 2);
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

pub fn semantic_input_hash(payload: &TypedPayload) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hash_field(&mut hasher, SEMANTIC_HASH_PROFILE);
    hash_field(&mut hasher, payload.owner.as_str().as_bytes());
    hash_field(&mut hasher, payload.schema_id.as_str().as_bytes());
    hash_field(&mut hasher, payload.schema_version.as_str().as_bytes());
    hash_field(&mut hasher, &payload.descriptor_hash);
    hash_field(&mut hasher, &[data_class_tag(payload.data_class)]);
    hash_field(&mut hasher, &[encoding_tag(payload.encoding)]);
    hash_field(&mut hasher, payload.retention_policy_id.as_str().as_bytes());
    hash_field(&mut hasher, &payload.maximum_size_bytes.to_be_bytes());
    hash_field(&mut hasher, &payload.bytes);
    hasher.finalize().into()
}

fn hash_field(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_be_bytes());
    hasher.update(bytes);
}

fn data_class_tag(value: DataClass) -> u8 {
    match value {
        DataClass::Public => 1,
        DataClass::Internal => 2,
        DataClass::Confidential => 3,
        DataClass::Restricted => 4,
        DataClass::Personal => 5,
        DataClass::SensitivePersonal => 6,
        DataClass::Biometric => 7,
        DataClass::Financial => 8,
        DataClass::Credential => 9,
    }
}

fn encoding_tag(value: PayloadEncoding) -> u8 {
    match value {
        PayloadEncoding::Protobuf => 1,
        PayloadEncoding::Json => 2,
        PayloadEncoding::Utf8Text => 3,
        PayloadEncoding::Binary => 4,
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
        ActorId, DataClass, ModuleId, PayloadEncoding, RetentionPolicyId, SchemaId, TenantId,
    };
    use std::collections::BTreeSet;

    #[test]
    fn resolves_tenant_actor_trace_and_timeout_into_complete_context() {
        let random = Arc::new(DeterministicRandom::from_bytes(0_u8..64));
        let resolver = ExecutionContextResolver::new(
            Arc::new(FixedClock::new(1_000)),
            random,
            TimeoutPolicy {
                default_millis: 500,
                maximum_millis: 2_000,
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
                CapabilityCallEnvelope {
                    route: route(),
                    input: payload(),
                    approval: None,
                    metadata: IngressMetadata {
                        tenant_id: Some("tenant-1".to_owned()),
                        idempotency_key: Some("idem-1".to_owned()),
                        timeout_millis: Some(750),
                        ..IngressMetadata::default()
                    },
                },
            )
            .unwrap();

        assert_eq!(
            resolved.request.context.execution.actor_id.as_str(),
            "actor-1"
        );
        assert_eq!(
            resolved.request.context.execution.tenant_id.as_str(),
            "tenant-1"
        );
        assert_eq!(resolved.timeout.duration_millis, 750);
        assert_eq!(resolved.timeout.deadline_unix_nanos, 750_001_000);
        assert_ne!(resolved.request.input_hash, [0; 32]);
        assert_eq!(resolved.authentication_id, "session-1");
    }

    #[test]
    fn rejects_cross_tenant_request_and_excessive_timeout() {
        let resolver = ExecutionContextResolver::new(
            Arc::new(FixedClock::new(1_000)),
            Arc::new(DeterministicRandom::from_bytes([0; 64])),
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
        let mut envelope = CapabilityCallEnvelope {
            route: route(),
            input: payload(),
            approval: None,
            metadata: IngressMetadata {
                tenant_id: Some("tenant-2".to_owned()),
                idempotency_key: Some("idem-1".to_owned()),
                ..IngressMetadata::default()
            },
        };
        assert_eq!(
            resolver.resolve(&principal, envelope.clone()).unwrap_err(),
            ContextResolutionError::TenantForbidden
        );

        envelope.metadata.tenant_id = Some("tenant-1".to_owned());
        envelope.metadata.timeout_millis = Some(1_001);
        assert_eq!(
            resolver.resolve(&principal, envelope).unwrap_err(),
            ContextResolutionError::TimeoutTooLarge
        );
    }

    #[test]
    fn semantic_hash_changes_when_contract_metadata_changes() {
        let original = payload();
        let mut changed = original.clone();
        changed.retention_policy_id = RetentionPolicyId::try_new("short").unwrap();
        assert_ne!(
            semantic_input_hash(&original),
            semantic_input_hash(&changed)
        );
    }

    fn route() -> CapabilityRoute {
        CapabilityRoute {
            owner_module_id: ModuleId::try_new("crm.sales").unwrap(),
            capability_id: CapabilityId::try_new("crm.sales.deal.create").unwrap(),
            capability_version: CapabilityVersion::try_new("1.0.0").unwrap(),
            schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
        }
    }

    fn payload() -> TypedPayload {
        TypedPayload {
            owner: ModuleId::try_new("crm.sales").unwrap(),
            schema_id: SchemaId::try_new("crm.sales.deal.create").unwrap(),
            schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
            descriptor_hash: [1; 32],
            data_class: DataClass::Internal,
            encoding: PayloadEncoding::Json,
            maximum_size_bytes: 4096,
            retention_policy_id: RetentionPolicyId::try_new("standard").unwrap(),
            bytes: br#"{"name":"Deal"}"#.to_vec(),
        }
    }
}
