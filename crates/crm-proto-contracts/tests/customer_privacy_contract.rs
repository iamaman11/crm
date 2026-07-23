use crm_proto_contracts::crm::{customer::v1 as customer, customer_privacy::v1 as privacy};
use crm_proto_contracts::message_descriptor_hash;
use prost::Message;

#[test]
fn privacy_case_contract_preserves_verified_canonical_subject_lineage() {
    let privacy_case = privacy::PrivacyCase {
        privacy_case_ref: Some(privacy::PrivacyCaseRef {
            privacy_case_id: "privacy-case-1".to_owned(),
        }),
        kind: privacy::PrivacyCaseKind::Erasure as i32,
        status: privacy::PrivacyCaseStatus::SubjectVerified as i32,
        version: 3,
        policy_version: "privacy-policy/1".to_owned(),
        created_at_unix_ms: 10,
        updated_at_unix_ms: 12,
        previous_privacy_case_ref: None,
        subject_binding: Some(privacy::SubjectBindingEvidence {
            submitted_party_ref: Some(customer::PartyRef {
                party_id: "party-submitted".to_owned(),
            }),
            canonical_party_ref: Some(customer::PartyRef {
                party_id: "party-canonical".to_owned(),
            }),
            identity_resolution_generation: 7,
            verification_method: privacy::SubjectVerificationMethod::AuthenticatedPortal as i32,
            verified_by_actor_id: "subject-actor".to_owned(),
            verified_at_unix_ms: 12,
        }),
        pending_rescope: None,
        scope_snapshot_id: String::new(),
        privacy_action_plan_ref: None,
        approval: None,
        retry_resume_stage: None,
    };

    assert_eq!(
        privacy::PrivacyCase::decode(privacy_case.encode_to_vec().as_slice()).unwrap(),
        privacy_case
    );
}

#[test]
fn restriction_and_legal_hold_contracts_preserve_exact_control_evidence() {
    let restriction = privacy::ProcessingRestriction {
        processing_restriction_ref: Some(privacy::ProcessingRestrictionRef {
            processing_restriction_id: "restriction-1".to_owned(),
        }),
        canonical_party_ref: Some(customer::PartyRef {
            party_id: "party-canonical".to_owned(),
        }),
        scope: privacy::ProcessingRestrictionScope::ProcessingAndCommunication as i32,
        status: privacy::ProcessingRestrictionStatus::Active as i32,
        version: 1,
        policy_version: "privacy-policy/1".to_owned(),
        placed_by_actor_id: "privacy-officer".to_owned(),
        placed_at_unix_ms: 20,
        effective_from_unix_ms: 20,
        expires_at_unix_ms: Some(200),
        released_by_actor_id: None,
        released_at_unix_ms: None,
    };
    let hold = privacy::CustomerDataLegalHold {
        customer_data_legal_hold_ref: Some(privacy::CustomerDataLegalHoldRef {
            customer_data_legal_hold_id: "hold-1".to_owned(),
        }),
        canonical_party_ref: Some(customer::PartyRef {
            party_id: "party-canonical".to_owned(),
        }),
        scope: Some(privacy::CustomerDataLegalHoldScope {
            scope: Some(privacy::customer_data_legal_hold_scope::Scope::DataClass(
                privacy::CustomerDataClass::Personal as i32,
            )),
        }),
        authority_reference_id: "authority-1".to_owned(),
        reason_code: "LITIGATION_HOLD".to_owned(),
        policy_version: "privacy-policy/1".to_owned(),
        status: privacy::CustomerDataLegalHoldStatus::Active as i32,
        version: 1,
        placed_by_actor_id: "legal-officer".to_owned(),
        effective_from_unix_ms: 30,
        effective_until_unix_ms: None,
        released_by_actor_id: None,
        released_at_unix_ms: None,
    };

    assert_eq!(
        privacy::ProcessingRestriction::decode(restriction.encode_to_vec().as_slice()).unwrap(),
        restriction
    );
    assert_eq!(
        privacy::CustomerDataLegalHold::decode(hold.encode_to_vec().as_slice()).unwrap(),
        hold
    );
}

#[test]
fn privacy_plan_and_owner_outcome_queries_expose_references_not_owner_payloads() {
    let response = privacy::ListPrivacyOwnerOutcomesResponse {
        privacy_owner_outcomes: vec![privacy::PrivacyOwnerOutcome {
            privacy_owner_outcome_ref: Some(privacy::PrivacyOwnerOutcomeRef {
                privacy_owner_outcome_id: "outcome-1".to_owned(),
            }),
            privacy_action_plan_ref: Some(privacy::PrivacyActionPlanRef {
                privacy_action_plan_id: "plan-1".to_owned(),
            }),
            owner_module_id: "crm.parties".to_owned(),
            action_code: "ANONYMIZE".to_owned(),
            status: privacy::PrivacyOwnerOutcomeStatus::Succeeded as i32,
            safe_failure_code: None,
            recorded_at_unix_ms: 50,
        }],
        next_cursor: String::new(),
    };

    assert_eq!(
        privacy::ListPrivacyOwnerOutcomesResponse::decode(response.encode_to_vec().as_slice())
            .unwrap(),
        response
    );
}

#[test]
fn owner_scope_contribution_contract_preserves_lineage_references_and_page_evidence() {
    let response = privacy::PartiesPrivacyScopeContributionResponse {
        contribution: Some(privacy::PrivacyScopeContributionResponseEnvelope {
            owner_module_id: "crm.parties".to_owned(),
            capability_id: "parties.privacy.scope.contribute".to_owned(),
            capability_version: "1.0.0".to_owned(),
            lineage: Some(privacy::PrivacyScopeContributionLineage {
                privacy_case_id: "privacy-case-1".to_owned(),
                tenant_id: "tenant-a".to_owned(),
                canonical_party_ref: Some(customer::PartyRef {
                    party_id: "party-canonical".to_owned(),
                }),
                identity_resolution_generation: 7,
                registry_version: "crm.customer-privacy.scope-registry/1.0.0".to_owned(),
                registry_digest_sha256: vec![1; 32],
                purpose_code: "PRIVACY_ERASURE_SCOPE".to_owned(),
                effective_request_at_unix_ms: 100,
            }),
            resources: vec![privacy::PrivacyScopeResourceReference {
                resource_type: "parties.party".to_owned(),
                resource_id: "party-canonical".to_owned(),
                resource_version: 3,
                data_class: privacy::CustomerDataClass::Restricted as i32,
                evidence_class: privacy::PrivacyScopeEvidenceClass::RetainMinimizedEvidence as i32,
                retention_policy_id: "crm.parties.party".to_owned(),
            }],
            page_evidence: Some(privacy::PrivacyScopeContributionPageEvidence {
                page_number: 1,
                scanned_resource_count: 1,
                emitted_resource_count: 1,
                next_cursor: String::new(),
                terminal_complete: true,
                cursor_digest_sha256: vec![2; 32],
                page_digest_sha256: vec![3; 32],
            }),
        }),
    };

    assert_eq!(
        privacy::PartiesPrivacyScopeContributionResponse::decode(
            response.encode_to_vec().as_slice()
        )
        .unwrap(),
        response
    );
}

#[test]
fn customer_privacy_descriptor_identities_are_stable_and_distinct() {
    let case = message_descriptor_hash("crm.customer_privacy.v1.PrivacyCase");
    let restriction = message_descriptor_hash("crm.customer_privacy.v1.ProcessingRestriction");
    let hold = message_descriptor_hash("crm.customer_privacy.v1.CustomerDataLegalHold");
    let create = message_descriptor_hash("crm.customer_privacy.v1.CreatePrivacyCaseRequest");
    let contribution =
        message_descriptor_hash("crm.customer_privacy.v1.PartiesPrivacyScopeContributionRequest");

    assert_eq!(
        case,
        message_descriptor_hash("crm.customer_privacy.v1.PrivacyCase")
    );
    assert_ne!(case, restriction);
    assert_ne!(restriction, hold);
    assert_ne!(hold, create);
    assert_ne!(create, contribution);
}
