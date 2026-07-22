use crate::{
    CustomerPrivacyQueryAdapter, LIST_PRIVACY_CASES_REQUEST_SCHEMA,
    LIST_PRIVACY_CASES_RESPONSE_SCHEMA, PARTY_RECORD_TYPE, case_state_invalid, decode_input,
    module_id, privacy_case_record_type, privacy_case_to_wire, query_configuration_invalid,
    redact_privacy_case,
};
use crm_capability_plan_support as support;
use crm_core_data::{RecordListQuery, RecordQueryContinuation, RecordQuerySort};
use crm_customer_privacy::PrivacyCase;
use crm_customer_privacy_persistence_adapter::privacy_case_from_snapshot;
use crm_module_sdk::{DataClass, ErrorCategory, RecordId, SdkError, TypedPayload};
use crm_proto_contracts::crm::{customer::v1::PartyRef, customer_privacy::v1 as wire};
use crm_query_runtime::{
    CursorBinding, CursorContinuation, QueryRequest, normalized_filter_hash,
};

const DEFAULT_PAGE_SIZE: u32 = 50;
const MAXIMUM_PAGE_SIZE: u32 = 100;
const INTERNAL_SCAN_PAGE_SIZE: u32 = 100;
const MAXIMUM_VISIBILITY_SCAN_RECORDS: usize = 4_096;

#[derive(Debug)]
struct ListParameters {
    party_id: RecordId,
    kind: Option<i32>,
    status: Option<i32>,
    page_size: u32,
    binding: CursorBinding,
    after: Option<RecordQueryContinuation>,
}

pub(super) fn validate(
    adapter: &CustomerPrivacyQueryAdapter,
    request: &QueryRequest,
) -> Result<(), SdkError> {
    parameters(adapter, request).map(|_| ())
}

pub(super) async fn execute(
    adapter: &CustomerPrivacyQueryAdapter,
    request: &QueryRequest,
) -> Result<TypedPayload, SdkError> {
    let parameters = parameters(adapter, request)?;
    let party_reference = support::record_ref(
        PARTY_RECORD_TYPE,
        parameters.party_id.as_str(),
        "customer_privacy.case.list.canonical_party_ref.party_id",
    )?;
    let party_visibility = adapter
        .visibility
        .authorize_visibility(request, &party_reference)
        .await?;
    if !party_visibility.resource_visible {
        return response(Vec::new(), String::new());
    }

    let (privacy_cases, next) = collect(adapter, request, &parameters).await?;
    let next_cursor = encode_next(adapter, &parameters.binding, next.as_ref())?;
    response(privacy_cases, next_cursor)
}

async fn collect(
    adapter: &CustomerPrivacyQueryAdapter,
    request: &QueryRequest,
    parameters: &ListParameters,
) -> Result<(Vec<wire::PrivacyCase>, Option<RecordQueryContinuation>), SdkError> {
    let mut output = Vec::with_capacity(parameters.page_size as usize);
    let mut after = parameters.after.clone();
    let mut scanned = 0_usize;

    loop {
        let remaining = parameters.page_size as usize - output.len();
        if remaining == 0 {
            let anchor = after.clone();
            let more = has_more(
                adapter,
                request,
                parameters,
                anchor.clone(),
                &mut scanned,
            )
            .await?;
            return Ok((output, more.then_some(anchor).flatten()));
        }

        let page = adapter
            .store
            .list_records_for_query(&RecordListQuery {
                tenant_id: request.context.tenant_id.clone(),
                owner_module_id: module_id()?,
                record_type: privacy_case_record_type()?,
                page_size: u32::try_from(remaining).map_err(query_configuration_invalid)?,
                sort: RecordQuerySort::UpdatedAtDescending,
                after: after.clone(),
            })
            .await?;
        scanned = scanned.saturating_add(page.records.len());
        enforce_scan_limit(scanned)?;

        for snapshot in &page.records {
            let privacy_case = rehydrate_case(request, snapshot)?;
            if !matches_filters(&privacy_case, parameters)? {
                continue;
            }
            let visibility = adapter
                .visibility
                .authorize_visibility(request, &snapshot.reference)
                .await?;
            if !visibility.resource_visible {
                continue;
            }
            let mut public = privacy_case_to_wire(&privacy_case)?;
            redact_privacy_case(&mut public, |field| visibility.allows_field(field));
            output.push(public);
        }

        after = page.next;
        if after.is_none() {
            return Ok((output, None));
        }
    }
}

async fn has_more(
    adapter: &CustomerPrivacyQueryAdapter,
    request: &QueryRequest,
    parameters: &ListParameters,
    mut after: Option<RecordQueryContinuation>,
    scanned: &mut usize,
) -> Result<bool, SdkError> {
    while after.is_some() {
        let page = adapter
            .store
            .list_records_for_query(&RecordListQuery {
                tenant_id: request.context.tenant_id.clone(),
                owner_module_id: module_id()?,
                record_type: privacy_case_record_type()?,
                page_size: INTERNAL_SCAN_PAGE_SIZE,
                sort: RecordQuerySort::UpdatedAtDescending,
                after: after.clone(),
            })
            .await?;
        *scanned = scanned.saturating_add(page.records.len());
        enforce_scan_limit(*scanned)?;

        for snapshot in &page.records {
            let privacy_case = rehydrate_case(request, snapshot)?;
            if !matches_filters(&privacy_case, parameters)? {
                continue;
            }
            if adapter
                .visibility
                .authorize_visibility(request, &snapshot.reference)
                .await?
                .resource_visible
            {
                return Ok(true);
            }
        }
        after = page.next;
    }
    Ok(false)
}

fn rehydrate_case(
    request: &QueryRequest,
    snapshot: &crm_module_sdk::RecordSnapshot,
) -> Result<PrivacyCase, SdkError> {
    let privacy_case = privacy_case_from_snapshot(snapshot)
        .map_err(|error| case_state_invalid(error.to_string()))?;
    if privacy_case.case_id() != &snapshot.reference.record_id
        || privacy_case.tenant_id() != &request.context.tenant_id
    {
        return Err(case_state_invalid(
            "privacy case identity differs from the persisted query snapshot",
        ));
    }
    Ok(privacy_case)
}

fn matches_filters(
    privacy_case: &PrivacyCase,
    parameters: &ListParameters,
) -> Result<bool, SdkError> {
    let Some(binding) = privacy_case.subject_binding() else {
        return Ok(false);
    };
    if binding.canonical_party_id != parameters.party_id {
        return Ok(false);
    }
    let public = privacy_case_to_wire(privacy_case)?;
    Ok(parameters.kind.is_none_or(|kind| public.kind == kind)
        && parameters.status.is_none_or(|status| public.status == status))
}

fn parameters(
    adapter: &CustomerPrivacyQueryAdapter,
    request: &QueryRequest,
) -> Result<ListParameters, SdkError> {
    let command: wire::ListPrivacyCasesRequest =
        decode_input(request, LIST_PRIVACY_CASES_REQUEST_SCHEMA)?;
    let party_id = party_id(command.canonical_party_ref)?;
    let kind = kind(command.kind)?;
    let status = status(command.status)?;
    let page_size = page_size(command.page_size)?;
    let binding = binding(request, &party_id, kind, status, page_size)?;
    let after = decode_after(adapter, &command.cursor, &binding)?;
    Ok(ListParameters {
        party_id,
        kind,
        status,
        page_size,
        binding,
        after,
    })
}

fn party_id(value: Option<PartyRef>) -> Result<RecordId, SdkError> {
    let value = value.ok_or_else(|| {
        SdkError::invalid_argument(
            "customer_privacy.case.list.canonical_party_ref",
            "Canonical Party reference is required.",
        )
    })?;
    RecordId::try_new(value.party_id).map_err(|error| {
        SdkError::invalid_argument(
            "customer_privacy.case.list.canonical_party_ref.party_id",
            error.to_string(),
        )
    })
}

fn kind(value: Option<i32>) -> Result<Option<i32>, SdkError> {
    value
        .map(|value| match wire::PrivacyCaseKind::try_from(value) {
            Ok(wire::PrivacyCaseKind::Unspecified) | Err(_) => Err(SdkError::invalid_argument(
                "customer_privacy.case.list.kind",
                "Kind must be a supported non-unspecified privacy case kind.",
            )),
            Ok(_) => Ok(value),
        })
        .transpose()
}

fn status(value: Option<i32>) -> Result<Option<i32>, SdkError> {
    value
        .map(|value| match wire::PrivacyCaseStatus::try_from(value) {
            Ok(wire::PrivacyCaseStatus::Unspecified) | Err(_) => Err(SdkError::invalid_argument(
                "customer_privacy.case.list.status",
                "Status must be a supported non-unspecified privacy case status.",
            )),
            Ok(_) => Ok(value),
        })
        .transpose()
}

fn page_size(value: i32) -> Result<u32, SdkError> {
    if value < 0 {
        return Err(SdkError::invalid_argument(
            "customer_privacy.case.list.page_size",
            "Page size must not be negative.",
        ));
    }
    let value = u32::try_from(value).map_err(query_configuration_invalid)?;
    let value = if value == 0 { DEFAULT_PAGE_SIZE } else { value };
    if value > MAXIMUM_PAGE_SIZE {
        return Err(SdkError::invalid_argument(
            "customer_privacy.case.list.page_size",
            format!("Page size must not exceed {MAXIMUM_PAGE_SIZE}."),
        ));
    }
    Ok(value)
}

fn binding(
    request: &QueryRequest,
    party_id: &RecordId,
    kind: Option<i32>,
    status: Option<i32>,
    page_size: u32,
) -> Result<CursorBinding, SdkError> {
    let kind = kind
        .unwrap_or(wire::PrivacyCaseKind::Unspecified as i32)
        .to_be_bytes();
    let status = status
        .unwrap_or(wire::PrivacyCaseStatus::Unspecified as i32)
        .to_be_bytes();
    Ok(CursorBinding {
        tenant_id: request.context.tenant_id.clone(),
        actor_id: Some(request.context.actor_id.clone()),
        capability_id: request.context.capability_id.clone(),
        capability_version: request.context.capability_version.clone(),
        resource_type: privacy_case_record_type()?,
        normalized_filter_hash: normalized_filter_hash([
            ("canonical_party_id", party_id.as_str().as_bytes()),
            ("kind", kind.as_slice()),
            ("status", status.as_slice()),
        ]),
        sort_id: RecordQuerySort::UpdatedAtDescending.id().to_owned(),
        page_size,
    })
}

fn decode_after(
    adapter: &CustomerPrivacyQueryAdapter,
    token: &str,
    binding: &CursorBinding,
) -> Result<Option<RecordQueryContinuation>, SdkError> {
    if token.is_empty() {
        return Ok(None);
    }
    let value = adapter
        .cursor_codec()?
        .decode(token, binding)
        .map_err(cursor_error)?;
    let after = RecordQueryContinuation {
        sort_value: String::from_utf8(value.sort_key).map_err(|_| cursor_invalid())?,
        record_id: value.record_id,
    };
    after.validate().map_err(cursor_error)?;
    Ok(Some(after))
}

fn encode_next(
    adapter: &CustomerPrivacyQueryAdapter,
    binding: &CursorBinding,
    next: Option<&RecordQueryContinuation>,
) -> Result<String, SdkError> {
    next.map(|value| {
        adapter
            .cursor_codec()?
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

fn response(
    privacy_cases: Vec<wire::PrivacyCase>,
    next_cursor: String,
) -> Result<TypedPayload, SdkError> {
    support::protobuf_payload(
        crm_customer_privacy::MODULE_ID,
        LIST_PRIVACY_CASES_RESPONSE_SCHEMA,
        DataClass::Confidential,
        &wire::ListPrivacyCasesResponse {
            privacy_cases,
            next_cursor,
        },
    )
}

fn enforce_scan_limit(scanned: usize) -> Result<(), SdkError> {
    if scanned > MAXIMUM_VISIBILITY_SCAN_RECORDS {
        Err(SdkError::new(
            "CUSTOMER_PRIVACY_CASE_LIST_SCAN_LIMIT_EXCEEDED",
            ErrorCategory::Unavailable,
            true,
            "The privacy case list is temporarily unavailable.",
        ))
    } else {
        Ok(())
    }
}

fn cursor_invalid() -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_CASE_LIST_CURSOR_INVALID",
        ErrorCategory::InvalidArgument,
        false,
        "The privacy case list cursor is invalid.",
    )
}

fn cursor_error(error: impl std::fmt::Display) -> SdkError {
    cursor_invalid().with_internal_reference(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_bounds_and_filters_are_strict() {
        assert_eq!(page_size(0).unwrap(), DEFAULT_PAGE_SIZE);
        assert_eq!(page_size(MAXIMUM_PAGE_SIZE as i32).unwrap(), MAXIMUM_PAGE_SIZE);
        assert!(page_size(-1).is_err());
        assert!(page_size(MAXIMUM_PAGE_SIZE as i32 + 1).is_err());
        assert!(kind(Some(wire::PrivacyCaseKind::Erasure as i32)).is_ok());
        assert!(kind(Some(wire::PrivacyCaseKind::Unspecified as i32)).is_err());
        assert!(status(Some(wire::PrivacyCaseStatus::Cancelled as i32)).is_ok());
        assert!(status(Some(wire::PrivacyCaseStatus::Unspecified as i32)).is_err());
    }
}
