use crate::{
    ApplicationAttemptId, EnrichmentRequestId, MappingVersionId, ProviderProfileVersionId,
    ProviderResponseClass, ReviewDecisionId, SuggestionId, TargetField,
};
use crm_module_sdk::{ActorId, PortFuture, RecordId, SdkError, TenantId};

/// Exact authoritative Party mutation coordinate for the first enrichment slice.
pub const PARTY_DISPLAY_NAME_UPDATE_CAPABILITY_ID: &str = "parties.party.update";
/// Exact authoritative Party mutation version for the first enrichment slice.
pub const PARTY_DISPLAY_NAME_UPDATE_CAPABILITY_VERSION: &str = "1.0.0";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartySnapshotRequest {
    pub tenant_id: TenantId,
    pub actor_id: ActorId,
    pub request_identity: String,
    pub party_id: RecordId,
    pub requested_at_unix_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartySnapshot {
    pub party_id: RecordId,
    pub display_name: String,
    pub resource_version: i64,
    pub observed_at_unix_ms: i64,
}

/// Governed minimized read of authoritative Party state.
pub trait PartySnapshotPort: Send + Sync {
    fn get<'a>(
        &'a self,
        request: PartySnapshotRequest,
    ) -> PortFuture<'a, Result<PartySnapshot, SdkError>>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyEvaluationPhase {
    RequestCreation,
    ProviderDispatch,
    ProtectedEvidenceDisclosure,
    SuggestionReview,
    OwnerApplication,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnrichmentPolicyRequest {
    pub tenant_id: TenantId,
    pub actor_id: ActorId,
    pub request_identity: String,
    pub enrichment_request_id: EnrichmentRequestId,
    pub party_id: RecordId,
    pub target_field: TargetField,
    pub provider_profile_version_id: ProviderProfileVersionId,
    pub mapping_version_id: MappingVersionId,
    pub purpose_code: String,
    pub legal_basis_code: String,
    pub consent_evidence_reference: Option<String>,
    pub phase: PolicyEvaluationPhase,
    pub evaluated_at_unix_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnrichmentPolicyDecision {
    Allowed {
        decision_id: String,
        policy_version: String,
    },
    Denied {
        decision_id: String,
        policy_version: String,
        safe_reason_code: String,
    },
}

impl EnrichmentPolicyDecision {
    pub fn is_allowed(&self) -> bool {
        matches!(self, Self::Allowed { .. })
    }

    pub fn decision_id(&self) -> &str {
        match self {
            Self::Allowed { decision_id, .. } | Self::Denied { decision_id, .. } => decision_id,
        }
    }

    pub fn policy_version(&self) -> &str {
        match self {
            Self::Allowed { policy_version, .. } | Self::Denied { policy_version, .. } => {
                policy_version
            }
        }
    }
}

/// Versioned purpose, consent, licensing and permitted-use evaluation.
pub trait EnrichmentPolicyPort: Send + Sync {
    fn evaluate<'a>(
        &'a self,
        request: EnrichmentPolicyRequest,
    ) -> PortFuture<'a, Result<EnrichmentPolicyDecision, SdkError>>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderDispatchRequest {
    pub tenant_id: TenantId,
    pub actor_id: ActorId,
    pub enrichment_request_id: EnrichmentRequestId,
    pub provider_profile_version_id: ProviderProfileVersionId,
    pub mapping_version_id: MappingVersionId,
    pub party_id: RecordId,
    pub party_resource_version: i64,
    pub current_display_name: String,
    pub provider_idempotency_key: String,
    pub credential_handle_aliases: Vec<String>,
    pub deadline_at_unix_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SanitizedProviderResponse {
    pub replay_key: String,
    pub provider_correlation_id: Option<String>,
    pub response_class: ProviderResponseClass,
    pub canonical_response_digest: [u8; 32],
    pub provider_observed_at_unix_ms: Option<i64>,
    pub retrieved_at_unix_ms: i64,
    pub metered_units: u64,
    pub protected_evidence_reference: Option<String>,
}

/// Infrastructure-owned provider dispatch boundary.
///
/// Implementations resolve credentials, perform network I/O and sanitize provider errors. Raw
/// credentials and raw provider payloads never cross this boundary into the pure module core.
pub trait ProviderDispatchPort: Send + Sync {
    fn dispatch<'a>(
        &'a self,
        request: ProviderDispatchRequest,
    ) -> PortFuture<'a, Result<SanitizedProviderResponse, SdkError>>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartyDisplayNameApplicationRequest {
    pub tenant_id: TenantId,
    pub actor_id: ActorId,
    pub suggestion_id: SuggestionId,
    pub review_decision_id: ReviewDecisionId,
    pub application_attempt_id: ApplicationAttemptId,
    pub party_id: RecordId,
    pub expected_party_resource_version: i64,
    pub reviewed_display_name: String,
    pub target_idempotency_key: String,
    pub final_authorization_decision_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PartyDisplayNameApplicationResult {
    Applied {
        business_transaction_id: String,
        resulting_party_resource_version: i64,
    },
    StaleTarget {
        actual_party_resource_version: i64,
    },
    AuthorizationDenied {
        decision_id: String,
    },
    RetryableFailure {
        safe_code: String,
    },
    TerminalFailure {
        safe_code: String,
    },
}

/// Exact owner-capability invocation boundary for `parties.party.update@1.0.0`.
pub trait PartyDisplayNameApplicationPort: Send + Sync {
    fn apply<'a>(
        &'a self,
        request: PartyDisplayNameApplicationRequest,
    ) -> PortFuture<'a, Result<PartyDisplayNameApplicationResult, SdkError>>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn owner_capability_coordinate_is_exact() {
        assert_eq!(
            PARTY_DISPLAY_NAME_UPDATE_CAPABILITY_ID,
            "parties.party.update"
        );
        assert_eq!(PARTY_DISPLAY_NAME_UPDATE_CAPABILITY_VERSION, "1.0.0");
    }

    #[test]
    fn policy_decision_accessors_are_closed() {
        let allowed = EnrichmentPolicyDecision::Allowed {
            decision_id: "decision-1".to_owned(),
            policy_version: "policy-v1".to_owned(),
        };
        assert!(allowed.is_allowed());
        assert_eq!(allowed.decision_id(), "decision-1");
        assert_eq!(allowed.policy_version(), "policy-v1");

        let denied = EnrichmentPolicyDecision::Denied {
            decision_id: "decision-2".to_owned(),
            policy_version: "policy-v2".to_owned(),
            safe_reason_code: "consent_required".to_owned(),
        };
        assert!(!denied.is_allowed());
        assert_eq!(denied.decision_id(), "decision-2");
        assert_eq!(denied.policy_version(), "policy-v2");
    }

    fn assert_send_sync<T: Send + Sync>() {}

    #[test]
    fn port_contracts_are_thread_safe() {
        assert_send_sync::<PartySnapshotRequest>();
        assert_send_sync::<EnrichmentPolicyRequest>();
        assert_send_sync::<ProviderDispatchRequest>();
        assert_send_sync::<PartyDisplayNameApplicationRequest>();
    }
}
