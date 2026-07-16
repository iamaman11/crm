use crm_capability_adapters::semantic_input_hash;
use crm_capability_plan_support as support;
use crm_capability_runtime::{
    CapabilityAuthorizer, CapabilityDefinition, CapabilityRequest, TransactionalCapabilityExecutor,
};
use crm_core_data::{PostgresDataStore, PostgresTransactionalAggregateExecutor};
use crm_data_quality::{
    PartyCompletenessProfileVersion, PartyEvaluationInputSnapshot, PartyEvaluationJob,
    PartyRuleSetVersion,
};
use crm_data_quality_capability_adapter::{
    MATERIALIZE_PARTY_EVALUATION_REQUEST_SCHEMA, MODULE_ID,
    DataQualityEvaluationMaterializationPlanner, evaluation_materialization_capability_definition,
};
use crm_module_sdk::{
    BusinessTransactionId, DataClass, ErrorCategory, IdempotencyKey, ModuleExecutionContext,
    PortFuture, SchemaVersion, SdkError, TypedPayload,
};
use crm_proto_contracts::crm::data_quality::v1 as wire;
use std::sync::Arc;

#[derive(Clone)]
pub struct PostgresPartyEvaluationMaterializationSink {
    store: PostgresDataStore,
    authorizer: Arc<dyn CapabilityAuthorizer>,
}

impl PostgresPartyEvaluationMaterializationSink {
    pub fn new(store: PostgresDataStore, authorizer: Arc<dyn CapabilityAuthorizer>) -> Self {
        Self { store, authorizer }
    }

    pub fn materialize<'a>(
        &'a self,
        base_context: &'a ModuleExecutionContext,
        job: &'a PartyEvaluationJob,
        expected_job_version: i64,
        rule_set: PartyRuleSetVersion,
        profile: PartyCompletenessProfileVersion,
        input: PartyEvaluationInputSnapshot,
    ) -> PortFuture<'a, Result<(), SdkError>> {
        Box::pin(async move {
            let planner = Arc::new(DataQualityEvaluationMaterializationPlanner::new(
                rule_set, profile, input,
            )?);
            let definition = evaluation_materialization_capability_definition()?;
            let command = wire::MaterializePartyEvaluationRequest {
                evaluation_job_ref: Some(wire::PartyEvaluationJobRef {
                    evaluation_job_id: job.job_id().as_str().to_owned(),
                }),
                expected_job_version,
            };
            let input = support::protobuf_payload(
                MODULE_ID,
                MATERIALIZE_PARTY_EVALUATION_REQUEST_SCHEMA,
                DataClass::Personal,
                &command,
            )?;
            let input_hash = semantic_input_hash(&input);
            let request = bind_materialization_request(base_context, &definition, input, input_hash)?;
            let decision = self.authorizer.authorize(&definition, &request).await?;
            if !decision.allowed {
                return Err(SdkError::new(
                    "DATA_QUALITY_EVALUATION_MATERIALIZATION_PERMISSION_DENIED",
                    ErrorCategory::Authorization,
                    false,
                    "The Data Quality worker is not authorized to materialize evaluation outcomes.",
                )
                .with_internal_reference(format!(
                    "decision_id={} reason_code={} policy_version={}",
                    decision.decision_id, decision.reason_code, decision.policy_version
                )));
            }
            PostgresTransactionalAggregateExecutor::new(self.store.clone(), planner)
                .execute(&definition, request)
                .await
                .map(|_| ())
        })
    }
}

impl std::fmt::Debug for PostgresPartyEvaluationMaterializationSink {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PostgresPartyEvaluationMaterializationSink")
            .field("store", &self.store)
            .field("authorizer", &"dyn CapabilityAuthorizer")
            .finish()
    }
}

fn bind_materialization_request(
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
    let identity = format!("dq-evaluation-materialize-{}", hex(&input_hash));
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
    SdkError::new(
        "DATA_QUALITY_EVALUATION_MATERIALIZATION_CONFIGURATION_INVALID",
        ErrorCategory::Internal,
        false,
        "The Data Quality evaluation materialization is not configured safely.",
    )
    .with_internal_reference(error.to_string())
}
