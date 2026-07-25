use crate::contract::{
    CAPABILITY_ID, CAPABILITY_VERSION, DEFAULT_PAGE_SIZE, MAXIMUM_PAGE_SIZE,
    parties_privacy_scope_definition,
};
use crate::errors::{invalid_contract, invalid_contract_with_reference};
use crm_customer_privacy::{CANONICAL_SCOPE_REGISTRY_VERSION, OwnerScopeRegistry};
use crm_identity_resolution::PartyReference;
use crm_module_sdk::{ErrorCategory, RecordId, SdkError};
use crm_parties::MODULE_ID;
use crm_proto_contracts::crm::customer_privacy::v1 as privacy;
use crm_query_runtime::{QueryExecutionContext, QueryRequest};
use prost::Message;
use sha2::{Digest, Sha256};

const MAXIMUM_PURPOSE_CODE_BYTES: usize = 96;

#[derive(Debug, Clone)]
pub(crate) struct ValidatedRequest {
    pub lineage: privacy::PrivacyScopeContributionLineage,
    pub canonical_party_id: RecordId,
    pub canonical_party: PartyReference,
    pub identity_resolution_generation: u64,
    pub page_size: u32,
}

pub(crate) fn validate_request_contract(request: &QueryRequest) -> Result<(), SdkError> {
    request.context.validate()?;
    request.input.validate()?;
    if request.owner_module_id.as_str() != MODULE_ID
        || request.context.capability_id.as_str() != CAPABILITY_ID
        || request.context.capability_version.as_str() != CAPABILITY_VERSION
    {
        return Err(invalid_contract(
            "PARTIES_PRIVACY_SCOPE_REQUEST_BINDING_MISMATCH",
            "The Parties privacy scope request binding is invalid.",
        ));
    }

    let definition = parties_privacy_scope_definition()?;
    if !definition.input_contract.matches(&request.input) {
        return Err(invalid_contract(
            "PARTIES_PRIVACY_SCOPE_INPUT_CONTRACT_MISMATCH",
            "The Parties privacy scope request contract is invalid.",
        ));
    }

    let actual_hash: [u8; 32] = Sha256::digest(&request.input.bytes).into();
    if request.input_hash != actual_hash {
        return Err(invalid_contract(
            "PARTIES_PRIVACY_SCOPE_INPUT_HASH_MISMATCH",
            "The Parties privacy scope request integrity check failed.",
        ));
    }
    Ok(())
}

pub(crate) fn validate_wire_request(
    context: &QueryExecutionContext,
    bytes: &[u8],
) -> Result<ValidatedRequest, SdkError> {
    let request =
        privacy::PartiesPrivacyScopeContributionRequest::decode(bytes).map_err(|error| {
            invalid_contract_with_reference(
                "PARTIES_PRIVACY_SCOPE_REQUEST_INVALID",
                "The Parties privacy scope request is invalid.",
                error.to_string(),
            )
        })?;
    let contribution = request.contribution.ok_or_else(|| {
        invalid_contract(
            "PARTIES_PRIVACY_SCOPE_REQUEST_INVALID",
            "The Parties privacy scope request is invalid.",
        )
    })?;
    let lineage = contribution.lineage.ok_or_else(|| {
        invalid_contract(
            "PARTIES_PRIVACY_SCOPE_LINEAGE_INVALID",
            "The Parties privacy scope lineage is invalid.",
        )
    })?;

    if lineage.tenant_id != context.tenant_id.as_str() {
        return Err(invalid_contract(
            "PARTIES_PRIVACY_SCOPE_TENANT_MISMATCH",
            "The Parties privacy scope lineage is invalid.",
        ));
    }
    RecordId::try_new(lineage.privacy_case_id.clone()).map_err(|error| {
        invalid_contract_with_reference(
            "PARTIES_PRIVACY_SCOPE_CASE_ID_INVALID",
            "The Parties privacy scope lineage is invalid.",
            error.to_string(),
        )
    })?;

    let canonical_party_id = lineage
        .canonical_party_ref
        .as_ref()
        .ok_or_else(|| {
            invalid_contract(
                "PARTIES_PRIVACY_SCOPE_PARTY_INVALID",
                "The Parties privacy scope lineage is invalid.",
            )
        })
        .and_then(|reference| {
            RecordId::try_new(reference.party_id.clone()).map_err(|error| {
                invalid_contract_with_reference(
                    "PARTIES_PRIVACY_SCOPE_PARTY_INVALID",
                    "The Parties privacy scope lineage is invalid.",
                    error.to_string(),
                )
            })
        })?;
    let canonical_party =
        PartyReference::try_new(canonical_party_id.as_str()).map_err(|error| {
            invalid_contract_with_reference(
                "PARTIES_PRIVACY_SCOPE_PARTY_INVALID",
                "The Parties privacy scope lineage is invalid.",
                error.to_string(),
            )
        })?;

    if lineage.identity_resolution_generation == 0 {
        return Err(invalid_contract(
            "PARTIES_PRIVACY_SCOPE_GENERATION_INVALID",
            "The Parties privacy scope lineage is invalid.",
        ));
    }
    if lineage.registry_version != CANONICAL_SCOPE_REGISTRY_VERSION
        || lineage.registry_digest_sha256.len() != 32
        || lineage.registry_digest_sha256.iter().all(|byte| *byte == 0)
    {
        return Err(invalid_contract(
            "PARTIES_PRIVACY_SCOPE_REGISTRY_INVALID",
            "The Parties privacy scope registry identity is invalid.",
        ));
    }

    let registry = OwnerScopeRegistry::canonical_v1().map_err(|error| {
        SdkError::new(
            "PARTIES_PRIVACY_SCOPE_REGISTRY_UNAVAILABLE",
            ErrorCategory::Internal,
            false,
            "The Parties privacy scope registry is unavailable.",
        )
        .with_internal_reference(error.to_string())
    })?;
    if lineage.registry_digest_sha256.as_slice() != registry.digest() {
        return Err(invalid_contract(
            "PARTIES_PRIVACY_SCOPE_REGISTRY_MISMATCH",
            "The Parties privacy scope registry identity is invalid.",
        ));
    }

    validate_purpose_code(&lineage.purpose_code)?;
    let request_started_at_unix_ms = context.request_started_at_unix_nanos / 1_000_000;
    if lineage.effective_request_at_unix_ms <= 0
        || lineage.effective_request_at_unix_ms > request_started_at_unix_ms
    {
        return Err(invalid_contract(
            "PARTIES_PRIVACY_SCOPE_REQUEST_TIME_INVALID",
            "The Parties privacy scope request time is invalid.",
        ));
    }

    let page_size = if contribution.page_size == 0 {
        DEFAULT_PAGE_SIZE
    } else {
        contribution.page_size
    };
    if page_size > MAXIMUM_PAGE_SIZE {
        return Err(invalid_contract(
            "PARTIES_PRIVACY_SCOPE_PAGE_SIZE_INVALID",
            "The Parties privacy scope page size is invalid.",
        ));
    }
    if !contribution.cursor.is_empty() {
        return Err(invalid_contract(
            "PARTIES_PRIVACY_SCOPE_CURSOR_INVALID",
            "The Parties privacy scope cursor is invalid.",
        ));
    }

    let identity_resolution_generation = lineage.identity_resolution_generation;
    Ok(ValidatedRequest {
        lineage,
        canonical_party_id,
        canonical_party,
        identity_resolution_generation,
        page_size,
    })
}

fn validate_purpose_code(value: &str) -> Result<(), SdkError> {
    if value.is_empty()
        || value.len() > MAXIMUM_PURPOSE_CODE_BYTES
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
    {
        return Err(invalid_contract(
            "PARTIES_PRIVACY_SCOPE_PURPOSE_INVALID",
            "The Parties privacy scope purpose is invalid.",
        ));
    }
    Ok(())
}
