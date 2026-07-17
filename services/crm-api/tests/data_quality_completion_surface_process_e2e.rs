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

use crm_application_runtime::gateway_v1::application_gateway_service_client::ApplicationGatewayServiceClient;
use crm_core_data::{PostgresDataStore, RecordGetQuery, RecordListQuery, RecordQuerySort};
use crm_data_quality::{
    PartyFindingStatus, decode_finding_state, decode_remediation_attempt_state,
};
use crm_module_sdk::{ModuleId, RecordId, RecordSnapshot, RecordType, TenantId};
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
use tonic::{Code, Status, transport::Channel};

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

type Client = ApplicationGatewayServiceClient<Channel>;

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
    let party_a_version = party_resource_version(&created_a);
    let party_b_version = party_resource_version(&created_b);
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
    let store = PostgresDataStore::connect(&database_url, 6)
        .await
        .expect("connect completion-surface evidence store");
    let evidence_a = evaluation_evidence(&store, &party_a, &job_a).await;
    let evidence_b = evaluation_evidence(&store, &party_b, &job_b).await;

    let tenants = format!("{TENANT},{OTHER_TENANT}");
    let hidden = "data_quality.party.evaluation.get|crm.data-quality|data_quality.party_evaluation_job|failed_rules";
    let environment = [
        ("CRM_API_TENANTS", tenants.as_str()),
        ("CRM_QUERY_HIDDEN_FIELDS", hidden),
    ];
    let mut api = start_with_environment(&database_url, &environment).await;

    let evaluation: data_quality::GetPartyEvaluationJobResponse = query_proto(
        &mut api.client,
        GET_EVALUATION,
        data_quality::GetPartyEvaluationJobRequest {
            evaluation_job_ref: Some(data_quality::PartyEvaluationJobRef {
                evaluation_job_id: job_a.clone(),
            }),
        },
    )
    .await;
    let evaluation = evaluation.evaluation_job.expect("evaluation job");
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

    let evaluation_definition = query_definition(GET_EVALUATION);
    let cross_tenant = query_for_tenant(
        &mut api.client,
        &evaluation_definition,
        payload(
            &evaluation_definition,
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

    let finding_response: data_quality::GetDataQualityFindingResponse = query_proto(
        &mut api.client,
        GET_FINDING,
        data_quality::GetDataQualityFindingRequest {
            finding_ref: Some(data_quality::DataQualityFindingRef {
                finding_id: evidence_a.finding_id.clone(),
            }),
        },
    )
    .await;
    let observation = finding_response
        .current_observation
        .as_ref()
        .expect("visible current finding observation");
    assert_eq!(
        observation.reason_code,
        "DATA_QUALITY_PARTY_DISPLAY_NAME_PLACEHOLDER"
    );
    assert_eq!(
        finding_response
            .finding
            .as_ref()
            .and_then(|finding| finding.party_ref.as_ref())
            .expect("visible finding Party ref")
            .party_id,
        party_a
    );

    let completeness_response: data_quality::GetPartyCompletenessResultResponse = query_proto(
        &mut api.client,
        GET_COMPLETENESS,
        data_quality::GetPartyCompletenessResultRequest {
            completeness_result_ref: Some(data_quality::PartyCompletenessResultRef {
                completeness_result_id: evidence_a.result_id.clone(),
            }),
        },
    )
    .await;
    let completeness = completeness_response
        .completeness_result
        .expect("completeness result");
    assert_eq!(completeness.score_basis_points, 4_000);
    assert_eq!(completeness.components.len(), 2);
    assert!(
        completeness
            .components
            .iter()
            .all(|component| component.rule_outcome_ref.is_some())
    );

    let by_party: data_quality::ListDataQualityFindingsByPartyResponse = query_proto(
        &mut api.client,
        LIST_BY_PARTY,
        data_quality::ListDataQualityFindingsByPartyRequest {
            party_ref: Some(customer::PartyRef {
                party_id: party_a.clone(),
            }),
            status: None,
            severity: None,
            page_size: 1,
            cursor: String::new(),
        },
    )
    .await;
    assert_eq!(by_party.findings.len(), 1);
    assert!(by_party.next_cursor.is_empty());

    let assigned_a = assign_finding(
        &mut api.client,
        &evidence_a.finding_id,
        1,
        &unique_id("completion-assign-a"),
    )
    .await;
    let assigned_b = assign_finding(
        &mut api.client,
        &evidence_b.finding_id,
        1,
        &unique_id("completion-assign-b"),
    )
    .await;
    assert_eq!(finding_resource_version(&assigned_a), 2);
    assert_eq!(finding_resource_version(&assigned_b), 2);

    let assign_definition = mutation_definition(ASSIGN);
    let stale_assign = mutate(
        &mut api.client,
        &assign_definition,
        payload(
            &assign_definition,
            data_quality::AssignDataQualityFindingRequest {
                finding_ref: Some(data_quality::DataQualityFindingRef {
                    finding_id: evidence_a.finding_id.clone(),
                }),
                expected_version: 1,
                assigned_actor_id: None,
            },
        ),
        &unique_id("completion-stale-assign"),
    )
    .await
    .expect_err("stale finding version must conflict");
    assert_ne!(stale_assign.code(), Code::Ok);

    let acknowledged: data_quality::AcknowledgeDataQualityFindingResponse = mutate_proto(
        &mut api.client,
        ACKNOWLEDGE,
        data_quality::AcknowledgeDataQualityFindingRequest {
            finding_ref: Some(data_quality::DataQualityFindingRef {
                finding_id: evidence_a.finding_id.clone(),
            }),
            expected_current_observation_ref: Some(
                data_quality::DataQualityFindingObservationRef {
                    finding_observation_id: evidence_a.observation_id.clone(),
                },
            ),
            expected_version: 2,
        },
        &unique_id("completion-acknowledge"),
    )
    .await;
    assert_eq!(
        acknowledged.finding.expect("acknowledged finding").status,
        data_quality::DataQualityFindingStatus::Acknowledged as i32
    );

    let waived: data_quality::WaiveDataQualityFindingResponse = mutate_proto(
        &mut api.client,
        WAIVE,
        data_quality::WaiveDataQualityFindingRequest {
            finding_ref: Some(data_quality::DataQualityFindingRef {
                finding_id: evidence_a.finding_id.clone(),
            }),
            expected_current_observation_ref: Some(
                data_quality::DataQualityFindingObservationRef {
                    finding_observation_id: evidence_a.observation_id.clone(),
                },
            ),
            expected_version: 3,
            reason: "Accepted source-system exception".to_owned(),
        },
        &unique_id("completion-waive"),
    )
    .await;
    let waived = waived.finding.expect("waived finding");
    assert_eq!(
        waived.status,
        data_quality::DataQualityFindingStatus::Waived as i32
    );
    assert_eq!(
        waived.waiver_reason.as_deref(),
        Some("Accepted source-system exception")
    );

    let first_page = list_assigned(&mut api.client, String::new(), 1).await;
    assert_eq!(first_page.findings.len(), 1);
    assert!(!first_page.next_cursor.is_empty());
    let second_page = list_assigned(&mut api.client, first_page.next_cursor.clone(), 1).await;
    assert_eq!(second_page.findings.len(), 1);
    assert_ne!(
        finding_id(&first_page.findings[0]),
        finding_id(&second_page.findings[0])
    );

    let list_definition = query_definition(LIST_ASSIGNED);
    let invalid_cursor = query(
        &mut api.client,
        &list_definition,
        payload(
            &list_definition,
            data_quality::ListAssignedDataQualityFindingsRequest {
                assigned_actor_id: None,
                status: None,
                severity: None,
                page_size: 1,
                cursor: format!("{}x", first_page.next_cursor),
            },
        ),
    )
    .await
    .expect_err("tampered finding cursor must be rejected");
    assert_eq!(invalid_cursor.code(), Code::InvalidArgument);
    stop(&mut api).await;

    let failpoint_tenants = format!("{TENANT},{OTHER_TENANT}");
    let failpoint_environment = [
        ("CRM_API_TENANTS", failpoint_tenants.as_str()),
        (
            "CRM_DATA_QUALITY_REMEDIATION_FAIL_AFTER_TARGET_ONCE",
            "true",
        ),
    ];
    let mut failpoint_api = start_with_environment(&database_url, &failpoint_environment).await;
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
    let remediation_definition = mutation_definition(REMEDIATE);
    let deferred = mutate(
        &mut failpoint_api.client,
        &remediation_definition,
        payload(&remediation_definition, remediation_request.clone()),
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

    let recovery_tenants = format!("{TENANT},{OTHER_TENANT}");
    let recovery_environment = [("CRM_API_TENANTS", recovery_tenants.as_str())];
    let mut recovery_api = start_with_environment(&database_url, &recovery_environment).await;
    let recovered: data_quality::RemediatePartyDisplayNameResponse = mutate_proto(
        &mut recovery_api.client,
        REMEDIATE,
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
    let attempt_id = recovered
        .remediation_attempt
        .as_ref()
        .and_then(|attempt| attempt.remediation_attempt_ref.as_ref())
        .expect("durable remediation attempt ref")
        .remediation_attempt_id
        .clone();
    assert_eq!(record_count(&admin, ATTEMPT_TYPE).await, 1);
    let replayed: data_quality::RemediatePartyDisplayNameResponse = mutate_proto(
        &mut recovery_api.client,
        REMEDIATE,
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

    let remediated = finding_snapshot(&store, &evidence_b.finding_id).await;
    assert_eq!(remediated.finding.status(), PartyFindingStatus::Remediated);
    assert_eq!(remediated.version, 3);
    assert!(remediated.finding.remediated_by_rule_outcome_id().is_some());
    assert_eq!(record_count(&admin, OBSERVATION_TYPE).await, 2);
    assert_force_rls_completion_records(
        &admin,
        &store,
        &database_url,
        &evidence_b.finding_id,
        &evidence_b.result_id,
        &attempt_id,
    )
    .await;
}

async fn query_proto<Req, Res>(client: &mut Client, capability_id: &str, request: Req) -> Res
where
    Req: Message,
    Res: Message + Default,
{
    let definition = query_definition(capability_id);
    let response = query(client, &definition, payload(&definition, request))
        .await
        .unwrap_or_else(|error| panic!("query {capability_id} failed: {error}"));
    Res::decode(
        response
            .output
            .unwrap_or_else(|| panic!("query {capability_id} output is missing"))
            .payload
            .as_slice(),
    )
    .unwrap_or_else(|error| panic!("decode query {capability_id} response: {error}"))
}

async fn mutate_proto<Req, Res>(
    client: &mut Client,
    capability_id: &str,
    request: Req,
    idempotency_key: &str,
) -> Res
where
    Req: Message,
    Res: Message + Default,
{
    let definition = mutation_definition(capability_id);
    let response = mutate(
        client,
        &definition,
        payload(&definition, request),
        idempotency_key,
    )
    .await
    .unwrap_or_else(|error| panic!("mutation {capability_id} failed: {error}"));
    Res::decode(
        response
            .output
            .unwrap_or_else(|| panic!("mutation {capability_id} output is missing"))
            .payload
            .as_slice(),
    )
    .unwrap_or_else(|error| panic!("decode mutation {capability_id} response: {error}"))
}

async fn assign_finding(
    client: &mut Client,
    finding_id: &str,
    expected_version: i64,
    idempotency_key: &str,
) -> data_quality::DataQualityFinding {
    let response: data_quality::AssignDataQualityFindingResponse = mutate_proto(
        client,
        ASSIGN,
        data_quality::AssignDataQualityFindingRequest {
            finding_ref: Some(data_quality::DataQualityFindingRef {
                finding_id: finding_id.to_owned(),
            }),
            expected_version,
            assigned_actor_id: Some("actor-a".to_owned()),
        },
        idempotency_key,
    )
    .await;
    response.finding.expect("assigned finding")
}

async fn list_assigned(
    client: &mut Client,
    cursor: String,
    page_size: i32,
) -> data_quality::ListAssignedDataQualityFindingsResponse {
    query_proto(
        client,
        LIST_ASSIGNED,
        data_quality::ListAssignedDataQualityFindingsRequest {
            assigned_actor_id: None,
            status: None,
            severity: None,
            page_size,
            cursor,
        },
    )
    .await
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
            return;
        }
    }
    panic!("Data Quality completion-surface worker did not become idle");
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
    let findings = list_snapshots(store, FINDING_TYPE).await;
    let finding_snapshot = findings
        .into_iter()
        .find(|snapshot| {
            decode_finding_state(&snapshot.payload.bytes)
                .is_ok_and(|finding| finding.party_id().as_str() == party_id)
        })
        .expect("finding for evaluated Party");
    let finding = decode_finding_state(&finding_snapshot.payload.bytes).expect("decode finding");
    let results = list_snapshots(store, RESULT_TYPE).await;
    let result_snapshot = results
        .into_iter()
        .find(|snapshot| {
            serde_json::from_slice::<serde_json::Value>(&snapshot.payload.bytes)
                .is_ok_and(|value| value["job_id"] == job_id)
        })
        .expect("completeness result for evaluation job");
    EvaluationEvidence {
        finding_id: finding_snapshot.reference.record_id.as_str().to_owned(),
        observation_id: finding.current_observation_id().to_owned(),
        result_id: result_snapshot.reference.record_id.as_str().to_owned(),
    }
}

async fn list_snapshots(store: &PostgresDataStore, record_type: &str) -> Vec<RecordSnapshot> {
    let page = store
        .list_records_for_query(&RecordListQuery {
            tenant_id: TenantId::try_new(TENANT).unwrap(),
            owner_module_id: ModuleId::try_new(DQ_MODULE).unwrap(),
            record_type: RecordType::try_new(record_type).unwrap(),
            page_size: 200,
            sort: RecordQuerySort::CreatedAtAscending,
            after: None,
        })
        .await
        .expect("list Data Quality completion-surface records");
    assert!(page.next.is_none());
    page.records
}

fn party_resource_version(party: &crm_proto_contracts::crm::parties::v1::Party) -> i64 {
    party
        .resource_version
        .as_ref()
        .expect("Party resource version")
        .version
}

fn finding_resource_version(finding: &data_quality::DataQualityFinding) -> i64 {
    finding
        .resource_version
        .as_ref()
        .expect("finding resource version")
        .version
}

fn finding_id(finding: &data_quality::DataQualityFinding) -> &str {
    finding
        .finding_ref
        .as_ref()
        .expect("finding ref")
        .finding_id
        .as_str()
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
    store: &PostgresDataStore,
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
            app_role_record_count(&application, OTHER_TENANT, TENANT, record_type, record_id).await,
            0,
            "FORCE RLS must hide tenant A completion records under tenant B context"
        );
        assert_eq!(
            app_role_record_count(&application, TENANT, TENANT, record_type, record_id).await,
            1,
            "application role must see completion record under owning tenant context"
        );
    }

    let attempt_snapshot = store
        .get_record_for_query(&RecordGetQuery {
            tenant_id: TenantId::try_new(TENANT).unwrap(),
            owner_module_id: ModuleId::try_new(DQ_MODULE).unwrap(),
            record_type: RecordType::try_new(ATTEMPT_TYPE).unwrap(),
            record_id: RecordId::try_new(attempt_id).unwrap(),
        })
        .await
        .expect("load persisted remediation attempt")
        .expect("persisted remediation attempt");
    decode_remediation_attempt_state(&attempt_snapshot.payload.bytes)
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
        "SELECT set_config('app.tenant_id', $1, true), set_config('app.actor_id', 'actor-a', true), set_config('app.request_id', 'dq-completion-rls-request', true), set_config('app.capability_id', $2, true), set_config('app.capability_version', '1.0.0', true), set_config('app.business_transaction_id', 'dq-completion-rls-transaction', true)",
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

fn _status_is_used(_: Status) {}
