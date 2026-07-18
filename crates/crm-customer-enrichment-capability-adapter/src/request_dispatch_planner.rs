use crate::{
    DISPATCH_ENRICHMENT_REQUEST_CAPABILITY, DISPATCH_ENRICHMENT_REQUEST_REQUEST_SCHEMA,
    DISPATCH_ENRICHMENT_REQUEST_RESPONSE_SCHEMA, DispatchExpectation,
    ENRICHMENT_REQUEST_STATUS_CHANGED_EVENT_SCHEMA, ENRICHMENT_REQUEST_STATUS_CHANGED_EVENT_TYPE,
    MODULE_ID, enrichment_request_from_snapshot, enrichment_request_persisted_payload,
    enrichment_request_to_wire, prepare_request_dispatch,
};
use crm_capability_plan_support::{self as support, EventSpec, PersistedPayloadContract};
use crm_capability_runtime::{CapabilityDefinition, CapabilityRequest};
use crm_core_data::{
    AggregatePresence, AggregateTarget, BatchMutationPlan, CapabilityBatchExecutionPlan,
    RecordMutation, TransactionalAggregatePlanner,
};
use crm_customer_enrichment::{
    ENRICHMENT_REQUEST_RECORD_TYPE, EnrichmentRequestStatus, PROVIDER_USAGE_ENTRY_RECORD_TYPE,
    PROVIDER_USAGE_ENTRY_STATE_MAXIMUM_BYTES, PROVIDER_USAGE_ENTRY_STATE_RETENTION_POLICY_ID,
    PROVIDER_USAGE_ENTRY_STATE_SCHEMA_ID, PROVIDER_USAGE_ENTRY_STATE_SCHEMA_VERSION,
    ProviderUsageEntry, ProviderUsageEntryDraft, ProviderUsageKind,
    encode_provider_usage_entry_state, provider_usage_entry_state_descriptor_hash,
};
use crm_module_sdk::{
    DataClass, ErrorCategory, RecordId, RecordRef, RecordSnapshot, SdkError, TypedPayload,
};
use crm_proto_contracts::crm::customer_enrichment::v1 as wire;
use serde::Deserialize;

const PROVIDER_USAGE_RECORDED_EVENT_TYPE: &str = "customer_enrichment.provider_usage.recorded";
const PROVIDER_USAGE_RECORDED_EVENT_SCHEMA: &str =
    "crm.customer_enrichment.v1.ProviderUsageRecordedEvent";

/// Atomic pre-I/O state planner for the non-runtime provider dispatch worker.
///
/// This planner locks the exact enrichment-request row and commits the final `Dispatched` state
/// together with immutable RequestDispatched evidence, idempotency, outbox and audits. Provider
/// selection and network I/O remain outside the transaction and occur only after this plan commits.
#[derive(Debug, Default, Clone, Copy)]
pub struct CustomerEnrichmentRequestDispatchPlanner;

impl TransactionalAggregatePlanner for CustomerEnrichmentRequestDispatchPlanner {
    fn target(
        &self,
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
    ) -> Result<AggregateTarget, SdkError> {
        ensure_definition(definition, request)?;
        Ok(AggregateTarget {
            reference: dispatch_request_record_ref(request)?,
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
        let command = dispatch_command(request)?;
        let current = current.ok_or_else(request_not_found)?;
        let expected_reference = dispatch_record_ref_from_command(&command)?;
        if current.reference != expected_reference || current.version <= 0 {
            return Err(invalid_plan(
                "locked enrichment request differs from the provider-dispatch target",
            ));
        }

        let dispatched_at_unix_ms = request_started_at_unix_ms(request)?;
        let mut enrichment_request = enrichment_request_from_snapshot(current)?;
        prepare_request_dispatch(
            &mut enrichment_request,
            DispatchExpectation {
                status: dispatch_status(command.expected_status)?,
                retry_generation: command.expected_retry_generation,
            },
            dispatched_at_unix_ms,
        )?;
        let usage = ProviderUsageEntry::record(ProviderUsageEntryDraft {
            request_id: enrichment_request.request_id().clone(),
            response_receipt_id: None,
            provider_profile_version_id: enrichment_request.provider_profile_version_id().clone(),
            kind: ProviderUsageKind::RequestDispatched,
            metered_units: 0,
            quota_bucket: None,
            quota_remaining: None,
            provider_observed_at_unix_ms: None,
            recorded_at_unix_ms: dispatched_at_unix_ms,
            safe_provider_code: None,
        })?;

        let output_request = enrichment_request_to_wire(&enrichment_request)?;
        let output = support::protobuf_payload(
            MODULE_ID,
            DISPATCH_ENRICHMENT_REQUEST_RESPONSE_SCHEMA,
            DataClass::Personal,
            &wire::DispatchEnrichmentRequestResponse {
                enrichment_request: Some(output_request.clone()),
            },
        )?;
        let output_usage = provider_usage_entry_to_wire(&usage)?;
        let next_request_version = current
            .version
            .checked_add(1)
            .ok_or_else(|| invalid_plan("enrichment request version overflow"))?;
        let usage_reference = provider_usage_entry_record_ref(&usage)?;

        let status_event = support::event_evidence_with_data_class(
            request,
            current.reference.clone(),
            MODULE_ID,
            EventSpec {
                event_type: ENRICHMENT_REQUEST_STATUS_CHANGED_EVENT_TYPE,
                event_schema_id: ENRICHMENT_REQUEST_STATUS_CHANGED_EVENT_SCHEMA,
                aggregate_version: next_request_version,
                previous_version: Some(current.version),
            },
            DataClass::Personal,
            &wire::EnrichmentRequestStatusChangedEvent {
                enrichment_request: Some(output_request),
            },
        )?;
        let usage_event = support::event_evidence_with_data_class(
            request,
            usage_reference.clone(),
            MODULE_ID,
            EventSpec {
                event_type: PROVIDER_USAGE_RECORDED_EVENT_TYPE,
                event_schema_id: PROVIDER_USAGE_RECORDED_EVENT_SCHEMA,
                aggregate_version: 1,
                previous_version: None,
            },
            DataClass::Confidential,
            &wire::ProviderUsageRecordedEvent {
                provider_usage_entry: Some(output_usage),
            },
        )?;

        let request_audit = support::audit_intent(
            request,
            &current.reference,
            next_request_version,
            definition.capability_id.as_str(),
            &output.bytes,
        )?;
        let usage_audit = support::audit_intent(
            request,
            &usage_reference,
            1,
            definition.capability_id.as_str(),
            &output.bytes,
        )?;

        Ok(CapabilityBatchExecutionPlan {
            batch: BatchMutationPlan {
                context: request.context.clone(),
                records: vec![
                    RecordMutation::Update {
                        reference: current.reference.clone(),
                        expected_version: current.version,
                        payload: enrichment_request_persisted_payload(&enrichment_request)?,
                    },
                    RecordMutation::Create {
                        reference: usage_reference,
                        payload: provider_usage_entry_persisted_payload(&usage)?,
                    },
                ],
                relationships: Vec::new(),
                events: vec![status_event, usage_event],
                idempotency: support::capability_idempotency(definition, request)?,
                audits: vec![request_audit, usage_audit],
            },
            output: Some(output),
        })
    }
}

fn dispatch_command(
    request: &CapabilityRequest,
) -> Result<wire::DispatchEnrichmentRequestRequest, SdkError> {
    support::decode_request_with_data_class(
        request,
        MODULE_ID,
        DISPATCH_ENRICHMENT_REQUEST_REQUEST_SCHEMA,
        DataClass::Personal,
    )
}

fn dispatch_request_record_ref(request: &CapabilityRequest) -> Result<RecordRef, SdkError> {
    dispatch_record_ref_from_command(&dispatch_command(request)?)
}

fn dispatch_record_ref_from_command(
    command: &wire::DispatchEnrichmentRequestRequest,
) -> Result<RecordRef, SdkError> {
    let request_ref = command.enrichment_request_ref.as_ref().ok_or_else(|| {
        SdkError::invalid_argument(
            "customer_enrichment.enrichment_request_ref",
            "Enrichment-request reference is required",
        )
    })?;
    support::record_ref(
        ENRICHMENT_REQUEST_RECORD_TYPE,
        RecordId::try_new(request_ref.enrichment_request_id.clone())
            .map_err(|error| {
                SdkError::invalid_argument(
                    "customer_enrichment.enrichment_request_ref.enrichment_request_id",
                    error.to_string(),
                )
            })?
            .as_str(),
        "customer_enrichment.enrichment_request_ref.enrichment_request_id",
    )
}

fn dispatch_status(value: i32) -> Result<EnrichmentRequestStatus, SdkError> {
    match wire::EnrichmentRequestStatus::try_from(value) {
        Ok(wire::EnrichmentRequestStatus::Created) => Ok(EnrichmentRequestStatus::Created),
        Ok(wire::EnrichmentRequestStatus::Queued) => Ok(EnrichmentRequestStatus::Queued),
        Ok(wire::EnrichmentRequestStatus::FailedRetryable) => {
            Ok(EnrichmentRequestStatus::FailedRetryable)
        }
        _ => Err(SdkError::invalid_argument(
            "customer_enrichment.dispatch.expected_status",
            "Dispatch expects Created, Queued or FailedRetryable status",
        )),
    }
}

fn provider_usage_entry_persisted_payload(
    usage: &ProviderUsageEntry,
) -> Result<TypedPayload, SdkError> {
    support::persisted_json_payload_with_data_class(
        PersistedPayloadContract {
            owner: MODULE_ID,
            schema_id: PROVIDER_USAGE_ENTRY_STATE_SCHEMA_ID,
            schema_version: PROVIDER_USAGE_ENTRY_STATE_SCHEMA_VERSION,
            descriptor_hash: provider_usage_entry_state_descriptor_hash(),
            maximum_size_bytes: PROVIDER_USAGE_ENTRY_STATE_MAXIMUM_BYTES,
            retention_policy_id: PROVIDER_USAGE_ENTRY_STATE_RETENTION_POLICY_ID,
        },
        DataClass::Confidential,
        encode_provider_usage_entry_state(usage)?,
    )
}

fn provider_usage_entry_record_ref(usage: &ProviderUsageEntry) -> Result<RecordRef, SdkError> {
    support::record_ref(
        PROVIDER_USAGE_ENTRY_RECORD_TYPE,
        usage.usage_entry_id().as_str(),
        "customer_enrichment.provider_usage_entry_ref.provider_usage_entry_id",
    )
}

fn provider_usage_entry_to_wire(
    usage: &ProviderUsageEntry,
) -> Result<wire::ProviderUsageEntry, SdkError> {
    let state: ProviderUsageEntryStateView =
        serde_json::from_slice(&encode_provider_usage_entry_state(usage)?)
            .map_err(|error| invalid_plan(error.to_string()))?;
    Ok(wire::ProviderUsageEntry {
        provider_usage_entry_ref: Some(wire::ProviderUsageEntryRef {
            provider_usage_entry_id: state.usage_entry_id,
        }),
        enrichment_request_ref: Some(wire::EnrichmentRequestRef {
            enrichment_request_id: state.request_id,
        }),
        provider_response_receipt_ref: None,
        provider_profile_version_ref: Some(wire::ProviderProfileVersionRef {
            provider_profile_version_id: state.provider_profile_version_id,
        }),
        kind: wire::ProviderUsageKind::RequestDispatched as i32,
        metered_units: state.metered_units,
        quota_bucket: state.quota_bucket,
        quota_remaining: state.quota_remaining,
        provider_observed_at_unix_ms: state
            .provider_observed_at_unix_ms
            .map(|value| checked_i64(value, "provider observed timestamp"))
            .transpose()?,
        recorded_at_unix_ms: checked_i64(state.recorded_at_unix_ms, "recorded timestamp")?,
        safe_provider_code: state.safe_provider_code,
    })
}

fn request_started_at_unix_ms(request: &CapabilityRequest) -> Result<u64, SdkError> {
    let nanos = request.context.execution.request_started_at_unix_nanos;
    if nanos < 0 {
        return Err(invalid_plan("request start timestamp is negative"));
    }
    u64::try_from(nanos / 1_000_000)
        .map_err(|_| invalid_plan("request start timestamp cannot be represented in milliseconds"))
}

fn checked_i64(value: u64, label: &'static str) -> Result<i64, SdkError> {
    i64::try_from(value).map_err(|_| invalid_plan(format!("{label} exceeds wire range")))
}

fn ensure_definition(
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
) -> Result<(), SdkError> {
    if definition.capability_id.as_str() != DISPATCH_ENRICHMENT_REQUEST_CAPABILITY
        || definition.owner_module_id.as_str() != MODULE_ID
        || definition.capability_id.as_str() != request.context.execution.capability_id.as_str()
    {
        return Err(invalid_plan(
            "capability definition does not match request context",
        ));
    }
    Ok(())
}

fn request_not_found() -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_REQUEST_NOT_FOUND",
        ErrorCategory::NotFound,
        false,
        "The requested enrichment request was not found.",
    )
}

fn invalid_plan(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_PROVIDER_DISPATCH_PLAN_INVALID",
        ErrorCategory::Internal,
        false,
        "The provider dispatch could not be committed safely.",
    )
    .with_internal_reference(reference.into())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProviderUsageEntryStateView {
    usage_entry_id: String,
    request_id: String,
    response_receipt_id: Option<String>,
    provider_profile_version_id: String,
    kind: ProviderUsageKind,
    metered_units: u64,
    quota_bucket: Option<String>,
    quota_remaining: Option<u64>,
    provider_observed_at_unix_ms: Option<u64>,
    recorded_at_unix_ms: u64,
    safe_provider_code: Option<String>,
}
