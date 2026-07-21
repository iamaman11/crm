from pathlib import Path

path = Path("crates/crm-customer-enrichment-provider-process-composition/src/conflict_resolution.rs")
text = path.read_text()
text = text.replace(
    "    BatchError, BatchMutationPlan, BatchMutationResult, PostgresDataStore, RecordMutation,\n",
    "    BatchError, BatchMutationPlan, BatchMutationResult, DataError, PostgresDataStore,\n    RecordMutation,\n",
    1,
)
text = text.replace(
    "            .get_record(&context, &record)\n            .await?\n            .ok_or_else(conflict_not_found)?;",
    "            .get_record(&context, &record)\n            .await\n            .map_err(resolution_read_error)?\n            .ok_or_else(conflict_not_found)?;",
    1,
)
marker = "fn resolution_batch_error(error: BatchError) -> SdkError {\n"
if marker not in text:
    raise SystemExit("resolution_batch_error marker not found")
read_error = '''fn resolution_read_error(error: DataError) -> SdkError {
    let (code, category, retryable) = match &error {
        DataError::Database(_) => (
            "CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_CONFLICT_RESOLUTION_STORE_UNAVAILABLE",
            ErrorCategory::Unavailable,
            true,
        ),
        DataError::Sdk(_) | DataError::InvalidPlan(_) | DataError::InvalidStoredValue(_) => (
            "CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_CONFLICT_RESOLUTION_STATE_INVALID",
            ErrorCategory::Internal,
            false,
        ),
    };
    SdkError::new(
        code,
        category,
        retryable,
        "The provider-response conflict could not be loaded for resolution.",
    )
    .with_internal_reference(error.to_string())
}

'''
text = text.replace(marker, read_error + marker, 1)
path.write_text(text)
