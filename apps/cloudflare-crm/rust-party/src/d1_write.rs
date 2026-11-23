//! Reconcile a single owner statement with its atomic, tenant-scoped receipt.
use super::diagnostics::{MutationFailure, ReceiptUncertainty};
use crm_module_sdk::{ErrorCategory, SdkError};
use serde::Deserialize;
use worker::{D1Result, Result as D1ResultOf};

pub(super) type WriteResult<T> = Result<T, MutationFailure>;

#[derive(Deserialize)]
pub(super) struct Receipt {
    pub digest: String,
    pub state: String,
    pub original_request_id: Option<String>,
}

pub(super) fn rejected(code: impl Into<String>, category: ErrorCategory) -> MutationFailure {
    MutationFailure::Rejected(Box::new(SdkError::new(
        code,
        category,
        false,
        "Command rejected.",
    )))
}

pub(super) enum Confirmation<R> {
    Applied,
    Replayed(R),
}

pub(super) async fn confirm<R>(
    write: D1ResultOf<D1Result>,
    lookup: impl Future<Output = D1ResultOf<Option<R>>>,
    zero_code: &'static str,
    unconfirmed_code: &'static str,
) -> WriteResult<Confirmation<R>> {
    let changes = write
        .ok()
        .filter(|result| result.success())
        .and_then(|result| result.meta().ok().flatten())
        .and_then(|meta| meta.changes);
    if changes.is_some_and(|count| count > 0) {
        return Ok(Confirmation::Applied);
    }
    let unknown = |reason| MutationFailure::CommitUnconfirmed {
        code: unconfirmed_code,
        reason,
    };
    match lookup.await {
        Ok(Some(receipt)) => Ok(Confirmation::Replayed(receipt)),
        Ok(None) if changes == Some(0) => Err(rejected(zero_code, ErrorCategory::Conflict)),
        Ok(None) => Err(unknown(ReceiptUncertainty::NotFound)),
        Err(_) => Err(unknown(ReceiptUncertainty::ReadFailed)),
    }
}

/// Domain rejections keep their Connect semantics. Storage failures carry
/// the evidence needed to describe a mutation's commit outcome.
pub(super) enum OwnerFailure {
    Domain {
        code: String,
        kind: connectrpc::ErrorCode,
    },
    Write(MutationFailure),
}
impl OwnerFailure {
    pub(super) fn domain(code: impl Into<String>, kind: connectrpc::ErrorCode) -> Self {
        Self::Domain {
            code: code.into(),
            kind,
        }
    }
    pub(super) fn connect(
        self,
        operation: &'static str,
        request_id: &str,
    ) -> connectrpc::ConnectError {
        match self {
            Self::Domain { code, kind } => connectrpc::ConnectError::new(kind, code),
            Self::Write(failure) => {
                super::diagnostics::mutation_error(operation, request_id, failure)
            }
        }
    }
}

/// Batches and triggered lineage writes have an immutable operation receipt.
pub(super) async fn require_receipt<R>(
    lookup: impl Future<Output = D1ResultOf<Option<R>>>,
    code: &'static str,
) -> Result<R, OwnerFailure> {
    let reason = match lookup.await {
        Ok(Some(receipt)) => return Ok(receipt),
        Ok(None) => ReceiptUncertainty::NotFound,
        Err(_) => ReceiptUncertainty::ReadFailed,
    };
    Err(OwnerFailure::Write(MutationFailure::CommitUnconfirmed {
        code,
        reason,
    }))
}
