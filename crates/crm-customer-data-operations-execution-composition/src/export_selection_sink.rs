use crm_capability_adapters::semantic_input_hash;
use crm_capability_plan_support as support;
use crm_capability_runtime::{CapabilityAuthorizer, CapabilityDefinition, CapabilityRequest};
use crm_core_data::{BatchError, PostgresDataStore, RecordGetQuery, TransactionalAggregatePlanner};
use crm_customer_data_operations::{
    ExportJobId, PartyExportJob, PartyExportSelectionProgress, PartyExportSelectionSummary,
};
use crm_customer_data_operations_capability_adapter::{
    INTERNAL_COMMIT_PARTY_EXPORT_SELECTION_PAGE_CAPABILITY,
    INTERNAL_COMMIT_PARTY_EXPORT_SELECTION_PAGE_REQUEST_SCHEMA,
    INTERNAL_FINALIZE_PARTY_EXPORT_SELECTION_CAPABILITY,
    INTERNAL_FINALIZE_PARTY_EXPORT_SELECTION_REQUEST_SCHEMA, MODULE_ID,
    PartyExportSelectionOutcomePlanner, internal_export_selection_capability_definition,
};
use crm_module_sdk::{
    BusinessTransactionId, DataClass, ErrorCategory, IdempotencyKey, ModuleExecutionContext,
    PortFuture, SchemaVersion, SdkError, TypedPayload,
};
use crm_proto_contracts::crm::customer_data_operations::v1 as wire;
use std::sync::Arc;

#[derive(Clone)]
pub struct PostgresPartyExportSelectionSink {
    store: PostgresDataStore,
    authorizer: Arc<dyn CapabilityAuthorizer>,
}

impl std::fmt::Debug for PostgresPartyExportSelectionSink {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PostgresPartyExportSelectionSink")
            .field("store", &self.store)
            .field("authorizer", &"dyn CapabilityAuthorizer")
            .finish()
    }
}

impl PostgresPartyExportSelectionSink {
    pub fn new(store: PostgresDataStore, authorizer: Arc<dyn CapabilityAuthorizer>) -> Self {
        Self { store, authorizer }
    }

    pub fn commit_page<'a>(
        &'a self,
        context: &'a ModuleExecutionContext,
        job_id: &'a ExportJobId,
        progress: &'a PartyExportSelectionProgress,
        candidates: Vec<wire::PartyExportSelectionCandidate>,
        source_after: Option<wire::PartyExportSourceContinuation>,
    ) -> PortFuture<'a, Result<(), SdkError>> {
        Box::pin(async move {
            let source_exhausted = source_after.is_none();
            let command = wire::InternalCommitPartyExportSelectionPageRequest {
                export_job_ref: Some(wire::ExportJobRef {
                    export_job_id: job_id.as_str().to_owned(),
                }),
                expected_progress_version: progress.version(),
                candidates,
                source_after,
                source_exhausted,
            };
            let prepared = prepare_internal_request(
                context,
                INTERNAL_COMMIT_PARTY_EXPORT_SELECTION_PAGE_CAPABILITY,
                INTERNAL_COMMIT_PARTY_EXPORT_SELECTION_PAGE_REQUEST_SCHEMA,
                &command,
            )?;
            self.authorize_plan_and_execute(prepared).await
        })
    }

    pub fn finalize<'a>(
        &'a self,
        context: &'a ModuleExecutionContext,
        job: &'a PartyExportJob,
        progress: &'a PartyExportSelectionProgress,
        summary: &'a PartyExportSelectionSummary,
    ) -> PortFuture<'a, Result<(), SdkError>> {
        Box::pin(async move {
            let command = wire::InternalFinalizePartyExportSelectionRequest {
                export_job_ref: Some(wire::ExportJobRef {
                    export_job_id: job.job_id().as_str().to_owned(),
                }),
                expected_job_version: job.version(),
                expected_progress_version: progress.version(),
                manifest_sha256: decode_sha256(summary.manifest_sha256())?.to_vec(),
                selected_resources: summary.selected_resources(),
            };
            let prepared = prepare_internal_request(
                context,
                INTERNAL_FINALIZE_PARTY_EXPORT_SELECTION_CAPABILITY,
                INTERNAL_FINALIZE_PARTY_EXPORT_SELECTION_REQUEST_SCHEMA,
                &command,
            )?;
            self.authorize_plan_and_execute(prepared).await
        })
    }

    async fn authorize_plan_and_execute(
        &self,
        prepared: PreparedInternalSelectionRequest,
    ) -> Result<(), SdkError> {
        let planner = PartyExportSelectionOutcomePlanner;
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
                "CUSTOMER_DATA_EXPORT_SELECTION_OUTCOME_PERMISSION_DENIED",
                ErrorCategory::Authorization,
                false,
                "The export selection worker is not authorized to persist this outcome.",
            )
            .with_internal_reference(format!(
                "decision_id={} reason_code={} policy_version={}",
                decision.decision_id, decision.reason_code, decision.policy_version
            )));
        }

        // Live authorization above is intentionally the final awaited policy decision before the
        // transactional export-owned side-effect boundary below.
        self.store
            .execute_batch(&plan.batch)
            .await
            .map(|_| ())
            .map_err(batch_error_to_sdk)
    }
}

struct PreparedInternalSelectionRequest {
    definition: CapabilityDefinition,
    request: CapabilityRequest,
}

fn prepare_internal_request<M: prost::Message>(
    base_context: &ModuleExecutionContext,
    capability_id: &'static str,
    schema_id: &'static str,
    command: &M,
) -> Result<PreparedInternalSelectionRequest, SdkError> {
    let definition = internal_export_selection_capability_definition(capability_id)?;
    let input = support::protobuf_payload(MODULE_ID, schema_id, DataClass::Personal, command)?;
    let input_hash = semantic_input_hash(&input);
    let request = internal_request(base_context, &definition, input, input_hash)?;
    Ok(PreparedInternalSelectionRequest {
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
    let outcome_identity = format!("cdo-export-selection-{}", hex(&input_hash));
    context.execution.idempotency_key =
        IdempotencyKey::try_new(outcome_identity.clone()).map_err(configuration_error)?;
    context.execution.business_transaction_id =
        BusinessTransactionId::try_new(outcome_identity).map_err(configuration_error)?;
    Ok(CapabilityRequest {
        context,
        input,
        input_hash,
        approval: None,
    })
}

fn decode_sha256(value: &str) -> Result<[u8; 32], SdkError> {
    if value.len() != 64 {
        return Err(selection_state_invalid());
    }
    let mut output = [0_u8; 32];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        let pair = std::str::from_utf8(pair).map_err(|_| selection_state_invalid())?;
        output[index] = u8::from_str_radix(pair, 16).map_err(|_| selection_state_invalid())?;
    }
    Ok(output)
}

fn batch_error_to_sdk(error: BatchError) -> SdkError {
    match error {
        BatchError::Sdk(error) => error,
        BatchError::Conflict(message) => SdkError::new(
            "CUSTOMER_DATA_EXPORT_SELECTION_OUTCOME_CONFLICT",
            ErrorCategory::Conflict,
            false,
            "The export selection outcome conflicted with newer state.",
        )
        .with_internal_reference(message),
        BatchError::IdempotencyKeyReused => SdkError::new(
            "CUSTOMER_DATA_EXPORT_SELECTION_OUTCOME_IDEMPOTENCY_CONFLICT",
            ErrorCategory::Conflict,
            false,
            "The export selection outcome idempotency key was reused for different input.",
        ),
        BatchError::IdempotencyInProgress => SdkError::new(
            "CUSTOMER_DATA_EXPORT_SELECTION_OUTCOME_IN_PROGRESS",
            ErrorCategory::Conflict,
            true,
            "The export selection outcome is already being committed.",
        ),
        BatchError::Database(error) => SdkError::new(
            "CUSTOMER_DATA_EXPORT_SELECTION_OUTCOME_STORAGE_UNAVAILABLE",
            ErrorCategory::Unavailable,
            true,
            "The export selection outcome could not be persisted temporarily.",
        )
        .with_internal_reference(error.to_string()),
        BatchError::InvalidPlan(message) => SdkError::new(
            "CUSTOMER_DATA_EXPORT_SELECTION_OUTCOME_PLAN_INVALID",
            ErrorCategory::Internal,
            false,
            "The export selection outcome could not be planned safely.",
        )
        .with_internal_reference(message),
        BatchError::InvalidStoredValue(message) => SdkError::new(
            "CUSTOMER_DATA_EXPORT_SELECTION_OUTCOME_REPLAY_INVALID",
            ErrorCategory::Unavailable,
            true,
            "Stored export selection outcome replay evidence is temporarily unavailable.",
        )
        .with_internal_reference(message),
    }
}

fn selection_state_invalid() -> SdkError {
    SdkError::new(
        "CUSTOMER_DATA_EXPORT_SELECTION_OUTCOME_STATE_INVALID",
        ErrorCategory::Internal,
        false,
        "The export selection outcome state is invalid.",
    )
}

fn configuration_error(error: crm_module_sdk::IdentifierError) -> SdkError {
    SdkError::new(
        "CUSTOMER_DATA_EXPORT_SELECTION_OUTCOME_CONFIGURATION_INVALID",
        ErrorCategory::Internal,
        false,
        "The export selection outcome sink is not configured safely.",
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
    fn private_selection_capabilities_remain_outside_public_mutation_catalog() {
        for capability_id in [
            INTERNAL_COMMIT_PARTY_EXPORT_SELECTION_PAGE_CAPABILITY,
            INTERNAL_FINALIZE_PARTY_EXPORT_SELECTION_CAPABILITY,
        ] {
            assert!(!MUTATION_CAPABILITY_IDS.contains(&capability_id));
        }
    }

    #[test]
    fn strict_sha256_decoder_accepts_only_exact_hex_digest() {
        assert_eq!(decode_sha256(&"ab".repeat(32)).unwrap(), [0xAB; 32]);
        assert!(decode_sha256(&"ab".repeat(31)).is_err());
        assert!(decode_sha256(&"zz".repeat(32)).is_err());
    }
}
