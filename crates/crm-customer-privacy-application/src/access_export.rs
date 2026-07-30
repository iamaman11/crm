use crm_application_composition::ModuleActivationPort;
use crm_customer_privacy::{
    ACCESS_EXPORT_REQUEST_COORDINATE, MODULE_ID, PrivacyAccessExportReference,
    encode_access_export_manifest,
};
use crm_module_sdk::{
    ActorId, CapabilityId, CapabilityVersion, CorrelationId, FileId, ModuleId, PortFuture,
    RecordId, RequestId, RetentionPolicyId, SdkError, TenantId, TraceId,
};
use std::sync::Arc;

pub const ACCESS_EXPORT_REQUEST_CAPABILITY: &str = "customer_privacy.access_export.request";
pub const ACCESS_EXPORT_CAPABILITY_VERSION: &str = "1.0.0";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessExportInvocation {
    pub tenant_id: TenantId,
    pub privacy_case_id: RecordId,
    pub action_plan_id: RecordId,
    pub actor_id: ActorId,
    pub request_id: RequestId,
    pub correlation_id: CorrelationId,
    pub trace_id: TraceId,
    /// Registered public capability that initiated the trusted internal orchestration.
    pub initiating_capability_id: CapabilityId,
    /// Registered public capability version preserved for audit provenance.
    pub initiating_capability_version: CapabilityVersion,
    pub request_started_at_unix_nanos: i64,
    pub trusted_internal: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccessExportPreparation {
    Ready {
        reference: PrivacyAccessExportReference,
        replayed: bool,
    },
    Complete {
        reference: PrivacyAccessExportReference,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivacyExportTargetRequest {
    pub tenant_id: TenantId,
    pub privacy_case_id: RecordId,
    pub action_plan_id: RecordId,
    pub export_job_id: RecordId,
    pub target_idempotency_key: String,
    pub manifest_id: RecordId,
    pub manifest_digest: [u8; 32],
    pub manifest_bytes: Vec<u8>,
    pub actor_id: ActorId,
    pub correlation_id: CorrelationId,
    pub trace_id: TraceId,
    pub initiating_capability_id: CapabilityId,
    pub initiating_capability_version: CapabilityVersion,
    pub prepared_at_unix_nanos: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivacyExportTargetResult {
    pub export_job_id: RecordId,
    pub file_id: FileId,
    pub media_type: String,
    pub content_sha256: [u8; 32],
    pub size_bytes: u64,
    pub retention_policy_id: RetentionPolicyId,
    pub completed_at_unix_nanos: i64,
    pub replayed: bool,
}

pub trait AccessExportPersistencePort: Send + Sync {
    fn prepare<'a>(
        &'a self,
        invocation: &'a AccessExportInvocation,
    ) -> PortFuture<'a, Result<AccessExportPreparation, SdkError>>;

    fn complete<'a>(
        &'a self,
        invocation: &'a AccessExportInvocation,
        prepared: &'a PrivacyAccessExportReference,
        result: &'a PrivacyExportTargetResult,
    ) -> PortFuture<'a, Result<(PrivacyAccessExportReference, bool), SdkError>>;
}

pub trait PrivacyExportTargetPort: Send + Sync {
    fn request<'a>(
        &'a self,
        request: PrivacyExportTargetRequest,
    ) -> PortFuture<'a, Result<PrivacyExportTargetResult, SdkError>>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessExportResult {
    pub reference: PrivacyAccessExportReference,
    pub preparation_replayed: bool,
    pub target_invoked: bool,
    pub target_replayed: bool,
    pub completion_replayed: bool,
}

#[derive(Clone)]
pub struct PrivacyAccessExportService {
    activation: Arc<dyn ModuleActivationPort>,
    persistence: Arc<dyn AccessExportPersistencePort>,
    target: Arc<dyn PrivacyExportTargetPort>,
}

impl std::fmt::Debug for PrivacyAccessExportService {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PrivacyAccessExportService")
            .field("activation", &"dyn ModuleActivationPort")
            .field("persistence", &"dyn AccessExportPersistencePort")
            .field("target", &"dyn PrivacyExportTargetPort")
            .finish()
    }
}

impl PrivacyAccessExportService {
    pub fn new(
        activation: Arc<dyn ModuleActivationPort>,
        persistence: Arc<dyn AccessExportPersistencePort>,
        target: Arc<dyn PrivacyExportTargetPort>,
    ) -> Self {
        Self {
            activation,
            persistence,
            target,
        }
    }

    pub async fn request(
        &self,
        invocation: AccessExportInvocation,
    ) -> Result<AccessExportResult, SdkError> {
        validate_invocation(&invocation)?;
        let module_id = ModuleId::try_new(MODULE_ID).map_err(configuration_invalid)?;
        if !self
            .activation
            .is_active(&invocation.tenant_id, &module_id)
            .await?
        {
            return Err(access_export_disabled());
        }

        let (prepared, preparation_replayed) = match self.persistence.prepare(&invocation).await? {
            AccessExportPreparation::Ready {
                reference,
                replayed,
            } => (reference, replayed),
            AccessExportPreparation::Complete { reference } => {
                return Ok(AccessExportResult {
                    reference,
                    preparation_replayed: true,
                    target_invoked: false,
                    target_replayed: true,
                    completion_replayed: true,
                });
            }
        };
        let manifest_bytes = encode_access_export_manifest(prepared.manifest())?;
        let target = self
            .target
            .request(PrivacyExportTargetRequest {
                tenant_id: invocation.tenant_id.clone(),
                privacy_case_id: invocation.privacy_case_id.clone(),
                action_plan_id: invocation.action_plan_id.clone(),
                export_job_id: prepared.export_job_id().clone(),
                target_idempotency_key: prepared.target_idempotency_key().as_str().to_owned(),
                manifest_id: prepared.manifest().manifest_id().clone(),
                manifest_digest: *prepared.manifest().digest(),
                manifest_bytes,
                actor_id: invocation.actor_id.clone(),
                correlation_id: invocation.correlation_id.clone(),
                trace_id: invocation.trace_id.clone(),
                initiating_capability_id: invocation.initiating_capability_id.clone(),
                initiating_capability_version: invocation.initiating_capability_version.clone(),
                prepared_at_unix_nanos: prepared.prepared_at_unix_nanos(),
            })
            .await?;
        let (reference, completed_now) = self
            .persistence
            .complete(&invocation, &prepared, &target)
            .await?;
        Ok(AccessExportResult {
            reference,
            preparation_replayed,
            target_invoked: true,
            target_replayed: target.replayed,
            completion_replayed: !completed_now,
        })
    }
}

fn validate_invocation(invocation: &AccessExportInvocation) -> Result<(), SdkError> {
    if !invocation.trusted_internal {
        return Err(SdkError::new(
            "CUSTOMER_PRIVACY_ACCESS_EXPORT_NOT_TRUSTED",
            crm_module_sdk::ErrorCategory::Authorization,
            false,
            "Customer Privacy access export is available only through trusted internal orchestration.",
        ));
    }
    if ACCESS_EXPORT_REQUEST_COORDINATE
        != format!("{ACCESS_EXPORT_REQUEST_CAPABILITY}@{ACCESS_EXPORT_CAPABILITY_VERSION}")
    {
        return Err(configuration_invalid(
            "access export service does not use the frozen internal coordinate",
        ));
    }
    if invocation.initiating_capability_id.as_str() == ACCESS_EXPORT_REQUEST_CAPABILITY {
        return Err(configuration_invalid(
            "the private access export coordinate cannot replace registered audit provenance",
        ));
    }
    if invocation.request_started_at_unix_nanos <= 0 {
        return Err(SdkError::invalid_argument(
            "access_export.request_started_at_unix_nanos",
            "request start time must be positive",
        ));
    }
    Ok(())
}

fn access_export_disabled() -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_ACCESS_EXPORT_DISABLED",
        crm_module_sdk::ErrorCategory::Conflict,
        false,
        "Customer Privacy is disabled for the tenant.",
    )
}

fn configuration_invalid(error: impl std::fmt::Display) -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_ACCESS_EXPORT_CONFIGURATION_INVALID",
        crm_module_sdk::ErrorCategory::Internal,
        false,
        "Customer Privacy access export is not configured safely.",
    )
    .with_internal_reference(error.to_string())
}
