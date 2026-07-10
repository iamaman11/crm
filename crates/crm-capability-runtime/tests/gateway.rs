use crm_capability_runtime::testing::{
    CallLog, FixedApprovalVerifier, FixedAuthorizer, FixedClock, FixedRateLimiter,
    FixedSemanticValidator, RecordingExecutor, StaticCapabilityRegistry, call_log,
};
use crm_capability_runtime::{
    ApprovalEvidence, AuthorizationDecision, CapabilityDefinition, CapabilityExecutionResult,
    CapabilityGateway, CapabilityRequest, CapabilityRisk, GatewayError, PayloadContract,
    RateLimitDecision,
};
use crm_module_sdk::{
    ActorId, BusinessTransactionId, CapabilityId, CapabilityVersion, CausationId, CorrelationId,
    DataClass, ExecutionContext, IdempotencyKey, ModuleExecutionContext, ModuleId, PayloadEncoding,
    RequestId, RetentionPolicyId, SchemaId, SchemaVersion, TenantId, TraceId, TypedPayload,
};
use std::future::Future;
use std::sync::Arc;
use std::task::{Context, Poll, Waker};

fn block_on<F: Future>(future: F) -> F::Output {
    let mut future = Box::pin(future);
    let waker = Waker::noop();
    let mut context = Context::from_waker(waker);
    loop {
        match future.as_mut().poll(&mut context) {
            Poll::Ready(value) => return value,
            Poll::Pending => std::thread::yield_now(),
        }
    }
}

fn payload_contract() -> PayloadContract {
    PayloadContract {
        owner: ModuleId::try_new("crm.sales").unwrap(),
        schema_id: SchemaId::try_new("sales.deal.command.v1").unwrap(),
        schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
        descriptor_hash: [7; 32],
        allowed_data_classes: vec![DataClass::Internal],
        allowed_encodings: vec![PayloadEncoding::Protobuf],
        maximum_size_bytes: 1024,
    }
}

fn definition() -> CapabilityDefinition {
    CapabilityDefinition {
        capability_id: CapabilityId::try_new("sales.deal.create").unwrap(),
        capability_version: CapabilityVersion::try_new("1.0.0").unwrap(),
        owner_module_id: ModuleId::try_new("crm.sales").unwrap(),
        input_contract: payload_contract(),
        output_contract: Some(payload_contract()),
        risk: CapabilityRisk::High,
        mutation: true,
        requires_idempotency: true,
        requires_approval: true,
        authorization_policy_id: "sales.deal.create.policy@1".to_owned(),
        rate_limit_policy_id: Some("sales.write.standard@1".to_owned()),
    }
}

fn payload() -> TypedPayload {
    TypedPayload {
        owner: ModuleId::try_new("crm.sales").unwrap(),
        schema_id: SchemaId::try_new("sales.deal.command.v1").unwrap(),
        schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
        descriptor_hash: [7; 32],
        data_class: DataClass::Internal,
        encoding: PayloadEncoding::Protobuf,
        maximum_size_bytes: 1024,
        retention_policy_id: RetentionPolicyId::try_new("standard").unwrap(),
        bytes: vec![1, 2, 3],
    }
}

fn request() -> CapabilityRequest {
    CapabilityRequest {
        context: ModuleExecutionContext {
            module_id: ModuleId::try_new("crm.sales").unwrap(),
            execution: ExecutionContext {
                tenant_id: TenantId::try_new("tenant-a").unwrap(),
                actor_id: ActorId::try_new("actor-a").unwrap(),
                request_id: RequestId::try_new("request-a").unwrap(),
                correlation_id: CorrelationId::try_new("correlation-a").unwrap(),
                causation_id: CausationId::try_new("causation-a").unwrap(),
                trace_id: TraceId::try_new("trace-a").unwrap(),
                capability_id: CapabilityId::try_new("sales.deal.create").unwrap(),
                capability_version: CapabilityVersion::try_new("1.0.0").unwrap(),
                idempotency_key: IdempotencyKey::try_new("idem-a").unwrap(),
                business_transaction_id: BusinessTransactionId::try_new("tx-a").unwrap(),
                schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
                request_started_at_unix_nanos: 100,
            },
        },
        input: payload(),
        input_hash: [9; 32],
        approval: Some(ApprovalEvidence {
            approval_id: "approval-a".to_owned(),
            actor_id: ActorId::try_new("actor-a").unwrap(),
            capability_id: CapabilityId::try_new("sales.deal.create").unwrap(),
            capability_version: CapabilityVersion::try_new("1.0.0").unwrap(),
            input_hash: [9; 32],
            policy_version: "approval-policy@1".to_owned(),
            expires_at_unix_nanos: 1_000,
            opaque_proof: vec![5, 6, 7],
        }),
    }
}

fn outcome(replayed: bool) -> CapabilityExecutionResult {
    CapabilityExecutionResult {
        output: Some(payload()),
        affected_resources: Vec::new(),
        replayed,
    }
}

fn gateway(
    calls: &CallLog,
    definition: Option<CapabilityDefinition>,
    rate_allowed: bool,
    authorization_allowed: bool,
    replayed: bool,
) -> CapabilityGateway {
    CapabilityGateway::new(
        Arc::new(StaticCapabilityRegistry {
            definition,
            error: None,
            calls: Arc::clone(calls),
        }),
        Arc::new(FixedSemanticValidator {
            error: None,
            calls: Arc::clone(calls),
        }),
        Arc::new(FixedRateLimiter {
            decision: RateLimitDecision {
                allowed: rate_allowed,
                decision_id: "rate-decision-a".to_owned(),
                retry_after_millis: (!rate_allowed).then_some(500),
            },
            error: None,
            calls: Arc::clone(calls),
        }),
        Arc::new(FixedApprovalVerifier {
            error: None,
            calls: Arc::clone(calls),
        }),
        Arc::new(FixedAuthorizer {
            decision: AuthorizationDecision {
                allowed: authorization_allowed,
                decision_id: "authorization-decision-a".to_owned(),
                reason_code: if authorization_allowed {
                    "allowed".to_owned()
                } else {
                    "policy_denied".to_owned()
                },
                policy_version: "sales.deal.create.policy@1".to_owned(),
            },
            error: None,
            calls: Arc::clone(calls),
        }),
        Arc::new(RecordingExecutor {
            result: Ok(outcome(replayed)),
            calls: Arc::clone(calls),
        }),
        Arc::new(FixedClock {
            now_unix_nanos: 200,
        }),
    )
}

fn calls(log: &CallLog) -> Vec<&'static str> {
    log.lock().expect("call log mutex poisoned").clone()
}

#[test]
fn live_authorization_is_the_final_stage_before_execution() {
    let log = call_log();
    let result = block_on(gateway(&log, Some(definition()), true, true, true).execute(request()))
        .expect("valid capability must execute");

    assert!(result.replayed);
    assert_eq!(
        calls(&log),
        vec![
            "registry",
            "validate",
            "rate",
            "approval",
            "authorize",
            "execute"
        ]
    );
}

#[test]
fn unknown_capability_never_reaches_validation_or_execution() {
    let log = call_log();
    let error = block_on(gateway(&log, None, true, true, false).execute(request())).unwrap_err();

    assert!(matches!(error, GatewayError::CapabilityNotFound));
    assert_eq!(calls(&log), vec!["registry"]);
}

#[test]
fn input_contract_mismatch_stops_before_semantic_validation() {
    let log = call_log();
    let mut value = request();
    value.input.descriptor_hash = [8; 32];
    let error = block_on(gateway(&log, Some(definition()), true, true, false).execute(value))
        .unwrap_err();

    assert!(matches!(error, GatewayError::InputContractMismatch));
    assert_eq!(calls(&log), vec!["registry"]);
}

#[test]
fn rate_limit_stops_before_approval_authorization_and_execution() {
    let log = call_log();
    let error = block_on(gateway(&log, Some(definition()), false, true, false).execute(request()))
        .unwrap_err();

    assert!(matches!(error, GatewayError::RateLimited { .. }));
    assert_eq!(calls(&log), vec!["registry", "validate", "rate"]);
}

#[test]
fn approval_binding_is_checked_before_proof_verification() {
    let log = call_log();
    let mut value = request();
    value.approval.as_mut().unwrap().input_hash = [4; 32];
    let error = block_on(gateway(&log, Some(definition()), true, true, false).execute(value))
        .unwrap_err();

    assert!(matches!(error, GatewayError::ApprovalBindingMismatch));
    assert_eq!(calls(&log), vec!["registry", "validate", "rate"]);
}

#[test]
fn authorization_denial_never_calls_the_transactional_executor() {
    let log = call_log();
    let error = block_on(gateway(&log, Some(definition()), true, false, false).execute(request()))
        .unwrap_err();

    assert!(matches!(error, GatewayError::PermissionDenied { .. }));
    assert_eq!(
        calls(&log),
        vec!["registry", "validate", "rate", "approval", "authorize"]
    );
}
