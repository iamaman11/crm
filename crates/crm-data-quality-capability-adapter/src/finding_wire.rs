use crm_data_quality::{PartyFinding, PartyFindingStatus, QualitySeverity};
use crm_proto_contracts::crm::{
    core::v1 as core, customer::v1 as customer, data_quality::v1 as wire,
};

pub fn party_finding_to_wire(
    finding: &PartyFinding,
    version: i64,
) -> wire::DataQualityFinding {
    wire::DataQualityFinding {
        finding_ref: Some(wire::DataQualityFindingRef {
            finding_id: finding.finding_id().to_owned(),
        }),
        party_ref: Some(customer::PartyRef {
            party_id: finding.party_id().as_str().to_owned(),
        }),
        rule_set_version_ref: Some(wire::PartyRuleSetVersionRef {
            rule_set_version_id: finding.rule_set_version_id().to_owned(),
        }),
        rule_key: finding.rule_key().as_str().to_owned(),
        severity: severity_to_wire(finding.severity()),
        status: status_to_wire(finding.status()),
        current_observation_ref: Some(wire::DataQualityFindingObservationRef {
            finding_observation_id: finding.current_observation_id().to_owned(),
        }),
        evaluated_party_resource_version: Some(customer::CustomerResourceVersion {
            version: finding.evaluated_party_resource_version(),
            created_at: None,
            updated_at: None,
        }),
        assigned_actor_id: finding
            .assigned_actor_id()
            .map(|value| value.as_str().to_owned()),
        waiver_reason: finding.waiver_reason().map(str::to_owned),
        created_at: Some(core::UnixTime {
            unix_nanos: finding.created_at(),
        }),
        updated_at: Some(core::UnixTime {
            unix_nanos: finding.updated_at(),
        }),
        resource_version: Some(customer::CustomerResourceVersion {
            version,
            created_at: Some(core::UnixTime {
                unix_nanos: finding.created_at(),
            }),
            updated_at: Some(core::UnixTime {
                unix_nanos: finding.updated_at(),
            }),
        }),
        remediated_by_rule_outcome_ref: finding.remediated_by_rule_outcome_id().map(|value| {
            wire::PartyRuleOutcomeRef {
                rule_outcome_id: value.to_owned(),
            }
        }),
    }
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
