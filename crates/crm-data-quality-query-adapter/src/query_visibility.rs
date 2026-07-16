use crm_module_sdk::RecordRef;
use std::collections::BTreeSet;

impl DataQualityQueryAdapter {
    async fn data_quality_visibility(
        &self,
        request: &QueryRequest,
        resource: &RecordRef,
    ) -> Result<QueryVisibilityDecision, SdkError> {
        let exact = self
            .visibility
            .authorize_visibility(request, resource)
            .await?;
        if exact.resource_visible {
            return Ok(exact);
        }
        let proxy = support::record_ref(
            PARTY_RULE_SET_VERSION_RECORD_TYPE,
            resource.record_id.as_str(),
            "data_quality.visibility_proxy.record_id",
        )?;
        let owner_visibility = self
            .visibility
            .authorize_visibility(request, &proxy)
            .await?;
        if !owner_visibility.resource_visible {
            return Ok(owner_visibility);
        }
        let mut allowed_fields = visible_fields(
            request.context.capability_id.as_str(),
            resource.record_type.as_str(),
        );
        apply_deployment_hidden_fields(
            request.context.capability_id.as_str(),
            resource.record_type.as_str(),
            &mut allowed_fields,
        );
        Ok(QueryVisibilityDecision {
            resource_visible: true,
            allowed_fields,
            decision_id: format!(
                "{}:dq-owner-projection:{}",
                owner_visibility.decision_id, resource.record_type
            ),
            policy_version: format!(
                "{}+data-quality-owner-projection/v1",
                owner_visibility.policy_version
            ),
        })
    }
}

fn visible_fields(capability_id: &str, record_type: &str) -> BTreeSet<String> {
    let fields: &[&str] = match (capability_id, record_type) {
        (GET_PARTY_RULE_SET_CAPABILITY, PARTY_RULE_SET_VERSION_RECORD_TYPE)
        | (
            GET_PARTY_COMPLETENESS_PROFILE_CAPABILITY,
            PARTY_RULE_SET_VERSION_RECORD_TYPE,
        ) => &["definition"],
        (GET_PARTY_EVALUATION_JOB_CAPABILITY, PARTY_EVALUATION_JOB_RECORD_TYPE) => &[
            "party_ref",
            "rule_set_version_ref",
            "completeness_profile_version_ref",
            "status",
            "evaluated_party_resource_version",
            "evaluated_rules",
            "failed_rules",
            "created_at",
            "updated_at",
            "resource_version",
        ],
        (GET_FINDING_CAPABILITY, FINDING_RECORD_TYPE)
        | (LIST_FINDINGS_BY_PARTY_CAPABILITY, FINDING_RECORD_TYPE)
        | (LIST_ASSIGNED_FINDINGS_CAPABILITY, FINDING_RECORD_TYPE) => &[
            "party_ref",
            "rule_set_version_ref",
            "rule_key",
            "severity",
            "status",
            "current_observation_ref",
            "current_observation",
            "evaluated_party_resource_version",
            "assigned_actor_id",
            "waiver_reason",
            "created_at",
            "updated_at",
            "resource_version",
            "remediated_by_rule_outcome_ref",
        ],
        (GET_FINDING_CAPABILITY, FINDING_OBSERVATION_RECORD_TYPE) => &[
            "finding_ref",
            "party_ref",
            "rule_set_version_ref",
            "rule_key",
            "evaluated_party_resource_version",
            "reason_code",
            "observed_at",
        ],
        (
            GET_PARTY_COMPLETENESS_RESULT_CAPABILITY,
            PARTY_COMPLETENESS_RESULT_RECORD_TYPE,
        ) => &[
            "evaluation_job_ref",
            "party_ref",
            "evaluated_party_resource_version",
            "completeness_profile_version_ref",
            "score_basis_points",
            "components",
            "computed_at",
            "resource_version",
        ],
        _ => &[],
    };
    fields.iter().map(|field| (*field).to_owned()).collect()
}

fn apply_deployment_hidden_fields(
    capability_id: &str,
    record_type: &str,
    allowed_fields: &mut BTreeSet<String>,
) {
    let Ok(value) = std::env::var("CRM_QUERY_HIDDEN_FIELDS") else {
        return;
    };
    for entry in value
        .split(',')
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
    {
        let mut parts = entry.split('|').map(str::trim);
        let Some(entry_capability) = parts.next() else {
            continue;
        };
        let Some(entry_owner) = parts.next() else {
            continue;
        };
        let Some(entry_record_type) = parts.next() else {
            continue;
        };
        let Some(field) = parts.next() else {
            continue;
        };
        if parts.next().is_none()
            && entry_capability == capability_id
            && entry_owner == MODULE_ID
            && entry_record_type == record_type
        {
            allowed_fields.remove(field);
        }
    }
}
