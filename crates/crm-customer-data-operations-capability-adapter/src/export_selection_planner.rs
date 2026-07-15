use crm_capability_plan_support::{self as support, EventSpec, PersistedPayloadContract};
use crm_capability_runtime::{CapabilityDefinition, CapabilityRequest, CapabilityRisk};
use crm_core_data::{
    AggregatePresence, AggregateTarget, BatchMutationPlan, CapabilityBatchExecutionPlan,
    RecordMutation, TransactionalAggregatePlanner,
};
use crm_customer_data_operations::{
    EXPORT_SELECTION_ITEM_STATE_MAXIMUM_BYTES, EXPORT_SELECTION_ITEM_STATE_RETENTION_POLICY_ID,
    EXPORT_SELECTION_ITEM_STATE_SCHEMA_ID, EXPORT_SELECTION_ITEM_STATE_SCHEMA_VERSION,
    PartyExportSelectionItem, PartyExportSelectionSummary, PartyExportSourceContinuation,
    SelectedPartyId, decode_export_selection_progress_state, encode_export_selection_item_state,
    export_selection_item_state_descriptor_hash,
};
use crm_module_sdk::{
    CapabilityId, CapabilityVersion, DataClass, ErrorCategory, ModuleId, PayloadEncoding, RecordId,
    RecordSnapshot, SdkError,
};
use crm_proto_contracts::crm::customer_data_operations::v1 as wire;
use prost::Message;

use crate::{
    MODULE_ID, export_job_from_snapshot, export_job_id_from_ref, export_job_persisted_payload,
    export_job_record_ref, export_job_to_wire, export_selection_progress_persisted_contract,
    export_selection_progress_persisted_payload, export_selection_progress_record_ref,
};

pub const INTERNAL_COMMIT_PARTY_EXPORT_SELECTION_PAGE_CAPABILITY: &str =
    "customer_data.export.party.selection.page.commit";
pub const INTERNAL_FINALIZE_PARTY_EXPORT_SELECTION_CAPABILITY: &str =
    "customer_data.export.party.selection.finalize";

pub const INTERNAL_COMMIT_PARTY_EXPORT_SELECTION_PAGE_REQUEST_SCHEMA: &str =
    "crm.customer_data_operations.v1.InternalCommitPartyExportSelectionPageRequest";
pub const INTERNAL_COMMIT_PARTY_EXPORT_SELECTION_PAGE_RESPONSE_SCHEMA: &str =
    "crm.customer_data_operations.v1.InternalCommitPartyExportSelectionPageResponse";
pub const INTERNAL_FINALIZE_PARTY_EXPORT_SELECTION_REQUEST_SCHEMA: &str =
    "crm.customer_data_operations.v1.InternalFinalizePartyExportSelectionRequest";
pub const INTERNAL_FINALIZE_PARTY_EXPORT_SELECTION_RESPONSE_SCHEMA: &str =
    "crm.customer_data_operations.v1.InternalFinalizePartyExportSelectionResponse";

pub const PARTY_EXPORT_SELECTION_PROGRESSED_EVENT_TYPE: &str =
    "customer_data.export.party.selection_progressed";
pub const PARTY_EXPORT_SELECTION_PROGRESSED_EVENT_SCHEMA: &str =
    "crm.customer_data_operations.v1.PartyExportSelectionProgressedEvent";

pub const INTERNAL_EXPORT_SELECTION_CAPABILITY_IDS: [&str; 2] = [
    INTERNAL_COMMIT_PARTY_EXPORT_SELECTION_PAGE_CAPABILITY,
    INTERNAL_FINALIZE_PARTY_EXPORT_SELECTION_CAPABILITY,
];

#[derive(Debug, Default, Clone, Copy)]
pub struct PartyExportSelectionOutcomePlanner;

pub fn internal_export_selection_capability_definitions()
-> Result<Vec<CapabilityDefinition>, SdkError> {
    INTERNAL_EXPORT_SELECTION_CAPABILITY_IDS
        .into_iter()
        .map(internal_export_selection_capability_definition)
        .collect()
}

pub fn internal_export_selection_capability_definition(
    capability_id: &str,
) -> Result<CapabilityDefinition, SdkError> {
    let (input_schema, output_schema) = match capability_id {
        INTERNAL_COMMIT_PARTY_EXPORT_SELECTION_PAGE_CAPABILITY => (
            INTERNAL_COMMIT_PARTY_EXPORT_SELECTION_PAGE_REQUEST_SCHEMA,
            INTERNAL_COMMIT_PARTY_EXPORT_SELECTION_PAGE_RESPONSE_SCHEMA,
        ),
        INTERNAL_FINALIZE_PARTY_EXPORT_SELECTION_CAPABILITY => (
            INTERNAL_FINALIZE_PARTY_EXPORT_SELECTION_REQUEST_SCHEMA,
            INTERNAL_FINALIZE_PARTY_EXPORT_SELECTION_RESPONSE_SCHEMA,
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
        risk: CapabilityRisk::Medium,
        mutation: true,
        requires_idempotency: true,
        requires_approval: false,
        authorization_policy_id: capability_id.to_owned(),
        rate_limit_policy_id: None,
    })
}

impl TransactionalAggregatePlanner for PartyExportSelectionOutcomePlanner {
    fn target(
        &self,
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
    ) -> Result<AggregateTarget, SdkError> {
        ensure_definition(definition, request)?;
        match definition.capability_id.as_str() {
            INTERNAL_COMMIT_PARTY_EXPORT_SELECTION_PAGE_CAPABILITY => {
                let command: wire::InternalCommitPartyExportSelectionPageRequest = decode_request(
                    request,
                    INTERNAL_COMMIT_PARTY_EXPORT_SELECTION_PAGE_REQUEST_SCHEMA,
                )?;
                let job_id = export_job_id_from_ref(command.export_job_ref)?;
                let progress = crm_customer_data_operations::PartyExportSelectionProgress::create(
                    job_id,
                    1,
                    request.context.execution.request_started_at_unix_nanos,
                )?;
                Ok(AggregateTarget {
                    reference: export_selection_progress_record_ref(&progress)?,
                    presence: AggregatePresence::MustExist,
                })
            }
            INTERNAL_FINALIZE_PARTY_EXPORT_SELECTION_CAPABILITY => {
                let command: wire::InternalFinalizePartyExportSelectionRequest = decode_request(
                    request,
                    INTERNAL_FINALIZE_PARTY_EXPORT_SELECTION_REQUEST_SCHEMA,
                )?;
                let job_id = export_job_id_from_ref(command.export_job_ref)?;
                Ok(AggregateTarget {
                    reference: export_job_record_ref(&job_id)?,
                    presence: AggregatePresence::MustExist,
                })
            }
            _ => Err(unsupported_internal_capability()),
        }
    }

    fn plan(
        &self,
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
        current: Option<&RecordSnapshot>,
    ) -> Result<CapabilityBatchExecutionPlan, SdkError> {
        ensure_definition(definition, request)?;
        match definition.capability_id.as_str() {
            INTERNAL_COMMIT_PARTY_EXPORT_SELECTION_PAGE_CAPABILITY => {
                plan_commit_selection_page(definition, request, current)
            }
            INTERNAL_FINALIZE_PARTY_EXPORT_SELECTION_CAPABILITY => {
                plan_finalize_selection(definition, request, current)
            }
            _ => Err(unsupported_internal_capability()),
        }
    }
}

fn plan_commit_selection_page(
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
    current: Option<&RecordSnapshot>,
) -> Result<CapabilityBatchExecutionPlan, SdkError> {
    let current = current.ok_or_else(invalid_plan)?;
    let command: wire::InternalCommitPartyExportSelectionPageRequest = decode_request(
        request,
        INTERNAL_COMMIT_PARTY_EXPORT_SELECTION_PAGE_REQUEST_SCHEMA,
    )?;
    let job_id = export_job_id_from_ref(command.export_job_ref)?;
    let mut progress = export_selection_progress_from_snapshot(current)?;
    if progress.job_id() != &job_id || progress.version() != current.version {
        return Err(stored_state_invalid());
    }

    let continuation =
        source_continuation_from_wire(command.source_after, command.source_exhausted)?;
    let occurred_at = request.context.execution.request_started_at_unix_nanos;
    let first_position = progress.next_manifest_position();
    let committed_items = u32::try_from(command.candidates.len()).map_err(|_| invalid_plan())?;
    let mut records = Vec::with_capacity(command.candidates.len() + 1);

    for (index, candidate) in command.candidates.into_iter().enumerate() {
        let offset = u32::try_from(index).map_err(|_| invalid_plan())?;
        let position = first_position
            .checked_add(offset)
            .ok_or_else(invalid_plan)?;
        let party_ref = candidate.party_ref.ok_or_else(|| {
            SdkError::invalid_argument(
                "customer_data.export.selection.candidate.party_ref",
                "Party selection candidate reference is required",
            )
        })?;
        let resource_version = candidate.resource_version.ok_or_else(|| {
            SdkError::invalid_argument(
                "customer_data.export.selection.candidate.resource_version",
                "Party selection candidate resource version is required",
            )
        })?;
        let item = PartyExportSelectionItem::create(
            job_id.clone(),
            position,
            SelectedPartyId::try_new(party_ref.party_id)?,
            resource_version.version,
            occurred_at,
        )?;
        records.push(RecordMutation::Create {
            reference: export_selection_item_record_ref(&item)?,
            payload: export_selection_item_persisted_payload(&item)?,
        });
    }

    progress.advance(
        command.expected_progress_version,
        committed_items,
        continuation,
        occurred_at,
    )?;
    let progress_ref = export_selection_progress_record_ref(&progress)?;
    records.push(RecordMutation::Update {
        reference: progress_ref.clone(),
        expected_version: current.version,
        payload: export_selection_progress_persisted_payload(&progress)?,
    });

    let output = support::protobuf_payload(
        MODULE_ID,
        INTERNAL_COMMIT_PARTY_EXPORT_SELECTION_PAGE_RESPONSE_SCHEMA,
        DataClass::Personal,
        &wire::InternalCommitPartyExportSelectionPageResponse {
            committed_items,
            next_manifest_position: progress.next_manifest_position(),
            progress_version: progress.version(),
            source_exhausted: progress.source_exhausted(),
        },
    )?;
    let event = support::event_evidence_with_data_class(
        request,
        progress_ref.clone(),
        MODULE_ID,
        EventSpec {
            event_type: PARTY_EXPORT_SELECTION_PROGRESSED_EVENT_TYPE,
            event_schema_id: PARTY_EXPORT_SELECTION_PROGRESSED_EVENT_SCHEMA,
            aggregate_version: progress.version(),
            previous_version: Some(current.version),
        },
        DataClass::Personal,
        &wire::PartyExportSelectionProgressedEvent {
            export_job_ref: Some(wire::ExportJobRef {
                export_job_id: job_id.as_str().to_owned(),
            }),
            committed_items,
            next_manifest_position: progress.next_manifest_position(),
            progress_version: progress.version(),
            source_exhausted: progress.source_exhausted(),
        },
    )?;
    let audit = support::audit_intent(
        request,
        &progress_ref,
        progress.version(),
        definition.capability_id.as_str(),
        &output.bytes,
    )?;

    Ok(CapabilityBatchExecutionPlan {
        batch: BatchMutationPlan {
            context: request.context.clone(),
            records,
            relationships: Vec::new(),
            events: vec![event],
            idempotency: support::capability_idempotency(definition, request)?,
            audits: vec![audit],
        },
        output: Some(output),
    })
}

fn plan_finalize_selection(
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
    current: Option<&RecordSnapshot>,
) -> Result<CapabilityBatchExecutionPlan, SdkError> {
    let current = current.ok_or_else(invalid_plan)?;
    let command: wire::InternalFinalizePartyExportSelectionRequest = decode_request(
        request,
        INTERNAL_FINALIZE_PARTY_EXPORT_SELECTION_REQUEST_SCHEMA,
    )?;
    if command.expected_progress_version <= 0 {
        return Err(SdkError::invalid_argument(
            "customer_data.export.selection.expected_progress_version",
            "expected selection progress version must be positive",
        ));
    }
    let requested_job_id = export_job_id_from_ref(command.export_job_ref)?;
    let mut job = export_job_from_snapshot(current)?;
    if job.job_id() != &requested_job_id || job.version() != current.version {
        return Err(stored_state_invalid());
    }
    let manifest_sha256 = bytes_to_sha256_hex(&command.manifest_sha256)?;
    let selection = PartyExportSelectionSummary::try_new(
        manifest_sha256,
        command.selected_resources,
        job.specification().scope().maximum_resources(),
    )?;
    job.complete_selection(
        command.expected_job_version,
        selection,
        request.context.execution.request_started_at_unix_nanos,
    )?;

    let aggregate = export_job_record_ref(job.job_id())?;
    let public_job = export_job_to_wire(&job)?;
    let output = support::protobuf_payload(
        MODULE_ID,
        INTERNAL_FINALIZE_PARTY_EXPORT_SELECTION_RESPONSE_SCHEMA,
        DataClass::Personal,
        &wire::InternalFinalizePartyExportSelectionResponse {
            export_job: Some(public_job.clone()),
        },
    )?;
    let event = support::event_evidence_with_data_class(
        request,
        aggregate.clone(),
        MODULE_ID,
        EventSpec {
            event_type: crate::PARTY_EXPORT_SELECTION_COMPLETED_EVENT_TYPE,
            event_schema_id: crate::PARTY_EXPORT_SELECTION_COMPLETED_EVENT_SCHEMA,
            aggregate_version: job.version(),
            previous_version: Some(current.version),
        },
        DataClass::Personal,
        &wire::PartyExportSelectionCompletedEvent {
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

pub fn export_selection_item_persisted_contract() -> PersistedPayloadContract<'static> {
    PersistedPayloadContract {
        owner: MODULE_ID,
        schema_id: EXPORT_SELECTION_ITEM_STATE_SCHEMA_ID,
        schema_version: EXPORT_SELECTION_ITEM_STATE_SCHEMA_VERSION,
        descriptor_hash: export_selection_item_state_descriptor_hash(),
        maximum_size_bytes: EXPORT_SELECTION_ITEM_STATE_MAXIMUM_BYTES,
        retention_policy_id: EXPORT_SELECTION_ITEM_STATE_RETENTION_POLICY_ID,
    }
}

pub fn export_selection_item_persisted_payload(
    item: &PartyExportSelectionItem,
) -> Result<crm_module_sdk::TypedPayload, SdkError> {
    support::persisted_json_payload_with_data_class(
        export_selection_item_persisted_contract(),
        DataClass::Personal,
        encode_export_selection_item_state(item)?,
    )
}

pub fn export_selection_item_record_ref(
    item: &PartyExportSelectionItem,
) -> Result<crm_module_sdk::RecordRef, SdkError> {
    support::record_ref(
        "customer_data.export_selection_item",
        item.item_id().as_str(),
        "customer_data.export.selection_item_id",
    )
}

fn export_selection_progress_from_snapshot(
    snapshot: &RecordSnapshot,
) -> Result<crm_customer_data_operations::PartyExportSelectionProgress, SdkError> {
    let contract = export_selection_progress_persisted_contract();
    let payload = &snapshot.payload;
    if payload.owner.as_str() != contract.owner
        || payload.schema_id.as_str() != contract.schema_id
        || payload.schema_version.as_str() != contract.schema_version
        || payload.descriptor_hash != contract.descriptor_hash
        || payload.data_class != DataClass::Personal
        || payload.encoding != PayloadEncoding::Json
        || payload.maximum_size_bytes != contract.maximum_size_bytes
        || payload.retention_policy_id.as_str() != contract.retention_policy_id
        || payload.validate().is_err()
    {
        return Err(stored_state_invalid());
    }
    decode_export_selection_progress_state(&payload.bytes).map_err(|_| stored_state_invalid())
}

fn source_continuation_from_wire(
    source_after: Option<wire::PartyExportSourceContinuation>,
    source_exhausted: bool,
) -> Result<Option<PartyExportSourceContinuation>, SdkError> {
    match (source_after, source_exhausted) {
        (None, true) => Ok(None),
        (Some(value), false) => Ok(Some(PartyExportSourceContinuation::try_new(
            value.sort_value,
            RecordId::try_new(value.record_id).map_err(|error| {
                SdkError::invalid_argument(
                    "customer_data.export.selection.source_after.record_id",
                    error.to_string(),
                )
            })?,
        )?)),
        _ => Err(SdkError::invalid_argument(
            "customer_data.export.selection.source_after",
            "a non-final selection page requires a continuation and a final page must omit it",
        )),
    }
}

fn bytes_to_sha256_hex(bytes: &[u8]) -> Result<String, SdkError> {
    if bytes.len() != 32 {
        return Err(SdkError::invalid_argument(
            "customer_data.export.selection.manifest_sha256",
            "manifest SHA-256 must contain exactly 32 bytes",
        ));
    }
    let mut output = String::with_capacity(64);
    for byte in bytes {
        use std::fmt::Write as _;
        write!(&mut output, "{byte:02x}").expect("writing to String cannot fail");
    }
    Ok(output)
}

fn ensure_definition(
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
) -> Result<(), SdkError> {
    if definition.owner_module_id.as_str() != MODULE_ID
        || definition.capability_version.as_str() != support::CONTRACT_VERSION
        || request.context.capability_id != definition.capability_id
        || request.context.capability_version != definition.capability_version
        || !INTERNAL_EXPORT_SELECTION_CAPABILITY_IDS.contains(&definition.capability_id.as_str())
    {
        return Err(unsupported_internal_capability());
    }
    Ok(())
}

fn decode_request<M: Message + Default>(
    request: &CapabilityRequest,
    schema_id: &str,
) -> Result<M, SdkError> {
    let payload = &request.input;
    if payload.owner.as_str() != MODULE_ID
        || payload.schema_id.as_str() != schema_id
        || payload.schema_version.as_str() != support::CONTRACT_VERSION
        || payload.descriptor_hash != support::message_descriptor_hash(schema_id)
        || payload.data_class != DataClass::Personal
        || payload.encoding != PayloadEncoding::Protobuf
        || payload.maximum_size_bytes > support::MAX_PROTOBUF_BYTES
        || payload.validate().is_err()
    {
        return Err(SdkError::new(
            "CUSTOMER_DATA_EXPORT_INTERNAL_INPUT_CONTRACT_MISMATCH",
            ErrorCategory::InvalidArgument,
            false,
            "The customer export worker input does not match the required contract.",
        ));
    }
    M::decode(payload.bytes.as_slice()).map_err(|_| {
        SdkError::new(
            "CUSTOMER_DATA_EXPORT_INTERNAL_INPUT_PROTOBUF_INVALID",
            ErrorCategory::InvalidArgument,
            false,
            "The customer export worker input is not valid Protobuf.",
        )
    })
}

fn configured<T>(value: Result<T, crm_module_sdk::IdentifierError>) -> Result<T, SdkError> {
    value.map_err(|error| {
        SdkError::new(
            "CUSTOMER_DATA_EXPORT_INTERNAL_CONFIGURATION_INVALID",
            ErrorCategory::Internal,
            false,
            "The customer export worker capability is not configured safely.",
        )
        .with_internal_reference(error.to_string())
    })
}

fn invalid_plan() -> SdkError {
    SdkError::new(
        "CUSTOMER_DATA_EXPORT_INTERNAL_PLAN_INVALID",
        ErrorCategory::Conflict,
        false,
        "The customer export worker outcome cannot be applied to the current state.",
    )
}

fn stored_state_invalid() -> SdkError {
    SdkError::new(
        "CUSTOMER_DATA_EXPORT_INTERNAL_STORED_STATE_INVALID",
        ErrorCategory::Internal,
        false,
        "Stored customer export worker state is invalid.",
    )
}

fn unsupported_internal_capability() -> SdkError {
    SdkError::new(
        "CUSTOMER_DATA_EXPORT_INTERNAL_CAPABILITY_UNSUPPORTED",
        ErrorCategory::Internal,
        false,
        "The customer export worker capability is not configured.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn publishes_only_private_versioned_selection_coordinates() {
        let definitions = internal_export_selection_capability_definitions().unwrap();
        assert_eq!(definitions.len(), 2);
        assert_eq!(
            definitions
                .iter()
                .map(|definition| definition.capability_id.as_str())
                .collect::<Vec<_>>(),
            INTERNAL_EXPORT_SELECTION_CAPABILITY_IDS
        );
        assert!(definitions.iter().all(|definition| {
            definition.mutation
                && definition.requires_idempotency
                && !definition.requires_approval
                && definition.input_contract.allowed_data_classes == vec![DataClass::Personal]
        }));
    }

    #[test]
    fn continuation_requires_exact_final_or_non_final_shape() {
        assert!(source_continuation_from_wire(None, true).is_ok());
        assert!(source_continuation_from_wire(None, false).is_err());
        assert!(
            source_continuation_from_wire(
                Some(wire::PartyExportSourceContinuation {
                    sort_value: "2026-07-15T00:00:00Z".to_owned(),
                    record_id: "party-1".to_owned(),
                }),
                false,
            )
            .is_ok()
        );
        assert!(
            source_continuation_from_wire(
                Some(wire::PartyExportSourceContinuation {
                    sort_value: "2026-07-15T00:00:00Z".to_owned(),
                    record_id: "party-1".to_owned(),
                }),
                true,
            )
            .is_err()
        );
    }

    #[test]
    fn manifest_digest_requires_exact_sha256_bytes() {
        assert_eq!(bytes_to_sha256_hex(&[0xAB; 32]).unwrap(), "ab".repeat(32));
        assert!(bytes_to_sha256_hex(&[0xAB; 31]).is_err());
    }
}
