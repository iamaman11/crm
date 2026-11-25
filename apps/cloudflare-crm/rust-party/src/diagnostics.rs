//! Single diagnostic boundary for the existing native Worker.
//! Original domain owners keep SdkError and the D1 adapter owns commit facts.
//! Only this module serializes technical outcome events and maps SDK errors.
use super::connect_sales::proto::crm::core::v2 as core_v2;
use buffa::MessageName;
use connectrpc::{ConnectError, ErrorCode};
use crm_module_sdk::{ErrorCategory, SdkError};
use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(super) enum ReceiptUncertainty {
    StorageUnavailable,
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

impl ReadFailureReason {
    pub(super) fn dispatch(error: &worker::Error) -> Self {
        match error {
            worker::Error::D1(_) => Self::D1ReadFailed,
            _ => Self::InternalReadError,
        }
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

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Phase {
    Access,
    Diagnostics,
    #[serde(rename = "domain_d1")]
    DomainD1,
    Receipt,
    Result,
    SalesDealList,
    SalesDealRead,
    SalesHistoryOwner,
    SalesHistoryRead,
    SalesMetrics,
    SalesPipelineRead,
    SalesPipelineReceiptDecode,
    SalesPipelineReceiptLookup,
    SalesPortfolio,
    SalesReceiptLookup,
    SalesStageList,
    SalesViewRead,
    SalesViewReceiptDecode,
    SalesViewReceiptLookup,
}

#[derive(Serialize)]
#[serde(tag = "event")]
enum Event<'a> {
    #[serde(rename = "crm.native.request")]
    Request {
        request_id: &'a str,
        operation: &'a str,
        phase: Phase,
        status: u16,
        error_code: Option<&'a str>,
        duration_ms: u64,
    },
    #[serde(rename = "crm.native.read.failure")]
    ReadFailure {
        request_id: &'a str,
        phase: Phase,
        reason: ReadFailureReason,
    },
    #[serde(rename = "crm.native.mutation.outcome")]
    Mutation {
        operation: &'static str,
        phase: Phase,
        request_id: &'a str,
        outcome: Outcome,
        #[serde(skip_serializing_if = "Option::is_none")]
        original_request_id: Option<&'a str>,
        #[serde(skip_serializing_if = "Option::is_none")]
        version: Option<i64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        // Raw D1Result.meta().changes for the primary write statement, not a Deal count.
        d1_statement_changes: Option<usize>,
        #[serde(skip_serializing_if = "Option::is_none")]
        domain_code: Option<&'a str>,
        #[serde(skip_serializing_if = "Option::is_none")]
        connect_code: Option<ErrorCode>,
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
    phase: Phase,
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

pub(super) fn read_failure(request_id: &str, phase: Phase, reason: ReadFailureReason) {
    emit(Event::ReadFailure {
        request_id,
        phase,
        reason,
    });
}

pub(super) fn read_error(request_id: &str, error: &worker::Error) -> ConnectError {
    read_failure(
        request_id,
        Phase::DomainD1,
        ReadFailureReason::dispatch(error),
    );
    ConnectError::internal("INTERNAL_ERROR")
}

// Outer adapter errors are raised before the D1 write is submitted.
// Receipt lookup failure is handled explicitly by the adapter as uncertain.
// A retry of an already uncertain UI intent still retains that intent.
pub(super) fn mutation_preparation_failure(
    operation: &'static str,
    request_id: &str,
) -> ConnectError {
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
    d1_statement_changes: Option<usize>,
) -> core_v2::MutationReceipt {
    emit(Event::Mutation {
        operation,
        phase: Phase::Receipt,
        request_id,
        outcome: if replayed {
            Outcome::Replayed
        } else {
            Outcome::Committed
        },
        original_request_id,
        version: Some(version),
        d1_statement_changes,
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
        phase: Phase::Result,
        request_id,
        outcome,
        original_request_id: None,
        version: None,
        d1_statement_changes: None,
        domain_code: Some(&code),
        connect_code: Some(error.code),
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

fn fallback_write_outcome(
    code: ErrorCode,
    before_dispatch: bool,
) -> core_v2::MutationCommitOutcome {
    if before_dispatch
        || matches!(
            code,
            ErrorCode::InvalidArgument
                | ErrorCode::NotFound
                | ErrorCode::AlreadyExists
                | ErrorCode::PermissionDenied
                | ErrorCode::FailedPrecondition
                | ErrorCode::Aborted
                | ErrorCode::OutOfRange
                | ErrorCode::Unauthenticated
                | ErrorCode::Unimplemented
        )
    {
        core_v2::MutationCommitOutcome::Rejected
    } else {
        core_v2::MutationCommitOutcome::Unconfirmed
    }
}

/// Every write uses the same protobuf outcome, including errors raised before
/// the owner runs. Preserve stronger owner evidence when it is available.
pub(super) fn wire_write_error(
    error: ConnectError,
    request_id: &str,
    before_dispatch: bool,
) -> ConnectError {
    if error
        .details
        .iter()
        .any(|d| d.type_url == core_v2::DomainErrorDetail::FULL_NAME)
    {
        return error;
    }
    let outcome = fallback_write_outcome(error.code, before_dispatch);
    // Fallback classification comes only from the typed Connect code.
    let domain_code = error.code.as_str().to_ascii_uppercase();
    let detail = core_v2::DomainErrorDetail {
        request_id: request_id.to_owned(),
        domain_code,
        commit_outcome: outcome.into(),
        ..Default::default()
    };
    error.with_detail(connectrpc::ErrorDetail::from_message(
        core_v2::DomainErrorDetail::FULL_NAME,
        &detail,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sdk_categories_preserve_the_connect_code() {
        for (category, expected) in [
            (ErrorCategory::InvalidArgument, ErrorCode::InvalidArgument),
            (ErrorCategory::Authentication, ErrorCode::Unauthenticated),
            (ErrorCategory::Authorization, ErrorCode::PermissionDenied),
            (ErrorCategory::Conflict, ErrorCode::Aborted),
            (ErrorCategory::NotFound, ErrorCode::NotFound),
            (ErrorCategory::RateLimit, ErrorCode::ResourceExhausted),
            (ErrorCategory::Dependency, ErrorCode::Unavailable),
            (ErrorCategory::Unavailable, ErrorCode::Unavailable),
            (ErrorCategory::Internal, ErrorCode::Internal),
        ] {
            let failure = SdkError::new("TYPED_CODE", category, false, "private-fixture");
            let error = sdk_connect(&failure);
            assert_eq!(error.code, expected);
            assert_eq!(error.message.as_deref(), Some("TYPED_CODE"));
        }
    }

    #[test]
    fn read_event_preserves_safe_typed_reason_and_correlation() {
        let event = Event::ReadFailure {
            request_id: "request-1",
            phase: Phase::SalesMetrics,
            reason: ReadFailureReason::D1ReadFailed,
        };
        let value = serde_json::to_value(event).unwrap();
        assert_eq!(value["request_id"], "request-1");
        assert_eq!(value["phase"], "sales_metrics");
        assert_eq!(value["reason"], "D1_READ_FAILED");
        let source = worker::Error::RustError("private-fixture".into());
        let internal = serde_json::to_value(Event::ReadFailure {
            request_id: "request-2",
            phase: Phase::DomainD1,
            reason: ReadFailureReason::dispatch(&source),
        })
        .unwrap();
        assert_eq!(internal["reason"], "INTERNAL_READ_ERROR");
        assert_eq!(
            internal,
            serde_json::json!({
                "event": "crm.native.read.failure",
                "request_id": "request-2",
                "phase": "domain_d1",
                "reason": "INTERNAL_READ_ERROR"
            })
        );
    }

    #[test]
    fn fallback_outcome_is_based_on_code_and_dispatch_boundary() {
        assert_eq!(
            fallback_write_outcome(ErrorCode::Aborted, false),
            core_v2::MutationCommitOutcome::Rejected
        );
        assert_eq!(
            fallback_write_outcome(ErrorCode::Internal, false),
            core_v2::MutationCommitOutcome::Unconfirmed
        );
        assert_eq!(
            fallback_write_outcome(ErrorCode::Internal, true),
            core_v2::MutationCommitOutcome::Rejected
        );
    }

    #[test]
    fn wire_detail_carries_a_typed_outcome_and_keeps_stronger_owner_evidence() {
        let detail = |domain_code: &str, outcome: core_v2::MutationCommitOutcome| {
            connectrpc::ErrorDetail::from_message(
                core_v2::DomainErrorDetail::FULL_NAME,
                &core_v2::DomainErrorDetail {
                    request_id: "request-1".into(),
                    domain_code: domain_code.into(),
                    commit_outcome: outcome.into(),
                    ..Default::default()
                },
            )
        };
        let fallback =
            wire_write_error(ConnectError::internal("UNTRUSTED_TEXT"), "request-1", false);
        let expected = detail("INTERNAL", core_v2::MutationCommitOutcome::Unconfirmed);
        assert_eq!(fallback.details.len(), 1);
        assert_eq!(fallback.details[0].type_url, expected.type_url);
        assert_eq!(fallback.details[0].value, expected.value);
        assert!(fallback.details[0].debug.is_none());

        let owner = detail(
            "TYPED_OWNER_CONFLICT",
            core_v2::MutationCommitOutcome::Rejected,
        );
        let expected = owner.value.clone();
        let preserved = wire_write_error(
            ConnectError::aborted("LOCALIZED_TEXT").with_detail(owner),
            "request-1",
            false,
        );
        assert_eq!(preserved.details.len(), 1);
        assert_eq!(preserved.details[0].value, expected);
    }
}
