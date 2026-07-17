fn completeness_result_to_wire_with_visibility(
    result: &PartyCompletenessResult,
    version: i64,
    visibility: &QueryVisibilityDecision,
) -> wire::PartyCompletenessResult {
    let mut output = wire::PartyCompletenessResult {
        completeness_result_ref: Some(wire::PartyCompletenessResultRef { completeness_result_id: result.result_id().to_owned() }),
        evaluation_job_ref: Some(wire::PartyEvaluationJobRef { evaluation_job_id: result.job_id().as_str().to_owned() }),
        party_ref: Some(customer::PartyRef { party_id: result.party_id().as_str().to_owned() }),
        evaluated_party_resource_version: Some(customer::CustomerResourceVersion { version: result.party_resource_version(), created_at: None, updated_at: None }),
        completeness_profile_version_ref: Some(wire::PartyCompletenessProfileVersionRef { completeness_profile_version_id: result.profile_version_id().to_owned() }),
        score_basis_points: result.score_basis_points(),
        components: result.components().iter().map(|component| wire::PartyCompletenessComponentResult {
            component_key: component.component_key().as_str().to_owned(),
            rule_key: component.rule_key().as_str().to_owned(),
            rule_outcome_ref: Some(wire::PartyRuleOutcomeRef { rule_outcome_id: component.rule_outcome_id().to_owned() }),
            awarded_basis_points: component.awarded_basis_points(),
        }).collect(),
        computed_at: Some(core::UnixTime { unix_nanos: result.computed_at() }),
        resource_version: Some(customer::CustomerResourceVersion {
            version,
            created_at: Some(core::UnixTime { unix_nanos: result.computed_at() }),
            updated_at: Some(core::UnixTime { unix_nanos: result.computed_at() }),
        }),
    };
    if !visibility.allows_field("evaluation_job_ref") { output.evaluation_job_ref = None; }
    if !visibility.allows_field("party_ref") { output.party_ref = None; }
    if !visibility.allows_field("evaluated_party_resource_version") { output.evaluated_party_resource_version = None; }
    if !visibility.allows_field("completeness_profile_version_ref") { output.completeness_profile_version_ref = None; }
    if !visibility.allows_field("score_basis_points") { output.score_basis_points = 0; }
    if !visibility.allows_field("components") { output.components.clear(); }
    if !visibility.allows_field("computed_at") { output.computed_at = None; }
    if !visibility.allows_field("resource_version") { output.resource_version = None; }
    output
}
