use crate::{provider_profile_persisted_contract, PROVIDER_PROFILE_VERSION_RECORD_TYPE};
use crm_capability_plan_support as support;
use crm_customer_enrichment::{decode_provider_profile_version_state, ProviderProfileVersion};
use crm_module_sdk::{DataClass, ErrorCategory, RecordSnapshot, SdkError};

/// Rehydrates one immutable provider-profile snapshot after exact persisted-contract validation.
pub fn provider_profile_from_snapshot(
    snapshot: &RecordSnapshot,
) -> Result<ProviderProfileVersion, SdkError> {
    if snapshot.reference.record_type.as_str() != PROVIDER_PROFILE_VERSION_RECORD_TYPE
        || snapshot.version != 1
    {
        return Err(invalid_snapshot("record type or immutable version is invalid"));
    }
    let bytes = support::persisted_json_bytes_with_data_class(
        snapshot,
        provider_profile_persisted_contract(),
        DataClass::Confidential,
    )?;
    let profile = decode_provider_profile_version_state(bytes)?;
    if snapshot.reference.record_id.as_str() != profile.version_id().as_str() {
        return Err(invalid_snapshot(
            "record identity differs from the content-derived provider-profile identity",
        ));
    }
    Ok(profile)
}

fn invalid_snapshot(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_PROVIDER_PROFILE_PERSISTED_STATE_INVALID",
        ErrorCategory::Internal,
        false,
        "The persisted provider profile is invalid.",
    )
    .with_internal_reference(reference.into())
}
