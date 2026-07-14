#![forbid(unsafe_code)]

//! Production composition for import jobs bound to immutable finalized source bytes.
//!
//! The source artifact is read through `crm-core-files`, parsed by the strict versioned import
//! parser, and converted to import-owned job/row mutations. Any read/parse work occurs before a
//! second live authorization decision; no awaited work occurs between that decision and the
//! atomic PostgreSQL evidence batch.

use crm_capability_plan_support::{self as support, EventSpec};
use crm_capability_runtime::{
    AuthorizationDecision, CapabilityAuthorizer, CapabilityDefinition, CapabilityExecutionResult,
    CapabilityRequest, CapabilityRisk, TransactionalCapabilityExecutor,
};
use crm_core_data::{
    BatchMutationPlan, FileArtifactCapabilityEvidence, FileArtifactCapabilityMutation,
    FileArtifactCapabilityMutationResult, PostgresDataStore, RecordGetQuery, RecordMutation,
    RelationshipMutation, batch_error_to_sdk,
};
use crm_core_files::{
    AppendImmutableFileChunk, CreateImmutableFileArtifact, FileArtifactMetadata,
    FileArtifactStatus, ImmutableFileArtifactStore,
};
use crm_customer_data_operations::{
    CreateImportJob, CreateImportRow, CreateValidatedImportRow, ExternalPartyIdentifierDigest,
    ImportCanonicalizationVersion, ImportHeaderMode, ImportJob, ImportJobId, ImportJobStatus,
    ImportParserProfile, ImportParserVersion, ImportRow, ImportSourceFormat, ImportTextEncoding,
    InitialImportRowValidation, PartialExecutionPolicy, PartyImportKind, PartyImportMapping,
    PreparedPartyRow, RecordImportValidationBatch, RowDiagnostic, RowIdentitySource,
    SourceDescriptor, SourceSystemId, TargetPartyId, create_validated_import_row,
    encode_import_job_state, encode_import_row_state, parse_import_source,
};
use crm_customer_data_operations_capability_adapter::{
    IMPORT_JOB_RECORD_TYPE, IMPORT_JOB_ROW_RELATIONSHIP_TYPE, IMPORT_ROW_RECORD_TYPE, MODULE_ID,
    PARTY_IMPORT_JOB_CREATED_EVENT_SCHEMA, PARTY_IMPORT_JOB_CREATED_EVENT_TYPE,
    PARTY_IMPORT_ROW_VALIDATED_EVENT_SCHEMA, PARTY_IMPORT_ROW_VALIDATED_EVENT_TYPE,
    PARTY_IMPORT_VALIDATION_PROGRESSED_EVENT_SCHEMA, PARTY_IMPORT_VALIDATION_PROGRESSED_EVENT_TYPE,
    import_job_from_snapshot, import_job_persisted_contract, import_row_persisted_contract,
    import_row_to_wire, job_to_wire,
};
use crm_module_sdk::{
    CapabilityId, CapabilityVersion, DataClass, ErrorCategory, FileId, ModuleId, PortFuture,
    RecordId, RecordRef, RecordType, RelationshipRef, RelationshipType, ResourceRef,
    RetentionPolicyId, SdkError, TypedPayload,
};
use crm_proto_contracts::crm::customer_data_operations::v1 as wire;
use std::collections::BTreeSet;
use std::fmt;
use std::sync::Arc;

pub const CREATE_SOURCE_ARTIFACT_CAPABILITY: &str = "customer_data.import.party.source.create";
pub const APPEND_SOURCE_CHUNK_CAPABILITY: &str = "customer_data.import.party.source.chunk.append";
pub const FINALIZE_SOURCE_ARTIFACT_CAPABILITY: &str = "customer_data.import.party.source.finalize";
pub const CREATE_JOB_FROM_SOURCE_CAPABILITY: &str = "customer_data.import.party.source.job.create";
pub const VALIDATE_SOURCE_BATCH_CAPABILITY: &str =
    "customer_data.import.party.source.rows.validate";

pub const CREATE_SOURCE_ARTIFACT_REQUEST_SCHEMA: &str =
    "crm.customer_data_operations.v1.CreatePartyImportSourceArtifactRequest";
pub const CREATE_SOURCE_ARTIFACT_RESPONSE_SCHEMA: &str =
    "crm.customer_data_operations.v1.CreatePartyImportSourceArtifactResponse";
pub const APPEND_SOURCE_CHUNK_REQUEST_SCHEMA: &str =
    "crm.customer_data_operations.v1.AppendPartyImportSourceChunkRequest";
pub const APPEND_SOURCE_CHUNK_RESPONSE_SCHEMA: &str =
    "crm.customer_data_operations.v1.AppendPartyImportSourceChunkResponse";
pub const FINALIZE_SOURCE_ARTIFACT_REQUEST_SCHEMA: &str =
    "crm.customer_data_operations.v1.FinalizePartyImportSourceArtifactRequest";
pub const FINALIZE_SOURCE_ARTIFACT_RESPONSE_SCHEMA: &str =
    "crm.customer_data_operations.v1.FinalizePartyImportSourceArtifactResponse";

pub const SOURCE_ARTIFACT_CREATED_EVENT_TYPE: &str = "customer_data.import.party.source.created";
pub const SOURCE_ARTIFACT_CREATED_EVENT_SCHEMA: &str =
    "crm.customer_data_operations.v1.PartyImportSourceArtifactCreatedEvent";
pub const SOURCE_CHUNK_APPENDED_EVENT_TYPE: &str =
    "customer_data.import.party.source.chunk_appended";
pub const SOURCE_CHUNK_APPENDED_EVENT_SCHEMA: &str =
    "crm.customer_data_operations.v1.PartyImportSourceChunkAppendedEvent";
pub const SOURCE_ARTIFACT_FINALIZED_EVENT_TYPE: &str =
    "customer_data.import.party.source.finalized";
pub const SOURCE_ARTIFACT_FINALIZED_EVENT_SCHEMA: &str =
    "crm.customer_data_operations.v1.PartyImportSourceArtifactFinalizedEvent";

pub const CREATE_JOB_FROM_SOURCE_REQUEST_SCHEMA: &str =
    "crm.customer_data_operations.v1.CreatePartyImportJobFromSourceArtifactRequest";
pub const CREATE_JOB_FROM_SOURCE_RESPONSE_SCHEMA: &str =
    "crm.customer_data_operations.v1.CreatePartyImportJobFromSourceArtifactResponse";
pub const VALIDATE_SOURCE_BATCH_REQUEST_SCHEMA: &str =
    "crm.customer_data_operations.v1.ValidatePartyImportSourceBatchRequest";
pub const VALIDATE_SOURCE_BATCH_RESPONSE_SCHEMA: &str =
    "crm.customer_data_operations.v1.ValidatePartyImportSourceBatchResponse";

pub const SOURCE_MUTATION_CAPABILITY_IDS: [&str; 5] = [
    CREATE_SOURCE_ARTIFACT_CAPABILITY,
    APPEND_SOURCE_CHUNK_CAPABILITY,
    FINALIZE_SOURCE_ARTIFACT_CAPABILITY,
    CREATE_JOB_FROM_SOURCE_CAPABILITY,
    VALIDATE_SOURCE_BATCH_CAPABILITY,
];

pub const MAXIMUM_SOURCE_VALIDATION_BATCH_ROWS: usize = 500;

pub fn source_capability_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {
    SOURCE_MUTATION_CAPABILITY_IDS
        .into_iter()
        .map(source_capability_definition)
        .collect()
}

pub fn source_capability_definition(capability_id: &str) -> Result<CapabilityDefinition, SdkError> {
    let (input_schema, output_schema, risk) = match capability_id {
        CREATE_SOURCE_ARTIFACT_CAPABILITY => (
            CREATE_SOURCE_ARTIFACT_REQUEST_SCHEMA,
            CREATE_SOURCE_ARTIFACT_RESPONSE_SCHEMA,
            CapabilityRisk::High,
        ),
        APPEND_SOURCE_CHUNK_CAPABILITY => (
            APPEND_SOURCE_CHUNK_REQUEST_SCHEMA,
            APPEND_SOURCE_CHUNK_RESPONSE_SCHEMA,
            CapabilityRisk::High,
        ),
        FINALIZE_SOURCE_ARTIFACT_CAPABILITY => (
            FINALIZE_SOURCE_ARTIFACT_REQUEST_SCHEMA,
            FINALIZE_SOURCE_ARTIFACT_RESPONSE_SCHEMA,
            CapabilityRisk::High,
        ),
        CREATE_JOB_FROM_SOURCE_CAPABILITY => (
            CREATE_JOB_FROM_SOURCE_REQUEST_SCHEMA,
            CREATE_JOB_FROM_SOURCE_RESPONSE_SCHEMA,
            CapabilityRisk::High,
        ),
        VALIDATE_SOURCE_BATCH_CAPABILITY => (
            VALIDATE_SOURCE_BATCH_REQUEST_SCHEMA,
            VALIDATE_SOURCE_BATCH_RESPONSE_SCHEMA,
            CapabilityRisk::Medium,
        ),
        _ => return Err(configuration_error("unsupported source capability")),
    };
    Ok(CapabilityDefinition {
        capability_id: CapabilityId::try_new(capability_id)
            .map_err(identifier_configuration_error)?,
        capability_version: CapabilityVersion::try_new(support::CONTRACT_VERSION)
            .map_err(identifier_configuration_error)?,
        owner_module_id: ModuleId::try_new(MODULE_ID).map_err(identifier_configuration_error)?,
        input_contract: support::protobuf_contract(
            MODULE_ID,
            input_schema,
            vec![DataClass::Personal],
        )?,
        output_contract: Some(support::protobuf_contract(
            MODULE_ID,
            output_schema,
            vec![DataClass::Personal],
        )?),
        risk,
        mutation: true,
        requires_idempotency: true,
        requires_approval: false,
        authorization_policy_id: capability_id.to_owned(),
        rate_limit_policy_id: None,
    })
}

#[derive(Clone)]
pub struct CustomerDataOperationsSourceExecutor {
    store: PostgresDataStore,
    artifacts: Arc<dyn ImmutableFileArtifactStore>,
    authorizer: Arc<dyn CapabilityAuthorizer>,
}

impl CustomerDataOperationsSourceExecutor {
    pub fn new(
        store: PostgresDataStore,
        artifacts: Arc<dyn ImmutableFileArtifactStore>,
        authorizer: Arc<dyn CapabilityAuthorizer>,
    ) -> Self {
        Self {
            store,
            artifacts,
            authorizer,
        }
    }
}

impl fmt::Debug for CustomerDataOperationsSourceExecutor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CustomerDataOperationsSourceExecutor")
            .field("store", &self.store)
            .field("artifacts", &"dyn ImmutableFileArtifactStore")
            .field("authorizer", &"dyn CapabilityAuthorizer")
            .finish()
    }
}

impl TransactionalCapabilityExecutor for CustomerDataOperationsSourceExecutor {
    fn execute<'a>(
        &'a self,
        definition: &'a CapabilityDefinition,
        request: CapabilityRequest,
    ) -> PortFuture<'a, Result<CapabilityExecutionResult, SdkError>> {
        Box::pin(async move {
            ensure_definition(definition, &request)?;
            match definition.capability_id.as_str() {
                CREATE_SOURCE_ARTIFACT_CAPABILITY => {
                    self.create_source_artifact(definition, request).await
                }
                APPEND_SOURCE_CHUNK_CAPABILITY => {
                    self.append_source_chunk(definition, request).await
                }
                FINALIZE_SOURCE_ARTIFACT_CAPABILITY => {
                    self.finalize_source_artifact(definition, request).await
                }
                CREATE_JOB_FROM_SOURCE_CAPABILITY => {
                    self.create_job_from_source(definition, request).await
                }
                VALIDATE_SOURCE_BATCH_CAPABILITY => {
                    self.validate_source_batch(definition, request).await
                }
                _ => Err(configuration_error("unsupported source capability")),
            }
        })
    }
}

impl CustomerDataOperationsSourceExecutor {
    async fn create_source_artifact(
        &self,
        definition: &CapabilityDefinition,
        request: CapabilityRequest,
    ) -> Result<CapabilityExecutionResult, SdkError> {
        let command: wire::CreatePartyImportSourceArtifactRequest =
            support::decode_request_with_data_class(
                &request,
                MODULE_ID,
                CREATE_SOURCE_ARTIFACT_REQUEST_SCHEMA,
                DataClass::Personal,
            )?;
        let file_id = artifact_id_from_ref(command.source_artifact_ref)?;
        let expected_sha256 = sha256_array(
            &command.expected_sha256,
            "customer_data.import.source_artifact.expected_sha256",
        )?;
        let mutation = FileArtifactCapabilityMutation::Create(CreateImmutableFileArtifact {
            file_id,
            owner_module_id: ModuleId::try_new(MODULE_ID)
                .map_err(identifier_configuration_error)?,
            media_type: "text/csv".to_owned(),
            data_class: DataClass::Personal,
            retention_policy_id: RetentionPolicyId::try_new("crm.customer_data.import_source")
                .map_err(identifier_configuration_error)?,
            expected_size_bytes: command.expected_size_bytes,
            expected_sha256,
        });
        self.store
            .execute_file_artifact_capability(definition, request, mutation, |result, request| {
                source_artifact_create_evidence(definition, result, request)
            })
            .await
            .map_err(batch_error_to_sdk)
    }

    async fn append_source_chunk(
        &self,
        definition: &CapabilityDefinition,
        request: CapabilityRequest,
    ) -> Result<CapabilityExecutionResult, SdkError> {
        let command: wire::AppendPartyImportSourceChunkRequest =
            support::decode_request_with_data_class(
                &request,
                MODULE_ID,
                APPEND_SOURCE_CHUNK_REQUEST_SCHEMA,
                DataClass::Personal,
            )?;
        let file_id = artifact_id_from_ref(command.source_artifact_ref)?;
        let chunk_index = command.chunk_index;
        let mutation = FileArtifactCapabilityMutation::AppendChunk(AppendImmutableFileChunk {
            file_id,
            chunk_index,
            chunk_sha256: sha256_array(
                &command.chunk_sha256,
                "customer_data.import.source_artifact.chunk_sha256",
            )?,
            bytes: command.chunk_bytes,
        });
        self.store
            .execute_file_artifact_capability(definition, request, mutation, |result, request| {
                source_chunk_append_evidence(definition, result, request, chunk_index)
            })
            .await
            .map_err(batch_error_to_sdk)
    }

    async fn finalize_source_artifact(
        &self,
        definition: &CapabilityDefinition,
        request: CapabilityRequest,
    ) -> Result<CapabilityExecutionResult, SdkError> {
        let command: wire::FinalizePartyImportSourceArtifactRequest =
            support::decode_request_with_data_class(
                &request,
                MODULE_ID,
                FINALIZE_SOURCE_ARTIFACT_REQUEST_SCHEMA,
                DataClass::Personal,
            )?;
        let mutation = FileArtifactCapabilityMutation::Finalize {
            file_id: artifact_id_from_ref(command.source_artifact_ref)?,
        };
        self.store
            .execute_file_artifact_capability(definition, request, mutation, |result, request| {
                source_artifact_finalize_evidence(definition, result, request)
            })
            .await
            .map_err(batch_error_to_sdk)
    }

    async fn create_job_from_source(
        &self,
        definition: &CapabilityDefinition,
        request: CapabilityRequest,
    ) -> Result<CapabilityExecutionResult, SdkError> {
        let command: wire::CreatePartyImportJobFromSourceArtifactRequest =
            support::decode_request_with_data_class(
                &request,
                MODULE_ID,
                CREATE_JOB_FROM_SOURCE_REQUEST_SCHEMA,
                DataClass::Personal,
            )?;
        let job_id = import_job_id_from_ref(command.import_job_ref)?;
        let artifact_id = artifact_id_from_ref(command.source_artifact_ref)?;
        let parser_profile = parser_profile_from_wire(command.parser_profile)?;
        let artifact = self
            .artifacts
            .read_finalized(&request.context, &artifact_id)
            .await?;
        if artifact.metadata.status != FileArtifactStatus::Finalized {
            return Err(source_conflict(
                "CUSTOMER_DATA_IMPORT_SOURCE_ARTIFACT_NOT_FINALIZED",
                "The import source artifact is not finalized.",
            ));
        }
        let parsed = parse_import_source(&artifact.bytes, &parser_profile)?;
        let source = SourceDescriptor::try_new_bound(
            artifact_id.clone(),
            command.source_name,
            hex_sha256(artifact.metadata.expected_sha256),
            parsed.row_count(),
            SourceSystemId::try_new(command.source_system_id)?,
            parser_profile,
        )?;
        let job = ImportJob::create(CreateImportJob {
            job_id,
            source,
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
            CREATE_JOB_FROM_SOURCE_RESPONSE_SCHEMA,
            DataClass::Personal,
            &wire::CreatePartyImportJobFromSourceArtifactResponse {
                import_job: Some(public_job.clone()),
                source_artifact_ref: Some(wire::PartyImportSourceArtifactRef {
                    file_id: artifact_id.as_str().to_owned(),
                }),
            },
        )?;
        let event = support::event_evidence_with_data_class(
            &request,
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
        let plan = BatchMutationPlan {
            context: request.context.clone(),
            records: vec![RecordMutation::Create {
                reference: aggregate.clone(),
                payload: import_job_persisted_payload(&job)?,
            }],
            relationships: Vec::new(),
            events: vec![event.clone()],
            idempotency: support::capability_idempotency(definition, &request)?,
            audits: vec![support::audit_intent(
                &request,
                &aggregate,
                job.version(),
                definition.capability_id.as_str(),
                &output.bytes,
            )?],
        };

        reauthorize(&*self.authorizer, definition, &request).await?;
        let result = self
            .store
            .execute_batch(&plan)
            .await
            .map_err(batch_error_to_sdk)?;
        Ok(CapabilityExecutionResult {
            output: Some(output),
            affected_resources: result
                .records
                .iter()
                .map(|record| crm_module_sdk::ResourceRef {
                    resource_type: record.reference.record_type.as_str().to_owned(),
                    resource_id: record.reference.record_id.as_str().to_owned(),
                    version: Some(record.version),
                })
                .collect(),
            replayed: result.replayed,
        })
    }

    async fn validate_source_batch(
        &self,
        definition: &CapabilityDefinition,
        request: CapabilityRequest,
    ) -> Result<CapabilityExecutionResult, SdkError> {
        let command: wire::ValidatePartyImportSourceBatchRequest =
            support::decode_request_with_data_class(
                &request,
                MODULE_ID,
                VALIDATE_SOURCE_BATCH_REQUEST_SCHEMA,
                DataClass::Personal,
            )?;
        if command.max_rows == 0
            || usize::try_from(command.max_rows).unwrap_or(usize::MAX)
                > MAXIMUM_SOURCE_VALIDATION_BATCH_ROWS
        {
            return Err(SdkError::invalid_argument(
                "customer_data.import.source_batch.max_rows",
                format!(
                    "Source validation batch must contain between 1 and {MAXIMUM_SOURCE_VALIDATION_BATCH_ROWS} rows"
                ),
            ));
        }
        let job_id = import_job_id_from_ref(command.import_job_ref)?;
        let current = self
            .store
            .get_record_for_query(&RecordGetQuery {
                tenant_id: request.context.execution.tenant_id.clone(),
                owner_module_id: ModuleId::try_new(MODULE_ID)
                    .map_err(identifier_configuration_error)?,
                record_type: RecordType::try_new(IMPORT_JOB_RECORD_TYPE)
                    .map_err(identifier_configuration_error)?,
                record_id: RecordId::try_new(job_id.as_str())
                    .map_err(identifier_configuration_error)?,
            })
            .await?
            .ok_or_else(|| source_not_found("The import job was not found."))?;
        let job = import_job_from_snapshot(&current)?;
        if job.status() != ImportJobStatus::Created {
            return Err(source_conflict(
                "CUSTOMER_DATA_IMPORT_JOB_NOT_CREATED",
                "Only a created import job can accept source validation.",
            ));
        }
        let artifact_id = job.source().source_artifact_id().cloned().ok_or_else(|| {
            source_conflict(
                "CUSTOMER_DATA_IMPORT_SOURCE_ARTIFACT_BINDING_MISSING",
                "The import job is not bound to an immutable source artifact.",
            )
        })?;
        let artifact = self
            .artifacts
            .read_finalized(&request.context, &artifact_id)
            .await?;
        if hex_sha256(artifact.metadata.expected_sha256) != job.source().content_sha256() {
            return Err(source_conflict(
                "CUSTOMER_DATA_IMPORT_SOURCE_ARTIFACT_DIGEST_MISMATCH",
                "The finalized source artifact no longer matches the import job binding.",
            ));
        }
        let parsed = parse_import_source(&artifact.bytes, job.source().parser_profile())?;
        if parsed.row_count() != job.source().row_count() {
            return Err(source_conflict(
                "CUSTOMER_DATA_IMPORT_SOURCE_ARTIFACT_ROW_COUNT_MISMATCH",
                "The finalized source artifact no longer matches the import job row count.",
            ));
        }
        let source_rows = parsed.rows_inclusive_from(
            command.start_row_position,
            usize::try_from(command.max_rows).map_err(|_| invalid_plan())?,
        )?;
        if source_rows.is_empty() {
            return Err(SdkError::invalid_argument(
                "customer_data.import.source_batch.start_row_position",
                "Source validation batch does not contain any rows",
            ));
        }

        let occurred_at = request.context.execution.request_started_at_unix_nanos;
        let mut seen_positions = BTreeSet::new();
        let mut seen_row_ids = BTreeSet::new();
        let mut seen_target_party_ids = BTreeSet::new();
        let mut public_rows = Vec::with_capacity(source_rows.len());
        let mut records = Vec::with_capacity(source_rows.len() + 1);
        let mut relationships = Vec::with_capacity(source_rows.len());
        let mut events = Vec::with_capacity(source_rows.len() + 1);

        for source_row in source_rows {
            let row = validate_source_row(
                &job,
                source_row.row_position,
                &source_row.columns,
                occurred_at,
                &mut seen_positions,
                &mut seen_row_ids,
                &mut seen_target_party_ids,
            )?;
            let aggregate = import_row_record_ref(&row)?;
            let public_row = import_row_to_wire(&row)?;
            records.push(RecordMutation::Create {
                reference: aggregate.clone(),
                payload: import_row_persisted_payload(&row)?,
            });
            let import_row_ref = public_row.import_row_ref.clone().ok_or_else(invalid_plan)?;
            relationships.push(RelationshipMutation::Link {
                relationship: RelationshipRef {
                    relationship_type: RelationshipType::try_new(IMPORT_JOB_ROW_RELATIONSHIP_TYPE)
                        .map_err(identifier_configuration_error)?,
                    source: current.reference.clone(),
                    target: aggregate.clone(),
                },
                payload: support::protobuf_payload(
                    MODULE_ID,
                    "crm.customer_data_operations.v1.ImportRowRef",
                    DataClass::Personal,
                    &import_row_ref,
                )?,
            });
            events.push(support::event_evidence_with_data_class(
                &request,
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
            public_rows.push(public_row);
        }

        let valid_rows = public_rows
            .iter()
            .filter(|row| row.status == wire::ImportRowStatus::Valid as i32)
            .count();
        let invalid_rows = public_rows
            .iter()
            .filter(|row| row.status == wire::ImportRowStatus::Invalid as i32)
            .count();
        let mut progressed_job = job.clone();
        progressed_job.record_validation_batch(RecordImportValidationBatch {
            expected_version: current.version,
            valid_rows: u32::try_from(valid_rows).map_err(|_| invalid_plan())?,
            invalid_rows: u32::try_from(invalid_rows).map_err(|_| invalid_plan())?,
            occurred_at_unix_nanos: occurred_at,
        })?;
        records.insert(
            0,
            RecordMutation::Update {
                reference: current.reference.clone(),
                expected_version: current.version,
                payload: import_job_persisted_payload(&progressed_job)?,
            },
        );
        events.push(support::event_evidence_with_data_class(
            &request,
            current.reference.clone(),
            MODULE_ID,
            EventSpec {
                event_type: PARTY_IMPORT_VALIDATION_PROGRESSED_EVENT_TYPE,
                event_schema_id: PARTY_IMPORT_VALIDATION_PROGRESSED_EVENT_SCHEMA,
                aggregate_version: progressed_job.version(),
                previous_version: Some(current.version),
            },
            DataClass::Personal,
            &wire::PartyImportValidationProgressedEvent {
                import_job: Some(job_to_wire(&progressed_job)?),
            },
        )?);

        let next_row_position = source_rows
            .last()
            .and_then(|row| row.row_position.checked_add(1))
            .ok_or_else(invalid_plan)?;
        let source_exhausted = next_row_position > job.source().row_count();
        let output = support::protobuf_payload(
            MODULE_ID,
            VALIDATE_SOURCE_BATCH_RESPONSE_SCHEMA,
            DataClass::Personal,
            &wire::ValidatePartyImportSourceBatchResponse {
                import_rows: public_rows,
                next_row_position,
                source_exhausted,
            },
        )?;
        let audits = events
            .iter()
            .map(|event| {
                support::audit_intent(
                    &request,
                    &event.event.aggregate,
                    event.aggregate_version,
                    definition.capability_id.as_str(),
                    &output.bytes,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let plan = BatchMutationPlan {
            context: request.context.clone(),
            records,
            relationships,
            events,
            idempotency: support::capability_idempotency(definition, &request)?,
            audits,
        };

        reauthorize(&*self.authorizer, definition, &request).await?;
        let result = self
            .store
            .execute_batch(&plan)
            .await
            .map_err(batch_error_to_sdk)?;
        Ok(CapabilityExecutionResult {
            output: Some(output),
            affected_resources: result
                .records
                .iter()
                .map(|record| crm_module_sdk::ResourceRef {
                    resource_type: record.reference.record_type.as_str().to_owned(),
                    resource_id: record.reference.record_id.as_str().to_owned(),
                    version: Some(record.version),
                })
                .collect(),
            replayed: result.replayed,
        })
    }
}

fn source_artifact_create_evidence(
    definition: &CapabilityDefinition,
    result: &FileArtifactCapabilityMutationResult,
    request: &CapabilityRequest,
) -> Result<FileArtifactCapabilityEvidence, SdkError> {
    let public = source_artifact_to_wire(&result.metadata);
    let output = support::protobuf_payload(
        MODULE_ID,
        CREATE_SOURCE_ARTIFACT_RESPONSE_SCHEMA,
        DataClass::Personal,
        &wire::CreatePartyImportSourceArtifactResponse {
            source_artifact: Some(public.clone()),
        },
    )?;
    let event = if result.changed {
        Some(support::event_evidence_with_data_class(
            request,
            file_artifact_record_ref(&result.metadata)?,
            MODULE_ID,
            EventSpec {
                event_type: SOURCE_ARTIFACT_CREATED_EVENT_TYPE,
                event_schema_id: SOURCE_ARTIFACT_CREATED_EVENT_SCHEMA,
                aggregate_version: file_artifact_version(&result.metadata)?,
                previous_version: None,
            },
            DataClass::Personal,
            &wire::PartyImportSourceArtifactCreatedEvent {
                source_artifact: Some(public),
            },
        )?)
    } else {
        None
    };
    file_artifact_evidence(definition, result, request, output, event)
}

fn source_chunk_append_evidence(
    definition: &CapabilityDefinition,
    result: &FileArtifactCapabilityMutationResult,
    request: &CapabilityRequest,
    chunk_index: u64,
) -> Result<FileArtifactCapabilityEvidence, SdkError> {
    let public = source_artifact_to_wire(&result.metadata);
    let output = support::protobuf_payload(
        MODULE_ID,
        APPEND_SOURCE_CHUNK_RESPONSE_SCHEMA,
        DataClass::Personal,
        &wire::AppendPartyImportSourceChunkResponse {
            source_artifact: Some(public.clone()),
            replayed: result.chunk_replayed,
        },
    )?;
    let event = if result.changed {
        let version = file_artifact_version(&result.metadata)?;
        Some(support::event_evidence_with_data_class(
            request,
            file_artifact_record_ref(&result.metadata)?,
            MODULE_ID,
            EventSpec {
                event_type: SOURCE_CHUNK_APPENDED_EVENT_TYPE,
                event_schema_id: SOURCE_CHUNK_APPENDED_EVENT_SCHEMA,
                aggregate_version: version,
                previous_version: version.checked_sub(1),
            },
            DataClass::Personal,
            &wire::PartyImportSourceChunkAppendedEvent {
                source_artifact: Some(public),
                chunk_index,
            },
        )?)
    } else {
        None
    };
    file_artifact_evidence(definition, result, request, output, event)
}

fn source_artifact_finalize_evidence(
    definition: &CapabilityDefinition,
    result: &FileArtifactCapabilityMutationResult,
    request: &CapabilityRequest,
) -> Result<FileArtifactCapabilityEvidence, SdkError> {
    let public = source_artifact_to_wire(&result.metadata);
    let output = support::protobuf_payload(
        MODULE_ID,
        FINALIZE_SOURCE_ARTIFACT_RESPONSE_SCHEMA,
        DataClass::Personal,
        &wire::FinalizePartyImportSourceArtifactResponse {
            source_artifact: Some(public.clone()),
        },
    )?;
    let event = if result.changed {
        let version = file_artifact_version(&result.metadata)?;
        Some(support::event_evidence_with_data_class(
            request,
            file_artifact_record_ref(&result.metadata)?,
            MODULE_ID,
            EventSpec {
                event_type: SOURCE_ARTIFACT_FINALIZED_EVENT_TYPE,
                event_schema_id: SOURCE_ARTIFACT_FINALIZED_EVENT_SCHEMA,
                aggregate_version: version,
                previous_version: version.checked_sub(1),
            },
            DataClass::Personal,
            &wire::PartyImportSourceArtifactFinalizedEvent {
                source_artifact: Some(public),
            },
        )?)
    } else {
        None
    };
    file_artifact_evidence(definition, result, request, output, event)
}

fn file_artifact_evidence(
    definition: &CapabilityDefinition,
    result: &FileArtifactCapabilityMutationResult,
    request: &CapabilityRequest,
    output: TypedPayload,
    event: Option<crm_core_data::EventEvidence>,
) -> Result<FileArtifactCapabilityEvidence, SdkError> {
    let aggregate = file_artifact_record_ref(&result.metadata)?;
    let version = file_artifact_version(&result.metadata)?;
    Ok(FileArtifactCapabilityEvidence {
        output: output.clone(),
        events: event.into_iter().collect(),
        audits: vec![support::audit_intent(
            request,
            &aggregate,
            version,
            definition.capability_id.as_str(),
            &output.bytes,
        )?],
        affected_resources: vec![ResourceRef {
            resource_type: "file_artifact".to_owned(),
            resource_id: result.metadata.file_id.as_str().to_owned(),
            version: Some(version),
        }],
    })
}

fn file_artifact_record_ref(metadata: &FileArtifactMetadata) -> Result<RecordRef, SdkError> {
    support::record_ref(
        "file_artifact",
        metadata.file_id.as_str(),
        "customer_data.import.source_artifact_ref.file_id",
    )
}

fn source_artifact_to_wire(metadata: &FileArtifactMetadata) -> wire::PartyImportSourceArtifact {
    wire::PartyImportSourceArtifact {
        source_artifact_ref: Some(wire::PartyImportSourceArtifactRef {
            file_id: metadata.file_id.as_str().to_owned(),
        }),
        expected_size_bytes: metadata.expected_size_bytes,
        expected_sha256: metadata.expected_sha256.to_vec(),
        received_size_bytes: metadata.received_size_bytes,
        next_chunk_index: metadata.next_chunk_index,
        finalized: metadata.status == FileArtifactStatus::Finalized,
    }
}

fn file_artifact_version(metadata: &FileArtifactMetadata) -> Result<i64, SdkError> {
    let base = metadata
        .next_chunk_index
        .checked_add(1)
        .and_then(|value| {
            if metadata.status == FileArtifactStatus::Finalized {
                value.checked_add(1)
            } else {
                Some(value)
            }
        })
        .ok_or_else(invalid_plan)?;
    i64::try_from(base).map_err(|_| invalid_plan())
}

fn sha256_array(bytes: &[u8], field: &'static str) -> Result<[u8; 32], SdkError> {
    bytes
        .try_into()
        .map_err(|_| SdkError::invalid_argument(field, "SHA-256 must contain exactly 32 bytes"))
}

fn validate_source_row(
    job: &ImportJob,
    row_position: u32,
    columns: &std::collections::BTreeMap<String, String>,
    occurred_at: i64,
    seen_positions: &mut BTreeSet<u32>,
    seen_row_ids: &mut BTreeSet<crm_customer_data_operations::ImportRowId>,
    seen_target_party_ids: &mut BTreeSet<TargetPartyId>,
) -> Result<ImportRow, SdkError> {
    if row_position == 0 || row_position > job.source().row_count() {
        return Err(SdkError::invalid_argument(
            "customer_data.import.source_batch.row_position",
            "Row position must be inside the immutable source row range",
        ));
    }
    if !seen_positions.insert(row_position) {
        return Err(SdkError::invalid_argument(
            "customer_data.import.source_batch.row_position",
            "Source validation batch contains a duplicate row position",
        ));
    }

    let mut diagnostics = Vec::new();
    let external_row_key = optional_mapped_value(
        columns,
        job.mapping().external_row_key_column(),
        "EXTERNAL_ROW_KEY_MISSING",
        &mut diagnostics,
    );
    let external_row_key = match external_row_key {
        Some(value) => match RowIdentitySource::for_row(row_position, Some(&value)) {
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
        },
        None => None,
    };
    let source_external_id = optional_mapped_value(
        columns,
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
    let probe = ImportRow::create(CreateImportRow {
        job_id: job.job_id().clone(),
        row_position,
        external_row_key: external_row_key.clone(),
        source_external_id: source_external_id.clone(),
        occurred_at_unix_nanos: occurred_at,
    })?;
    if !seen_row_ids.insert(probe.row_id().clone()) {
        return Err(SdkError::invalid_argument(
            "customer_data.import.source_batch.external_row_key",
            "Source validation batch resolves more than one row to the same deterministic identity",
        ));
    }

    let target_party_id =
        target_party_id_for_row(columns, job.mapping(), &probe, &mut diagnostics)?;
    let party_kind = party_kind_for_row(columns, job.mapping(), &mut diagnostics)?;
    let display_name = required_mapped_value(
        columns,
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
    create_validated_import_row(CreateValidatedImportRow {
        job_id: job.job_id().clone(),
        row_position,
        external_row_key,
        source_external_id,
        outcome,
        occurred_at_unix_nanos: occurred_at,
    })
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
        _ => {
            return Err(SdkError::invalid_argument(
                "customer_data.import.source.parser_profile.format",
                "Import source format must be CSV",
            ));
        }
    };
    let encoding = match wire::ImportTextEncoding::try_from(value.encoding) {
        Ok(wire::ImportTextEncoding::Utf8) => ImportTextEncoding::Utf8,
        _ => {
            return Err(SdkError::invalid_argument(
                "customer_data.import.source.parser_profile.encoding",
                "Import source encoding must be UTF8",
            ));
        }
    };
    let header_mode = match wire::ImportHeaderMode::try_from(value.header_mode) {
        Ok(wire::ImportHeaderMode::RequiredFirstRow) => ImportHeaderMode::RequiredFirstRow,
        _ => {
            return Err(SdkError::invalid_argument(
                "customer_data.import.source.parser_profile.header_mode",
                "Import header mode must be REQUIRED_FIRST_ROW",
            ));
        }
    };
    let parser_version = match wire::ImportParserVersion::try_from(value.parser_version) {
        Ok(wire::ImportParserVersion::CsvV1) => ImportParserVersion::CsvV1,
        _ => {
            return Err(SdkError::invalid_argument(
                "customer_data.import.source.parser_profile.parser_version",
                "Import parser version must be CSV_V1",
            ));
        }
    };
    let canonicalization_version =
        match wire::ImportCanonicalizationVersion::try_from(value.canonicalization_version) {
            Ok(wire::ImportCanonicalizationVersion::V1) => ImportCanonicalizationVersion::V1,
            _ => {
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

fn partial_execution_policy_from_wire(value: i32) -> Result<PartialExecutionPolicy, SdkError> {
    match wire::PartialExecutionPolicy::try_from(value) {
        Ok(wire::PartialExecutionPolicy::AllValidRows) => Ok(PartialExecutionPolicy::AllValidRows),
        Ok(wire::PartialExecutionPolicy::RequireAllValid) => {
            Ok(PartialExecutionPolicy::RequireAllValid)
        }
        _ => Err(SdkError::invalid_argument(
            "customer_data.import.partial_execution_policy",
            "Partial execution policy must be ALL_VALID_ROWS or REQUIRE_ALL_VALID",
        )),
    }
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

fn artifact_id_from_ref(
    value: Option<wire::PartyImportSourceArtifactRef>,
) -> Result<FileId, SdkError> {
    let value = value.ok_or_else(|| {
        SdkError::invalid_argument(
            "customer_data.import.source_artifact_ref",
            "Import source artifact reference is required",
        )
    })?;
    FileId::try_new(value.file_id).map_err(identifier_configuration_error)
}

fn import_job_record_ref(job_id: &ImportJobId) -> Result<RecordRef, SdkError> {
    support::record_ref(
        IMPORT_JOB_RECORD_TYPE,
        job_id.as_str(),
        "customer_data.import_job_ref.import_job_id",
    )
}

fn import_row_record_ref(row: &ImportRow) -> Result<RecordRef, SdkError> {
    support::record_ref(
        IMPORT_ROW_RECORD_TYPE,
        row.row_id().as_str(),
        "customer_data.import_row_ref.import_row_id",
    )
}

fn import_job_persisted_payload(job: &ImportJob) -> Result<TypedPayload, SdkError> {
    support::persisted_json_payload_with_data_class(
        import_job_persisted_contract(),
        DataClass::Personal,
        encode_import_job_state(job)?,
    )
}

fn import_row_persisted_payload(row: &ImportRow) -> Result<TypedPayload, SdkError> {
    support::persisted_json_payload_with_data_class(
        import_row_persisted_contract(),
        DataClass::Personal,
        encode_import_row_state(row)?,
    )
}

async fn reauthorize(
    authorizer: &dyn CapabilityAuthorizer,
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
) -> Result<AuthorizationDecision, SdkError> {
    let decision = authorizer.authorize(definition, request).await?;
    if decision.allowed {
        Ok(decision)
    } else {
        Err(SdkError::new(
            "CAPABILITY_PERMISSION_DENIED",
            ErrorCategory::Authorization,
            false,
            "You are not permitted to perform this action.",
        )
        .with_internal_reference(format!(
            "decision_id={};reason_code={}",
            decision.decision_id, decision.reason_code
        )))
    }
}

fn ensure_definition(
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
) -> Result<(), SdkError> {
    if !SOURCE_MUTATION_CAPABILITY_IDS.contains(&definition.capability_id.as_str())
        || definition.owner_module_id.as_str() != MODULE_ID
        || definition.capability_id != request.context.execution.capability_id
    {
        return Err(configuration_error(
            "source capability definition binding is invalid",
        ));
    }
    Ok(())
}

fn hex_sha256(bytes: [u8; 32]) -> String {
    let mut value = String::with_capacity(64);
    for byte in bytes {
        use std::fmt::Write as _;
        write!(&mut value, "{byte:02x}").expect("writing to String cannot fail");
    }
    value
}

fn invalid_plan() -> SdkError {
    configuration_error("source capability plan is invalid")
}

fn identifier_configuration_error(error: crm_module_sdk::IdentifierError) -> SdkError {
    configuration_error(&error.to_string())
}

fn configuration_error(reference: &str) -> SdkError {
    SdkError::new(
        "CUSTOMER_DATA_IMPORT_SOURCE_CONFIGURATION_INVALID",
        ErrorCategory::Internal,
        false,
        "The customer-data import source capability is not configured safely.",
    )
    .with_internal_reference(reference.to_owned())
}

fn source_conflict(code: &'static str, safe_message: &'static str) -> SdkError {
    SdkError::new(code, ErrorCategory::Conflict, false, safe_message)
}

fn source_not_found(safe_message: &'static str) -> SdkError {
    SdkError::new(
        "CUSTOMER_DATA_IMPORT_SOURCE_NOT_FOUND",
        ErrorCategory::NotFound,
        false,
        safe_message,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn publishes_only_artifact_backed_job_create_and_validation_coordinates() {
        let definitions = source_capability_definitions().unwrap();
        assert_eq!(definitions.len(), SOURCE_MUTATION_CAPABILITY_IDS.len());
        assert_eq!(
            definitions
                .iter()
                .map(|definition| definition.capability_id.as_str())
                .collect::<Vec<_>>(),
            SOURCE_MUTATION_CAPABILITY_IDS
        );
        assert!(definitions.iter().all(|definition| definition.mutation));
        assert!(
            definitions
                .iter()
                .all(|definition| definition.requires_idempotency)
        );
    }
}
