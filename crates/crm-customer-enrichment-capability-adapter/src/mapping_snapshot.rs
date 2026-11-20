use crate::{MAPPING_VERSION_RECORD_TYPE, mapping_persisted_contract};
use crm_capability_plan_support as support;
use crm_customer_enrichment::{MappingVersion, decode_mapping_version_state};
use crm_module_sdk::{DataClass, ErrorCategory, RecordSnapshot, SdkError};

/// Rehydrates one immutable mapping snapshot after exact persisted-contract validation.
pub fn mapping_from_snapshot(snapshot: &RecordSnapshot) -> Result<MappingVersion, SdkError> {
    if snapshot.reference.record_type.as_str() != MAPPING_VERSION_RECORD_TYPE
        || snapshot.version != 1
    {
        return Err(invalid_snapshot(
            "record type or immutable version is invalid",
        ));
    }
    let bytes = support::persisted_json_bytes_with_data_class(
        snapshot,
        mapping_persisted_contract(),
        DataClass::Confidential,
    )?;
    let mapping = decode_mapping_version_state(bytes)?;
    if snapshot.reference.record_id.as_str() != mapping.version_id().as_str() {
        return Err(invalid_snapshot(
            "record identity differs from the content-derived mapping identity",
        ));
    }
    Ok(mapping)
}

fn invalid_snapshot(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_MAPPING_PERSISTED_STATE_INVALID",
        ErrorCategory::Internal,
        false,
        "The persisted mapping version is invalid.",
    )
    .with_internal_reference(reference.into())
}
