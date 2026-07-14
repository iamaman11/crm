use crate::{
    ImportExecutionOutcomePlan, ImportExecutionOutcomeSink, PlannedImportJobUpdate,
    PlannedImportRowUpdate, plan_completion, plan_retryable_failure, plan_skip_invalid,
    plan_success,
};
use crm_capability_adapters::semantic_input_hash;
use crm_capability_plan_support::{self as support, EventSpec};
use crm_capability_runtime::{
    CapabilityAuthorizer, CapabilityDefinition, CapabilityRequest, CapabilityRisk,
};
use crm_core_data::{BatchError, BatchMutationPlan, PostgresDataStore, RecordMutation};
use crm_customer_data_operations::{
    ImportJob, ImportRow, ImportRowId, TargetPartyId, encode_import_job_state,
    encode_import_row_state,
};
use crm_customer_data_operations_capability_adapter::{
    IMPORT_JOB_RECORD_TYPE, IMPORT_ROW_RECORD_TYPE, MODULE_ID,
    PARTY_IMPORT_CHECKPOINT_ADVANCED_EVENT_SCHEMA, PARTY_IMPORT_CHECKPOINT_ADVANCED_EVENT_TYPE,
    PARTY_IMPORT_COMPLETED_EVENT_SCHEMA, PARTY_IMPORT_COMPLETED_EVENT_TYPE,
    PARTY_IMPORT_ROW_FAILED_EVENT_SCHEMA, PARTY_IMPORT_ROW_FAILED_EVENT_TYPE,
    PARTY_IMPORT_ROW_SUCCEEDED_EVENT_SCHEMA, PARTY_IMPORT_ROW_SUCCEEDED_EVENT_TYPE,
    import_job_persisted_contract, import_row_persisted_contract, import_row_to_wire, job_to_wire,
};
use crm_module_sdk::{
    CapabilityId, CapabilityVersion, DataClass, ErrorCategory, IdempotencyKey,
    ModuleExecutionContext, ModuleId, PortFuture, RecordRef, SchemaVersion, SdkError, TypedPayload,
};
use crm_proto_contracts::crm::{
    customer::v1 as customer, customer_data_operations::v1 as wire,
    customer_data_operations_internal::v1 as internal_wire,
};
use std::fmt;
use std::sync::Arc;

pub const INTERNAL_SKIP_INVALID_CAPABILITY: &str =
    "customer_data.import.party.internal.skip_invalid";
pub const INTERNAL_RECORD_SUCCESS_CAPABILITY: &str =
    "customer_data.import.party.internal.record_success";
pub const INTERNAL_RECORD_RETRYABLE_FAILURE_CAPABILITY: &str =
    "customer_data.import.party.internal.record_retryable_failure";
pub const INTERNAL_COMPLETE_CAPABILITY: &str = "customer_data.import.party.internal.complete";

pub const INTERNAL_SKIP_INVALID_REQUEST_SCHEMA: &str =
    "crm.customer_data_operations_internal.v1.CommitPartyImportInvalidSkipRequest";
pub const INTERNAL_RECORD_SUCCESS_REQUEST_SCHEMA: &str =
    "crm.customer_data_operations_internal.v1.CommitPartyImportSuccessRequest";
pub const INTERNAL_RECORD_RETRYABLE_FAILURE_REQUEST_SCHEMA: &str =
    "crm.customer_data_operations_internal.v1.RecordPartyImportRetryableFailureRequest";
pub const INTERNAL_COMPLETE_REQUEST_SCHEMA: &str =
    "crm.customer_data_operations_internal.v1.CompletePartyImportExecutionRequest";

pub const INTERNAL_OUTCOME_CAPABILITY_IDS: [&str; 4] = [
    INTERNAL_SKIP_INVALID_CAPABILITY,
    INTERNAL_RECORD_SUCCESS_CAPABILITY,
    INTERNAL_RECORD_RETRYABLE_FAILURE_CAPABILITY,
    INTERNAL_COMPLETE_CAPABILITY,
];

#[derive(Clone)]
pub struct PostgresImportExecutionOutcomeSink {
    store: PostgresDataStore,
    authorizer: Arc<dyn CapabilityAuthorizer>,
}

impl PostgresImportExecutionOutcomeSink {
    pub fn new(store: PostgresDataStore, authorizer: Arc<dyn CapabilityAuthorizer>) -> Self {
        Self { store, authorizer }
    }
}

impl fmt::Debug for PostgresImportExecutionOutcomeSink {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PostgresImportExecutionOutcomeSink")
            .field("store", &self.store)
            .field("authorizer", &"dyn CapabilityAuthorizer")
            .finish()
    }
}

impl ImportExecutionOutcomeSink for PostgresImportExecutionOutcomeSink {
    fn skip_invalid<'a>(
        &'a self,
        context: &'a ModuleExecutionContext,
        job: &'a ImportJob,
        row: &'a ImportRow,
    ) -> PortFuture<'a, Result<(), SdkError>> {
        Box::pin(async move {
            let command = internal_wire::CommitPartyImportInvalidSkipRequest {
                import_job_ref: Some(job_ref(job)),
                expected_job_version: job.version(),
                import_row_ref: Some(row_ref(row)),
                row_position: row.row_position(),
            };
            let prepared = prepare_outcome_commit(
                context,
                INTERNAL_SKIP_INVALID_CAPABILITY,
                INTERNAL_SKIP_INVALID_REQUEST_SCHEMA,
                &command,
                plan_skip_invalid(job, row, context.execution.request_started_at_unix_nanos)?,
            )?;
            self.authorize_and_execute(prepared).await
        })
    }

    fn record_success<'a>(
        &'a self,
        context: &'a ModuleExecutionContext,
        job: &'a ImportJob,
        row: &'a ImportRow,
        target_party_id: &'a TargetPartyId,
    ) -> PortFuture<'a, Result<(), SdkError>> {
        Box::pin(async move {
            let command = internal_wire::CommitPartyImportSuccessRequest {
                import_job_ref: Some(job_ref(job)),
                expected_job_version: job.version(),
                import_row_ref: Some(row_ref(row)),
                expected_row_version: row.version(),
                target_party_ref: Some(customer::PartyRef {
                    party_id: target_party_id.as_str().to_owned(),
                }),
                row_position: row.row_position(),
            };
            let prepared = prepare_outcome_commit(
                context,
                INTERNAL_RECORD_SUCCESS_CAPABILITY,
                INTERNAL_RECORD_SUCCESS_REQUEST_SCHEMA,
                &command,
                plan_success(
                    job,
                    row,
                    target_party_id.clone(),
                    context.execution.request_started_at_unix_nanos,
                )?,
            )?;
            self.authorize_and_execute(prepared).await
        })
    }

    fn record_retryable_failure<'a>(
        &'a self,
        context: &'a ModuleExecutionContext,
        job: &'a ImportJob,
        row: &'a ImportRow,
        error_code: &'a str,
    ) -> PortFuture<'a, Result<(), SdkError>> {
        Box::pin(async move {
            let command = internal_wire::RecordPartyImportRetryableFailureRequest {
                import_job_ref: Some(job_ref(job)),
                import_row_ref: Some(row_ref(row)),
                expected_row_version: row.version(),
                error_code: error_code.to_owned(),
            };
            let prepared = prepare_outcome_commit(
                context,
                INTERNAL_RECORD_RETRYABLE_FAILURE_CAPABILITY,
                INTERNAL_RECORD_RETRYABLE_FAILURE_REQUEST_SCHEMA,
                &command,
                plan_retryable_failure(
                    job,
                    row,
                    error_code.to_owned(),
                    context.execution.request_started_at_unix_nanos,
                )?,
            )?;
            self.authorize_and_execute(prepared).await
        })
    }

    fn complete<'a>(
        &'a self,
        context: &'a ModuleExecutionContext,
        job: &'a ImportJob,
    ) -> PortFuture<'a, Result<(), SdkError>> {
        Box::pin(async move {
            let command = internal_wire::CompletePartyImportExecutionRequest {
                import_job_ref: Some(job_ref(job)),
                expected_job_version: job.version(),
            };
            let prepared = prepare_outcome_commit(
                context,
                INTERNAL_COMPLETE_CAPABILITY,
                INTERNAL_COMPLETE_REQUEST_SCHEMA,
                &command,
                plan_completion(job, context.execution.request_started_at_unix_nanos)?,
            )?;
            self.authorize_and_execute(prepared).await
        })
    }
}

impl PostgresImportExecutionOutcomeSink {
    async fn authorize_and_execute(&self, prepared: PreparedOutcomeCommit) -> Result<(), SdkError> {
        let decision = self
            .authorizer
            .authorize(&prepared.definition, &prepared.request)
            .await?;
        if !decision.allowed {
            return Err(SdkError::new(
                "CUSTOMER_DATA_IMPORT_EXECUTION_OUTCOME_PERMISSION_DENIED",
                ErrorCategory::Authorization,
                false,
                "The import execution worker is not authorized to persist this outcome.",
            )
            .with_internal_reference(format!(
                "decision_id={} reason_code={} policy_version={}",
                decision.decision_id, decision.reason_code, decision.policy_version
            )));
        }

        // Live authorization above is intentionally the final awaited decision before the
        // transactional import-owned side-effect boundary below.
        self.store
            .execute_batch(&prepared.batch)
            .await
            .map(|_| ())
            .map_err(batch_error_to_sdk)
    }
}

struct PreparedOutcomeCommit {
    definition: CapabilityDefinition,
    request: CapabilityRequest,
    batch: BatchMutationPlan,
}

fn prepare_outcome_commit<M: prost::Message>(
    base_context: &ModuleExecutionContext,
    capability_id: &'static str,
    schema_id: &'static str,
    command: &M,
    plan: ImportExecutionOutcomePlan,
) -> Result<PreparedOutcomeCommit, SdkError> {
    let definition = internal_capability_definition(capability_id)?;
    let input = support::protobuf_payload(MODULE_ID, schema_id, DataClass::Personal, command)?;
    let input_hash = semantic_input_hash(&input);
    let request = internal_request(base_context, &definition, input, input_hash)?;
    let batch = batch_from_plan(&definition, &request, plan)?;
    Ok(PreparedOutcomeCommit {
        definition,
        request,
        batch,
    })
}

pub fn internal_capability_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {
    INTERNAL_OUTCOME_CAPABILITY_IDS
        .into_iter()
        .map(internal_capability_definition)
        .collect()
}

pub fn internal_capability_definition(
    capability_id: &str,
) -> Result<CapabilityDefinition, SdkError> {
    let schema_id = match capability_id {
        INTERNAL_SKIP_INVALID_CAPABILITY => INTERNAL_SKIP_INVALID_REQUEST_SCHEMA,
        INTERNAL_RECORD_SUCCESS_CAPABILITY => INTERNAL_RECORD_SUCCESS_REQUEST_SCHEMA,
        INTERNAL_RECORD_RETRYABLE_FAILURE_CAPABILITY => {
            INTERNAL_RECORD_RETRYABLE_FAILURE_REQUEST_SCHEMA
        }
        INTERNAL_COMPLETE_CAPABILITY => INTERNAL_COMPLETE_REQUEST_SCHEMA,
        _ => {
            return Err(SdkError::new(
                "CUSTOMER_DATA_IMPORT_EXECUTION_INTERNAL_CAPABILITY_UNSUPPORTED",
                ErrorCategory::Internal,
                false,
                "The import execution internal capability is not configured.",
            ));
        }
    };
    Ok(CapabilityDefinition {
        capability_id: CapabilityId::try_new(capability_id).map_err(configuration_error)?,
        capability_version: CapabilityVersion::try_new(support::CONTRACT_VERSION)
            .map_err(configuration_error)?,
        owner_module_id: ModuleId::try_new(MODULE_ID).map_err(configuration_error)?,
        input_contract: support::protobuf_contract(
            MODULE_ID,
            schema_id,
            vec![DataClass::Personal],
        )?,
        output_contract: None,
        risk: CapabilityRisk::Medium,
        mutation: true,
        requires_idempotency: true,
        requires_approval: false,
        authorization_policy_id: capability_id.to_owned(),
        rate_limit_policy_id: None,
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
    context.execution.idempotency_key =
        IdempotencyKey::try_new(format!("cdo-outcome-{}", hex(&input_hash)))
            .map_err(configuration_error)?;
    Ok(CapabilityRequest {
        context,
        input,
        input_hash,
        approval: None,
    })
}

fn batch_from_plan(
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
    plan: ImportExecutionOutcomePlan,
) -> Result<BatchMutationPlan, SdkError> {
    match plan {
        ImportExecutionOutcomePlan::SkippedInvalid {
            job,
            row_id,
            row_position: _,
        } => skipped_invalid_batch(definition, request, &job, &row_id),
        ImportExecutionOutcomePlan::Succeeded {
            job,
            row,
            target_party_id: _,
        } => success_batch(definition, request, &job, &row),
        ImportExecutionOutcomePlan::RetryableFailure { row, error_code: _ } => {
            retryable_failure_batch(definition, request, &row)
        }
        ImportExecutionOutcomePlan::Completed { job } => {
            completion_batch(definition, request, &job)
        }
    }
}

fn skipped_invalid_batch(
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
    job: &PlannedImportJobUpdate,
    row_id: &ImportRowId,
) -> Result<BatchMutationPlan, SdkError> {
    let job_ref = job_record_ref(job.after())?;
    let event = support::event_evidence_with_data_class(
        request,
        job_ref.clone(),
        MODULE_ID,
        EventSpec {
            event_type: PARTY_IMPORT_CHECKPOINT_ADVANCED_EVENT_TYPE,
            event_schema_id: PARTY_IMPORT_CHECKPOINT_ADVANCED_EVENT_SCHEMA,
            aggregate_version: job.after().version(),
            previous_version: Some(job.expected_version()),
        },
        DataClass::Personal,
        &wire::PartyImportCheckpointAdvancedEvent {
            import_job: Some(job_to_wire(job.after())?),
            import_row_ref: Some(wire::ImportRowRef {
                import_job_ref: Some(job_ref_wire(job.after())),
                import_row_id: row_id.as_str().to_owned(),
            }),
            skipped_invalid: true,
        },
    )?;
    Ok(BatchMutationPlan {
        context: request.context.clone(),
        records: vec![RecordMutation::Update {
            reference: job_ref.clone(),
            expected_version: job.expected_version(),
            payload: job_payload(job.after())?,
        }],
        relationships: Vec::new(),
        audits: vec![support::audit_intent(
            request,
            &job_ref,
            job.after().version(),
            definition.capability_id.as_str(),
            &event.event.payload.bytes,
        )?],
        idempotency: support::capability_idempotency(definition, request)?,
        events: vec![event],
    })
}

fn success_batch(
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
    job: &PlannedImportJobUpdate,
    row: &PlannedImportRowUpdate,
) -> Result<BatchMutationPlan, SdkError> {
    let job_ref = job_record_ref(job.after())?;
    let row_ref = row_record_ref(row.after())?;
    let row_wire = import_row_to_wire(row.after())?;
    let row_event = support::event_evidence_with_data_class(
        request,
        row_ref.clone(),
        MODULE_ID,
        EventSpec {
            event_type: PARTY_IMPORT_ROW_SUCCEEDED_EVENT_TYPE,
            event_schema_id: PARTY_IMPORT_ROW_SUCCEEDED_EVENT_SCHEMA,
            aggregate_version: row.after().version(),
            previous_version: Some(row.expected_version()),
        },
        DataClass::Personal,
        &wire::PartyImportRowSucceededEvent {
            import_row: Some(row_wire.clone()),
        },
    )?;
    let checkpoint_event = support::event_evidence_with_data_class(
        request,
        job_ref.clone(),
        MODULE_ID,
        EventSpec {
            event_type: PARTY_IMPORT_CHECKPOINT_ADVANCED_EVENT_TYPE,
            event_schema_id: PARTY_IMPORT_CHECKPOINT_ADVANCED_EVENT_SCHEMA,
            aggregate_version: job.after().version(),
            previous_version: Some(job.expected_version()),
        },
        DataClass::Personal,
        &wire::PartyImportCheckpointAdvancedEvent {
            import_job: Some(job_to_wire(job.after())?),
            import_row_ref: row_wire.import_row_ref,
            skipped_invalid: false,
        },
    )?;
    Ok(BatchMutationPlan {
        context: request.context.clone(),
        records: vec![
            RecordMutation::Update {
                reference: row_ref.clone(),
                expected_version: row.expected_version(),
                payload: row_payload(row.after())?,
            },
            RecordMutation::Update {
                reference: job_ref.clone(),
                expected_version: job.expected_version(),
                payload: job_payload(job.after())?,
            },
        ],
        relationships: Vec::new(),
        audits: vec![
            support::audit_intent(
                request,
                &row_ref,
                row.after().version(),
                definition.capability_id.as_str(),
                &row_event.event.payload.bytes,
            )?,
            support::audit_intent(
                request,
                &job_ref,
                job.after().version(),
                definition.capability_id.as_str(),
                &checkpoint_event.event.payload.bytes,
            )?,
        ],
        idempotency: support::capability_idempotency(definition, request)?,
        events: vec![row_event, checkpoint_event],
    })
}

fn retryable_failure_batch(
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
    row: &PlannedImportRowUpdate,
) -> Result<BatchMutationPlan, SdkError> {
    let row_ref = row_record_ref(row.after())?;
    let event = support::event_evidence_with_data_class(
        request,
        row_ref.clone(),
        MODULE_ID,
        EventSpec {
            event_type: PARTY_IMPORT_ROW_FAILED_EVENT_TYPE,
            event_schema_id: PARTY_IMPORT_ROW_FAILED_EVENT_SCHEMA,
            aggregate_version: row.after().version(),
            previous_version: Some(row.expected_version()),
        },
        DataClass::Personal,
        &wire::PartyImportRowFailedEvent {
            import_row: Some(import_row_to_wire(row.after())?),
        },
    )?;
    Ok(BatchMutationPlan {
        context: request.context.clone(),
        records: vec![RecordMutation::Update {
            reference: row_ref.clone(),
            expected_version: row.expected_version(),
            payload: row_payload(row.after())?,
        }],
        relationships: Vec::new(),
        audits: vec![support::audit_intent(
            request,
            &row_ref,
            row.after().version(),
            definition.capability_id.as_str(),
            &event.event.payload.bytes,
        )?],
        idempotency: support::capability_idempotency(definition, request)?,
        events: vec![event],
    })
}

fn completion_batch(
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
    job: &PlannedImportJobUpdate,
) -> Result<BatchMutationPlan, SdkError> {
    let job_ref = job_record_ref(job.after())?;
    let event = support::event_evidence_with_data_class(
        request,
        job_ref.clone(),
        MODULE_ID,
        EventSpec {
            event_type: PARTY_IMPORT_COMPLETED_EVENT_TYPE,
            event_schema_id: PARTY_IMPORT_COMPLETED_EVENT_SCHEMA,
            aggregate_version: job.after().version(),
            previous_version: Some(job.expected_version()),
        },
        DataClass::Personal,
        &wire::PartyImportCompletedEvent {
            import_job: Some(job_to_wire(job.after())?),
        },
    )?;
    Ok(BatchMutationPlan {
        context: request.context.clone(),
        records: vec![RecordMutation::Update {
            reference: job_ref.clone(),
            expected_version: job.expected_version(),
            payload: job_payload(job.after())?,
        }],
        relationships: Vec::new(),
        audits: vec![support::audit_intent(
            request,
            &job_ref,
            job.after().version(),
            definition.capability_id.as_str(),
            &event.event.payload.bytes,
        )?],
        idempotency: support::capability_idempotency(definition, request)?,
        events: vec![event],
    })
}

fn job_payload(job: &ImportJob) -> Result<TypedPayload, SdkError> {
    support::persisted_json_payload_with_data_class(
        import_job_persisted_contract(),
        DataClass::Personal,
        encode_import_job_state(job)?,
    )
}

fn row_payload(row: &ImportRow) -> Result<TypedPayload, SdkError> {
    support::persisted_json_payload_with_data_class(
        import_row_persisted_contract(),
        DataClass::Personal,
        encode_import_row_state(row)?,
    )
}

fn job_record_ref(job: &ImportJob) -> Result<RecordRef, SdkError> {
    support::record_ref(
        IMPORT_JOB_RECORD_TYPE,
        job.job_id().as_str(),
        "customer_data.import_job_ref.import_job_id",
    )
}

fn row_record_ref(row: &ImportRow) -> Result<RecordRef, SdkError> {
    support::record_ref(
        IMPORT_ROW_RECORD_TYPE,
        row.row_id().as_str(),
        "customer_data.import_row_ref.import_row_id",
    )
}

fn job_ref(job: &ImportJob) -> wire::ImportJobRef {
    job_ref_wire(job)
}

fn job_ref_wire(job: &ImportJob) -> wire::ImportJobRef {
    wire::ImportJobRef {
        import_job_id: job.job_id().as_str().to_owned(),
    }
}

fn row_ref(row: &ImportRow) -> wire::ImportRowRef {
    wire::ImportRowRef {
        import_job_ref: Some(wire::ImportJobRef {
            import_job_id: row.job_id().as_str().to_owned(),
        }),
        import_row_id: row.row_id().as_str().to_owned(),
    }
}

fn batch_error_to_sdk(error: BatchError) -> SdkError {
    match error {
        BatchError::Sdk(error) => error,
        BatchError::Conflict(message) => SdkError::new(
            "CUSTOMER_DATA_IMPORT_EXECUTION_OUTCOME_CONFLICT",
            ErrorCategory::Conflict,
            false,
            "The import execution outcome conflicted with newer state.",
        )
        .with_internal_reference(message),
        BatchError::IdempotencyKeyReused => SdkError::new(
            "CUSTOMER_DATA_IMPORT_EXECUTION_OUTCOME_IDEMPOTENCY_CONFLICT",
            ErrorCategory::Conflict,
            false,
            "The import execution outcome idempotency key was reused for different input.",
        ),
        BatchError::IdempotencyInProgress => SdkError::new(
            "CUSTOMER_DATA_IMPORT_EXECUTION_OUTCOME_IN_PROGRESS",
            ErrorCategory::Conflict,
            true,
            "The import execution outcome is already being committed.",
        ),
        BatchError::Database(error) => SdkError::new(
            "CUSTOMER_DATA_IMPORT_EXECUTION_OUTCOME_STORAGE_UNAVAILABLE",
            ErrorCategory::Unavailable,
            true,
            "The import execution outcome could not be persisted temporarily.",
        )
        .with_internal_reference(error.to_string()),
        BatchError::InvalidPlan(message) => SdkError::new(
            "CUSTOMER_DATA_IMPORT_EXECUTION_OUTCOME_PLAN_INVALID",
            ErrorCategory::Internal,
            false,
            "The import execution outcome could not be planned safely.",
        )
        .with_internal_reference(message),
        BatchError::InvalidStoredValue(message) => SdkError::new(
            "CUSTOMER_DATA_IMPORT_EXECUTION_OUTCOME_REPLAY_INVALID",
            ErrorCategory::Unavailable,
            true,
            "Stored import execution outcome replay evidence is temporarily unavailable.",
        )
        .with_internal_reference(message),
    }
}

fn configuration_error(error: crm_module_sdk::IdentifierError) -> SdkError {
    SdkError::new(
        "CUSTOMER_DATA_IMPORT_EXECUTION_OUTCOME_CONFIGURATION_INVALID",
        ErrorCategory::Internal,
        false,
        "The import execution outcome sink is not configured safely.",
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
    fn internal_outcome_capabilities_are_typed_idempotent_and_not_public_mutation_coordinates() {
        let definitions = internal_capability_definitions().unwrap();
        assert_eq!(definitions.len(), INTERNAL_OUTCOME_CAPABILITY_IDS.len());
        for definition in definitions {
            assert!(definition.mutation);
            assert!(definition.requires_idempotency);
            assert!(!definition.requires_approval);
            assert_eq!(definition.owner_module_id.as_str(), MODULE_ID);
            assert_eq!(
                definition.input_contract.allowed_data_classes,
                vec![DataClass::Personal]
            );
            assert!(!MUTATION_CAPABILITY_IDS.contains(&definition.capability_id.as_str()));
        }
    }
}
