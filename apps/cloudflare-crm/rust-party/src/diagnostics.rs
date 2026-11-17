//! Shared Worker diagnostic boundary. Domain owners report facts; only this
//! module maps semantic failures to Connect and emits technical events.
use crate::connect_sales::proto::crm::core::v2 as core_v2;
use connectrpc::ConnectError;
use crm_module_sdk::{ErrorCategory, SdkError};
use serde::Serialize;
use serde_json::json;

#[derive(Debug, Clone, Copy)]
pub(super) enum ReceiptUncertainty {
    NotFound,
    ReadFailed,
    Invalid,
}
impl ReceiptUncertainty {
    const fn as_str(self) -> &'static str {
        match self {
            Self::NotFound => "RECEIPT_NOT_FOUND",
            Self::ReadFailed => "RECEIPT_READ_FAILED",
            Self::Invalid => "RECEIPT_INVALID",
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub(super) enum MutationFailure {
    #[error(transparent)]
    Rejected(Box<SdkError>),
    #[error("{code}")]
    Unconfirmed {
        code: &'static str,
        reason: ReceiptUncertainty,
    },
}

#[derive(Serialize)]
struct RequestEvent<'a> {
    event: &'static str,
    request_id: &'a str,
    operation: &'a str,
    phase: &'a str,
    status: u16,
    error_code: Option<&'a str>,
    duration_ms: u64,
}
#[derive(Serialize)]
struct MutationOutcomeEvent<'a> {
    event: &'static str,
    operation: &'static str,
    phase: &'static str,
    request_id: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    original_request_id: Option<&'a str>,
    outcome: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    version: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    domain_code: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    connect_code: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<&'static str>,
}
fn emit(event: &impl Serialize) {
    if let Ok(encoded) = serde_json::to_string(event) {
        worker::console_log!("{}", encoded);
    }
}

pub(super) fn request_completed(
    request_id: &str,
    operation: &str,
    phase: &str,
    status: u16,
    error_code: Option<&str>,
    duration_ms: u64,
) {
    emit(&RequestEvent {
        event: "crm.native.request",
        request_id,
        operation,
        phase,
        status,
        error_code,
        duration_ms,
    });
}

pub(super) fn mutation_receipt(
    request_id: &str,
    operation: &'static str,
    version: i64,
    replayed: bool,
    original_request_id: Option<&str>,
) -> core_v2::MutationReceipt {
    emit(&MutationOutcomeEvent {
        event: "crm.native.mutation.outcome",
        operation,
        phase: "receipt",
        request_id,
        original_request_id,
        outcome: if replayed { "replayed" } else { "committed" },
        version: Some(version),
        domain_code: None,
        connect_code: None,
        reason: None,
    });
    core_v2::MutationReceipt {
        request_id: request_id.to_owned(),
        resource_version: version,
        replayed,
        original_request_id: original_request_id.map(str::to_owned),
        ..Default::default()
    }
}

pub(super) fn mutation_error(
    request_id: &str,
    operation: &'static str,
    failure: MutationFailure,
) -> ConnectError {
    use buffa::MessageName;
    let (error, code, outcome, field_violations, reason) = match failure {
        MutationFailure::Rejected(failure) => {
            let code = failure.code;
            let error = match failure.category {
                ErrorCategory::InvalidArgument => ConnectError::invalid_argument(&code),
                ErrorCategory::Authentication => ConnectError::unauthenticated(&code),
                ErrorCategory::Authorization => ConnectError::permission_denied(&code),
                ErrorCategory::Conflict => ConnectError::aborted(&code),
                ErrorCategory::NotFound => ConnectError::not_found(&code),
                ErrorCategory::RateLimit => ConnectError::resource_exhausted(&code),
                ErrorCategory::Dependency | ErrorCategory::Unavailable => {
                    ConnectError::unavailable(&code)
                }
                ErrorCategory::Internal => ConnectError::internal(&code),
            };
            let field_violations = failure
                .field_violations
                .into_iter()
                .map(|field| core_v2::FieldViolation {
                    field_path: field.field.as_str().to_owned(),
                    description: field.safe_message,
                    ..Default::default()
                })
                .collect();
            (
                error,
                code,
                core_v2::MutationCommitOutcome::Rejected,
                field_violations,
                None,
            )
        }
        MutationFailure::Unconfirmed { code, reason } => {
            let code = code.to_owned();
            (
                ConnectError::unavailable(&code),
                code,
                core_v2::MutationCommitOutcome::Unconfirmed,
                Vec::new(),
                Some(reason.as_str()),
            )
        }
    };
    emit(&MutationOutcomeEvent {
        event: "crm.native.mutation.outcome",
        operation,
        phase: "result",
        request_id,
        original_request_id: None,
        outcome: if outcome == core_v2::MutationCommitOutcome::Rejected {
            "rejected"
        } else {
            "unconfirmed"
        },
        version: None,
        domain_code: Some(&code),
        connect_code: Some(error.code.as_str()),
        reason,
    });
    let detail = core_v2::DomainErrorDetail {
        domain_code: code,
        field_violations,
        request_id: request_id.to_owned(),
        commit_outcome: outcome.into(),
        ..Default::default()
    };
    error.with_detail(connectrpc::ErrorDetail::from_message(
        core_v2::DomainErrorDetail::FULL_NAME,
        &detail,
    ))
}

// Log structured, non-payload diagnostic evidence at the failed boundary.
// Detailed provider errors are restricted to isolated native CI fixtures.
pub(super) fn log_read_failure(request_id: &str, phase: &str, error: &worker::Error) {
    let reason = match error {
        worker::Error::RustError(code)
            if (code.starts_with("SALES_") || code.starts_with("CONTRACT_"))
                && code.bytes().all(|b| b.is_ascii_uppercase() || b == b'_') =>
        {
            code.as_str()
        }
        _ => "INTERNAL_READ_ERROR",
    };
    emit(
        &json!({"event":"crm.native.read.failure","request_id":request_id,
               "phase":phase,"reason":reason}),
    );
    #[cfg(feature = "native-ci")]
    emit(
        &json!({"event":"crm.native-ci.read.failure","request_id":request_id,
               "phase":phase,"detail":error.to_string()}),
    );
}
