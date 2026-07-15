use crm_capability_ingress::{
    CapabilityRoute, QueryCallEnvelope, QueryContextResolver, QueryIngressMetadata,
    RequestAuthenticator,
};
use crm_capability_runtime::CapabilityDefinition;
use crm_core_data::{
    AuditIntent, AuditedReadPlan, ImmutableFileArtifactStore, PostgresDataStore,
    PostgresImmutableFileArtifactStore, database_error_to_sdk,
};
use crm_customer_data_operations_capability_adapter::MODULE_ID;
use crm_customer_data_operations_query_adapter::{
    PartyExportArtifactDownloadResolver, artifact_download_capability_definition,
    artifact_download_request_payload,
};
use crm_module_sdk::{
    BusinessTransactionId, CausationId, DataClass, ErrorCategory, ExecutionContext, IdempotencyKey,
    ModuleExecutionContext, ModuleId, SdkError,
};
use crm_query_runtime::{QueryAuthorizer, QueryRequest};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartyExportArtifactDownloadRequest {
    pub authorization: String,
    pub tenant_id: Option<String>,
    pub request_id: Option<String>,
    pub correlation_id: Option<String>,
    pub trace_id: Option<String>,
    pub timeout_millis: Option<u64>,
    pub export_job_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartyExportArtifactDownloadResult {
    pub export_job_id: String,
    pub media_type: String,
    pub content_sha256: [u8; 32],
    pub bytes: Vec<u8>,
}

#[derive(Clone)]
pub struct PartyExportArtifactDownloadService {
    authenticator: Arc<dyn RequestAuthenticator>,
    context_resolver: QueryContextResolver,
    authorizer: Arc<dyn QueryAuthorizer>,
    resolver: Arc<PartyExportArtifactDownloadResolver>,
    file_store: Arc<PostgresImmutableFileArtifactStore>,
    store: PostgresDataStore,
    retention_policies: BTreeMap<String, u64>,
    definition: CapabilityDefinition,
}

impl std::fmt::Debug for PartyExportArtifactDownloadService {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PartyExportArtifactDownloadService")
            .field("authenticator", &"dyn RequestAuthenticator")
            .field("context_resolver", &self.context_resolver)
            .field("authorizer", &"dyn QueryAuthorizer")
            .field("resolver", &self.resolver)
            .field("file_store", &self.file_store)
            .field("definition", &self.definition.capability_id)
            .finish()
    }
}

impl PartyExportArtifactDownloadService {
    pub fn new(
        authenticator: Arc<dyn RequestAuthenticator>,
        context_resolver: QueryContextResolver,
        authorizer: Arc<dyn QueryAuthorizer>,
        resolver: Arc<PartyExportArtifactDownloadResolver>,
        file_store: Arc<PostgresImmutableFileArtifactStore>,
        store: PostgresDataStore,
        retention_policies: BTreeMap<String, u64>,
    ) -> Result<Self, SdkError> {
        validate_retention_policies(&retention_policies)?;
        Ok(Self {
            authenticator,
            context_resolver,
            authorizer,
            resolver,
            file_store,
            store,
            retention_policies,
            definition: artifact_download_capability_definition()?,
        })
    }

    pub fn definition(&self) -> &CapabilityDefinition {
        &self.definition
    }

    pub async fn download(
        &self,
        request: PartyExportArtifactDownloadRequest,
    ) -> Result<PartyExportArtifactDownloadResult, SdkError> {
        let principal = self
            .authenticator
            .authenticate(&request.authorization)
            .await
            .map_err(authentication_error)?;
        let input = artifact_download_request_payload(&request.export_job_id)?;
        let resolved = self
            .context_resolver
            .resolve(
                &principal,
                QueryCallEnvelope {
                    route: CapabilityRoute {
                        owner_module_id: self.definition.owner_module_id.clone(),
                        capability_id: self.definition.capability_id.clone(),
                        capability_version: self.definition.capability_version.clone(),
                        schema_version: input.schema_version.clone(),
                    },
                    input,
                    metadata: QueryIngressMetadata {
                        tenant_id: request.tenant_id,
                        request_id: request.request_id,
                        correlation_id: request.correlation_id,
                        trace_id: request.trace_id,
                        timeout_millis: request.timeout_millis,
                    },
                },
            )
            .map_err(context_error)?;
        let timeout = Duration::from_millis(resolved.timeout.duration_millis);
        tokio::time::timeout(
            timeout,
            self.download_authorized(
                resolved.request,
                resolved.authentication_id,
                request.export_job_id,
            ),
        )
        .await
        .map_err(|_| download_timeout())?
    }

    async fn download_authorized(
        &self,
        request: QueryRequest,
        authentication_id: String,
        requested_export_job_id: String,
    ) -> Result<PartyExportArtifactDownloadResult, SdkError> {
        let decision = self
            .authorizer
            .authorize(&self.definition, &request)
            .await?;
        if !decision.allowed {
            return Err(SdkError::new(
                "CUSTOMER_DATA_EXPORT_ARTIFACT_DOWNLOAD_PERMISSION_DENIED",
                ErrorCategory::Authorization,
                false,
                "The export artifact disclosure is not authorized.",
            )
            .with_internal_reference(format!(
                "decision_id={} reason_code={} policy_version={}",
                decision.decision_id, decision.reason_code, decision.policy_version
            )));
        }

        let evidence = self.resolver.resolve(&request).await?;
        if evidence.export_job_id != requested_export_job_id {
            return Err(integrity_error("resolved export job identity changed"));
        }
        let expires_at_unix_nanos = artifact_expires_at_unix_nanos(
            &evidence.retention_policy_id,
            evidence.completed_at_unix_nanos,
            &self.retention_policies,
        )?;
        if request.context.request_started_at_unix_nanos >= expires_at_unix_nanos {
            return Err(artifact_expired());
        }
        let context = disclosure_execution_context(&request)?;
        let finalized = self
            .file_store
            .read_finalized(&context, &evidence.file_id)
            .await?;
        validate_finalized_artifact(&evidence, &finalized.metadata)?;

        let audit = disclosure_audit_intent(
            &request,
            &authentication_id,
            &decision.decision_id,
            &decision.policy_version,
            &evidence.export_job_id,
            evidence.export_job_version,
            evidence.file_id.as_str(),
            &evidence.content_sha256,
            evidence.size_bytes,
            &evidence.retention_policy_id,
            expires_at_unix_nanos,
        )?;
        self.store
            .record_audited_read(&AuditedReadPlan { context, audit })
            .await
            .map_err(database_error_to_sdk)?;

        Ok(PartyExportArtifactDownloadResult {
            export_job_id: evidence.export_job_id,
            media_type: evidence.media_type.to_owned(),
            content_sha256: evidence.content_sha256,
            bytes: finalized.bytes,
        })
    }
}

fn disclosure_execution_context(
    request: &QueryRequest,
) -> Result<ModuleExecutionContext, SdkError> {
    let request_identity = request.context.request_id.as_str().to_owned();
    Ok(ModuleExecutionContext {
        module_id: ModuleId::try_new(MODULE_ID).map_err(identifier_error)?,
        execution: ExecutionContext {
            tenant_id: request.context.tenant_id.clone(),
            actor_id: request.context.actor_id.clone(),
            request_id: request.context.request_id.clone(),
            correlation_id: request.context.correlation_id.clone(),
            causation_id: CausationId::try_new(request_identity.clone())
                .map_err(identifier_error)?,
            trace_id: request.context.trace_id.clone(),
            capability_id: request.context.capability_id.clone(),
            capability_version: request.context.capability_version.clone(),
            idempotency_key: IdempotencyKey::try_new(request_identity.clone())
                .map_err(identifier_error)?,
            business_transaction_id: BusinessTransactionId::try_new(request_identity)
                .map_err(identifier_error)?,
            schema_version: request.context.schema_version.clone(),
            request_started_at_unix_nanos: request.context.request_started_at_unix_nanos,
        },
    })
}

fn validate_finalized_artifact(
    evidence: &crm_customer_data_operations_query_adapter::PartyExportArtifactDownloadEvidence,
    metadata: &crm_core_data::FileArtifactMetadata,
) -> Result<(), SdkError> {
    if metadata.file_id != evidence.file_id
        || metadata.owner_module_id.as_str() != MODULE_ID
        || metadata.media_type != evidence.media_type
        || metadata.data_class != DataClass::Personal
        || metadata.retention_policy_id.as_str() != evidence.retention_policy_id.as_str()
        || metadata.expected_size_bytes != evidence.size_bytes
        || metadata.received_size_bytes != evidence.size_bytes
        || metadata.expected_sha256 != evidence.content_sha256
        || metadata.status != crm_core_data::FileArtifactStatus::Finalized
    {
        return Err(integrity_error(
            "finalized artifact metadata does not match authoritative export job evidence",
        ));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn disclosure_audit_intent(
    request: &QueryRequest,
    authentication_id: &str,
    authorization_decision_id: &str,
    authorization_policy_version: &str,
    export_job_id: &str,
    export_job_version: i64,
    file_id: &str,
    content_sha256: &[u8; 32],
    size_bytes: u64,
    retention_policy_id: &str,
    expires_at_unix_nanos: i64,
) -> Result<AuditIntent, SdkError> {
    let mut envelope = BTreeMap::new();
    envelope.insert("actor_id", request.context.actor_id.as_str().to_owned());
    envelope.insert("authentication_id", authentication_id.to_owned());
    envelope.insert(
        "authorization_decision_id",
        authorization_decision_id.to_owned(),
    );
    envelope.insert(
        "authorization_policy_version",
        authorization_policy_version.to_owned(),
    );
    envelope.insert(
        "capability_id",
        request.context.capability_id.as_str().to_owned(),
    );
    envelope.insert("content_sha256", hex(content_sha256));
    envelope.insert("export_job_id", export_job_id.to_owned());
    envelope.insert("export_job_version", export_job_version.to_string());
    envelope.insert("expires_at_unix_nanos", expires_at_unix_nanos.to_string());
    envelope.insert("file_id", file_id.to_owned());
    envelope.insert(
        "operation",
        "customer_data.export.artifact.disclose".to_owned(),
    );
    envelope.insert("request_hash", hex(&request.input_hash));
    envelope.insert("retention_policy_id", retention_policy_id.to_owned());
    envelope.insert("size_bytes", size_bytes.to_string());
    envelope.insert("tenant_id", request.context.tenant_id.as_str().to_owned());
    let canonical_envelope = serde_json::to_vec(&envelope).map_err(|_| {
        SdkError::new(
            "CUSTOMER_DATA_EXPORT_ARTIFACT_AUDIT_SERIALIZATION_FAILED",
            ErrorCategory::Internal,
            false,
            "The export artifact disclosure audit evidence could not be produced.",
        )
    })?;
    Ok(AuditIntent {
        audit_record_id: format!(
            "export-artifact-disclosure-{}",
            hex(&Sha256::digest(
                request.context.request_id.as_str().as_bytes()
            )),
        ),
        canonicalization_profile: "crm.cjson/v1".to_owned(),
        canonical_envelope,
        occurred_at_unix_nanos: request.context.request_started_at_unix_nanos,
    })
}

fn authentication_error(error: crm_capability_ingress::AuthenticationError) -> SdkError {
    SdkError::new(
        error.code(),
        ErrorCategory::Authentication,
        error.retryable(),
        "Authentication is required for export artifact disclosure.",
    )
}

fn context_error(error: crm_capability_ingress::ContextResolutionError) -> SdkError {
    let category = match &error {
        crm_capability_ingress::ContextResolutionError::TenantForbidden => {
            ErrorCategory::Authorization
        }
        crm_capability_ingress::ContextResolutionError::ClockInvalid
        | crm_capability_ingress::ContextResolutionError::IdentityGenerationUnavailable => {
            ErrorCategory::Unavailable
        }
        crm_capability_ingress::ContextResolutionError::InvalidServerConfiguration => {
            ErrorCategory::Internal
        }
        _ => ErrorCategory::InvalidArgument,
    };
    SdkError::new(
        error.code(),
        category,
        error.retryable(),
        "The export artifact disclosure request context is invalid.",
    )
}

fn validate_retention_policies(policies: &BTreeMap<String, u64>) -> Result<(), SdkError> {
    for (policy_id, seconds) in policies {
        if policy_id.is_empty()
            || policy_id.chars().any(char::is_control)
            || *seconds == 0
            || seconds
                .checked_mul(1_000_000_000)
                .and_then(|value| i64::try_from(value).ok())
                .is_none()
        {
            return Err(retention_configuration_error());
        }
    }
    Ok(())
}

fn artifact_expires_at_unix_nanos(
    policy_id: &str,
    completed_at_unix_nanos: i64,
    policies: &BTreeMap<String, u64>,
) -> Result<i64, SdkError> {
    let seconds = policies.get(policy_id).ok_or_else(retention_policy_unavailable)?;
    let duration_nanos = seconds
        .checked_mul(1_000_000_000)
        .and_then(|value| i64::try_from(value).ok())
        .ok_or_else(retention_configuration_error)?;
    completed_at_unix_nanos
        .checked_add(duration_nanos)
        .ok_or_else(retention_configuration_error)
}

fn artifact_expired() -> SdkError {
    SdkError::new(
        "CUSTOMER_DATA_EXPORT_ARTIFACT_EXPIRED",
        ErrorCategory::NotFound,
        false,
        "The requested export artifact is unavailable.",
    )
}

fn retention_policy_unavailable() -> SdkError {
    SdkError::new(
        "CUSTOMER_DATA_EXPORT_RETENTION_POLICY_UNAVAILABLE",
        ErrorCategory::Internal,
        false,
        "The export artifact retention policy is not configured.",
    )
}

fn retention_configuration_error() -> SdkError {
    SdkError::new(
        "CUSTOMER_DATA_EXPORT_RETENTION_CONFIGURATION_INVALID",
        ErrorCategory::Internal,
        false,
        "The export artifact retention policy configuration is invalid.",
    )
}

fn download_timeout() -> SdkError {
    SdkError::new(
        "CUSTOMER_DATA_EXPORT_ARTIFACT_DOWNLOAD_TIMEOUT",
        ErrorCategory::Unavailable,
        true,
        "The export artifact disclosure timed out.",
    )
}

fn integrity_error(reference: &str) -> SdkError {
    SdkError::new(
        "CUSTOMER_DATA_EXPORT_ARTIFACT_INTEGRITY_INVALID",
        ErrorCategory::Internal,
        false,
        "The export artifact failed integrity validation.",
    )
    .with_internal_reference(reference)
}

fn identifier_error(error: crm_module_sdk::IdentifierError) -> SdkError {
    SdkError::new(
        "CUSTOMER_DATA_EXPORT_ARTIFACT_CONTEXT_INVALID",
        ErrorCategory::Internal,
        false,
        "The export artifact disclosure context is invalid.",
    )
    .with_internal_reference(error.to_string())
}

fn hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        write!(&mut output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]

    #[test]
    fn retention_expiry_is_exact_and_unknown_policies_fail_closed() {
        let policies = BTreeMap::from([("standard".to_owned(), 60)]);
        assert_eq!(
            artifact_expires_at_unix_nanos("standard", 1_000_000_000, &policies).unwrap(),
            61_000_000_000
        );
        assert!(artifact_expires_at_unix_nanos("unknown", 1, &policies).is_err());
        assert!(artifact_expires_at_unix_nanos("standard", i64::MAX, &policies).is_err());
    }

    fn disclosure_definition_is_job_bound_high_risk_read() {
        let definition = artifact_download_capability_definition().unwrap();
        assert!(!definition.mutation);
        assert_eq!(
            definition.risk,
            crm_capability_runtime::CapabilityRisk::High
        );
        assert!(definition.output_contract.is_none());
    }
}
