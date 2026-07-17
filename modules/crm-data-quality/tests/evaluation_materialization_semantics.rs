use crm_data_quality::{
    ComponentKey, EvaluatedPartyKind, PartyCompletenessComponent, PartyCompletenessProfileVersion,
    PartyEvaluationJob, PartyEvaluationJobStatus, PartyQualityEvaluator, PartyQualityRule,
    PartyRuleSetVersion, QualitySeverity, RuleKey, decode_party_evaluation_job_state,
    encode_party_evaluation_job_state,
};
use crm_module_sdk::RecordId;

#[test]
fn staged_job_crosses_completion_only_after_exact_materialized_counts() {
    let rule_key = RuleKey::try_new("display_name.minimum").unwrap();
    let rule_set = PartyRuleSetVersion::publish(vec![
        PartyQualityRule::try_new(
            rule_key.clone(),
            QualitySeverity::Warning,
            PartyQualityEvaluator::display_name_min_utf8_bytes(4).unwrap(),
            "Display name minimum",
            "Use a meaningful display name.",
        )
        .unwrap(),
    ])
    .unwrap();
    let profile = PartyCompletenessProfileVersion::publish(
        &rule_set,
        vec![
            PartyCompletenessComponent::try_new(
                ComponentKey::try_new("name.minimum").unwrap(),
                rule_key,
                10_000,
            )
            .unwrap(),
        ],
    )
    .unwrap();
    let created = PartyEvaluationJob::create(
        RecordId::try_new("materialization-job").unwrap(),
        RecordId::try_new("materialization-party").unwrap(),
        &rule_set,
        &profile,
        100,
    )
    .unwrap();
    let (staged, input) = created
        .stage(EvaluatedPartyKind::Person, "Ada", 7, 101)
        .unwrap();

    let materialized = staged
        .record_materialized_outcomes(1, 1, input.captured_at())
        .unwrap();
    assert_eq!(materialized.status(), PartyEvaluationJobStatus::Staged);
    assert!(materialized.outcomes_materialized());
    assert_eq!(materialized.evaluated_rules(), 1);
    assert_eq!(materialized.failed_rules(), 1);
    assert_eq!(materialized.updated_at(), input.captured_at());
    assert!(materialized.complete(2, 1, 102).is_err());
    assert!(materialized.complete(1, 0, 102).is_err());

    let completed = materialized.complete(1, 1, 102).unwrap();
    assert_eq!(completed.status(), PartyEvaluationJobStatus::Completed);
    assert_eq!(completed.evaluated_rules(), 1);
    assert_eq!(completed.failed_rules(), 1);
    assert_eq!(completed.updated_at(), 102);
    let bytes = encode_party_evaluation_job_state(&completed).unwrap();
    assert_eq!(
        decode_party_evaluation_job_state(&bytes).unwrap(),
        completed
    );
    assert!(completed.complete(1, 1, 103).is_err());

    assert!(
        staged
            .record_materialized_outcomes(0, 0, input.captured_at())
            .is_err()
    );
    assert!(
        staged
            .record_materialized_outcomes(1, 2, input.captured_at())
            .is_err()
    );
    assert!(
        materialized
            .record_materialized_outcomes(1, 1, input.captured_at())
            .is_err()
    );
}
