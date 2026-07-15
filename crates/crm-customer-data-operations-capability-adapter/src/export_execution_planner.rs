use crm_capability_plan_support::{self as support, EventSpec};
use crm_capability_runtime::{CapabilityDefinition, CapabilityRequest, CapabilityRisk};
use crm_core_data::{
    AggregatePresence, AggregateTarget, BatchMutationPlan, CapabilityBatchExecutionPlan,
    RecordMutation, TransactionalAggregatePlanner,
};
use crm_customer_data_operations::{
    PartyExportArtifactEvidence, PartyExportExclusionReason, PartyExportExecutionOutcome,
    PartyExportExecutionStage, PartyExportJobStatus, PartyExportReconciliation,
};
use crm_module_sdk::{
    CapabilityId, CapabilityVersion, DataClass, ErrorCategory, FileId, ModuleId, RecordSnapshot,
    SdkError,
};
use crm_proto_contracts::crm::customer_data_operations::v1 as wire;
use prost::Message;

use crate::{
    EXPORT_JOB_RECORD_TYPE, MODULE_ID, PARTY_EXPORT_COMPLETED_EVENT_SCHEMA,
    PARTY_EXPORT_COMPLETED_EVENT_TYPE, export_execution_outcome_persisted_payload,
    export_execution_outcome_record_ref, export_execution_stage_persisted_payload,
    export_execution_stage_record_ref, export_job_from_snapshot, export_job_id_from_ref,
    export_job_persisted_payload, export_job_record_ref, export_job_to_wire,
};

pub const INTERNAL_STAGE_PARTY_EXPORT_EXECUTION_CAPABILITY: &str =
    "customer_data.export.party.execution.stage";
pub const INTERNAL_COMMIT_PARTY_EXPORT_EXECUTION_OUTCOME_CAPABILITY: &str =
    "customer_data.export.party.execution.outcome.commit";
pub const INTERNAL_COMPLETE_PARTY_EXPORT_EXECUTION_CAPABILITY: &str =
    "customer_data.export.party.execution.complete";

pub const INTERNAL_STAGE_PARTY_EXPORT_EXECUTION_REQUEST_SCHEMA: &str =
    "crm.customer_data_operations.v1.InternalStagePartyExportExecutionRequest";
pub const INTERNAL_STAGE_PARTY_EXPORT_EXECUTION_RESPONSE_SCHEMA: &str =
    "crm.customer_data_operations.v1.InternalStagePartyExportExecutionResponse";
pub const INTERNAL_COMMIT_PARTY_EXPORT_EXECUTION_OUTCOME_REQUEST_SCHEMA: &str =
    "crm.customer_data_operations.v1.InternalCommitPartyExportExecutionOutcomeRequest";
pub const INTERNAL_COMMIT_PARTY_EXPORT_EXECUTION_OUTCOME_RESPONSE_SCHEMA: &str =
    "crm.customer_data_operations.v1.InternalCommitPartyExportExecutionOutcomeResponse";
pub const INTERNAL_COMPLETE_PARTY_EXPORT_EXECUTION_REQUEST_SCHEMA: &str =
    "crm.customer_data_operations.v1.InternalCompletePartyExportExecutionRequest";
pub const INTERNAL_COMPLETE_PARTY_EXPORT_EXECUTION_RESPONSE_SCHEMA: &str =
    "crm.customer_data_operations.v1.InternalCompletePartyExportExecutionResponse";

pub const PARTY_EXPORT_EXECUTION_PROGRESSED_EVENT_TYPE: &str =
    "customer_data.export.party.execution_progressed";
pub const PARTY_EXPORT_EXECUTION_PROGRESSED_EVENT_SCHEMA: &str =
    "crm.customer_data_operations.v1.PartyExportExecutionProgressedEvent";

pub const INTERNAL_EXPORT_EXECUTION_CAPABILITY_IDS: [&str; 3] = [
    INTERNAL_STAGE_PARTY_EXPORT_EXECUTION_CAPABILITY,
    INTERNAL_COMMIT_PARTY_EXPORT_EXECUTION_OUTCOME_CAPABILITY,
    INTERNAL_COMPLETE_PARTY_EXPORT_EXECUTION_CAPABILITY,
];

#[derive(Debug, Default, Clone, Copy)]
pub struct PartyExportExecutionOutcomePlanner;

pub fn internal_export_execution_capability_definitions()
-> Result<Vec<CapabilityDefinition>, SdkError> {
    INTERNAL_EXPORT_EXECUTION_CAPABILITY_IDS
        .into_iter()
        .map(internal_export_execution_capability_definition)
        .collect()
}

pub fn internal_export_execution_capability_definition(
    capability_id: &str,
) -> Result<CapabilityDefinition, SdkError> {
    let (input_schema, output_schema, risk) = match capability_id {
        INTERNAL_STAGE_PARTY_EXPORT_EXECUTION_CAPABILITY => (
            INTERNAL_STAGE_PARTY_EXPORT_EXECUTION_REQUEST_SCHEMA,
            INTERNAL_STAGE_PARTY_EXPORT_EXECUTION_RESPONSE_SCHEMA,
            CapabilityRisk::Medium,
        ),
        INTERNAL_COMMIT_PARTY_EXPORT_EXECUTION_OUTCOME_CAPABILITY => (
            INTERNAL_COMMIT_PARTY_EXPORT_EXECUTION_OUTCOME_REQUEST_SCHEMA,
            INTERNAL_COMMIT_PARTY_EXPORT_EXECUTION_OUTCOME_RESPONSE_SCHEMA,
            CapabilityRisk::Medium,
        ),
        INTERNAL_COMPLETE_PARTY_EXPORT_EXECUTION_CAPABILITY => (
            INTERNAL_COMPLETE_PARTY_EXPORT_EXECUTION_REQUEST_SCHEMA,
            INTERNAL_COMPLETE_PARTY_EXPORT_EXECUTION_RESPONSE_SCHEMA,
            CapabilityRisk::High,
        ),
        _ => return Err(unsupported_internal_capability()),
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

impl TransactionalAggregatePlanner for PartyExportExecutionOutcomePlanner {
    fn target(
        &self,
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
    ) -> Result<AggregateTarget, SdkError> {
        ensure_definition(definition, request)?;
        let job_id = match definition.capability_id.as_str() {
            INTERNAL_STAGE_PARTY_EXPORT_EXECUTION_CAPABILITY => {
                let command: wire::InternalStagePartyExportExecutionRequest = decode_request(
                    request,
                    INTERNAL_STAGE_PARTY_EXPORT_EXECUTION_REQUEST_SCHEMA,
                )?;
                export_job_id_from_ref(command.export_job_ref)?
            }
            INTERNAL_COMMIT_PARTY_EXPORT_EXECUTION_OUTCOME_CAPABILITY => {
                let command: wire::InternalCommitPartyExportExecutionOutcomeRequest =
                    decode_request(
                        request,
                        INTERNAL_COMMIT_PARTY_EXPORT_EXECUTION_OUTCOME_REQUEST_SCHEMA,
                    )?;
                export_job_id_from_ref(command.export_job_ref)?
            }
            INTERNAL_COMPLETE_PARTY_EXPORT_EXECUTION_CAPABILITY => {
                let command: wire::InternalCompletePartyExportExecutionRequest = decode_request(
                    request,
                    INTERNAL_COMPLETE_PARTY_EXPORT_EXECUTION_REQUEST_SCHEMA,
                )?;
                export_job_id_from_ref(command.export_job_ref)?
            }
            _ => return Err(unsupported_internal_capability()),
        };
        Ok(AggregateTarget {
            reference: export_job_record_ref(&job_id)?,
            presence: AggregatePresence::MustExist,
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
            INTERNAL_STAGE_PARTY_EXPORT_EXECUTION_CAPABILITY => {
                plan_stage_execution(definition, request, current)
            }
            INTERNAL_COMMIT_PARTY_EXPORT_EXECUTION_OUTCOME_CAPABILITY => {
                plan_commit_outcome(definition, request, current)
            }
            INTERNAL_COMPLETE_PARTY_EXPORT_EXECUTION_CAPABILITY => {
                plan_complete_execution(definition, request, current)
            }
            _ => Err(unsupported_internal_capability()),
        }
    }
}

fn plan_stage_execution(
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
    current: Option<&RecordSnapshot>,
) -> Result<CapabilityBatchExecutionPlan, SdkError> {
    let current = current.ok_or_else(invalid_plan)?;
    let command: wire::InternalStagePartyExportExecutionRequest = decode_request(
        request,
        INTERNAL_STAGE_PARTY_EXPORT_EXECUTION_REQUEST_SCHEMA,
    )?;
    let requested_job_id = export_job_id_from_ref(command.export_job_ref)?;
    let job = export_job_from_snapshot(current)?;
    ensure_job_identity(&job, current, &requested_job_id)?;
    if job.status() != PartyExportJobStatus::Executing {
        return Err(execution_state_invalid());
    }
    let selected = job
        .selection()
        .ok_or_else(execution_state_invalid)?
        .selected_resources();
    if command.manifest_position == 0 || command.manifest_position > selected {
        return Err(SdkError::invalid_argument(
            "customer_data.export.execution.manifest_position",
            "execution stage position is outside the finalized selection",
        ));
    }
    let occurred_at = request.context.execution.request_started_at_unix_nanos;
    let stage = match command.result.ok_or_else(execution_result_required)? {
        wire::internal_stage_party_export_execution_request::Result::Emitted(emitted) => {
            PartyExportExecutionStage::emitted(
                requested_job_id,
                command.manifest_position,
                String::from_utf8(emitted.row_utf8).map_err(|_| invalid_utf8_row())?,
                emitted.redacted_fields,
                occurred_at,
            )?
        }
        wire::internal_stage_party_export_execution_request::Result::ExclusionReason(reason) => {
            PartyExportExecutionStage::excluded(
                requested_job_id,
                command.manifest_position,
                exclusion_reason(reason)?,
                occurred_at,
            )?
        }
    };
    let stage_ref = export_execution_stage_record_ref(&stage)?;
    let output = support::protobuf_payload(
        MODULE_ID,
        INTERNAL_STAGE_PARTY_EXPORT_EXECUTION_RESPONSE_SCHEMA,
        DataClass::Personal,
        &wire::InternalStagePartyExportExecutionResponse {
            manifest_position: stage.manifest_position(),
        },
    )?;
    let audit = support::audit_intent(
        request,
        &stage_ref,
        1,
        definition.capability_id.as_str(),
        &output.bytes,
    )?;
    Ok(CapabilityBatchExecutionPlan {
        batch: BatchMutationPlan {
            context: request.context.clone(),
            records: vec![RecordMutation::Create {
                reference: stage_ref,
                payload: export_execution_stage_persisted_payload(&stage)?,
            }],
            relationships: Vec::new(),
            events: Vec::new(),
            idempotency: support::capability_idempotency(definition, request)?,
            audits: vec![audit],
        },
        output: Some(output),
    })
}

fn plan_commit_outcome(
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
    current: Option<&RecordSnapshot>,
) -> Result<CapabilityBatchExecutionPlan, SdkError> {
    let current = current.ok_or_else(invalid_plan)?;
    let command: wire::InternalCommitPartyExportExecutionOutcomeRequest = decode_request(
        request,
        INTERNAL_COMMIT_PARTY_EXPORT_EXECUTION_OUTCOME_REQUEST_SCHEMA,
    )?;
    let requested_job_id = export_job_id_from_ref(command.export_job_ref)?;
    let mut job = export_job_from_snapshot(current)?;
    ensure_job_identity(&job, current, &requested_job_id)?;
    let occurred_at = request.context.execution.request_started_at_unix_nanos;
    let outcome = match command.result.ok_or_else(execution_result_required)? {
        wire::internal_commit_party_export_execution_outcome_request::Result::Emitted(emitted) => {
            PartyExportExecutionOutcome::emitted(
                requested_job_id,
                command.manifest_position,
                emitted.artifact_chunk_index,
                bytes_to_sha256_hex(&emitted.chunk_sha256)?,
                emitted.chunk_size_bytes,
                emitted.redacted_fields,
                occurred_at,
            )?
        }
        wire::internal_commit_party_export_execution_outcome_request::Result::ExclusionReason(
            reason,
        ) => PartyExportExecutionOutcome::excluded(
            requested_job_id,
            command.manifest_position,
            exclusion_reason(reason)?,
            occurred_at,
        )?,
    };
    job.advance_checkpoint(
        command.expected_job_version,
        command.manifest_position,
        occurred_at,
    )?;
    let aggregate = export_job_record_ref(job.job_id())?;
    let public_job = export_job_to_wire(&job)?;
    let output = support::protobuf_payload(
        MODULE_ID,
        INTERNAL_COMMIT_PARTY_EXPORT_EXECUTION_OUTCOME_RESPONSE_SCHEMA,
        DataClass::Personal,
        &wire::InternalCommitPartyExportExecutionOutcomeResponse {
            export_job: Some(public_job.clone()),
        },
    )?;
    let event = support::event_evidence_with_data_class(
        request,
        aggregate.clone(),
        MODULE_ID,
        EventSpec {
            event_type: PARTY_EXPORT_EXECUTION_PROGRESSED_EVENT_TYPE,
            event_schema_id: PARTY_EXPORT_EXECUTION_PROGRESSED_EVENT_SCHEMA,
            aggregate_version: job.version(),
            previous_version: Some(current.version),
        },
        DataClass::Personal,
        &wire::PartyExportExecutionProgressedEvent {
            export_job: Some(public_job),
            manifest_position: command.manifest_position,
        },
    )?;
    let audit = support::audit_intent(
        request,
        &aggregate,
        job.version(),
        definition.capability_id.as_str(),
        &output.bytes,
    )?;
    Ok(CapabilityBatchExecutionPlan {
        batch: BatchMutationPlan {
            context: request.context.clone(),
            records: vec![
                RecordMutation::Create {
                    reference: export_execution_outcome_record_ref(&outcome)?,
                    payload: export_execution_outcome_persisted_payload(&outcome)?,
                },
                RecordMutation::Update {
                    reference: aggregate,
                    expected_version: current.version,
                    payload: export_job_persisted_payload(&job)?,
                },
            ],
            relationships: Vec::new(),
            events: vec![event],
            idempotency: support::capability_idempotency(definition, request)?,
            audits: vec![audit],
        },
        output: Some(output),
    })
}

fn plan_complete_execution(
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
    current: Option<&RecordSnapshot>,
) -> Result<CapabilityBatchExecutionPlan, SdkError> {
    let current = current.ok_or_else(invalid_plan)?;
    let command: wire::InternalCompletePartyExportExecutionRequest = decode_request(
        request,
        INTERNAL_COMPLETE_PARTY_EXPORT_EXECUTION_REQUEST_SCHEMA,
    )?;
    let requested_job_id = export_job_id_from_ref(command.export_job_ref)?;
    let mut job = export_job_from_snapshot(current)?;
    ensure_job_identity(&job, current, &requested_job_id)?;
    let artifact = command.artifact.ok_or_else(completion_evidence_required)?;
    if artifact.media_type != "text/csv; charset=utf-8" {
        return Err(SdkError::invalid_argument(
            "customer_data.export.artifact.media_type",
            "completed Party export artifact media type must be canonical UTF-8 CSV",
        ));
    }
    let artifact = PartyExportArtifactEvidence::try_new(
        FileId::try_new(artifact.file_id).map_err(configured)?,
        bytes_to_sha256_hex(&artifact.content_sha256)?,
        artifact.size_bytes,
        artifact.retention_policy_id,
    )?;
    let reconciliation = command
        .reconciliation
        .ok_or_else(completion_evidence_required)?;
    let reconciliation = PartyExportReconciliation::try_new(
        reconciliation.selected_resources,
        reconciliation.emitted_rows,
        reconciliation.excluded_not_visible,
        reconciliation.excluded_version_changed,
        reconciliation.excluded_unavailable,
        reconciliation.redacted_fields,
    )?;
    job.complete(
        command.expected_job_version,
        artifact,
        reconciliation,
        request.context.execution.request_started_at_unix_nanos,
    )?;
    let aggregate = export_job_record_ref(job.job_id())?;
    let public_job = export_job_to_wire(&job)?;
    let output = support::protobuf_payload(
        MODULE_ID,
        INTERNAL_COMPLETE_PARTY_EXPORT_EXECUTION_RESPONSE_SCHEMA,
        DataClass::Personal,
        &wire::InternalCompletePartyExportExecutionResponse {
            export_job: Some(public_job.clone()),
        },
    )?;
    let event = support::event_evidence_with_data_class(
        request,
        aggregate.clone(),
        MODULE_ID,
        EventSpec {
            event_type: PARTY_EXPORT_COMPLETED_EVENT_TYPE,
            event_schema_id: PARTY_EXPORT_COMPLETED_EVENT_SCHEMA,
            aggregate_version: job.version(),
            previous_version: Some(current.version),
        },
        DataClass::Personal,
        &wire::PartyExportCompletedEvent {
            export_job: Some(public_job),
        },
    )?;
    let audit = support::audit_intent(
        request,
        &aggregate,
        job.version(),
        definition.capability_id.as_str(),
        &output.bytes,
    )?;
    Ok(CapabilityBatchExecutionPlan {
        batch: BatchMutationPlan {
            context: request.context.clone(),
            records: vec![RecordMutation::Update {
                reference: aggregate,
                expected_version: current.version,
                payload: export_job_persisted_payload(&job)?,
            }],
            relationships: Vec::new(),
            events: vec![event],
            idempotency: support::capability_idempotency(definition, request)?,
            audits: vec![audit],
        },
        output: Some(output),
    })
}

fn ensure_job_identity(
    job: &crm_customer_data_operations::PartyExportJob,
    snapshot: &RecordSnapshot,
    requested_job_id: &crm_customer_data_operations::ExportJobId,
) -> Result<(), SdkError> {
    if job.job_id() != requested_job_id
        || job.version() != snapshot.version
        || snapshot.reference.record_type.as_str() != EXPORT_JOB_RECORD_TYPE
    {
        return Err(stored_state_invalid());
    }
    Ok(())
}

fn exclusion_reason(value: i32) -> Result<PartyExportExclusionReason, SdkError> {
    match wire::PartyExportExecutionExclusionReason::try_from(value) {
        Ok(wire::PartyExportExecutionExclusionReason::NotVisible) => {
            Ok(PartyExportExclusionReason::NotVisible)
        }
        Ok(wire::PartyExportExecutionExclusionReason::VersionChanged) => {
            Ok(PartyExportExclusionReason::VersionChanged)
        }
        Ok(wire::PartyExportExecutionExclusionReason::Unavailable) => {
            Ok(PartyExportExclusionReason::Unavailable)
        }
        _ => Err(SdkError::invalid_argument(
            "customer_data.export.execution.exclusion_reason",
            "execution exclusion reason is invalid",
        )),
    }
}

fn bytes_to_sha256_hex(value: &[u8]) -> Result<String, SdkError> {
    if value.len() != 32 {
        return Err(SdkError::invalid_argument(
            "customer_data.export.sha256",
            "SHA-256 evidence must contain exactly 32 bytes",
        ));
    }
    let mut output = String::with_capacity(64);
    for byte in value {
        use std::fmt::Write as _;
        write!(&mut output, "{byte:02x}").expect("writing to String cannot fail");
    }
    Ok(output)
}

fn decode_request<M: Message + Default>(
    request: &CapabilityRequest,
    schema_id: &'static str,
) -> Result<M, SdkError> {
    support::decode_request_with_data_class(request, MODULE_ID, schema_id, DataClass::Personal)
}

fn ensure_definition(
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
) -> Result<(), SdkError> {
    let expected =
        internal_export_execution_capability_definition(definition.capability_id.as_str())?;
    if definition != &expected
        || request.context.module_id != definition.owner_module_id
        || request.context.execution.capability_id != definition.capability_id
        || request.context.execution.capability_version != definition.capability_version
    {
        return Err(unsupported_internal_capability());
    }
    Ok(())
}

fn configured<T>(value: Result<T, crm_module_sdk::IdentifierError>) -> Result<T, SdkError> {
    value.map_err(|error| {
        SdkError::new(
            "CUSTOMER_DATA_EXPORT_EXECUTION_CONFIGURATION_INVALID",
            ErrorCategory::Internal,
            false,
            "The customer export execution capability is not configured safely.",
        )
        .with_internal_reference(error.to_string())
    })
}

fn invalid_plan() -> SdkError {
    SdkError::new(
        "CUSTOMER_DATA_EXPORT_EXECUTION_PLAN_INVALID",
        ErrorCategory::Internal,
        false,
        "The customer export execution mutation could not be planned safely.",
    )
}

fn stored_state_invalid() -> SdkError {
    support::stored_data_error("CUSTOMER_DATA_EXPORT_EXECUTION_STORED_STATE_INVALID")
}

fn execution_state_invalid() -> SdkError {
    SdkError::new(
        "CUSTOMER_DATA_EXPORT_EXECUTION_STATE_INVALID",
        ErrorCategory::Conflict,
        false,
        "The export job is not in the required execution state.",
    )
}

fn execution_result_required() -> SdkError {
    SdkError::invalid_argument(
        "customer_data.export.execution.result",
        "exact emitted or excluded execution result is required",
    )
}

fn invalid_utf8_row() -> SdkError {
    SdkError::invalid_argument(
        "customer_data.export.execution.row_utf8",
        "staged Party export row must be valid UTF-8",
    )
}

fn completion_evidence_required() -> SdkError {
    SdkError::invalid_argument(
        "customer_data.export.execution.completion",
        "artifact and reconciliation evidence are required for completion",
    )
}

fn unsupported_internal_capability() -> SdkError {
    SdkError::new(
        "CUSTOMER_DATA_EXPORT_EXECUTION_INTERNAL_CAPABILITY_UNSUPPORTED",
        ErrorCategory::Internal,
        false,
        "The internal customer export execution capability is not configured.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MUTATION_CAPABILITY_IDS;

    #[test]
    fn private_execution_capabilities_remain_outside_public_mutation_catalog() {
        for capability_id in INTERNAL_EXPORT_EXECUTION_CAPABILITY_IDS {
            assert!(!MUTATION_CAPABILITY_IDS.contains(&capability_id));
        }
    }
}
