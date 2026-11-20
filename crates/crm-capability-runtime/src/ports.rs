use crate::types::{
    ApprovalEvidence, AuthorizationDecision, CapabilityDefinition, CapabilityExecutionResult,
    CapabilityRequest, RateLimitDecision,
};
use crm_module_sdk::{CapabilityId, CapabilityVersion, PortFuture, SdkError};

pub trait CapabilityRegistryPort: Send + Sync {
    fn resolve<'a>(
        &'a self,
        capability_id: &'a CapabilityId,
        capability_version: &'a CapabilityVersion,
    ) -> PortFuture<'a, Result<Option<CapabilityDefinition>, SdkError>>;
}

pub trait CapabilitySemanticValidator: Send + Sync {
    fn validate<'a>(
        &'a self,
        definition: &'a CapabilityDefinition,
        request: &'a CapabilityRequest,
    ) -> PortFuture<'a, Result<(), SdkError>>;
}

pub trait CapabilityRateLimiter: Send + Sync {
    fn check<'a>(
        &'a self,
        definition: &'a CapabilityDefinition,
        request: &'a CapabilityRequest,
    ) -> PortFuture<'a, Result<RateLimitDecision, SdkError>>;
}

pub trait CapabilityApprovalVerifier: Send + Sync {
    fn verify<'a>(
        &'a self,
        definition: &'a CapabilityDefinition,
        request: &'a CapabilityRequest,
        approval: &'a ApprovalEvidence,
    ) -> PortFuture<'a, Result<(), SdkError>>;
}

pub trait CapabilityAuthorizer: Send + Sync {
    fn authorize<'a>(
        &'a self,
        definition: &'a CapabilityDefinition,
        request: &'a CapabilityRequest,
    ) -> PortFuture<'a, Result<AuthorizationDecision, SdkError>>;
}

pub trait TransactionalCapabilityExecutor: Send + Sync {
    fn execute<'a>(
        &'a self,
        definition: &'a CapabilityDefinition,
        request: CapabilityRequest,
    ) -> PortFuture<'a, Result<CapabilityExecutionResult, SdkError>>;
}
