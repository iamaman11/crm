use crm_data_quality::{
    ComponentKey, EvaluatedPartyKind, PartyCompletenessComponent, PartyCompletenessProfileVersion,
    PartyEvaluationJob, PartyFinding, PartyFindingObservation, PartyFindingStatus,
    PartyQualityEvaluator, PartyQualityInput, PartyQualityRule, PartyRuleOutcome, PartyRuleSetVersion,
    QualitySeverity, RuleKey, decode_finding_observation_state, decode_finding_state,
    encode_finding_observation_state, encode_finding_state,
};
use crm_module_sdk::{RecordId, TenantId};

const LARGE_UNIX_NANOS: i64 = 1_700_000_000_000_000_000;

#[test]
fn logical_finding_is_stable_across_jobs_and_tracks_versioned_observations() {
    let rule_key = RuleKey::try_new("display_name.placeholder").unwrap();
    let rule = PartyQualityRule::try_new(
        rule_key.clone(),
        QualitySeverity::Error,
        PartyQualityEvaluator::display_name_placeholder_exact_ascii_casefold(vec![
            "unknown".to_owned(),
        ])
        .unwrap(),
        "Display name placeholder",
        "Replace the placeholder display name.",
    )
    .unwrap();
    let rule_set = PartyRuleSetVersion::publish(vec![rule.clone()]).unwrap();
    let profile = PartyCompletenessProfileVersion::publish(
        &rule_set,
        vec![PartyCompletenessComponent::try_new(
            ComponentKey::try_new("name.placeholder").unwrap(),
            rule_key,
            10_000,
        )
        .unwrap()],
    )
    .unwrap();
    let tenant_id = TenantId::try_new("finding-tenant").unwrap();
    let party_id = RecordId::try_new("finding-party").unwrap();

    let failed = outcome(
        "finding-job-1",
        &party_id,
        &rule_set,
        &profile,
        "unknown",
        7,
        LARGE_UNIX_NANOS,
    );
    let replayed_failure = outcome(
        "finding-job-2",
        &party_id,
        &rule_set,
        &profile,
        "unknown",
        7,
        LARGE_UNIX_NANOS + 10,
    );
    let first_observation =
        PartyFindingObservation::observe_failure(tenant_id.clone(), &rule, &failed).unwrap();
    let replayed_observation =
        PartyFindingObservation::observe_failure(tenant_id.clone(), &rule, &replayed_failure)
            .unwrap();
    assert_eq!(
        first_observation.finding_id(),
        replayed_observation.finding_id()
    );
    assert_eq!(
        first_observation.observation_id(),
        replayed_observation.observation_id()
    );

    let open = PartyFinding::open(&rule, &first_observation).unwrap();
    assert_eq!(open.status(), PartyFindingStatus::Open);
    assert_eq!(
        open.apply_failed_observation(&first_observation).unwrap(),
        open
    );

    let passing = outcome(
        "finding-job-3",
        &party_id,
        &rule_set,
        &profile,
        "Alice",
        8,
        LARGE_UNIX_NANOS + 20,
    );
    let remediated = open.apply_passing_outcome(&passing).unwrap();
    assert_eq!(remediated.status(), PartyFindingStatus::Remediated);
    assert_eq!(remediated.evaluated_party_resource_version(), 8);
    assert_eq!(
        remediated.remediated_by_rule_outcome_id(),
        Some(passing.outcome_id())
    );
    assert_eq!(
        remediated.apply_passing_outcome(&passing).unwrap(),
        remediated
    );

    let newer_failure = outcome(
        "finding-job-4",
        &party_id,
        &rule_set,
        &profile,
        "unknown",
        9,
        LARGE_UNIX_NANOS + 30,
    );
    let newer_observation =
        PartyFindingObservation::observe_failure(tenant_id, &rule, &newer_failure).unwrap();
    let reopened = remediated
        .apply_failed_observation(&newer_observation)
        .unwrap();
    assert_eq!(reopened.status(), PartyFindingStatus::Open);
    assert_eq!(reopened.evaluated_party_resource_version(), 9);
    assert_eq!(
        reopened.current_observation_id(),
        newer_observation.observation_id()
    );
    assert_eq!(reopened.remediated_by_rule_outcome_id(), None);
    assert!(reopened.apply_passing_outcome(&passing).is_err());
}

#[test]
fn finding_persistence_is_strict_canonical_and_recomputes_identities() {
    let rule_key = RuleKey::try_new("display_name.placeholder").unwrap();
    let rule = PartyQualityRule::try_new(
        rule_key.clone(),
        QualitySeverity::Critical,
        PartyQualityEvaluator::display_name_placeholder_exact_ascii_casefold(vec![
            "unknown".to_owned(),
        ])
        .unwrap(),
        "Display name placeholder",
        "Replace the placeholder display name.",
    )
    .unwrap();
    let rule_set = PartyRuleSetVersion::publish(vec![rule.clone()]).unwrap();
    let profile = PartyCompletenessProfileVersion::publish(
        &rule_set,
        vec![PartyCompletenessComponent::try_new(
            ComponentKey::try_new("name.placeholder").unwrap(),
            rule_key,
            10_000,
        )
        .unwrap()],
    )
    .unwrap();
    let failed = outcome(
        "finding-persistence-job",
        &RecordId::try_new("finding-persistence-party").unwrap(),
        &rule_set,
        &profile,
        "unknown",
        11,
        LARGE_UNIX_NANOS,
    );
    let observation = PartyFindingObservation::observe_failure(
        TenantId::try_new("finding-persistence-tenant").unwrap(),
        &rule,
        &failed,
    )
    .unwrap();
    let finding = PartyFinding::open(&rule, &observation).unwrap();

    let observation_bytes = encode_finding_observation_state(&observation).unwrap();
    assert_eq!(
        decode_finding_observation_state(&observation_bytes).unwrap(),
        observation
    );
    let observation_text = std::str::from_utf8(&observation_bytes).unwrap();
    let noncanonical_observation = observation_text.replace(
        &format!("\"observed_at\":\"{}\"", failed.evaluated_at()),
        &format!("\"observed_at\":\"0{}\"", failed.evaluated_at()),
    );
    assert!(decode_finding_observation_state(noncanonical_observation.as_bytes()).is_err());
    let forged_observation = observation_text.replace(
        observation.observation_id(),
        "dq-finding-observation-forged",
    );
    assert!(decode_finding_observation_state(forged_observation.as_bytes()).is_err());

    let finding_bytes = encode_finding_state(&finding).unwrap();
    assert_eq!(decode_finding_state(&finding_bytes).unwrap(), finding);
    let finding_text = std::str::from_utf8(&finding_bytes).unwrap();
    let noncanonical_finding = finding_text.replace(
        &format!("\"created_at\":\"{}\"", finding.created_at()),
        &format!("\"created_at\":\"0{}\"", finding.created_at()),
    );
    assert!(decode_finding_state(noncanonical_finding.as_bytes()).is_err());
    let forged_finding = finding_text.replace(finding.finding_id(), "dq-finding-forged");
    assert!(decode_finding_state(forged_finding.as_bytes()).is_err());
    let unknown_field = finding_text.replacen('{', "{\"unknown\":true,", 1);
    assert!(decode_finding_state(unknown_field.as_bytes()).is_err());
}

fn outcome(
    job_id: &str,
    party_id: &RecordId,
    rule_set: &PartyRuleSetVersion,
    profile: &PartyCompletenessProfileVersion,
    display_name: &str,
    party_resource_version: i64,
    now: i64,
) -> PartyRuleOutcome {
    let created = PartyEvaluationJob::create(
        RecordId::try_new(job_id).unwrap(),
        party_id.clone(),
        rule_set,
        profile,
        now,
    )
    .unwrap();
    let (staged, _) = created
        .stage(
            EvaluatedPartyKind::Person,
            display_name,
            party_resource_version,
            now + 1,
        )
        .unwrap();
    let evaluation = rule_set
        .evaluate(
            &PartyQualityInput::try_new(EvaluatedPartyKind::Person, display_name).unwrap(),
        )
        .into_iter()
        .next()
        .unwrap();
    PartyRuleOutcome::evaluate(&staged, &evaluation, now + 2).unwrap()
}
