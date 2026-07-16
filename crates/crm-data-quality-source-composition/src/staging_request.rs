use crm_capability_adapters::semantic_input_hash;
use crm_capability_plan_support as support;
use crm_capability_runtime::{CapabilityDefinition, CapabilityRequest};
use crm_data_quality::PartyEvaluationJob;
use crm_data_quality_capability_adapter::{
    MODULE_ID, STAGE_PARTY_EVALUATION_INPUT_REQUEST_SCHEMA,
    evaluation_stage_capability_definition,
};
use crm_module_sdk::{DataClass, ModuleExecutionContext, SdkError};

use crate::{
    PartyQualitySourceSnapshot, staging_command::stage_command,
    staging_context::bind_stage_request,
};

pub(crate) struct PreparedStageRequest {
    pub definition: CapabilityDefinition,
    pub request: CapabilityRequest,
}

pub(crate) fn prepare_stage_request(
    base_context: &ModuleExecutionContext,
    job: &PartyEvaluationJob,
    expected_job_version: i64,
    source: &PartyQualitySourceSnapshot,
) -> Result<PreparedStageRequest, SdkError> {
    let command = stage_command(job, expected_job_version, source)?;
    let definition = evaluation_stage_capability_definition()?;
    let input = support::protobuf_payload(
        MODULE_ID,
        STAGE_PARTY_EVALUATION_INPUT_REQUEST_SCHEMA,
        DataClass::Personal,
        &command,
    )?;
    let input_hash = semantic_input_hash(&input);
    let request = bind_stage_request(base_context, &definition, input, input_hash)?;
    Ok(PreparedStageRequest {
        definition,
        request,
    })
}
