use crate::contract::{
    DEFAULT_PAGE_SIZE, MAXIMUM_CURSOR_BYTES, MAXIMUM_PAGE_SIZE,
    contact_points_privacy_scope_definition,
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

#[derive(Debug, Clone)]
pub(crate) struct ValidatedRequest {
    pub lineage: privacy::PrivacyScopeContributionLineage,
    pub canonical_party_id: RecordId,
    pub identity_resolution_generation: u64,
    pub page_size: u32,
    pub page_number: u32,
    pub after_record_id: Option<RecordId>,
}

pub(crate) fn validate_request_contract(request: &QueryRequest) -> Result<(), SdkError> {
    let definition = contact_points_privacy_scope_definition()?;
    validate_query_request_contract(request, &definition).map_err(map_request_contract_error)
}

pub(crate) fn validate_wire_request(
    context: &QueryExecutionContext,
    bytes: &[u8],
) -> Result<ValidatedRequest, SdkError> {
    let request =
        privacy::ContactPointsPrivacyScopeContributionRequest::decode(bytes).map_err(|error| {
            invalid_contract_with_reference(
                "CONTACT_POINTS_PRIVACY_SCOPE_REQUEST_INVALID",
                "The Contact Points privacy scope request is invalid.",
                error.to_string(),
            )
        })?;
    let contribution = request.contribution.ok_or_else(|| {
        invalid_contract(
            "CONTACT_POINTS_PRIVACY_SCOPE_REQUEST_INVALID",
            "The Contact Points privacy scope request is invalid.",
        )
    })?;
    let lineage = contribution.lineage.ok_or_else(|| {
        invalid_contract(
            "CONTACT_POINTS_PRIVACY_SCOPE_LINEAGE_INVALID",
            "The Contact Points privacy scope lineage is invalid.",
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

    let (page_number, after_record_id) = if contribution.cursor.is_empty() {
        (1, None)
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
        after_record_id,
    })
}

pub(crate) fn encode_cursor(
    request: &ValidatedRequest,
    next_page_number: u32,
    after_record_id: &RecordId,
) -> Result<String, SdkError> {
    if next_page_number < 2 {
        return Err(cursor_invalid());
    }
    let page = next_page_number.to_string();
    let after = encode_hex(after_record_id.as_str().as_bytes());
    let digest = cursor_digest(
        &request.lineage,
        &request.canonical_party_id,
        request.page_size,
        next_page_number,
        after_record_id,
    );
    Ok(format!(
        "{CURSOR_VERSION}.{page}.{after}.{}",
        encode_hex(&digest)
    ))
}

fn decode_cursor(
    lineage: &privacy::PrivacyScopeContributionLineage,
    canonical_party_id: &RecordId,
    page_size: u32,
    cursor: &str,
) -> Result<(u32, Option<RecordId>), SdkError> {
    if cursor.len() > MAXIMUM_CURSOR_BYTES {
        return Err(cursor_invalid());
    }
    let parts = cursor.split('.').collect::<Vec<_>>();
    if parts.len() != 4 || parts[0] != CURSOR_VERSION {
        return Err(cursor_invalid());
    }
    let page_number = parts[1].parse::<u32>().map_err(|_| cursor_invalid())?;
    if page_number < 2 {
        return Err(cursor_invalid());
    }
    let after_bytes = decode_hex(parts[2])?;
    let after_value = String::from_utf8(after_bytes).map_err(|_| cursor_invalid())?;
    let after_record_id = RecordId::try_new(after_value).map_err(|_| cursor_invalid())?;
    let actual_digest = decode_hex(parts[3])?;
    let expected_digest = cursor_digest(
        lineage,
        canonical_party_id,
        page_size,
        page_number,
        &after_record_id,
    );
    if actual_digest.as_slice() != expected_digest.as_slice() {
        return Err(cursor_invalid());
    }
    Ok((page_number, Some(after_record_id)))
}

fn cursor_digest(
    lineage: &privacy::PrivacyScopeContributionLineage,
    canonical_party_id: &RecordId,
    page_size: u32,
    page_number: u32,
    after_record_id: &RecordId,
) -> [u8; 32] {
    framed_digest(
        b"crm.contact-points.privacy.scope.request-cursor/v1",
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
            after_record_id.as_str().as_bytes(),
        ],
    )
}

fn cursor_invalid() -> SdkError {
    invalid_contract(
        "CONTACT_POINTS_PRIVACY_SCOPE_CURSOR_INVALID",
        "The Contact Points privacy scope cursor is invalid.",
    )
}
