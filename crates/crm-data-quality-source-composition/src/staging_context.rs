use crm_capability_plan_support as support;
use crm_capability_runtime::{CapabilityDefinition, CapabilityRequest};
use crm_module_sdk::{
    BusinessTransactionId, IdempotencyKey, ModuleExecutionContext, SchemaVersion, SdkError,
    TypedPayload,
};

pub(crate) fn bind_stage_request(
    base_context: &ModuleExecutionContext,
    definition: &CapabilityDefinition,
    input: TypedPayload,
    input_hash: [u8; 32],
) -> Result<CapabilityRequest, SdkError> {
    base_context.validate()?;
    let mut context = base_context.clone();
    context.module_id = definition.owner_module_id.clone();
    context.execution.capability_id = definition.capability_id.clone();
    context.execution.capability_version = definition.capability_version.clone();
    context.execution.schema_version =
        SchemaVersion::try_new(support::CONTRACT_VERSION).map_err(configuration_error)?;
    let identity = format!("dq-evaluation-stage-{}", hex(&input_hash));
    context.execution.idempotency_key =
        IdempotencyKey::try_new(identity.clone()).map_err(configuration_error)?;
    context.execution.business_transaction_id =
        BusinessTransactionId::try_new(identity).map_err(configuration_error)?;
    Ok(CapabilityRequest {
        context,
        input,
        input_hash,
        approval: None,
    })
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(DIGITS[(byte >> 4) as usize] as char);
        output.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    output
}

fn configuration_error(error: crm_module_sdk::IdentifierError) -> SdkError {
    crm_module_sdk::SdkError::new(
        "DATA_QUALITY_EVALUATION_STAGE_CONFIGURATION_INVALID",
        crm_module_sdk::ErrorCategory::Internal,
        false,
        "The Data Quality evaluation stage is not configured safely.",
    )
    .with_internal_reference(error.to_string())
}
