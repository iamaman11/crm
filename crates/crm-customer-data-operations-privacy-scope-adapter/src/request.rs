use crate::contract::{
    DEFAULT_PAGE_SIZE, MAXIMUM_CURSOR_BYTES, MAXIMUM_PAGE_SIZE,
    customer_data_privacy_scope_definition,
};
use crate::digest::{decode_hex, encode_hex};
use crate::errors::{
    invalid_contract, invalid_contract_with_reference, map_common_lineage_error,
    map_request_contract_error,
};
use crm_customer_privacy_owner_scope_support::{
    framed_digest, validate_common_lineage, validate_query_request_contract,
};
use crm_module_sdk::{RecordId, SdkError};
use crm_proto_contracts::crm::customer_privacy::v1 as privacy;
use crm_query_runtime::{QueryExecutionContext, QueryRequest};
use prost::Message;

const CURSOR_VERSION: &str = "v1";
const ORIGIN: &str = "origin";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum ResourceFamily {
    ImportRow,
    ExportSelectionItem,
    ExportExecutionStage,
    ExportExecutionOutcome,
}

impl ResourceFamily {
    pub(crate) fn token(self) -> &'static str {
        match self {
            Self::ImportRow => "import",
            Self::ExportSelectionItem => "selection",
            Self::ExportExecutionStage => "stage",
            Self::ExportExecutionOutcome => "outcome",
        }
    }

    fn parse(value: &str) -> Result<Self, SdkError> {
        match value {
            "import" => Ok(Self::ImportRow),
            "selection" => Ok(Self::ExportSelectionItem),
            "stage" => Ok(Self::ExportExecutionStage),
            "outcome" => Ok(Self::ExportExecutionOutcome),
            _ => Err(cursor_invalid()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CursorState {
    pub family: ResourceFamily,
    pub after_record_id: Option<RecordId>,
}

#[derive(Debug, Clone)]
pub(crate) struct ValidatedRequest {
    pub lineage: privacy::PrivacyScopeContributionLineage,
    pub canonical_party_id: RecordId,
    pub identity_resolution_generation: u64,
    pub page_size: u32,
    pub page_number: u32,
    pub cursor_state: CursorState,
}

pub(crate) fn validate_request_contract(request: &QueryRequest) -> Result<(), SdkError> {
    let definition = customer_data_privacy_scope_definition()?;
    validate_query_request_contract(request, &definition).map_err(map_request_contract_error)
}

pub(crate) fn validate_wire_request(
    context: &QueryExecutionContext,
    bytes: &[u8],
) -> Result<ValidatedRequest, SdkError> {
    let request =
        privacy::CustomerDataPrivacyScopeContributionRequest::decode(bytes).map_err(|error| {
            invalid_contract_with_reference(
                "CUSTOMER_DATA_PRIVACY_SCOPE_REQUEST_INVALID",
                "The Customer Data Operations privacy scope request is invalid.",
                error.to_string(),
            )
        })?;
    let contribution = request.contribution.ok_or_else(|| {
        invalid_contract(
            "CUSTOMER_DATA_PRIVACY_SCOPE_REQUEST_INVALID",
            "The Customer Data Operations privacy scope request is invalid.",
        )
    })?;
    let lineage = contribution.lineage.ok_or_else(|| {
        invalid_contract(
            "CUSTOMER_DATA_PRIVACY_SCOPE_LINEAGE_INVALID",
            "The Customer Data Operations privacy scope lineage is invalid.",
        )
    })?;

    let common = validate_common_lineage(
        context,
        lineage,
        contribution.page_size,
        DEFAULT_PAGE_SIZE,
        MAXIMUM_PAGE_SIZE,
    )
    .map_err(map_common_lineage_error)?;

    let (page_number, cursor_state) = if contribution.cursor.is_empty() {
        (
            1,
            CursorState {
                family: ResourceFamily::ImportRow,
                after_record_id: None,
            },
        )
    } else {
        decode_cursor(
            &common.lineage,
            &common.canonical_party_id,
            common.page_size,
            &contribution.cursor,
        )?
    };

    Ok(ValidatedRequest {
        lineage: common.lineage,
        canonical_party_id: common.canonical_party_id,
        identity_resolution_generation: common.identity_resolution_generation,
        page_size: common.page_size,
        page_number,
        cursor_state,
    })
}

pub(crate) fn encode_cursor(
    request: &ValidatedRequest,
    next_page_number: u32,
    state: &CursorState,
) -> Result<String, SdkError> {
    if next_page_number < 2 {
        return Err(cursor_invalid());
    }
    let page = next_page_number.to_string();
    let after = state
        .after_record_id
        .as_ref()
        .map(|value| encode_hex(value.as_str().as_bytes()))
        .unwrap_or_else(|| ORIGIN.to_owned());
    let digest = cursor_digest(
        &request.lineage,
        &request.canonical_party_id,
        request.page_size,
        next_page_number,
        state.family,
        state.after_record_id.as_ref(),
    );
    Ok(format!(
        "{CURSOR_VERSION}.{page}.{}.{after}.{}",
        state.family.token(),
        encode_hex(&digest)
    ))
}

fn decode_cursor(
    lineage: &privacy::PrivacyScopeContributionLineage,
    canonical_party_id: &RecordId,
    page_size: u32,
    cursor: &str,
) -> Result<(u32, CursorState), SdkError> {
    if cursor.len() > MAXIMUM_CURSOR_BYTES {
        return Err(cursor_invalid());
    }
    let parts = cursor.split('.').collect::<Vec<_>>();
    if parts.len() != 5 || parts[0] != CURSOR_VERSION {
        return Err(cursor_invalid());
    }
    let page_number = parts[1].parse::<u32>().map_err(|_| cursor_invalid())?;
    if page_number < 2 {
        return Err(cursor_invalid());
    }
    let family = ResourceFamily::parse(parts[2])?;
    let after_record_id = if parts[3] == ORIGIN {
        None
    } else {
        let bytes = decode_hex(parts[3])?;
        let value = String::from_utf8(bytes).map_err(|_| cursor_invalid())?;
        Some(RecordId::try_new(value).map_err(|_| cursor_invalid())?)
    };
    if family == ResourceFamily::ImportRow && after_record_id.is_none() {
        return Err(cursor_invalid());
    }
    let actual_digest = decode_hex(parts[4])?;
    let expected_digest = cursor_digest(
        lineage,
        canonical_party_id,
        page_size,
        page_number,
        family,
        after_record_id.as_ref(),
    );
    if actual_digest.as_slice() != expected_digest.as_slice() {
        return Err(cursor_invalid());
    }
    Ok((
        page_number,
        CursorState {
            family,
            after_record_id,
        },
    ))
}

fn cursor_digest(
    lineage: &privacy::PrivacyScopeContributionLineage,
    canonical_party_id: &RecordId,
    page_size: u32,
    page_number: u32,
    family: ResourceFamily,
    after_record_id: Option<&RecordId>,
) -> [u8; 32] {
    framed_digest(
        b"crm.customer-data-operations.privacy.scope.request-cursor/v1",
        &[
            lineage.tenant_id.as_bytes(),
            lineage.privacy_case_id.as_bytes(),
            canonical_party_id.as_str().as_bytes(),
            lineage
                .identity_resolution_generation
                .to_string()
                .as_bytes(),
            lineage.registry_digest_sha256.as_slice(),
            lineage.purpose_code.as_bytes(),
            lineage.effective_request_at_unix_ms.to_string().as_bytes(),
            page_size.to_string().as_bytes(),
            page_number.to_string().as_bytes(),
            family.token().as_bytes(),
            after_record_id
                .map(RecordId::as_str)
                .unwrap_or(ORIGIN)
                .as_bytes(),
        ],
    )
}

fn cursor_invalid() -> SdkError {
    invalid_contract(
        "CUSTOMER_DATA_PRIVACY_SCOPE_CURSOR_INVALID",
        "The Customer Data Operations privacy scope cursor is invalid.",
    )
}
