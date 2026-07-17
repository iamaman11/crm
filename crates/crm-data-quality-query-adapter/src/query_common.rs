fn apply_definition_visibility<T>(definition: &mut Option<T>, definition_visible: bool) {
    if !definition_visible { *definition = None; }
}
fn validate_record_id(value: String, field: &'static str) -> Result<(), SdkError> {
    RecordId::try_new(value).map(|_| ()).map_err(|_| SdkError::invalid_argument(field, "Data Quality record reference is invalid"))
}
fn ensure_definition(definition: &CapabilityDefinition) -> Result<(), SdkError> {
    if !QUERY_CAPABILITY_IDS.contains(&definition.capability_id.as_str()) || definition.owner_module_id.as_str() != MODULE_ID { return Err(unsupported_query()); }
    Ok(())
}
fn module_id() -> Result<ModuleId, SdkError> { ModuleId::try_new(MODULE_ID).map_err(|_| configuration_error()) }
fn finding_record_type() -> Result<RecordType, SdkError> { RecordType::try_new(FINDING_RECORD_TYPE).map_err(|_| configuration_error()) }
fn enforce_scan_limit(scanned: usize) -> Result<(), SdkError> { if scanned > MAXIMUM_VISIBILITY_SCAN_RECORDS { Err(scan_limit_error()) } else { Ok(()) } }
fn rule_set_not_found() -> SdkError { SdkError::new("DATA_QUALITY_PARTY_RULE_SET_NOT_FOUND", ErrorCategory::NotFound, false, "The requested Party rule-set version was not found.") }
fn completeness_profile_not_found() -> SdkError { SdkError::new("DATA_QUALITY_PARTY_COMPLETENESS_PROFILE_NOT_FOUND", ErrorCategory::NotFound, false, "The requested Party completeness-profile version was not found.") }
fn evaluation_job_not_found() -> SdkError { SdkError::new("DATA_QUALITY_PARTY_EVALUATION_NOT_FOUND", ErrorCategory::NotFound, false, "The requested Party evaluation job was not found.") }
fn finding_not_found() -> SdkError { SdkError::new("DATA_QUALITY_FINDING_NOT_FOUND", ErrorCategory::NotFound, false, "The requested Data Quality finding was not found.") }
fn completeness_result_not_found() -> SdkError { SdkError::new("DATA_QUALITY_PARTY_COMPLETENESS_RESULT_NOT_FOUND", ErrorCategory::NotFound, false, "The requested Party completeness result was not found.") }
fn persisted_reference_missing() -> SdkError { SdkError::new("DATA_QUALITY_PERSISTED_STATE_INVALID", ErrorCategory::Internal, false, "The persisted Data Quality state is invalid.").with_internal_reference("Party completeness profile references a missing rule-set version") }
fn persisted_observation_missing() -> SdkError { SdkError::new("DATA_QUALITY_PERSISTED_STATE_INVALID", ErrorCategory::Internal, false, "The persisted Data Quality state is invalid.").with_internal_reference("Data Quality finding references a missing current observation") }
fn unsupported_query() -> SdkError { SdkError::new("DATA_QUALITY_QUERY_UNSUPPORTED", ErrorCategory::InvalidArgument, false, "The requested Data Quality query is not supported.") }
fn configuration_error() -> SdkError { SdkError::new("DATA_QUALITY_QUERY_CONFIGURATION_INVALID", ErrorCategory::Internal, false, "The Data Quality query configuration is invalid.") }
fn cursor_invalid() -> SdkError { SdkError::new("DATA_QUALITY_FINDING_QUERY_CURSOR_INVALID", ErrorCategory::InvalidArgument, false, "The Data Quality finding page cursor is invalid.") }
fn cursor_error(error: impl std::fmt::Display) -> SdkError { cursor_invalid().with_internal_reference(error.to_string()) }
fn scan_limit_error() -> SdkError { SdkError::new("DATA_QUALITY_FINDING_QUERY_SCAN_LIMIT_EXCEEDED", ErrorCategory::Unavailable, true, "The Data Quality finding query exceeded its bounded visibility scan limit.") }
