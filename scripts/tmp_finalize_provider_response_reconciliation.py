from pathlib import Path

lifecycle_path = Path("modules/crm-customer-enrichment/src/lifecycle.rs")
worker_path = Path("crates/crm-customer-enrichment-worker-composition/src/lib.rs")

lifecycle = lifecycle_path.read_text()
worker = worker_path.read_text()


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise RuntimeError(f"expected exactly one {label}, found {count}")
    return text.replace(old, new, 1)


lifecycle = replace_once(
    lifecycle,
    '''#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplayDisposition {
    New,
    Duplicate,
    SemanticDuplicate,
}
''',
    '''#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplayDisposition {
    New,
    Duplicate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderResponseReplayDisposition {
    New,
    ExactDuplicate,
    SemanticDuplicate,
}
''',
    "shared replay disposition block",
)

lifecycle = replace_once(
    lifecycle,
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
''',
    '''    pub fn reconcile(
        &self,
        candidate: &Self,
    ) -> Result<ProviderResponseReplayDisposition, SdkError> {
        if self.receipt_id != candidate.receipt_id {
            return Ok(ProviderResponseReplayDisposition::New);
        }
        if self == candidate {
            return Ok(ProviderResponseReplayDisposition::ExactDuplicate);
        }
        if self.semantic_identity_matches(candidate) {
            return Ok(ProviderResponseReplayDisposition::SemanticDuplicate);
        }
        Err(conflict(
            "CUSTOMER_ENRICHMENT_CONFLICTING_PROVIDER_REPLAY",
            "the same provider replay identity produced conflicting canonical response evidence",
        ))
    }
''',
    "provider receipt reconcile function",
)

lifecycle = replace_once(
    lifecycle,
    '''        assert_eq!(
            first.reconcile(&duplicate).unwrap(),
            ReplayDisposition::Duplicate
        );
        assert_eq!(
            first.reconcile(&semantic_duplicate).unwrap(),
            ReplayDisposition::SemanticDuplicate
        );
''',
    '''        assert_eq!(
            first.reconcile(&duplicate).unwrap(),
            ProviderResponseReplayDisposition::ExactDuplicate
        );
        assert_eq!(
            first.reconcile(&semantic_duplicate).unwrap(),
            ProviderResponseReplayDisposition::SemanticDuplicate
        );
''',
    "provider receipt replay assertions",
)

worker = replace_once(
    worker,
    '''    if error.code == "DATA_CONFLICT" {
''',
    '''    if matches!(
        error.code.as_str(),
        "DATA_CONFLICT" | "CAPABILITY_IDEMPOTENCY_KEY_REUSED"
    ) {
''',
    "response executor conflict mapping",
)

lifecycle_path.write_text(lifecycle)
worker_path.write_text(worker)
