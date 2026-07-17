use crm_data_quality::{
    CANONICALIZATION_PROFILE_ID, ComponentKey, EvaluatedPartyKind, PartyCompletenessComponent,
    PartyCompletenessProfileVersion, PartyQualityEvaluator, PartyQualityInput, PartyQualityRule,
    PartyRuleSetVersion, QualitySeverity, RuleKey, decode_party_completeness_profile_version_state,
    decode_party_rule_set_version_state, encode_party_completeness_profile_version_state,
    encode_party_rule_set_version_state,
};

const RULE_SET_ID: &str =
    "dq-party-rule-set-cc9379d29bffef0f0f78260c025d8765e3c6135504050c31ab931b0e137d91e3";
const PROFILE_ID: &str = "dq-party-completeness-profile-70d1e7f969c44866e7232a2df5bf290d360cf62fd604bf07dc6a966b9832b6d1";

fn key(value: &str) -> RuleKey {
    RuleKey::try_new(value).unwrap()
}

fn rule_set() -> PartyRuleSetVersion {
    PartyRuleSetVersion::publish(vec![
        PartyQualityRule::try_new(
            key("display_name.placeholder"),
            QualitySeverity::Error,
            PartyQualityEvaluator::display_name_placeholder_exact_ascii_casefold(vec![
                " UNKNOWN ".to_owned(),
                "N/A".to_owned(),
            ])
            .unwrap(),
            "Placeholder display name",
            "Replace the placeholder with the real customer name.",
        )
        .unwrap(),
        PartyQualityRule::try_new(
            key("display_name.minimum"),
            QualitySeverity::Warning,
            PartyQualityEvaluator::display_name_min_utf8_bytes(4).unwrap(),
            "Display name length",
            "Replace the display name with a meaningful customer name.",
        )
        .unwrap(),
    ])
    .unwrap()
}

fn profile(rule_set: &PartyRuleSetVersion) -> PartyCompletenessProfileVersion {
    PartyCompletenessProfileVersion::publish(
        rule_set,
        vec![
            PartyCompletenessComponent::try_new(
                ComponentKey::try_new("display_name.placeholder").unwrap(),
                key("display_name.placeholder"),
                6_000,
            )
            .unwrap(),
            PartyCompletenessComponent::try_new(
                ComponentKey::try_new("display_name.minimum").unwrap(),
                key("display_name.minimum"),
                4_000,
            )
            .unwrap(),
        ],
    )
    .unwrap()
}

#[test]
fn v1_identities_are_bound_to_the_explicit_canonicalization_profile() {
    assert_eq!(CANONICALIZATION_PROFILE_ID, "crm.cjson/v1");
    let rule_set = rule_set();
    assert_eq!(rule_set.version_id().as_str(), RULE_SET_ID);
    let profile = profile(&rule_set);
    assert_eq!(profile.version_id().as_str(), PROFILE_ID);
    assert_eq!(profile.rule_set_version_id().as_str(), RULE_SET_ID);
}

#[test]
fn persisted_definitions_round_trip_with_profile_beside_identity() {
    let rule_set = rule_set();
    let rule_bytes = encode_party_rule_set_version_state(&rule_set).unwrap();
    assert!(
        String::from_utf8_lossy(&rule_bytes)
            .contains("\"canonicalization_profile\":\"crm.cjson/v1\"")
    );
    let restored_rule_set = decode_party_rule_set_version_state(&rule_bytes).unwrap();
    assert_eq!(restored_rule_set, rule_set);
    assert_eq!(
        encode_party_rule_set_version_state(&restored_rule_set).unwrap(),
        rule_bytes
    );

    let profile = profile(&rule_set);
    let profile_bytes = encode_party_completeness_profile_version_state(&profile).unwrap();
    assert!(
        String::from_utf8_lossy(&profile_bytes)
            .contains("\"canonicalization_profile\":\"crm.cjson/v1\"")
    );
    let restored_profile =
        decode_party_completeness_profile_version_state(&profile_bytes, &restored_rule_set)
            .unwrap();
    assert_eq!(restored_profile, profile);
    assert_eq!(
        encode_party_completeness_profile_version_state(&restored_profile).unwrap(),
        profile_bytes
    );
}

#[test]
fn evaluator_and_integer_scoring_semantics_remain_exact() {
    let rule_set = rule_set();
    let outcomes = rule_set
        .evaluate(&PartyQualityInput::try_new(EvaluatedPartyKind::Person, "Unknown").unwrap());
    assert_eq!(outcomes.len(), 2);
    assert!(outcomes[0].passed());
    assert!(!outcomes[1].passed());
    let score = profile(&rule_set).score(&outcomes).unwrap();
    assert_eq!(score.score_basis_points(), 4_000);
    assert_eq!(
        score
            .awards()
            .iter()
            .map(|award| award.awarded_basis_points())
            .sum::<u32>(),
        4_000
    );
}
