use crm_data_quality::{
    ComponentKey, EvaluatedPartyKind, PartyCompletenessComponent, PartyCompletenessProfileVersion,
    PartyEvaluationJob, PartyFindingObservation, PartyQualityEvaluator, PartyQualityInput,
    PartyQualityRule, PartyRuleOutcome, PartyRuleSetVersion, QualitySeverity, RuleKey,
};
use crm_module_sdk::{RecordId, TenantId};

const NOW: i64 = 1_700_000_000_000_000_000;

#[test]
fn finding_identity_is_stable_within_a_tenant_and_isolated_across_tenants() {
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
    let party_id = RecordId::try_new("tenant-isolated-party").unwrap();
    let first = failed_outcome(
        "tenant-isolation-job-1",
        &party_id,
        &rule_set,
        &profile,
        NOW,
    );
    let replay = failed_outcome(
        "tenant-isolation-job-2",
        &party_id,
        &rule_set,
        &profile,
        NOW + 10,
    );

    let tenant_a = TenantId::try_new("tenant-a").unwrap();
    let tenant_b = TenantId::try_new("tenant-b").unwrap();
    let first_a =
        PartyFindingObservation::observe_failure(tenant_a.clone(), &rule, &first).unwrap();
    let replay_a = PartyFindingObservation::observe_failure(tenant_a, &rule, &replay).unwrap();
    let first_b = PartyFindingObservation::observe_failure(tenant_b, &rule, &first).unwrap();

    assert_eq!(first_a.finding_id(), replay_a.finding_id());
    assert_eq!(first_a.observation_id(), replay_a.observation_id());
    assert_ne!(first_a.finding_id(), first_b.finding_id());
    assert_ne!(first_a.observation_id(), first_b.observation_id());
}

fn failed_outcome(
    job_id: &str,
    party_id: &RecordId,
    rule_set: &PartyRuleSetVersion,
    profile: &PartyCompletenessProfileVersion,
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
        .stage(EvaluatedPartyKind::Person, "unknown", 7, now + 1)
        .unwrap();
    let evaluation = rule_set
        .evaluate(&PartyQualityInput::try_new(EvaluatedPartyKind::Person, "unknown").unwrap())
        .into_iter()
        .next()
        .unwrap();
    let outcome = PartyRuleOutcome::evaluate(&staged, &evaluation, now + 2).unwrap();
    assert!(!outcome.passed());
    outcome
}
