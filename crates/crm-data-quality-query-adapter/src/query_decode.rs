fn finding_from_snapshot(snapshot: &RecordSnapshot) -> Result<PartyFinding, SdkError> {
    let finding = decode_finding_state(support::persisted_json_bytes_with_data_class(snapshot, party_finding_persisted_contract(), DataClass::Personal)?)?;
    if finding.finding_id() != snapshot.reference.record_id.as_str() || snapshot.version <= 0 {
        return Err(support::stored_data_error("DATA_QUALITY_PERSISTED_FINDING_IDENTITY_INVALID"));
    }
    Ok(finding)
}

fn observation_from_snapshot(snapshot: &RecordSnapshot) -> Result<PartyFindingObservation, SdkError> {
    let observation = decode_finding_observation_state(support::persisted_json_bytes_with_data_class(snapshot, party_finding_observation_persisted_contract(), DataClass::Personal)?)?;
    if observation.observation_id() != snapshot.reference.record_id.as_str() || snapshot.version != 1 {
        return Err(support::stored_data_error("DATA_QUALITY_PERSISTED_FINDING_OBSERVATION_IDENTITY_INVALID"));
    }
    Ok(observation)
}

fn completeness_result_from_snapshot(snapshot: &RecordSnapshot) -> Result<PartyCompletenessResult, SdkError> {
    let result = decode_party_completeness_result_state(support::persisted_json_bytes_with_data_class(snapshot, party_completeness_result_persisted_contract(), DataClass::Personal)?)?;
    if result.result_id() != snapshot.reference.record_id.as_str() || snapshot.version != 1 {
        return Err(support::stored_data_error("DATA_QUALITY_PERSISTED_COMPLETENESS_RESULT_IDENTITY_INVALID"));
    }
    Ok(result)
}

fn decode_input<T: Message + Default>(request: &QueryRequest, schema_id: &'static str, data_class: DataClass, code_prefix: &'static str) -> Result<T, SdkError> {
    let payload = &request.input;
    if payload.owner.as_str() != MODULE_ID
        || payload.schema_id.as_str() != schema_id
        || payload.schema_version.as_str() != support::CONTRACT_VERSION
        || payload.descriptor_hash != support::message_descriptor_hash(schema_id)
        || payload.data_class != data_class
        || payload.encoding != PayloadEncoding::Protobuf
        || payload.maximum_size_bytes != support::MAX_PROTOBUF_BYTES
        || payload.validate().is_err()
    {
        return Err(SdkError::new(format!("{code_prefix}_CONTRACT_MISMATCH"), ErrorCategory::InvalidArgument, false, "The Data Quality query input does not match the required contract."));
    }
    T::decode(payload.bytes.as_slice()).map_err(|_| SdkError::new(format!("{code_prefix}_PROTOBUF_INVALID"), ErrorCategory::InvalidArgument, false, "The Data Quality query input is not valid Protobuf."))
}

fn required_rule_set_ref(value: Option<wire::PartyRuleSetVersionRef>) -> Result<wire::PartyRuleSetVersionRef, SdkError> {
    value.ok_or_else(|| SdkError::invalid_argument("data_quality.party_rule_set.rule_set_version_ref", "Party rule-set version reference is required"))
}
fn required_completeness_profile_ref(value: Option<wire::PartyCompletenessProfileVersionRef>) -> Result<wire::PartyCompletenessProfileVersionRef, SdkError> {
    value.ok_or_else(|| SdkError::invalid_argument("data_quality.party_completeness_profile.completeness_profile_version_ref", "Party completeness-profile version reference is required"))
}
fn required_evaluation_job_ref(value: Option<wire::PartyEvaluationJobRef>) -> Result<wire::PartyEvaluationJobRef, SdkError> {
    value.ok_or_else(|| SdkError::invalid_argument("data_quality.party_evaluation.evaluation_job_ref", "Party evaluation job reference is required"))
}
fn finding_record_id(value: Option<wire::DataQualityFindingRef>) -> Result<RecordId, SdkError> {
    let value = value.ok_or_else(|| SdkError::invalid_argument("data_quality.finding_ref", "Data Quality finding reference is required"))?;
    RecordId::try_new(value.finding_id).map_err(|error| SdkError::invalid_argument("data_quality.finding_ref.finding_id", error.to_string()))
}
fn party_record_id(value: Option<customer::PartyRef>) -> Result<RecordId, SdkError> {
    let value = value.ok_or_else(|| SdkError::invalid_argument("data_quality.party_ref", "Party reference is required"))?;
    RecordId::try_new(value.party_id).map_err(|error| SdkError::invalid_argument("data_quality.party_ref.party_id", error.to_string()))
}
fn completeness_result_record_id(value: Option<wire::PartyCompletenessResultRef>) -> Result<RecordId, SdkError> {
    let value = value.ok_or_else(|| SdkError::invalid_argument("data_quality.completeness_result_ref", "Party completeness-result reference is required"))?;
    RecordId::try_new(value.completeness_result_id).map_err(|error| SdkError::invalid_argument("data_quality.completeness_result_ref.completeness_result_id", error.to_string()))
}
fn assigned_actor_filter(value: Option<String>, requesting_actor_id: &str) -> Result<ActorId, SdkError> {
    ActorId::try_new(value.unwrap_or_else(|| requesting_actor_id.to_owned())).map_err(|error| SdkError::invalid_argument("data_quality.assigned_actor_id", format!("Assigned actor filter is invalid: {error}")))
}
