use crm_customer_privacy_owner_scope_support::{
    CanonicalPartyClaimError, CommonLineageError, QueryRequestContractError,
};
use crm_module_sdk::{ErrorCategory, IdentifierError, SdkError};

pub(crate) fn configured<T>(value: Result<T, IdentifierError>) -> Result<T, SdkError> {
    value.map_err(|error| {
        SdkError::new(
            "DATA_QUALITY_PRIVACY_SCOPE_CONFIGURATION_INVALID",
            ErrorCategory::Internal,
            false,
            "The Data Quality privacy scope configuration is invalid.",
        )
        .with_internal_reference(error.to_string())
    })
}

pub(crate) fn map_request_contract_error(error: QueryRequestContractError) -> SdkError {
    match error {
        QueryRequestContractError::InvalidContext(error)
        | QueryRequestContractError::InvalidInput(error) => error,
        QueryRequestContractError::BindingMismatch => invalid_contract(
            "DATA_QUALITY_PRIVACY_SCOPE_REQUEST_BINDING_MISMATCH",
            "The Data Quality privacy scope request binding is invalid.",
        ),
        QueryRequestContractError::InputContractMismatch => invalid_contract(
            "DATA_QUALITY_PRIVACY_SCOPE_INPUT_CONTRACT_MISMATCH",
            "The Data Quality privacy scope request contract is invalid.",
        ),
        QueryRequestContractError::InputHashMismatch => invalid_contract(
            "DATA_QUALITY_PRIVACY_SCOPE_INPUT_HASH_MISMATCH",
            "The Data Quality privacy scope request integrity check failed.",
        ),
    }
}

pub(crate) fn map_common_lineage_error(error: CommonLineageError) -> SdkError {
    match error {
        CommonLineageError::TenantMismatch => lineage_contract_invalid("TENANT_MISMATCH"),
        CommonLineageError::CaseIdInvalid(reference) => invalid_contract_with_reference(
            "DATA_QUALITY_PRIVACY_SCOPE_CASE_ID_INVALID",
            "The Data Quality privacy scope lineage is invalid.",
            reference,
        ),
        CommonLineageError::PartyMissing => lineage_contract_invalid("PARTY_INVALID"),
        CommonLineageError::PartyInvalid(reference) => invalid_contract_with_reference(
            "DATA_QUALITY_PRIVACY_SCOPE_PARTY_INVALID",
            "The Data Quality privacy scope lineage is invalid.",
            reference,
        ),
        CommonLineageError::GenerationInvalid => lineage_contract_invalid("GENERATION_INVALID"),
        CommonLineageError::RegistryInvalid => lineage_contract_invalid("REGISTRY_INVALID"),
        CommonLineageError::RegistryUnavailable(reference) => SdkError::new(
            "DATA_QUALITY_PRIVACY_SCOPE_REGISTRY_UNAVAILABLE",
            ErrorCategory::Internal,
            false,
            "The Data Quality privacy scope registry is unavailable.",
        )
        .with_internal_reference(reference),
        CommonLineageError::RegistryMismatch => lineage_contract_invalid("REGISTRY_MISMATCH"),
        CommonLineageError::PurposeInvalid => lineage_contract_invalid("PURPOSE_INVALID"),
        CommonLineageError::RequestTimeInvalid => lineage_contract_invalid("REQUEST_TIME_INVALID"),
        CommonLineageError::PageSizeInvalid => lineage_contract_invalid("PAGE_SIZE_INVALID"),
    }
}

fn lineage_contract_invalid(suffix: &'static str) -> SdkError {
    let code = match suffix {
        "TENANT_MISMATCH" => "DATA_QUALITY_PRIVACY_SCOPE_TENANT_MISMATCH",
        "PARTY_INVALID" => "DATA_QUALITY_PRIVACY_SCOPE_PARTY_INVALID",
        "GENERATION_INVALID" => "DATA_QUALITY_PRIVACY_SCOPE_GENERATION_INVALID",
        "REGISTRY_INVALID" => "DATA_QUALITY_PRIVACY_SCOPE_REGISTRY_INVALID",
        "REGISTRY_MISMATCH" => "DATA_QUALITY_PRIVACY_SCOPE_REGISTRY_MISMATCH",
        "PURPOSE_INVALID" => "DATA_QUALITY_PRIVACY_SCOPE_PURPOSE_INVALID",
        "REQUEST_TIME_INVALID" => "DATA_QUALITY_PRIVACY_SCOPE_REQUEST_TIME_INVALID",
        "PAGE_SIZE_INVALID" => "DATA_QUALITY_PRIVACY_SCOPE_PAGE_SIZE_INVALID",
        _ => "DATA_QUALITY_PRIVACY_SCOPE_LINEAGE_INVALID",
    };
    invalid_contract(code, "The Data Quality privacy scope lineage is invalid.")
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
        "DATA_QUALITY_PRIVACY_SCOPE_LINEAGE_INVALID",
        category,
        retryable,
        "The requested Data Quality privacy scope is not available.",
    )
    .with_internal_reference(reference.into())
}

pub(crate) fn stored_state_invalid(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "DATA_QUALITY_PRIVACY_SCOPE_STORED_STATE_INVALID",
        ErrorCategory::Unavailable,
        true,
        "The Data Quality privacy scope is temporarily unavailable.",
    )
    .with_internal_reference(reference.into())
}

pub(crate) fn association_state_invalid(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "DATA_QUALITY_PRIVACY_SCOPE_ASSOCIATION_STATE_INVALID",
        ErrorCategory::Unavailable,
        true,
        "The Data Quality privacy scope is temporarily unavailable.",
    )
    .with_internal_reference(reference.into())
}

pub(crate) fn scan_limit_exceeded(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "DATA_QUALITY_PRIVACY_SCOPE_SCAN_LIMIT_EXCEEDED",
        ErrorCategory::Unavailable,
        true,
        "The Data Quality privacy scope is temporarily unavailable.",
    )
    .with_internal_reference(reference.into())
}

pub(crate) fn canonical_resolution_unavailable(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "DATA_QUALITY_PRIVACY_SCOPE_CANONICAL_RESOLUTION_UNAVAILABLE",
        ErrorCategory::Unavailable,
        true,
        "The Data Quality privacy scope is temporarily unavailable.",
    )
    .with_internal_reference(reference.into())
}

pub(crate) fn database_unavailable(error: sqlx::Error) -> SdkError {
    SdkError::new(
        "DATA_QUALITY_PRIVACY_SCOPE_UNAVAILABLE",
        ErrorCategory::Unavailable,
        true,
        "The Data Quality privacy scope is temporarily unavailable.",
    )
    .with_internal_reference(error.to_string())
}
