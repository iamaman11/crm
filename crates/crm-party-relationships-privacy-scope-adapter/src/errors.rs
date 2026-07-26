use crm_customer_privacy_owner_scope_support::{
    CanonicalPartyClaimError, CommonLineageError, QueryRequestContractError,
};
use crm_module_sdk::{ErrorCategory, IdentifierError, SdkError};

pub(crate) fn configured<T>(value: Result<T, IdentifierError>) -> Result<T, SdkError> {
    value.map_err(|error| {
        SdkError::new(
            "PARTY_RELATIONSHIPS_PRIVACY_SCOPE_CONFIGURATION_INVALID",
            ErrorCategory::Internal,
            false,
            "The Party Relationships privacy scope configuration is invalid.",
        )
        .with_internal_reference(error.to_string())
    })
}

pub(crate) fn map_request_contract_error(error: QueryRequestContractError) -> SdkError {
    match error {
        QueryRequestContractError::InvalidContext(error)
        | QueryRequestContractError::InvalidInput(error) => error,
        QueryRequestContractError::BindingMismatch => invalid_contract(
            "PARTY_RELATIONSHIPS_PRIVACY_SCOPE_REQUEST_BINDING_MISMATCH",
            "The Party Relationships privacy scope request binding is invalid.",
        ),
        QueryRequestContractError::InputContractMismatch => invalid_contract(
            "PARTY_RELATIONSHIPS_PRIVACY_SCOPE_INPUT_CONTRACT_MISMATCH",
            "The Party Relationships privacy scope request contract is invalid.",
        ),
        QueryRequestContractError::InputHashMismatch => invalid_contract(
            "PARTY_RELATIONSHIPS_PRIVACY_SCOPE_INPUT_HASH_MISMATCH",
            "The Party Relationships privacy scope request integrity check failed.",
        ),
    }
}

pub(crate) fn map_common_lineage_error(error: CommonLineageError) -> SdkError {
    match error {
        CommonLineageError::TenantMismatch => invalid_contract(
            "PARTY_RELATIONSHIPS_PRIVACY_SCOPE_TENANT_MISMATCH",
            "The Party Relationships privacy scope lineage is invalid.",
        ),
        CommonLineageError::CaseIdInvalid(reference) => invalid_contract_with_reference(
            "PARTY_RELATIONSHIPS_PRIVACY_SCOPE_CASE_ID_INVALID",
            "The Party Relationships privacy scope lineage is invalid.",
            reference,
        ),
        CommonLineageError::PartyMissing => invalid_contract(
            "PARTY_RELATIONSHIPS_PRIVACY_SCOPE_PARTY_INVALID",
            "The Party Relationships privacy scope lineage is invalid.",
        ),
        CommonLineageError::PartyInvalid(reference) => invalid_contract_with_reference(
            "PARTY_RELATIONSHIPS_PRIVACY_SCOPE_PARTY_INVALID",
            "The Party Relationships privacy scope lineage is invalid.",
            reference,
        ),
        CommonLineageError::GenerationInvalid => invalid_contract(
            "PARTY_RELATIONSHIPS_PRIVACY_SCOPE_GENERATION_INVALID",
            "The Party Relationships privacy scope lineage is invalid.",
        ),
        CommonLineageError::RegistryInvalid => invalid_contract(
            "PARTY_RELATIONSHIPS_PRIVACY_SCOPE_REGISTRY_INVALID",
            "The Party Relationships privacy scope registry identity is invalid.",
        ),
        CommonLineageError::RegistryUnavailable(reference) => SdkError::new(
            "PARTY_RELATIONSHIPS_PRIVACY_SCOPE_REGISTRY_UNAVAILABLE",
            ErrorCategory::Internal,
            false,
            "The Party Relationships privacy scope registry is unavailable.",
        )
        .with_internal_reference(reference),
        CommonLineageError::RegistryMismatch => invalid_contract(
            "PARTY_RELATIONSHIPS_PRIVACY_SCOPE_REGISTRY_MISMATCH",
            "The Party Relationships privacy scope registry identity is invalid.",
        ),
        CommonLineageError::PurposeInvalid => invalid_contract(
            "PARTY_RELATIONSHIPS_PRIVACY_SCOPE_PURPOSE_INVALID",
            "The Party Relationships privacy scope purpose is invalid.",
        ),
        CommonLineageError::RequestTimeInvalid => invalid_contract(
            "PARTY_RELATIONSHIPS_PRIVACY_SCOPE_REQUEST_TIME_INVALID",
            "The Party Relationships privacy scope request time is invalid.",
        ),
        CommonLineageError::PageSizeInvalid => invalid_contract(
            "PARTY_RELATIONSHIPS_PRIVACY_SCOPE_PAGE_SIZE_INVALID",
            "The Party Relationships privacy scope page size is invalid.",
        ),
    }
}

pub(crate) fn map_canonical_party_claim_error(error: CanonicalPartyClaimError) -> SdkError {
    match error {
        CanonicalPartyClaimError::Database(error) => database_unavailable(error),
        CanonicalPartyClaimError::GenerationNotPositive => lineage_invalid(
            ErrorCategory::Conflict,
            true,
            "authoritative Identity Resolution generation is not positive",
        ),
        CanonicalPartyClaimError::StaleGeneration => lineage_invalid(
            ErrorCategory::Conflict,
            true,
            "claimed Identity Resolution generation is stale",
        ),
        CanonicalPartyClaimError::PartyNotVisible => lineage_invalid(
            ErrorCategory::NotFound,
            false,
            "claimed canonical Party is not visible in the tenant snapshot",
        ),
        CanonicalPartyClaimError::ActiveRedirect => lineage_invalid(
            ErrorCategory::Conflict,
            false,
            "claimed Party has an active canonical redirect",
        ),
    }
}

pub(crate) fn lineage_invalid(
    category: ErrorCategory,
    retryable: bool,
    reference: impl Into<String>,
) -> SdkError {
    SdkError::new(
        "PARTY_RELATIONSHIPS_PRIVACY_SCOPE_LINEAGE_INVALID",
        category,
        retryable,
        "The requested Party Relationships privacy scope is not available.",
    )
    .with_internal_reference(reference.into())
}

pub(crate) fn stored_state_invalid(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "PARTY_RELATIONSHIPS_PRIVACY_SCOPE_STORED_STATE_INVALID",
        ErrorCategory::Unavailable,
        true,
        "The Party Relationships privacy scope is temporarily unavailable.",
    )
    .with_internal_reference(reference.into())
}

pub(crate) fn row_decode_error(error: sqlx::Error) -> SdkError {
    stored_state_invalid(error.to_string())
}

pub(crate) fn database_unavailable(error: sqlx::Error) -> SdkError {
    SdkError::new(
        "PARTY_RELATIONSHIPS_PRIVACY_SCOPE_UNAVAILABLE",
        ErrorCategory::Unavailable,
        true,
        "The Party Relationships privacy scope is temporarily unavailable.",
    )
    .with_internal_reference(error.to_string())
}

pub(crate) fn invalid_contract(code: &'static str, safe_message: &'static str) -> SdkError {
    SdkError::new(code, ErrorCategory::InvalidArgument, false, safe_message)
}

pub(crate) fn invalid_contract_with_reference(
    code: &'static str,
    safe_message: &'static str,
    reference: impl Into<String>,
) -> SdkError {
    invalid_contract(code, safe_message).with_internal_reference(reference.into())
}
