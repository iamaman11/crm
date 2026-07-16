impl DataQualityQueryAdapter {
    pub fn new(
        store: PostgresDataStore,
        visibility: Arc<dyn QueryVisibilityAuthorizer>,
    ) -> Self {
        let cursor_codec = CursorCodec::new(data_quality_cursor_key())
            .expect("the Data Quality cursor key is exactly 32 bytes");
        let party_query_adapter = PartyQueryAdapter::new(
            store.clone(),
            cursor_codec.clone(),
            visibility.clone(),
        )
        .expect("the governed Party query adapter configuration is valid");
        register_party_quality_query_adapter(Arc::new(party_query_adapter))
            .expect("the governed Party source registry is available");
        let page_policy = PageSizePolicy {
            default_size: DEFAULT_PAGE_SIZE,
            maximum_size: MAXIMUM_PAGE_SIZE,
        }
        .validate()
        .expect("the static Data Quality page policy is valid");
        Self {
            store,
            cursor_codec,
            visibility,
            page_policy,
        }
    }

    async fn execute_get_party_rule_set(
        &self,
        request: &QueryRequest,
    ) -> Result<TypedPayload, SdkError> {
        let command: wire::GetPartyRuleSetVersionRequest = decode_input(
            request,
            GET_PARTY_RULE_SET_REQUEST_SCHEMA,
            DataClass::Confidential,
            "DATA_QUALITY_RULE_SET_QUERY_INPUT",
        )?;
        let version_ref = required_rule_set_ref(command.rule_set_version_ref)?;
        let snapshot = self
            .load_snapshot(
                request,
                PARTY_RULE_SET_VERSION_RECORD_TYPE,
                version_ref.rule_set_version_id,
                rule_set_not_found,
            )
            .await?;
        let visibility = self.visible_or(&snapshot, request, rule_set_not_found).await?;
        let rule_set = party_rule_set_from_snapshot(&snapshot)?;
        let mut output = party_rule_set_to_wire(&rule_set);
        apply_definition_visibility(
            &mut output.definition,
            visibility.allows_field("definition"),
        );
        support::protobuf_payload(
            MODULE_ID,
            GET_PARTY_RULE_SET_RESPONSE_SCHEMA,
            DataClass::Confidential,
            &wire::GetPartyRuleSetVersionResponse {
                rule_set_version: Some(output),
            },
        )
    }

    async fn execute_get_party_completeness_profile(
        &self,
        request: &QueryRequest,
    ) -> Result<TypedPayload, SdkError> {
        let command: wire::GetPartyCompletenessProfileVersionRequest = decode_input(
            request,
            GET_PARTY_COMPLETENESS_PROFILE_REQUEST_SCHEMA,
            DataClass::Confidential,
            "DATA_QUALITY_COMPLETENESS_PROFILE_QUERY_INPUT",
        )?;
        let version_ref =
            required_completeness_profile_ref(command.completeness_profile_version_ref)?;
        let snapshot = self
            .load_snapshot(
                request,
                PARTY_COMPLETENESS_PROFILE_VERSION_RECORD_TYPE,
                version_ref.completeness_profile_version_id,
                completeness_profile_not_found,
            )
            .await?;
        let rule_set_version_id =
            completeness_profile_rule_set_version_id_from_snapshot(&snapshot)?;
        let rule_set_snapshot = self
            .load_snapshot(
                request,
                PARTY_RULE_SET_VERSION_RECORD_TYPE,
                rule_set_version_id,
                persisted_reference_missing,
            )
            .await?;
        let visibility = self
            .visible_or(
                &rule_set_snapshot,
                request,
                completeness_profile_not_found,
            )
            .await?;
        let rule_set = party_rule_set_from_snapshot(&rule_set_snapshot)?;
        let profile = party_completeness_profile_from_immutable_snapshot(&snapshot, &rule_set)?;
        let mut output = party_completeness_profile_to_wire(&profile);
        apply_definition_visibility(
            &mut output.definition,
            visibility.allows_field("definition"),
        );
        support::protobuf_payload(
            MODULE_ID,
            GET_PARTY_COMPLETENESS_PROFILE_RESPONSE_SCHEMA,
            DataClass::Confidential,
            &wire::GetPartyCompletenessProfileVersionResponse {
                completeness_profile_version: Some(output),
            },
        )
    }

    async fn execute_get_party_evaluation_job(
        &self,
        request: &QueryRequest,
    ) -> Result<TypedPayload, SdkError> {
        let command: wire::GetPartyEvaluationJobRequest = decode_input(
            request,
            GET_PARTY_EVALUATION_JOB_REQUEST_SCHEMA,
            DataClass::Personal,
            "DATA_QUALITY_EVALUATION_QUERY_INPUT",
        )?;
        let job_ref = required_evaluation_job_ref(command.evaluation_job_ref)?;
        let snapshot = self
            .load_snapshot(
                request,
                PARTY_EVALUATION_JOB_RECORD_TYPE,
                job_ref.evaluation_job_id,
                evaluation_job_not_found,
            )
            .await?;
        let visibility = self
            .visible_or(&snapshot, request, evaluation_job_not_found)
            .await?;
        let job = party_evaluation_job_from_snapshot(&snapshot)?;
        let output = evaluation_job_to_wire_with_visibility(&job, snapshot.version, &visibility);
        support::protobuf_payload(
            MODULE_ID,
            GET_PARTY_EVALUATION_JOB_RESPONSE_SCHEMA,
            DataClass::Personal,
            &wire::GetPartyEvaluationJobResponse {
                evaluation_job: Some(output),
            },
        )
    }
}

fn data_quality_cursor_key() -> [u8; 32] {
    if let Ok(value) = std::env::var("CRM_CURSOR_SIGNING_KEY") {
        if value.len() >= 32 {
            let mut key = [0_u8; 32];
            key.copy_from_slice(&value.as_bytes()[..32]);
            return key;
        }
    }
    [0x44; 32]
}
