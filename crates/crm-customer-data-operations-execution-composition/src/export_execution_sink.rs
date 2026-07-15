use crm_capability_adapters::semantic_input_hash;
use crm_capability_plan_support as support;
use crm_capability_runtime::{CapabilityAuthorizer, CapabilityDefinition, CapabilityRequest};
use crm_core_data::{BatchError, PostgresDataStore, RecordGetQuery, TransactionalAggregatePlanner};
use crm_customer_data_operations::{
    PartyExportArtifactEvidence, PartyExportExclusionReason,
    PartyExportExecutionStage, PartyExportJob, PartyExportReconciliation,
};
use crm_customer_data_operations_capability_adapter::{
    INTERNAL_COMMIT_PARTY_EXPORT_EXECUTION_OUTCOME_CAPABILITY,
    INTERNAL_COMMIT_PARTY_EXPORT_EXECUTION_OUTCOME_REQUEST_SCHEMA,
    INTERNAL_COMPLETE_PARTY_EXPORT_EXECUTION_CAPABILITY,
    INTERNAL_COMPLETE_PARTY_EXPORT_EXECUTION_REQUEST_SCHEMA,
    INTERNAL_STAGE_PARTY_EXPORT_EXECUTION_CAPABILITY,
    INTERNAL_STAGE_PARTY_EXPORT_EXECUTION_REQUEST_SCHEMA, MODULE_ID,
    PartyExportExecutionOutcomePlanner, internal_export_execution_capability_definition,
};
use crm_module_sdk::{
    BusinessTransactionId, DataClass, ErrorCategory, IdempotencyKey, ModuleExecutionContext,
    PortFuture, SchemaVersion, SdkError, TypedPayload,
};
use crm_proto_contracts::crm::customer_data_operations::v1 as wire;
use std::sync::Arc;

#[derive(Clone)]
pub struct PostgresPartyExportExecutionSink {
    store: PostgresDataStore,
    authorizer: Arc<dyn CapabilityAuthorizer>,
}

impl std::fmt::Debug for PostgresPartyExportExecutionSink {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PostgresPartyExportExecutionSink")
            .field("store", &self.store)
            .field("authorizer", &"dyn CapabilityAuthorizer")
            .finish()
    }
}

impl PostgresPartyExportExecutionSink {
    pub fn new(store: PostgresDataStore, authorizer: Arc<dyn CapabilityAuthorizer>) -> Self {
        Self { store, authorizer }
    }

    pub fn stage<'a>(
        &'a self,
        context: &'a ModuleExecutionContext,
        stage: &'a PartyExportExecutionStage,
    ) -> PortFuture<'a, Result<(), SdkError>> {
        Box::pin(async move {
            let result = match stage.kind() {
                crm_customer_data_operations::PartyExportExecutionStageKind::Emitted {
                    row_utf8,
                    redacted_fields,
                    ..
                } => wire::internal_stage_party_export_execution_request::Result::Emitted(
                    wire::InternalStagePartyExportEmittedRow {
                        row_utf8: row_utf8.as_bytes().to_vec(),
                        redacted_fields: *redacted_fields,
                    },
                ),
                crm_customer_data_operations::PartyExportExecutionStageKind::Excluded(reason) => {
                    wire::internal_stage_party_export_execution_request::Result::ExclusionReason(
                        exclusion_reason_to_wire(*reason) as i32,
                    )
                }
            };
            let command = wire::InternalStagePartyExportExecutionRequest {
                export_job_ref: Some(wire::ExportJobRef {
                    export_job_id: stage.job_id().as_str().to_owned(),
                }),
                manifest_position: stage.manifest_position(),
                result: Some(result),
            };
            let prepared = prepare_internal_request(
                context,
                INTERNAL_STAGE_PARTY_EXPORT_EXECUTION_CAPABILITY,
                INTERNAL_STAGE_PARTY_EXPORT_EXECUTION_REQUEST_SCHEMA,
                &command,
            )?;
            self.authorize_plan_and_execute(prepared).await
        })
    }

    pub fn commit_emitted<'a>(
        &'a self,
        context: &'a ModuleExecutionContext,
        job: &'a PartyExportJob,
        manifest_position: u32,
        artifact_chunk_index: u32,
        chunk_sha256: [u8; 32],
        chunk_size_bytes: u64,
        redacted_fields: u32,
    ) -> PortFuture<'a, Result<(), SdkError>> {
        Box::pin(async move {
            let command = wire::InternalCommitPartyExportExecutionOutcomeRequest {
                export_job_ref: Some(wire::ExportJobRef {
                    export_job_id: job.job_id().as_str().to_owned(),
                }),
                expected_job_version: job.version(),
                manifest_position,
                result: Some(
                    wire::internal_commit_party_export_execution_outcome_request::Result::Emitted(
                        wire::InternalPartyExportEmittedOutcome {
                            artifact_chunk_index,
                            chunk_sha256: chunk_sha256.to_vec(),
                            chunk_size_bytes,
                            redacted_fields,
                        },
                    ),
                ),
            };
            let prepared = prepare_internal_request(
                context,
                INTERNAL_COMMIT_PARTY_EXPORT_EXECUTION_OUTCOME_CAPABILITY,
                INTERNAL_COMMIT_PARTY_EXPORT_EXECUTION_OUTCOME_REQUEST_SCHEMA,
                &command,
            )?;
            self.authorize_plan_and_execute(prepared).await
        })
    }

    pub fn commit_excluded<'a>(
        &'a self,
        context: &'a ModuleExecutionContext,
        job: &'a PartyExportJob,
        manifest_position: u32,
        reason: PartyExportExclusionReason,
    ) -> PortFuture<'a, Result<(), SdkError>> {
        Box::pin(async move {
            let command = wire::InternalCommitPartyExportExecutionOutcomeRequest {
                export_job_ref: Some(wire::ExportJobRef {
                    export_job_id: job.job_id().as_str().to_owned(),
                }),
                expected_job_version: job.version(),
                manifest_position,
                result: Some(
                    wire::internal_commit_party_export_execution_outcome_request::Result::ExclusionReason(
                        exclusion_reason_to_wire(reason) as i32,
                    ),
                ),
            };
            let prepared = prepare_internal_request(
                context,
                INTERNAL_COMMIT_PARTY_EXPORT_EXECUTION_OUTCOME_CAPABILITY,
                INTERNAL_COMMIT_PARTY_EXPORT_EXECUTION_OUTCOME_REQUEST_SCHEMA,
                &command,
            )?;
            self.authorize_plan_and_execute(prepared).await
        })
    }

    pub fn complete<'a>(
        &'a self,
        context: &'a ModuleExecutionContext,
        job: &'a PartyExportJob,
        artifact: &'a PartyExportArtifactEvidence,
        reconciliation: &'a PartyExportReconciliation,
    ) -> PortFuture<'a, Result<(), SdkError>> {
        Box::pin(async move {
            let command = wire::InternalCompletePartyExportExecutionRequest {
                export_job_ref: Some(wire::ExportJobRef {
                    export_job_id: job.job_id().as_str().to_owned(),
                }),
                expected_job_version: job.version(),
                artifact: Some(wire::PartyExportArtifact {
                    file_id: artifact.file_id().as_str().to_owned(),
                    media_type: crm_customer_data_operations::PARTY_EXPORT_CSV_MEDIA_TYPE
                        .to_owned(),
                    content_sha256: decode_sha256(artifact.content_sha256())?.to_vec(),
                    size_bytes: artifact.size_bytes(),
                    retention_policy_id: artifact.retention_policy_id().to_owned(),
                }),
                reconciliation: Some(wire::PartyExportReconciliation {
                    selected_resources: reconciliation.selected_resources(),
                    emitted_rows: reconciliation.emitted_rows(),
                    excluded_not_visible: reconciliation.excluded_not_visible(),
                    excluded_version_changed: reconciliation.excluded_version_changed(),
                    excluded_unavailable: reconciliation.excluded_unavailable(),
                    redacted_fields: reconciliation.redacted_fields(),
                }),
            };
            let prepared = prepare_internal_request(
                context,
                INTERNAL_COMPLETE_PARTY_EXPORT_EXECUTION_CAPABILITY,
                INTERNAL_COMPLETE_PARTY_EXPORT_EXECUTION_REQUEST_SCHEMA,
                &command,
            )?;
            self.authorize_plan_and_execute(prepared).await
        })
    }

    async fn authorize_plan_and_execute(
        &self,
        prepared: PreparedInternalExecutionRequest,
    ) -> Result<(), SdkError> {
        let planner = PartyExportExecutionOutcomePlanner;
        let target = planner.target(&prepared.definition, &prepared.request)?;
        let current = self
            .store
            .get_record_for_query(&RecordGetQuery {
                tenant_id: prepared.request.context.execution.tenant_id.clone(),
                owner_module_id: prepared.definition.owner_module_id.clone(),
                record_type: target.reference.record_type.clone(),
                record_id: target.reference.record_id.clone(),
            })
            .await?;
        let plan = planner.plan(&prepared.definition, &prepared.request, current.as_ref())?;
        let decision = self
            .authorizer
            .authorize(&prepared.definition, &prepared.request)
            .await?;
        if !decision.allowed {
            return Err(SdkError::new(
                "CUSTOMER_DATA_EXPORT_EXECUTION_OUTCOME_PERMISSION_DENIED",
                ErrorCategory::Authorization,
                false,
                "The export execution worker is not authorized to persist this outcome.",
            )
            .with_internal_reference(format!(
                "decision_id={} reason_code={} policy_version={}",
                decision.decision_id, decision.reason_code, decision.policy_version
            )));
        }
        self.store
            .execute_batch(&plan.batch)
            .await
            .map(|_| ())
            .map_err(batch_error_to_sdk)
    }
}

struct PreparedInternalExecutionRequest {
    definition: CapabilityDefinition,
    request: CapabilityRequest,
}

fn prepare_internal_request<M: prost::Message>(
    base_context: &ModuleExecutionContext,
    capability_id: &'static str,
    schema_id: &'static str,
    command: &M,
) -> Result<PreparedInternalExecutionRequest, SdkError> {
    let definition = internal_export_execution_capability_definition(capability_id)?;
    let input = support::protobuf_payload(MODULE_ID, schema_id, DataClass::Personal, command)?;
    let input_hash = semantic_input_hash(&input);
    let request = internal_request(base_context, &definition, input, input_hash)?;
    Ok(PreparedInternalExecutionRequest {
        definition,
        request,
    })
}

fn internal_request(
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
    let identity = format!("cdo-export-execution-{}", hex(&input_hash));
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

fn exclusion_reason_to_wire(
    reason: PartyExportExclusionReason,
) -> wire::PartyExportExecutionExclusionReason {
    match reason {
        PartyExportExclusionReason::NotVisible => {
            wire::PartyExportExecutionExclusionReason::NotVisible
        }
        PartyExportExclusionReason::VersionChanged => {
            wire::PartyExportExecutionExclusionReason::VersionChanged
        }
        PartyExportExclusionReason::Unavailable => {
            wire::PartyExportExecutionExclusionReason::Unavailable
        }
    }
}

fn decode_sha256(value: &str) -> Result<[u8; 32], SdkError> {
    if value.len() != 64 {
        return Err(execution_state_invalid());
    }
    let mut output = [0_u8; 32];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        let pair = std::str::from_utf8(pair).map_err(|_| execution_state_invalid())?;
        output[index] = u8::from_str_radix(pair, 16).map_err(|_| execution_state_invalid())?;
    }
    Ok(output)
}

fn batch_error_to_sdk(error: BatchError) -> SdkError {
    match error {
        BatchError::Sdk(error) => error,
        BatchError::Conflict(message) => SdkError::new(
            "CUSTOMER_DATA_EXPORT_EXECUTION_OUTCOME_CONFLICT",
            ErrorCategory::Conflict,
            false,
            "The export execution outcome conflicted with newer state.",
        )
        .with_internal_reference(message),
        BatchError::IdempotencyKeyReused => SdkError::new(
            "CUSTOMER_DATA_EXPORT_EXECUTION_OUTCOME_IDEMPOTENCY_CONFLICT",
            ErrorCategory::Conflict,
            false,
            "The export execution outcome idempotency key was reused for different input.",
        ),
        BatchError::IdempotencyInProgress => SdkError::new(
            "CUSTOMER_DATA_EXPORT_EXECUTION_OUTCOME_IN_PROGRESS",
            ErrorCategory::Conflict,
            true,
            "The export execution outcome is already being committed.",
        ),
        BatchError::Database(error) => SdkError::new(
            "CUSTOMER_DATA_EXPORT_EXECUTION_OUTCOME_STORAGE_UNAVAILABLE",
            ErrorCategory::Unavailable,
            true,
            "The export execution outcome could not be persisted temporarily.",
        )
        .with_internal_reference(error.to_string()),
        BatchError::InvalidPlan(message) => SdkError::new(
            "CUSTOMER_DATA_EXPORT_EXECUTION_OUTCOME_PLAN_INVALID",
            ErrorCategory::Internal,
            false,
            "The export execution outcome could not be planned safely.",
        )
        .with_internal_reference(message),
        BatchError::InvalidStoredValue(message) => SdkError::new(
            "CUSTOMER_DATA_EXPORT_EXECUTION_OUTCOME_REPLAY_INVALID",
            ErrorCategory::Unavailable,
            true,
            "Stored export execution outcome replay evidence is temporarily unavailable.",
        )
        .with_internal_reference(message),
    }
}

fn execution_state_invalid() -> SdkError {
    SdkError::new(
        "CUSTOMER_DATA_EXPORT_EXECUTION_OUTCOME_STATE_INVALID",
        ErrorCategory::Internal,
        false,
        "The export execution outcome state is invalid.",
    )
}

fn configuration_error(error: crm_module_sdk::IdentifierError) -> SdkError {
    SdkError::new(
        "CUSTOMER_DATA_EXPORT_EXECUTION_OUTCOME_CONFIGURATION_INVALID",
        ErrorCategory::Internal,
        false,
        "The export execution outcome sink is not configured safely.",
    )
    .with_internal_reference(error.to_string())
}

fn hex(bytes: &[u8; 32]) -> String {
    let mut value = String::with_capacity(64);
    for byte in bytes {
        use std::fmt::Write as _;
        write!(&mut value, "{byte:02x}").expect("writing to String cannot fail");
    }
    value
}

#[cfg(test)]
mod tests {
    use super::*;
    use crm_customer_data_operations_capability_adapter::MUTATION_CAPABILITY_IDS;

    #[test]
    fn private_execution_capabilities_remain_outside_public_mutation_catalog() {
        for capability_id in [
            INTERNAL_STAGE_PARTY_EXPORT_EXECUTION_CAPABILITY,
            INTERNAL_COMMIT_PARTY_EXPORT_EXECUTION_OUTCOME_CAPABILITY,
            INTERNAL_COMPLETE_PARTY_EXPORT_EXECUTION_CAPABILITY,
        ] {
            assert!(!MUTATION_CAPABILITY_IDS.contains(&capability_id));
        }
    }
}
