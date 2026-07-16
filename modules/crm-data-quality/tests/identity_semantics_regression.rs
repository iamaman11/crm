use crm_data_quality::{
    ComponentKey, EvaluatedPartyKind, PartyCompletenessComponent,
    PartyCompletenessProfileVersion, PartyQualityEvaluator, PartyQualityInput, PartyQualityRule,
    PartyRuleSetVersion, QualitySeverity, RuleKey, decode_party_completeness_profile_version_state,
    decode_party_rule_set_version_state, encode_party_completeness_profile_version_state,
    encode_party_rule_set_version_state,
};

const EXPECTED_RULE_SET_VERSION_ID: &str =
    "dq-party-rule-set-b40f7aecbc7fc18101d674e5d941a9fc6dfdf9c1d0827565c7f32a670c894036";
const EXPECTED_COMPLETENESS_PROFILE_VERSION_ID: &str =
    "dq-party-completeness-profile-79ee0692769de52723abe5b51330e3f3dc366ddb626223f9087853e443bfefe5";

fn rule_key(value: &str) -> RuleKey {
    RuleKey::try_new(value).expect("valid regression rule key")
}

fn component_key(value: &str) -> ComponentKey {
    ComponentKey::try_new(value).expect("valid regression component key")
}

fn minimum_rule() -> PartyQualityRule {
    PartyQualityRule::try_new(
        rule_key("display_name.minimum"),
        QualitySeverity::Warning,
        PartyQualityEvaluator::display_name_min_utf8_bytes(4)
            .expect("valid minimum evaluator"),
        "Display name length",
        "Replace the display name with a meaningful customer name.",
    )
    .expect("valid minimum rule")
}

fn placeholder_rule(tokens: &[&str]) -> PartyQualityRule {
    PartyQualityRule::try_new(
        rule_key("display_name.placeholder"),
        QualitySeverity::Error,
        PartyQualityEvaluator::display_name_placeholder_exact_ascii_casefold(
            tokens.iter().map(|value| (*value).to_owned()).collect(),
        )
        .expect("valid placeholder evaluator"),
        "Placeholder display name",
        "Replace the placeholder with the real customer name.",
    )
    .expect("valid placeholder rule")
}

fn rule_set() -> PartyRuleSetVersion {
    PartyRuleSetVersion::publish(vec![
        placeholder_rule(&[" UNKNOWN ", "N/A"]),
        minimum_rule(),
    ])
    .expect("valid canonical regression rule set")
}

fn completeness_profile(rule_set: &PartyRuleSetVersion) -> PartyCompletenessProfileVersion {
    PartyCompletenessProfileVersion::publish(
        rule_set,
        vec![
            PartyCompletenessComponent::try_new(
                component_key("display_name.placeholder"),
                rule_key("display_name.placeholder"),
                6_000,
            )
            .expect("valid placeholder completeness component"),
            PartyCompletenessComponent::try_new(
                component_key("display_name.minimum"),
                rule_key("display_name.minimum"),
                4_000,
            )
            .expect("valid minimum completeness component"),
        ],
    )
    .expect("valid canonical regression completeness profile")
}

#[test]
fn published_v1_identity_fixtures_cannot_be_silently_reinterpreted() {
    let rule_set = rule_set();
    assert_eq!(
        rule_set.version_id().as_str(),
        EXPECTED_RULE_SET_VERSION_ID,
        "changing this fixture requires a new explicitly versioned identity/canonicalization contract"
    );

    let profile = completeness_profile(&rule_set);
    assert_eq!(
        profile.version_id().as_str(),
        EXPECTED_COMPLETENESS_PROFILE_VERSION_ID,
        "changing this fixture requires a new explicitly versioned identity/canonicalization contract"
    );
    assert_eq!(
        profile.rule_set_version_id().as_str(),
        EXPECTED_RULE_SET_VERSION_ID
    );
}

#[test]
fn strict_persisted_definition_round_trip_preserves_exact_identity_and_order() {
    let rule_set = rule_set();
    let rule_set_bytes =
        encode_party_rule_set_version_state(&rule_set).expect("encode regression rule set");
    let restored_rule_set = decode_party_rule_set_version_state(&rule_set_bytes)
        .expect("strictly decode regression rule set");
    assert_eq!(restored_rule_set.version_id(), rule_set.version_id());
    assert_eq!(restored_rule_set.rules(), rule_set.rules());
    assert_eq!(
        encode_party_rule_set_version_state(&restored_rule_set)
            .expect("re-encode regression rule set"),
        rule_set_bytes
    );

    let profile = completeness_profile(&rule_set);
    let profile_bytes = encode_party_completeness_profile_version_state(&profile)
        .expect("encode regression completeness profile");
    let restored_profile =
        decode_party_completeness_profile_version_state(&profile_bytes, &restored_rule_set)
            .expect("strictly decode regression completeness profile");
    assert_eq!(restored_profile.version_id(), profile.version_id());
    assert_eq!(restored_profile.components(), profile.components());
    assert_eq!(
        encode_party_completeness_profile_version_state(&restored_profile)
            .expect("re-encode regression completeness profile"),
        profile_bytes
    );
}

#[test]
fn evaluator_and_integer_completeness_semantics_remain_exact() {
    let rule_set = rule_set();
    let profile = completeness_profile(&rule_set);
    let input = PartyQualityInput::try_new(EvaluatedPartyKind::Person, "Unknown")
        .expect("valid canonical Party quality input");

    let outcomes = rule_set.evaluate(&input);
    assert_eq!(outcomes.len(), 2);
    assert_eq!(outcomes[0].rule_key().as_str(), "display_name.minimum");
    assert!(outcomes[0].passed());
    assert_eq!(outcomes[0].reason_code(), "DATA_QUALITY_RULE_PASSED");
    assert_eq!(outcomes[1].rule_key().as_str(), "display_name.placeholder");
    assert!(!outcomes[1].passed());
    assert_eq!(
        outcomes[1].reason_code(),
        "DATA_QUALITY_PARTY_DISPLAY_NAME_PLACEHOLDER"
    );

    let score = profile
        .score(&outcomes)
        .expect("exact completeness reconciliation");
    assert_eq!(score.score_basis_points(), 4_000);
    assert_eq!(score.awards().len(), 2);
    assert_eq!(
        score.awards()[0].component_key().as_str(),
        "display_name.minimum"
    );
    assert_eq!(score.awards()[0].awarded_basis_points(), 4_000);
    assert_eq!(
        score.awards()[1].component_key().as_str(),
        "display_name.placeholder"
    );
    assert_eq!(score.awards()[1].awarded_basis_points(), 0);
    assert_eq!(
        score
            .awards()
            .iter()
            .map(|award| award.awarded_basis_points())
            .sum::<u32>(),
        score.score_basis_points()
    );
}
