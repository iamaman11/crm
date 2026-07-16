use super::data_quality_evaluation_evidence::{
    audit_count, event_count, load_record, record_count,
};
use super::data_quality_evaluation_fixture::{INTERNAL_STAGE, REQUEST_EVALUATION};
use crm_core_data::PostgresDataStore;
use sqlx::PgPool;

const JOB_RECORD_TYPE: &str = "data_quality.party_evaluation_job";
const INPUT_RECORD_TYPE: &str = "data_quality.party_evaluation_input";
const REQUESTED_EVENT: &str = "data_quality.party.evaluation.requested";
const STAGED_EVENT: &str = "data_quality.party.evaluation.staged";

pub struct ExpectedEvaluationEvidence<'a> {
    pub job_id: &'a str,
    pub party_id: &'a str,
    pub rule_set_id: &'a str,
    pub profile_id: &'a str,
    pub party_version: i64,
}

pub struct StableEvaluationEvidence {
    pub job_bytes: Vec<u8>,
    pub input_bytes: Vec<u8>,
}

pub async fn assert_staged_evidence(
    store: &PostgresDataStore,
    admin: &PgPool,
    expected: ExpectedEvaluationEvidence<'_>,
) -> StableEvaluationEvidence {
    let job = load_record(store, JOB_RECORD_TYPE, expected.job_id).await;
    assert_eq!(job.version, 2);
    assert_eq!(job.json["canonicalization_profile"], "crm.cjson/v1");
    assert_eq!(job.json["status"], "staged");
    assert_eq!(job.json["party_id"], expected.party_id);
    assert_eq!(job.json["rule_set_version_id"], expected.rule_set_id);
    assert_eq!(job.json["profile_version_id"], expected.profile_id);
    assert_eq!(job.json["party_resource_version"], expected.party_version);

    let input = load_record(store, INPUT_RECORD_TYPE, expected.job_id).await;
    assert_eq!(input.version, 1);
    assert_eq!(input.json["canonicalization_profile"], "crm.cjson/v1");
    assert_eq!(input.json["job_id"], expected.job_id);
    assert_eq!(input.json["party_id"], expected.party_id);
    assert_eq!(input.json["display_name"], "Ada Lovelace");
    assert_eq!(input.json["party_resource_version"], expected.party_version);

    assert_side_effect_counts(admin).await;
    StableEvaluationEvidence {
        job_bytes: job.bytes,
        input_bytes: input.bytes,
    }
}

pub async fn assert_restart_unchanged(
    store: &PostgresDataStore,
    admin: &PgPool,
    job_id: &str,
    stable: StableEvaluationEvidence,
) {
    let job = load_record(store, JOB_RECORD_TYPE, job_id).await;
    let input = load_record(store, INPUT_RECORD_TYPE, job_id).await;
    assert_eq!(job.version, 2);
    assert_eq!(input.version, 1);
    assert_eq!(job.bytes, stable.job_bytes);
    assert_eq!(input.bytes, stable.input_bytes);
    assert_side_effect_counts(admin).await;
}

async fn assert_side_effect_counts(admin: &PgPool) {
    assert_eq!(record_count(admin, JOB_RECORD_TYPE).await, 1);
    assert_eq!(record_count(admin, INPUT_RECORD_TYPE).await, 1);
    assert_eq!(event_count(admin, REQUESTED_EVENT).await, 1);
    assert_eq!(event_count(admin, STAGED_EVENT).await, 1);
    assert_eq!(audit_count(admin, REQUEST_EVALUATION).await, 1);
    assert_eq!(audit_count(admin, INTERNAL_STAGE).await, 1);
}
