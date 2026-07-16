#![cfg(unix)]

#[path = "support/data_quality_evaluation_actor.rs"]
mod data_quality_evaluation_actor;
#[path = "support/data_quality_evaluation_fixture.rs"]
mod data_quality_evaluation_fixture;
#[path = "support/data_quality_evaluation_gateway.rs"]
mod data_quality_evaluation_gateway;
#[path = "support/data_quality_evaluation_operations.rs"]
mod data_quality_evaluation_operations;
#[path = "support/data_quality_evaluation_process.rs"]
mod data_quality_evaluation_process;
#[path = "support/data_quality_evaluation_registry.rs"]
mod data_quality_evaluation_registry;
#[path = "support/data_quality_evaluation_worker.rs"]
mod data_quality_evaluation_worker;

use crm_core_data::{PostgresDataStore, RecordGetQuery, RecordListQuery, RecordQuerySort};
use crm_data_quality::{decode_finding_state, decode_remediation_attempt_state};
use crm_module_sdk::{ModuleId, RecordId, RecordType, TenantId};
use crm_proto_contracts::crm::{customer::v1 as customer, data_quality::v1 as data_quality};
use data_quality_evaluation_actor::provision_worker_actor;
use data_quality_evaluation_fixture::{TENANT, profile_input, rule_set_input, unique_id};
use data_quality_evaluation_gateway::{
    mutate, mutation_definition, payload, query, query_definition, query_for_tenant,
};
use data_quality_evaluation_operations::{
    create_party_with_display_name, profile_version_id, publish_profile, publish_rule_set,
    request_evaluation, rule_set_version_id,
};
use data_quality_evaluation_process::{start, start_with_environment, stop};
use data_quality_evaluation_registry::register_evaluation_capabilities;
use data_quality_evaluation_worker::build_evaluation_worker;
use prost::Message;
use sqlx::{PgPool, Row};
use tonic::Code;

const OTHER_TENANT: &str = "tenant-evaluation-other";
const DQ_MODULE: &str = "crm.data-quality";
const FINDING_TYPE: &str = "data_quality.finding";
const OBSERVATION_TYPE: &str = "data_quality.finding_observation";
const RESULT_TYPE: &str = "data_quality.party_completeness_result";
const ATTEMPT_TYPE: &str = "data_quality.remediation_attempt";
const PARTY_MODULE: &str = "crm.parties";
const PARTY_TYPE: &str = "parties.party";

const GET_EVALUATION: &str = "data_quality.party.evaluation.get";
const GET_FINDING: &str = "data_quality.finding.get";
const LIST_BY_PARTY: &str = "data_quality.finding.list_by_party";
const LIST_ASSIGNED: &str = "data_quality.finding.list_assigned";
const GET_COMPLETENESS: &str = "data_quality.party.completeness.get";
const ASSIGN: &str = "data_quality.finding.assign";
const ACKNOWLEDGE: &str = "data_quality.finding.acknowledge";
const WAIVE: &str = "data_quality.finding.waive";
const REMEDIATE: &str = "data_quality.party.display_name.remediate";

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn completion_surfaces_stewardship_and_remediation_recover_without_duplicates() {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!(
            "skipping Data Quality completion-surface process proof because DATABASE_URL is absent"
        );
        return;
    };
    let admin_database_url = std::env::var("ADMIN_DATABASE_URL")
        .expect("ADMIN_DATABASE_URL must accompany DATABASE_URL");
    let admin = PgPool::connect(&admin_database_url)
        .await
        .expect("connect Data Quality completion-surface administrator");
    register_evaluation_capabilities(&admin).await;
    provision_worker_actor(&admin).await;

    let mut api = start(&database_url).await;
    let rule_set = publish_rule_set(
        &mut api.client,
        rule_set_input(),
        &unique_id("completion-rule-set"),
    )
    .await;
    let rule_set_id = rule_set_version_id(&rule_set);
    let profile = publish_profile(
        &mut api.client,
        profile_input(&rule_set_id),
        &unique_id("completion-profile"),
    )
    .await;
    let profile_id = profile_version_id(&profile);

    let party_a = unique_id("completion-party-a");
    let party_b = unique_id("completion-party-b");
    let created_a = create_party_with_display_name(
        &mut api.client,
        &party_a,
        "unknown",
        &unique_id("completion-party-a-create"),
    )
    .await;
    let created_b = create_party_with_display_name(
        &mut api.client,
        &party_b,
        "unknown",
        &unique_id("completion-party-b-create"),
    )
    .await;
    let party_a_version = resource_version(&created_a);
    let party_b_version = resource_version(&created_b);
    assert_eq!(party_a_version, 1);
    assert_eq!(party_b_version, 1);

    let job_a = unique_id("completion-job-a");
    let job_b = unique_id("completion-job-b");
    request_evaluation(
        &mut api.client,
        &job_a,
        &party_a,
        &rule_set_id,
        &profile_id,
        &unique_id("completion-request-a"),
    )
    .await;
    request_evaluation(
        &mut api.client,
        &job_b,
        &party_b,
        &rule_set_id,
        &profile_id,
        &unique_id("completion-request-b"),
    )
    .await;
    stop(&mut api).await;

    run_worker_until_idle(&database_url).await;
    let store = PostgresDataStore::connect(&database_url)
        .await
        .expect("connect completion-surface evidence store");
    let evidence_a = evaluation_evidence(&store, &party_a, &job_a).await;
    let evidence_b = evaluation_evidence(&store, &party_b, &job_b).await;

    let hidden = "data_quality.party.evaluation.get|crm.data-quality|data_quality.party_evaluation_job|failed_rules";
    let mut api = start_with_environment(
        &database_url,
        &[
            ("CRM_API_TENANTS", &format!("{TENANT},{OTHER_TENANT}")),
            ("CRM_QUERY_HIDDEN_FIELDS", hidden),
        ],
    )
    .await;

    let evaluation = get_evaluation(&mut api.client, &job_a).await;
    assert_eq!(
        evaluation.status,
        data_quality::PartyEvaluationJobStatus::Completed as i32
    );
    assert_eq!(evaluation.evaluated_rules, 2);
    assert_eq!(evaluation.failed_rules, 0, "failed_rules must be redacted");
    assert_eq!(
        evaluation
            .evaluated_party_resource_version
            .as_ref()
            .expect("visible evaluated Party version")
            .version,
        party_a_version
    );

    let cross_tenant = query_for_tenant(
        &mut api.client,
        &query_definition(GET_EVALUATION),
        payload(
            &query_definition(GET_EVALUATION),
            data_quality::GetPartyEvaluationJobRequest {
                evaluation_job_ref: Some(data_quality::PartyEvaluationJobRef {
                    evaluation_job_id: job_a.clone(),
                }),
            },
        ),
        OTHER_TENANT,
    )
    .await
    .expect_err("tenant B must not discover tenant A evaluation job");
    assert_eq!(cross_tenant.code(), Code::NotFound);

    let finding_a = get_finding(&mut api.client, &evidence_a.finding_id).await;
    let observation_a = finding_a
        .current_observation
        .as_ref()
        .expect("visible current finding observation");
    assert_eq!(
        observation_a.reason_code,
        "DATA_QUALITY_PARTY_DISPLAY_NAME_PLACEHOLDER"
    );
    assert_eq!(
        finding_a
            .finding
            .as_ref()
            .expect("visible finding")
            .party_ref
            .as_ref()
            .expect("visible Party ref")
            .party_id,
        party_a
    );

    let completeness = get_completeness(&mut api.client, &evidence_a.result_id).await;
    assert_eq!(completeness.score_basis_points, 4_000);
    assert_eq!(completeness.components.len(), 2);
    assert!(
        completeness
            .components
            .iter()
            .all(|component| component.rule_outcome_ref.is_some())
    );

    let listed_a = list_by_party(&mut api.client, &party_a).await;
    assert_eq!(listed_a.findings.len(), 1);
    assert!(listed_a.next_cursor.is_empty());

    let assigned_a = assign_finding(
        &mut api.client,
        &evidence_a.finding_id,
        1,
        Some("actor-evaluation"),
        &unique_id("completion-assign-a"),
    )
    .await;
    let assigned_b = assign_finding(
        &mut api.client,
        &evidence_b.finding_id,
        1,
        Some("actor-evaluation"),
        &unique_id("completion-assign-b"),
    )
    .await;
    assert_eq!(wire_resource_version(&assigned_a), 2);
    assert_eq!(wire_resource_version(&assigned_b), 2);

    let stale_assign = mutate(
        &mut api.client,
        &mutation_definition(ASSIGN),
        payload(
            &mutation_definition(ASSIGN),
            data_quality::AssignDataQualityFindingRequest {
                finding_ref: Some(data_quality::DataQualityFindingRef {
                    finding_id: evidence_a.finding_id.clone(),
                }),
                assigned_actor_id: None,
                expected_version: 1,
            },
        ),
        &unique_id("completion-stale-assign"),
    )
    .await
    .expect_err("stale finding version must conflict");
    assert_ne!(stale_assign.code(), Code::Ok);

    let acknowledged = acknowledge_finding(
        &mut api.client,
        &evidence_a.finding_id,
        &evidence_a.observation_id,
        2,
        &unique_id("completion-acknowledge"),
    )
    .await;
    assert_eq!(
        acknowledged.status,
        data_quality::DataQualityFindingStatus::Acknowledged as i32
    );
    let waived = waive_finding(
        &mut api.client,
        &evidence_a.finding_id,
        &evidence_a.observation_id,
        3,
        "Accepted source-system exception",
        &unique_id("completion-waive"),
    )
    .await;
    assert_eq!(
        waived.status,
        data_quality::DataQualityFindingStatus::Waived as i32
    );
    assert_eq!(
        waived.waiver_reason.as_deref(),
        Some("Accepted source-system exception")
    );

    let first_page = list_assigned(&mut api.client, "", 1).await;
    assert_eq!(first_page.findings.len(), 1);
    assert!(!first_page.next_cursor.is_empty());
    let second_page = list_assigned(&mut api.client, &first_page.next_cursor, 1).await;
    assert_eq!(second_page.findings.len(), 1);
    assert_ne!(
        first_page.findings[0]
            .finding_ref
            .as_ref()
            .expect("first finding ref")
            .finding_id,
        second_page.findings[0]
            .finding_ref
            .as_ref()
            .expect("second finding ref")
            .finding_id
    );
    let tampered = format!("{}x", first_page.next_cursor);
    let invalid_cursor = query(
        &mut api.client,
        &query_definition(LIST_ASSIGNED),
        payload(
            &query_definition(LIST_ASSIGNED),
            data_quality::ListAssignedDataQualityFindingsRequest {
                assigned_actor_id: None,
                status: None,
                severity: None,
                page_size: 1,
                cursor: tampered,
            },
        ),
    )
    .await
    .expect_err("tampered finding cursor must be rejected");
    assert_eq!(invalid_cursor.code(), Code::InvalidArgument);
    stop(&mut api).await;

    let mut failpoint_api = start_with_environment(
        &database_url,
        &[
            ("CRM_API_TENANTS", &format!("{TENANT},{OTHER_TENANT}")),
            (
                "CRM_DATA_QUALITY_REMEDIATION_FAIL_AFTER_TARGET_ONCE",
                "true",
            ),
        ],
    )
    .await;
    let remediation_key = unique_id("completion-remediation");
    let remediation_request = data_quality::RemediatePartyDisplayNameRequest {
        finding_ref: Some(data_quality::DataQualityFindingRef {
            finding_id: evidence_b.finding_id.clone(),
        }),
        expected_finding_version: 2,
        expected_current_observation_ref: Some(data_quality::DataQualityFindingObservationRef {
            finding_observation_id: evidence_b.observation_id.clone(),
        }),
        expected_party_resource_version: party_b_version,
        display_name: "Grace Hopper".to_owned(),
    };
    let deferred = mutate(
        &mut failpoint_api.client,
        &mutation_definition(REMEDIATE),
        payload(&mutation_definition(REMEDIATE), remediation_request.clone()),
        &remediation_key,
    )
    .await
    .expect_err("failpoint must defer Data Quality outcome after Party update");
    assert_eq!(deferred.code(), Code::Unavailable);
    let updated_after_failure = party_snapshot(&store, &party_b).await;
    assert_eq!(updated_after_failure.version, 2);
    assert_eq!(updated_after_failure.display_name, "Grace Hopper");
    assert_eq!(record_count(&admin, ATTEMPT_TYPE).await, 0);
    stop(&mut failpoint_api).await;

    let mut recovery_api = start_with_environment(
        &database_url,
        &[("CRM_API_TENANTS", &format!("{TENANT},{OTHER_TENANT}"))],
    )
    .await;
    let recovered = remediate(
        &mut recovery_api.client,
        remediation_request.clone(),
        &remediation_key,
    )
    .await;
    assert_eq!(
        recovered
            .party
            .as_ref()
            .and_then(|party| party.resource_version.as_ref())
            .expect("replayed Party version")
            .version,
        2
    );
    let attempt = recovered
        .remediation_attempt
        .as_ref()
        .expect("durable remediation attempt");
    assert_eq!(
        attempt
            .finding_ref
            .as_ref()
            .expect("attempt finding ref")
            .finding_id,
        evidence_b.finding_id
    );
    assert_eq!(record_count(&admin, ATTEMPT_TYPE).await, 1);
    let replayed = remediate(
        &mut recovery_api.client,
        remediation_request,
        &remediation_key,
    )
    .await;
    assert_eq!(replayed, recovered);
    let stable_party = party_snapshot(&store, &party_b).await;
    assert_eq!(stable_party.version, 2);
    assert_eq!(stable_party.display_name, "Grace Hopper");
    assert_eq!(record_count(&admin, ATTEMPT_TYPE).await, 1);

    let remediation_job = unique_id("completion-remediation-job");
    request_evaluation(
        &mut recovery_api.client,
        &remediation_job,
        &party_b,
        &rule_set_id,
        &profile_id,
        &unique_id("completion-remediation-evaluation"),
    )
    .await;
    stop(&mut recovery_api).await;
    run_worker_until_idle(&database_url).await;

    let remediated_snapshot = finding_snapshot(&store, &evidence_b.finding_id).await;
    assert_eq!(
        remediated_snapshot.finding.status(),
        crm_data_quality::PartyFindingStatus::Remediated
    );
    assert_eq!(remediated_snapshot.version, 3);
    assert!(
        remediated_snapshot
            .finding
            .remediated_by_rule_outcome_id()
            .is_some()
    );
    assert_eq!(record_count(&admin, OBSERVATION_TYPE).await, 2);
    assert_force_rls_completion_records(
        &admin,
        &database_url,
        &evidence_b.finding_id,
        &evidence_b.result_id,
        attempt
            .remediation_attempt_ref
            .as_ref()
            .expect("attempt ref")
            .remediation_attempt_id
            .as_str(),
    )
    .await;
}

async fn run_worker_until_idle(database_url: &str) {
    let runtime = build_evaluation_worker(database_url).await;
    let tenant = TenantId::try_new(TENANT).unwrap();
    for _ in 0..8 {
        let result = runtime
            .worker
            .run_tenant_cycle(tenant.clone())
            .await
            .expect("run Data Quality completion-surface worker cycle");
        if result.staged_jobs == 0 && result.materialized_jobs == 0 && result.deferred_jobs == 0 {
            break;
        }
    }
}

struct EvaluationEvidence {
    finding_id: String,
    observation_id: String,
    result_id: String,
}

async fn evaluation_evidence(
    store: &PostgresDataStore,
    party_id: &str,
    job_id: &str,
) -> EvaluationEvidence {
    let findings = list_records(store, FINDING_TYPE).await;
    let (finding_id, finding_json) = findings
        .into_iter()
        .find(|(_, json)| json["party_id"] == party_id)
        .expect("finding for evaluated Party");
    let observation_id = finding_json["current_observation_id"]
        .as_str()
        .expect("current observation id")
        .to_owned();
    let results = list_records(store, RESULT_TYPE).await;
    let (result_id, _) = results
        .into_iter()
        .find(|(_, json)| json["job_id"] == job_id)
        .expect("completeness result for evaluation job");
    EvaluationEvidence {
        finding_id,
        observation_id,
        result_id,
    }
}

async fn list_records(
    store: &PostgresDataStore,
    record_type_value: &str,
) -> Vec<(String, serde_json::Value)> {
    let page = store
        .list_records_for_query(&RecordListQuery {
            tenant_id: TenantId::try_new(TENANT).unwrap(),
            owner_module_id: ModuleId::try_new(DQ_MODULE).unwrap(),
            record_type: RecordType::try_new(record_type_value).unwrap(),
            page_size: 200,
            sort: RecordQuerySort::CreatedAtAscending,
            after: None,
        })
        .await
        .expect("list Data Quality completion-surface records");
    assert!(page.next.is_none());
    page.records
        .into_iter()
        .map(|snapshot| {
            (
                snapshot.reference.record_id.as_str().to_owned(),
                serde_json::from_slice(&snapshot.payload.bytes)
                    .expect("decode Data Quality owner record"),
            )
        })
        .collect()
}

async fn get_evaluation(
    client: &mut crm_application_runtime::gateway_v1::application_gateway_service_client::ApplicationGatewayServiceClient<tonic::transport::Channel>,
    job_id: &str,
) -> data_quality::PartyEvaluationJob {
    let definition = query_definition(GET_EVALUATION);
    let response = query(
        client,
        &definition,
        payload(
            &definition,
            data_quality::GetPartyEvaluationJobRequest {
                evaluation_job_ref: Some(data_quality::PartyEvaluationJobRef {
                    evaluation_job_id: job_id.to_owned(),
                }),
            },
        ),
    )
    .await
    .expect("query completed Party evaluation");
    data_quality::GetPartyEvaluationJobResponse::decode(
        response
            .output
            .expect("evaluation query output")
            .payload
            .as_slice(),
    )
    .expect("decode evaluation query response")
    .evaluation_job
    .expect("evaluation job")
}

async fn get_finding(
    client: &mut crm_application_runtime::gateway_v1::application_gateway_service_client::ApplicationGatewayServiceClient<tonic::transport::Channel>,
    finding_id: &str,
) -> data_quality::GetDataQualityFindingResponse {
    let definition = query_definition(GET_FINDING);
    let response = query(
        client,
        &definition,
        payload(
            &definition,
            data_quality::GetDataQualityFindingRequest {
                finding_ref: Some(data_quality::DataQualityFindingRef {
                    finding_id: finding_id.to_owned(),
                }),
            },
        ),
    )
    .await
    .expect("query Data Quality finding");
    data_quality::GetDataQualityFindingResponse::decode(
        response
            .output
            .expect("finding query output")
            .payload
            .as_slice(),
    )
    .expect("decode finding query response")
}

async fn get_completeness(
    client: &mut crm_application_runtime::gateway_v1::application_gateway_service_client::ApplicationGatewayServiceClient<tonic::transport::Channel>,
    result_id: &str,
) -> data_quality::PartyCompletenessResult {
    let definition = query_definition(GET_COMPLETENESS);
    let response = query(
        client,
        &definition,
        payload(
            &definition,
            data_quality::GetPartyCompletenessResultRequest {
                completeness_result_ref: Some(data_quality::PartyCompletenessResultRef {
                    completeness_result_id: result_id.to_owned(),
                }),
            },
        ),
    )
    .await
    .expect("query Party completeness result");
    data_quality::GetPartyCompletenessResultResponse::decode(
        response
            .output
            .expect("completeness query output")
            .payload
            .as_slice(),
    )
    .expect("decode completeness query response")
    .completeness_result
    .expect("completeness result")
}

async fn list_by_party(
    client: &mut crm_application_runtime::gateway_v1::application_gateway_service_client::ApplicationGatewayServiceClient<tonic::transport::Channel>,
    party_id: &str,
) -> data_quality::ListDataQualityFindingsByPartyResponse {
    let definition = query_definition(LIST_BY_PARTY);
    let response = query(
        client,
        &definition,
        payload(
            &definition,
            data_quality::ListDataQualityFindingsByPartyRequest {
                party_ref: Some(customer::PartyRef {
                    party_id: party_id.to_owned(),
                }),
                status: None,
                severity: None,
                page_size: 1,
                cursor: String::new(),
            },
        ),
    )
    .await
    .expect("list Party findings");
    data_quality::ListDataQualityFindingsByPartyResponse::decode(
        response
            .output
            .expect("finding list output")
            .payload
            .as_slice(),
    )
    .expect("decode finding list")
}

async fn list_assigned(
    client: &mut crm_application_runtime::gateway_v1::application_gateway_service_client::ApplicationGatewayServiceClient<tonic::transport::Channel>,
    cursor: &str,
    page_size: u32,
) -> data_quality::ListAssignedDataQualityFindingsResponse {
    let definition = query_definition(LIST_ASSIGNED);
    let response = query(
        client,
        &definition,
        payload(
            &definition,
            data_quality::ListAssignedDataQualityFindingsRequest {
                assigned_actor_id: None,
                status: None,
                severity: None,
                page_size,
                cursor: cursor.to_owned(),
            },
        ),
    )
    .await
    .expect("list assigned findings");
    data_quality::ListAssignedDataQualityFindingsResponse::decode(
        response
            .output
            .expect("assigned finding list output")
            .payload
            .as_slice(),
    )
    .expect("decode assigned finding list")
}

async fn assign_finding(
    client: &mut crm_application_runtime::gateway_v1::application_gateway_service_client::ApplicationGatewayServiceClient<tonic::transport::Channel>,
    finding_id: &str,
    expected_version: i64,
    assigned_actor_id: Option<&str>,
    key: &str,
) -> data_quality::DataQualityFinding {
    let definition = mutation_definition(ASSIGN);
    let response = mutate(
        client,
        &definition,
        payload(
            &definition,
            data_quality::AssignDataQualityFindingRequest {
                finding_ref: Some(data_quality::DataQualityFindingRef {
                    finding_id: finding_id.to_owned(),
                }),
                assigned_actor_id: assigned_actor_id.map(str::to_owned),
                expected_version,
            },
        ),
        key,
    )
    .await
    .expect("assign Data Quality finding");
    data_quality::AssignDataQualityFindingResponse::decode(
        response
            .output
            .expect("assignment output")
            .payload
            .as_slice(),
    )
    .expect("decode finding assignment")
    .finding
    .expect("assigned finding")
}

async fn acknowledge_finding(
    client: &mut crm_application_runtime::gateway_v1::application_gateway_service_client::ApplicationGatewayServiceClient<tonic::transport::Channel>,
    finding_id: &str,
    observation_id: &str,
    expected_version: i64,
    key: &str,
) -> data_quality::DataQualityFinding {
    let definition = mutation_definition(ACKNOWLEDGE);
    let response = mutate(
        client,
        &definition,
        payload(
            &definition,
            data_quality::AcknowledgeDataQualityFindingRequest {
                finding_ref: Some(data_quality::DataQualityFindingRef {
                    finding_id: finding_id.to_owned(),
                }),
                expected_current_observation_ref: Some(
                    data_quality::DataQualityFindingObservationRef {
                        finding_observation_id: observation_id.to_owned(),
                    },
                ),
                expected_version,
            },
        ),
        key,
    )
    .await
    .expect("acknowledge Data Quality finding");
    data_quality::AcknowledgeDataQualityFindingResponse::decode(
        response
            .output
            .expect("acknowledgement output")
            .payload
            .as_slice(),
    )
    .expect("decode finding acknowledgement")
    .finding
    .expect("acknowledged finding")
}

async fn waive_finding(
    client: &mut crm_application_runtime::gateway_v1::application_gateway_service_client::ApplicationGatewayServiceClient<tonic::transport::Channel>,
    finding_id: &str,
    observation_id: &str,
    expected_version: i64,
    reason: &str,
    key: &str,
) -> data_quality::DataQualityFinding {
    let definition = mutation_definition(WAIVE);
    let response = mutate(
        client,
        &definition,
        payload(
            &definition,
            data_quality::WaiveDataQualityFindingRequest {
                finding_ref: Some(data_quality::DataQualityFindingRef {
                    finding_id: finding_id.to_owned(),
                }),
                expected_current_observation_ref: Some(
                    data_quality::DataQualityFindingObservationRef {
                        finding_observation_id: observation_id.to_owned(),
                    },
                ),
                reason: reason.to_owned(),
                expected_version,
            },
        ),
        key,
    )
    .await
    .expect("waive Data Quality finding");
    data_quality::WaiveDataQualityFindingResponse::decode(
        response.output.expect("waiver output").payload.as_slice(),
    )
    .expect("decode finding waiver")
    .finding
    .expect("waived finding")
}

async fn remediate(
    client: &mut crm_application_runtime::gateway_v1::application_gateway_service_client::ApplicationGatewayServiceClient<tonic::transport::Channel>,
    request: data_quality::RemediatePartyDisplayNameRequest,
    key: &str,
) -> data_quality::RemediatePartyDisplayNameResponse {
    let definition = mutation_definition(REMEDIATE);
    let response = mutate(client, &definition, payload(&definition, request), key)
        .await
        .expect("recover Party display-name remediation");
    data_quality::RemediatePartyDisplayNameResponse::decode(
        response
            .output
            .expect("remediation output")
            .payload
            .as_slice(),
    )
    .expect("decode remediation response")
}

fn resource_version(party: &crm_proto_contracts::crm::parties::v1::Party) -> i64 {
    party
        .resource_version
        .as_ref()
        .expect("Party resource version")
        .version
}

fn wire_resource_version(finding: &data_quality::DataQualityFinding) -> i64 {
    finding
        .resource_version
        .as_ref()
        .expect("finding resource version")
        .version
}

struct PartySnapshot {
    version: i64,
    display_name: String,
}

async fn party_snapshot(store: &PostgresDataStore, party_id: &str) -> PartySnapshot {
    let snapshot = store
        .get_record_for_query(&RecordGetQuery {
            tenant_id: TenantId::try_new(TENANT).unwrap(),
            owner_module_id: ModuleId::try_new(PARTY_MODULE).unwrap(),
            record_type: RecordType::try_new(PARTY_TYPE).unwrap(),
            record_id: RecordId::try_new(party_id).unwrap(),
        })
        .await
        .expect("load Party after remediation")
        .expect("Party record after remediation");
    let json: serde_json::Value =
        serde_json::from_slice(&snapshot.payload.bytes).expect("decode persisted Party state");
    PartySnapshot {
        version: snapshot.version,
        display_name: json["display_name"]
            .as_str()
            .expect("persisted Party display name")
            .to_owned(),
    }
}

struct FindingSnapshot {
    version: i64,
    finding: crm_data_quality::PartyFinding,
}

async fn finding_snapshot(store: &PostgresDataStore, finding_id: &str) -> FindingSnapshot {
    let snapshot = store
        .get_record_for_query(&RecordGetQuery {
            tenant_id: TenantId::try_new(TENANT).unwrap(),
            owner_module_id: ModuleId::try_new(DQ_MODULE).unwrap(),
            record_type: RecordType::try_new(FINDING_TYPE).unwrap(),
            record_id: RecordId::try_new(finding_id).unwrap(),
        })
        .await
        .expect("load finding after reevaluation")
        .expect("finding after reevaluation");
    FindingSnapshot {
        version: snapshot.version,
        finding: decode_finding_state(&snapshot.payload.bytes)
            .expect("decode finding after reevaluation"),
    }
}

async fn record_count(admin: &PgPool, record_type: &str) -> i64 {
    sqlx::query_scalar(
        "SELECT count(*) FROM crm.records WHERE tenant_id = $1 AND owner_module_id = $2 AND record_type = $3 AND deleted_at IS NULL",
    )
    .bind(TENANT)
    .bind(DQ_MODULE)
    .bind(record_type)
    .fetch_one(admin)
    .await
    .expect("count Data Quality records")
}

async fn assert_force_rls_completion_records(
    admin: &PgPool,
    database_url: &str,
    finding_id: &str,
    result_id: &str,
    attempt_id: &str,
) {
    let application = PgPool::connect(database_url)
        .await
        .expect("connect application role for Data Quality RLS proof");
    let current_user: String = sqlx::query("SELECT current_user")
        .fetch_one(&application)
        .await
        .expect("read application database role")
        .get(0);
    assert_eq!(current_user, "crm_app_test");
    for (record_type, record_id) in [
        (FINDING_TYPE, finding_id),
        (RESULT_TYPE, result_id),
        (ATTEMPT_TYPE, attempt_id),
    ] {
        assert_eq!(
            app_role_record_count(&application, OTHER_TENANT, TENANT, record_type, record_id,)
                .await,
            0,
            "FORCE RLS must hide tenant A completion records under tenant B context"
        );
        assert_eq!(
            app_role_record_count(&application, TENANT, TENANT, record_type, record_id).await,
            1,
            "application role must see completion record under owning tenant context"
        );
    }
    let persisted_attempt = list_records(
        &PostgresDataStore::connect(database_url).await.unwrap(),
        ATTEMPT_TYPE,
    )
    .await
    .into_iter()
    .find(|(record_id, _)| record_id == attempt_id)
    .expect("persisted remediation attempt");
    let attempt_bytes = serde_json::to_vec(&persisted_attempt.1).unwrap();
    decode_remediation_attempt_state(&attempt_bytes)
        .expect("strict remediation attempt persisted state");
    let admin_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM crm.records WHERE tenant_id = $1 AND owner_module_id = $2 AND record_type = $3 AND record_id = $4 AND deleted_at IS NULL",
    )
    .bind(TENANT)
    .bind(DQ_MODULE)
    .bind(ATTEMPT_TYPE)
    .bind(attempt_id)
    .fetch_one(admin)
    .await
    .expect("administrator confirms remediation attempt");
    assert_eq!(admin_count, 1);
}

async fn app_role_record_count(
    application: &PgPool,
    context_tenant: &str,
    row_tenant: &str,
    record_type: &str,
    record_id: &str,
) -> i64 {
    let mut transaction = application
        .begin()
        .await
        .expect("begin Data Quality RLS transaction");
    sqlx::query(
        "SELECT set_config('app.tenant_id', $1, true), set_config('app.actor_id', 'actor-evaluation', true), set_config('app.request_id', 'dq-completion-rls-request', true), set_config('app.capability_id', $2, true), set_config('app.capability_version', '1.0.0', true), set_config('app.business_transaction_id', 'dq-completion-rls-transaction', true)",
    )
    .bind(context_tenant)
    .bind(GET_FINDING)
    .execute(&mut *transaction)
    .await
    .expect("set Data Quality RLS context");
    let count = sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM crm.records WHERE tenant_id = $1 AND owner_module_id = $2 AND record_type = $3 AND record_id = $4 AND deleted_at IS NULL",
    )
    .bind(row_tenant)
    .bind(DQ_MODULE)
    .bind(record_type)
    .bind(record_id)
    .fetch_one(&mut *transaction)
    .await
    .expect("read Data Quality completion record through application role");
    transaction
        .rollback()
        .await
        .expect("rollback Data Quality RLS proof");
    count
}
