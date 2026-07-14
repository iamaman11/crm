#![forbid(unsafe_code)]

//! Governed execution composition for validated customer-data Party imports.
//!
//! The pure customer-data-operations domain owns import coordination state while
//! `crm.parties` remains the only Party owner. This composition selects the next
//! authoritative source position, invokes exact Party creation through a
//! `CapabilityClient`, and delegates import-owned outcome/checkpoint persistence
//! to a private sink. No public bulk-write or direct Party storage path exists here.

pub mod postgres_reader;
pub use postgres_reader::*;
pub mod outcome_plan;
pub use outcome_plan::*;

use crm_capability_plan_support as support;
use crm_customer_data_operations::{
    ExecutionPositionIndex, ExecutionRowReference, ImportJob, ImportJobStatus, ImportRow,
    ImportRowStatus, PartialExecutionPolicy, PartyImportKind, TargetPartyId,
};
use crm_module_sdk::{
    CapabilityClient, CapabilityId, CapabilityInvocation, CapabilityOutcome, CapabilityVersion,
    DataClass, ErrorCategory, IdempotencyKey, ModuleExecutionContext, ModuleId, PortFuture,
    SdkError,
};
use crm_parties_capability_adapter::{
    CREATE_CAPABILITY as PARTY_CREATE_CAPABILITY,
    CREATE_REQUEST_SCHEMA as PARTY_CREATE_REQUEST_SCHEMA, MODULE_ID as PARTIES_MODULE_ID,
    RECORD_TYPE as PARTY_RECORD_TYPE,
};
use crm_proto_contracts::crm::{customer::v1 as customer, parties::v1 as parties};
use std::sync::Arc;

pub const MODULE_ID: &str = "crm.customer-data-operations";
pub const CONTRACT_VERSION: &str = "1.0.0";

#[derive(Debug, Clone)]
pub struct ImportExecutionSnapshot {
    job: ImportJob,
    rows: Vec<ImportRow>,
    position_index: ExecutionPositionIndex,
}

impl ImportExecutionSnapshot {
    pub fn try_new(job: ImportJob, rows: Vec<ImportRow>) -> Result<Self, SdkError> {
        if job.status() != ImportJobStatus::Executing {
            return Err(execution_error(
                "CUSTOMER_DATA_IMPORT_EXECUTION_JOB_NOT_EXECUTING",
                ErrorCategory::Conflict,
                false,
                "Only an executing import job can be processed.",
            ));
        }
        if rows.iter().any(|row| row.job_id() != job.job_id()) {
            return Err(execution_error(
                "CUSTOMER_DATA_IMPORT_EXECUTION_ROW_JOB_MISMATCH",
                ErrorCategory::Internal,
                false,
                "Customer-data import execution state is inconsistent.",
            ));
        }
        let position_index = ExecutionPositionIndex::build(
            job.total_rows(),
            rows.iter().map(|row| {
                ExecutionRowReference::new(row.row_id().clone(), row.row_position(), row.status())
            }),
        )?;
        Ok(Self {
            job,
            rows,
            position_index,
        })
    }

    pub fn job(&self) -> &ImportJob {
        &self.job
    }

    pub fn rows(&self) -> &[ImportRow] {
        &self.rows
    }

    fn next_row(&self) -> Result<Option<&ImportRow>, SdkError> {
        let Some(next) = self
            .position_index
            .next_after_checkpoint(self.job.checkpoint_row_position())?
        else {
            return Ok(None);
        };
        self.rows
            .iter()
            .find(|row| row.row_id() == next.row_id())
            .map(Some)
            .ok_or_else(|| {
                execution_error(
                    "CUSTOMER_DATA_IMPORT_EXECUTION_ROW_REFERENCE_MISSING",
                    ErrorCategory::Internal,
                    false,
                    "Customer-data import execution state is inconsistent.",
                )
            })
    }
}

pub trait ImportExecutionOutcomeSink: Send + Sync {
    fn skip_invalid<'a>(
        &'a self,
        context: &'a ModuleExecutionContext,
        job: &'a ImportJob,
        row: &'a ImportRow,
    ) -> PortFuture<'a, Result<(), SdkError>>;

    fn record_success<'a>(
        &'a self,
        context: &'a ModuleExecutionContext,
        job: &'a ImportJob,
        row: &'a ImportRow,
        target_party_id: &'a TargetPartyId,
    ) -> PortFuture<'a, Result<(), SdkError>>;

    fn record_retryable_failure<'a>(
        &'a self,
        context: &'a ModuleExecutionContext,
        job: &'a ImportJob,
        row: &'a ImportRow,
        error_code: &'a str,
    ) -> PortFuture<'a, Result<(), SdkError>>;

    fn complete<'a>(
        &'a self,
        context: &'a ModuleExecutionContext,
        job: &'a ImportJob,
    ) -> PortFuture<'a, Result<(), SdkError>>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutionCycleOutcome {
    Completed,
    SkippedInvalid {
        row_position: u32,
    },
    PartySucceeded {
        row_position: u32,
        party_id: String,
    },
    RetryableFailureRecorded {
        row_position: u32,
        error_code: String,
    },
}

#[derive(Clone)]
pub struct PartyImportExecutionCoordinator {
    target_client: Arc<dyn CapabilityClient>,
    outcomes: Arc<dyn ImportExecutionOutcomeSink>,
}

impl std::fmt::Debug for PartyImportExecutionCoordinator {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PartyImportExecutionCoordinator")
            .field("target_client", &"dyn CapabilityClient")
            .field("outcomes", &"dyn ImportExecutionOutcomeSink")
            .finish()
    }
}

impl PartyImportExecutionCoordinator {
    pub fn new(
        target_client: Arc<dyn CapabilityClient>,
        outcomes: Arc<dyn ImportExecutionOutcomeSink>,
    ) -> Self {
        Self {
            target_client,
            outcomes,
        }
    }

    pub fn execute_next<'a>(
        &'a self,
        context: &'a ModuleExecutionContext,
        snapshot: &'a ImportExecutionSnapshot,
    ) -> PortFuture<'a, Result<ExecutionCycleOutcome, SdkError>> {
        Box::pin(async move {
            context.validate()?;
            let Some(row) = snapshot.next_row()? else {
                self.outcomes.complete(context, snapshot.job()).await?;
                return Ok(ExecutionCycleOutcome::Completed);
            };

            match row.status() {
                ImportRowStatus::Invalid => {
                    if snapshot.job().snapshot().partial_execution_policy
                        != PartialExecutionPolicy::AllValidRows
                    {
                        return Err(execution_error(
                            "CUSTOMER_DATA_IMPORT_EXECUTION_INVALID_ROW_POLICY_CONFLICT",
                            ErrorCategory::Internal,
                            false,
                            "Customer-data import execution state is inconsistent.",
                        ));
                    }
                    self.outcomes
                        .skip_invalid(context, snapshot.job(), row)
                        .await?;
                    Ok(ExecutionCycleOutcome::SkippedInvalid {
                        row_position: row.row_position(),
                    })
                }
                ImportRowStatus::Valid | ImportRowStatus::FailedRetryable => {
                    let prepared_party = row.prepared_party().ok_or_else(|| {
                        execution_error(
                            "CUSTOMER_DATA_IMPORT_EXECUTION_PREPARED_PARTY_MISSING",
                            ErrorCategory::Internal,
                            false,
                            "Customer-data import execution state is inconsistent.",
                        )
                    })?;
                    let target_context = target_context(context, row)?;
                    let invocation = party_create_invocation(row)?;
                    match self.target_client.invoke(&target_context, invocation).await {
                        Ok(outcome) => {
                            validate_target_outcome(&outcome, prepared_party.party_id())?;
                            self.outcomes
                                .record_success(
                                    context,
                                    snapshot.job(),
                                    row,
                                    prepared_party.party_id(),
                                )
                                .await?;
                            Ok(ExecutionCycleOutcome::PartySucceeded {
                                row_position: row.row_position(),
                                party_id: prepared_party.party_id().as_str().to_owned(),
                            })
                        }
                        Err(error) if error.retryable => {
                            self.outcomes
                                .record_retryable_failure(context, snapshot.job(), row, &error.code)
                                .await?;
                            Ok(ExecutionCycleOutcome::RetryableFailureRecorded {
                                row_position: row.row_position(),
                                error_code: error.code,
                            })
                        }
                        Err(error) => Err(error),
                    }
                }
                ImportRowStatus::Pending => Err(execution_error(
                    "CUSTOMER_DATA_IMPORT_EXECUTION_PENDING_ROW",
                    ErrorCategory::Internal,
                    false,
                    "Customer-data import execution state is inconsistent.",
                )),
                ImportRowStatus::Succeeded => Err(execution_error(
                    "CUSTOMER_DATA_IMPORT_EXECUTION_SUCCEEDED_ROW_AFTER_CHECKPOINT",
                    ErrorCategory::Internal,
                    false,
                    "Customer-data import execution state is inconsistent.",
                )),
            }
        })
    }
}

pub fn party_create_invocation(row: &ImportRow) -> Result<CapabilityInvocation, SdkError> {
    if !matches!(
        row.status(),
        ImportRowStatus::Valid | ImportRowStatus::FailedRetryable
    ) {
        return Err(execution_error(
            "CUSTOMER_DATA_IMPORT_EXECUTION_ROW_NOT_EXECUTABLE",
            ErrorCategory::Conflict,
            false,
            "The import row is not executable.",
        ));
    }
    let prepared = row.prepared_party().ok_or_else(|| {
        execution_error(
            "CUSTOMER_DATA_IMPORT_EXECUTION_PREPARED_PARTY_MISSING",
            ErrorCategory::Internal,
            false,
            "Customer-data import execution state is inconsistent.",
        )
    })?;
    let kind = match prepared.kind() {
        PartyImportKind::Person => parties::PartyKind::Person,
        PartyImportKind::Organization => parties::PartyKind::Organization,
    };
    let input = support::protobuf_payload(
        PARTIES_MODULE_ID,
        PARTY_CREATE_REQUEST_SCHEMA,
        DataClass::Personal,
        &parties::CreatePartyRequest {
            party_ref: Some(customer::PartyRef {
                party_id: prepared.party_id().as_str().to_owned(),
            }),
            kind: kind as i32,
            display_name: prepared.display_name().to_owned(),
        },
    )?;
    Ok(CapabilityInvocation {
        capability_id: configured_capability_id(PARTY_CREATE_CAPABILITY)?,
        capability_version: configured_capability_version(CONTRACT_VERSION)?,
        input,
    })
}

pub fn target_context(
    base: &ModuleExecutionContext,
    row: &ImportRow,
) -> Result<ModuleExecutionContext, SdkError> {
    base.validate()?;
    let mut context = base.clone();
    context.module_id = ModuleId::try_new(MODULE_ID).map_err(configuration_error)?;
    context.execution.idempotency_key =
        IdempotencyKey::try_new(row.target_idempotency_key()).map_err(configuration_error)?;
    Ok(context)
}

fn validate_target_outcome(
    outcome: &CapabilityOutcome,
    expected_party_id: &TargetPartyId,
) -> Result<(), SdkError> {
    if outcome.affected_resources.iter().any(|resource| {
        resource.resource_type == PARTY_RECORD_TYPE
            && resource.resource_id == expected_party_id.as_str()
    }) {
        Ok(())
    } else {
        Err(execution_error(
            "CUSTOMER_DATA_IMPORT_EXECUTION_TARGET_RESULT_MISMATCH",
            ErrorCategory::Dependency,
            false,
            "The governed Party result did not match the prepared import target.",
        ))
    }
}

fn configured_capability_id(value: &str) -> Result<CapabilityId, SdkError> {
    CapabilityId::try_new(value).map_err(configuration_error)
}

fn configured_capability_version(value: &str) -> Result<CapabilityVersion, SdkError> {
    CapabilityVersion::try_new(value).map_err(configuration_error)
}

fn configuration_error(error: crm_module_sdk::IdentifierError) -> SdkError {
    execution_error(
        "CUSTOMER_DATA_IMPORT_EXECUTION_CONFIGURATION_INVALID",
        ErrorCategory::Internal,
        false,
        "Customer-data import execution is not configured safely.",
    )
    .with_internal_reference(error.to_string())
}

fn execution_error(
    code: &'static str,
    category: ErrorCategory,
    retryable: bool,
    safe_message: &'static str,
) -> SdkError {
    SdkError::new(code, category, retryable, safe_message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crm_customer_data_operations::{
        CreateImportJob, CreateValidatedImportRow, ImportJobId, ImportParserProfile,
        InitialImportRowValidation, MarkImportJobValidated, PartyImportMapping, PreparedPartyRow,
        RowDiagnostic, SourceDescriptor, SourceSystemId, StartImportExecution,
    };
    use crm_module_sdk::{
        ActorId, BusinessTransactionId, CausationId, CorrelationId, ExecutionContext, RequestId,
        ResourceRef, SchemaVersion, TenantId, TraceId,
    };
    use prost::Message;
    use std::sync::Mutex;

    #[derive(Debug)]
    struct FakeClient {
        calls: Mutex<Vec<(ModuleExecutionContext, CapabilityInvocation)>>,
        result: Mutex<Result<CapabilityOutcome, SdkError>>,
    }

    impl CapabilityClient for FakeClient {
        fn invoke<'a>(
            &'a self,
            context: &'a ModuleExecutionContext,
            request: CapabilityInvocation,
        ) -> PortFuture<'a, Result<CapabilityOutcome, SdkError>> {
            self.calls.lock().unwrap().push((context.clone(), request));
            let result = self.result.lock().unwrap().clone();
            Box::pin(async move { result })
        }
    }

    #[derive(Debug, Default)]
    struct FakeSink {
        actions: Mutex<Vec<String>>,
    }

    impl ImportExecutionOutcomeSink for FakeSink {
        fn skip_invalid<'a>(
            &'a self,
            _context: &'a ModuleExecutionContext,
            _job: &'a ImportJob,
            row: &'a ImportRow,
        ) -> PortFuture<'a, Result<(), SdkError>> {
            self.actions
                .lock()
                .unwrap()
                .push(format!("skip:{}", row.row_position()));
            Box::pin(async { Ok(()) })
        }

        fn record_success<'a>(
            &'a self,
            _context: &'a ModuleExecutionContext,
            _job: &'a ImportJob,
            row: &'a ImportRow,
            target_party_id: &'a TargetPartyId,
        ) -> PortFuture<'a, Result<(), SdkError>> {
            self.actions.lock().unwrap().push(format!(
                "success:{}:{}",
                row.row_position(),
                target_party_id.as_str()
            ));
            Box::pin(async { Ok(()) })
        }

        fn record_retryable_failure<'a>(
            &'a self,
            _context: &'a ModuleExecutionContext,
            _job: &'a ImportJob,
            row: &'a ImportRow,
            error_code: &'a str,
        ) -> PortFuture<'a, Result<(), SdkError>> {
            self.actions.lock().unwrap().push(format!(
                "retry:{}:{}",
                row.row_position(),
                error_code
            ));
            Box::pin(async { Ok(()) })
        }

        fn complete<'a>(
            &'a self,
            _context: &'a ModuleExecutionContext,
            _job: &'a ImportJob,
        ) -> PortFuture<'a, Result<(), SdkError>> {
            self.actions.lock().unwrap().push("complete".to_owned());
            Box::pin(async { Ok(()) })
        }
    }

    #[tokio::test]
    async fn out_of_order_rows_execute_the_next_source_position_with_deterministic_idempotency() {
        let job = executing_job(2, 2, 0, PartialExecutionPolicy::AllValidRows);
        let row_one = valid_row(job.job_id().clone(), 1, "party-1");
        let row_two = valid_row(job.job_id().clone(), 2, "party-2");
        let snapshot =
            ImportExecutionSnapshot::try_new(job, vec![row_two.clone(), row_one.clone()]).unwrap();
        let client = Arc::new(FakeClient {
            calls: Mutex::new(Vec::new()),
            result: Mutex::new(Ok(CapabilityOutcome {
                output: None,
                affected_resources: vec![ResourceRef {
                    resource_type: PARTY_RECORD_TYPE.to_owned(),
                    resource_id: "party-1".to_owned(),
                    version: Some(1),
                }],
            })),
        });
        let sink = Arc::new(FakeSink::default());
        let coordinator = PartyImportExecutionCoordinator::new(client.clone(), sink.clone());

        let outcome = coordinator
            .execute_next(&context(), &snapshot)
            .await
            .unwrap();

        assert_eq!(
            outcome,
            ExecutionCycleOutcome::PartySucceeded {
                row_position: 1,
                party_id: "party-1".to_owned(),
            }
        );
        let calls = client.calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(
            calls[0].0.execution.idempotency_key.as_str(),
            row_one.target_idempotency_key()
        );
        assert_eq!(calls[0].1.capability_id.as_str(), PARTY_CREATE_CAPABILITY);
        let request =
            parties::CreatePartyRequest::decode(calls[0].1.input.bytes.as_slice()).unwrap();
        assert_eq!(request.party_ref.unwrap().party_id, "party-1");
        assert_eq!(
            sink.actions.lock().unwrap().as_slice(),
            ["success:1:party-1"]
        );
    }

    #[tokio::test]
    async fn invalid_row_skips_without_invoking_the_party_owner() {
        let job = executing_job(2, 1, 1, PartialExecutionPolicy::AllValidRows);
        let row_one = invalid_row(job.job_id().clone(), 1);
        let row_two = valid_row(job.job_id().clone(), 2, "party-2");
        let snapshot = ImportExecutionSnapshot::try_new(job, vec![row_two, row_one]).unwrap();
        let client = Arc::new(FakeClient {
            calls: Mutex::new(Vec::new()),
            result: Mutex::new(Ok(CapabilityOutcome {
                output: None,
                affected_resources: Vec::new(),
            })),
        });
        let sink = Arc::new(FakeSink::default());
        let coordinator = PartyImportExecutionCoordinator::new(client.clone(), sink.clone());

        let outcome = coordinator
            .execute_next(&context(), &snapshot)
            .await
            .unwrap();

        assert_eq!(
            outcome,
            ExecutionCycleOutcome::SkippedInvalid { row_position: 1 }
        );
        assert!(client.calls.lock().unwrap().is_empty());
        assert_eq!(sink.actions.lock().unwrap().as_slice(), ["skip:1"]);
    }

    #[tokio::test]
    async fn retryable_target_failure_is_persisted_without_claiming_success() {
        let job = executing_job(1, 1, 0, PartialExecutionPolicy::AllValidRows);
        let row = valid_row(job.job_id().clone(), 1, "party-1");
        let snapshot = ImportExecutionSnapshot::try_new(job, vec![row]).unwrap();
        let client = Arc::new(FakeClient {
            calls: Mutex::new(Vec::new()),
            result: Mutex::new(Err(SdkError::new(
                "PARTY_DEPENDENCY_RETRY",
                ErrorCategory::Unavailable,
                true,
                "Party creation is temporarily unavailable.",
            ))),
        });
        let sink = Arc::new(FakeSink::default());
        let coordinator = PartyImportExecutionCoordinator::new(client, sink.clone());

        let outcome = coordinator
            .execute_next(&context(), &snapshot)
            .await
            .unwrap();

        assert_eq!(
            outcome,
            ExecutionCycleOutcome::RetryableFailureRecorded {
                row_position: 1,
                error_code: "PARTY_DEPENDENCY_RETRY".to_owned(),
            }
        );
        assert_eq!(
            sink.actions.lock().unwrap().as_slice(),
            ["retry:1:PARTY_DEPENDENCY_RETRY"]
        );
    }

    #[tokio::test]
    async fn non_retryable_target_failure_stops_without_import_outcome_mutation() {
        let job = executing_job(1, 1, 0, PartialExecutionPolicy::AllValidRows);
        let row = valid_row(job.job_id().clone(), 1, "party-1");
        let snapshot = ImportExecutionSnapshot::try_new(job, vec![row]).unwrap();
        let client = Arc::new(FakeClient {
            calls: Mutex::new(Vec::new()),
            result: Mutex::new(Err(SdkError::new(
                "PARTY_CREATE_REJECTED",
                ErrorCategory::InvalidArgument,
                false,
                "Party creation was rejected.",
            ))),
        });
        let sink = Arc::new(FakeSink::default());
        let coordinator = PartyImportExecutionCoordinator::new(client, sink.clone());

        let error = coordinator
            .execute_next(&context(), &snapshot)
            .await
            .unwrap_err();

        assert_eq!(error.code, "PARTY_CREATE_REJECTED");
        assert!(sink.actions.lock().unwrap().is_empty());
    }

    fn executing_job(
        total_rows: u32,
        valid_rows: u32,
        invalid_rows: u32,
        policy: PartialExecutionPolicy,
    ) -> ImportJob {
        let job_id = ImportJobId::try_new("import-job-execution-1").unwrap();
        let mut job = ImportJob::create(CreateImportJob {
            job_id,
            source: SourceDescriptor::try_new(
                "customers.csv",
                "11".repeat(32),
                total_rows,
                SourceSystemId::try_new("legacy-crm").unwrap(),
                ImportParserProfile::csv_v1(b',', b'"').unwrap(),
            )
            .unwrap(),
            mapping: PartyImportMapping::try_new(
                None,
                "kind",
                "display_name",
                Some("legacy_id".to_owned()),
                Some("row_key".to_owned()),
            )
            .unwrap(),
            partial_execution_policy: policy,
            occurred_at_unix_nanos: 10,
        })
        .unwrap();
        job.mark_validated(MarkImportJobValidated {
            expected_version: 1,
            valid_rows,
            invalid_rows,
            occurred_at_unix_nanos: 20,
        })
        .unwrap();
        job.start_execution(StartImportExecution {
            expected_version: 2,
            occurred_at_unix_nanos: 30,
        })
        .unwrap();
        job
    }

    fn valid_row(job_id: ImportJobId, row_position: u32, party_id: &str) -> ImportRow {
        crm_customer_data_operations::create_validated_import_row(CreateValidatedImportRow {
            job_id,
            row_position,
            external_row_key: Some(format!("row-{row_position}")),
            source_external_id: Some(format!("legacy-{row_position}")),
            outcome: InitialImportRowValidation::Valid(
                PreparedPartyRow::try_new(
                    TargetPartyId::try_new(party_id).unwrap(),
                    PartyImportKind::Person,
                    format!("Party {row_position}"),
                )
                .unwrap(),
            ),
            occurred_at_unix_nanos: 15,
        })
        .unwrap()
    }

    fn invalid_row(job_id: ImportJobId, row_position: u32) -> ImportRow {
        crm_customer_data_operations::create_validated_import_row(CreateValidatedImportRow {
            job_id,
            row_position,
            external_row_key: None,
            source_external_id: None,
            outcome: InitialImportRowValidation::Invalid(vec![
                RowDiagnostic::try_new("DISPLAY_NAME_MISSING", "display_name").unwrap(),
            ]),
            occurred_at_unix_nanos: 15,
        })
        .unwrap()
    }

    fn context() -> ModuleExecutionContext {
        ModuleExecutionContext {
            module_id: ModuleId::try_new(MODULE_ID).unwrap(),
            execution: ExecutionContext {
                tenant_id: TenantId::try_new("tenant-a").unwrap(),
                actor_id: ActorId::try_new("import-worker").unwrap(),
                request_id: RequestId::try_new("request-1").unwrap(),
                correlation_id: CorrelationId::try_new("correlation-1").unwrap(),
                causation_id: CausationId::try_new("causation-1").unwrap(),
                trace_id: TraceId::try_new("trace-1").unwrap(),
                capability_id: CapabilityId::try_new("customer_data.import.party.execute.worker")
                    .unwrap(),
                capability_version: CapabilityVersion::try_new(CONTRACT_VERSION).unwrap(),
                idempotency_key: IdempotencyKey::try_new("worker-cycle-1").unwrap(),
                business_transaction_id: BusinessTransactionId::try_new("import-tx-1").unwrap(),
                schema_version: SchemaVersion::try_new(CONTRACT_VERSION).unwrap(),
                request_started_at_unix_nanos: 40,
            },
        }
    }
}
