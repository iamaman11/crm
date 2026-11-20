impl DataQualityQueryAdapter {
    async fn execute_list_findings_by_party(&self, request: &QueryRequest) -> Result<TypedPayload, SdkError> {
        let command: wire::ListDataQualityFindingsByPartyRequest = decode_input(request, LIST_FINDINGS_BY_PARTY_REQUEST_SCHEMA, DataClass::Personal, "DATA_QUALITY_FINDING_QUERY_INPUT")?;
        let party_id = party_record_id(command.party_ref)?;
        let status = validated_status_filter(command.status)?;
        let severity = validated_severity_filter(command.severity)?;
        let page_size = self.page_policy.resolve(command.page_size).map_err(cursor_error)?;
        let filter = FindingFilter { party_id: Some(party_id), assigned_actor_id: None, status, severity };
        let binding = finding_cursor_binding(request, filter.hash(), page_size)?;
        let after = decode_finding_after(self, &command.cursor, &binding)?;
        let (findings, next) = self.collect_findings(request, page_size, after, &filter).await?;
        let next_cursor = encode_finding_next(self, &binding, next.as_ref())?;
        support::protobuf_payload(MODULE_ID, LIST_FINDINGS_BY_PARTY_RESPONSE_SCHEMA, DataClass::Personal, &wire::ListDataQualityFindingsByPartyResponse { findings, next_cursor })
    }

    async fn execute_list_assigned_findings(&self, request: &QueryRequest) -> Result<TypedPayload, SdkError> {
        let command: wire::ListAssignedDataQualityFindingsRequest = decode_input(request, LIST_ASSIGNED_FINDINGS_REQUEST_SCHEMA, DataClass::Personal, "DATA_QUALITY_FINDING_QUERY_INPUT")?;
        let assigned_actor_id = assigned_actor_filter(command.assigned_actor_id, request.context.actor_id.as_str())?;
        let status = validated_status_filter(command.status)?;
        let severity = validated_severity_filter(command.severity)?;
        let page_size = self.page_policy.resolve(command.page_size).map_err(cursor_error)?;
        let filter = FindingFilter { party_id: None, assigned_actor_id: Some(assigned_actor_id), status, severity };
        let binding = finding_cursor_binding(request, filter.hash(), page_size)?;
        let after = decode_finding_after(self, &command.cursor, &binding)?;
        let (findings, next) = self.collect_findings(request, page_size, after, &filter).await?;
        let next_cursor = encode_finding_next(self, &binding, next.as_ref())?;
        support::protobuf_payload(MODULE_ID, LIST_ASSIGNED_FINDINGS_RESPONSE_SCHEMA, DataClass::Personal, &wire::ListAssignedDataQualityFindingsResponse { findings, next_cursor })
    }

    async fn execute_get_party_completeness_result(&self, request: &QueryRequest) -> Result<TypedPayload, SdkError> {
        let command: wire::GetPartyCompletenessResultRequest = decode_input(request, GET_PARTY_COMPLETENESS_RESULT_REQUEST_SCHEMA, DataClass::Personal, "DATA_QUALITY_COMPLETENESS_RESULT_QUERY_INPUT")?;
        let result_id = completeness_result_record_id(command.completeness_result_ref)?;
        let snapshot = self.load_snapshot(request, PARTY_COMPLETENESS_RESULT_RECORD_TYPE, result_id.as_str().to_owned(), completeness_result_not_found).await?;
        let visibility = self.visible_or(&snapshot, request, completeness_result_not_found).await?;
        let result = completeness_result_from_snapshot(&snapshot)?;
        let output = completeness_result_to_wire_with_visibility(&result, snapshot.version, &visibility);
        support::protobuf_payload(MODULE_ID, GET_PARTY_COMPLETENESS_RESULT_RESPONSE_SCHEMA, DataClass::Personal, &wire::GetPartyCompletenessResultResponse { completeness_result: Some(output) })
    }
}
