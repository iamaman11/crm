use crate::{
    ApprovalRequirement, EnrichmentRequestId, MappingVersionId, ProviderProfileVersionId,
    ReviewDecisionKind, SuggestionId, TargetField,
};
use crm_module_sdk::{ActorId, PortFuture, RecordId, SdkError, TenantId};

/// Exact resource-specific review-policy input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuggestionReviewPolicyRequest {
    pub tenant_id: TenantId,
    pub actor_id: ActorId,
    pub request_identity: String,
    pub enrichment_request_id: EnrichmentRequestId,
    pub suggestion_id: SuggestionId,
    pub party_id: RecordId,
    pub party_resource_version: i64,
    pub target_field: TargetField,
    pub provider_profile_version_id: ProviderProfileVersionId,
    pub mapping_version_id: MappingVersionId,
    pub purpose_code: String,
    pub legal_basis_code: String,
    pub consent_evidence_reference: Option<String>,
    pub proposed_value_digest: [u8; 32],
    pub decision_kind: ReviewDecisionKind,
    pub evaluated_at_unix_ms: i64,
}

/// Versioned final authorization outcome for one exact suggestion review.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SuggestionReviewPolicyDecision {
    Allowed {
        decision_id: String,
        policy_version: String,
        acceptance_approval_requirement: ApprovalRequirement,
    },
    Denied {
        decision_id: String,
        policy_version: String,
        safe_reason_code: String,
    },
}

impl SuggestionReviewPolicyDecision {
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

    pub const fn acceptance_approval_requirement(&self) -> Option<ApprovalRequirement> {
        match self {
            Self::Allowed {
                acceptance_approval_requirement,
                ..
            } => Some(*acceptance_approval_requirement),
            Self::Denied { .. } => None,
        }
    }
}

/// Infrastructure-owned, versioned final authorization boundary for suggestion review.
///
/// Implementations must evaluate the exact immutable suggestion binding and fail closed. They may
/// combine purpose/legal-basis policy, consent state, resource authorization and approval policy,
/// but must not replace the target Party or weaken the supplied optimistic bindings.
pub trait SuggestionReviewPolicyPort: Send + Sync {
    fn evaluate<'a>(
        &'a self,
        request: SuggestionReviewPolicyRequest,
    ) -> PortFuture<'a, Result<SuggestionReviewPolicyDecision, SdkError>>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn policy_outcome_exposes_only_closed_evidence() {
        let allowed = SuggestionReviewPolicyDecision::Allowed {
            decision_id: "decision-a".to_owned(),
            policy_version: "review-policy-v1".to_owned(),
            acceptance_approval_requirement: ApprovalRequirement::Required,
        };
        assert_eq!(allowed.decision_id(), "decision-a");
        assert_eq!(allowed.policy_version(), "review-policy-v1");
        assert_eq!(
            allowed.acceptance_approval_requirement(),
            Some(ApprovalRequirement::Required)
        );

        let denied = SuggestionReviewPolicyDecision::Denied {
            decision_id: "decision-b".to_owned(),
            policy_version: "review-policy-v2".to_owned(),
            safe_reason_code: "review_not_permitted".to_owned(),
        };
        assert_eq!(denied.decision_id(), "decision-b");
        assert_eq!(denied.policy_version(), "review-policy-v2");
        assert_eq!(denied.acceptance_approval_requirement(), None);
    }

    fn assert_send_sync<T: Send + Sync>() {}

    #[test]
    fn policy_contract_is_thread_safe() {
        assert_send_sync::<SuggestionReviewPolicyRequest>();
        assert_send_sync::<SuggestionReviewPolicyDecision>();
    }
}
