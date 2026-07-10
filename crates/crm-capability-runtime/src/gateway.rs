use crate::ports::{
    CapabilityApprovalVerifier, CapabilityAuthorizer, CapabilityRateLimiter,
    CapabilityRegistryPort, CapabilitySemanticValidator, TransactionalCapabilityExecutor,
};
use crate::types::{CapabilityDefinition, CapabilityExecutionResult, CapabilityRequest};
use crm_module_sdk::{Clock, ErrorCategory, SdkError};
use std::error::Error;
use std::fmt;
use std::sync::Arc;

#[derive(Clone)]
pub struct CapabilityGateway {
    registry: Arc<dyn CapabilityRegistryPort>,
    validator: Arc<dyn CapabilitySemanticValidator>,
    rate_limiter: Arc<dyn CapabilityRateLimiter>,
    approval_verifier: Arc<dyn CapabilityApprovalVerifier>,
    authorizer: Arc<dyn CapabilityAuthorizer>,
    executor: Arc<dyn TransactionalCapabilityExecutor>,
    clock: Arc<dyn Clock>,
}

impl fmt::Debug for CapabilityGateway {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CapabilityGateway")
            .field("registry", &"dyn CapabilityRegistryPort")
            .field("validator", &"dyn CapabilitySemanticValidator")
            .field("rate_limiter", &"dyn CapabilityRateLimiter")
            .field("approval_verifier", &"dyn CapabilityApprovalVerifier")
            .field("authorizer", &"dyn CapabilityAuthorizer")
            .field("executor", &"dyn TransactionalCapabilityExecutor")
            .field("clock", &"dyn Clock")
            .finish()
    }
}

impl CapabilityGateway {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        registry: Arc<dyn CapabilityRegistryPort>,
        validator: Arc<dyn CapabilitySemanticValidator>,
        rate_limiter: Arc<dyn CapabilityRateLimiter>,
        approval_verifier: Arc<dyn CapabilityApprovalVerifier>,
        authorizer: Arc<dyn CapabilityAuthorizer>,
        executor: Arc<dyn TransactionalCapabilityExecutor>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            registry,
            validator,
            rate_limiter,
            approval_verifier,
            authorizer,
            executor,
            clock,
        }
    }

    pub async fn execute(
        &self,
        request: CapabilityRequest,
    ) -> Result<CapabilityExecutionResult, GatewayError> {
        request.context.validate().map_err(GatewayError::Context)?;
        request.input.validate().map_err(GatewayError::Input)?;
        if request.input_hash.iter().all(|byte| *byte == 0) {
            return Err(GatewayError::InputHashMissing);
        }

        let capability_id = &request.context.execution.capability_id;
        let capability_version = &request.context.execution.capability_version;
        let definition = self
            .registry
            .resolve(capability_id, capability_version)
            .await
            .map_err(GatewayError::Registry)?
            .ok_or(GatewayError::CapabilityNotFound)?;

        validate_definition_binding(&definition, &request)?;
        validate_input_contract(&definition, &request)?;
        self.validator
            .validate(&definition, &request)
            .await
            .map_err(GatewayError::SemanticValidation)?;

        if definition.rate_limit_policy_id.is_some() {
            let decision = self
                .rate_limiter
                .check(&definition, &request)
                .await
                .map_err(GatewayError::RateLimitDependency)?;
            if !decision.allowed {
                return Err(GatewayError::RateLimited {
                    decision_id: decision.decision_id,
                    retry_after_millis: decision.retry_after_millis,
                });
            }
        }

        if definition.requires_approval {
            let approval = request
                .approval
                .as_ref()
                .ok_or(GatewayError::ApprovalRequired)?;
            validate_approval_binding(
                &definition,
                &request,
                approval,
                self.clock.now_unix_nanos(),
            )?;
            self.approval_verifier
                .verify(&definition, &request, approval)
                .await
                .map_err(GatewayError::ApprovalInvalid)?;
        }

        // Invariant: live authorization is the final awaited decision before the
        // transactional side-effect boundary. Do not insert validation, network
        // access or other awaited work between this call and executor.execute().
        let authorization = self
            .authorizer
            .authorize(&definition, &request)
            .await
            .map_err(GatewayError::AuthorizationDependency)?;
        if !authorization.allowed {
            return Err(GatewayError::PermissionDenied {
                decision_id: authorization.decision_id,
                reason_code: authorization.reason_code,
            });
        }

        let result = self
            .executor
            .execute(&definition, request)
            .await
            .map_err(GatewayError::Execution)?;
        validate_output_contract(&definition, &result)?;
        Ok(result)
    }
}

fn validate_definition_binding(
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
) -> Result<(), GatewayError> {
    if definition.capability_id != request.context.execution.capability_id
        || definition.capability_version != request.context.execution.capability_version
    {
        return Err(GatewayError::DefinitionMismatch);
    }
    if definition.owner_module_id != request.context.module_id {
        return Err(GatewayError::DefinitionMismatch);
    }
    if definition.authorization_policy_id.is_empty()
        || definition.input_contract.allowed_data_classes.is_empty()
        || definition.input_contract.allowed_encodings.is_empty()
        || definition
            .input_contract
            .descriptor_hash
            .iter()
            .all(|byte| *byte == 0)
    {
        return Err(GatewayError::InvalidDefinition);
    }
    if definition.requires_idempotency
        && request
            .context
            .execution
            .idempotency_key
            .as_str()
            .is_empty()
    {
        return Err(GatewayError::IdempotencyRequired);
    }
    Ok(())
}

fn validate_input_contract(
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
) -> Result<(), GatewayError> {
    if !definition.input_contract.matches(&request.input) {
        return Err(GatewayError::InputContractMismatch);
    }
    Ok(())
}

fn validate_output_contract(
    definition: &CapabilityDefinition,
    result: &CapabilityExecutionResult,
) -> Result<(), GatewayError> {
    match (&definition.output_contract, &result.output) {
        (None, None) => Ok(()),
        (Some(contract), Some(output)) => {
            output.validate().map_err(GatewayError::Output)?;
            if contract.matches(output) {
                Ok(())
            } else {
                Err(GatewayError::OutputContractMismatch)
            }
        }
        _ => Err(GatewayError::OutputContractMismatch),
    }
}

fn validate_approval_binding(
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
    approval: &crate::types::ApprovalEvidence,
    now_unix_nanos: i64,
) -> Result<(), GatewayError> {
    if approval.approval_id.is_empty()
        || approval.policy_version.is_empty()
        || approval.opaque_proof.is_empty()
        || approval.actor_id != request.context.execution.actor_id
        || approval.capability_id != definition.capability_id
        || approval.capability_version != definition.capability_version
        || approval.input_hash != request.input_hash
        || approval.expires_at_unix_nanos <= now_unix_nanos
    {
        return Err(GatewayError::ApprovalBindingMismatch);
    }
    Ok(())
}

#[derive(Debug)]
pub enum GatewayError {
    Context(SdkError),
    Input(SdkError),
    InputHashMissing,
    Registry(SdkError),
    CapabilityNotFound,
    DefinitionMismatch,
    InvalidDefinition,
    IdempotencyRequired,
    InputContractMismatch,
    SemanticValidation(SdkError),
    RateLimitDependency(SdkError),
    RateLimited {
        decision_id: String,
        retry_after_millis: Option<u64>,
    },
    ApprovalRequired,
    ApprovalBindingMismatch,
    ApprovalInvalid(SdkError),
    AuthorizationDependency(SdkError),
    PermissionDenied {
        decision_id: String,
        reason_code: String,
    },
    Execution(SdkError),
    Output(SdkError),
    OutputContractMismatch,
}

impl fmt::Display for GatewayError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Context(_) => "execution context is invalid",
            Self::Input(_) => "capability input is invalid",
            Self::InputHashMissing => "semantic input hash is missing",
            Self::Registry(_) => "capability registry is unavailable",
            Self::CapabilityNotFound => "capability was not found",
            Self::DefinitionMismatch => {
                "capability definition does not match the execution context"
            }
            Self::InvalidDefinition => "capability definition is invalid",
            Self::IdempotencyRequired => "capability requires idempotency",
            Self::InputContractMismatch => "capability input contract does not match",
            Self::SemanticValidation(_) => "capability semantic validation failed",
            Self::RateLimitDependency(_) => "rate-limit service is unavailable",
            Self::RateLimited { .. } => "capability rate limit was exceeded",
            Self::ApprovalRequired => "capability approval is required",
            Self::ApprovalBindingMismatch => "capability approval binding is invalid",
            Self::ApprovalInvalid(_) => "capability approval could not be verified",
            Self::AuthorizationDependency(_) => "authorization service is unavailable",
            Self::PermissionDenied { .. } => "capability authorization was denied",
            Self::Execution(_) => "capability execution failed",
            Self::Output(_) | Self::OutputContractMismatch => {
                "capability output contract is invalid"
            }
        })
    }
}

impl Error for GatewayError {}

pub fn gateway_error_to_sdk(error: GatewayError) -> SdkError {
    match error {
        GatewayError::Context(error)
        | GatewayError::Input(error)
        | GatewayError::SemanticValidation(error) => error,
        GatewayError::CapabilityNotFound => SdkError::new(
            "CAPABILITY_NOT_FOUND",
            ErrorCategory::NotFound,
            false,
            "The requested capability was not found.",
        ),
        GatewayError::RateLimited {
            decision_id,
            retry_after_millis,
        } => SdkError::new(
            "CAPABILITY_RATE_LIMITED",
            ErrorCategory::RateLimit,
            true,
            "The capability rate limit was exceeded.",
        )
        .with_internal_reference(format!(
            "decision={decision_id};retry_after_millis={retry_after_millis:?}"
        )),
        GatewayError::ApprovalRequired => SdkError::new(
            "CAPABILITY_APPROVAL_REQUIRED",
            ErrorCategory::Authorization,
            false,
            "Approval is required before this action can be performed.",
        ),
        GatewayError::PermissionDenied { decision_id, .. } => SdkError::new(
            "CAPABILITY_PERMISSION_DENIED",
            ErrorCategory::Authorization,
            false,
            "You are not permitted to perform this action.",
        )
        .with_internal_reference(decision_id),
        GatewayError::Registry(error)
        | GatewayError::RateLimitDependency(error)
        | GatewayError::ApprovalInvalid(error)
        | GatewayError::AuthorizationDependency(error)
        | GatewayError::Execution(error) => error,
        GatewayError::InputHashMissing
        | GatewayError::DefinitionMismatch
        | GatewayError::InvalidDefinition
        | GatewayError::IdempotencyRequired
        | GatewayError::InputContractMismatch
        | GatewayError::ApprovalBindingMismatch
        | GatewayError::Output(_)
        | GatewayError::OutputContractMismatch => SdkError::new(
            "CAPABILITY_INVALID",
            ErrorCategory::InvalidArgument,
            false,
            "The capability request is invalid.",
        ),
    }
}
