use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::error::Error;
use std::fmt;

pub const MAX_IDENTIFIER_BYTES: usize = 180;
pub const MAX_SAFE_PAYLOAD_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdentifierError {
    message: String,
}

impl IdentifierError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for IdentifierError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for IdentifierError {}

fn validate_identifier(value: &str) -> Result<(), IdentifierError> {
    if value.is_empty() {
        return Err(IdentifierError::new("identifier must not be empty"));
    }
    if value.len() > MAX_IDENTIFIER_BYTES {
        return Err(IdentifierError::new(format!(
            "identifier must not exceed {MAX_IDENTIFIER_BYTES} bytes"
        )));
    }
    if value.chars().any(char::is_control) {
        return Err(IdentifierError::new(
            "identifier must not contain control characters",
        ));
    }
    Ok(())
}

macro_rules! identifier_type {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(String);

        impl $name {
            pub fn try_new(value: impl Into<String>) -> Result<Self, IdentifierError> {
                let value = value.into();
                validate_identifier(&value)?;
                Ok(Self(value))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }

            pub fn into_inner(self) -> String {
                self.0
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                self.as_str()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(self.as_str())
            }
        }

        impl Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                serializer.serialize_str(self.as_str())
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                let value = String::deserialize(deserializer)?;
                Self::try_new(value).map_err(D::Error::custom)
            }
        }
    };
}

identifier_type!(TenantId);
identifier_type!(ActorId);
identifier_type!(RequestId);
identifier_type!(CorrelationId);
identifier_type!(CausationId);
identifier_type!(TraceId);
identifier_type!(CapabilityId);
identifier_type!(CapabilityVersion);
identifier_type!(IdempotencyKey);
identifier_type!(ModuleId);
identifier_type!(RecordId);
identifier_type!(RecordType);
identifier_type!(RelationshipType);
identifier_type!(EventType);
identifier_type!(WorkflowId);
identifier_type!(WorkflowRunId);
identifier_type!(FileId);
identifier_type!(SchemaId);
identifier_type!(SchemaVersion);
identifier_type!(RetentionPolicyId);
identifier_type!(StateKey);
identifier_type!(FieldName);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionContext {
    pub tenant_id: TenantId,
    pub actor_id: ActorId,
    pub request_id: RequestId,
    pub correlation_id: CorrelationId,
    pub causation_id: CausationId,
    pub trace_id: TraceId,
    pub capability_id: CapabilityId,
    pub capability_version: CapabilityVersion,
    pub idempotency_key: IdempotencyKey,
    pub schema_version: SchemaVersion,
    pub request_started_at_unix_nanos: i64,
}

impl ExecutionContext {
    pub fn validate(&self) -> Result<(), SdkError> {
        if self.request_started_at_unix_nanos < 0 {
            return Err(SdkError::invalid_argument(
                "execution_context.request_started_at_unix_nanos",
                "request start time must not be negative",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModuleExecutionContext {
    pub module_id: ModuleId,
    pub execution: ExecutionContext,
}

impl ModuleExecutionContext {
    pub fn validate(&self) -> Result<(), SdkError> {
        self.execution.validate()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DataClass {
    Public,
    Internal,
    Confidential,
    Restricted,
    Personal,
    SensitivePersonal,
    Biometric,
    Financial,
    Credential,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PayloadEncoding {
    Protobuf,
    Json,
    Utf8Text,
    Binary,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TypedPayload {
    pub owner: ModuleId,
    pub schema_id: SchemaId,
    pub schema_version: SchemaVersion,
    pub descriptor_hash: [u8; 32],
    pub data_class: DataClass,
    pub encoding: PayloadEncoding,
    pub maximum_size_bytes: u64,
    pub retention_policy_id: RetentionPolicyId,
    pub bytes: Vec<u8>,
}

impl TypedPayload {
    pub fn validate(&self) -> Result<(), SdkError> {
        if self.maximum_size_bytes > MAX_SAFE_PAYLOAD_BYTES {
            return Err(SdkError::invalid_argument(
                "payload.maximum_size_bytes",
                format!("must not exceed {MAX_SAFE_PAYLOAD_BYTES}"),
            ));
        }
        if self.bytes.len() as u64 > self.maximum_size_bytes {
            return Err(SdkError::invalid_argument(
                "payload.bytes",
                "payload exceeds its declared maximum size",
            ));
        }
        if self.descriptor_hash.iter().all(|byte| *byte == 0) {
            return Err(SdkError::invalid_argument(
                "payload.descriptor_hash",
                "descriptor hash must not be all zeroes",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceRef {
    pub resource_type: String,
    pub resource_id: String,
    pub version: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FieldViolation {
    pub field: FieldName,
    pub code: String,
    pub safe_message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCategory {
    InvalidArgument,
    Authentication,
    Authorization,
    Conflict,
    NotFound,
    RateLimit,
    Dependency,
    Unavailable,
    Internal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SdkError {
    pub code: String,
    pub category: ErrorCategory,
    pub retryable: bool,
    pub safe_message: String,
    pub internal_reference: Option<String>,
    pub field_violations: Vec<FieldViolation>,
}

impl SdkError {
    pub fn new(
        code: impl Into<String>,
        category: ErrorCategory,
        retryable: bool,
        safe_message: impl Into<String>,
    ) -> Self {
        Self {
            code: code.into(),
            category,
            retryable,
            safe_message: safe_message.into(),
            internal_reference: None,
            field_violations: Vec::new(),
        }
    }

    pub fn invalid_argument(field: &str, message: impl Into<String>) -> Self {
        let message = message.into();
        Self {
            code: "SDK_INVALID_ARGUMENT".to_owned(),
            category: ErrorCategory::InvalidArgument,
            retryable: false,
            safe_message: "The request contains invalid data.".to_owned(),
            internal_reference: None,
            field_violations: vec![FieldViolation {
                field: FieldName::try_new(field).expect("static field path must be valid"),
                code: "INVALID".to_owned(),
                safe_message: message,
            }],
        }
    }

    pub fn not_found(resource: &ResourceRef) -> Self {
        Self::new(
            "SDK_NOT_FOUND",
            ErrorCategory::NotFound,
            false,
            format!(
                "{} {} was not found.",
                resource.resource_type, resource.resource_id
            ),
        )
    }

    pub fn with_internal_reference(mut self, reference: impl Into<String>) -> Self {
        self.internal_reference = Some(reference.into());
        self
    }
}

impl fmt::Display for SdkError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code, self.safe_message)
    }
}

impl Error for SdkError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifier_deserialization_rejects_empty_values() {
        let error = serde_json::from_str::<TenantId>("\"\"").expect_err("empty id must fail");
        assert!(error.to_string().contains("must not be empty"));
    }

    #[test]
    fn typed_payload_rejects_oversized_content() {
        let payload = TypedPayload {
            owner: ModuleId::try_new("crm.sales").unwrap(),
            schema_id: SchemaId::try_new("sales.deal.v1").unwrap(),
            schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
            descriptor_hash: [1; 32],
            data_class: DataClass::Internal,
            encoding: PayloadEncoding::Protobuf,
            maximum_size_bytes: 1,
            retention_policy_id: RetentionPolicyId::try_new("standard").unwrap(),
            bytes: vec![1, 2],
        };

        assert_eq!(
            payload.validate().unwrap_err().category,
            ErrorCategory::InvalidArgument
        );
    }
}
