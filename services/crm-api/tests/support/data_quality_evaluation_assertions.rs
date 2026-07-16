use super::data_quality_evaluation_evidence::{
    audit_count, event_count, load_record, record_count,
};
use super::data_quality_evaluation_fixture::{
    INTERNAL_MATERIALIZE, INTERNAL_STAGE, REQUEST_EVALUATION, TENANT,
};
use crm_core_data::{PostgresDataStore, RecordListQuery, RecordQuerySort};
use crm_module_sdk::{ModuleId, RecordType, TenantId};
use sqlx::PgPool;

const JOB_RECORD_TYPE: &str = "data_quality.party_evaluation_job";
const INPUT_RECORD_TYPE: &str = "data_quality.party_evaluation_input";
const OUTCOME_RECORD_TYPE: &str = "data_quality.rule_outcome";
const RESULT_RECORD_TYPE: &str = "data_quality.party_completeness_result";
const REQUESTED_EVENT: &str = "data_quality.party.evaluation.requested";
const STAGED_EVENT: &str = "data_quality.party.evaluation.staged";
const MATERIALIZED_EVENT: &str = "data_quality.party.evaluation.materialized";

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
    pub outcome_bytes: Vec<Vec<u8>>,
    pub result_bytes: Vec<u8>,
}

pub async fn assert_staged_evidence(
    store: &PostgresDataStore,
    admin: &PgPool,
    expected: ExpectedEvaluationEvidence<'_>,
) {
    let job = load_record(store, JOB_RECORD_TYPE, expected.job_id).await;
    assert_eq!(job.version, 2);
    assert_eq!(job.json["canonicalization_profile"], "crm.cjson/v1");
    assert_eq!(job.json["status"], "staged");
    assert_eq!(job.json["party_id"], expected.party_id);
    assert_eq!(job.json["rule_set_version_id"], expected.rule_set_id);
    assert_eq!(job.json["profile_version_id"], expected.profile_id);
    assert_eq!(job.json["party_resource_version"], expected.party_version);
    assert_eq!(job.json["evaluated_rules"], 0);
    assert_eq!(job.json["failed_rules"], 0);

    let input = load_record(store, INPUT_RECORD_TYPE, expected.job_id).await;
    assert_eq!(input.version, 1);
    assert_eq!(input.json["canonicalization_profile"], "crm.cjson/v1");
    assert_eq!(input.json["job_id"], expected.job_id);
    assert_eq!(input.json["party_id"], expected.party_id);
    assert_eq!(input.json["display_name"], "Ada Lovelace");
    assert_eq!(input.json["party_resource_version"], expected.party_version);

    assert_eq!(record_count(admin, OUTCOME_RECORD_TYPE).await, 0);
    assert_eq!(record_count(admin, RESULT_RECORD_TYPE).await, 0);
    assert_eq!(event_count(admin, MATERIALIZED_EVENT).await, 0);
    assert_eq!(audit_count(admin, INTERNAL_MATERIALIZE).await, 0);
    assert_base_side_effect_counts(admin).await;
}

pub async fn assert_materialized_evidence(
    store: &PostgresDataStore,
    admin: &PgPool,
    expected: ExpectedEvaluationEvidence<'_>,
) -> StableEvaluationEvidence {
    let job = load_record(store, JOB_RECORD_TYPE, expected.job_id).await;
    assert_eq!(job.version, 3);
    assert_eq!(job.json["status"], "staged");
    assert_eq!(job.json["evaluated_rules"], 2);
    assert_eq!(job.json["failed_rules"], 0);
    assert_eq!(job.json["party_resource_version"], expected.party_version);

    let input = load_record(store, INPUT_RECORD_TYPE, expected.job_id).await;
    let outcomes = list_record_bytes(store, OUTCOME_RECORD_TYPE).await;
    assert_eq!(outcomes.len(), 2);
    for (_, bytes) in &outcomes {
        let json: serde_json::Value =
            serde_json::from_slice(bytes).expect("decode materialized rule outcome");
        assert_eq!(json["canonicalization_profile"], "crm.cjson/v1");
        assert_eq!(json["job_id"], expected.job_id);
        assert_eq!(json["party_id"], expected.party_id);
        assert_eq!(json["party_resource_version"], expected.party_version);
        assert_eq!(json["rule_set_version_id"], expected.rule_set_id);
        assert_eq!(json["passed"], true);
    }

    let results = list_record_bytes(store, RESULT_RECORD_TYPE).await;
    assert_eq!(results.len(), 1);
    let result_json: serde_json::Value =
        serde_json::from_slice(&results[0].1).expect("decode materialized completeness result");
    assert_eq!(result_json["canonicalization_profile"], "crm.cjson/v1");
    assert_eq!(result_json["job_id"], expected.job_id);
    assert_eq!(result_json["party_id"], expected.party_id);
    assert_eq!(
        result_json["party_resource_version"],
        expected.party_version
    );
    assert_eq!(result_json["profile_version_id"], expected.profile_id);
    assert_eq!(result_json["score_basis_points"], 10_000);
    assert_eq!(
        result_json["components"]
            .as_array()
            .expect("completeness components")
            .len(),
        2
    );

    assert_eq!(record_count(admin, OUTCOME_RECORD_TYPE).await, 2);
    assert_eq!(record_count(admin, RESULT_RECORD_TYPE).await, 1);
    assert_eq!(event_count(admin, MATERIALIZED_EVENT).await, 1);
    assert_eq!(audit_count(admin, INTERNAL_MATERIALIZE).await, 1);
    assert_base_side_effect_counts(admin).await;

    StableEvaluationEvidence {
        job_bytes: job.bytes,
        input_bytes: input.bytes,
        outcome_bytes: outcomes.into_iter().map(|(_, bytes)| bytes).collect(),
        result_bytes: results.into_iter().next().unwrap().1,
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
    let outcomes = list_record_bytes(store, OUTCOME_RECORD_TYPE).await;
    let results = list_record_bytes(store, RESULT_RECORD_TYPE).await;
    assert_eq!(job.version, 3);
    assert_eq!(input.version, 1);
    assert_eq!(job.bytes, stable.job_bytes);
    assert_eq!(input.bytes, stable.input_bytes);
    assert_eq!(
        outcomes
            .into_iter()
            .map(|(_, bytes)| bytes)
            .collect::<Vec<_>>(),
        stable.outcome_bytes
    );
    assert_eq!(results.len(), 1);
    assert_eq!(&results[0].1, &stable.result_bytes);
    assert_eq!(record_count(admin, OUTCOME_RECORD_TYPE).await, 2);
    assert_eq!(record_count(admin, RESULT_RECORD_TYPE).await, 1);
    assert_eq!(event_count(admin, MATERIALIZED_EVENT).await, 1);
    assert_eq!(audit_count(admin, INTERNAL_MATERIALIZE).await, 1);
    assert_base_side_effect_counts(admin).await;
}

async fn list_record_bytes(
    store: &PostgresDataStore,
    record_type_value: &str,
) -> Vec<(String, Vec<u8>)> {
    let page = store
        .list_records_for_query(&RecordListQuery {
            tenant_id: TenantId::try_new(TENANT).unwrap(),
            owner_module_id: ModuleId::try_new("crm.data-quality").unwrap(),
            record_type: RecordType::try_new(record_type_value).unwrap(),
            page_size: 100,
            sort: RecordQuerySort::CreatedAtAscending,
            after: None,
        })
        .await
        .expect("list materialized evaluation records");
    assert!(page.next.is_none());
    let mut records = page
        .records
        .into_iter()
        .map(|snapshot| {
            (
                snapshot.reference.record_id.as_str().to_owned(),
                snapshot.payload.bytes,
            )
        })
        .collect::<Vec<_>>();
    records.sort_by(|left, right| left.0.cmp(&right.0));
    records
}

async fn assert_base_side_effect_counts(admin: &PgPool) {
    assert_eq!(record_count(admin, JOB_RECORD_TYPE).await, 1);
    assert_eq!(record_count(admin, INPUT_RECORD_TYPE).await, 1);
    assert_eq!(event_count(admin, REQUESTED_EVENT).await, 1);
    assert_eq!(event_count(admin, STAGED_EVENT).await, 1);
    assert_eq!(audit_count(admin, REQUEST_EVALUATION).await, 1);
    assert_eq!(audit_count(admin, INTERNAL_STAGE).await, 1);
}
