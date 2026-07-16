use crm_data_quality::{
    ComponentKey, EvaluatedPartyKind, PartyCompletenessComponent, PartyCompletenessProfileVersion,
    PartyDisplayNameRemediationAttempt, PartyDisplayNameRemediationIdentity, PartyEvaluationJob,
    PartyFinding, PartyFindingObservation, PartyQualityEvaluator, PartyQualityInput,
    PartyQualityRule, PartyRuleOutcome, PartyRuleSetVersion, QualitySeverity, RuleKey,
    decode_remediation_attempt_state, encode_remediation_attempt_state,
};
use crm_module_sdk::{IdempotencyKey, RecordId, TenantId};

#[test]
fn remediation_identity_rejects_cross_tenant_evidence() {
    let finding = open_finding();
    let observation = finding.current_observation_id().to_owned();
    let error = PartyDisplayNameRemediationIdentity::derive(
        &TenantId::try_new("tenant-other").unwrap(),
        &IdempotencyKey::try_new("remediation-cross-tenant").unwrap(),
        &finding,
        1,
        &observation,
        7,
        "Ada Lovelace",
    )
    .unwrap_err();
    assert_eq!(error.code, "DATA_QUALITY_REMEDIATION_EVIDENCE_CONFLICT");
}

#[test]
fn remediation_identity_is_stable_for_exact_replay_and_changes_with_caller_key() {
    let finding = open_finding();
    let tenant = TenantId::try_new("tenant-remediation").unwrap();
    let observation = finding.current_observation_id().to_owned();
    let caller_key = IdempotencyKey::try_new("remediation-stable-replay").unwrap();
    let first = PartyDisplayNameRemediationIdentity::derive(
        &tenant,
        &caller_key,
        &finding,
        1,
        &observation,
        7,
        "Ada Lovelace",
    )
    .unwrap();
    let replay = PartyDisplayNameRemediationIdentity::derive(
        &tenant,
        &caller_key,
        &finding,
        1,
        &observation,
        7,
        "Ada Lovelace",
    )
    .unwrap();
    let different_request = PartyDisplayNameRemediationIdentity::derive(
        &tenant,
        &IdempotencyKey::try_new("remediation-different-request").unwrap(),
        &finding,
        1,
        &observation,
        7,
        "Ada Lovelace",
    )
    .unwrap();

    assert_eq!(first, replay);
    assert_ne!(first.attempt_id(), different_request.attempt_id());
    assert_ne!(
        first.target_idempotency_key(),
        different_request.target_idempotency_key()
    );
}

#[test]
fn persisted_remediation_identity_rejects_forged_party_reference() {
    let finding = open_finding();
    let tenant = TenantId::try_new("tenant-remediation").unwrap();
    let observation = finding.current_observation_id().to_owned();
    let identity = PartyDisplayNameRemediationIdentity::derive(
        &tenant,
        &IdempotencyKey::try_new("remediation-forged-party").unwrap(),
        &finding,
        1,
        &observation,
        7,
        "Ada Lovelace",
    )
    .unwrap();
    let attempt = PartyDisplayNameRemediationAttempt::complete(
        tenant,
        identity,
        &finding,
        1,
        &observation,
        7,
        "Ada Lovelace",
        8,
        103,
    )
    .unwrap();
    let bytes = encode_remediation_attempt_state(&attempt).unwrap();
    let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    value["party_id"] = serde_json::Value::String("forged-party".to_owned());
    assert!(decode_remediation_attempt_state(&serde_json::to_vec(&value).unwrap()).is_err());
}

#[test]
fn remediation_completion_requires_exact_next_party_version() {
    let finding = open_finding();
    let tenant = TenantId::try_new("tenant-remediation").unwrap();
    let observation = finding.current_observation_id().to_owned();
    let identity = PartyDisplayNameRemediationIdentity::derive(
        &tenant,
        &IdempotencyKey::try_new("remediation-version-gap").unwrap(),
        &finding,
        1,
        &observation,
        7,
        "Ada Lovelace",
    )
    .unwrap();
    assert!(
        PartyDisplayNameRemediationAttempt::complete(
            tenant,
            identity,
            &finding,
            1,
            &observation,
            7,
            "Ada Lovelace",
            9,
            103,
        )
        .is_err()
    );
}

fn open_finding() -> PartyFinding {
    let rule_key = RuleKey::try_new("display_name.placeholder").unwrap();
    let rule_set = PartyRuleSetVersion::publish(vec![
        PartyQualityRule::try_new(
            rule_key.clone(),
            QualitySeverity::Warning,
            PartyQualityEvaluator::display_name_placeholder_exact_ascii_casefold(vec![
                "unknown".to_owned(),
            ])
            .unwrap(),
            "Placeholder display name",
            "Replace the placeholder with a meaningful display name.",
        )
        .unwrap(),
    ])
    .unwrap();
    let profile = PartyCompletenessProfileVersion::publish(
        &rule_set,
        vec![
            PartyCompletenessComponent::try_new(
                ComponentKey::try_new("name.placeholder").unwrap(),
                rule_key,
                10_000,
            )
            .unwrap(),
        ],
    )
    .unwrap();
    let created = PartyEvaluationJob::create(
        RecordId::try_new("remediation-integrity-job").unwrap(),
        RecordId::try_new("remediation-party").unwrap(),
        &rule_set,
        &profile,
        100,
    )
    .unwrap();
    let (staged, _) = created
        .stage(EvaluatedPartyKind::Person, "unknown", 7, 101)
        .unwrap();
    let input = PartyQualityInput::try_new(EvaluatedPartyKind::Person, "unknown").unwrap();
    let evaluation = rule_set.evaluate(&input).into_iter().next().unwrap();
    let outcome = PartyRuleOutcome::evaluate(&staged, &evaluation, 102).unwrap();
    let observation = PartyFindingObservation::observe_failure(
        TenantId::try_new("tenant-remediation").unwrap(),
        &rule_set.rules()[0],
        &outcome,
    )
    .unwrap();
    PartyFinding::open(&rule_set.rules()[0], &observation).unwrap()
}
