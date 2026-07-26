use crate::contract::{
    CAPABILITY_ID, CAPABILITY_VERSION, DEFAULT_PAGE_SIZE, MAXIMUM_CURSOR_BYTES, MAXIMUM_PAGE_SIZE,
    consents_privacy_scope_definition,
};
use crate::digest::{decode_hex, encode_hex, framed_digest};
use crate::errors::{invalid_contract, invalid_contract_with_reference};
use crm_consents::MODULE_ID;
use crm_customer_privacy::{CANONICAL_SCOPE_REGISTRY_VERSION, OwnerScopeRegistry};
use crm_module_sdk::{ErrorCategory, RecordId, SdkError};
use crm_proto_contracts::crm::customer_privacy::v1 as privacy;
use crm_query_runtime::{QueryExecutionContext, QueryRequest};
use prost::Message;
use sha2::{Digest, Sha256};

const MAXIMUM_PURPOSE_CODE_BYTES: usize = 96;
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
    request.context.validate()?;
    request.input.validate()?;
    if request.owner_module_id.as_str() != MODULE_ID
        || request.context.capability_id.as_str() != CAPABILITY_ID
        || request.context.capability_version.as_str() != CAPABILITY_VERSION
    {
        return Err(invalid_contract(
            "CONSENTS_PRIVACY_SCOPE_REQUEST_BINDING_MISMATCH",
            "The Consents privacy scope request binding is invalid.",
        ));
    }

    let definition = consents_privacy_scope_definition()?;
    if !definition.input_contract.matches(&request.input) {
        return Err(invalid_contract(
            "CONSENTS_PRIVACY_SCOPE_INPUT_CONTRACT_MISMATCH",
            "The Consents privacy scope request contract is invalid.",
        ));
    }

    let actual_hash: [u8; 32] = Sha256::digest(&request.input.bytes).into();
    if request.input_hash != actual_hash {
        return Err(invalid_contract(
            "CONSENTS_PRIVACY_SCOPE_INPUT_HASH_MISMATCH",
            "The Consents privacy scope request integrity check failed.",
        ));
    }
    Ok(())
}

pub(crate) fn validate_wire_request(
    context: &QueryExecutionContext,
    bytes: &[u8],
) -> Result<ValidatedRequest, SdkError> {
    let request =
        privacy::ConsentsPrivacyScopeContributionRequest::decode(bytes).map_err(|error| {
            invalid_contract_with_reference(
                "CONSENTS_PRIVACY_SCOPE_REQUEST_INVALID",
                "The Consents privacy scope request is invalid.",
                error.to_string(),
            )
        })?;
    let contribution = request.contribution.ok_or_else(|| {
        invalid_contract(
            "CONSENTS_PRIVACY_SCOPE_REQUEST_INVALID",
            "The Consents privacy scope request is invalid.",
        )
    })?;
    let lineage = contribution.lineage.ok_or_else(|| {
        invalid_contract(
            "CONSENTS_PRIVACY_SCOPE_LINEAGE_INVALID",
            "The Consents privacy scope lineage is invalid.",
        )
    })?;

    if lineage.tenant_id != context.tenant_id.as_str() {
        return Err(invalid_contract(
            "CONSENTS_PRIVACY_SCOPE_TENANT_MISMATCH",
            "The Consents privacy scope lineage is invalid.",
        ));
    }
    RecordId::try_new(lineage.privacy_case_id.clone()).map_err(|error| {
        invalid_contract_with_reference(
            "CONSENTS_PRIVACY_SCOPE_CASE_ID_INVALID",
            "The Consents privacy scope lineage is invalid.",
            error.to_string(),
        )
    })?;

    let canonical_party_id = lineage
        .canonical_party_ref
        .as_ref()
        .ok_or_else(|| {
            invalid_contract(
                "CONSENTS_PRIVACY_SCOPE_PARTY_INVALID",
                "The Consents privacy scope lineage is invalid.",
            )
        })
        .and_then(|reference| {
            RecordId::try_new(reference.party_id.clone()).map_err(|error| {
                invalid_contract_with_reference(
                    "CONSENTS_PRIVACY_SCOPE_PARTY_INVALID",
                    "The Consents privacy scope lineage is invalid.",
                    error.to_string(),
                )
            })
        })?;

    if lineage.identity_resolution_generation == 0 {
        return Err(invalid_contract(
            "CONSENTS_PRIVACY_SCOPE_GENERATION_INVALID",
            "The Consents privacy scope lineage is invalid.",
        ));
    }
    if lineage.registry_version != CANONICAL_SCOPE_REGISTRY_VERSION
        || lineage.registry_digest_sha256.len() != 32
        || lineage.registry_digest_sha256.iter().all(|byte| *byte == 0)
    {
        return Err(invalid_contract(
            "CONSENTS_PRIVACY_SCOPE_REGISTRY_INVALID",
            "The Consents privacy scope registry identity is invalid.",
        ));
    }

    let registry = OwnerScopeRegistry::canonical_v1().map_err(|error| {
        SdkError::new(
            "CONSENTS_PRIVACY_SCOPE_REGISTRY_UNAVAILABLE",
            ErrorCategory::Internal,
            false,
            "The Consents privacy scope registry is unavailable.",
        )
        .with_internal_reference(error.to_string())
    })?;
    if lineage.registry_digest_sha256.as_slice() != registry.digest() {
        return Err(invalid_contract(
            "CONSENTS_PRIVACY_SCOPE_REGISTRY_MISMATCH",
            "The Consents privacy scope registry identity is invalid.",
        ));
    }

    validate_purpose_code(&lineage.purpose_code)?;
    let request_started_at_unix_ms = context.request_started_at_unix_nanos / 1_000_000;
    if lineage.effective_request_at_unix_ms <= 0
        || lineage.effective_request_at_unix_ms > request_started_at_unix_ms
    {
        return Err(invalid_contract(
            "CONSENTS_PRIVACY_SCOPE_REQUEST_TIME_INVALID",
            "The Consents privacy scope request time is invalid.",
        ));
    }

    let page_size = if contribution.page_size == 0 {
        DEFAULT_PAGE_SIZE
    } else {
        contribution.page_size
    };
    if page_size > MAXIMUM_PAGE_SIZE {
        return Err(invalid_contract(
            "CONSENTS_PRIVACY_SCOPE_PAGE_SIZE_INVALID",
            "The Consents privacy scope page size is invalid.",
        ));
    }

    let (page_number, after_record_id) = if contribution.cursor.is_empty() {
        (1, None)
    } else {
        decode_cursor(
            &lineage,
            &canonical_party_id,
            page_size,
            &contribution.cursor,
        )?
    };
    let identity_resolution_generation = lineage.identity_resolution_generation;

    Ok(ValidatedRequest {
        lineage,
        canonical_party_id,
        identity_resolution_generation,
        page_size,
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
        b"crm.consents.privacy.scope.request-cursor/v1",
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

fn validate_purpose_code(value: &str) -> Result<(), SdkError> {
    if value.is_empty()
        || value.len() > MAXIMUM_PURPOSE_CODE_BYTES
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
    {
        return Err(invalid_contract(
            "CONSENTS_PRIVACY_SCOPE_PURPOSE_INVALID",
            "The Consents privacy scope purpose is invalid.",
        ));
    }
    Ok(())
}

fn cursor_invalid() -> SdkError {
    invalid_contract(
        "CONSENTS_PRIVACY_SCOPE_CURSOR_INVALID",
        "The Consents privacy scope cursor is invalid.",
    )
}
