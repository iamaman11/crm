from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
worker_path = ROOT / "crates/crm-customer-enrichment-worker-composition/src/lib.rs"
process_test_path = ROOT / "crates/crm-customer-enrichment-worker-composition/tests/postgres_worker_process.rs"

worker = worker_path.read_text()
process_test = process_test_path.read_text()


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected exactly one match, found {count}")
    return text.replace(old, new, 1)


worker = replace_once(
    worker,
    """use crm_customer_enrichment::{
    ProviderAdapterRegistryPort, ProviderDispatchRequest, ProviderResponseClass,
    SanitizedProviderResponse,
};
""",
    """use crm_customer_enrichment::{
    ProviderAdapterRegistryPort, ProviderDispatchRequest, ProviderResponseClass,
    ProviderResponseConflictDraft, ProviderResponseReceipt, ProviderResponseReceiptDraft,
    SanitizedProviderResponse,
};
""",
    "worker imports",
)

worker = replace_once(
    worker,
    """pub struct ProviderDispatchWorkerResult {
    pub dispatch_replayed: bool,
    pub response_replayed: bool,
    pub response_reconciliation: ProviderResponseReconciliation,
    pub response: wire::RecordProviderResponseResponse,
}

/// Infrastructure coordinator for commit-before-I/O and atomic response recording.
""",
    """pub struct ProviderDispatchWorkerResult {
    pub dispatch_replayed: bool,
    pub response_replayed: bool,
    pub response_reconciliation: ProviderResponseReconciliation,
    pub response: wire::RecordProviderResponseResponse,
}

/// Structured result used by provider-process orchestration before durable conflict persistence.
#[derive(Debug, Clone, PartialEq)]
pub enum ProviderDispatchExecution {
    Recorded(ProviderDispatchWorkerResult),
    Conflicting(ProviderResponseConflictDraft),
}

/// Infrastructure coordinator for commit-before-I/O and atomic response recording.
""",
    "structured result",
)

old_execute = """    pub async fn execute(
        &self,
        item: ProviderDispatchWorkItem,
    ) -> Result<ProviderDispatchWorkerResult, SdkError> {
        let expectation = validate_work_item(&self.dispatch_definition, &item)?;

        let dispatch_result = self
            .dispatch_executor
            .execute(&self.dispatch_definition, item.dispatch_request.clone())
            .await?;
        validate_dispatch_output(&dispatch_result, &item.provider_request, expectation)?;

        let sanitized = self
            .registry
            .dispatch_exact(item.provider_request.clone())
            .await?;
        validate_sanitized_response(&item.provider_request, &sanitized)?;

        let response_request = build_response_request(
            &self.response_definition,
            &item.dispatch_request,
            &item.provider_request,
            &sanitized,
        )?;
        let response_result = self
            .response_executor
            .execute(&self.response_definition, response_request)
            .await
            .map_err(response_reconciliation_error)?;
        let response: wire::RecordProviderResponseResponse = decode_execution_output(
            &response_result,
            RECORD_PROVIDER_RESPONSE_RESPONSE_SCHEMA,
            DataClass::Personal,
        )?;
        validate_response_output(
            &response,
            &item.provider_request,
            &sanitized,
            response_result.replayed,
        )?;
        let response_reconciliation =
            classify_response_reconciliation(&response, &sanitized, response_result.replayed)?;

        Ok(ProviderDispatchWorkerResult {
            dispatch_replayed: dispatch_result.replayed,
            response_replayed: response_result.replayed,
            response_reconciliation,
            response,
        })
    }
"""
new_execute = """    pub async fn execute(
        &self,
        item: ProviderDispatchWorkItem,
    ) -> Result<ProviderDispatchWorkerResult, SdkError> {
        match self.execute_reconciled(item).await? {
            ProviderDispatchExecution::Recorded(result) => Ok(result),
            ProviderDispatchExecution::Conflicting(_) => Err(conflicting_provider_replay()),
        }
    }

    /// Executes one provider attempt while preserving a typed conflict candidate for orchestration.
    ///
    /// The ordinary `execute` API remains fail-closed for existing callers. Provider-process
    /// composition uses this method to persist exact conflict evidence without parsing error text.
    pub async fn execute_reconciled(
        &self,
        item: ProviderDispatchWorkItem,
    ) -> Result<ProviderDispatchExecution, SdkError> {
        let expectation = validate_work_item(&self.dispatch_definition, &item)?;

        let dispatch_result = self
            .dispatch_executor
            .execute(&self.dispatch_definition, item.dispatch_request.clone())
            .await?;
        validate_dispatch_output(&dispatch_result, &item.provider_request, expectation)?;

        let sanitized = self
            .registry
            .dispatch_exact(item.provider_request.clone())
            .await?;
        validate_sanitized_response(&item.provider_request, &sanitized)?;

        let response_request = build_response_request(
            &self.response_definition,
            &item.dispatch_request,
            &item.provider_request,
            &sanitized,
        )?;
        let conflicting_semantic_fingerprint = response_request.input_hash;
        let response_result = match self
            .response_executor
            .execute(&self.response_definition, response_request)
            .await
        {
            Ok(result) => result,
            Err(error) if is_response_reconciliation_conflict(&error) => {
                return Ok(ProviderDispatchExecution::Conflicting(
                    provider_response_conflict_draft(
                        &item.provider_request,
                        &sanitized,
                        conflicting_semantic_fingerprint,
                    )?,
                ));
            }
            Err(error) => return Err(error),
        };
        let response: wire::RecordProviderResponseResponse = decode_execution_output(
            &response_result,
            RECORD_PROVIDER_RESPONSE_RESPONSE_SCHEMA,
            DataClass::Personal,
        )?;
        validate_response_output(
            &response,
            &item.provider_request,
            &sanitized,
            response_result.replayed,
        )?;
        let response_reconciliation =
            classify_response_reconciliation(&response, &sanitized, response_result.replayed)?;

        Ok(ProviderDispatchExecution::Recorded(
            ProviderDispatchWorkerResult {
                dispatch_replayed: dispatch_result.replayed,
                response_replayed: response_result.replayed,
                response_reconciliation,
                response,
            },
        ))
    }
"""
worker = replace_once(worker, old_execute, new_execute, "execute implementation")

worker = replace_once(
    worker,
    """fn response_reconciliation_error(error: SdkError) -> SdkError {
    if matches!(
        error.code.as_str(),
        "DATA_CONFLICT" | "CAPABILITY_IDEMPOTENCY_KEY_REUSED"
    ) {
        return SdkError::new(
            "CUSTOMER_ENRICHMENT_CONFLICTING_PROVIDER_REPLAY",
            ErrorCategory::Conflict,
            false,
            "The provider replay conflicts with immutable response evidence.",
        )
        .with_internal_reference("response idempotency semantic fingerprint conflict");
    }
    error
}
""",
    """fn provider_response_conflict_draft(
    request: &ProviderDispatchRequest,
    response: &SanitizedProviderResponse,
    conflicting_semantic_fingerprint: [u8; 32],
) -> Result<ProviderResponseConflictDraft, SdkError> {
    let provider_observed_at_unix_ms = response
        .provider_observed_at_unix_ms
        .map(u64::try_from)
        .transpose()
        .map_err(|_| worker_input_invalid("provider observation timestamp is negative"))?;
    let retrieved_at_unix_ms = u64::try_from(response.retrieved_at_unix_ms)
        .map_err(|_| worker_input_invalid("provider retrieval timestamp is negative"))?;
    let first_receipt = ProviderResponseReceipt::record(ProviderResponseReceiptDraft {
        request_id: request.enrichment_request_id.clone(),
        provider_profile_version_id: request.provider_profile_version_id.clone(),
        mapping_version_id: request.mapping_version_id.clone(),
        replay_key: response.replay_key.clone(),
        provider_correlation_id: response.provider_correlation_id.clone(),
        response_class: response.response_class,
        canonical_response_digest: response.canonical_response_digest,
        provider_observed_at_unix_ms,
        retrieved_at_unix_ms,
        metered_units: response.metered_units,
        protected_evidence_reference: response.protected_evidence_reference.clone(),
    })?;
    Ok(ProviderResponseConflictDraft {
        tenant_id: request.tenant_id.clone(),
        request_id: request.enrichment_request_id.clone(),
        retry_generation: request.retry_generation,
        first_receipt_id: first_receipt.receipt_id().clone(),
        conflicting_semantic_fingerprint,
        detected_at_unix_ms: retrieved_at_unix_ms,
    })
}

fn is_response_reconciliation_conflict(error: &SdkError) -> bool {
    matches!(
        error.code.as_str(),
        "DATA_CONFLICT" | "CAPABILITY_IDEMPOTENCY_KEY_REUSED"
    )
}

fn conflicting_provider_replay() -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_CONFLICTING_PROVIDER_REPLAY",
        ErrorCategory::Conflict,
        false,
        "The provider replay conflicts with immutable response evidence.",
    )
    .with_internal_reference("response idempotency semantic fingerprint conflict")
}

fn response_reconciliation_error(error: SdkError) -> SdkError {
    if is_response_reconciliation_conflict(&error) {
        return conflicting_provider_replay();
    }
    error
}
""",
    "conflict helpers",
)

process_test = replace_once(
    process_test,
    """use crm_customer_enrichment_worker_composition::{
    CustomerEnrichmentProviderWorker, ProviderDispatchWorkItem, ProviderResponseReconciliation,
};
""",
    """use crm_customer_enrichment_worker_composition::{
    CustomerEnrichmentProviderWorker, ProviderDispatchExecution, ProviderDispatchWorkItem,
    ProviderResponseReconciliation,
};
""",
    "process-test import",
)

process_test = replace_once(
    process_test,
    """    let conflict = worker
        .execute(fixture.work_item.clone())
        .await
        .expect_err("reject conflicting canonical provider response");
    assert_eq!(
        conflict.code,
        "CUSTOMER_ENRICHMENT_CONFLICTING_PROVIDER_REPLAY"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 4);
""",
    """    let conflict = worker
        .execute_reconciled(fixture.work_item.clone())
        .await
        .expect("return typed conflicting canonical provider response");
    let ProviderDispatchExecution::Conflicting(conflict) = conflict else {
        panic!("expected a typed provider-response conflict");
    };
    let first_receipt_id = first
        .response
        .provider_response_receipt
        .as_ref()
        .and_then(|receipt| receipt.provider_response_receipt_ref.as_ref())
        .expect("first response receipt identity")
        .provider_response_receipt_id
        .clone();
    assert_eq!(conflict.tenant_id.as_str(), TENANT_ID);
    assert_eq!(
        conflict.request_id.as_str(),
        fixture.provider_request.enrichment_request_id.as_str()
    );
    assert_eq!(conflict.retry_generation, 0);
    assert_eq!(conflict.first_receipt_id.as_str(), first_receipt_id);
    assert!(
        conflict
            .conflicting_semantic_fingerprint
            .iter()
            .any(|byte| *byte != 0)
    );
    assert_eq!(conflict.detected_at_unix_ms, 32);

    let fail_closed = worker
        .execute(fixture.work_item.clone())
        .await
        .expect_err("ordinary worker API remains fail closed");
    assert_eq!(
        fail_closed.code,
        "CUSTOMER_ENRICHMENT_CONFLICTING_PROVIDER_REPLAY"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 5);
""",
    "process-test conflict assertions",
)

worker_path.write_text(worker)
process_test_path.write_text(process_test)
print("applied structured provider conflict patch")
