use crm_module_sdk::{ErrorCategory, IdentifierError, SdkError};

pub(crate) fn configured<T>(value: Result<T, IdentifierError>) -> Result<T, SdkError> {
    value.map_err(|error| {
        SdkError::new(
            "PARTIES_PRIVACY_SCOPE_CONFIGURATION_INVALID",
            ErrorCategory::Internal,
            false,
            "The Parties privacy scope configuration is invalid.",
        )
        .with_internal_reference(error.to_string())
    })
}

pub(crate) fn map_lineage_error(error: SdkError) -> SdkError {
    let category = if error.category == ErrorCategory::NotFound {
        ErrorCategory::NotFound
    } else {
        ErrorCategory::Conflict
    };
    SdkError::new(
        "PARTIES_PRIVACY_SCOPE_LINEAGE_INVALID",
        category,
        error.retryable,
        "The requested Parties privacy scope is not available.",
    )
    .with_internal_reference(error.code)
}

pub(crate) fn subject_not_found() -> SdkError {
    SdkError::new(
        "PARTIES_PRIVACY_SCOPE_SUBJECT_NOT_FOUND",
        ErrorCategory::NotFound,
        false,
        "The requested Parties privacy scope was not found.",
    )
}

pub(crate) fn stored_state_invalid(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "PARTIES_PRIVACY_SCOPE_STORED_STATE_INVALID",
        ErrorCategory::Unavailable,
        true,
        "The Parties privacy scope is temporarily unavailable.",
    )
    .with_internal_reference(reference.into())
}

pub(crate) fn row_decode_error(error: sqlx::Error) -> SdkError {
    stored_state_invalid(error.to_string())
}

pub(crate) fn database_unavailable(error: sqlx::Error) -> SdkError {
    SdkError::new(
        "PARTIES_PRIVACY_SCOPE_UNAVAILABLE",
        ErrorCategory::Unavailable,
        true,
        "The Parties privacy scope is temporarily unavailable.",
    )
    .with_internal_reference(error.to_string())
}

pub(crate) fn invalid_contract(
    code: &'static str,
    safe_message: &'static str,
) -> SdkError {
    SdkError::new(code, ErrorCategory::InvalidArgument, false, safe_message)
}

pub(crate) fn invalid_contract_with_reference(
    code: &'static str,
    safe_message: &'static str,
    reference: impl Into<String>,
) -> SdkError {
    invalid_contract(code, safe_message).with_internal_reference(reference.into())
}
