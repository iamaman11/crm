impl QuerySemanticValidator for DataQualityQueryAdapter {
    fn validate<'a>(&'a self, definition: &'a CapabilityDefinition, request: &'a QueryRequest) -> PortFuture<'a, Result<(), SdkError>> {
        Box::pin(async move {
            ensure_definition(definition)?;
            match definition.capability_id.as_str() {
                GET_PARTY_RULE_SET_CAPABILITY => {
                    let command: wire::GetPartyRuleSetVersionRequest = decode_input(request, GET_PARTY_RULE_SET_REQUEST_SCHEMA, DataClass::Confidential, "DATA_QUALITY_RULE_SET_QUERY_INPUT")?;
                    validate_record_id(required_rule_set_ref(command.rule_set_version_ref)?.rule_set_version_id, "data_quality.party_rule_set.rule_set_version_ref.rule_set_version_id")
                }
                GET_PARTY_COMPLETENESS_PROFILE_CAPABILITY => {
                    let command: wire::GetPartyCompletenessProfileVersionRequest = decode_input(request, GET_PARTY_COMPLETENESS_PROFILE_REQUEST_SCHEMA, DataClass::Confidential, "DATA_QUALITY_COMPLETENESS_PROFILE_QUERY_INPUT")?;
                    validate_record_id(required_completeness_profile_ref(command.completeness_profile_version_ref)?.completeness_profile_version_id, "data_quality.party_completeness_profile.completeness_profile_version_ref.completeness_profile_version_id")
                }
                GET_PARTY_EVALUATION_JOB_CAPABILITY => {
                    let command: wire::GetPartyEvaluationJobRequest = decode_input(request, GET_PARTY_EVALUATION_JOB_REQUEST_SCHEMA, DataClass::Personal, "DATA_QUALITY_EVALUATION_QUERY_INPUT")?;
                    validate_record_id(required_evaluation_job_ref(command.evaluation_job_ref)?.evaluation_job_id, "data_quality.party_evaluation.evaluation_job_ref.evaluation_job_id")
                }
                GET_FINDING_CAPABILITY => {
                    let command: wire::GetDataQualityFindingRequest = decode_input(request, GET_FINDING_REQUEST_SCHEMA, DataClass::Personal, "DATA_QUALITY_FINDING_QUERY_INPUT")?;
                    finding_record_id(command.finding_ref).map(|_| ())
                }
                LIST_FINDINGS_BY_PARTY_CAPABILITY => {
                    let command: wire::ListDataQualityFindingsByPartyRequest = decode_input(request, LIST_FINDINGS_BY_PARTY_REQUEST_SCHEMA, DataClass::Personal, "DATA_QUALITY_FINDING_QUERY_INPUT")?;
                    let filter = FindingFilter { party_id: Some(party_record_id(command.party_ref)?), assigned_actor_id: None, status: validated_status_filter(command.status)?, severity: validated_severity_filter(command.severity)? };
                    let page_size = self.page_policy.resolve(command.page_size).map_err(cursor_error)?;
                    let binding = finding_cursor_binding(request, filter.hash(), page_size)?;
                    decode_finding_after(self, &command.cursor, &binding).map(|_| ())
                }
                LIST_ASSIGNED_FINDINGS_CAPABILITY => {
                    let command: wire::ListAssignedDataQualityFindingsRequest = decode_input(request, LIST_ASSIGNED_FINDINGS_REQUEST_SCHEMA, DataClass::Personal, "DATA_QUALITY_FINDING_QUERY_INPUT")?;
                    let filter = FindingFilter { party_id: None, assigned_actor_id: Some(assigned_actor_filter(command.assigned_actor_id, request.context.actor_id.as_str())?), status: validated_status_filter(command.status)?, severity: validated_severity_filter(command.severity)? };
                    let page_size = self.page_policy.resolve(command.page_size).map_err(cursor_error)?;
                    let binding = finding_cursor_binding(request, filter.hash(), page_size)?;
                    decode_finding_after(self, &command.cursor, &binding).map(|_| ())
                }
                GET_PARTY_COMPLETENESS_RESULT_CAPABILITY => {
                    let command: wire::GetPartyCompletenessResultRequest = decode_input(request, GET_PARTY_COMPLETENESS_RESULT_REQUEST_SCHEMA, DataClass::Personal, "DATA_QUALITY_COMPLETENESS_RESULT_QUERY_INPUT")?;
                    completeness_result_record_id(command.completeness_result_ref).map(|_| ())
                }
                _ => Err(unsupported_query()),
            }
        })
    }
}

impl QueryExecutor for DataQualityQueryAdapter {
    fn execute<'a>(&'a self, definition: &'a CapabilityDefinition, request: QueryRequest) -> PortFuture<'a, Result<QueryExecutionResult, SdkError>> {
        Box::pin(async move {
            ensure_definition(definition)?;
            let output = match definition.capability_id.as_str() {
                GET_PARTY_RULE_SET_CAPABILITY => self.execute_get_party_rule_set(&request).await?,
                GET_PARTY_COMPLETENESS_PROFILE_CAPABILITY => self.execute_get_party_completeness_profile(&request).await?,
                GET_PARTY_EVALUATION_JOB_CAPABILITY => self.execute_get_party_evaluation_job(&request).await?,
                GET_FINDING_CAPABILITY => self.execute_get_finding(&request).await?,
                LIST_FINDINGS_BY_PARTY_CAPABILITY => self.execute_list_findings_by_party(&request).await?,
                LIST_ASSIGNED_FINDINGS_CAPABILITY => self.execute_list_assigned_findings(&request).await?,
                GET_PARTY_COMPLETENESS_RESULT_CAPABILITY => self.execute_get_party_completeness_result(&request).await?,
                _ => return Err(unsupported_query()),
            };
            Ok(QueryExecutionResult { output })
        })
    }
}
