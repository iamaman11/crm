use crm_data_quality::{
    ComponentKey, EvaluatedPartyKind, PartyCompletenessComponent,
    PartyCompletenessProfileVersion, PartyCompletenessResult, PartyEvaluationJob,
    PartyQualityEvaluator, PartyQualityInput, PartyQualityRule, PartyRuleOutcome,
    PartyRuleSetVersion, QualitySeverity, RuleKey,
};
use crm_module_sdk::RecordId;

#[test]
fn staged_evaluation_produces_replay_stable_outcomes_and_exact_lineage() {
    let minimum_key = RuleKey::try_new("display_name.minimum").unwrap();
    let placeholder_key = RuleKey::try_new("display_name.placeholder").unwrap();
    let rule_set = PartyRuleSetVersion::publish(vec![
        PartyQualityRule::try_new(
            minimum_key.clone(),
            QualitySeverity::Warning,
            PartyQualityEvaluator::display_name_min_utf8_bytes(4).unwrap(),
            "Display name minimum",
            "Use a meaningful display name.",
        )
        .unwrap(),
        PartyQualityRule::try_new(
            placeholder_key.clone(),
            QualitySeverity::Error,
            PartyQualityEvaluator::display_name_placeholder_exact_ascii_casefold(vec![
                "unknown".to_owned(),
            ])
            .unwrap(),
            "Display name placeholder",
            "Replace the placeholder display name.",
        )
        .unwrap(),
    ])
    .unwrap();
    let profile = PartyCompletenessProfileVersion::publish(
        &rule_set,
        vec![
            PartyCompletenessComponent::try_new(
                ComponentKey::try_new("name.minimum").unwrap(),
                minimum_key,
                4_000,
            )
            .unwrap(),
            PartyCompletenessComponent::try_new(
                ComponentKey::try_new("name.placeholder").unwrap(),
                placeholder_key,
                6_000,
            )
            .unwrap(),
        ],
    )
    .unwrap();
    let created = PartyEvaluationJob::create(
        RecordId::try_new("outcome-job").unwrap(),
        RecordId::try_new("outcome-party").unwrap(),
        &rule_set,
        &profile,
        100,
    )
    .unwrap();
    let (staged, _) = created
        .stage(EvaluatedPartyKind::Person, "unknown", 7, 110)
        .unwrap();
    let evaluations = rule_set.evaluate(
        &PartyQualityInput::try_new(EvaluatedPartyKind::Person, "unknown").unwrap(),
    );
    let outcomes = evaluations
        .iter()
        .map(|evaluation| PartyRuleOutcome::evaluate(&staged, evaluation, 120).unwrap())
        .collect::<Vec<_>>();
    let replayed_outcomes = evaluations
        .iter()
        .map(|evaluation| PartyRuleOutcome::evaluate(&staged, evaluation, 120).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(outcomes, replayed_outcomes);
    assert_eq!(outcomes.len(), 2);
    assert_eq!(outcomes.iter().filter(|outcome| !outcome.passed()).count(), 1);

    let result = PartyCompletenessResult::compute(&staged, &profile, &outcomes, 130).unwrap();
    let replayed = PartyCompletenessResult::compute(&staged, &profile, &outcomes, 130).unwrap();
    assert_eq!(result, replayed);
    assert_eq!(result.score_basis_points(), 4_000);
    assert_eq!(result.components().len(), 2);
    assert_eq!(
        result
            .components()
            .iter()
            .map(|component| component.awarded_basis_points())
            .sum::<u32>(),
        result.score_basis_points()
    );
    for component in result.components() {
        assert!(
            outcomes
                .iter()
                .any(|outcome| outcome.outcome_id() == component.rule_outcome_id())
        );
    }
}
