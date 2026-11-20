use crm_data_quality::PartyEvaluationJob;
use crm_module_sdk::{ErrorCategory, SdkError};
use crm_proto_contracts::crm::{
    customer::v1 as customer, data_quality::v1 as wire, parties::v1 as parties,
};

use crate::{PartyQualitySourceKind, PartyQualitySourceSnapshot};

pub(crate) fn stage_command(
    job: &PartyEvaluationJob,
    expected_job_version: i64,
    source: &PartyQualitySourceSnapshot,
) -> Result<wire::StagePartyEvaluationInputRequest, SdkError> {
    if source.party_id != *job.party_id() {
        return Err(SdkError::new(
            "DATA_QUALITY_EVALUATION_STAGE_STATE_INVALID",
            ErrorCategory::Internal,
            false,
            "The Party evaluation input could not be staged safely.",
        ));
    }
    Ok(wire::StagePartyEvaluationInputRequest {
        evaluation_job_ref: Some(wire::PartyEvaluationJobRef {
            evaluation_job_id: job.job_id().as_str().to_owned(),
        }),
        expected_job_version,
        party_ref: Some(customer::PartyRef {
            party_id: source.party_id.as_str().to_owned(),
        }),
        party_kind: match source.kind {
            PartyQualitySourceKind::Person => parties::PartyKind::Person as i32,
            PartyQualitySourceKind::Organization => parties::PartyKind::Organization as i32,
        },
        display_name: source.display_name.clone(),
        party_resource_version: source.resource_version,
    })
}
