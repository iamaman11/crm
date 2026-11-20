use crate::ports::{
    CapabilityApprovalVerifier, CapabilityAuthorizer, CapabilityRateLimiter,
    CapabilityRegistryPort, CapabilitySemanticValidator, TransactionalCapabilityExecutor,
};
use crate::types::{
    ApprovalEvidence, AuthorizationDecision, CapabilityDefinition, CapabilityExecutionResult,
    CapabilityRequest, RateLimitDecision,
};
use crm_module_sdk::{CapabilityId, CapabilityVersion, Clock, PortFuture, SdkError};
use std::sync::{Arc, Mutex};

pub type CallLog = Arc<Mutex<Vec<&'static str>>>;

pub fn call_log() -> CallLog {
    Arc::new(Mutex::new(Vec::new()))
}

fn record(log: &CallLog, call: &'static str) {
    log.lock().expect("call log mutex poisoned").push(call);
}

#[derive(Clone)]
pub struct StaticCapabilityRegistry {
    pub definition: Option<CapabilityDefinition>,
    pub error: Option<SdkError>,
    pub calls: CallLog,
}

impl CapabilityRegistryPort for StaticCapabilityRegistry {
    fn resolve<'a>(
        &'a self,
        _capability_id: &'a CapabilityId,
        _capability_version: &'a CapabilityVersion,
    ) -> PortFuture<'a, Result<Option<CapabilityDefinition>, SdkError>> {
        record(&self.calls, "registry");
        let result = self
            .error
            .clone()
            .map_or_else(|| Ok(self.definition.clone()), Err);
        Box::pin(async move { result })
    }
}

#[derive(Clone)]
pub struct FixedSemanticValidator {
    pub error: Option<SdkError>,
    pub calls: CallLog,
}

impl CapabilitySemanticValidator for FixedSemanticValidator {
    fn validate<'a>(
        &'a self,
        _definition: &'a CapabilityDefinition,
        _request: &'a CapabilityRequest,
    ) -> PortFuture<'a, Result<(), SdkError>> {
        record(&self.calls, "validate");
        let result = self.error.clone().map_or(Ok(()), Err);
        Box::pin(async move { result })
    }
}

#[derive(Clone)]
pub struct FixedRateLimiter {
    pub decision: RateLimitDecision,
    pub error: Option<SdkError>,
    pub calls: CallLog,
}

impl CapabilityRateLimiter for FixedRateLimiter {
    fn check<'a>(
        &'a self,
        _definition: &'a CapabilityDefinition,
        _request: &'a CapabilityRequest,
    ) -> PortFuture<'a, Result<RateLimitDecision, SdkError>> {
        record(&self.calls, "rate");
        let result = self
            .error
            .clone()
            .map_or_else(|| Ok(self.decision.clone()), Err);
        Box::pin(async move { result })
    }
}

#[derive(Clone)]
pub struct FixedApprovalVerifier {
    pub error: Option<SdkError>,
    pub calls: CallLog,
}

impl CapabilityApprovalVerifier for FixedApprovalVerifier {
    fn verify<'a>(
        &'a self,
        _definition: &'a CapabilityDefinition,
        _request: &'a CapabilityRequest,
        _approval: &'a ApprovalEvidence,
    ) -> PortFuture<'a, Result<(), SdkError>> {
        record(&self.calls, "approval");
        let result = self.error.clone().map_or(Ok(()), Err);
        Box::pin(async move { result })
    }
}

#[derive(Clone)]
pub struct FixedAuthorizer {
    pub decision: AuthorizationDecision,
    pub error: Option<SdkError>,
    pub calls: CallLog,
}

impl CapabilityAuthorizer for FixedAuthorizer {
    fn authorize<'a>(
        &'a self,
        _definition: &'a CapabilityDefinition,
        _request: &'a CapabilityRequest,
    ) -> PortFuture<'a, Result<AuthorizationDecision, SdkError>> {
        record(&self.calls, "authorize");
        let result = self
            .error
            .clone()
            .map_or_else(|| Ok(self.decision.clone()), Err);
        Box::pin(async move { result })
    }
}

#[derive(Clone)]
pub struct RecordingExecutor {
    pub result: Result<CapabilityExecutionResult, SdkError>,
    pub calls: CallLog,
}

impl TransactionalCapabilityExecutor for RecordingExecutor {
    fn execute<'a>(
        &'a self,
        _definition: &'a CapabilityDefinition,
        _request: CapabilityRequest,
    ) -> PortFuture<'a, Result<CapabilityExecutionResult, SdkError>> {
        record(&self.calls, "execute");
        let result = self.result.clone();
        Box::pin(async move { result })
    }
}

#[derive(Debug, Clone, Copy)]
pub struct FixedClock {
    pub now_unix_nanos: i64,
}

impl Clock for FixedClock {
    fn now_unix_nanos(&self) -> i64 {
        self.now_unix_nanos
    }
}
