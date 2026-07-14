use crm_capability_plan_support::{self as support, EventSpec, PersistedPayloadContract};
use crm_capability_runtime::{CapabilityDefinition, CapabilityRequest, CapabilityRisk};
use crm_core_data::{
    AggregatePresence, AggregateTarget, BatchMutationPlan, CapabilityBatchExecutionPlan,
    RecordMutation, TransactionalAggregatePlanner,
};
use crm_customer_data_operations::{
    EXPORT_JOB_STATE_MAXIMUM_BYTES, EXPORT_JOB_STATE_RETENTION_POLICY_ID, EXPORT_JOB_STATE_SCHEMA_ID,
    EXPORT_JOB_STATE_SCHEMA_VERSION, ExportJobId, PartyExportField, PartyExportJob,
    PartyExportJobStatus, PartyExportKindFilter, PartyExportProfile, PartyExportScope,
    PartyExportSpecification, decode_export_job_state, encode_export_job_state,
    export_job_state_descriptor_hash,
};
use crm_module_sdk::{
    CapabilityId, CapabilityVersion, DataClass, ErrorCategory, ModuleId, RecordSnapshot, SdkError,
};
use crm_proto_contracts::crm::{
    core::v1 as core, customer::v1 as customer, customer_data_operations::v1 as wire,
    parties::v1 as parties,
};

use crate::MODULE_ID;

pub const EXPORT_JOB_RECORD_TYPE: &str = "customer_data.export_job";

pub const CREATE_PARTY_EXPORT_JOB_CAPABILITY: &str = "customer_data.export.party.create";
pub const START_PARTY_EXPORT_EXECUTION_CAPABILITY: &str =
    "customer_data.export.party.execution.start";
pub const CANCEL_PARTY_EXPORT_JOB_CAPABILITY: &str = "customer_data.export.party.cancel";

pub const CREATE_PARTY_EXPORT_JOB_REQUEST_SCHEMA: &str =
    "crm.customer_data_operations.v1.CreatePartyExportJobRequest";
pub const CREATE_PARTY_EXPORT_JOB_RESPONSE_SCHEMA: &str =
    "crm.customer_data_operations.v1.CreatePartyExportJobResponse";
pub const START_PARTY_EXPORT_EXECUTION_REQUEST_SCHEMA: &str =
    "crm.customer_data_operations.v1.StartPartyExportExecutionRequest";
pub const START_PARTY_EXPORT_EXECUTION_RESPONSE_SCHEMA: &str =
    "crm.customer_data_operations.v1.StartPartyExportExecutionResponse";
pub const CANCEL_PARTY_EXPORT_JOB_REQUEST_SCHEMA: &str =
    "crm.customer_data_operations.v1.CancelPartyExportJobRequest";
pub const CANCEL_PARTY_EXPORT_JOB_RESPONSE_SCHEMA: &str =
    "crm.customer_data_operations.v1.CancelPartyExportJobResponse";

pub const PARTY_EXPORT_JOB_CREATED_EVENT_TYPE: &str = "customer_data.export.party.created";
pub const PARTY_EXPORT_JOB_CREATED_EVENT_SCHEMA: &str =
    "crm.customer_data_operations.v1.PartyExportJobCreatedEvent";
pub const PARTY_EXPORT_EXECUTION_STARTED_EVENT_TYPE: &str =
    "customer_data.export.party.execution_started";
pub const PARTY_EXPORT_EXECUTION_STARTED_EVENT_SCHEMA: &str =
    "crm.customer_data_operations.v1.PartyExportExecutionStartedEvent";
pub const PARTY_EXPORT_CANCELLED_EVENT_TYPE: &str = "customer_data.export.party.cancelled";
pub const PARTY_EXPORT_CANCELLED_EVENT_SCHEMA: &str =
    "crm.customer_data_operations.v1.PartyExportCancelledEvent";

pub const EXPORT_MUTATION_CAPABILITY_IDS: [&str; 3] = [
    CREATE_PARTY_EXPORT_JOB_CAPABILITY,
    START_PARTY_EXPORT_EXECUTION_CAPABILITY,
    CANCEL_PARTY_EXPORT_JOB_CAPABILITY,
];

const PARTY_EXPORT_MEDIA_TYPE: &str = "text/csv; charset=utf-8";

#[derive(Debug, Default, Clone, Copy)]
pub struct PartyExportCapabilityPlanner;

pub fn export_capability_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {
    EXPORT_MUTATION_CAPABILITY_IDS
        .into_iter()
        .map(export_capability_definition)
        .collect()
}

pub fn export_capability_definition(
    capability_id: &str,
) -> Result<CapabilityDefinition, SdkError> {
    let (input_schema, output_schema, risk) = match capability_id {
        CREATE_PARTY_EXPORT_JOB_CAPABILITY => (
            CREATE_PARTY_EXPORT_JOB_REQUEST_SCHEMA,
            CREATE_PARTY_EXPORT_JOB_RESPONSE_SCHEMA,
            CapabilityRisk::Medium,
        ),
        START_PARTY_EXPORT_EXECUTION_CAPABILITY => (
            START_PARTY_EXPORT_EXECUTION_REQUEST_SCHEMA,
            START_PARTY_EXPORT_EXECUTION_RESPONSE_SCHEMA,
            CapabilityRisk::High,
        ),
        CANCEL_PARTY_EXPORT_JOB_CAPABILITY => (
            CANCEL_PARTY_EXPORT_JOB_REQUEST_SCHEMA,
            CANCEL_PARTY_EXPORT_JOB_RESPONSE_SCHEMA,
            CapabilityRisk::Medium,
        ),
        _ => return Err(unsupported_capability()),
    };
    Ok(CapabilityDefinition {
        capability_id: configured(CapabilityId::try_new(capability_id))?,
        capability_version: configured(CapabilityVersion::try_new(support::CONTRACT_VERSION))?,
        owner_module_id: configured(ModuleId::try_new(MODULE_ID))?,
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

impl TransactionalAggregatePlanner for PartyExportCapabilityPlanner {
    fn target(
        &self,
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
    ) -> Result<AggregateTarget, SdkError> {
        ensure_definition(definition, request)?;
        let (job_id, presence) = match definition.capability_id.as_str() {
            CREATE_PARTY_EXPORT_JOB_CAPABILITY => {
                let command: wire::CreatePartyExportJobRequest = decode_request(
                    request,
                    CREATE_PARTY_EXPORT_JOB_REQUEST_SCHEMA,
                )?;
                (
                    export_job_id_from_ref(command.export_job_ref)?,
                    AggregatePresence::MustBeAbsent,
                )
            }
            START_PARTY_EXPORT_EXECUTION_CAPABILITY => {
                let command: wire::StartPartyExportExecutionRequest = decode_request(
                    request,
                    START_PARTY_EXPORT_EXECUTION_REQUEST_SCHEMA,
                )?;
                (
                    export_job_id_from_ref(command.export_job_ref)?,
                    AggregatePresence::MustExist,
                )
            }
            CANCEL_PARTY_EXPORT_JOB_CAPABILITY => {
                let command: wire::CancelPartyExportJobRequest = decode_request(
                    request,
                    CANCEL_PARTY_EXPORT_JOB_REQUEST_SCHEMA,
                )?;
                (
                    export_job_id_from_ref(command.export_job_ref)?,
                    AggregatePresence::MustExist,
                )
            }
            _ => return Err(unsupported_capability()),
        };
        Ok(AggregateTarget {
            reference: export_job_record_ref(&job_id)?,
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
            CREATE_PARTY_EXPORT_JOB_CAPABILITY => plan_create(definition, request, current),
            START_PARTY_EXPORT_EXECUTION_CAPABILITY => plan_start(definition, request, current),
            CANCEL_PARTY_EXPORT_JOB_CAPABILITY => plan_cancel(definition, request, current),
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
    let command: wire::CreatePartyExportJobRequest =
        decode_request(request, CREATE_PARTY_EXPORT_JOB_REQUEST_SCHEMA)?;
    let job = PartyExportJob::create(
        export_job_id_from_ref(command.export_job_ref)?,
        specification_from_wire(command.specification)?,
        request.context.execution.request_started_at_unix_nanos,
    )?;
    let aggregate = export_job_record_ref(job.job_id())?;
    let public_job = export_job_to_wire(&job)?;
    let output = support::protobuf_payload(
        MODULE_ID,
        CREATE_PARTY_EXPORT_JOB_RESPONSE_SCHEMA,
        DataClass::Personal,
        &wire::CreatePartyExportJobResponse {
            export_job: Some(public_job.clone()),
        },
    )?;
    let event = support::event_evidence_with_data_class(
        request,
        aggregate.clone(),
        MODULE_ID,
        EventSpec {
            event_type: PARTY_EXPORT_JOB_CREATED_EVENT_TYPE,
            event_schema_id: PARTY_EXPORT_JOB_CREATED_EVENT_SCHEMA,
            aggregate_version: job.version(),
            previous_version: None,
        },
        DataClass::Personal,
        &wire::PartyExportJobCreatedEvent {
            export_job: Some(public_job),
        },
    )?;
    single_mutation_plan(
        definition,
        request,
        aggregate.clone(),
        RecordMutation::Create {
            reference: aggregate,
            payload: export_job_persisted_payload(&job)?,
        },
        event,
        output,
    )
}

fn plan_start(
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
    current: Option<&RecordSnapshot>,
) -> Result<CapabilityBatchExecutionPlan, SdkError> {
    let current = current.ok_or_else(invalid_plan)?;
    let command: wire::StartPartyExportExecutionRequest =
        decode_request(request, START_PARTY_EXPORT_EXECUTION_REQUEST_SCHEMA)?;
    let requested_job_id = export_job_id_from_ref(command.export_job_ref)?;
    ensure_snapshot_identity(current, &requested_job_id)?;
    let mut job = export_job_from_snapshot(current)?;
    job.start_or_resume(
        command.expected_version,
        request.context.execution.request_started_at_unix_nanos,
    )?;
    let aggregate = export_job_record_ref(job.job_id())?;
    let public_job = export_job_to_wire(&job)?;
    let output = support::protobuf_payload(
        MODULE_ID,
        START_PARTY_EXPORT_EXECUTION_RESPONSE_SCHEMA,
        DataClass::Personal,
        &wire::StartPartyExportExecutionResponse {
            export_job: Some(public_job.clone()),
        },
    )?;
    let event = support::event_evidence_with_data_class(
        request,
        aggregate.clone(),
        MODULE_ID,
        EventSpec {
            event_type: PARTY_EXPORT_EXECUTION_STARTED_EVENT_TYPE,
            event_schema_id: PARTY_EXPORT_EXECUTION_STARTED_EVENT_SCHEMA,
            aggregate_version: job.version(),
            previous_version: Some(current.version),
        },
        DataClass::Personal,
        &wire::PartyExportExecutionStartedEvent {
            export_job: Some(public_job),
        },
    )?;
    single_mutation_plan(
        definition,
        request,
        aggregate.clone(),
        RecordMutation::Update {
            reference: aggregate,
            expected_version: current.version,
            payload: export_job_persisted_payload(&job)?,
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
    let command: wire::CancelPartyExportJobRequest =
        decode_request(request, CANCEL_PARTY_EXPORT_JOB_REQUEST_SCHEMA)?;
    let requested_job_id = export_job_id_from_ref(command.export_job_ref)?;
    ensure_snapshot_identity(current, &requested_job_id)?;
    let mut job = export_job_from_snapshot(current)?;
    job.cancel(
        command.expected_version,
        request.context.execution.request_started_at_unix_nanos,
    )?;
    let aggregate = export_job_record_ref(job.job_id())?;
    let public_job = export_job_to_wire(&job)?;
    let output = support::protobuf_payload(
        MODULE_ID,
        CANCEL_PARTY_EXPORT_JOB_RESPONSE_SCHEMA,
        DataClass::Personal,
        &wire::CancelPartyExportJobResponse {
            export_job: Some(public_job.clone()),
        },
    )?;
    let event = support::event_evidence_with_data_class(
        request,
        aggregate.clone(),
        MODULE_ID,
        EventSpec {
            event_type: PARTY_EXPORT_CANCELLED_EVENT_TYPE,
            event_schema_id: PARTY_EXPORT_CANCELLED_EVENT_SCHEMA,
            aggregate_version: job.version(),
            previous_version: Some(current.version),
        },
        DataClass::Personal,
        &wire::PartyExportCancelledEvent {
            export_job: Some(public_job),
        },
    )?;
    single_mutation_plan(
        definition,
        request,
        aggregate.clone(),
        RecordMutation::Update {
            reference: aggregate,
            expected_version: current.version,
            payload: export_job_persisted_payload(&job)?,
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

pub fn export_job_persisted_contract() -> PersistedPayloadContract<'static> {
    PersistedPayloadContract {
        owner: MODULE_ID,
        schema_id: EXPORT_JOB_STATE_SCHEMA_ID,
        schema_version: EXPORT_JOB_STATE_SCHEMA_VERSION,
        descriptor_hash: export_job_state_descriptor_hash(),
        maximum_size_bytes: EXPORT_JOB_STATE_MAXIMUM_BYTES,
        retention_policy_id: EXPORT_JOB_STATE_RETENTION_POLICY_ID,
    }
}

pub fn export_job_persisted_payload(
    job: &PartyExportJob,
) -> Result<crm_module_sdk::TypedPayload, SdkError> {
    support::persisted_json_payload_with_data_class(
        export_job_persisted_contract(),
        DataClass::Personal,
        encode_export_job_state(job)?,
    )
}

pub fn export_job_from_snapshot(snapshot: &RecordSnapshot) -> Result<PartyExportJob, SdkError> {
    let job = decode_export_job_state(support::persisted_json_bytes_with_data_class(
        snapshot,
        export_job_persisted_contract(),
        DataClass::Personal,
    )?)?;
    if job.job_id().as_str() != snapshot.reference.record_id.as_str()
        || job.version() != snapshot.version
    {
        return Err(support::stored_data_error(
            "CUSTOMER_DATA_EXPORT_PERSISTED_JOB_IDENTITY_INVALID",
        ));
    }
    Ok(job)
}

pub fn export_job_to_wire(job: &PartyExportJob) -> Result<wire::PartyExportJob, SdkError> {
    Ok(wire::PartyExportJob {
        export_job_ref: Some(wire::ExportJobRef {
            export_job_id: job.job_id().as_str().to_owned(),
        }),
        specification: Some(specification_to_wire(job.specification())),
        export_specification_version_id: job.specification().version_id().as_str().to_owned(),
        status: status_to_wire(job.status()) as i32,
        selection: job.selection().map(|selection| wire::PartyExportSelectionSummary {
            manifest_sha256: sha256_hex_to_bytes(selection.manifest_sha256())
                .expect("validated selection SHA-256 must decode"),
            selected_resources: selection.selected_resources(),
        }),
        checkpoint_manifest_position: job.checkpoint_manifest_position(),
        execution_attempts: job.execution_attempts(),
        last_execution_error_code: job.last_execution_error_code().unwrap_or_default().to_owned(),
        artifact: job.artifact().map(|artifact| wire::PartyExportArtifact {
            file_id: artifact.file_id().as_str().to_owned(),
            media_type: PARTY_EXPORT_MEDIA_TYPE.to_owned(),
            content_sha256: sha256_hex_to_bytes(artifact.content_sha256())
                .expect("validated artifact SHA-256 must decode"),
            size_bytes: artifact.size_bytes(),
            retention_policy_id: artifact.retention_policy_id().to_owned(),
        }),
        reconciliation: job.reconciliation().map(|reconciliation| wire::PartyExportReconciliation {
            selected_resources: reconciliation.selected_resources(),
            emitted_rows: reconciliation.emitted_rows(),
            excluded_not_visible: reconciliation.excluded_not_visible(),
            excluded_version_changed: reconciliation.excluded_version_changed(),
            excluded_unavailable: reconciliation.excluded_unavailable(),
            redacted_fields: reconciliation.redacted_fields(),
        }),
        created_at: Some(core::UnixTime {
            unix_nanos: job.created_at_unix_nanos(),
        }),
        updated_at: Some(core::UnixTime {
            unix_nanos: job.updated_at_unix_nanos(),
        }),
        resource_version: Some(customer::CustomerResourceVersion {
            version: job.version(),
            created_at: Some(core::UnixTime {
                unix_nanos: job.created_at_unix_nanos(),
            }),
            updated_at: Some(core::UnixTime {
                unix_nanos: job.updated_at_unix_nanos(),
            }),
        }),
    })
}

fn specification_from_wire(
    value: Option<wire::PartyExportSpecification>,
) -> Result<PartyExportSpecification, SdkError> {
    let value = value.ok_or_else(|| {
        SdkError::invalid_argument(
            "customer_data.export.specification",
            "Party export specification is required",
        )
    })?;
    let scope = value.scope.ok_or_else(|| {
        SdkError::invalid_argument(
            "customer_data.export.specification.scope",
            "Party export scope is required",
        )
    })?;
    let kind_filter = scope
        .kind
        .map(|value| match parties::PartyKind::try_from(value) {
            Ok(parties::PartyKind::Person) => Ok(PartyExportKindFilter::Person),
            Ok(parties::PartyKind::Organization) => Ok(PartyExportKindFilter::Organization),
            Ok(parties::PartyKind::Unspecified) | Err(_) => Err(SdkError::invalid_argument(
                "customer_data.export.specification.scope.kind",
                "Party export kind filter is invalid",
            )),
        })
        .transpose()?;
    let scope = PartyExportScope::try_new(kind_filter, scope.maximum_resources)?;

    let profile = value.profile.ok_or_else(|| {
        SdkError::invalid_argument(
            "customer_data.export.specification.profile",
            "Party export profile is required",
        )
    })?;
    match wire::PartyExportProfileVersion::try_from(profile.profile_version) {
        Ok(wire::PartyExportProfileVersion::V1) => {}
        Ok(wire::PartyExportProfileVersion::Unspecified) | Err(_) => {
            return Err(SdkError::invalid_argument(
                "customer_data.export.specification.profile.profile_version",
                "Party export profile version must be V1",
            ));
        }
    }
    match wire::PartyExportFormat::try_from(profile.format) {
        Ok(wire::PartyExportFormat::CsvUtf8) => {}
        Ok(wire::PartyExportFormat::Unspecified) | Err(_) => {
            return Err(SdkError::invalid_argument(
                "customer_data.export.specification.profile.format",
                "Party export format must be CSV_UTF8",
            ));
        }
    }
    match wire::PartyExportCanonicalizationVersion::try_from(profile.canonicalization_version) {
        Ok(wire::PartyExportCanonicalizationVersion::V1) => {}
        Ok(wire::PartyExportCanonicalizationVersion::Unspecified) | Err(_) => {
            return Err(SdkError::invalid_argument(
                "customer_data.export.specification.profile.canonicalization_version",
                "Party export canonicalization version must be V1",
            ));
        }
    }
    let fields = profile
        .fields
        .into_iter()
        .map(|value| match wire::PartyExportField::try_from(value) {
            Ok(wire::PartyExportField::PartyId) => Ok(PartyExportField::PartyId),
            Ok(wire::PartyExportField::Kind) => Ok(PartyExportField::Kind),
            Ok(wire::PartyExportField::DisplayName) => Ok(PartyExportField::DisplayName),
            Ok(wire::PartyExportField::ResourceVersion) => Ok(PartyExportField::ResourceVersion),
            Ok(wire::PartyExportField::Unspecified) | Err(_) => Err(SdkError::invalid_argument(
                "customer_data.export.specification.profile.fields",
                "Party export fields contain an unsupported value",
            )),
        })
        .collect::<Result<Vec<_>, _>>()?;
    let profile = PartyExportProfile::v1(fields, profile.retention_policy_id)?;
    PartyExportSpecification::try_new(scope, profile)
}

fn specification_to_wire(specification: &PartyExportSpecification) -> wire::PartyExportSpecification {
    wire::PartyExportSpecification {
        scope: Some(wire::PartyExportScope {
            kind: specification.scope().kind_filter().map(|kind| match kind {
                PartyExportKindFilter::Person => parties::PartyKind::Person as i32,
                PartyExportKindFilter::Organization => parties::PartyKind::Organization as i32,
            }),
            maximum_resources: specification.scope().maximum_resources(),
        }),
        profile: Some(wire::PartyExportProfile {
            profile_version: wire::PartyExportProfileVersion::V1 as i32,
            format: wire::PartyExportFormat::CsvUtf8 as i32,
            canonicalization_version: wire::PartyExportCanonicalizationVersion::V1 as i32,
            fields: specification
                .profile()
                .fields()
                .iter()
                .map(|field| match field {
                    PartyExportField::PartyId => wire::PartyExportField::PartyId as i32,
                    PartyExportField::Kind => wire::PartyExportField::Kind as i32,
                    PartyExportField::DisplayName => wire::PartyExportField::DisplayName as i32,
                    PartyExportField::ResourceVersion => wire::PartyExportField::ResourceVersion as i32,
                })
                .collect(),
            retention_policy_id: specification.profile().retention_policy_id().to_owned(),
        }),
    }
}

fn status_to_wire(status: PartyExportJobStatus) -> wire::PartyExportJobStatus {
    match status {
        PartyExportJobStatus::Created => wire::PartyExportJobStatus::Created,
        PartyExportJobStatus::Selecting => wire::PartyExportJobStatus::Selecting,
        PartyExportJobStatus::Ready => wire::PartyExportJobStatus::Ready,
        PartyExportJobStatus::Executing => wire::PartyExportJobStatus::Executing,
        PartyExportJobStatus::Completed => wire::PartyExportJobStatus::Completed,
        PartyExportJobStatus::FailedRetryable => wire::PartyExportJobStatus::FailedRetryable,
        PartyExportJobStatus::Cancelled => wire::PartyExportJobStatus::Cancelled,
    }
}

fn export_job_id_from_ref(value: Option<wire::ExportJobRef>) -> Result<ExportJobId, SdkError> {
    let value = value.ok_or_else(|| {
        SdkError::invalid_argument(
            "customer_data.export_job_ref",
            "Export job reference is required",
        )
    })?;
    ExportJobId::try_new(value.export_job_id)
}

fn export_job_record_ref(
    job_id: &ExportJobId,
) -> Result<crm_module_sdk::RecordRef, SdkError> {
    support::record_ref(
        EXPORT_JOB_RECORD_TYPE,
        job_id.as_str(),
        "customer_data.export_job_ref.export_job_id",
    )
}

fn ensure_snapshot_identity(
    current: &RecordSnapshot,
    requested_job_id: &ExportJobId,
) -> Result<(), SdkError> {
    if current.reference.record_type.as_str() != EXPORT_JOB_RECORD_TYPE
        || current.reference.record_id.as_str() != requested_job_id.as_str()
    {
        return Err(invalid_plan());
    }
    Ok(())
}

fn decode_request<T: prost::Message + Default>(
    request: &CapabilityRequest,
    schema: &'static str,
) -> Result<T, SdkError> {
    support::decode_request_with_data_class(request, MODULE_ID, schema, DataClass::Personal)
}

fn ensure_definition(
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
) -> Result<(), SdkError> {
    if !EXPORT_MUTATION_CAPABILITY_IDS.contains(&definition.capability_id.as_str())
        || definition.owner_module_id.as_str() != MODULE_ID
        || definition.capability_id.as_str() != request.context.execution.capability_id.as_str()
    {
        return Err(invalid_plan());
    }
    Ok(())
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

fn configured<T>(value: Result<T, crm_module_sdk::IdentifierError>) -> Result<T, SdkError> {
    value.map_err(|error| {
        configuration_error(
            "CUSTOMER_DATA_EXPORT_CONFIGURATION_INVALID",
            "The customer-data export capability configuration is invalid.",
        )
        .with_internal_reference(error.to_string())
    })
}

fn invalid_plan() -> SdkError {
    SdkError::new(
        "CUSTOMER_DATA_EXPORT_CAPABILITY_PLAN_INVALID",
        ErrorCategory::Internal,
        false,
        "The customer-data export capability could not be planned safely.",
    )
}

fn unsupported_capability() -> SdkError {
    SdkError::new(
        "CUSTOMER_DATA_EXPORT_CAPABILITY_UNSUPPORTED",
        ErrorCategory::Internal,
        false,
        "The customer-data export capability is not configured.",
    )
}

fn configuration_error(code: &'static str, safe_message: &'static str) -> SdkError {
    SdkError::new(code, ErrorCategory::Internal, false, safe_message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn publishes_exact_export_mutation_coordinates() {
        let definitions = export_capability_definitions().unwrap();
        assert_eq!(definitions.len(), 3);
        for (definition, capability_id) in definitions.iter().zip(EXPORT_MUTATION_CAPABILITY_IDS) {
            assert_eq!(definition.capability_id.as_str(), capability_id);
            assert_eq!(definition.owner_module_id.as_str(), MODULE_ID);
            assert_eq!(definition.capability_version.as_str(), support::CONTRACT_VERSION);
            assert_eq!(
                definition.input_contract.allowed_data_classes,
                vec![DataClass::Personal]
            );
            assert!(definition.mutation);
            assert!(definition.requires_idempotency);
            assert!(!definition.requires_approval);
        }
        assert_eq!(definitions[1].risk, CapabilityRisk::High);
    }

    #[test]
    fn rejects_unknown_export_mutation_coordinate() {
        let error = export_capability_definition("customer_data.export.party.destroy").unwrap_err();
        assert_eq!(error.code, "CUSTOMER_DATA_EXPORT_CAPABILITY_UNSUPPORTED");
    }
}
