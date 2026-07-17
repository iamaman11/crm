use crm_capability_runtime::CapabilityAuthorizer;
use crm_core_data::{BatchError, PostgresDataStore, RecordGetQuery, TransactionalAggregatePlanner};
use crm_data_quality_capability_adapter::DataQualityEvaluationStagePlanner;
use crm_module_sdk::{ErrorCategory, SdkError};

use crate::staging_request::PreparedStageRequest;

pub(crate) async fn execute_stage(
    store: &PostgresDataStore,
    authorizer: &dyn CapabilityAuthorizer,
    prepared: PreparedStageRequest,
) -> Result<(), SdkError> {
    let planner = DataQualityEvaluationStagePlanner;
    let target = planner.target(&prepared.definition, &prepared.request)?;
    let current = store
        .get_record_for_query(&RecordGetQuery {
            tenant_id: prepared.request.context.execution.tenant_id.clone(),
            owner_module_id: prepared.definition.owner_module_id.clone(),
            record_type: target.reference.record_type,
            record_id: target.reference.record_id,
        })
        .await?;
    let plan = planner.plan(&prepared.definition, &prepared.request, current.as_ref())?;
    let decision = authorizer
        .authorize(&prepared.definition, &prepared.request)
        .await?;
    if !decision.allowed {
        return Err(SdkError::new(
            "DATA_QUALITY_EVALUATION_STAGE_PERMISSION_DENIED",
            ErrorCategory::Authorization,
            false,
            "The Data Quality worker is not authorized to stage evaluation input.",
        )
        .with_internal_reference(format!(
            "decision_id={} reason_code={} policy_version={}",
            decision.decision_id, decision.reason_code, decision.policy_version
        )));
    }
    store
        .execute_batch(&plan.batch)
        .await
        .map(|_| ())
        .map_err(batch_error_to_sdk)
}

fn batch_error_to_sdk(error: BatchError) -> SdkError {
    match error {
        BatchError::Sdk(error) => error,
        BatchError::Conflict(message) => SdkError::new(
            "DATA_QUALITY_EVALUATION_STAGE_CONFLICT",
            ErrorCategory::Conflict,
            false,
            "The Party evaluation job changed before staging completed.",
        )
        .with_internal_reference(message),
        other => SdkError::new(
            "DATA_QUALITY_EVALUATION_STAGE_FAILED",
            ErrorCategory::Internal,
            false,
            "The Party evaluation input could not be staged.",
        )
        .with_internal_reference(other.to_string()),
    }
}
