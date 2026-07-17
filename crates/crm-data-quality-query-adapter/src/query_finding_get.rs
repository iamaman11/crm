impl DataQualityQueryAdapter {
    async fn execute_get_finding(
        &self,
        request: &QueryRequest,
    ) -> Result<TypedPayload, SdkError> {
        let command: wire::GetDataQualityFindingRequest = decode_input(
            request,
            GET_FINDING_REQUEST_SCHEMA,
            DataClass::Personal,
            "DATA_QUALITY_FINDING_QUERY_INPUT",
        )?;
        let finding_id = finding_record_id(command.finding_ref)?;
        let snapshot = self
            .load_snapshot(
                request,
                FINDING_RECORD_TYPE,
                finding_id.as_str().to_owned(),
                finding_not_found,
            )
            .await?;
        let visibility = self.visible_or(&snapshot, request, finding_not_found).await?;
        let finding = finding_from_snapshot(&snapshot)?;
        let finding_output =
            finding_to_wire_with_visibility(&finding, snapshot.version, &visibility);
        let observation_output = if visibility.allows_field("current_observation") {
            let observation_snapshot = self
                .load_snapshot(
                    request,
                    FINDING_OBSERVATION_RECORD_TYPE,
                    finding.current_observation_id().to_owned(),
                    persisted_observation_missing,
                )
                .await?;
            let observation_visibility = self
                .data_quality_visibility(request, &observation_snapshot.reference)
                .await?;
            if observation_visibility.resource_visible {
                let observation = observation_from_snapshot(&observation_snapshot)?;
                Some(observation_to_wire_with_visibility(
                    &observation,
                    &observation_visibility,
                ))
            } else {
                None
            }
        } else {
            None
        };
        support::protobuf_payload(
            MODULE_ID,
            GET_FINDING_RESPONSE_SCHEMA,
            DataClass::Personal,
            &wire::GetDataQualityFindingResponse {
                finding: Some(finding_output),
                current_observation: observation_output,
            },
        )
    }
}
