use crate::{CustomerEnrichmentSuggestionQueryAdapter, query_configuration_invalid, suggestion_record_type};
use crm_core_data::{RecordQueryContinuation, RecordQuerySort};
use crm_module_sdk::{ErrorCategory, RecordId, SdkError};
use crm_proto_contracts::crm::{customer::v1::PartyRef, customer_enrichment::v1 as wire};
use crm_query_runtime::{CursorBinding, CursorContinuation, QueryRequest, normalized_filter_hash};

pub(super) const DEFAULT_PAGE_SIZE: u32 = 50;
pub(super) const MAXIMUM_PAGE_SIZE: u32 = 100;

pub(super) fn party_id(value: Option<PartyRef>) -> Result<RecordId, SdkError> {
    let value = value.ok_or_else(|| {
        SdkError::invalid_argument(
            "customer_enrichment.suggestion.list.party_ref",
            "Party reference is required",
        )
    })?;
    RecordId::try_new(value.party_id).map_err(|error| {
        SdkError::invalid_argument(
            "customer_enrichment.suggestion.list.party_ref.party_id",
            error.to_string(),
        )
    })
}

pub(super) fn profile_id(
    value: Option<wire::ProviderProfileVersionRef>,
) -> Result<RecordId, SdkError> {
    let value = value.ok_or_else(|| {
        SdkError::invalid_argument(
            "customer_enrichment.suggestion.list.provider_profile_version_ref",
            "Provider-profile version reference is required",
        )
    })?;
    RecordId::try_new(value.provider_profile_version_id).map_err(|error| {
        SdkError::invalid_argument(
            "customer_enrichment.suggestion.list.provider_profile_version_ref.provider_profile_version_id",
            error.to_string(),
        )
    })
}

pub(super) fn status(value: i32) -> Result<i32, SdkError> {
    match wire::SuggestionLifecycleStatus::try_from(value) {
        Ok(wire::SuggestionLifecycleStatus::Unspecified) | Err(_) => Err(
            SdkError::invalid_argument(
                "customer_enrichment.suggestion.list.status",
                "Status must be a supported non-unspecified suggestion status",
            ),
        ),
        Ok(_) => Ok(value),
    }
}

pub(super) fn page_size(value: i32) -> Result<u32, SdkError> {
    if value < 0 {
        return Err(SdkError::invalid_argument(
            "customer_enrichment.suggestion.list.page_size",
            "Page size must not be negative",
        ));
    }
    let value = u32::try_from(value).map_err(query_configuration_invalid)?;
    let value = if value == 0 { DEFAULT_PAGE_SIZE } else { value };
    if value > MAXIMUM_PAGE_SIZE {
        return Err(SdkError::invalid_argument(
            "customer_enrichment.suggestion.list.page_size",
            format!("Page size must not exceed {MAXIMUM_PAGE_SIZE}"),
        ));
    }
    Ok(value)
}

pub(super) fn binding(
    request: &QueryRequest,
    party_id: &RecordId,
    profile_id: &RecordId,
    status: Option<i32>,
    page_size: u32,
) -> Result<CursorBinding, SdkError> {
    let status = status
        .unwrap_or(wire::SuggestionLifecycleStatus::Unspecified as i32)
        .to_be_bytes();
    Ok(CursorBinding {
        tenant_id: request.context.tenant_id.clone(),
        actor_id: Some(request.context.actor_id.clone()),
        capability_id: request.context.capability_id.clone(),
        capability_version: request.context.capability_version.clone(),
        resource_type: suggestion_record_type()?,
        normalized_filter_hash: normalized_filter_hash([
            ("party_id", party_id.as_str().as_bytes()),
            ("provider_profile_version_id", profile_id.as_str().as_bytes()),
            ("status", status.as_slice()),
        ]),
        sort_id: RecordQuerySort::UpdatedAtDescending.id().to_owned(),
        page_size,
    })
}

pub(super) fn decode_after(
    adapter: &CustomerEnrichmentSuggestionQueryAdapter,
    token: &str,
    binding: &CursorBinding,
) -> Result<Option<RecordQueryContinuation>, SdkError> {
    if token.is_empty() {
        return Ok(None);
    }
    let value = adapter
        .cursor_codec
        .decode(token, binding)
        .map_err(cursor_error)?;
    let after = RecordQueryContinuation {
        sort_value: String::from_utf8(value.sort_key).map_err(|_| cursor_invalid())?,
        record_id: value.record_id,
    };
    after.validate().map_err(cursor_error)?;
    Ok(Some(after))
}

pub(super) fn encode_next(
    adapter: &CustomerEnrichmentSuggestionQueryAdapter,
    binding: &CursorBinding,
    next: Option<&RecordQueryContinuation>,
) -> Result<String, SdkError> {
    next.map(|value| {
        adapter
            .cursor_codec
            .encode(
                binding,
                &CursorContinuation {
                    sort_key: value.sort_value.as_bytes().to_vec(),
                    record_id: value.record_id.clone(),
                },
            )
            .map_err(cursor_error)
    })
    .transpose()
    .map(|value| value.unwrap_or_default())
}

fn cursor_invalid() -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_SUGGESTION_LIST_CURSOR_INVALID",
        ErrorCategory::InvalidArgument,
        false,
        "The suggestion list cursor is invalid.",
    )
}

fn cursor_error(error: impl std::fmt::Display) -> SdkError {
    cursor_invalid().with_internal_reference(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_and_status_bounds_are_strict() {
        assert_eq!(page_size(0).unwrap(), DEFAULT_PAGE_SIZE);
        assert_eq!(page_size(100).unwrap(), 100);
        assert!(page_size(-1).is_err());
        assert!(page_size(101).is_err());
        assert!(status(wire::SuggestionLifecycleStatus::Accepted as i32).is_ok());
        assert!(status(wire::SuggestionLifecycleStatus::Unspecified as i32).is_err());
    }
}
