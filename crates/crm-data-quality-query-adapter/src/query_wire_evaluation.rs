fn evaluation_job_to_wire_with_visibility(
    job: &PartyEvaluationJob,
    version: i64,
    visibility: &QueryVisibilityDecision,
) -> wire::PartyEvaluationJob {
    let mut output = party_evaluation_job_to_wire(job, version);
    if !visibility.allows_field("party_ref") { output.party_ref = None; }
    if !visibility.allows_field("rule_set_version_ref") { output.rule_set_version_ref = None; }
    if !visibility.allows_field("completeness_profile_version_ref") { output.completeness_profile_version_ref = None; }
    if !visibility.allows_field("status") { output.status = wire::PartyEvaluationJobStatus::Unspecified as i32; }
    if !visibility.allows_field("evaluated_party_resource_version") { output.evaluated_party_resource_version = None; }
    if !visibility.allows_field("evaluated_rules") { output.evaluated_rules = 0; }
    if !visibility.allows_field("failed_rules") { output.failed_rules = 0; }
    if !visibility.allows_field("created_at") { output.created_at = None; }
    if !visibility.allows_field("updated_at") { output.updated_at = None; }
    if !visibility.allows_field("resource_version") { output.resource_version = None; }
    output
}
