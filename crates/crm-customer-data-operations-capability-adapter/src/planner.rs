use crate::{
    CANCEL_PARTY_IMPORT_JOB_CAPABILITY, CANCEL_PARTY_IMPORT_JOB_REQUEST_SCHEMA,
    CANCEL_PARTY_IMPORT_JOB_RESPONSE_SCHEMA, CREATE_PARTY_IMPORT_JOB_CAPABILITY,
    CREATE_PARTY_IMPORT_JOB_REQUEST_SCHEMA, CREATE_PARTY_IMPORT_JOB_RESPONSE_SCHEMA,
    FINALIZE_PARTY_IMPORT_VALIDATION_CAPABILITY, IMPORT_JOB_RECORD_TYPE, IMPORT_ROW_RECORD_TYPE,
    MODULE_ID, MUTATION_CAPABILITY_IDS, PARTY_IMPORT_CANCELLED_EVENT_SCHEMA,
    PARTY_IMPORT_CANCELLED_EVENT_TYPE, PARTY_IMPORT_EXECUTION_STARTED_EVENT_SCHEMA,
    PARTY_IMPORT_EXECUTION_STARTED_EVENT_TYPE, PARTY_IMPORT_JOB_CREATED_EVENT_SCHEMA,
    PARTY_IMPORT_JOB_CREATED_EVENT_TYPE, PARTY_IMPORT_ROW_VALIDATED_EVENT_SCHEMA,
    PARTY_IMPORT_ROW_VALIDATED_EVENT_TYPE, START_PARTY_IMPORT_EXECUTION_CAPABILITY,
    START_PARTY_IMPORT_EXECUTION_REQUEST_SCHEMA, START_PARTY_IMPORT_EXECUTION_RESPONSE_SCHEMA,
    VALIDATE_PARTY_IMPORT_ROWS_CAPABILITY, VALIDATE_PARTY_IMPORT_ROWS_REQUEST_SCHEMA,
    VALIDATE_PARTY_IMPORT_ROWS_RESPONSE_SCHEMA,
};
use crm_capability_plan_support::{self as support, EventSpec, PersistedPayloadContract};
use crm_capability_runtime::{CapabilityDefinition, CapabilityRequest};
use crm_core_data::{
    AggregatePresence, AggregateTarget, BatchMutationPlan, CapabilityBatchExecutionPlan,
    RecordMutation, TransactionalAggregatePlanner,
};
use crm_customer_data_operations::{
    CancelImportJob, CreateImportJob, CreateValidatedImportRow, ExternalPartyIdentifierDigest,
    IMPORT_JOB_STATE_MAXIMUM_BYTES, IMPORT_JOB_STATE_RETENTION_POLICY_ID,
    IMPORT_JOB_STATE_SCHEMA_ID, IMPORT_JOB_STATE_SCHEMA_VERSION, IMPORT_ROW_STATE_MAXIMUM_BYTES,
    IMPORT_ROW_STATE_RETENTION_POLICY_ID, IMPORT_ROW_STATE_SCHEMA_ID,
    IMPORT_ROW_STATE_SCHEMA_VERSION, ImportCanonicalizationVersion, ImportHeaderMode, ImportJob,
    ImportJobId, ImportJobStatus, ImportParserProfile, ImportParserVersion, ImportRow,
    ImportRowStatus, ImportSourceFormat, ImportTextEncoding, InitialImportRowValidation,
    PartialExecutionPolicy, PartyImportKind, PartyImportMapping, PreparedPartyRow, RowDiagnostic,
    RowIdentitySource, SourceDescriptor, SourceSystemId, StartImportExecution, TargetPartyId,
    create_validated_import_row, decode_import_job_state, encode_import_job_state,
    encode_import_row_state, import_job_state_descriptor_hash, import_row_state_descriptor_hash,
};
use crm_module_sdk::{DataClass, ErrorCategory, RecordSnapshot, SdkError};
use crm_proto_contracts::crm::{
    core::v1 as core, customer::v1 as customer, customer_data_operations::v1 as wire,
};
use std::collections::BTreeSet;
use std::fmt::Write as _;

const MAX_VALIDATION_BATCH_ROWS: usize = 500;

#[derive(Debug, Default, Clone, Copy)]
pub struct CustomerDataOperationsCapabilityPlanner;

impl TransactionalAggregatePlanner for CustomerDataOperationsCapabilityPlanner {
    fn target(
        &self,
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
    ) -> Result<AggregateTarget, SdkError> {
        ensure_definition(definition, request)?;
        let (job_id, presence) = match definition.capability_id.as_str() {
            CREATE_PARTY_IMPORT_JOB_CAPABILITY => {
                let command: wire::CreatePartyImportJobRequest =
                    support::decode_request_with_data_class(
                        request,
                        MODULE_ID,
                        CREATE_PARTY_IMPORT_JOB_REQUEST_SCHEMA,
                        DataClass::Personal,
                    )?;
                (
                    import_job_id_from_ref(command.import_job_ref)?,
                    AggregatePresence::MustBeAbsent,
                )
            }
            VALIDATE_PARTY_IMPORT_ROWS_CAPABILITY => {
                let command: wire::ValidatePartyImportRowsRequest =
                    support::decode_request_with_data_class(
                        request,
                        MODULE_ID,
                        VALIDATE_PARTY_IMPORT_ROWS_REQUEST_SCHEMA,
                        DataClass::Personal,
                    )?;
                (
                    import_job_id_from_ref(command.import_job_ref)?,
                    AggregatePresence::MustExist,
                )
            }
            FINALIZE_PARTY_IMPORT_VALIDATION_CAPABILITY => {
                let command: wire::FinalizePartyImportValidationRequest =
                    support::decode_request_with_data_class(
                        request,
                        MODULE_ID,
                        crate::FINALIZE_PARTY_IMPORT_VALIDATION_REQUEST_SCHEMA,
                        DataClass::Personal,
                    )?;
                (
                    import_job_id_from_ref(command.import_job_ref)?,
                    AggregatePresence::MustExist,
                )
            }
            START_PARTY_IMPORT_EXECUTION_CAPABILITY => {
                let command: wire::StartPartyImportExecutionRequest =
                    support::decode_request_with_data_class(
                        request,
                        MODULE_ID,
                        START_PARTY_IMPORT_EXECUTION_REQUEST_SCHEMA,
                        DataClass::Personal,
                    )?;
                (
                    import_job_id_from_ref(command.import_job_ref)?,
                    AggregatePresence::MustExist,
                )
            }
            CANCEL_PARTY_IMPORT_JOB_CAPABILITY => {
                let command: wire::CancelPartyImportJobRequest =
                    support::decode_request_with_data_class(
                        request,
                        MODULE_ID,
                        CANCEL_PARTY_IMPORT_JOB_REQUEST_SCHEMA,
                        DataClass::Personal,
                    )?;
                (
                    import_job_id_from_ref(command.import_job_ref)?,
                    AggregatePresence::MustExist,
                )
            }
            _ => return Err(unsupported_capability()),
        };

        Ok(AggregateTarget {
            reference: support::record_ref(
                IMPORT_JOB_RECORD_TYPE,
                job_id.as_str(),
                "customer_data.import_job_ref.import_job_id",
            )?,
            presence,
        })
    }

    fn plan(
        &self,
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
        current: Option<&RecordSnapshot>,
    ) -> Result<CapabilityBatchExecutionPlan, SdkError> {
        ensure_definition(definition, request)?;
        match definition.capability_id.as_str() {
            CREATE_PARTY_IMPORT_JOB_CAPABILITY => plan_create(definition, request, current),
            VALIDATE_PARTY_IMPORT_ROWS_CAPABILITY => {
                plan_validate_rows(definition, request, current)
            }
            START_PARTY_IMPORT_EXECUTION_CAPABILITY => {
                plan_start_execution(definition, request, current)
            }
            CANCEL_PARTY_IMPORT_JOB_CAPABILITY => plan_cancel(definition, request, current),
            FINALIZE_PARTY_IMPORT_VALIDATION_CAPABILITY => Err(finalize_requires_composition()),
            _ => Err(unsupported_capability()),
        }
    }
}

fn plan_create(
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
    current: Option<&RecordSnapshot>,
) -> Result<CapabilityBatchExecutionPlan, SdkError> {
    if current.is_some() {
        return Err(invalid_plan());
    }
    let command: wire::CreatePartyImportJobRequest = support::decode_request_with_data_class(
        request,
        MODULE_ID,
        CREATE_PARTY_IMPORT_JOB_REQUEST_SCHEMA,
        DataClass::Personal,
    )?;
    let job = ImportJob::create(CreateImportJob {
        job_id: import_job_id_from_ref(command.import_job_ref)?,
        source: source_from_wire(command.source)?,
        mapping: mapping_from_wire(command.mapping)?,
        partial_execution_policy: partial_execution_policy_from_wire(
            command.partial_execution_policy,
        )?,
        occurred_at_unix_nanos: request.context.execution.request_started_at_unix_nanos,
    })?;
    let aggregate = import_job_record_ref(job.job_id())?;
    let public_job = job_to_wire(&job)?;
    let output = support::protobuf_payload(
        MODULE_ID,
        CREATE_PARTY_IMPORT_JOB_RESPONSE_SCHEMA,
        DataClass::Personal,
        &wire::CreatePartyImportJobResponse {
            import_job: Some(public_job.clone()),
        },
    )?;
    let event = support::event_evidence_with_data_class(
        request,
        aggregate.clone(),
        MODULE_ID,
        EventSpec {
            event_type: PARTY_IMPORT_JOB_CREATED_EVENT_TYPE,
            event_schema_id: PARTY_IMPORT_JOB_CREATED_EVENT_SCHEMA,
            aggregate_version: job.version(),
            previous_version: None,
        },
        DataClass::Personal,
        &wire::PartyImportJobCreatedEvent {
            import_job: Some(public_job),
        },
    )?;

    single_mutation_plan(
        definition,
        request,
        aggregate.clone(),
        RecordMutation::Create {
            reference: aggregate,
            payload: import_job_persisted_payload(&job)?,
        },
        event,
        output,
    )
}

fn plan_validate_rows(
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
    current: Option<&RecordSnapshot>,
) -> Result<CapabilityBatchExecutionPlan, SdkError> {
    let current = current.ok_or_else(invalid_plan)?;
    let command: wire::ValidatePartyImportRowsRequest = support::decode_request_with_data_class(
        request,
        MODULE_ID,
        VALIDATE_PARTY_IMPORT_ROWS_REQUEST_SCHEMA,
        DataClass::Personal,
    )?;
    let requested_job_id = import_job_id_from_ref(command.import_job_ref)?;
    if requested_job_id.as_str() != current.reference.record_id.as_str() {
        return Err(invalid_plan());
    }
    let job = import_job_from_snapshot(current)?;
    if job.status() != ImportJobStatus::Created {
        return Err(SdkError::new(
            "CUSTOMER_DATA_IMPORT_JOB_NOT_CREATED",
            ErrorCategory::Conflict,
            false,
            "Only a created import job can accept row validation.",
        ));
    }
    if command.rows.is_empty() || command.rows.len() > MAX_VALIDATION_BATCH_ROWS {
        return Err(SdkError::invalid_argument(
            "customer_data.import.rows",
            format!("Validation batch must contain between 1 and {MAX_VALIDATION_BATCH_ROWS} rows"),
        ));
    }

    let occurred_at = request.context.execution.request_started_at_unix_nanos;
    let mut seen_positions = BTreeSet::new();
    let mut seen_row_ids = BTreeSet::new();
    let mut seen_target_party_ids = BTreeSet::new();
    let mut rows = Vec::with_capacity(command.rows.len());
    let mut records = Vec::with_capacity(command.rows.len());
    let mut events = Vec::with_capacity(command.rows.len());

    for source_row in command.rows {
        if source_row.row_position == 0 || source_row.row_position > job.source().row_count() {
            return Err(SdkError::invalid_argument(
                "customer_data.import.rows.row_position",
                "Row position must be inside the immutable source row range",
            ));
        }
        if !seen_positions.insert(source_row.row_position) {
            return Err(SdkError::invalid_argument(
                "customer_data.import.rows.row_position",
                "Validation batch contains a duplicate row position",
            ));
        }

        let mut diagnostics = Vec::new();
        let external_row_key = optional_mapped_value(
            &source_row.columns,
            job.mapping().external_row_key_column(),
            "EXTERNAL_ROW_KEY_MISSING",
            &mut diagnostics,
        );
        let external_row_key = match external_row_key {
            Some(value) => {
                match RowIdentitySource::for_row(source_row.row_position, Some(&value)) {
                    Ok(_) => Some(value),
                    Err(_) => {
                        diagnostics.push(RowDiagnostic::try_new(
                            "EXTERNAL_ROW_KEY_INVALID",
                            job.mapping()
                                .external_row_key_column()
                                .unwrap_or("external_row_key"),
                        )?);
                        None
                    }
                }
            }
            None => None,
        };

        let source_external_id = optional_mapped_value(
            &source_row.columns,
            job.mapping().source_external_id_column(),
            "SOURCE_EXTERNAL_ID_MISSING",
            &mut diagnostics,
        );
        let source_external_id = match source_external_id {
            Some(value) => match ExternalPartyIdentifierDigest::for_identifier(value.clone()) {
                Ok(_) => Some(value),
                Err(_) => {
                    diagnostics.push(RowDiagnostic::try_new(
                        "SOURCE_EXTERNAL_ID_INVALID",
                        job.mapping()
                            .source_external_id_column()
                            .unwrap_or("source_external_id"),
                    )?);
                    None
                }
            },
            None => None,
        };

        let probe = ImportRow::create(crm_customer_data_operations::CreateImportRow {
            job_id: requested_job_id.clone(),
            row_position: source_row.row_position,
            external_row_key: external_row_key.clone(),
            source_external_id: source_external_id.clone(),
            occurred_at_unix_nanos: occurred_at,
        })?;
        if !seen_row_ids.insert(probe.row_id().clone()) {
            return Err(SdkError::invalid_argument(
                "customer_data.import.rows.external_row_key",
                "Validation batch resolves more than one source row to the same deterministic row identity",
            ));
        }

        let target_party_id =
            target_party_id_for_row(&source_row.columns, job.mapping(), &probe, &mut diagnostics)?;
        let party_kind = party_kind_for_row(&source_row.columns, job.mapping(), &mut diagnostics)?;
        let display_name = required_mapped_value(
            &source_row.columns,
            job.mapping().display_name_column(),
            "DISPLAY_NAME_MISSING",
            &mut diagnostics,
        );

        if diagnostics.is_empty() && !seen_target_party_ids.insert(target_party_id.clone()) {
            diagnostics.push(RowDiagnostic::try_new(
                "TARGET_PARTY_DUPLICATE_IN_BATCH",
                job.mapping()
                    .target_party_id_column()
                    .unwrap_or("derived_target_party_id"),
            )?);
        }

        let outcome = if diagnostics.is_empty() {
            InitialImportRowValidation::Valid(PreparedPartyRow::try_new(
                target_party_id,
                party_kind.ok_or_else(invalid_plan)?,
                display_name.ok_or_else(invalid_plan)?,
            )?)
        } else {
            InitialImportRowValidation::Invalid(diagnostics)
        };
        let row = create_validated_import_row(CreateValidatedImportRow {
            job_id: requested_job_id.clone(),
            row_position: source_row.row_position,
            external_row_key,
            source_external_id,
            outcome,
            occurred_at_unix_nanos: occurred_at,
        })?;
        let aggregate = import_row_record_ref(&row)?;
        let public_row = import_row_to_wire(&row)?;
        records.push(RecordMutation::Create {
            reference: aggregate.clone(),
            payload: import_row_persisted_payload(&row)?,
        });
        events.push(support::event_evidence_with_data_class(
            request,
            aggregate,
            MODULE_ID,
            EventSpec {
                event_type: PARTY_IMPORT_ROW_VALIDATED_EVENT_TYPE,
                event_schema_id: PARTY_IMPORT_ROW_VALIDATED_EVENT_SCHEMA,
                aggregate_version: row.version(),
                previous_version: None,
            },
            DataClass::Personal,
            &wire::PartyImportRowValidatedEvent {
                import_row: Some(public_row.clone()),
            },
        )?);
        rows.push(public_row);
    }

    let output = support::protobuf_payload(
        MODULE_ID,
        VALIDATE_PARTY_IMPORT_ROWS_RESPONSE_SCHEMA,
        DataClass::Personal,
        &wire::ValidatePartyImportRowsResponse { import_rows: rows },
    )?;
    let audits = events
        .iter()
        .map(|event| {
            support::audit_intent(
                request,
                &event.event.aggregate,
                event.aggregate_version,
                definition.capability_id.as_str(),
                &output.bytes,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(CapabilityBatchExecutionPlan {
        batch: BatchMutationPlan {
            context: request.context.clone(),
            records,
            relationships: Vec::new(),
            events,
            idempotency: support::capability_idempotency(definition, request)?,
            audits,
        },
        output: Some(output),
    })
}

fn plan_start_execution(
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
    current: Option<&RecordSnapshot>,
) -> Result<CapabilityBatchExecutionPlan, SdkError> {
    let current = current.ok_or_else(invalid_plan)?;
    let command: wire::StartPartyImportExecutionRequest = support::decode_request_with_data_class(
        request,
        MODULE_ID,
        START_PARTY_IMPORT_EXECUTION_REQUEST_SCHEMA,
        DataClass::Personal,
    )?;
    let requested_job_id = import_job_id_from_ref(command.import_job_ref)?;
    if requested_job_id.as_str() != current.reference.record_id.as_str() {
        return Err(invalid_plan());
    }
    let mut job = import_job_from_snapshot(current)?;
    job.start_execution(StartImportExecution {
        expected_version: command.expected_version,
        occurred_at_unix_nanos: request.context.execution.request_started_at_unix_nanos,
    })?;
    let public_job = job_to_wire(&job)?;
    let output = support::protobuf_payload(
        MODULE_ID,
        START_PARTY_IMPORT_EXECUTION_RESPONSE_SCHEMA,
        DataClass::Personal,
        &wire::StartPartyImportExecutionResponse {
            import_job: Some(public_job.clone()),
        },
    )?;
    let aggregate = current.reference.clone();
    let event = support::event_evidence_with_data_class(
        request,
        aggregate.clone(),
        MODULE_ID,
        EventSpec {
            event_type: PARTY_IMPORT_EXECUTION_STARTED_EVENT_TYPE,
            event_schema_id: PARTY_IMPORT_EXECUTION_STARTED_EVENT_SCHEMA,
            aggregate_version: job.version(),
            previous_version: Some(current.version),
        },
        DataClass::Personal,
        &wire::PartyImportExecutionStartedEvent {
            import_job: Some(public_job),
        },
    )?;
    single_mutation_plan(
        definition,
        request,
        aggregate.clone(),
        RecordMutation::Update {
            reference: aggregate,
            expected_version: current.version,
            payload: import_job_persisted_payload(&job)?,
        },
        event,
        output,
    )
}

fn plan_cancel(
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
    current: Option<&RecordSnapshot>,
) -> Result<CapabilityBatchExecutionPlan, SdkError> {
    let current = current.ok_or_else(invalid_plan)?;
    let command: wire::CancelPartyImportJobRequest = support::decode_request_with_data_class(
        request,
        MODULE_ID,
        CANCEL_PARTY_IMPORT_JOB_REQUEST_SCHEMA,
        DataClass::Personal,
    )?;
    let requested_job_id = import_job_id_from_ref(command.import_job_ref)?;
    if requested_job_id.as_str() != current.reference.record_id.as_str() {
        return Err(invalid_plan());
    }
    let mut job = import_job_from_snapshot(current)?;
    job.cancel(CancelImportJob {
        expected_version: command.expected_version,
        occurred_at_unix_nanos: request.context.execution.request_started_at_unix_nanos,
    })?;
    let public_job = job_to_wire(&job)?;
    let output = support::protobuf_payload(
        MODULE_ID,
        CANCEL_PARTY_IMPORT_JOB_RESPONSE_SCHEMA,
        DataClass::Personal,
        &wire::CancelPartyImportJobResponse {
            import_job: Some(public_job.clone()),
        },
    )?;
    let aggregate = current.reference.clone();
    let event = support::event_evidence_with_data_class(
        request,
        aggregate.clone(),
        MODULE_ID,
        EventSpec {
            event_type: PARTY_IMPORT_CANCELLED_EVENT_TYPE,
            event_schema_id: PARTY_IMPORT_CANCELLED_EVENT_SCHEMA,
            aggregate_version: job.version(),
            previous_version: Some(current.version),
        },
        DataClass::Personal,
        &wire::PartyImportCancelledEvent {
            import_job: Some(public_job),
        },
    )?;
    single_mutation_plan(
        definition,
        request,
        aggregate.clone(),
        RecordMutation::Update {
            reference: aggregate,
            expected_version: current.version,
            payload: import_job_persisted_payload(&job)?,
        },
        event,
        output,
    )
}

fn single_mutation_plan(
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
    aggregate: crm_module_sdk::RecordRef,
    mutation: RecordMutation,
    event: crm_core_data::EventEvidence,
    output: crm_module_sdk::TypedPayload,
) -> Result<CapabilityBatchExecutionPlan, SdkError> {
    let audit = support::audit_intent(
        request,
        &aggregate,
        event.aggregate_version,
        definition.capability_id.as_str(),
        &output.bytes,
    )?;
    Ok(CapabilityBatchExecutionPlan {
        batch: BatchMutationPlan {
            context: request.context.clone(),
            records: vec![mutation],
            relationships: Vec::new(),
            events: vec![event],
            idempotency: support::capability_idempotency(definition, request)?,
            audits: vec![audit],
        },
        output: Some(output),
    })
}

pub fn import_job_persisted_contract() -> PersistedPayloadContract<'static> {
    PersistedPayloadContract {
        owner: MODULE_ID,
        schema_id: IMPORT_JOB_STATE_SCHEMA_ID,
        schema_version: IMPORT_JOB_STATE_SCHEMA_VERSION,
        descriptor_hash: import_job_state_descriptor_hash(),
        maximum_size_bytes: IMPORT_JOB_STATE_MAXIMUM_BYTES,
        retention_policy_id: IMPORT_JOB_STATE_RETENTION_POLICY_ID,
    }
}

pub fn import_row_persisted_contract() -> PersistedPayloadContract<'static> {
    PersistedPayloadContract {
        owner: MODULE_ID,
        schema_id: IMPORT_ROW_STATE_SCHEMA_ID,
        schema_version: IMPORT_ROW_STATE_SCHEMA_VERSION,
        descriptor_hash: import_row_state_descriptor_hash(),
        maximum_size_bytes: IMPORT_ROW_STATE_MAXIMUM_BYTES,
        retention_policy_id: IMPORT_ROW_STATE_RETENTION_POLICY_ID,
    }
}

pub fn import_job_persisted_payload(
    job: &ImportJob,
) -> Result<crm_module_sdk::TypedPayload, SdkError> {
    support::persisted_json_payload_with_data_class(
        import_job_persisted_contract(),
        DataClass::Personal,
        encode_import_job_state(job)?,
    )
}

pub fn import_row_persisted_payload(
    row: &ImportRow,
) -> Result<crm_module_sdk::TypedPayload, SdkError> {
    support::persisted_json_payload_with_data_class(
        import_row_persisted_contract(),
        DataClass::Personal,
        encode_import_row_state(row)?,
    )
}

pub fn import_job_from_snapshot(snapshot: &RecordSnapshot) -> Result<ImportJob, SdkError> {
    let job = decode_import_job_state(support::persisted_json_bytes_with_data_class(
        snapshot,
        import_job_persisted_contract(),
        DataClass::Personal,
    )?)?;
    if job.job_id().as_str() != snapshot.reference.record_id.as_str()
        || job.version() != snapshot.version
    {
        return Err(support::stored_data_error(
            "CUSTOMER_DATA_IMPORT_PERSISTED_JOB_IDENTITY_INVALID",
        ));
    }
    Ok(job)
}

pub fn job_to_wire(job: &ImportJob) -> Result<wire::ImportJob, SdkError> {
    let snapshot = job.snapshot();
    Ok(wire::ImportJob {
        import_job_ref: Some(wire::ImportJobRef {
            import_job_id: snapshot.job_id.as_str().to_owned(),
        }),
        source: Some(source_to_wire(&snapshot.source)?),
        mapping: Some(mapping_to_wire(&snapshot.mapping)),
        mapping_version_id: snapshot.mapping_version_id.as_str().to_owned(),
        partial_execution_policy: match snapshot.partial_execution_policy {
            PartialExecutionPolicy::AllValidRows => {
                wire::PartialExecutionPolicy::AllValidRows as i32
            }
            PartialExecutionPolicy::RequireAllValid => {
                wire::PartialExecutionPolicy::RequireAllValid as i32
            }
        },
        status: import_job_status_to_wire(snapshot.status) as i32,
        total_rows: snapshot.total_rows,
        valid_rows: snapshot.valid_rows,
        invalid_rows: snapshot.invalid_rows,
        succeeded_rows: snapshot.succeeded_rows,
        checkpoint_row_position: snapshot.checkpoint_row_position,
        created_at: Some(core::UnixTime {
            unix_nanos: snapshot.created_at_unix_nanos,
        }),
        updated_at: Some(core::UnixTime {
            unix_nanos: snapshot.updated_at_unix_nanos,
        }),
        resource_version: Some(customer::CustomerResourceVersion {
            version: snapshot.version,
            created_at: Some(core::UnixTime {
                unix_nanos: snapshot.created_at_unix_nanos,
            }),
            updated_at: Some(core::UnixTime {
                unix_nanos: snapshot.updated_at_unix_nanos,
            }),
        }),
    })
}

pub fn import_row_to_wire(row: &ImportRow) -> Result<wire::ImportRow, SdkError> {
    let snapshot = row.snapshot();
    let external_row_key_sha256 = match &snapshot.identity_source {
        RowIdentitySource::Position(_) => String::new(),
        RowIdentitySource::ExternalKeySha256(value) => value.clone(),
    };
    Ok(wire::ImportRow {
        import_row_ref: Some(wire::ImportRowRef {
            import_job_ref: Some(wire::ImportJobRef {
                import_job_id: snapshot.job_id.as_str().to_owned(),
            }),
            import_row_id: snapshot.row_id.as_str().to_owned(),
        }),
        row_position: snapshot.row_position,
        external_row_key_sha256,
        source_external_id_sha256: snapshot
            .source_external_id_sha256
            .as_ref()
            .map(|value| value.as_str().to_owned())
            .unwrap_or_default(),
        status: import_row_status_to_wire(snapshot.status) as i32,
        prepared_party: snapshot.prepared_party.as_ref().map(prepared_party_to_wire),
        diagnostics: snapshot
            .diagnostics
            .iter()
            .map(|diagnostic| wire::RowDiagnostic {
                code: diagnostic.code().to_owned(),
                field: diagnostic.field().to_owned(),
            })
            .collect(),
        execution_attempts: snapshot.execution_attempts,
        last_execution_error_code: snapshot.last_execution_error_code.unwrap_or_default(),
        target_party_ref: snapshot.target_party_id.map(|party_id| customer::PartyRef {
            party_id: party_id.as_str().to_owned(),
        }),
        created_at: Some(core::UnixTime {
            unix_nanos: snapshot.created_at_unix_nanos,
        }),
        updated_at: Some(core::UnixTime {
            unix_nanos: snapshot.updated_at_unix_nanos,
        }),
        resource_version: Some(customer::CustomerResourceVersion {
            version: snapshot.version,
            created_at: Some(core::UnixTime {
                unix_nanos: snapshot.created_at_unix_nanos,
            }),
            updated_at: Some(core::UnixTime {
                unix_nanos: snapshot.updated_at_unix_nanos,
            }),
        }),
    })
}

fn source_from_wire(
    value: Option<wire::ImportSourceDescriptor>,
) -> Result<SourceDescriptor, SdkError> {
    let value = value.ok_or_else(|| {
        SdkError::invalid_argument("customer_data.import.source", "Import source is required")
    })?;
    SourceDescriptor::try_new(
        value.source_name,
        sha256_bytes_to_hex(&value.content_sha256)?,
        value.row_count,
        SourceSystemId::try_new(value.source_system_id)?,
        parser_profile_from_wire(value.parser_profile)?,
    )
}

fn parser_profile_from_wire(
    value: Option<wire::ImportParserProfile>,
) -> Result<ImportParserProfile, SdkError> {
    let value = value.ok_or_else(|| {
        SdkError::invalid_argument(
            "customer_data.import.source.parser_profile",
            "Import parser profile is required",
        )
    })?;
    let format = match wire::ImportSourceFormat::try_from(value.format) {
        Ok(wire::ImportSourceFormat::Csv) => ImportSourceFormat::Csv,
        Ok(wire::ImportSourceFormat::Unspecified) | Err(_) => {
            return Err(SdkError::invalid_argument(
                "customer_data.import.source.parser_profile.format",
                "Import source format must be CSV",
            ));
        }
    };
    let encoding = match wire::ImportTextEncoding::try_from(value.encoding) {
        Ok(wire::ImportTextEncoding::Utf8) => ImportTextEncoding::Utf8,
        Ok(wire::ImportTextEncoding::Unspecified) | Err(_) => {
            return Err(SdkError::invalid_argument(
                "customer_data.import.source.parser_profile.encoding",
                "Import text encoding must be UTF8",
            ));
        }
    };
    let header_mode = match wire::ImportHeaderMode::try_from(value.header_mode) {
        Ok(wire::ImportHeaderMode::RequiredFirstRow) => ImportHeaderMode::RequiredFirstRow,
        Ok(wire::ImportHeaderMode::Unspecified) | Err(_) => {
            return Err(SdkError::invalid_argument(
                "customer_data.import.source.parser_profile.header_mode",
                "Import header mode must be REQUIRED_FIRST_ROW",
            ));
        }
    };
    let parser_version = match wire::ImportParserVersion::try_from(value.parser_version) {
        Ok(wire::ImportParserVersion::CsvV1) => ImportParserVersion::CsvV1,
        Ok(wire::ImportParserVersion::Unspecified) | Err(_) => {
            return Err(SdkError::invalid_argument(
                "customer_data.import.source.parser_profile.parser_version",
                "Import parser version must be CSV_V1",
            ));
        }
    };
    let canonicalization_version =
        match wire::ImportCanonicalizationVersion::try_from(value.canonicalization_version) {
            Ok(wire::ImportCanonicalizationVersion::V1) => ImportCanonicalizationVersion::V1,
            Ok(wire::ImportCanonicalizationVersion::Unspecified) | Err(_) => {
                return Err(SdkError::invalid_argument(
                    "customer_data.import.source.parser_profile.canonicalization_version",
                    "Import canonicalization version must be V1",
                ));
            }
        };
    ImportParserProfile::try_new(
        format,
        encoding,
        u8::try_from(value.delimiter_ascii).map_err(|_| {
            SdkError::invalid_argument(
                "customer_data.import.source.parser_profile.delimiter_ascii",
                "Delimiter must fit one ASCII byte",
            )
        })?,
        u8::try_from(value.quote_ascii).map_err(|_| {
            SdkError::invalid_argument(
                "customer_data.import.source.parser_profile.quote_ascii",
                "Quote character must fit one ASCII byte",
            )
        })?,
        header_mode,
        parser_version,
        canonicalization_version,
    )
}

fn source_to_wire(source: &SourceDescriptor) -> Result<wire::ImportSourceDescriptor, SdkError> {
    Ok(wire::ImportSourceDescriptor {
        source_name: source.source_name().to_owned(),
        content_sha256: sha256_hex_to_bytes(source.content_sha256())?,
        row_count: source.row_count(),
        source_system_id: source.source_system_id().as_str().to_owned(),
        parser_profile: Some(parser_profile_to_wire(source.parser_profile())),
    })
}

fn parser_profile_to_wire(profile: &ImportParserProfile) -> wire::ImportParserProfile {
    wire::ImportParserProfile {
        format: wire::ImportSourceFormat::Csv as i32,
        encoding: wire::ImportTextEncoding::Utf8 as i32,
        delimiter_ascii: u32::from(profile.delimiter()),
        quote_ascii: u32::from(profile.quote()),
        header_mode: wire::ImportHeaderMode::RequiredFirstRow as i32,
        parser_version: wire::ImportParserVersion::CsvV1 as i32,
        canonicalization_version: wire::ImportCanonicalizationVersion::V1 as i32,
    }
}

fn mapping_from_wire(
    value: Option<wire::PartyImportMapping>,
) -> Result<PartyImportMapping, SdkError> {
    let value = value.ok_or_else(|| {
        SdkError::invalid_argument(
            "customer_data.import.mapping",
            "Party import mapping is required",
        )
    })?;
    PartyImportMapping::try_new(
        value.target_party_id_column,
        value.party_kind_column,
        value.display_name_column,
        value.source_external_id_column,
        value.external_row_key_column,
    )
}

fn mapping_to_wire(mapping: &PartyImportMapping) -> wire::PartyImportMapping {
    wire::PartyImportMapping {
        target_party_id_column: mapping.target_party_id_column().map(str::to_owned),
        party_kind_column: mapping.party_kind_column().to_owned(),
        display_name_column: mapping.display_name_column().to_owned(),
        source_external_id_column: mapping.source_external_id_column().map(str::to_owned),
        external_row_key_column: mapping.external_row_key_column().map(str::to_owned),
    }
}

fn partial_execution_policy_from_wire(value: i32) -> Result<PartialExecutionPolicy, SdkError> {
    match wire::PartialExecutionPolicy::try_from(value) {
        Ok(wire::PartialExecutionPolicy::AllValidRows) => Ok(PartialExecutionPolicy::AllValidRows),
        Ok(wire::PartialExecutionPolicy::RequireAllValid) => {
            Ok(PartialExecutionPolicy::RequireAllValid)
        }
        Ok(wire::PartialExecutionPolicy::Unspecified) | Err(_) => Err(SdkError::invalid_argument(
            "customer_data.import.partial_execution_policy",
            "Partial execution policy must be ALL_VALID_ROWS or REQUIRE_ALL_VALID",
        )),
    }
}

fn import_job_status_to_wire(value: ImportJobStatus) -> wire::ImportJobStatus {
    match value {
        ImportJobStatus::Created => wire::ImportJobStatus::Created,
        ImportJobStatus::Validated => wire::ImportJobStatus::Validated,
        ImportJobStatus::Executing => wire::ImportJobStatus::Executing,
        ImportJobStatus::Completed => wire::ImportJobStatus::Completed,
        ImportJobStatus::Cancelled => wire::ImportJobStatus::Cancelled,
    }
}

fn import_row_status_to_wire(value: ImportRowStatus) -> wire::ImportRowStatus {
    match value {
        ImportRowStatus::Pending => wire::ImportRowStatus::Pending,
        ImportRowStatus::Valid => wire::ImportRowStatus::Valid,
        ImportRowStatus::Invalid => wire::ImportRowStatus::Invalid,
        ImportRowStatus::FailedRetryable => wire::ImportRowStatus::FailedRetryable,
        ImportRowStatus::Succeeded => wire::ImportRowStatus::Succeeded,
    }
}

fn prepared_party_to_wire(value: &PreparedPartyRow) -> wire::PreparedPartyRow {
    wire::PreparedPartyRow {
        party_ref: Some(customer::PartyRef {
            party_id: value.party_id().as_str().to_owned(),
        }),
        kind: match value.kind() {
            PartyImportKind::Person => wire::PartyImportKind::Person as i32,
            PartyImportKind::Organization => wire::PartyImportKind::Organization as i32,
        },
        display_name: value.display_name().to_owned(),
    }
}

fn target_party_id_for_row(
    columns: &std::collections::BTreeMap<String, String>,
    mapping: &PartyImportMapping,
    probe: &ImportRow,
    diagnostics: &mut Vec<RowDiagnostic>,
) -> Result<TargetPartyId, SdkError> {
    if let Some(column) = mapping.target_party_id_column() {
        match required_mapped_value(columns, column, "TARGET_PARTY_ID_MISSING", diagnostics) {
            Some(value) => match TargetPartyId::try_new(value) {
                Ok(value) => Ok(value),
                Err(_) => {
                    diagnostics.push(RowDiagnostic::try_new("TARGET_PARTY_ID_INVALID", column)?);
                    probe.derived_target_party_id()
                }
            },
            None => probe.derived_target_party_id(),
        }
    } else {
        probe.derived_target_party_id()
    }
}

fn party_kind_for_row(
    columns: &std::collections::BTreeMap<String, String>,
    mapping: &PartyImportMapping,
    diagnostics: &mut Vec<RowDiagnostic>,
) -> Result<Option<PartyImportKind>, SdkError> {
    let Some(value) = required_mapped_value(
        columns,
        mapping.party_kind_column(),
        "PARTY_KIND_MISSING",
        diagnostics,
    ) else {
        return Ok(None);
    };
    match value.trim() {
        "person" => Ok(Some(PartyImportKind::Person)),
        "organization" => Ok(Some(PartyImportKind::Organization)),
        _ => {
            diagnostics.push(RowDiagnostic::try_new(
                "PARTY_KIND_INVALID",
                mapping.party_kind_column(),
            )?);
            Ok(None)
        }
    }
}

fn required_mapped_value(
    columns: &std::collections::BTreeMap<String, String>,
    column: &str,
    missing_code: &str,
    diagnostics: &mut Vec<RowDiagnostic>,
) -> Option<String> {
    match columns
        .get(column)
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
    {
        Some(value) => Some(value.to_owned()),
        None => {
            if let Ok(diagnostic) = RowDiagnostic::try_new(missing_code, column) {
                diagnostics.push(diagnostic);
            }
            None
        }
    }
}

fn optional_mapped_value(
    columns: &std::collections::BTreeMap<String, String>,
    column: Option<&str>,
    missing_code: &str,
    diagnostics: &mut Vec<RowDiagnostic>,
) -> Option<String> {
    column.and_then(|column| required_mapped_value(columns, column, missing_code, diagnostics))
}

fn import_job_id_from_ref(value: Option<wire::ImportJobRef>) -> Result<ImportJobId, SdkError> {
    let value = value.ok_or_else(|| {
        SdkError::invalid_argument(
            "customer_data.import_job_ref",
            "Import job reference is required",
        )
    })?;
    ImportJobId::try_new(value.import_job_id)
}

fn import_job_record_ref(job_id: &ImportJobId) -> Result<crm_module_sdk::RecordRef, SdkError> {
    support::record_ref(
        IMPORT_JOB_RECORD_TYPE,
        job_id.as_str(),
        "customer_data.import_job_ref.import_job_id",
    )
}

fn import_row_record_ref(row: &ImportRow) -> Result<crm_module_sdk::RecordRef, SdkError> {
    support::record_ref(
        IMPORT_ROW_RECORD_TYPE,
        row.row_id().as_str(),
        "customer_data.import_row_ref.import_row_id",
    )
}

fn sha256_bytes_to_hex(bytes: &[u8]) -> Result<String, SdkError> {
    if bytes.len() != 32 {
        return Err(SdkError::invalid_argument(
            "customer_data.import.source.content_sha256",
            "Source SHA-256 must contain exactly 32 bytes",
        ));
    }
    let mut value = String::with_capacity(64);
    for byte in bytes {
        write!(&mut value, "{byte:02x}").expect("writing to String cannot fail");
    }
    Ok(value)
}

fn sha256_hex_to_bytes(value: &str) -> Result<Vec<u8>, SdkError> {
    if value.len() != 64 {
        return Err(invalid_plan());
    }
    (0..64)
        .step_by(2)
        .map(|index| u8::from_str_radix(&value[index..index + 2], 16).map_err(|_| invalid_plan()))
        .collect()
}

fn ensure_definition(
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
) -> Result<(), SdkError> {
    if !MUTATION_CAPABILITY_IDS.contains(&definition.capability_id.as_str())
        || definition.owner_module_id.as_str() != MODULE_ID
        || definition.capability_id.as_str() != request.context.execution.capability_id.as_str()
    {
        return Err(invalid_plan());
    }
    Ok(())
}

fn finalize_requires_composition() -> SdkError {
    SdkError::new(
        "CUSTOMER_DATA_IMPORT_FINALIZE_REQUIRES_COMPOSITION",
        ErrorCategory::Internal,
        false,
        "Import validation finalization requires authoritative row-state composition.",
    )
}

fn invalid_plan() -> SdkError {
    SdkError::new(
        "CUSTOMER_DATA_IMPORT_CAPABILITY_PLAN_INVALID",
        ErrorCategory::Internal,
        false,
        "The customer-data import capability could not be planned safely.",
    )
}

fn unsupported_capability() -> SdkError {
    SdkError::new(
        "CUSTOMER_DATA_IMPORT_CAPABILITY_UNSUPPORTED",
        ErrorCategory::Internal,
        false,
        "The customer-data import capability is not configured.",
    )
}
