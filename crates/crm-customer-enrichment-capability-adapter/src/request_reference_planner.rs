use crate::{
    CustomerEnrichmentRequestCreateCapabilityPlanner, REQUEST_PARTY_SOURCE_RECORD_TYPE,
    enrichment_request_from_create_request,
};
use crm_capability_runtime::{CapabilityDefinition, CapabilityRequest};
use crm_core_data::{
    AggregatePresence, AggregateTarget, CapabilityBatchExecutionPlan, TransactionalAggregatePlanner,
};
use crm_module_sdk::{ErrorCategory, RecordId, RecordRef, RecordSnapshot, RecordType, SdkError};

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
        let expected_reference = party_record_ref(enrichment_request.target().resource_id.as_str())?;
        let snapshot = current.ok_or_else(target_unavailable)?;
        let expected_version = i64::try_from(enrichment_request.target().resource_version)
            .map_err(|_| stale_target("requested Party resource version exceeds the storage range"))?;
        if snapshot.reference != expected_reference || snapshot.version != expected_version {
            return Err(stale_target(
                "locked Party snapshot differs from the exact request target version",
            ));
        }
        CustomerEnrichmentRequestCreateCapabilityPlanner.plan(definition, request, None)
    }
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

fn configuration_error(error: impl std::fmt::Display) -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_REQUEST_CONFIGURATION_INVALID",
        ErrorCategory::Internal,
        false,
        "The enrichment request capability is not configured safely.",
    )
    .with_internal_reference(error.to_string())
}
