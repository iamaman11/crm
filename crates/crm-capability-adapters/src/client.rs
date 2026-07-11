use crm_capability_runtime::{CapabilityGateway, CapabilityRequest, gateway_error_to_sdk};
use crm_module_sdk::{
    CapabilityClient, CapabilityInvocation, CapabilityOutcome, ExecutionContext,
    ModuleExecutionContext, PortFuture, PortResult, TypedPayload,
};
use sha2::{Digest, Sha256};
use std::fmt;
use std::sync::Arc;

const SEMANTIC_HASH_PROFILE: &[u8] = b"crm.capability-input/v1";

/// Production `CapabilityClient` adapter for module-to-module capability calls.
///
/// The caller supplies its immutable execution lineage. The adapter preserves
/// tenant, actor, request, correlation, causation, trace, idempotency and
/// business-transaction identities while rebinding the execution coordinate to
/// the target capability and its owning module. All target side effects still
/// pass through the ordinary `CapabilityGateway` authorization and transaction
/// boundary.
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
        caller_context: &'a ModuleExecutionContext,
        invocation: CapabilityInvocation,
    ) -> PortFuture<'a, PortResult<CapabilityOutcome>> {
        Box::pin(async move {
            caller_context.validate()?;
            invocation.input.validate()?;

            let CapabilityInvocation {
                capability_id,
                capability_version,
                input,
            } = invocation;
            let target_context = ModuleExecutionContext {
                module_id: input.owner.clone(),
                execution: ExecutionContext {
                    tenant_id: caller_context.execution.tenant_id.clone(),
                    actor_id: caller_context.execution.actor_id.clone(),
                    request_id: caller_context.execution.request_id.clone(),
                    correlation_id: caller_context.execution.correlation_id.clone(),
                    causation_id: caller_context.execution.causation_id.clone(),
                    trace_id: caller_context.execution.trace_id.clone(),
                    capability_id,
                    capability_version,
                    idempotency_key: caller_context.execution.idempotency_key.clone(),
                    business_transaction_id: caller_context
                        .execution
                        .business_transaction_id
                        .clone(),
                    schema_version: input.schema_version.clone(),
                    request_started_at_unix_nanos: caller_context
                        .execution
                        .request_started_at_unix_nanos,
                },
            };
            let input_hash = semantic_input_hash(&input);
            let result = self
                .gateway
                .execute(CapabilityRequest {
                    context: target_context,
                    input,
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

fn data_class_tag(value: crm_module_sdk::DataClass) -> u8 {
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

fn encoding_tag(value: crm_module_sdk::PayloadEncoding) -> u8 {
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
        FixedSemanticValidator, StaticCapabilityRegistry, call_log,
    };
    use crm_capability_runtime::{
        AuthorizationDecision, CapabilityDefinition, CapabilityExecutionResult, CapabilityRisk,
        PayloadContract, RateLimitDecision, TransactionalCapabilityExecutor,
    };
    use crm_module_sdk::{
        ActorId, BusinessTransactionId, CapabilityId, CapabilityVersion, CausationId,
        CorrelationId, DataClass, ErrorCategory, IdempotencyKey, ModuleId, PayloadEncoding,
        RequestId, ResourceRef, RetentionPolicyId, SchemaId, SchemaVersion, SdkError, TenantId,
        TraceId,
    };
    use std::sync::Mutex;

    #[derive(Clone)]
    struct CapturingExecutor {
        request: Arc<Mutex<Option<CapabilityRequest>>>,
        result: CapabilityExecutionResult,
    }

    impl TransactionalCapabilityExecutor for CapturingExecutor {
        fn execute<'a>(
            &'a self,
            _definition: &'a CapabilityDefinition,
            request: CapabilityRequest,
        ) -> PortFuture<'a, Result<CapabilityExecutionResult, SdkError>> {
            *self.request.lock().expect("request mutex poisoned") = Some(request);
            let result = self.result.clone();
            Box::pin(async move { Ok(result) })
        }
    }

    #[tokio::test]
    async fn rebinds_module_and_capability_while_preserving_delivery_lineage() {
        let definition = target_definition();
        let captured = Arc::new(Mutex::new(None));
        let gateway = gateway(
            definition.clone(),
            AuthorizationDecision {
                allowed: true,
                decision_id: "allow-1".to_owned(),
                reason_code: "allowed".to_owned(),
                policy_version: "1".to_owned(),
            },
            captured.clone(),
        );
        let client = GatewayCapabilityClient::new(Arc::new(gateway));
        let caller = caller_context();
        let input = target_input();

        let outcome = client
            .invoke(
                &caller,
                CapabilityInvocation {
                    capability_id: definition.capability_id.clone(),
                    capability_version: definition.capability_version.clone(),
                    input: input.clone(),
                },
            )
            .await
            .expect("governed target call must succeed");

        assert_eq!(outcome.affected_resources.len(), 1);
        let request = captured
            .lock()
            .expect("request mutex poisoned")
            .clone()
            .expect("gateway executor must receive the request");
        assert_eq!(request.context.module_id.as_str(), "crm.activities");
        assert_eq!(
            request.context.execution.capability_id.as_str(),
            "activities.task.create"
        );
        assert_eq!(
            request.context.execution.tenant_id,
            caller.execution.tenant_id
        );
        assert_eq!(
            request.context.execution.actor_id,
            caller.execution.actor_id
        );
        assert_eq!(
            request.context.execution.correlation_id,
            caller.execution.correlation_id
        );
        assert_eq!(
            request.context.execution.causation_id,
            caller.execution.causation_id
        );
        assert_eq!(
            request.context.execution.trace_id,
            caller.execution.trace_id
        );
        assert_eq!(
            request.context.execution.idempotency_key,
            caller.execution.idempotency_key
        );
        assert_eq!(
            request.context.execution.business_transaction_id,
            caller.execution.business_transaction_id
        );
        assert_eq!(request.input, input);
        assert_ne!(request.input_hash, [0; 32]);
    }

    #[tokio::test]
    async fn maps_live_target_authorization_denial_to_sdk_error() {
        let definition = target_definition();
        let captured = Arc::new(Mutex::new(None));
        let gateway = gateway(
            definition.clone(),
            AuthorizationDecision {
                allowed: false,
                decision_id: "deny-1".to_owned(),
                reason_code: "revoked".to_owned(),
                policy_version: "2".to_owned(),
            },
            captured.clone(),
        );
        let client = GatewayCapabilityClient::new(Arc::new(gateway));

        let error = client
            .invoke(
                &caller_context(),
                CapabilityInvocation {
                    capability_id: definition.capability_id,
                    capability_version: definition.capability_version,
                    input: target_input(),
                },
            )
            .await
            .expect_err("live target denial must fail the module call");

        assert_eq!(error.code, "CAPABILITY_PERMISSION_DENIED");
        assert_eq!(error.category, ErrorCategory::Authorization);
        assert!(
            captured.lock().expect("request mutex poisoned").is_none(),
            "denied calls must never reach the transactional executor"
        );
    }

    fn gateway(
        definition: CapabilityDefinition,
        authorization: AuthorizationDecision,
        captured: Arc<Mutex<Option<CapabilityRequest>>>,
    ) -> CapabilityGateway {
        let calls = call_log();
        CapabilityGateway::new(
            Arc::new(StaticCapabilityRegistry {
                definition: Some(definition),
                error: None,
                calls: calls.clone(),
            }),
            Arc::new(FixedSemanticValidator {
                error: None,
                calls: calls.clone(),
            }),
            Arc::new(FixedRateLimiter {
                decision: RateLimitDecision {
                    allowed: true,
                    decision_id: "rate-1".to_owned(),
                    retry_after_millis: None,
                },
                error: None,
                calls: calls.clone(),
            }),
            Arc::new(FixedApprovalVerifier {
                error: None,
                calls: calls.clone(),
            }),
            Arc::new(FixedAuthorizer {
                decision: authorization,
                error: None,
                calls,
            }),
            Arc::new(CapturingExecutor {
                request: captured,
                result: CapabilityExecutionResult {
                    output: None,
                    affected_resources: vec![ResourceRef {
                        resource_type: "activities.task".to_owned(),
                        resource_id: "task-1".to_owned(),
                        version: Some(1),
                    }],
                    replayed: false,
                },
            }),
            Arc::new(FixedClock {
                now_unix_nanos: 100,
            }),
        )
    }

    fn target_definition() -> CapabilityDefinition {
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
                maximum_size_bytes: 4_096,
            },
            output_contract: None,
            risk: CapabilityRisk::Medium,
            mutation: true,
            requires_idempotency: true,
            requires_approval: false,
            authorization_policy_id: "activities.task.create".to_owned(),
            rate_limit_policy_id: None,
        }
    }

    fn target_input() -> TypedPayload {
        TypedPayload {
            owner: ModuleId::try_new("crm.activities").unwrap(),
            schema_id: SchemaId::try_new("crm.activities.v1.CreateTaskRequest").unwrap(),
            schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
            descriptor_hash: [7; 32],
            data_class: DataClass::Confidential,
            encoding: PayloadEncoding::Protobuf,
            maximum_size_bytes: 4_096,
            retention_policy_id: RetentionPolicyId::try_new("standard").unwrap(),
            bytes: vec![1, 2, 3],
        }
    }

    fn caller_context() -> ModuleExecutionContext {
        ModuleExecutionContext {
            module_id: ModuleId::try_new("crm.sales-activities-link").unwrap(),
            execution: ExecutionContext {
                tenant_id: TenantId::try_new("tenant-a").unwrap(),
                actor_id: ActorId::try_new("link-worker").unwrap(),
                request_id: RequestId::try_new("request-1").unwrap(),
                correlation_id: CorrelationId::try_new("correlation-1").unwrap(),
                causation_id: CausationId::try_new("event-1").unwrap(),
                trace_id: TraceId::try_new("trace-1").unwrap(),
                capability_id: CapabilityId::try_new("link.sales.stage_changed.process").unwrap(),
                capability_version: CapabilityVersion::try_new("1.0.0").unwrap(),
                idempotency_key: IdempotencyKey::try_new("delivery-1").unwrap(),
                business_transaction_id: BusinessTransactionId::try_new("delivery-tx-1").unwrap(),
                schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
                request_started_at_unix_nanos: 99,
            },
        }
    }
}
