//! Single diagnostic boundary for the existing native Worker.
//! Original domain owners keep SdkError and the D1 adapter owns commit facts.
//! Only this module serializes technical outcome events and maps SDK errors.
use super::connect_sales::proto::crm::core::v2 as core_v2;
use buffa::MessageName;
use connectrpc::ConnectError;
use crm_module_sdk::{ErrorCategory, SdkError};
use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(super) enum ReceiptUncertainty {
    #[serde(rename = "RECEIPT_READ_FAILED")]
    ReadFailed,
    #[serde(rename = "RECEIPT_NOT_FOUND")]
    NotFound,
    #[serde(rename = "RECEIPT_DECODE_FAILED")]
    DecodeFailed,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(super) enum ReadFailureReason {
    AccessProviderFailed,
    InternalReadError,
    D1ReadFailed,
    SalesStateDecodeFailed,
    SalesMetricsCountOverflow,
    SalesMetricsMoneyOverflow,
    SalesMetricsWeightOverflow,
    SalesMetricsSnapshotTimeInvalid,
    SalesMetricsStageTimeInvalid,
    SalesMetricsStageTimeOverflow,
    SalesMetricsTaskFlagInvalid,
    SalesBoardCountInvalid,
    SalesBoardAmountOverflow,
    SalesBoardAmountCountOverflow,
}
impl From<worker::Error> for ReadFailureReason {
    fn from(_: worker::Error) -> Self {
        Self::D1ReadFailed
    }
}

#[derive(Debug, thiserror::Error)]
pub(super) enum MutationFailure {
    #[error(transparent)]
    Rejected(Box<SdkError>),
    #[error("{code}")]
    CommitUnconfirmed {
        code: &'static str,
        reason: ReceiptUncertainty,
    },
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
enum Outcome {
    Committed,
    Replayed,
    Rejected,
    Unconfirmed,
}

#[derive(Serialize)]
#[serde(tag = "event")]
enum Event<'a> {
    #[serde(rename = "crm.native.request")]
    Request {
        request_id: &'a str,
        operation: &'a str,
        phase: &'static str,
        status: u16,
        error_code: Option<&'a str>,
        duration_ms: u64,
    },
    #[serde(rename = "crm.native.read.failure")]
    ReadFailure {
        request_id: &'a str,
        phase: &'static str,
        reason: ReadFailureReason,
    },
    #[serde(rename = "crm.native.mutation.outcome")]
    Mutation {
        operation: &'static str,
        phase: &'static str,
        request_id: &'a str,
        outcome: Outcome,
        #[serde(skip_serializing_if = "Option::is_none")]
        original_request_id: Option<&'a str>,
        #[serde(skip_serializing_if = "Option::is_none")]
        version: Option<i64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        d1_deal_changes: Option<usize>,
        #[serde(skip_serializing_if = "Option::is_none")]
        domain_code: Option<&'a str>,
        #[serde(skip_serializing_if = "Option::is_none")]
        connect_code: Option<&'a str>,
        #[serde(skip_serializing_if = "Option::is_none")]
        reason: Option<ReceiptUncertainty>,
    },
}

fn emit(event: Event<'_>) {
    // Closed event shapes: no SQL, request payload, identity, credentials or
    // raw provider error are accepted by this production logging boundary.
    worker::console_log!("{}", serde_json::json!(event));
}

pub(super) fn request(
    request_id: &str,
    operation: &str,
    phase: &'static str,
    status: u16,
    error_code: Option<&str>,
    duration_ms: u64,
) {
    emit(Event::Request {
        request_id,
        operation,
        phase,
        status,
        error_code,
        duration_ms,
    });
}

pub(super) fn read_failure(request_id: &str, phase: &'static str, reason: ReadFailureReason) {
    emit(Event::ReadFailure {
        request_id,
        phase,
        reason,
    });
}

// Outer adapter errors are raised before the D1 write is submitted.
// Receipt lookup failure is handled explicitly by the adapter as uncertain.
// A retry of an already uncertain UI intent still retains that intent.
pub(super) fn mutation_preparation_failure(operation: &'static str, request_id: &str) -> ConnectError {
    let code = "SALES_WRITE_PREPARATION_FAILED";
    mutation_error(
        operation,
        request_id,
        MutationFailure::Rejected(Box::new(SdkError::new(
            code,
            ErrorCategory::Unavailable,
            true,
            "The current write was not submitted.",
        ))),
    )
}

pub(super) fn receipt(
    operation: &'static str,
    request_id: &str,
    original_request_id: Option<&str>,
    version: i64,
    replayed: bool,
    d1_deal_changes: Option<usize>,
) -> core_v2::MutationReceipt {
    emit(Event::Mutation {
        operation,
        phase: "receipt",
        request_id,
        outcome: if replayed {
            Outcome::Replayed
        } else {
            Outcome::Committed
        },
        original_request_id,
        version: Some(version),
        d1_deal_changes,
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

fn sdk_connect(error: &SdkError) -> ConnectError {
    let code = &error.code;
    match error.category {
        ErrorCategory::InvalidArgument => ConnectError::invalid_argument(code),
        ErrorCategory::Authentication => ConnectError::unauthenticated(code),
        ErrorCategory::Authorization => ConnectError::permission_denied(code),
        ErrorCategory::Conflict => ConnectError::aborted(code),
        ErrorCategory::NotFound => ConnectError::not_found(code),
        ErrorCategory::RateLimit => ConnectError::resource_exhausted(code),
        ErrorCategory::Dependency | ErrorCategory::Unavailable => ConnectError::unavailable(code),
        ErrorCategory::Internal => ConnectError::internal(code),
    }
}

pub(super) fn mutation_error(
    operation: &'static str,
    request_id: &str,
    failure: MutationFailure,
) -> ConnectError {
    let (error, code, outcome, wire_outcome, field_violations, reason) = match failure {
        MutationFailure::Rejected(failure) => {
            let error = sdk_connect(&failure);
            let fields = failure
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
                failure.code,
                Outcome::Rejected,
                core_v2::MutationCommitOutcome::Rejected,
                fields,
                None,
            )
        }
        MutationFailure::CommitUnconfirmed { code, reason } => (
            ConnectError::unavailable(code),
            code.to_owned(),
            Outcome::Unconfirmed,
            core_v2::MutationCommitOutcome::Unconfirmed,
            Vec::new(),
            Some(reason),
        ),
    };
    emit(Event::Mutation {
        operation,
        phase: "result",
        request_id,
        outcome,
        original_request_id: None,
        version: None,
        d1_deal_changes: None,
        domain_code: Some(&code),
        connect_code: Some(error.code.as_str()),
        reason,
    });
    let detail = core_v2::DomainErrorDetail {
        domain_code: code,
        field_violations,
        request_id: request_id.to_owned(),
        commit_outcome: wire_outcome.into(),
        ..Default::default()
    };
    error.with_detail(connectrpc::ErrorDetail::from_message(
        core_v2::DomainErrorDetail::FULL_NAME,
        &detail,
    ))
}
