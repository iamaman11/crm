use crm_capability_runtime::{CapabilityGateway, CapabilityRequest, gateway_error_to_sdk};
use crm_module_sdk::{
    CapabilityClient, CapabilityInvocation, CapabilityOutcome, ModuleExecutionContext, PortFuture,
    PortResult, TypedPayload,
};
use sha2::{Digest, Sha256};
use std::fmt;
use std::sync::Arc;

const SEMANTIC_HASH_PROFILE: &[u8] = b"crm.capability-input/v1";

/// In-process implementation of the Module SDK `CapabilityClient` that still enters
/// the exact same governed `CapabilityGateway` used by public mutation ingress.
///
/// The caller's tenant, actor, request lineage, idempotency identity and business
/// transaction identity are preserved. Only the target owner module, capability
/// coordinate and input schema version are rebound to the invoked published contract.
#[derive(Clone)]
pub struct GatewayCapabilityClient {
    gateway: Arc<CapabilityGateway>,
}

impl GatewayCapabilityClient {
    pub fn new(gateway: Arc<CapabilityGateway>) -> Self {
        Self { gateway }
    }
}

impl fmt::Debug for GatewayCapabilityClient {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GatewayCapabilityClient")
            .field("gateway", &"CapabilityGateway")
            .finish()
    }
}

impl CapabilityClient for GatewayCapabilityClient {
    fn invoke<'a>(
        &'a self,
        context: &'a ModuleExecutionContext,
        invocation: CapabilityInvocation,
    ) -> PortFuture<'a, PortResult<CapabilityOutcome>> {
        Box::pin(async move {
            context.validate()?;
            invocation.input.validate()?;

            let mut execution = context.execution.clone();
            execution.capability_id = invocation.capability_id;
            execution.capability_version = invocation.capability_version;
            execution.schema_version = invocation.input.schema_version.clone();
            let target_context = ModuleExecutionContext {
                module_id: invocation.input.owner.clone(),
                execution,
            };
            let input_hash = semantic_input_hash(&invocation.input);
            let result = self
                .gateway
                .execute(CapabilityRequest {
                    context: target_context,
                    input: invocation.input,
                    input_hash,
                    approval: None,
                })
                .await
                .map_err(gateway_error_to_sdk)?;

            Ok(CapabilityOutcome {
                output: result.output,
                affected_resources: result.affected_resources,
            })
        })
    }
}

fn semantic_input_hash(payload: &TypedPayload) -> [u8; 32] {
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

const fn data_class_tag(value: crm_module_sdk::DataClass) -> u8 {
    match value {
        crm_module_sdk::DataClass::Public => 1,
        crm_module_sdk::DataClass::Internal => 2,
        crm_module_sdk::DataClass::Confidential => 3,
        crm_module_sdk::DataClass::Restricted => 4,
        crm_module_sdk::DataClass::Personal => 5,
        crm_module_sdk::DataClass::SensitivePersonal => 6,
        crm_module_sdk::DataClass::Biometric => 7,
        crm_module_sdk::DataClass::Financial => 8,
        crm_module_sdk::DataClass::Credential => 9,
    }
}

const fn encoding_tag(value: crm_module_sdk::PayloadEncoding) -> u8 {
    match value {
        crm_module_sdk::PayloadEncoding::Protobuf => 1,
        crm_module_sdk::PayloadEncoding::Json => 2,
        crm_module_sdk::PayloadEncoding::Utf8Text => 3,
        crm_module_sdk::PayloadEncoding::Binary => 4,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crm_capability_runtime::testing::{
        FixedApprovalVerifier, FixedAuthorizer, FixedClock, FixedRateLimiter,
        FixedSemanticValidator, RecordingExecutor, StaticCapabilityRegistry, call_log,
    };
    use crm_capability_runtime::{
        AuthorizationDecision, CapabilityDefinition, CapabilityExecutionResult, CapabilityRisk,
        PayloadContract, RateLimitDecision,
    };
    use crm_module_sdk::{
        ActorId, BusinessTransactionId, CapabilityId, CapabilityVersion, CausationId,
        CorrelationId, DataClass, ExecutionContext, IdempotencyKey, ModuleId, PayloadEncoding,
        RequestId, RetentionPolicyId, SchemaId, SchemaVersion, TenantId, TraceId, TypedPayload,
    };

    #[test]
    fn module_client_rebinds_only_the_target_coordinate_and_enters_the_gateway() {
        let log = call_log();
        let definition = definition();
        let gateway = Arc::new(CapabilityGateway::new(
            Arc::new(StaticCapabilityRegistry {
                definition: Some(definition.clone()),
                error: None,
                calls: log.clone(),
            }),
            Arc::new(FixedSemanticValidator {
                error: None,
                calls: log.clone(),
            }),
            Arc::new(FixedRateLimiter {
                decision: RateLimitDecision {
                    allowed: true,
                    decision_id: "rate-1".to_owned(),
                    retry_after_millis: None,
                },
                error: None,
                calls: log.clone(),
            }),
            Arc::new(FixedApprovalVerifier {
                error: None,
                calls: log.clone(),
            }),
            Arc::new(FixedAuthorizer {
                decision: AuthorizationDecision {
                    allowed: true,
                    decision_id: "auth-1".to_owned(),
                    reason_code: "allowed".to_owned(),
                    policy_version: "1".to_owned(),
                },
                error: None,
                calls: log.clone(),
            }),
            Arc::new(RecordingExecutor {
                result: Ok(CapabilityExecutionResult {
                    output: None,
                    affected_resources: Vec::new(),
                    replayed: false,
                }),
                calls: log.clone(),
            }),
            Arc::new(FixedClock { now_unix_nanos: 10 }),
        ));
        let client = GatewayCapabilityClient::new(gateway);
        let invocation = CapabilityInvocation {
            capability_id: definition.capability_id.clone(),
            capability_version: definition.capability_version.clone(),
            input: input(),
        };

        let outcome = run_ready(client.invoke(&link_context(), invocation)).unwrap();

        assert!(outcome.affected_resources.is_empty());
        assert_eq!(
            log.lock().unwrap().as_slice(),
            &["registry", "validate", "authorize", "execute"]
        );
    }

    fn definition() -> CapabilityDefinition {
        CapabilityDefinition {
            capability_id: CapabilityId::try_new("activities.task.create").unwrap(),
            capability_version: CapabilityVersion::try_new("1.0.0").unwrap(),
            owner_module_id: ModuleId::try_new("crm.activities").unwrap(),
            input_contract: PayloadContract {
                owner: ModuleId::try_new("crm.activities").unwrap(),
                schema_id: SchemaId::try_new("crm.activities.v1.CreateTaskRequest").unwrap(),
                schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
                descriptor_hash: [7; 32],
                allowed_data_classes: vec![DataClass::Confidential],
                allowed_encodings: vec![PayloadEncoding::Protobuf],
                maximum_size_bytes: 1024,
            },
            output_contract: None,
            risk: CapabilityRisk::Low,
            mutation: true,
            requires_idempotency: true,
            requires_approval: false,
            authorization_policy_id: "activities.task.create".to_owned(),
            rate_limit_policy_id: None,
        }
    }

    fn input() -> TypedPayload {
        TypedPayload {
            owner: ModuleId::try_new("crm.activities").unwrap(),
            schema_id: SchemaId::try_new("crm.activities.v1.CreateTaskRequest").unwrap(),
            schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
            descriptor_hash: [7; 32],
            data_class: DataClass::Confidential,
            encoding: PayloadEncoding::Protobuf,
            maximum_size_bytes: 1024,
            retention_policy_id: RetentionPolicyId::try_new("standard").unwrap(),
            bytes: vec![1],
        }
    }

    fn link_context() -> ModuleExecutionContext {
        ModuleExecutionContext {
            module_id: ModuleId::try_new("crm.sales-activities-link").unwrap(),
            execution: ExecutionContext {
                tenant_id: TenantId::try_new("tenant-a").unwrap(),
                actor_id: ActorId::try_new("link-service").unwrap(),
                request_id: RequestId::try_new("request-1").unwrap(),
                correlation_id: CorrelationId::try_new("correlation-1").unwrap(),
                causation_id: CausationId::try_new("event-1").unwrap(),
                trace_id: TraceId::try_new("trace-1").unwrap(),
                capability_id: CapabilityId::try_new("link.process").unwrap(),
                capability_version: CapabilityVersion::try_new("1.0.0").unwrap(),
                idempotency_key: IdempotencyKey::try_new("delivery-1").unwrap(),
                business_transaction_id: BusinessTransactionId::try_new("transaction-1").unwrap(),
                schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
                request_started_at_unix_nanos: 1,
            },
        }
    }

    fn run_ready<F: std::future::Future>(future: F) -> F::Output {
        use std::task::{Context, Poll, Waker};
        let mut context = Context::from_waker(Waker::noop());
        let mut future = Box::pin(future);
        match future.as_mut().poll(&mut context) {
            Poll::Ready(output) => output,
            Poll::Pending => panic!("test future unexpectedly returned Pending"),
        }
    }
}
