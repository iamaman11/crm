use crate::{
    CustomerEnrichmentRequestCreateCapabilityPlanner,
    ENRICHMENT_REQUEST_STATUS_CHANGED_EVENT_SCHEMA, ENRICHMENT_REQUEST_STATUS_CHANGED_EVENT_TYPE,
    MODULE_ID, ProviderResponseEvidence, RECORD_PROVIDER_RESPONSE_CAPABILITY,
    RECORD_PROVIDER_RESPONSE_REQUEST_SCHEMA, RECORD_PROVIDER_RESPONSE_RESPONSE_SCHEMA,
    REQUEST_PARTY_SOURCE_RECORD_TYPE, ResponseExpectation, enrichment_request_from_create_request,
    enrichment_request_from_snapshot, enrichment_request_persisted_payload,
    enrichment_request_to_wire, prepare_provider_response,
};
use crm_capability_plan_support::{self as support, EventSpec, PersistedPayloadContract};
use crm_capability_runtime::{CapabilityDefinition, CapabilityRequest};
use crm_core_data::{
    AggregatePresence, AggregateTarget, BatchMutationPlan, CapabilityBatchExecutionPlan,
    RecordMutation, TransactionalAggregatePlanner,
};
use crm_customer_enrichment::{
    ENRICHMENT_REQUEST_RECORD_TYPE, EnrichmentRequestStatus, LIFECYCLE_STATE_RETENTION_POLICY_ID,
    LIFECYCLE_STATE_SCHEMA_VERSION, PROVIDER_RESPONSE_RECEIPT_RECORD_TYPE,
    PROVIDER_RESPONSE_RECEIPT_STATE_MAXIMUM_BYTES, PROVIDER_RESPONSE_RECEIPT_STATE_SCHEMA_ID,
    PROVIDER_USAGE_ENTRY_RECORD_TYPE, PROVIDER_USAGE_ENTRY_STATE_MAXIMUM_BYTES,
    PROVIDER_USAGE_ENTRY_STATE_RETENTION_POLICY_ID, PROVIDER_USAGE_ENTRY_STATE_SCHEMA_ID,
    PROVIDER_USAGE_ENTRY_STATE_SCHEMA_VERSION, ProviderResponseClass, ProviderResponseReceipt,
    ProviderUsageEntry, ProviderUsageEntryDraft, ProviderUsageKind,
    encode_provider_response_receipt_state, encode_provider_usage_entry_state,
    provider_response_receipt_state_descriptor_hash, provider_usage_entry_state_descriptor_hash,
};
use crm_module_sdk::{
    DataClass, ErrorCategory, RecordId, RecordRef, RecordSnapshot, RecordType, SdkError,
    TypedPayload,
};
use crm_proto_contracts::crm::customer_enrichment::v1 as wire;
use serde::Deserialize;

const PROVIDER_RESPONSE_RECORDED_EVENT_TYPE: &str =
    "customer_enrichment.provider_response.recorded";
const PROVIDER_RESPONSE_RECORDED_EVENT_SCHEMA: &str =
    "crm.customer_enrichment.v1.ProviderResponseRecordedEvent";
const PROVIDER_USAGE_RECORDED_EVENT_TYPE: &str = "customer_enrichment.provider_usage.recorded";
const PROVIDER_USAGE_RECORDED_EVENT_SCHEMA: &str =
    "crm.customer_enrichment.v1.ProviderUsageRecordedEvent";

/// Locks the exact Party aggregate before the immutable request record, relationship, outbox,
/// idempotency and audit evidence are created in the same database transaction.
#[derive(Debug, Default, Clone, Copy)]
pub struct CustomerEnrichmentRequestReferencePlanner;

impl TransactionalAggregatePlanner for CustomerEnrichmentRequestReferencePlanner {
    fn target(
        &self,
        _definition: &CapabilityDefinition,
        request: &CapabilityRequest,
    ) -> Result<AggregateTarget, SdkError> {
        let enrichment_request = enrichment_request_from_create_request(request)?;
        Ok(AggregateTarget {
            reference: party_record_ref(enrichment_request.target().resource_id.as_str())?,
            presence: AggregatePresence::MustExist,
        })
    }

    fn plan(
        &self,
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
        current: Option<&RecordSnapshot>,
    ) -> Result<CapabilityBatchExecutionPlan, SdkError> {
        let enrichment_request = enrichment_request_from_create_request(request)?;
        let expected_reference =
            party_record_ref(enrichment_request.target().resource_id.as_str())?;
        let snapshot = current.ok_or_else(target_unavailable)?;
        let expected_version = i64::try_from(enrichment_request.target().resource_version)
            .map_err(|_| {
                stale_target("requested Party resource version exceeds the storage range")
            })?;
        if snapshot.reference != expected_reference || snapshot.version != expected_version {
            return Err(stale_target(
                "locked Party snapshot differs from the exact request target version",
            ));
        }
        CustomerEnrichmentRequestCreateCapabilityPlanner.plan(definition, request, None)
    }
}

/// Atomic post-provider-I/O planner. Provider selection, credential resolution and network I/O
/// happen before this planner. The planner locks one exact request and commits request state,
/// immutable receipt, provider-usage evidence, idempotency, outbox and audit in one transaction.
/// It remains outside production routing until the infrastructure worker owns the pre-I/O and
/// crash-window protocol.
#[derive(Debug, Default, Clone, Copy)]
pub struct CustomerEnrichmentProviderResponseReferencePlanner;

impl TransactionalAggregatePlanner for CustomerEnrichmentProviderResponseReferencePlanner {
    fn target(
        &self,
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
    ) -> Result<AggregateTarget, SdkError> {
        ensure_response_definition(definition, request)?;
        Ok(AggregateTarget {
            reference: provider_response_request_record_ref(request)?,
            presence: AggregatePresence::MustExist,
        })
    }

    fn plan(
        &self,
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
        current: Option<&RecordSnapshot>,
    ) -> Result<CapabilityBatchExecutionPlan, SdkError> {
        ensure_response_definition(definition, request)?;
        let command = provider_response_command(request)?;
        let current = current.ok_or_else(response_request_not_found)?;
        let expected_reference = provider_response_record_ref_from_command(&command)?;
        if current.reference != expected_reference || current.version <= 0 {
            return Err(response_plan_invalid(
                "locked enrichment request differs from the provider-response target",
            ));
        }

        let mut enrichment_request = enrichment_request_from_snapshot(current)?;
        let evidence = provider_response_evidence(&command)?;
        let receipt = prepare_provider_response(
            &mut enrichment_request,
            ResponseExpectation {
                status: EnrichmentRequestStatus::Dispatched,
                retry_generation: command.expected_retry_generation,
            },
            evidence,
        )?;
        let usage_entries = provider_usage_entries(&enrichment_request, &receipt, &command)?;

        let output_request = enrichment_request_to_wire(&enrichment_request)?;
        let output_receipt = provider_response_receipt_to_wire(&receipt)?;
        let output_usage = usage_entries
            .iter()
            .map(provider_usage_entry_to_wire)
            .collect::<Result<Vec<_>, _>>()?;
        let output = support::protobuf_payload(
            MODULE_ID,
            RECORD_PROVIDER_RESPONSE_RESPONSE_SCHEMA,
            DataClass::Personal,
            &wire::RecordProviderResponseResponse {
                enrichment_request: Some(output_request.clone()),
                provider_response_receipt: Some(output_receipt.clone()),
                provider_usage_entries: output_usage.clone(),
            },
        )?;

        let next_request_version = current
            .version
            .checked_add(1)
            .ok_or_else(|| response_plan_invalid("enrichment request version overflow"))?;
        let receipt_reference = provider_response_receipt_record_ref(&receipt)?;
        let usage_references = usage_entries
            .iter()
            .map(provider_usage_entry_record_ref)
            .collect::<Result<Vec<_>, _>>()?;

        let mut records = vec![
            RecordMutation::Update {
                reference: current.reference.clone(),
                expected_version: current.version,
                payload: enrichment_request_persisted_payload(&enrichment_request)?,
            },
            RecordMutation::Create {
                reference: receipt_reference.clone(),
                payload: provider_response_receipt_persisted_payload(&receipt)?,
            },
        ];
        records.extend(
            usage_entries
                .iter()
                .zip(&usage_references)
                .map(|(usage, reference)| {
                    Ok(RecordMutation::Create {
                        reference: reference.clone(),
                        payload: provider_usage_entry_persisted_payload(usage)?,
                    })
                })
                .collect::<Result<Vec<_>, SdkError>>()?,
        );

        let mut events = vec![
            support::event_evidence_with_data_class(
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
            )?,
            support::event_evidence_with_data_class(
                request,
                receipt_reference.clone(),
                MODULE_ID,
                EventSpec {
                    event_type: PROVIDER_RESPONSE_RECORDED_EVENT_TYPE,
                    event_schema_id: PROVIDER_RESPONSE_RECORDED_EVENT_SCHEMA,
                    aggregate_version: 1,
                    previous_version: None,
                },
                DataClass::Personal,
                &wire::ProviderResponseRecordedEvent {
                    provider_response_receipt: Some(output_receipt),
                },
            )?,
        ];
        events.extend(
            usage_references
                .iter()
                .zip(output_usage)
                .map(|(reference, usage)| {
                    support::event_evidence_with_data_class(
                        request,
                        reference.clone(),
                        MODULE_ID,
                        EventSpec {
                            event_type: PROVIDER_USAGE_RECORDED_EVENT_TYPE,
                            event_schema_id: PROVIDER_USAGE_RECORDED_EVENT_SCHEMA,
                            aggregate_version: 1,
                            previous_version: None,
                        },
                        DataClass::Confidential,
                        &wire::ProviderUsageRecordedEvent {
                            provider_usage_entry: Some(usage),
                        },
                    )
                })
                .collect::<Result<Vec<_>, SdkError>>()?,
        );

        let mut audits = vec![support::audit_intent(
            request,
            &current.reference,
            next_request_version,
            definition.capability_id.as_str(),
            &output.bytes,
        )?];
        audits.push(support::audit_intent(
            request,
            &receipt_reference,
            1,
            definition.capability_id.as_str(),
            &output.bytes,
        )?);
        audits.extend(
            usage_references
                .iter()
                .map(|reference| {
                    support::audit_intent(
                        request,
                        reference,
                        1,
                        definition.capability_id.as_str(),
                        &output.bytes,
                    )
                })
                .collect::<Result<Vec<_>, SdkError>>()?,
        );

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
}

pub fn provider_response_request_record_ref(
    request: &CapabilityRequest,
) -> Result<RecordRef, SdkError> {
    provider_response_record_ref_from_command(&provider_response_command(request)?)
}

fn provider_response_command(
    request: &CapabilityRequest,
) -> Result<wire::RecordProviderResponseRequest, SdkError> {
    support::decode_request_with_data_class(
        request,
        MODULE_ID,
        RECORD_PROVIDER_RESPONSE_REQUEST_SCHEMA,
        DataClass::Personal,
    )
}

fn provider_response_record_ref_from_command(
    command: &wire::RecordProviderResponseRequest,
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

fn provider_response_evidence(
    command: &wire::RecordProviderResponseRequest,
) -> Result<ProviderResponseEvidence, SdkError> {
    let canonical_response_digest: [u8; 32] = command
        .canonical_response_digest
        .clone()
        .try_into()
        .map_err(|_| {
            SdkError::invalid_argument(
                "customer_enrichment.response.canonical_response_digest",
                "Canonical response digest must contain exactly 32 bytes",
            )
        })?;
    Ok(ProviderResponseEvidence {
        replay_key: command.replay_key.clone(),
        provider_correlation_id: command.provider_correlation_id.clone(),
        response_class: provider_response_class(command.response_class)?,
        canonical_response_digest,
        provider_observed_at_unix_ms: command
            .provider_observed_at_unix_ms
            .map(|value| non_negative_time(value, "provider_observed_at_unix_ms"))
            .transpose()?,
        retrieved_at_unix_ms: non_negative_time(
            command.retrieved_at_unix_ms,
            "retrieved_at_unix_ms",
        )?,
        metered_units: command.metered_units,
        protected_evidence_reference: command.protected_evidence_reference.clone(),
    })
}

fn provider_usage_entries(
    enrichment_request: &crm_customer_enrichment::EnrichmentRequest,
    receipt: &ProviderResponseReceipt,
    command: &wire::RecordProviderResponseRequest,
) -> Result<Vec<ProviderUsageEntry>, SdkError> {
    let observed_at = command
        .provider_observed_at_unix_ms
        .map(|value| non_negative_time(value, "provider_observed_at_unix_ms"))
        .transpose()?;
    let recorded_at = non_negative_time(command.retrieved_at_unix_ms, "retrieved_at_unix_ms")?;
    let mut entries = vec![ProviderUsageEntry::record(ProviderUsageEntryDraft {
        request_id: enrichment_request.request_id().clone(),
        response_receipt_id: Some(receipt.receipt_id().clone()),
        provider_profile_version_id: enrichment_request.provider_profile_version_id().clone(),
        kind: ProviderUsageKind::ResponseReceived,
        metered_units: 0,
        quota_bucket: None,
        quota_remaining: None,
        provider_observed_at_unix_ms: observed_at,
        recorded_at_unix_ms: recorded_at,
        safe_provider_code: command.safe_provider_code.clone(),
    })?];
    if command.metered_units > 0 {
        entries.push(ProviderUsageEntry::record(ProviderUsageEntryDraft {
            request_id: enrichment_request.request_id().clone(),
            response_receipt_id: Some(receipt.receipt_id().clone()),
            provider_profile_version_id: enrichment_request.provider_profile_version_id().clone(),
            kind: ProviderUsageKind::BillableUnits,
            metered_units: command.metered_units,
            quota_bucket: None,
            quota_remaining: None,
            provider_observed_at_unix_ms: observed_at,
            recorded_at_unix_ms: recorded_at,
            safe_provider_code: None,
        })?);
    }
    Ok(entries)
}

fn provider_response_receipt_persisted_payload(
    receipt: &ProviderResponseReceipt,
) -> Result<TypedPayload, SdkError> {
    support::persisted_json_payload_with_data_class(
        PersistedPayloadContract {
            owner: MODULE_ID,
            schema_id: PROVIDER_RESPONSE_RECEIPT_STATE_SCHEMA_ID,
            schema_version: LIFECYCLE_STATE_SCHEMA_VERSION,
            descriptor_hash: provider_response_receipt_state_descriptor_hash(),
            maximum_size_bytes: PROVIDER_RESPONSE_RECEIPT_STATE_MAXIMUM_BYTES,
            retention_policy_id: LIFECYCLE_STATE_RETENTION_POLICY_ID,
        },
        DataClass::Personal,
        encode_provider_response_receipt_state(receipt)?,
    )
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

fn provider_response_receipt_record_ref(
    receipt: &ProviderResponseReceipt,
) -> Result<RecordRef, SdkError> {
    support::record_ref(
        PROVIDER_RESPONSE_RECEIPT_RECORD_TYPE,
        receipt.receipt_id().as_str(),
        "customer_enrichment.provider_response_receipt_ref.provider_response_receipt_id",
    )
}

fn provider_usage_entry_record_ref(usage: &ProviderUsageEntry) -> Result<RecordRef, SdkError> {
    support::record_ref(
        PROVIDER_USAGE_ENTRY_RECORD_TYPE,
        usage.usage_entry_id().as_str(),
        "customer_enrichment.provider_usage_entry_ref.provider_usage_entry_id",
    )
}

fn provider_response_receipt_to_wire(
    receipt: &ProviderResponseReceipt,
) -> Result<wire::ProviderResponseReceipt, SdkError> {
    let state: ProviderResponseReceiptStateView =
        serde_json::from_slice(&encode_provider_response_receipt_state(receipt)?)
            .map_err(|error| response_plan_invalid(error.to_string()))?;
    Ok(wire::ProviderResponseReceipt {
        provider_response_receipt_ref: Some(wire::ProviderResponseReceiptRef {
            provider_response_receipt_id: state.receipt_id,
        }),
        enrichment_request_ref: Some(wire::EnrichmentRequestRef {
            enrichment_request_id: state.request_id,
        }),
        provider_profile_version_ref: Some(wire::ProviderProfileVersionRef {
            provider_profile_version_id: state.provider_profile_version_id,
        }),
        mapping_version_ref: Some(wire::MappingVersionRef {
            mapping_version_id: state.mapping_version_id,
        }),
        replay_key: state.replay_key,
        provider_correlation_id: state.provider_correlation_id,
        response_class: provider_response_class_to_wire(state.response_class),
        canonical_response_digest: state.canonical_response_digest.to_vec(),
        provider_observed_at_unix_ms: state
            .provider_observed_at_unix_ms
            .map(|value| checked_i64(value, "provider observed timestamp"))
            .transpose()?,
        retrieved_at_unix_ms: checked_i64(state.retrieved_at_unix_ms, "retrieved timestamp")?,
        metered_units: state.metered_units,
        protected_evidence_reference: state.protected_evidence_reference,
    })
}

fn provider_usage_entry_to_wire(
    usage: &ProviderUsageEntry,
) -> Result<wire::ProviderUsageEntry, SdkError> {
    let state: ProviderUsageEntryStateView =
        serde_json::from_slice(&encode_provider_usage_entry_state(usage)?)
            .map_err(|error| response_plan_invalid(error.to_string()))?;
    Ok(wire::ProviderUsageEntry {
        provider_usage_entry_ref: Some(wire::ProviderUsageEntryRef {
            provider_usage_entry_id: state.usage_entry_id,
        }),
        enrichment_request_ref: Some(wire::EnrichmentRequestRef {
            enrichment_request_id: state.request_id,
        }),
        provider_response_receipt_ref: state.response_receipt_id.map(|value| {
            wire::ProviderResponseReceiptRef {
                provider_response_receipt_id: value,
            }
        }),
        provider_profile_version_ref: Some(wire::ProviderProfileVersionRef {
            provider_profile_version_id: state.provider_profile_version_id,
        }),
        kind: provider_usage_kind_to_wire(state.kind),
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

fn provider_response_class(value: i32) -> Result<ProviderResponseClass, SdkError> {
    match wire::ProviderResponseClass::try_from(value) {
        Ok(wire::ProviderResponseClass::Success) => Ok(ProviderResponseClass::Success),
        Ok(wire::ProviderResponseClass::NoMatch) => Ok(ProviderResponseClass::NoMatch),
        Ok(wire::ProviderResponseClass::RetryableFailure) => {
            Ok(ProviderResponseClass::RetryableFailure)
        }
        Ok(wire::ProviderResponseClass::TerminalFailure) => {
            Ok(ProviderResponseClass::TerminalFailure)
        }
        Ok(wire::ProviderResponseClass::Unspecified) | Err(_) => Err(SdkError::invalid_argument(
            "customer_enrichment.response.response_class",
            "Provider response class must be specified",
        )),
    }
}

fn provider_response_class_to_wire(value: ProviderResponseClass) -> i32 {
    match value {
        ProviderResponseClass::Success => wire::ProviderResponseClass::Success as i32,
        ProviderResponseClass::NoMatch => wire::ProviderResponseClass::NoMatch as i32,
        ProviderResponseClass::RetryableFailure => {
            wire::ProviderResponseClass::RetryableFailure as i32
        }
        ProviderResponseClass::TerminalFailure => {
            wire::ProviderResponseClass::TerminalFailure as i32
        }
    }
}

fn provider_usage_kind_to_wire(value: ProviderUsageKind) -> i32 {
    match value {
        ProviderUsageKind::RequestDispatched => wire::ProviderUsageKind::RequestDispatched as i32,
        ProviderUsageKind::ResponseReceived => wire::ProviderUsageKind::ResponseReceived as i32,
        ProviderUsageKind::BillableUnits => wire::ProviderUsageKind::BillableUnits as i32,
        ProviderUsageKind::QuotaSnapshot => wire::ProviderUsageKind::QuotaSnapshot as i32,
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProviderResponseReceiptStateView {
    receipt_id: String,
    request_id: String,
    provider_profile_version_id: String,
    mapping_version_id: String,
    replay_key: String,
    provider_correlation_id: Option<String>,
    response_class: ProviderResponseClass,
    canonical_response_digest: [u8; 32],
    provider_observed_at_unix_ms: Option<u64>,
    retrieved_at_unix_ms: u64,
    metered_units: u64,
    protected_evidence_reference: Option<String>,
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

fn ensure_response_definition(
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
) -> Result<(), SdkError> {
    if definition.capability_id.as_str() != RECORD_PROVIDER_RESPONSE_CAPABILITY
        || definition.owner_module_id.as_str() != MODULE_ID
        || definition.capability_id.as_str() != request.context.execution.capability_id.as_str()
    {
        return Err(response_plan_invalid(
            "capability definition does not match request context",
        ));
    }
    Ok(())
}

fn non_negative_time(value: i64, field: &'static str) -> Result<u64, SdkError> {
    u64::try_from(value).map_err(|_| {
        SdkError::invalid_argument(field, "Provider response timestamps must not be negative")
    })
}

fn checked_i64(value: u64, label: &'static str) -> Result<i64, SdkError> {
    i64::try_from(value).map_err(|_| response_plan_invalid(format!("{label} exceeds wire range")))
}

fn party_record_ref(party_id: &str) -> Result<RecordRef, SdkError> {
    Ok(RecordRef {
        record_type: RecordType::try_new(REQUEST_PARTY_SOURCE_RECORD_TYPE)
            .map_err(configuration_error)?,
        record_id: RecordId::try_new(party_id).map_err(configuration_error)?,
    })
}

fn target_unavailable() -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_REQUEST_TARGET_UNAVAILABLE",
        ErrorCategory::NotFound,
        false,
        "The Party target is unavailable.",
    )
}

fn stale_target(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_REQUEST_TARGET_STALE",
        ErrorCategory::Conflict,
        false,
        "The Party resource version changed before the enrichment request was committed.",
    )
    .with_internal_reference(reference.into())
}

fn response_request_not_found() -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_REQUEST_NOT_FOUND",
        ErrorCategory::NotFound,
        false,
        "The requested enrichment request was not found.",
    )
}

fn response_plan_invalid(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_PLAN_INVALID",
        ErrorCategory::Internal,
        false,
        "The provider response could not be committed safely.",
    )
    .with_internal_reference(reference.into())
}

fn configuration_error(error: impl std::fmt::Display) -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_REQUEST_CONFIGURATION_INVALID",
        ErrorCategory::Internal,
        false,
        "The enrichment request capability is not configured safely.",
    )
    .with_internal_reference(error.to_string())
}
