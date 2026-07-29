use reqwest::StatusCode;
use serde_json::Value;
use std::collections::BTreeSet;
use std::fmt::Debug;
use tonic::{Code, Status};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EvidenceSnapshot {
    pub records: i64,
    pub relationships: i64,
    pub events: i64,
    pub audits: i64,
    pub idempotency: i64,
    pub transactions: i64,
}

impl EvidenceSnapshot {
    fn one_atomic_mutation_after(self) -> Self {
        Self {
            records: self.records + 1,
            relationships: self.relationships + 1,
            events: self.events + 1,
            audits: self.audits + 1,
            idempotency: self.idempotency + 1,
            transactions: self.transactions + 1,
        }
    }
}

#[derive(Debug, Clone)]
struct SafeSurface {
    forbidden: Vec<String>,
}

impl SafeSurface {
    fn new<I, S>(forbidden: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            forbidden: forbidden.into_iter().map(Into::into).collect(),
        }
    }

    fn assert_status(
        &self,
        status: &Status,
        expected_code: Code,
        expected_error_code: &str,
        retryable: bool,
    ) {
        assert_eq!(status.code(), expected_code);
        assert_eq!(
            status
                .metadata()
                .get("x-error-code")
                .expect("typed gRPC error code")
                .to_str()
                .expect("ASCII gRPC error code"),
            expected_error_code
        );
        assert_eq!(
            status
                .metadata()
                .get("x-error-retryable")
                .expect("retryability metadata")
                .to_str()
                .expect("ASCII retryability metadata"),
            retryable.to_string()
        );
        self.assert_text(status.message());
        self.assert_text(&format!("{:?}", status.metadata()));
    }

    fn assert_text(&self, value: &str) {
        for forbidden in &self.forbidden {
            assert!(
                !value.contains(forbidden),
                "safe conformance surface leaked {forbidden}: {value}"
            );
        }
    }
}

#[derive(Debug, Clone)]
pub struct MutationConformanceSuite {
    safe_surface: SafeSurface,
}

impl MutationConformanceSuite {
    pub fn new<I, S>(forbidden: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            safe_surface: SafeSurface::new(forbidden),
        }
    }

    pub fn assert_unauthenticated_http(
        &self,
        status: StatusCode,
        body: &Value,
        before: EvidenceSnapshot,
        after: EvidenceSnapshot,
    ) {
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body, &serde_json::json!({"error": "request_failed"}));
        self.safe_surface.assert_text(&body.to_string());
        self.assert_no_side_effects(before, after);
    }

    pub fn assert_denied(
        &self,
        status: &Status,
        expected_code: Code,
        expected_error_code: &str,
        retryable: bool,
        before: EvidenceSnapshot,
        after: EvidenceSnapshot,
    ) {
        self.safe_surface
            .assert_status(status, expected_code, expected_error_code, retryable);
        self.assert_no_side_effects(before, after);
    }

    pub fn assert_atomic_commit(&self, before: EvidenceSnapshot, after: EvidenceSnapshot) {
        assert_eq!(after, before.one_atomic_mutation_after());
    }

    pub fn assert_exact_replay<T: PartialEq + Debug + ?Sized>(
        &self,
        first_output: &T,
        replay_output: &T,
        committed: EvidenceSnapshot,
        after_replay: EvidenceSnapshot,
    ) {
        assert_eq!(replay_output, first_output);
        self.assert_no_side_effects(committed, after_replay);
    }

    pub fn assert_no_side_effects(&self, before: EvidenceSnapshot, after: EvidenceSnapshot) {
        assert_eq!(after, before);
    }
}

#[derive(Debug, Clone)]
pub struct QueryConformanceSuite {
    safe_surface: SafeSurface,
}

impl QueryConformanceSuite {
    pub fn new<I, S>(forbidden: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            safe_surface: SafeSurface::new(forbidden),
        }
    }

    pub fn assert_denied(
        &self,
        status: &Status,
        expected_code: Code,
        expected_error_code: &str,
        retryable: bool,
        before: EvidenceSnapshot,
        after: EvidenceSnapshot,
    ) {
        self.safe_surface
            .assert_status(status, expected_code, expected_error_code, retryable);
        self.assert_no_writes(before, after);
    }

    pub fn assert_keyset_pages(
        &self,
        first_ids: &[String],
        first_cursor: &str,
        second_ids: &[String],
        second_cursor: &str,
        expected_ids: &BTreeSet<String>,
        before: EvidenceSnapshot,
        after: EvidenceSnapshot,
    ) {
        assert_eq!(first_ids.len(), 1);
        assert_eq!(second_ids.len(), 1);
        assert!(!first_cursor.is_empty());
        assert!(second_cursor.is_empty());
        let actual_ids = first_ids
            .iter()
            .chain(second_ids)
            .cloned()
            .collect::<BTreeSet<_>>();
        assert_eq!(&actual_ids, expected_ids);
        self.assert_no_writes(before, after);
    }

    pub fn assert_not_found_concealed(
        &self,
        result_count: usize,
        cursor: &str,
        before: EvidenceSnapshot,
        after: EvidenceSnapshot,
    ) {
        assert_eq!(result_count, 0);
        assert!(cursor.is_empty());
        self.assert_no_writes(before, after);
    }

    pub fn assert_no_writes(&self, before: EvidenceSnapshot, after: EvidenceSnapshot) {
        assert_eq!(after, before);
    }
}
