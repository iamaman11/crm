from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
TARGETS = {
    "lifecycle": ROOT / "modules/crm-customer-enrichment/src/lifecycle.rs",
    "worker": ROOT / "crates/crm-customer-enrichment-worker-composition/src/lib.rs",
    "tests": ROOT / "crates/crm-customer-enrichment-worker-composition/src/tests.rs",
    "process": ROOT / "crates/crm-customer-enrichment-worker-composition/tests/postgres_worker_process.rs",
}
texts = {name: path.read_text() for name, path in TARGETS.items()}


def replace_once(name: str, old: str, new: str) -> None:
    text = texts[name]
    count = text.count(old)
    if count != 1:
        raise RuntimeError(
            f"expected exactly one match in {TARGETS[name]}, found {count}: {old[:160]!r}"
        )
    texts[name] = text.replace(old, new, 1)


# Pure-core receipt reconciliation. The enum variant was introduced by the earlier guarded
# staging attempt; this atomic pass verifies it and completes the behavior and tests.
if texts["lifecycle"].count("    SemanticDuplicate,\n") != 1:
    raise RuntimeError("expected exactly one ReplayDisposition::SemanticDuplicate variant")
replace_once(
    "lifecycle",
    '''    pub fn reconcile(&self, candidate: &Self) -> Result<ReplayDisposition, SdkError> {
        if self.receipt_id != candidate.receipt_id {
            return Ok(ReplayDisposition::New);
        }
        if self == candidate {
            return Ok(ReplayDisposition::Duplicate);
        }
        Err(conflict(
            "CUSTOMER_ENRICHMENT_CONFLICTING_PROVIDER_REPLAY",
            "the same provider replay identity produced different canonical evidence",
        ))
    }
''',
    '''    pub fn reconcile(&self, candidate: &Self) -> Result<ReplayDisposition, SdkError> {
        if self.receipt_id != candidate.receipt_id {
            return Ok(ReplayDisposition::New);
        }
        if self == candidate {
            return Ok(ReplayDisposition::Duplicate);
        }
        if self.semantic_identity_matches(candidate) {
            return Ok(ReplayDisposition::SemanticDuplicate);
        }
        Err(conflict(
            "CUSTOMER_ENRICHMENT_CONFLICTING_PROVIDER_REPLAY",
            "the same provider replay identity produced conflicting canonical response evidence",
        ))
    }

    fn semantic_identity_matches(&self, candidate: &Self) -> bool {
        self.request_id == candidate.request_id
            && self.provider_profile_version_id == candidate.provider_profile_version_id
            && self.mapping_version_id == candidate.mapping_version_id
            && self.replay_key == candidate.replay_key
            && self.response_class == candidate.response_class
            && self.canonical_response_digest == candidate.canonical_response_digest
            && self.provider_observed_at_unix_ms == candidate.provider_observed_at_unix_ms
            && self.metered_units == candidate.metered_units
            && self.protected_evidence_reference == candidate.protected_evidence_reference
    }
''',
)
replace_once(
    "lifecycle",
    '''    fn response_replay_is_idempotent_and_conflicting_content_is_rejected() {
        let request = request();
        let first = receipt(&request, 7);
        let duplicate = receipt(&request, 7);
        let conflict = receipt(&request, 8);
        assert_eq!(
            first.reconcile(&duplicate).unwrap(),
            ReplayDisposition::Duplicate
        );
        assert_eq!(first.receipt_id(), conflict.receipt_id());
        assert!(first.reconcile(&conflict).is_err());
    }
''',
    '''    fn response_replay_distinguishes_exact_semantic_and_conflicting_evidence() {
        let request = request();
        let first = receipt(&request, 7);
        let duplicate = receipt(&request, 7);
        let mut semantic_duplicate = receipt(&request, 7);
        semantic_duplicate.provider_correlation_id =
            Some("provider-correlation-retry".to_owned());
        semantic_duplicate.retrieved_at_unix_ms = 201;
        let conflict = receipt(&request, 8);
        assert_eq!(
            first.reconcile(&duplicate).unwrap(),
            ReplayDisposition::Duplicate
        );
        assert_eq!(
            first.reconcile(&semantic_duplicate).unwrap(),
            ReplayDisposition::SemanticDuplicate
        );
        assert_eq!(first.receipt_id(), conflict.receipt_id());
        let error = first.reconcile(&conflict).unwrap_err();
        assert_eq!(
            error.code,
            "CUSTOMER_ENRICHMENT_CONFLICTING_PROVIDER_REPLAY"
        );
    }
''',
)

# Worker response request semantic fingerprint and explicit reconciliation result.
replace_once("worker", "use crm_capability_ingress::semantic_input_hash;\n", "")
replace_once(
    "worker",
    '''const RESPONSE_IDENTITY_DOMAIN: &[u8] = b"crm.customer-enrichment.response-worker/v1";
const MAX_INTERNAL_KEY_BYTES: usize = 180;
''',
    '''const RESPONSE_IDENTITY_DOMAIN: &[u8] = b"crm.customer-enrichment.response-worker/v1";
const RESPONSE_SEMANTIC_HASH_DOMAIN: &[u8] =
    b"crm.customer-enrichment.response-worker.semantic-request/v1";
const MAX_INTERNAL_KEY_BYTES: usize = 180;
''',
)
replace_once(
    "worker",
    '''/// Durable worker outcome after the provider response has been atomically recorded.
#[derive(Debug, Clone, PartialEq)]
pub struct ProviderDispatchWorkerResult {
    pub dispatch_replayed: bool,
    pub response_replayed: bool,
    pub response: wire::RecordProviderResponseResponse,
}
''',
    '''/// Deterministic reconciliation result for one provider replay identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderResponseReconciliation {
    New,
    ExactDuplicate,
    SemanticDuplicate,
}

/// Durable worker outcome after the provider response has been atomically recorded.
#[derive(Debug, Clone, PartialEq)]
pub struct ProviderDispatchWorkerResult {
    pub dispatch_replayed: bool,
    pub response_replayed: bool,
    pub response_reconciliation: ProviderResponseReconciliation,
    pub response: wire::RecordProviderResponseResponse,
}
''',
)
replace_once(
    "worker",
    '''        let response_result = self
            .response_executor
            .execute(&self.response_definition, response_request)
            .await?;
''',
    '''        let response_result = self
            .response_executor
            .execute(&self.response_definition, response_request)
            .await
            .map_err(response_reconciliation_error)?;
''',
)
replace_once(
    "worker",
    '''        validate_response_output(&response, &item.provider_request, &sanitized)?;

        Ok(ProviderDispatchWorkerResult {
            dispatch_replayed: dispatch_result.replayed,
            response_replayed: response_result.replayed,
            response,
        })
''',
    '''        validate_response_output(
            &response,
            &item.provider_request,
            &sanitized,
            response_result.replayed,
        )?;
        let response_reconciliation = classify_response_reconciliation(
            &response,
            &sanitized,
            response_result.replayed,
        )?;

        Ok(ProviderDispatchWorkerResult {
            dispatch_replayed: dispatch_result.replayed,
            response_replayed: response_result.replayed,
            response_reconciliation,
            response,
        })
''',
)
replace_once(
    "worker",
    '''    let input_hash = semantic_input_hash(&input);
    Ok(CapabilityRequest {
''',
    '''    let input_hash = response_semantic_input_hash(provider_request, response);
    Ok(CapabilityRequest {
''',
)
replace_once(
    "worker",
    '''fn validate_response_output(
    response: &wire::RecordProviderResponseResponse,
    provider_request: &ProviderDispatchRequest,
    sanitized: &SanitizedProviderResponse,
) -> Result<(), SdkError> {
''',
    '''fn validate_response_output(
    response: &wire::RecordProviderResponseResponse,
    provider_request: &ProviderDispatchRequest,
    sanitized: &SanitizedProviderResponse,
    replayed: bool,
) -> Result<(), SdkError> {
''',
)
replace_once(
    "worker",
    '''    if request_ref.enrichment_request_id != provider_request.enrichment_request_id.as_str()
        || request.status != wire::EnrichmentRequestStatus::ResponseRecorded as i32
        || request.retry_generation != provider_request.retry_generation
        || receipt_request_ref.enrichment_request_id
            != provider_request.enrichment_request_id.as_str()
        || receipt.replay_key != provider_request.provider_idempotency_key
        || receipt.response_class != provider_response_class_to_wire(sanitized.response_class)
        || receipt.canonical_response_digest != sanitized.canonical_response_digest
    {
''',
    '''    let response_usage = response_received_usage(response)?;
    if request_ref.enrichment_request_id != provider_request.enrichment_request_id.as_str()
        || request.status != wire::EnrichmentRequestStatus::ResponseRecorded as i32
        || request.retry_generation != provider_request.retry_generation
        || receipt_request_ref.enrichment_request_id
            != provider_request.enrichment_request_id.as_str()
        || receipt.replay_key != provider_request.provider_idempotency_key
        || receipt.response_class != provider_response_class_to_wire(sanitized.response_class)
        || receipt.canonical_response_digest != sanitized.canonical_response_digest
        || receipt.provider_observed_at_unix_ms != sanitized.provider_observed_at_unix_ms
        || receipt.metered_units != sanitized.metered_units
        || receipt.protected_evidence_reference != sanitized.protected_evidence_reference
        || response_usage.safe_provider_code != sanitized.safe_provider_code
        || (!replayed
            && (receipt.provider_correlation_id != sanitized.provider_correlation_id
                || receipt.retrieved_at_unix_ms != sanitized.retrieved_at_unix_ms))
    {
''',
)
replace_once(
    "worker",
    '''fn provider_response_class_to_wire(value: ProviderResponseClass) -> i32 {
''',
    r'''fn response_semantic_input_hash(
    request: &ProviderDispatchRequest,
    response: &SanitizedProviderResponse,
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hash_frame(&mut hasher, RESPONSE_SEMANTIC_HASH_DOMAIN);
    hash_frame(&mut hasher, request.tenant_id.as_str().as_bytes());
    hash_frame(&mut hasher, request.actor_id.as_str().as_bytes());
    hash_frame(
        &mut hasher,
        request.enrichment_request_id.as_str().as_bytes(),
    );
    hash_frame(
        &mut hasher,
        request.provider_profile_version_id.as_str().as_bytes(),
    );
    hash_frame(
        &mut hasher,
        request.mapping_version_id.as_str().as_bytes(),
    );
    hash_frame(
        &mut hasher,
        request.adapter_coordinate.adapter_kind().as_bytes(),
    );
    hash_frame(
        &mut hasher,
        request
            .adapter_coordinate
            .adapter_contract_version()
            .as_bytes(),
    );
    hash_frame(&mut hasher, &request.retry_generation.to_be_bytes());
    hash_frame(&mut hasher, request.party_id.as_str().as_bytes());
    hash_frame(
        &mut hasher,
        &request.party_resource_version.to_be_bytes(),
    );
    hash_frame(&mut hasher, response.replay_key.as_bytes());
    hash_frame(
        &mut hasher,
        &[provider_response_class_tag(response.response_class)],
    );
    hash_frame(&mut hasher, &response.canonical_response_digest);
    hash_optional_i64(&mut hasher, response.provider_observed_at_unix_ms);
    hash_frame(&mut hasher, &response.metered_units.to_be_bytes());
    hash_optional_text(
        &mut hasher,
        response.protected_evidence_reference.as_deref(),
    );
    hash_optional_text(&mut hasher, response.safe_provider_code.as_deref());
    hasher.finalize().into()
}

fn provider_response_class_tag(value: ProviderResponseClass) -> u8 {
    match value {
        ProviderResponseClass::Success => 1,
        ProviderResponseClass::NoMatch => 2,
        ProviderResponseClass::RetryableFailure => 3,
        ProviderResponseClass::TerminalFailure => 4,
    }
}

fn hash_optional_i64(hasher: &mut Sha256, value: Option<i64>) {
    match value {
        Some(value) => {
            hash_frame(hasher, &[1]);
            hash_frame(hasher, &value.to_be_bytes());
        }
        None => hash_frame(hasher, &[0]),
    }
}

fn hash_optional_text(hasher: &mut Sha256, value: Option<&str>) {
    match value {
        Some(value) => {
            hash_frame(hasher, &[1]);
            hash_frame(hasher, value.as_bytes());
        }
        None => hash_frame(hasher, &[0]),
    }
}

fn response_received_usage(
    response: &wire::RecordProviderResponseResponse,
) -> Result<&wire::ProviderUsageEntry, SdkError> {
    let mut matching = response
        .provider_usage_entries
        .iter()
        .filter(|entry| entry.kind == wire::ProviderUsageKind::ResponseReceived as i32);
    let usage = matching.next().ok_or_else(|| {
        response_output_invalid("response output is missing ResponseReceived usage evidence")
    })?;
    if matching.next().is_some() {
        return Err(response_output_invalid(
            "response output contains duplicate ResponseReceived usage evidence",
        ));
    }
    Ok(usage)
}

fn classify_response_reconciliation(
    response: &wire::RecordProviderResponseResponse,
    sanitized: &SanitizedProviderResponse,
    replayed: bool,
) -> Result<ProviderResponseReconciliation, SdkError> {
    if !replayed {
        return Ok(ProviderResponseReconciliation::New);
    }
    let receipt = response
        .provider_response_receipt
        .as_ref()
        .ok_or_else(|| response_output_invalid("response output is missing receipt evidence"))?;
    let response_usage = response_received_usage(response)?;
    if receipt.provider_correlation_id == sanitized.provider_correlation_id
        && receipt.provider_observed_at_unix_ms == sanitized.provider_observed_at_unix_ms
        && receipt.retrieved_at_unix_ms == sanitized.retrieved_at_unix_ms
        && receipt.metered_units == sanitized.metered_units
        && receipt.protected_evidence_reference == sanitized.protected_evidence_reference
        && response_usage.safe_provider_code == sanitized.safe_provider_code
    {
        Ok(ProviderResponseReconciliation::ExactDuplicate)
    } else {
        Ok(ProviderResponseReconciliation::SemanticDuplicate)
    }
}

fn provider_response_class_to_wire(value: ProviderResponseClass) -> i32 {
''',
)
replace_once(
    "worker",
    '''fn response_output_invalid(reference: impl Into<String>) -> SdkError {
''',
    '''fn response_reconciliation_error(error: SdkError) -> SdkError {
    if error.code == "DATA_CONFLICT" {
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

fn response_output_invalid(reference: impl Into<String>) -> SdkError {
''',
)

# Focused unit coverage.
replace_once(
    "tests",
    "use super::*;\n",
    "use super::*;\nuse crm_capability_ingress::semantic_input_hash;\n",
)
replace_once(
    "tests",
    '''    assert!(!result.dispatch_replayed);
    assert!(!result.response_replayed);
''',
    '''    assert!(!result.dispatch_replayed);
    assert!(!result.response_replayed);
    assert_eq!(
        result.response_reconciliation,
        ProviderResponseReconciliation::New
    );
''',
)
replace_once(
    "tests",
    '''#[tokio::test]
async fn repeated_work_item_builds_identical_response_commit_identity() {
''',
    '''#[test]
fn response_semantic_hash_ignores_only_volatile_transport_metadata() {
    let fixture = fixture();
    let definition = provider_response_capability_definition().unwrap();
    let first = build_response_request(
        &definition,
        &fixture.item.dispatch_request,
        &fixture.item.provider_request,
        &fixture.response,
    )
    .unwrap();
    let mut semantic_duplicate = fixture.response.clone();
    semantic_duplicate.provider_correlation_id = Some("provider-correlation-2".to_owned());
    semantic_duplicate.retrieved_at_unix_ms += 1;
    let semantic = build_response_request(
        &definition,
        &fixture.item.dispatch_request,
        &fixture.item.provider_request,
        &semantic_duplicate,
    )
    .unwrap();
    assert_ne!(first.input, semantic.input);
    assert_eq!(first.input_hash, semantic.input_hash);

    let mut conflict = semantic_duplicate;
    conflict.canonical_response_digest = [10; 32];
    let conflicting = build_response_request(
        &definition,
        &fixture.item.dispatch_request,
        &fixture.item.provider_request,
        &conflict,
    )
    .unwrap();
    assert_ne!(first.input_hash, conflicting.input_hash);
}

#[tokio::test]
async fn repeated_work_item_builds_identical_response_commit_identity() {
''',
)
replace_once(
    "tests",
    '''            provider_response_receipt: Some(receipt),
            provider_usage_entries: Vec::new(),
''',
    '''            provider_response_receipt: Some(receipt),
            provider_usage_entries: vec![wire::ProviderUsageEntry {
                provider_usage_entry_ref: Some(wire::ProviderUsageEntryRef {
                    provider_usage_entry_id: "provider-usage-response-1".to_owned(),
                }),
                enrichment_request_ref: command.enrichment_request_ref.clone(),
                provider_response_receipt_ref: Some(wire::ProviderResponseReceiptRef {
                    provider_response_receipt_id: "provider-receipt-1".to_owned(),
                }),
                provider_profile_version_ref: Some(wire::ProviderProfileVersionRef {
                    provider_profile_version_id: provider
                        .provider_profile_version_id
                        .as_str()
                        .to_owned(),
                }),
                kind: wire::ProviderUsageKind::ResponseReceived as i32,
                metered_units: command.metered_units,
                quota_bucket: None,
                quota_remaining: None,
                provider_observed_at_unix_ms: command.provider_observed_at_unix_ms,
                recorded_at_unix_ms: command.retrieved_at_unix_ms,
                safe_provider_code: command.safe_provider_code.clone(),
            }],
''',
)

# Fresh PostgreSQL process matrix through the concrete HTTP transport.
replace_once(
    "process",
    '''use crm_customer_enrichment_worker_composition::{
    CustomerEnrichmentProviderWorker, ProviderDispatchWorkItem,
};
''',
    '''use crm_customer_enrichment_worker_composition::{
    CustomerEnrichmentProviderWorker, ProviderDispatchWorkItem, ProviderResponseReconciliation,
};
''',
)
replace_once(
    "process",
    '''    state.calls.fetch_add(1, Ordering::SeqCst);
''',
    '''    let call = state.calls.fetch_add(1, Ordering::SeqCst) + 1;
''',
)
replace_once(
    "process",
    '''    Json(json!({
        "schema_version": PROVIDER_RESPONSE_SCHEMA,
        "replay_key": state.expected_key,
        "provider_correlation_id": "provider-correlation-process-1",
        "response_class": "success",
''',
    '''    let response_class = if call >= 4 { "no_match" } else { "success" };
    Json(json!({
        "schema_version": PROVIDER_RESPONSE_SCHEMA,
        "replay_key": state.expected_key,
        "provider_correlation_id": "provider-correlation-process-1",
        "response_class": response_class,
''',
)
replace_once(
    "process",
    '''            ConsecutiveFailureProviderCircuitBreaker::try_new(3, 60_000_000_000, clock)
                .expect("build provider circuit"),
''',
    '''            ConsecutiveFailureProviderCircuitBreaker::try_new(
                3,
                60_000_000_000,
                clock.clone(),
            )
            .expect("build provider circuit"),
''',
)
replace_once(
    "process",
    '''    assert!(!first.dispatch_replayed);
    assert!(!first.response_replayed);

    let second = worker
''',
    '''    assert!(!first.dispatch_replayed);
    assert!(!first.response_replayed);
    assert_eq!(
        first.response_reconciliation,
        ProviderResponseReconciliation::New
    );

    let second = worker
''',
)
replace_once(
    "process",
    '''    assert!(second.dispatch_replayed);
    assert!(second.response_replayed);
    assert_eq!(calls.load(Ordering::SeqCst), 2);

    let request_snapshot = store
''',
    '''    assert!(second.dispatch_replayed);
    assert!(second.response_replayed);
    assert_eq!(
        second.response_reconciliation,
        ProviderResponseReconciliation::ExactDuplicate
    );

    clock.advance(1_000_000);
    let semantic = worker
        .execute(fixture.work_item.clone())
        .await
        .expect("reconcile changed retrieval metadata to the first receipt");
    assert!(semantic.dispatch_replayed);
    assert!(semantic.response_replayed);
    assert_eq!(
        semantic.response_reconciliation,
        ProviderResponseReconciliation::SemanticDuplicate
    );

    let conflict = worker
        .execute(fixture.work_item.clone())
        .await
        .expect_err("reject conflicting canonical provider response");
    assert_eq!(
        conflict.code,
        "CUSTOMER_ENRICHMENT_CONFLICTING_PROVIDER_REPLAY"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 4);

    let request_snapshot = store
''',
)

# Only now, after every replacement has been validated, write all four files.
for name, path in TARGETS.items():
    path.write_text(texts[name])
