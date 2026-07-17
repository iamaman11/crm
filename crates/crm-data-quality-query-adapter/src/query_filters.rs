fn validated_status_filter(value: Option<i32>) -> Result<Option<PartyFindingStatus>, SdkError> {
    value
        .map(|value| match wire::DataQualityFindingStatus::try_from(value) {
            Ok(wire::DataQualityFindingStatus::Open) => Ok(PartyFindingStatus::Open),
            Ok(wire::DataQualityFindingStatus::Acknowledged) => {
                Ok(PartyFindingStatus::Acknowledged)
            }
            Ok(wire::DataQualityFindingStatus::Waived) => Ok(PartyFindingStatus::Waived),
            Ok(wire::DataQualityFindingStatus::Remediated) => {
                Ok(PartyFindingStatus::Remediated)
            }
            Ok(wire::DataQualityFindingStatus::Unspecified) | Err(_) => {
                Err(SdkError::invalid_argument(
                    "data_quality.finding.status",
                    "Finding status filter is invalid",
                ))
            }
        })
        .transpose()
}

fn validated_severity_filter(value: Option<i32>) -> Result<Option<QualitySeverity>, SdkError> {
    value
        .map(|value| match wire::QualitySeverity::try_from(value) {
            Ok(wire::QualitySeverity::Info) => Ok(QualitySeverity::Info),
            Ok(wire::QualitySeverity::Warning) => Ok(QualitySeverity::Warning),
            Ok(wire::QualitySeverity::Error) => Ok(QualitySeverity::Error),
            Ok(wire::QualitySeverity::Critical) => Ok(QualitySeverity::Critical),
            Ok(wire::QualitySeverity::Unspecified) | Err(_) => Err(SdkError::invalid_argument(
                "data_quality.finding.severity",
                "Finding severity filter is invalid",
            )),
        })
        .transpose()
}

fn status_to_wire(value: PartyFindingStatus) -> i32 {
    match value {
        PartyFindingStatus::Open => wire::DataQualityFindingStatus::Open as i32,
        PartyFindingStatus::Acknowledged => wire::DataQualityFindingStatus::Acknowledged as i32,
        PartyFindingStatus::Waived => wire::DataQualityFindingStatus::Waived as i32,
        PartyFindingStatus::Remediated => wire::DataQualityFindingStatus::Remediated as i32,
    }
}

fn severity_to_wire(value: QualitySeverity) -> i32 {
    match value {
        QualitySeverity::Info => wire::QualitySeverity::Info as i32,
        QualitySeverity::Warning => wire::QualitySeverity::Warning as i32,
        QualitySeverity::Error => wire::QualitySeverity::Error as i32,
        QualitySeverity::Critical => wire::QualitySeverity::Critical as i32,
    }
}

fn status_filter_wire(value: Option<PartyFindingStatus>) -> i32 {
    value.map_or(0, status_to_wire)
}

fn severity_filter_wire(value: Option<QualitySeverity>) -> i32 {
    value.map_or(0, severity_to_wire)
}

fn finding_cursor_binding(
    request: &QueryRequest,
    filter_hash: [u8; 32],
    page_size: u32,
) -> Result<CursorBinding, SdkError> {
    Ok(CursorBinding {
        tenant_id: request.context.tenant_id.clone(),
        actor_id: Some(request.context.actor_id.clone()),
        capability_id: request.context.capability_id.clone(),
        capability_version: request.context.capability_version.clone(),
        resource_type: finding_record_type()?,
        normalized_filter_hash: filter_hash,
        sort_id: RecordQuerySort::UpdatedAtDescending.id().to_owned(),
        page_size,
    })
}

fn decode_finding_after(
    adapter: &DataQualityQueryAdapter,
    token: &str,
    binding: &CursorBinding,
) -> Result<Option<RecordQueryContinuation>, SdkError> {
    if token.is_empty() {
        return Ok(None);
    }
    let continuation = adapter
        .cursor_codec
        .decode(token, binding)
        .map_err(cursor_error)?;
    let sort_value =
        String::from_utf8(continuation.sort_key).map_err(|_| cursor_invalid())?;
    let after = RecordQueryContinuation {
        sort_value,
        record_id: continuation.record_id,
    };
    after.validate()?;
    Ok(Some(after))
}

fn encode_finding_next(
    adapter: &DataQualityQueryAdapter,
    binding: &CursorBinding,
    next: Option<&RecordQueryContinuation>,
) -> Result<String, SdkError> {
    next.map(|next| {
        adapter
            .cursor_codec
            .encode(
                binding,
                &CursorContinuation {
                    sort_key: next.sort_value.as_bytes().to_vec(),
                    record_id: next.record_id.clone(),
                },
            )
            .map_err(cursor_error)
    })
    .transpose()
    .map(|value| value.unwrap_or_default())
}
