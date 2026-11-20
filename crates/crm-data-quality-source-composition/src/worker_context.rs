use crm_data_quality_capability_adapter::STAGE_PARTY_EVALUATION_INPUT_CAPABILITY;
use crm_module_sdk::{
    ActorId, BusinessTransactionId, CapabilityId, CapabilityVersion, CausationId, CorrelationId,
    ExecutionContext, IdempotencyKey, ModuleExecutionContext, ModuleId, RequestId, SchemaVersion,
    SdkError, TenantId, TraceId,
};

pub const EVALUATION_WORKER_ACTOR_ID: &str = "crm-api-data-quality-evaluation-worker";
pub const EVALUATION_WORKER_CAPABILITY_VERSION: &str = "1.0.0";

pub(crate) fn evaluation_worker_context(
    tenant_id: &TenantId,
    actor_id: &ActorId,
    job_id: &str,
    now: i64,
) -> Result<ModuleExecutionContext, SdkError> {
    Ok(ModuleExecutionContext {
        module_id: ModuleId::try_new(crm_data_quality::MODULE_ID).map_err(config_error)?,
        execution: ExecutionContext {
            tenant_id: tenant_id.clone(),
            actor_id: actor_id.clone(),
            request_id: RequestId::try_new(job_id).map_err(config_error)?,
            correlation_id: CorrelationId::try_new(job_id).map_err(config_error)?,
            causation_id: CausationId::try_new(job_id).map_err(config_error)?,
            trace_id: TraceId::try_new(job_id).map_err(config_error)?,
            capability_id: CapabilityId::try_new(STAGE_PARTY_EVALUATION_INPUT_CAPABILITY)
                .map_err(config_error)?,
            capability_version: CapabilityVersion::try_new(EVALUATION_WORKER_CAPABILITY_VERSION)
                .map_err(config_error)?,
            idempotency_key: IdempotencyKey::try_new(job_id).map_err(config_error)?,
            business_transaction_id: BusinessTransactionId::try_new(job_id)
                .map_err(config_error)?,
            schema_version: SchemaVersion::try_new(EVALUATION_WORKER_CAPABILITY_VERSION)
                .map_err(config_error)?,
            request_started_at_unix_nanos: now,
        },
    })
}

pub(crate) fn worker_actor_id() -> Result<ActorId, SdkError> {
    ActorId::try_new(EVALUATION_WORKER_ACTOR_ID).map_err(config_error)
}

fn config_error(error: crm_module_sdk::IdentifierError) -> SdkError {
    SdkError::new(
        "DATA_QUALITY_EVALUATION_WORKER_CONFIGURATION_INVALID",
        crm_module_sdk::ErrorCategory::Internal,
        false,
        "The Data Quality evaluation worker is not configured safely.",
    )
    .with_internal_reference(error.to_string())
}
