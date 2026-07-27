use crm_customer_privacy_owner_scope_support::{
    CanonicalPartyClaimError, CommonLineageError, QueryRequestContractError,
};
use crm_module_sdk::{ErrorCategory, IdentifierError, SdkError};

pub(crate) fn configured<T>(value: Result<T, IdentifierError>) -> Result<T, SdkError> {
    value.map_err(|error| {
        SdkError::new(
            "CUSTOMER_DATA_PRIVACY_SCOPE_CONFIGURATION_INVALID",
            ErrorCategory::Internal,
            false,
            "The Customer Data Operations privacy scope configuration is invalid.",
        )
        .with_internal_reference(error.to_string())
    })
}

pub(crate) fn map_request_contract_error(error: QueryRequestContractError) -> SdkError {
    match error {
        QueryRequestContractError::InvalidContext(error)
        | QueryRequestContractError::InvalidInput(error) => error,
        QueryRequestContractError::BindingMismatch => invalid_contract(
            "CUSTOMER_DATA_PRIVACY_SCOPE_REQUEST_BINDING_MISMATCH",
            "The Customer Data Operations privacy scope request binding is invalid.",
        ),
        QueryRequestContractError::InputContractMismatch => invalid_contract(
            "CUSTOMER_DATA_PRIVACY_SCOPE_INPUT_CONTRACT_MISMATCH",
            "The Customer Data Operations privacy scope request contract is invalid.",
        ),
        QueryRequestContractError::InputHashMismatch => invalid_contract(
            "CUSTOMER_DATA_PRIVACY_SCOPE_INPUT_HASH_MISMATCH",
            "The Customer Data Operations privacy scope request integrity check failed.",
        ),
    }
}

pub(crate) fn map_common_lineage_error(error: CommonLineageError) -> SdkError {
    match error {
        CommonLineageError::TenantMismatch => invalid_contract(
            "CUSTOMER_DATA_PRIVACY_SCOPE_TENANT_MISMATCH",
            "The Customer Data Operations privacy scope lineage is invalid.",
        ),
        CommonLineageError::CaseIdInvalid(reference) => invalid_contract_with_reference(
            "CUSTOMER_DATA_PRIVACY_SCOPE_CASE_ID_INVALID",
            "The Customer Data Operations privacy scope lineage is invalid.",
            reference,
        ),
        CommonLineageError::PartyMissing => invalid_contract(
            "CUSTOMER_DATA_PRIVACY_SCOPE_PARTY_INVALID",
            "The Customer Data Operations privacy scope lineage is invalid.",
        ),
        CommonLineageError::PartyInvalid(reference) => invalid_contract_with_reference(
            "CUSTOMER_DATA_PRIVACY_SCOPE_PARTY_INVALID",
            "The Customer Data Operations privacy scope lineage is invalid.",
            reference,
        ),
        CommonLineageError::GenerationInvalid => invalid_contract(
            "CUSTOMER_DATA_PRIVACY_SCOPE_GENERATION_INVALID",
            "The Customer Data Operations privacy scope lineage is invalid.",
        ),
        CommonLineageError::RegistryInvalid => invalid_contract(
            "CUSTOMER_DATA_PRIVACY_SCOPE_REGISTRY_INVALID",
            "The Customer Data Operations privacy scope registry identity is invalid.",
        ),
        CommonLineageError::RegistryUnavailable(reference) => SdkError::new(
            "CUSTOMER_DATA_PRIVACY_SCOPE_REGISTRY_UNAVAILABLE",
            ErrorCategory::Internal,
            false,
            "The Customer Data Operations privacy scope registry is unavailable.",
        )
        .with_internal_reference(reference),
        CommonLineageError::RegistryMismatch => invalid_contract(
            "CUSTOMER_DATA_PRIVACY_SCOPE_REGISTRY_MISMATCH",
            "The Customer Data Operations privacy scope registry identity is invalid.",
        ),
        CommonLineageError::PurposeInvalid => invalid_contract(
            "CUSTOMER_DATA_PRIVACY_SCOPE_PURPOSE_INVALID",
            "The Customer Data Operations privacy scope purpose is invalid.",
        ),
        CommonLineageError::RequestTimeInvalid => invalid_contract(
            "CUSTOMER_DATA_PRIVACY_SCOPE_REQUEST_TIME_INVALID",
            "The Customer Data Operations privacy scope request time is invalid.",
        ),
        CommonLineageError::PageSizeInvalid => invalid_contract(
            "CUSTOMER_DATA_PRIVACY_SCOPE_PAGE_SIZE_INVALID",
            "The Customer Data Operations privacy scope page size is invalid.",
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

pub(crate) fn lineage_invalid(
    category: ErrorCategory,
    retryable: bool,
    reference: impl Into<String>,
) -> SdkError {
    SdkError::new(
        "CUSTOMER_DATA_PRIVACY_SCOPE_LINEAGE_INVALID",
        category,
        retryable,
        "The requested Customer Data Operations privacy scope is not available.",
    )
    .with_internal_reference(reference.into())
}

pub(crate) fn import_row_state_invalid(reference: impl Into<String>) -> SdkError {
    stored_state_invalid(
        "CUSTOMER_DATA_PRIVACY_SCOPE_IMPORT_ROW_STATE_INVALID",
        "The Customer Data Operations import-row privacy scope is temporarily unavailable.",
        reference,
    )
}

pub(crate) fn export_selection_state_invalid(reference: impl Into<String>) -> SdkError {
    stored_state_invalid(
        "CUSTOMER_DATA_PRIVACY_SCOPE_EXPORT_SELECTION_STATE_INVALID",
        "The Customer Data Operations export-selection privacy scope is temporarily unavailable.",
        reference,
    )
}

pub(crate) fn export_stage_state_invalid(reference: impl Into<String>) -> SdkError {
    stored_state_invalid(
        "CUSTOMER_DATA_PRIVACY_SCOPE_EXPORT_STAGE_STATE_INVALID",
        "The Customer Data Operations export-stage privacy scope is temporarily unavailable.",
        reference,
    )
}

pub(crate) fn export_outcome_state_invalid(reference: impl Into<String>) -> SdkError {
    stored_state_invalid(
        "CUSTOMER_DATA_PRIVACY_SCOPE_EXPORT_OUTCOME_STATE_INVALID",
        "The Customer Data Operations export-outcome privacy scope is temporarily unavailable.",
        reference,
    )
}

pub(crate) fn association_state_invalid(reference: impl Into<String>) -> SdkError {
    stored_state_invalid(
        "CUSTOMER_DATA_PRIVACY_SCOPE_ASSOCIATION_STATE_INVALID",
        "The Customer Data Operations privacy scope is temporarily unavailable.",
        reference,
    )
}

pub(crate) fn scan_limit_exceeded(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_DATA_PRIVACY_SCOPE_SCAN_LIMIT_EXCEEDED",
        ErrorCategory::Unavailable,
        true,
        "The Customer Data Operations privacy scope is temporarily unavailable.",
    )
    .with_internal_reference(reference.into())
}

pub(crate) fn canonical_resolution_unavailable(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_DATA_PRIVACY_SCOPE_CANONICAL_RESOLUTION_UNAVAILABLE",
        ErrorCategory::Unavailable,
        true,
        "The Customer Data Operations privacy scope is temporarily unavailable.",
    )
    .with_internal_reference(reference.into())
}

pub(crate) fn database_unavailable(error: sqlx::Error) -> SdkError {
    SdkError::new(
        "CUSTOMER_DATA_PRIVACY_SCOPE_UNAVAILABLE",
        ErrorCategory::Unavailable,
        true,
        "The Customer Data Operations privacy scope is temporarily unavailable.",
    )
    .with_internal_reference(error.to_string())
}

fn stored_state_invalid(
    code: &'static str,
    safe_message: &'static str,
    reference: impl Into<String>,
) -> SdkError {
    SdkError::new(code, ErrorCategory::Unavailable, true, safe_message)
        .with_internal_reference(reference.into())
}
