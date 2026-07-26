use crate::contract::{DEFAULT_PAGE_SIZE, MAXIMUM_PAGE_SIZE, parties_privacy_scope_definition};
use crate::errors::{invalid_contract, invalid_contract_with_reference};
use crm_customer_privacy_owner_scope_support::{
    CommonLineageError, QueryRequestContractError, validate_common_lineage,
    validate_query_request_contract,
};
use crm_module_sdk::{ErrorCategory, RecordId, SdkError};
use crm_proto_contracts::crm::customer_privacy::v1 as privacy;
use crm_query_runtime::{QueryExecutionContext, QueryRequest};
use prost::Message;

#[derive(Debug, Clone)]
pub(crate) struct ValidatedRequest {
    pub lineage: privacy::PrivacyScopeContributionLineage,
    pub canonical_party_id: RecordId,
    pub identity_resolution_generation: u64,
    pub page_size: u32,
}

pub(crate) fn validate_request_contract(request: &QueryRequest) -> Result<(), SdkError> {
    let definition = parties_privacy_scope_definition()?;
    validate_query_request_contract(request, &definition).map_err(map_request_contract_error)
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

    let common = validate_common_lineage(
        context,
        lineage,
        contribution.page_size,
        DEFAULT_PAGE_SIZE,
        MAXIMUM_PAGE_SIZE,
    )
    .map_err(map_common_lineage_error)?;

    if !contribution.cursor.is_empty() {
        return Err(invalid_contract(
            "PARTIES_PRIVACY_SCOPE_CURSOR_INVALID",
            "The Parties privacy scope cursor is invalid.",
        ));
    }

    Ok(ValidatedRequest {
        lineage: common.lineage,
        canonical_party_id: common.canonical_party_id,
        identity_resolution_generation: common.identity_resolution_generation,
        page_size: common.page_size,
    })
}

fn map_request_contract_error(error: QueryRequestContractError) -> SdkError {
    match error {
        QueryRequestContractError::InvalidContext(error)
        | QueryRequestContractError::InvalidInput(error) => error,
        QueryRequestContractError::BindingMismatch => invalid_contract(
            "PARTIES_PRIVACY_SCOPE_REQUEST_BINDING_MISMATCH",
            "The Parties privacy scope request binding is invalid.",
        ),
        QueryRequestContractError::InputContractMismatch => invalid_contract(
            "PARTIES_PRIVACY_SCOPE_INPUT_CONTRACT_MISMATCH",
            "The Parties privacy scope request contract is invalid.",
        ),
        QueryRequestContractError::InputHashMismatch => invalid_contract(
            "PARTIES_PRIVACY_SCOPE_INPUT_HASH_MISMATCH",
            "The Parties privacy scope request integrity check failed.",
        ),
    }
}

fn map_common_lineage_error(error: CommonLineageError) -> SdkError {
    match error {
        CommonLineageError::TenantMismatch => invalid_contract(
            "PARTIES_PRIVACY_SCOPE_TENANT_MISMATCH",
            "The Parties privacy scope lineage is invalid.",
        ),
        CommonLineageError::CaseIdInvalid(reference) => invalid_contract_with_reference(
            "PARTIES_PRIVACY_SCOPE_CASE_ID_INVALID",
            "The Parties privacy scope lineage is invalid.",
            reference,
        ),
        CommonLineageError::PartyMissing => invalid_contract(
            "PARTIES_PRIVACY_SCOPE_PARTY_INVALID",
            "The Parties privacy scope lineage is invalid.",
        ),
        CommonLineageError::PartyInvalid(reference) => invalid_contract_with_reference(
            "PARTIES_PRIVACY_SCOPE_PARTY_INVALID",
            "The Parties privacy scope lineage is invalid.",
            reference,
        ),
        CommonLineageError::GenerationInvalid => invalid_contract(
            "PARTIES_PRIVACY_SCOPE_GENERATION_INVALID",
            "The Parties privacy scope lineage is invalid.",
        ),
        CommonLineageError::RegistryInvalid => invalid_contract(
            "PARTIES_PRIVACY_SCOPE_REGISTRY_INVALID",
            "The Parties privacy scope registry identity is invalid.",
        ),
        CommonLineageError::RegistryUnavailable(reference) => SdkError::new(
            "PARTIES_PRIVACY_SCOPE_REGISTRY_UNAVAILABLE",
            ErrorCategory::Internal,
            false,
            "The Parties privacy scope registry is unavailable.",
        )
        .with_internal_reference(reference),
        CommonLineageError::RegistryMismatch => invalid_contract(
            "PARTIES_PRIVACY_SCOPE_REGISTRY_MISMATCH",
            "The Parties privacy scope registry identity is invalid.",
        ),
        CommonLineageError::PurposeInvalid => invalid_contract(
            "PARTIES_PRIVACY_SCOPE_PURPOSE_INVALID",
            "The Parties privacy scope purpose is invalid.",
        ),
        CommonLineageError::RequestTimeInvalid => invalid_contract(
            "PARTIES_PRIVACY_SCOPE_REQUEST_TIME_INVALID",
            "The Parties privacy scope request time is invalid.",
        ),
        CommonLineageError::PageSizeInvalid => invalid_contract(
            "PARTIES_PRIVACY_SCOPE_PAGE_SIZE_INVALID",
            "The Parties privacy scope page size is invalid.",
        ),
    }
}
