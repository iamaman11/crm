use crm_data_quality::{
    ComponentKey, EvaluatedPartyKind, PartyCompletenessComponent, PartyCompletenessProfileVersion,
    PartyDisplayNameRemediationAttempt, PartyDisplayNameRemediationIdentity, PartyEvaluationJob,
    PartyFinding, PartyFindingObservation, PartyQualityEvaluator, PartyQualityInput,
    PartyQualityRule, PartyRuleOutcome, PartyRuleSetVersion, QualitySeverity, RuleKey,
    decode_remediation_attempt_state, encode_remediation_attempt_state,
};
use crm_module_sdk::{IdempotencyKey, RecordId, TenantId};

#[test]
fn remediation_identity_is_deterministic_and_binds_exact_request_evidence() {
    let finding = open_finding();
    let tenant = TenantId::try_new("tenant-remediation").unwrap();
    let caller = IdempotencyKey::try_new("remediation-call-1").unwrap();
    let observation = finding.current_observation_id().to_owned();

    let first = PartyDisplayNameRemediationIdentity::derive(
        &tenant,
        &caller,
        &finding,
        1,
        &observation,
        7,
        "Ada Lovelace",
    )
    .unwrap();
    let replay = PartyDisplayNameRemediationIdentity::derive(
        &tenant,
        &caller,
        &finding,
        1,
        &observation,
        7,
        "Ada Lovelace",
    )
    .unwrap();
    assert_eq!(first, replay);

    let changed_name = PartyDisplayNameRemediationIdentity::derive(
        &tenant,
        &caller,
        &finding,
        1,
        &observation,
        7,
        "Grace Hopper",
    )
    .unwrap();
    assert_ne!(first.attempt_id(), changed_name.attempt_id());
    assert_ne!(
        first.target_idempotency_key(),
        changed_name.target_idempotency_key()
    );

    let changed_caller = PartyDisplayNameRemediationIdentity::derive(
        &tenant,
        &IdempotencyKey::try_new("remediation-call-2").unwrap(),
        &finding,
        1,
        &observation,
        7,
        "Ada Lovelace",
    )
    .unwrap();
    assert_ne!(first.attempt_id(), changed_caller.attempt_id());
}

#[test]
fn completed_remediation_attempt_round_trips_strict_canonical_state() {
    let finding = open_finding();
    let tenant = TenantId::try_new("tenant-remediation").unwrap();
    let caller = IdempotencyKey::try_new("remediation-call-persisted").unwrap();
    let observation = finding.current_observation_id().to_owned();
    let identity = PartyDisplayNameRemediationIdentity::derive(
        &tenant,
        &caller,
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
    assert_eq!(decode_remediation_attempt_state(&bytes).unwrap(), attempt);

    let mut json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    json.as_object_mut()
        .unwrap()
        .insert("unexpected".to_owned(), serde_json::Value::Bool(true));
    assert!(decode_remediation_attempt_state(&serde_json::to_vec(&json).unwrap()).is_err());
}

#[test]
fn remediation_rejects_stale_observation_party_version_and_nonadvancing_target() {
    let finding = open_finding();
    let tenant = TenantId::try_new("tenant-remediation").unwrap();
    let caller = IdempotencyKey::try_new("remediation-call-conflict").unwrap();
    let observation = finding.current_observation_id().to_owned();

    assert!(
        PartyDisplayNameRemediationIdentity::derive(
            &tenant,
            &caller,
            &finding,
            1,
            "stale-observation",
            7,
            "Ada Lovelace",
        )
        .is_err()
    );
    assert!(
        PartyDisplayNameRemediationIdentity::derive(
            &tenant,
            &caller,
            &finding,
            1,
            &observation,
            8,
            "Ada Lovelace",
        )
        .is_err()
    );

    let identity = PartyDisplayNameRemediationIdentity::derive(
        &tenant,
        &caller,
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
            7,
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
        RecordId::try_new("remediation-job").unwrap(),
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
