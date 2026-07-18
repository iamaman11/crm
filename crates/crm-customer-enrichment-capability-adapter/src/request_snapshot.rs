use crate::enrichment_request_persisted_contract;
use crm_capability_plan_support as support;
use crm_customer_enrichment::{
    EnrichmentRequest, ENRICHMENT_REQUEST_RECORD_TYPE, decode_enrichment_request_state,
};
use crm_module_sdk::{DataClass, ErrorCategory, RecordSnapshot, SdkError};

/// Rehydrates one mutable enrichment-request snapshot after exact persisted-contract validation.
pub fn enrichment_request_from_snapshot(
    snapshot: &RecordSnapshot,
) -> Result<EnrichmentRequest, SdkError> {
    if snapshot.reference.record_type.as_str() != ENRICHMENT_REQUEST_RECORD_TYPE
        || snapshot.version <= 0
    {
        return Err(invalid_snapshot(
            "record type or mutable resource version is invalid",
        ));
    }
    let bytes = support::persisted_json_bytes_with_data_class(
        snapshot,
        enrichment_request_persisted_contract(),
        DataClass::Personal,
    )?;
    let enrichment_request = decode_enrichment_request_state(bytes)?;
    if snapshot.reference.record_id.as_str() != enrichment_request.request_id().as_str() {
        return Err(invalid_snapshot(
            "record identity differs from the deterministic enrichment-request identity",
        ));
    }
    Ok(enrichment_request)
}

fn invalid_snapshot(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_REQUEST_PERSISTED_STATE_INVALID",
        ErrorCategory::Internal,
        false,
        "The persisted enrichment request is invalid.",
    )
    .with_internal_reference(reference.into())
}
