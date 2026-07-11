use crm_capability_runtime::{
    AuthorizationDecision, CapabilityDefinition, CapabilityRegistryPort,
};
use crm_module_sdk::{
    ActorId, CapabilityId, CapabilityVersion, CorrelationId, ErrorCategory, ModuleId, PortFuture,
    RequestId, SchemaVersion, SdkError, TenantId, TraceId, TypedPayload,
};
use std::error::Error;
use std::fmt;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryExecutionContext {
    pub tenant_id: TenantId,
    pub actor_id: ActorId,
    pub request_id: RequestId,
    pub correlation_id: CorrelationId,
    pub trace_id: TraceId,
    pub capability_id: CapabilityId,
    pub capability_version: CapabilityVersion,
    pub schema_version: SchemaVersion,
    pub request_started_at_unix_nanos: i64,
}

impl QueryExecutionContext {
    pub fn validate(&self) -> Result<(), SdkError> {
        if self.request_started_at_unix_nanos <= 0 {
            return Err(SdkError::new(
                "QUERY_EXECUTION_CONTEXT_INVALID",
                ErrorCategory::InvalidArgument,
                false,
                "The query execution context is invalid.",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryRequest {
    pub owner_module_id: ModuleId,
    pub context: QueryExecutionContext,
    pub input: TypedPayload,
    pub input_hash: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryExecutionResult {
    pub output: TypedPayload,
}

pub trait QuerySemanticValidator: Send + Sync {
    fn validate<'a>(
        &'a self,
        definition: &'a CapabilityDefinition,
        request: &'a QueryRequest,
    ) -> PortFuture<'a, Result<(), SdkError>>;
}

pub trait QueryAuthorizer: Send + Sync {
    fn authorize<'a>(
        &'a self,
        definition: &'a CapabilityDefinition,
        request: &'a QueryRequest,
    ) -> PortFuture<'a, Result<AuthorizationDecision, SdkError>>;
}

pub trait QueryExecutor: Send + Sync {
    fn execute<'a>(
        &'a self,
        definition: &'a CapabilityDefinition,
        request: QueryRequest,
    ) -> PortFuture<'a, Result<QueryExecutionResult, SdkError>>;
}

#[derive(Clone)]
pub struct QueryGateway {
    registry: Arc<dyn CapabilityRegistryPort>,
    validator: Arc<dyn QuerySemanticValidator>,
    authorizer: Arc<dyn QueryAuthorizer>,
    executor: Arc<dyn QueryExecutor>,
}

impl fmt::Debug for QueryGateway {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("QueryGateway")
            .field("registry", &"dyn CapabilityRegistryPort")
            .field("validator", &"dyn QuerySemanticValidator")
            .field("authorizer", &"dyn QueryAuthorizer")
            .field("executor", &"dyn QueryExecutor")
            .finish()
    }
}

impl QueryGateway {
    pub fn new(
        registry: Arc<dyn CapabilityRegistryPort>,
        validator: Arc<dyn QuerySemanticValidator>,
        authorizer: Arc<dyn QueryAuthorizer>,
        executor: Arc<dyn QueryExecutor>,
    ) -> Self {
        Self {
            registry,
            validator,
            authorizer,
            executor,
        }
    }

    pub async fn execute(
        &self,
        request: QueryRequest,
    ) -> Result<QueryExecutionResult, QueryGatewayError> {
        request
            .context
            .validate()
            .map_err(QueryGatewayError::Context)?;
        request.input.validate().map_err(QueryGatewayError::Input)?;
        if request.input_hash.iter().all(|byte| *byte == 0) {
            return Err(QueryGatewayError::InputHashMissing);
        }

        let definition = self
            .registry
            .resolve(
                &request.context.capability_id,
                &request.context.capability_version,
            )
            .await
            .map_err(QueryGatewayError::Registry)?
            .ok_or(QueryGatewayError::QueryNotFound)?;

        validate_definition_binding(&definition, &request)?;
        if !definition.input_contract.matches(&request.input) {
            return Err(QueryGatewayError::InputContractMismatch);
        }
        self.validator
            .validate(&definition, &request)
            .await
            .map_err(QueryGatewayError::SemanticValidation)?;

        // Invariant: live authorization is the final awaited policy decision before
        // the authoritative read executor. The executor may await PostgreSQL only.
        let authorization = self
            .authorizer
            .authorize(&definition, &request)
            .await
            .map_err(QueryGatewayError::AuthorizationDependency)?;
        if !authorization.allowed {
            return Err(QueryGatewayError::PermissionDenied {
                decision_id: authorization.decision_id,
                reason_code: authorization.reason_code,
            });
        }

        let result = self
            .executor
            .execute(&definition, request)
            .await
            .map_err(QueryGatewayError::Execution)?;
        validate_output_contract(&definition, &result)?;
        Ok(result)
    }
}

fn validate_definition_binding(
    definition: &CapabilityDefinition,
    request: &QueryRequest,
) -> Result<(), QueryGatewayError> {
    if definition.capability_id != request.context.capability_id
        || definition.capability_version != request.context.capability_version
        || definition.owner_module_id != request.owner_module_id
    {
        return Err(QueryGatewayError::DefinitionMismatch);
    }
    if definition.mutation
        || definition.requires_idempotency
        || definition.requires_approval
        || definition.authorization_policy_id.is_empty()
        || definition.input_contract.allowed_data_classes.is_empty()
        || definition.input_contract.allowed_encodings.is_empty()
        || definition
            .input_contract
            .descriptor_hash
            .iter()
            .all(|byte| *byte == 0)
    {
        return Err(QueryGatewayError::InvalidDefinition);
    }
    Ok(())
}

fn validate_output_contract(
    definition: &CapabilityDefinition,
    result: &QueryExecutionResult,
) -> Result<(), QueryGatewayError> {
    let contract = definition
        .output_contract
        .as_ref()
        .ok_or(QueryGatewayError::OutputContractMismatch)?;
    result.output.validate().map_err(QueryGatewayError::Output)?;
    if contract.matches(&result.output) {
        Ok(())
    } else {
        Err(QueryGatewayError::OutputContractMismatch)
    }
}

#[derive(Debug)]
pub enum QueryGatewayError {
    Context(SdkError),
    Input(SdkError),
    InputHashMissing,
    Registry(SdkError),
    QueryNotFound,
    DefinitionMismatch,
    InvalidDefinition,
    InputContractMismatch,
    SemanticValidation(SdkError),
    AuthorizationDependency(SdkError),
    PermissionDenied {
        decision_id: String,
        reason_code: String,
    },
    Execution(SdkError),
    Output(SdkError),
    OutputContractMismatch,
}

impl fmt::Display for QueryGatewayError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Context(_) => "query execution context is invalid",
            Self::Input(_) => "query input is invalid",
            Self::InputHashMissing => "query semantic input hash is missing",
            Self::Registry(_) => "query capability registry is unavailable",
            Self::QueryNotFound => "query capability was not found",
            Self::DefinitionMismatch => "query definition does not match the execution context",
            Self::InvalidDefinition => "query capability definition is invalid",
            Self::InputContractMismatch => "query input contract does not match",
            Self::SemanticValidation(_) => "query semantic validation failed",
            Self::AuthorizationDependency(_) => "query authorization service is unavailable",
            Self::PermissionDenied { .. } => "query authorization was denied",
            Self::Execution(_) => "query execution failed",
            Self::Output(_) | Self::OutputContractMismatch => {
                "query output contract is invalid"
            }
        })
    }
}

impl Error for QueryGatewayError {}

pub fn query_gateway_error_to_sdk(error: QueryGatewayError) -> SdkError {
    match error {
        QueryGatewayError::Context(error)
        | QueryGatewayError::Input(error)
        | QueryGatewayError::SemanticValidation(error) => error,
        QueryGatewayError::QueryNotFound => SdkError::new(
            "QUERY_NOT_FOUND",
            ErrorCategory::NotFound,
            false,
            "The requested query was not found.",
        ),
        QueryGatewayError::PermissionDenied { decision_id, .. } => SdkError::new(
            "QUERY_PERMISSION_DENIED",
            ErrorCategory::Authorization,
            false,
            "You are not permitted to perform this query.",
        )
        .with_internal_reference(decision_id),
        QueryGatewayError::Registry(error)
        | QueryGatewayError::AuthorizationDependency(error)
        | QueryGatewayError::Execution(error) => error,
        QueryGatewayError::InputHashMissing
        | QueryGatewayError::DefinitionMismatch
        | QueryGatewayError::InvalidDefinition
        | QueryGatewayError::InputContractMismatch
        | QueryGatewayError::Output(_)
        | QueryGatewayError::OutputContractMismatch => SdkError::new(
            "QUERY_INVALID",
            ErrorCategory::InvalidArgument,
            false,
            "The query request is invalid.",
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crm_capability_runtime::{CapabilityRisk, PayloadContract};
    use crm_module_sdk::{DataClass, PayloadEncoding, RetentionPolicyId, SchemaId};
    use std::sync::Mutex;

    #[derive(Debug)]
    struct Registry {
        definition: CapabilityDefinition,
    }

    impl CapabilityRegistryPort for Registry {
        fn resolve<'a>(
            &'a self,
            capability_id: &'a CapabilityId,
            capability_version: &'a CapabilityVersion,
        ) -> PortFuture<'a, Result<Option<CapabilityDefinition>, SdkError>> {
            Box::pin(async move {
                if &self.definition.capability_id == capability_id
                    && &self.definition.capability_version == capability_version
                {
                    Ok(Some(self.definition.clone()))
                } else {
                    Ok(None)
                }
            })
        }
    }

    #[derive(Debug)]
    struct Validator;

    impl QuerySemanticValidator for Validator {
        fn validate<'a>(
            &'a self,
            _definition: &'a CapabilityDefinition,
            _request: &'a QueryRequest,
        ) -> PortFuture<'a, Result<(), SdkError>> {
            Box::pin(async { Ok(()) })
        }
    }

    #[derive(Debug)]
    struct Authorizer {
        allowed: bool,
        calls: Arc<Mutex<usize>>,
    }

    impl QueryAuthorizer for Authorizer {
        fn authorize<'a>(
            &'a self,
            _definition: &'a CapabilityDefinition,
            _request: &'a QueryRequest,
        ) -> PortFuture<'a, Result<AuthorizationDecision, SdkError>> {
            Box::pin(async move {
                *self.calls.lock().unwrap() += 1;
                Ok(AuthorizationDecision {
                    allowed: self.allowed,
                    decision_id: "decision-query".to_owned(),
                    reason_code: if self.allowed { "allowed" } else { "denied" }.to_owned(),
                    policy_version: "1".to_owned(),
                })
            })
        }
    }

    #[derive(Debug)]
    struct Executor {
        calls: Arc<Mutex<usize>>,
        output: TypedPayload,
    }

    impl QueryExecutor for Executor {
        fn execute<'a>(
            &'a self,
            _definition: &'a CapabilityDefinition,
            _request: QueryRequest,
        ) -> PortFuture<'a, Result<QueryExecutionResult, SdkError>> {
            Box::pin(async move {
                *self.calls.lock().unwrap() += 1;
                Ok(QueryExecutionResult {
                    output: self.output.clone(),
                })
            })
        }
    }

    #[tokio::test]
    async fn live_authorization_denial_never_reaches_query_executor() {
        let definition = definition();
        let authorizer_calls = Arc::new(Mutex::new(0));
        let executor_calls = Arc::new(Mutex::new(0));
        let gateway = QueryGateway::new(
            Arc::new(Registry {
                definition: definition.clone(),
            }),
            Arc::new(Validator),
            Arc::new(Authorizer {
                allowed: false,
                calls: Arc::clone(&authorizer_calls),
            }),
            Arc::new(Executor {
                calls: Arc::clone(&executor_calls),
                output: output_payload(&definition),
            }),
        );

        let error = gateway.execute(request(&definition)).await.unwrap_err();
        assert!(matches!(error, QueryGatewayError::PermissionDenied { .. }));
        assert_eq!(*authorizer_calls.lock().unwrap(), 1);
        assert_eq!(*executor_calls.lock().unwrap(), 0);
    }

    #[tokio::test]
    async fn query_gateway_rejects_mutation_definition_before_authorization() {
        let mut definition = definition();
        definition.mutation = true;
        let authorizer_calls = Arc::new(Mutex::new(0));
        let executor_calls = Arc::new(Mutex::new(0));
        let gateway = QueryGateway::new(
            Arc::new(Registry {
                definition: definition.clone(),
            }),
            Arc::new(Validator),
            Arc::new(Authorizer {
                allowed: true,
                calls: Arc::clone(&authorizer_calls),
            }),
            Arc::new(Executor {
                calls: Arc::clone(&executor_calls),
                output: output_payload(&definition),
            }),
        );

        let error = gateway.execute(request(&definition)).await.unwrap_err();
        assert!(matches!(error, QueryGatewayError::InvalidDefinition));
        assert_eq!(*authorizer_calls.lock().unwrap(), 0);
        assert_eq!(*executor_calls.lock().unwrap(), 0);
    }

    fn definition() -> CapabilityDefinition {
        let input_contract = contract("crm.test.QueryRequest");
        let output_contract = contract("crm.test.QueryResponse");
        CapabilityDefinition {
            capability_id: CapabilityId::try_new("test.record.get").unwrap(),
            capability_version: CapabilityVersion::try_new("1.0.0").unwrap(),
            owner_module_id: ModuleId::try_new("crm.test").unwrap(),
            input_contract,
            output_contract: Some(output_contract),
            risk: CapabilityRisk::Low,
            mutation: false,
            requires_idempotency: false,
            requires_approval: false,
            authorization_policy_id: "test.record.get".to_owned(),
            rate_limit_policy_id: None,
        }
    }

    fn contract(schema: &str) -> PayloadContract {
        PayloadContract {
            owner: ModuleId::try_new("crm.test").unwrap(),
            schema_id: SchemaId::try_new(schema).unwrap(),
            schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
            descriptor_hash: [0x51; 32],
            allowed_data_classes: vec![DataClass::Internal],
            allowed_encodings: vec![PayloadEncoding::Protobuf],
            maximum_size_bytes: 1024,
        }
    }

    fn request(definition: &CapabilityDefinition) -> QueryRequest {
        QueryRequest {
            owner_module_id: definition.owner_module_id.clone(),
            context: QueryExecutionContext {
                tenant_id: TenantId::try_new("tenant-a").unwrap(),
                actor_id: ActorId::try_new("actor-a").unwrap(),
                request_id: RequestId::try_new("request-a").unwrap(),
                correlation_id: CorrelationId::try_new("correlation-a").unwrap(),
                trace_id: TraceId::try_new("trace-a").unwrap(),
                capability_id: definition.capability_id.clone(),
                capability_version: definition.capability_version.clone(),
                schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
                request_started_at_unix_nanos: 1,
            },
            input: input_payload(definition),
            input_hash: [1; 32],
        }
    }

    fn input_payload(definition: &CapabilityDefinition) -> TypedPayload {
        TypedPayload {
            owner: definition.input_contract.owner.clone(),
            schema_id: definition.input_contract.schema_id.clone(),
            schema_version: definition.input_contract.schema_version.clone(),
            descriptor_hash: definition.input_contract.descriptor_hash,
            data_class: DataClass::Internal,
            encoding: PayloadEncoding::Protobuf,
            maximum_size_bytes: definition.input_contract.maximum_size_bytes,
            retention_policy_id: RetentionPolicyId::try_new("standard").unwrap(),
            bytes: vec![1],
        }
    }

    fn output_payload(definition: &CapabilityDefinition) -> TypedPayload {
        let contract = definition.output_contract.as_ref().unwrap();
        TypedPayload {
            owner: contract.owner.clone(),
            schema_id: contract.schema_id.clone(),
            schema_version: contract.schema_version.clone(),
            descriptor_hash: contract.descriptor_hash,
            data_class: DataClass::Internal,
            encoding: PayloadEncoding::Protobuf,
            maximum_size_bytes: contract.maximum_size_bytes,
            retention_policy_id: RetentionPolicyId::try_new("standard").unwrap(),
            bytes: vec![2],
        }
    }
}
