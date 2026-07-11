fn checked_size(value: u64, label: &str) -> Result<i64, BatchError> {
    i64::try_from(value).map_err(|_| BatchError::InvalidPlan(format!("{label} size exceeds i64")))
}

fn relationship_key(relationship: &RelationshipRef) -> String {
    format!(
        "{}:{}:{}->{}:{}",
        relationship.relationship_type,
        relationship.source.record_type,
        relationship.source.record_id,
        relationship.target.record_type,
        relationship.target.record_id
    )
}

fn batch_result_descriptor_hash() -> [u8; 32] {
    Sha256::digest(BATCH_RESULT_SCHEMA_DESCRIPTOR).into()
}

const fn data_class_name(value: DataClass) -> &'static str {
    match value {
        DataClass::Public => "public",
        DataClass::Internal => "internal",
        DataClass::Confidential => "confidential",
        DataClass::Restricted => "restricted",
        DataClass::Personal => "personal",
        DataClass::SensitivePersonal => "sensitive_personal",
        DataClass::Biometric => "biometric",
        DataClass::Financial => "financial",
        DataClass::Credential => "credential",
    }
}

const fn payload_encoding_name(value: PayloadEncoding) -> &'static str {
    match value {
        PayloadEncoding::Protobuf => "protobuf",
        PayloadEncoding::Json => "json",
        PayloadEncoding::Utf8Text => "utf8_text",
        PayloadEncoding::Binary => "binary",
    }
}


fn audit_materialization_to_batch_error(error: AuditMaterializationError) -> BatchError {
    match error {
        AuditMaterializationError::Database(error) => BatchError::Database(error),
        AuditMaterializationError::InvalidIntent(message) => BatchError::InvalidPlan(message),
        AuditMaterializationError::InvalidStoredValue(message) => {
            BatchError::InvalidStoredValue(message)
        }
    }
}

pub fn batch_error_to_sdk(error: BatchError) -> SdkError {
    match error {
        BatchError::Sdk(error) => error,
        BatchError::InvalidPlan(message) | BatchError::InvalidStoredValue(message) => {
            SdkError::new(
                "DATA_INVALID",
                ErrorCategory::InvalidArgument,
                false,
                message,
            )
        }
        BatchError::Conflict(message) => SdkError::new(
            "DATA_CONFLICT",
            ErrorCategory::Conflict,
            false,
            message,
        ),
        BatchError::IdempotencyKeyReused => SdkError::new(
            "DATA_CONFLICT",
            ErrorCategory::Conflict,
            false,
            "The idempotency key was used for a different request.",
        ),
        BatchError::IdempotencyInProgress => SdkError::new(
            "DATA_IDEMPOTENCY_IN_PROGRESS",
            ErrorCategory::Conflict,
            true,
            "The same request is already being processed.",
        ),
        BatchError::Database(error) => SdkError::new(
            "DATA_UNAVAILABLE",
            ErrorCategory::Unavailable,
            true,
            "The data service is temporarily unavailable.",
        )
        .with_internal_reference(error.to_string()),
    }
}
