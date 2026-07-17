#[derive(Debug, Clone)]
struct FindingFilter {
    party_id: Option<RecordId>,
    assigned_actor_id: Option<ActorId>,
    status: Option<PartyFindingStatus>,
    severity: Option<QualitySeverity>,
}

impl FindingFilter {
    fn matches(&self, finding: &PartyFinding) -> bool {
        self.party_id.as_ref().is_none_or(|party_id| party_id == finding.party_id())
            && self.assigned_actor_id.as_ref().is_none_or(|actor_id| {
                finding.assigned_actor_id().is_some_and(|value| value == actor_id)
            })
            && self.status.is_none_or(|status| finding.status() == status)
            && self.severity.is_none_or(|severity| finding.severity() == severity)
    }

    fn hash(&self) -> [u8; 32] {
        let status = status_filter_wire(self.status).to_be_bytes();
        let severity = severity_filter_wire(self.severity).to_be_bytes();
        normalized_filter_hash([
            ("party_id", self.party_id.as_ref().map_or(&[][..], |value| value.as_str().as_bytes())),
            ("assigned_actor_id", self.assigned_actor_id.as_ref().map_or(&[][..], |value| value.as_str().as_bytes())),
            ("status", status.as_slice()),
            ("severity", severity.as_slice()),
        ])
    }
}

pub fn query_capability_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {
    Ok(vec![
        rule_set_query_capability_definition()?,
        completeness_profile_query_capability_definition()?,
        evaluation_job_query_capability_definition()?,
        finding_query_capability_definition()?,
        list_findings_by_party_query_capability_definition()?,
        list_assigned_findings_query_capability_definition()?,
        completeness_result_query_capability_definition()?,
    ])
}

pub fn query_capability_definition() -> Result<CapabilityDefinition, SdkError> { rule_set_query_capability_definition() }

pub fn rule_set_query_capability_definition() -> Result<CapabilityDefinition, SdkError> {
    query_definition(GET_PARTY_RULE_SET_CAPABILITY, GET_PARTY_RULE_SET_REQUEST_SCHEMA, GET_PARTY_RULE_SET_RESPONSE_SCHEMA, DataClass::Confidential)
}

pub fn completeness_profile_query_capability_definition() -> Result<CapabilityDefinition, SdkError> {
    query_definition(GET_PARTY_COMPLETENESS_PROFILE_CAPABILITY, GET_PARTY_COMPLETENESS_PROFILE_REQUEST_SCHEMA, GET_PARTY_COMPLETENESS_PROFILE_RESPONSE_SCHEMA, DataClass::Confidential)
}

pub fn evaluation_job_query_capability_definition() -> Result<CapabilityDefinition, SdkError> {
    query_definition(GET_PARTY_EVALUATION_JOB_CAPABILITY, GET_PARTY_EVALUATION_JOB_REQUEST_SCHEMA, GET_PARTY_EVALUATION_JOB_RESPONSE_SCHEMA, DataClass::Personal)
}

pub fn finding_query_capability_definition() -> Result<CapabilityDefinition, SdkError> {
    query_definition(GET_FINDING_CAPABILITY, GET_FINDING_REQUEST_SCHEMA, GET_FINDING_RESPONSE_SCHEMA, DataClass::Personal)
}

pub fn list_findings_by_party_query_capability_definition() -> Result<CapabilityDefinition, SdkError> {
    query_definition(LIST_FINDINGS_BY_PARTY_CAPABILITY, LIST_FINDINGS_BY_PARTY_REQUEST_SCHEMA, LIST_FINDINGS_BY_PARTY_RESPONSE_SCHEMA, DataClass::Personal)
}

pub fn list_assigned_findings_query_capability_definition() -> Result<CapabilityDefinition, SdkError> {
    query_definition(LIST_ASSIGNED_FINDINGS_CAPABILITY, LIST_ASSIGNED_FINDINGS_REQUEST_SCHEMA, LIST_ASSIGNED_FINDINGS_RESPONSE_SCHEMA, DataClass::Personal)
}

pub fn completeness_result_query_capability_definition() -> Result<CapabilityDefinition, SdkError> {
    query_definition(GET_PARTY_COMPLETENESS_RESULT_CAPABILITY, GET_PARTY_COMPLETENESS_RESULT_REQUEST_SCHEMA, GET_PARTY_COMPLETENESS_RESULT_RESPONSE_SCHEMA, DataClass::Personal)
}

fn query_definition(capability_id: &'static str, input_schema: &'static str, output_schema: &'static str, data_class: DataClass) -> Result<CapabilityDefinition, SdkError> {
    Ok(CapabilityDefinition {
        capability_id: support::configured_identifier(CapabilityId::try_new(capability_id))?,
        capability_version: support::configured_identifier(CapabilityVersion::try_new(support::CONTRACT_VERSION))?,
        owner_module_id: support::configured_identifier(ModuleId::try_new(MODULE_ID))?,
        input_contract: support::protobuf_contract(MODULE_ID, input_schema, vec![data_class])?,
        output_contract: Some(support::protobuf_contract(MODULE_ID, output_schema, vec![data_class])?),
        risk: CapabilityRisk::Low,
        mutation: false,
        requires_idempotency: false,
        requires_approval: false,
        authorization_policy_id: capability_id.to_owned(),
        rate_limit_policy_id: None,
    })
}
