use crate::{
    CommunicationChannel, ConsentAuthorization, ConsentAuthorizationId, ConsentDecisionPointEffect,
    ContactPointReference, PartyReference, PurposeCode,
};
use crm_module_sdk::{ErrorCategory, SdkError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvaluateCommunicationAuthorization {
    pub party_ref: PartyReference,
    pub contact_point_ref: Option<ContactPointReference>,
    pub purpose: PurposeCode,
    pub channel: CommunicationChannel,
    pub evaluation_time_unix_nanos: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommunicationAuthorizationReason {
    ActiveGrant,
    ActiveDeny,
    Withdrawn,
    NoApplicableGrant,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommunicationAuthorizationDecision {
    pub allowed: bool,
    pub reason: CommunicationAuthorizationReason,
    pub determining_authorization_ids: Vec<ConsentAuthorizationId>,
}

pub fn evaluate_communication_authorization<'a>(
    command: &EvaluateCommunicationAuthorization,
    authorizations: impl IntoIterator<Item = &'a ConsentAuthorization>,
) -> Result<CommunicationAuthorizationDecision, SdkError> {
    if command.evaluation_time_unix_nanos <= 0 {
        return Err(SdkError::new(
            "CONSENTS_COMMUNICATION_EVALUATION_TIME_INVALID",
            ErrorCategory::InvalidArgument,
            false,
            "The communication authorization evaluation time is invalid.",
        ));
    }

    let mut latest_time = None;
    let mut latest_points = Vec::new();

    for authorization in authorizations {
        if !scope_matches(command, authorization) {
            continue;
        }
        let Some(point) = authorization.decision_point_at(command.evaluation_time_unix_nanos)
        else {
            continue;
        };

        match latest_time {
            None => {
                latest_time = Some(point.occurred_at_unix_nanos);
                latest_points.push((authorization.authorization_id().clone(), point.effect));
            }
            Some(current_latest) if point.occurred_at_unix_nanos > current_latest => {
                latest_time = Some(point.occurred_at_unix_nanos);
                latest_points.clear();
                latest_points.push((authorization.authorization_id().clone(), point.effect));
            }
            Some(current_latest) if point.occurred_at_unix_nanos == current_latest => {
                latest_points.push((authorization.authorization_id().clone(), point.effect));
            }
            Some(_) => {}
        }
    }

    if latest_points.is_empty() {
        return Ok(CommunicationAuthorizationDecision {
            allowed: false,
            reason: CommunicationAuthorizationReason::NoApplicableGrant,
            determining_authorization_ids: Vec::new(),
        });
    }

    latest_points.sort_by(|left, right| left.0.as_str().cmp(right.0.as_str()));
    let reason = if latest_points
        .iter()
        .any(|(_, effect)| *effect == ConsentDecisionPointEffect::Withdrawal)
    {
        CommunicationAuthorizationReason::Withdrawn
    } else if latest_points
        .iter()
        .any(|(_, effect)| *effect == ConsentDecisionPointEffect::Deny)
    {
        CommunicationAuthorizationReason::ActiveDeny
    } else {
        CommunicationAuthorizationReason::ActiveGrant
    };

    Ok(CommunicationAuthorizationDecision {
        allowed: reason == CommunicationAuthorizationReason::ActiveGrant,
        reason,
        determining_authorization_ids: latest_points
            .into_iter()
            .map(|(authorization_id, _)| authorization_id)
            .collect(),
    })
}

fn scope_matches(
    command: &EvaluateCommunicationAuthorization,
    authorization: &ConsentAuthorization,
) -> bool {
    if authorization.party_ref() != &command.party_ref
        || authorization.purpose() != &command.purpose
        || authorization.channel() != command.channel
    {
        return false;
    }

    match authorization.contact_point_ref() {
        None => true,
        Some(assertion_contact_point) => {
            command
                .contact_point_ref
                .as_ref()
                .is_some_and(|requested_contact_point| {
                    requested_contact_point == assertion_contact_point
                })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ConsentEffect, CreateConsentAuthorization, EvidenceReference, JurisdictionCode,
        LegalBasisCode, SourceCode, WithdrawConsentAuthorization,
    };

    fn assertion(
        id: &str,
        effect: ConsentEffect,
        effective_from_unix_nanos: i64,
        contact_point_ref: Option<&str>,
    ) -> ConsentAuthorization {
        ConsentAuthorization::create(CreateConsentAuthorization {
            authorization_id: ConsentAuthorizationId::try_new(id).unwrap(),
            party_ref: PartyReference::try_new("party-1").unwrap(),
            contact_point_ref: contact_point_ref
                .map(ContactPointReference::try_new)
                .transpose()
                .unwrap(),
            purpose: PurposeCode::try_new("marketing.newsletter").unwrap(),
            channel: CommunicationChannel::Email,
            effect,
            legal_basis: LegalBasisCode::try_new("consent").unwrap(),
            jurisdiction: JurisdictionCode::try_new("eu-lt").unwrap(),
            source: SourceCode::try_new("web.form").unwrap(),
            evidence_ref: EvidenceReference::try_new(format!("evidence://{id}")).unwrap(),
            effective_from_unix_nanos,
            expires_at_unix_nanos: None,
            occurred_at_unix_nanos: effective_from_unix_nanos,
        })
        .unwrap()
    }

    fn request(at: i64, contact_point_ref: Option<&str>) -> EvaluateCommunicationAuthorization {
        EvaluateCommunicationAuthorization {
            party_ref: PartyReference::try_new("party-1").unwrap(),
            contact_point_ref: contact_point_ref
                .map(ContactPointReference::try_new)
                .transpose()
                .unwrap(),
            purpose: PurposeCode::try_new("marketing.newsletter").unwrap(),
            channel: CommunicationChannel::Email,
            evaluation_time_unix_nanos: at,
        }
    }

    #[test]
    fn defaults_closed_when_no_applicable_grant_exists() {
        let decision = evaluate_communication_authorization(&request(100, None), []).unwrap();
        assert!(!decision.allowed);
        assert_eq!(
            decision.reason,
            CommunicationAuthorizationReason::NoApplicableGrant
        );
        assert!(decision.determining_authorization_ids.is_empty());
    }

    #[test]
    fn later_grant_supersedes_older_deny() {
        let deny = assertion("deny-1", ConsentEffect::Deny, 100, None);
        let grant = assertion("grant-1", ConsentEffect::Grant, 200, None);
        let decision =
            evaluate_communication_authorization(&request(300, None), [&deny, &grant]).unwrap();
        assert!(decision.allowed);
        assert_eq!(
            decision.reason,
            CommunicationAuthorizationReason::ActiveGrant
        );
        assert_eq!(
            decision.determining_authorization_ids,
            vec![grant.authorization_id().clone()]
        );
    }

    #[test]
    fn withdrawal_is_an_immediate_barrier_until_a_later_grant() {
        let mut withdrawn = assertion("grant-old", ConsentEffect::Grant, 100, None);
        withdrawn
            .withdraw(WithdrawConsentAuthorization {
                expected_version: 1,
                occurred_at_unix_nanos: 200,
            })
            .unwrap();
        let later_grant = assertion("grant-new", ConsentEffect::Grant, 300, None);

        let denied =
            evaluate_communication_authorization(&request(250, None), [&withdrawn]).unwrap();
        assert!(!denied.allowed);
        assert_eq!(denied.reason, CommunicationAuthorizationReason::Withdrawn);

        let allowed =
            evaluate_communication_authorization(&request(300, None), [&withdrawn, &later_grant])
                .unwrap();
        assert!(allowed.allowed);
        assert_eq!(
            allowed.reason,
            CommunicationAuthorizationReason::ActiveGrant
        );
    }

    #[test]
    fn latest_timestamp_ties_fail_closed_and_are_deterministic() {
        let grant = assertion("grant-1", ConsentEffect::Grant, 200, None);
        let deny = assertion("deny-1", ConsentEffect::Deny, 200, None);
        let decision =
            evaluate_communication_authorization(&request(200, None), [&grant, &deny]).unwrap();
        assert!(!decision.allowed);
        assert_eq!(
            decision.reason,
            CommunicationAuthorizationReason::ActiveDeny
        );
        assert_eq!(
            decision
                .determining_authorization_ids
                .iter()
                .map(ConsentAuthorizationId::as_str)
                .collect::<Vec<_>>(),
            vec!["deny-1", "grant-1"]
        );
    }

    #[test]
    fn contact_point_scoped_assertion_requires_the_exact_requested_contact_point() {
        let scoped = assertion(
            "grant-scoped",
            ConsentEffect::Grant,
            100,
            Some("contact-point-1"),
        );
        let denied = evaluate_communication_authorization(&request(100, None), [&scoped]).unwrap();
        assert!(!denied.allowed);
        let denied =
            evaluate_communication_authorization(&request(100, Some("contact-point-2")), [&scoped])
                .unwrap();
        assert!(!denied.allowed);
        let allowed =
            evaluate_communication_authorization(&request(100, Some("contact-point-1")), [&scoped])
                .unwrap();
        assert!(allowed.allowed);
    }
}
