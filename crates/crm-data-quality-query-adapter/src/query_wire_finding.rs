fn finding_to_wire_with_visibility(finding: &PartyFinding, version: i64, visibility: &QueryVisibilityDecision) -> wire::DataQualityFinding {
    let mut output = wire::DataQualityFinding {
        finding_ref: Some(wire::DataQualityFindingRef { finding_id: finding.finding_id().to_owned() }),
        party_ref: Some(customer::PartyRef { party_id: finding.party_id().as_str().to_owned() }),
        rule_set_version_ref: Some(wire::PartyRuleSetVersionRef { rule_set_version_id: finding.rule_set_version_id().to_owned() }),
        rule_key: finding.rule_key().as_str().to_owned(),
        severity: severity_to_wire(finding.severity()),
        status: status_to_wire(finding.status()),
        current_observation_ref: Some(wire::DataQualityFindingObservationRef { finding_observation_id: finding.current_observation_id().to_owned() }),
        evaluated_party_resource_version: Some(customer::CustomerResourceVersion { version: finding.evaluated_party_resource_version(), created_at: None, updated_at: None }),
        assigned_actor_id: finding.assigned_actor_id().map(|value| value.as_str().to_owned()),
        waiver_reason: finding.waiver_reason().map(str::to_owned),
        created_at: Some(core::UnixTime { unix_nanos: finding.created_at() }),
        updated_at: Some(core::UnixTime { unix_nanos: finding.updated_at() }),
        resource_version: Some(customer::CustomerResourceVersion {
            version,
            created_at: Some(core::UnixTime { unix_nanos: finding.created_at() }),
            updated_at: Some(core::UnixTime { unix_nanos: finding.updated_at() }),
        }),
        remediated_by_rule_outcome_ref: finding.remediated_by_rule_outcome_id().map(|value| wire::PartyRuleOutcomeRef { rule_outcome_id: value.to_owned() }),
    };
    if !visibility.allows_field("party_ref") { output.party_ref = None; }
    if !visibility.allows_field("rule_set_version_ref") { output.rule_set_version_ref = None; }
    if !visibility.allows_field("rule_key") { output.rule_key.clear(); }
    if !visibility.allows_field("severity") { output.severity = wire::QualitySeverity::Unspecified as i32; }
    if !visibility.allows_field("status") { output.status = wire::DataQualityFindingStatus::Unspecified as i32; }
    if !visibility.allows_field("current_observation_ref") { output.current_observation_ref = None; }
    if !visibility.allows_field("evaluated_party_resource_version") { output.evaluated_party_resource_version = None; }
    if !visibility.allows_field("assigned_actor_id") { output.assigned_actor_id = None; }
    if !visibility.allows_field("waiver_reason") { output.waiver_reason = None; }
    if !visibility.allows_field("created_at") { output.created_at = None; }
    if !visibility.allows_field("updated_at") { output.updated_at = None; }
    if !visibility.allows_field("resource_version") { output.resource_version = None; }
    if !visibility.allows_field("remediated_by_rule_outcome_ref") { output.remediated_by_rule_outcome_ref = None; }
    output
}

fn observation_to_wire_with_visibility(observation: &PartyFindingObservation, visibility: &QueryVisibilityDecision) -> wire::DataQualityFindingObservation {
    let mut output = wire::DataQualityFindingObservation {
        finding_observation_ref: Some(wire::DataQualityFindingObservationRef { finding_observation_id: observation.observation_id().to_owned() }),
        finding_ref: Some(wire::DataQualityFindingRef { finding_id: observation.finding_id().to_owned() }),
        party_ref: Some(customer::PartyRef { party_id: observation.party_id().as_str().to_owned() }),
        rule_set_version_ref: Some(wire::PartyRuleSetVersionRef { rule_set_version_id: observation.rule_set_version_id().to_owned() }),
        rule_key: observation.rule_key().as_str().to_owned(),
        evaluated_party_resource_version: Some(customer::CustomerResourceVersion { version: observation.party_resource_version(), created_at: None, updated_at: None }),
        reason_code: observation.reason_code().to_owned(),
        observed_at: Some(core::UnixTime { unix_nanos: observation.observed_at() }),
    };
    if !visibility.allows_field("finding_ref") { output.finding_ref = None; }
    if !visibility.allows_field("party_ref") { output.party_ref = None; }
    if !visibility.allows_field("rule_set_version_ref") { output.rule_set_version_ref = None; }
    if !visibility.allows_field("rule_key") { output.rule_key.clear(); }
    if !visibility.allows_field("evaluated_party_resource_version") { output.evaluated_party_resource_version = None; }
    if !visibility.allows_field("reason_code") { output.reason_code.clear(); }
    if !visibility.allows_field("observed_at") { output.observed_at = None; }
    output
}
