use crm_data_quality::{
    ComponentKey, EvaluatedPartyKind, PartyCompletenessComponent,
    PartyCompletenessProfileVersion, PartyEvaluationJob, PartyQualityEvaluator, PartyQualityRule,
    PartyRuleSetVersion, QualitySeverity, RuleKey, decode_party_evaluation_input_state,
    decode_party_evaluation_job_state, encode_party_evaluation_input_state,
    encode_party_evaluation_job_state,
};
use crm_module_sdk::RecordId;

const LARGE_UNIX_NANOS: i64 = 1_700_000_000_000_000_000;

#[test]
fn evaluation_timestamps_above_safe_json_integer_range_are_decimal_strings() {
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
                ComponentKey::try_new("display_name").unwrap(),
                rule_key,
                10_000,
            )
            .unwrap(),
        ],
    )
    .unwrap();
    let job = PartyEvaluationJob::create(
        RecordId::try_new("evaluation-timestamp-job").unwrap(),
        RecordId::try_new("evaluation-timestamp-party").unwrap(),
        &rule_set,
        &profile,
        LARGE_UNIX_NANOS,
    )
    .unwrap();

    let job_bytes = encode_party_evaluation_job_state(&job).unwrap();
    let job_text = std::str::from_utf8(&job_bytes).unwrap();
    assert!(job_text.contains(&format!("\"created_at\":\"{LARGE_UNIX_NANOS}\"")));
    assert!(job_text.contains(&format!("\"updated_at\":\"{LARGE_UNIX_NANOS}\"")));
    assert_eq!(decode_party_evaluation_job_state(&job_bytes).unwrap(), job);

    let (staged, input) = job
        .stage(
            EvaluatedPartyKind::Person,
            "Ada Lovelace",
            7,
            LARGE_UNIX_NANOS + 1,
        )
        .unwrap();
    assert_eq!(
        decode_party_evaluation_job_state(&encode_party_evaluation_job_state(&staged).unwrap())
            .unwrap(),
        staged
    );
    let input_bytes = encode_party_evaluation_input_state(&input).unwrap();
    let input_text = std::str::from_utf8(&input_bytes).unwrap();
    assert!(input_text.contains(&format!(
        "\"captured_at\":\"{}\"",
        LARGE_UNIX_NANOS + 1
    )));
    assert_eq!(
        decode_party_evaluation_input_state(&input_bytes).unwrap(),
        input
    );

    let noncanonical = job_text.replace(
        &format!("\"created_at\":\"{LARGE_UNIX_NANOS}\""),
        &format!("\"created_at\":\"0{LARGE_UNIX_NANOS}\""),
    );
    assert!(decode_party_evaluation_job_state(noncanonical.as_bytes()).is_err());
}
