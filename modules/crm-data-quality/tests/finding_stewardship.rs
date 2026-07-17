use crm_data_quality::{
    ComponentKey, EvaluatedPartyKind, PartyCompletenessComponent, PartyCompletenessProfileVersion,
    PartyEvaluationJob, PartyFinding, PartyFindingObservation, PartyFindingStatus,
    PartyQualityEvaluator, PartyQualityInput, PartyQualityRule, PartyRuleOutcome,
    PartyRuleSetVersion, QualitySeverity, RuleKey, decode_finding_state, encode_finding_state,
};
use crm_module_sdk::{ActorId, ErrorCategory, RecordId, TenantId};

#[test]
fn assignment_acknowledgement_and_waiver_bind_exact_current_evidence() {
    let finding = open_finding();
    let observation_id = finding.current_observation_id().to_owned();

    let assigned = finding
        .assign(Some(ActorId::try_new("steward-1").unwrap()), 103)
        .unwrap();
    assert_eq!(assigned.status(), PartyFindingStatus::Open);
    assert_eq!(assigned.current_observation_id(), observation_id);
    assert_eq!(assigned.assigned_actor_id().unwrap().as_str(), "steward-1");

    let stale = assigned
        .acknowledge("different-observation", 104)
        .unwrap_err();
    assert_eq!(stale.category, ErrorCategory::Conflict);

    let acknowledged = assigned.acknowledge(&observation_id, 104).unwrap();
    assert_eq!(acknowledged.status(), PartyFindingStatus::Acknowledged);
    assert_eq!(acknowledged.current_observation_id(), observation_id);
    assert!(acknowledged.waiver_reason().is_none());

    let waived = acknowledged
        .waive(&observation_id, "Accepted source exception", 105)
        .unwrap();
    assert_eq!(waived.status(), PartyFindingStatus::Waived);
    assert_eq!(waived.waiver_reason(), Some("Accepted source exception"));
    assert_eq!(waived.assigned_actor_id().unwrap().as_str(), "steward-1");

    let cleared = waived.assign(None, 106).unwrap();
    assert!(cleared.assigned_actor_id().is_none());
    assert_eq!(cleared.status(), PartyFindingStatus::Waived);
    assert_eq!(cleared.waiver_reason(), Some("Accepted source exception"));

    let bytes = encode_finding_state(&cleared).unwrap();
    assert_eq!(decode_finding_state(&bytes).unwrap(), cleared);
}

#[test]
fn stewardship_rejects_invalid_reason_time_and_remediated_state() {
    let finding = open_finding();
    let observation_id = finding.current_observation_id().to_owned();
    assert!(finding.waive(&observation_id, " padded ", 103).is_err());
    assert!(finding.assign(None, 99).is_err());

    let (rule_set, _profile, staged, input) = evaluation_fixture("Meaningful Name", 8, 200);
    let evaluation = rule_set.evaluate(&input).into_iter().next().unwrap();
    let passing = PartyRuleOutcome::evaluate(&staged, &evaluation, 201).unwrap();
    let remediated = finding.apply_passing_outcome(&passing).unwrap();
    assert_eq!(remediated.status(), PartyFindingStatus::Remediated);
    assert!(remediated.assign(None, 202).is_err());
    assert!(remediated.acknowledge(&observation_id, 202).is_err());
}

fn open_finding() -> PartyFinding {
    let (rule_set, _profile, staged, input) = evaluation_fixture("unknown", 7, 100);
    let rule = &rule_set.rules()[0];
    let evaluation = rule_set.evaluate(&input).into_iter().next().unwrap();
    let outcome = PartyRuleOutcome::evaluate(&staged, &evaluation, 102).unwrap();
    let observation = PartyFindingObservation::observe_failure(
        TenantId::try_new("tenant-stewardship").unwrap(),
        rule,
        &outcome,
    )
    .unwrap();
    PartyFinding::open(rule, &observation).unwrap()
}

fn evaluation_fixture(
    display_name: &str,
    party_version: i64,
    now: i64,
) -> (
    PartyRuleSetVersion,
    PartyCompletenessProfileVersion,
    PartyEvaluationJob,
    PartyQualityInput,
) {
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
        RecordId::try_new(format!("stewardship-job-{party_version}")).unwrap(),
        RecordId::try_new("stewardship-party").unwrap(),
        &rule_set,
        &profile,
        now,
    )
    .unwrap();
    let (staged, _) = created
        .stage(
            EvaluatedPartyKind::Person,
            display_name,
            party_version,
            now + 1,
        )
        .unwrap();
    let input = PartyQualityInput::try_new(EvaluatedPartyKind::Person, display_name).unwrap();
    (rule_set, profile, staged, input)
}
