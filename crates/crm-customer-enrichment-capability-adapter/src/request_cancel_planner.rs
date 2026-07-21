#[path = "request_dispatch_planner.rs"]
mod dispatch_planner;
pub use dispatch_planner::CustomerEnrichmentRequestDispatchPlanner;

use crate::{
    MODULE_ID, enrichment_request_from_snapshot, enrichment_request_persisted_payload,
    enrichment_request_to_wire,
};
use crm_capability_plan_support::{self as support, EventSpec};
use crm_capability_runtime::{CapabilityDefinition, CapabilityRequest};
use crm_core_data::{
    AggregatePresence, AggregateTarget, BatchMutationPlan, CapabilityBatchExecutionPlan,
    RecordMutation, TransactionalAggregatePlanner,
};
use crm_customer_enrichment::ENRICHMENT_REQUEST_RECORD_TYPE;
use crm_module_sdk::{DataClass, ErrorCategory, RecordId, RecordRef, RecordSnapshot, SdkError};
use crm_proto_contracts::crm::customer_enrichment::v1 as wire;

pub const CANCEL_ENRICHMENT_REQUEST_CAPABILITY: &str = "customer_enrichment.request.cancel";
pub const CANCEL_ENRICHMENT_REQUEST_REQUEST_SCHEMA: &str =
    "crm.customer_enrichment.v1.CancelEnrichmentRequestRequest";
pub const CANCEL_ENRICHMENT_REQUEST_RESPONSE_SCHEMA: &str =
    "crm.customer_enrichment.v1.CancelEnrichmentRequestResponse";
pub const ENRICHMENT_REQUEST_STATUS_CHANGED_EVENT_TYPE: &str =
    "customer_enrichment.request.status_changed";
pub const ENRICHMENT_REQUEST_STATUS_CHANGED_EVENT_SCHEMA: &str =
    "crm.customer_enrichment.v1.EnrichmentRequestStatusChangedEvent";

#[derive(Debug, Default, Clone, Copy)]
pub struct CustomerEnrichmentRequestCancelPlanner;

impl TransactionalAggregatePlanner for CustomerEnrichmentRequestCancelPlanner {
    fn target(
        &self,
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
    ) -> Result<AggregateTarget, SdkError> {
        ensure_definition(definition, request)?;
        Ok(AggregateTarget {
            reference: cancel_request_record_ref(request)?,
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
        let current = current.ok_or_else(request_not_found)?;
        let expected_reference = cancel_request_record_ref(request)?;
        if current.reference != expected_reference || current.version <= 0 {
            return Err(invalid_plan(
                "locked enrichment request differs from the cancellation target",
            ));
        }
        let mut enrichment_request = enrichment_request_from_snapshot(current)?;
        enrichment_request.cancel(request_started_at_unix_ms(request)?)?;
        let output_request = enrichment_request_to_wire(&enrichment_request)?;
        let output = support::protobuf_payload(
            MODULE_ID,
            CANCEL_ENRICHMENT_REQUEST_RESPONSE_SCHEMA,
            DataClass::Personal,
            &wire::CancelEnrichmentRequestResponse {
                enrichment_request: Some(output_request.clone()),
            },
        )?;
        let next_version = current
            .version
            .checked_add(1)
            .ok_or_else(|| invalid_plan("enrichment request version overflow"))?;
        let event = support::event_evidence_with_data_class(
            request,
            current.reference.clone(),
            MODULE_ID,
            EventSpec {
                event_type: ENRICHMENT_REQUEST_STATUS_CHANGED_EVENT_TYPE,
                event_schema_id: ENRICHMENT_REQUEST_STATUS_CHANGED_EVENT_SCHEMA,
                aggregate_version: next_version,
                previous_version: Some(current.version),
            },
            DataClass::Personal,
            &wire::EnrichmentRequestStatusChangedEvent {
                enrichment_request: Some(output_request),
            },
        )?;
        let audit = support::audit_intent(
            request,
            &current.reference,
            next_version,
            definition.capability_id.as_str(),
            &output.bytes,
        )?;
        Ok(CapabilityBatchExecutionPlan {
            batch: BatchMutationPlan {
                context: request.context.clone(),
                records: vec![RecordMutation::Update {
                    reference: current.reference.clone(),
                    expected_version: current.version,
                    payload: enrichment_request_persisted_payload(&enrichment_request)?,
                }],
                relationships: Vec::new(),
                events: vec![event],
                idempotency: support::capability_idempotency(definition, request)?,
                audits: vec![audit],
            },
            output: Some(output),
        })
    }
}

pub fn cancel_request_record_ref(request: &CapabilityRequest) -> Result<RecordRef, SdkError> {
    let command: wire::CancelEnrichmentRequestRequest = support::decode_request_with_data_class(
        request,
        MODULE_ID,
        CANCEL_ENRICHMENT_REQUEST_REQUEST_SCHEMA,
        DataClass::Personal,
    )?;
    let request_ref = command.enrichment_request_ref.ok_or_else(|| {
        SdkError::invalid_argument(
            "customer_enrichment.enrichment_request_ref",
            "Enrichment-request reference is required",
        )
    })?;
    support::record_ref(
        ENRICHMENT_REQUEST_RECORD_TYPE,
        RecordId::try_new(request_ref.enrichment_request_id)
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

fn ensure_definition(
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
) -> Result<(), SdkError> {
    if definition.capability_id.as_str() != CANCEL_ENRICHMENT_REQUEST_CAPABILITY
        || definition.owner_module_id.as_str() != MODULE_ID
        || definition.capability_id.as_str() != request.context.execution.capability_id.as_str()
    {
        return Err(invalid_plan(
            "capability definition does not match request context",
        ));
    }
    Ok(())
}

fn request_started_at_unix_ms(request: &CapabilityRequest) -> Result<u64, SdkError> {
    let nanos = request.context.execution.request_started_at_unix_nanos;
    if nanos < 0 {
        return Err(invalid_plan("request start timestamp is negative"));
    }
    u64::try_from(nanos / 1_000_000)
        .map_err(|_| invalid_plan("request start timestamp cannot be represented in milliseconds"))
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
        "CUSTOMER_ENRICHMENT_REQUEST_CANCEL_PLAN_INVALID",
        ErrorCategory::Internal,
        false,
        "The enrichment request cancellation could not be planned safely.",
    )
    .with_internal_reference(reference.into())
}
