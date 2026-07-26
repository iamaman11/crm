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

pub(crate) fn invalid_contract(code: &'static str, safe_message: &'static str) -> SdkError {
    SdkError::new(code, ErrorCategory::InvalidArgument, false, safe_message)
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
