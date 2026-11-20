use crm_capability_runtime::CapabilityDefinition;
use crm_customer_privacy::{CANONICAL_SCOPE_REGISTRY_VERSION, OwnerScopeRegistry};
use crm_module_sdk::{RecordId, SdkError};
use crm_proto_contracts::crm::customer_privacy::v1 as privacy;
use crm_query_runtime::{QueryExecutionContext, QueryRequest};
use sha2::{Digest, Sha256};

const MAXIMUM_PURPOSE_CODE_BYTES: usize = 96;

#[derive(Debug)]
pub enum QueryRequestContractError {
    InvalidContext(SdkError),
    InvalidInput(SdkError),
    BindingMismatch,
    InputContractMismatch,
    InputHashMismatch,
}

#[derive(Debug, Clone)]
pub struct ValidatedCommonLineage {
    pub lineage: privacy::PrivacyScopeContributionLineage,
    pub canonical_party_id: RecordId,
    pub identity_resolution_generation: u64,
    pub page_size: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommonLineageError {
    TenantMismatch,
    CaseIdInvalid(String),
    PartyMissing,
    PartyInvalid(String),
    GenerationInvalid,
    RegistryInvalid,
    RegistryUnavailable(String),
    RegistryMismatch,
    PurposeInvalid,
    RequestTimeInvalid,
    PageSizeInvalid,
}

pub fn validate_query_request_contract(
    request: &QueryRequest,
    definition: &CapabilityDefinition,
) -> Result<(), QueryRequestContractError> {
    request
        .context
        .validate()
        .map_err(QueryRequestContractError::InvalidContext)?;
    request
        .input
        .validate()
        .map_err(QueryRequestContractError::InvalidInput)?;

    if request.owner_module_id != definition.owner_module_id
        || request.context.capability_id != definition.capability_id
        || request.context.capability_version != definition.capability_version
    {
        return Err(QueryRequestContractError::BindingMismatch);
    }
    if !definition.input_contract.matches(&request.input) {
        return Err(QueryRequestContractError::InputContractMismatch);
    }

    let actual_hash: [u8; 32] = Sha256::digest(&request.input.bytes).into();
    if request.input_hash != actual_hash {
        return Err(QueryRequestContractError::InputHashMismatch);
    }
    Ok(())
}

pub fn validate_common_lineage(
    context: &QueryExecutionContext,
    lineage: privacy::PrivacyScopeContributionLineage,
    requested_page_size: u32,
    default_page_size: u32,
    maximum_page_size: u32,
) -> Result<ValidatedCommonLineage, CommonLineageError> {
    if lineage.tenant_id != context.tenant_id.as_str() {
        return Err(CommonLineageError::TenantMismatch);
    }
    RecordId::try_new(lineage.privacy_case_id.clone())
        .map_err(|error| CommonLineageError::CaseIdInvalid(error.to_string()))?;

    let canonical_party_id = lineage
        .canonical_party_ref
        .as_ref()
        .ok_or(CommonLineageError::PartyMissing)
        .and_then(|reference| {
            RecordId::try_new(reference.party_id.clone())
                .map_err(|error| CommonLineageError::PartyInvalid(error.to_string()))
        })?;

    if lineage.identity_resolution_generation == 0 {
        return Err(CommonLineageError::GenerationInvalid);
    }
    if lineage.registry_version != CANONICAL_SCOPE_REGISTRY_VERSION
        || lineage.registry_digest_sha256.len() != 32
        || lineage.registry_digest_sha256.iter().all(|byte| *byte == 0)
    {
        return Err(CommonLineageError::RegistryInvalid);
    }

    let registry = OwnerScopeRegistry::canonical_v1()
        .map_err(|error| CommonLineageError::RegistryUnavailable(error.to_string()))?;
    if lineage.registry_digest_sha256.as_slice() != registry.digest() {
        return Err(CommonLineageError::RegistryMismatch);
    }

    if !valid_purpose_code(&lineage.purpose_code) {
        return Err(CommonLineageError::PurposeInvalid);
    }
    let request_started_at_unix_ms = context.request_started_at_unix_nanos / 1_000_000;
    if lineage.effective_request_at_unix_ms <= 0
        || lineage.effective_request_at_unix_ms > request_started_at_unix_ms
    {
        return Err(CommonLineageError::RequestTimeInvalid);
    }

    let page_size = if requested_page_size == 0 {
        default_page_size
    } else {
        requested_page_size
    };
    if page_size > maximum_page_size {
        return Err(CommonLineageError::PageSizeInvalid);
    }

    let identity_resolution_generation = lineage.identity_resolution_generation;
    Ok(ValidatedCommonLineage {
        lineage,
        canonical_party_id,
        identity_resolution_generation,
        page_size,
    })
}

fn valid_purpose_code(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAXIMUM_PURPOSE_CODE_BYTES
        && value
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
}
