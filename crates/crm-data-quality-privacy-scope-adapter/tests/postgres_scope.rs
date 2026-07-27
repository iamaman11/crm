#![allow(clippy::too_many_arguments)]

use crm_capability_plan_support::{self as support, PersistedPayloadContract};
use crm_capability_runtime::{
    CapabilityDefinition, CapabilityRequest, TransactionalCapabilityExecutor,
};
use crm_core_data::{PostgresDataStore, PostgresTransactionalAggregateExecutor};
use crm_customer_privacy::{CANONICAL_SCOPE_REGISTRY_VERSION, OwnerScopeRegistry};
use crm_data_quality::{
    ComponentKey, EvaluatedPartyKind, PartyCompletenessComponent, PartyCompletenessProfileVersion,
    PartyCompletenessResult, PartyDisplayNameRemediationAttempt,
    PartyDisplayNameRemediationIdentity, PartyEvaluationInputSnapshot, PartyEvaluationJob,
    PartyFinding, PartyFindingObservation, PartyQualityEvaluator, PartyQualityInput,
    PartyQualityRule, PartyRuleOutcome, PartyRuleSetVersion, QualitySeverity, RuleKey,
    encode_finding_observation_state, encode_finding_state,
    encode_party_completeness_profile_version_state, encode_party_completeness_result_state,
    encode_party_evaluation_input_state, encode_party_evaluation_job_state,
    encode_party_rule_set_version_state, encode_remediation_attempt_state,
    encode_rule_outcome_state,
};
use crm_data_quality_capability_adapter::{
    party_completeness_profile_persisted_contract, party_completeness_result_persisted_contract,
    party_evaluation_input_persisted_contract, party_evaluation_job_persisted_contract,
    party_finding_observation_persisted_contract, party_finding_persisted_contract,
    party_rule_outcome_persisted_contract, party_rule_set_persisted_contract,
    remediation_attempt_persisted_contract,
};
use crm_data_quality_privacy_scope_adapter::{
    CAPABILITY_ID, CAPABILITY_VERSION, CONTRACT_SCHEMA_VERSION,
    DataQualityPrivacyScopeQueryAdapter, INPUT_MAXIMUM_BYTES, INPUT_RETENTION_POLICY_ID,
    INPUT_SCHEMA_ID, data_quality_privacy_scope_definition,
};
use crm_identity_resolution_capability_adapter::{
    IdentityResolutionCapabilityPlanner, MERGE_CAPABILITY,
    capability_definition as identity_definition,
};
use crm_module_sdk::{
    ActorId, BusinessTransactionId, CapabilityId, CapabilityVersion, CausationId, CorrelationId,
    DataClass, ExecutionContext, IdempotencyKey, ModuleExecutionContext, ModuleId, PayloadEncoding,
    RecordId, RequestId, RetentionPolicyId, SchemaId, SchemaVersion, TenantId, TraceId,
    TypedPayload,
};
use crm_parties_capability_adapter::{
    CREATE_CAPABILITY as CREATE_PARTY_CAPABILITY, PartyCapabilityPlanner,
    capability_definition as party_definition,
};
use crm_proto_contracts::{
    crm::{
        customer::v1 as customer, customer_privacy::v1 as privacy,
        identity_resolution::v1 as identity, parties::v1 as parties,
    },
    message_descriptor_hash,
};
use crm_query_runtime::{QueryExecutionContext, QueryExecutor, QueryRequest};
use prost::Message;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use std::collections::BTreeMap;
use std::sync::Arc;

const TENANT_A: &str = "tenant-a";
const TENANT_B: &str = "tenant-b";
const ACTOR: &str = "privacy-worker";

#[derive(Debug, Clone)]
struct GraphIds {
    job: String,
    input: String,
    outcome: String,
    finding: String,
    observation: String,
    completeness: String,
    remediation: String,
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn data_quality_scope_is_alias_aware_strict_complete_minimized_and_side_effect_free() {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!("skipping Data Quality privacy scope proof without DATABASE_URL");
        return;
    };
    let admin_url = std::env::var("ADMIN_DATABASE_URL")
        .expect("ADMIN_DATABASE_URL must accompany DATABASE_URL");

    let store = PostgresDataStore::connect(&database_url, 8)
        .await
        .expect("connect Data Quality privacy scope runtime store");
    let admin = PgPool::connect(&admin_url)
        .await
        .expect("connect Data Quality privacy scope evidence reader");
    let party_executor: Arc<dyn TransactionalCapabilityExecutor> =
        Arc::new(PostgresTransactionalAggregateExecutor::new(
            store.clone(),
            Arc::new(PartyCapabilityPlanner),
        ));
    let identity_executor: Arc<dyn TransactionalCapabilityExecutor> =
        Arc::new(PostgresTransactionalAggregateExecutor::new(
            store.clone(),
            Arc::new(IdentityResolutionCapabilityPlanner),
        ));
    let party_create = party_definition(CREATE_PARTY_CAPABILITY).unwrap();
    let merge_execute = identity_definition(MERGE_CAPABILITY).unwrap();

    for (party_id, seed) in [
        ("party-canonical", 11),
        ("party-alias", 12),
        ("party-unrelated", 13),
    ] {
        create_party(&party_executor, &party_create, TENANT_A, party_id, seed).await;
    }
    create_party(
        &party_executor,
        &party_create,
        TENANT_B,
        "party-canonical",
        21,
    )
    .await;
    merge_party(
        &identity_executor,
        &merge_execute,
        TENANT_A,
        "merge-data-quality-alias",
        "party-alias",
        "party-canonical",
        31,
    )
    .await;

    let (rule_set, profile) = definitions();
    insert_definitions(&admin, TENANT_A, &rule_set, &profile).await;
    insert_definitions(&admin, TENANT_B, &rule_set, &profile).await;

    let alias = insert_complete_graph(
        &admin,
        TENANT_A,
        "dq-job-001",
        "party-alias",
        "Private Bad Name Alpha",
        "Private Corrected Name Alpha",
        100,
        &rule_set,
        &profile,
    )
    .await;
    let canonical = insert_complete_graph(
        &admin,
        TENANT_A,
        "dq-job-002",
        "party-canonical",
        "Private Bad Name Beta",
        "Private Corrected Name Beta",
        200,
        &rule_set,
        &profile,
    )
    .await;
    let unrelated = insert_complete_graph(
        &admin,
        TENANT_A,
        "dq-job-003",
        "party-unrelated",
        "Private Bad Name Gamma",
        "Private Corrected Name Gamma",
        300,
        &rule_set,
        &profile,
    )
    .await;
    let tenant_b = insert_complete_graph(
        &admin,
        TENANT_B,
        "dq-job-004",
        "party-canonical",
        "Private Bad Name Delta",
        "Private Corrected Name Delta",
        400,
        &rule_set,
        &profile,
    )
    .await;

    prove_records_primary_key_scan(&admin, TENANT_A).await;

    let generation_a = current_generation(&admin, TENANT_A).await;
    let definition = data_quality_privacy_scope_definition().unwrap();
    let adapter = DataQualityPrivacyScopeQueryAdapter::new(store);
    let before = write_surface_counts(&admin).await;

    let mut cursor = String::new();
    let mut page = 1_u32;
    let mut resources = Vec::new();
    let mut encoded_pages = Vec::new();
    loop {
        let result = adapter
            .execute(
                &definition,
                scope_request(
                    TENANT_A,
                    "party-canonical",
                    generation_a,
                    3,
                    &cursor,
                    "data-quality-pages",
                ),
            )
            .await
            .expect("enumerate authoritative Data Quality privacy scope");
        assert_eq!(write_surface_counts(&admin).await, before);
        assert_response_omits_private_data_quality_values(&result.output.bytes);
        encoded_pages.push(result.output.bytes.clone());
        let response = decode(&result.output.bytes);
        let contribution = response.contribution.unwrap();
        assert_eq!(contribution.owner_module_id, crm_data_quality::MODULE_ID);
        assert_eq!(contribution.capability_id, CAPABILITY_ID);
        assert_eq!(contribution.capability_version, CAPABILITY_VERSION);
        let evidence = contribution.page_evidence.unwrap();
        assert_eq!(evidence.page_number, page);
        assert_eq!(evidence.scanned_resource_count, 21);
        assert_eq!(
            evidence.emitted_resource_count as usize,
            contribution.resources.len()
        );
        resources.extend(contribution.resources);
        if evidence.terminal_complete {
            assert!(evidence.next_cursor.is_empty());
            break;
        }
        assert!(!evidence.next_cursor.is_empty());
        cursor = evidence.next_cursor;
        page += 1;
        assert!(page <= 6, "seven-family pagination must terminate");
    }

    assert_eq!(page, 5);
    assert_eq!(resources.len(), 14);
    let by_type = resources.into_iter().fold(
        BTreeMap::<String, Vec<String>>::new(),
        |mut output, resource| {
            assert_eq!(
                resource.data_class,
                privacy::CustomerDataClass::Personal as i32
            );
            assert_eq!(
                resource.evidence_class,
                privacy::PrivacyScopeEvidenceClass::RetainMinimizedEvidence as i32
            );
            output
                .entry(resource.resource_type)
                .or_default()
                .push(resource.resource_id);
            output
        },
    );
    assert_eq!(
        by_type,
        expected_two_graphs(&alias, &canonical),
        "exact seven-family reference inventory must be stable"
    );
    assert!(!by_type.contains_key("data_quality.party_rule_set_version"));
    assert!(!by_type.contains_key("data_quality.party_completeness_profile_version"));

    for forbidden_id in graph_values(&unrelated) {
        assert!(
            encoded_pages
                .iter()
                .all(|bytes| !contains(bytes, forbidden_id)),
            "unrelated resource leaked: {forbidden_id}"
        );
    }
    for forbidden_id in graph_values(&tenant_b) {
        assert!(
            encoded_pages
                .iter()
                .all(|bytes| !contains(bytes, forbidden_id)),
            "cross-tenant resource leaked: {forbidden_id}"
        );
    }

    let generation_b = current_generation(&admin, TENANT_B).await;
    let tenant_b_response = adapter
        .execute(
            &definition,
            scope_request(
                TENANT_B,
                "party-canonical",
                generation_b,
                20,
                "",
                "data-quality-tenant-b",
            ),
        )
        .await
        .expect("enumerate tenant B Data Quality scope");
    assert_eq!(
        decode(&tenant_b_response.output.bytes)
            .contribution
            .unwrap()
            .resources
            .len(),
        7
    );
    assert_eq!(write_surface_counts(&admin).await, before);

    let stale = adapter
        .execute(
            &definition,
            scope_request(
                TENANT_A,
                "party-canonical",
                generation_a - 1,
                3,
                "",
                "data-quality-stale",
            ),
        )
        .await
        .expect_err("stale topology generation must fail closed");
    assert_eq!(stale.code, "DATA_QUALITY_PRIVACY_SCOPE_LINEAGE_INVALID");
    assert_eq!(write_surface_counts(&admin).await, before);

    let first_page = decode(&encoded_pages[0])
        .contribution
        .unwrap()
        .page_evidence
        .unwrap();
    let rebound = adapter
        .execute(
            &definition,
            scope_request(
                TENANT_A,
                "party-canonical",
                generation_a,
                4,
                &first_page.next_cursor,
                "data-quality-pages",
            ),
        )
        .await
        .expect_err("cursor page-size rebinding must fail closed");
    assert_eq!(rebound.code, "DATA_QUALITY_PRIVACY_SCOPE_CURSOR_INVALID");
    assert_eq!(write_surface_counts(&admin).await, before);

    set_record_data_class(&admin, TENANT_A, &unrelated.remediation, "confidential").await;
    let malformed_baseline = write_surface_counts(&admin).await;
    let malformed = adapter
        .execute(
            &definition,
            scope_request(
                TENANT_A,
                "party-canonical",
                generation_a,
                20,
                "",
                "data-quality-malformed",
            ),
        )
        .await
        .expect_err("malformed unrelated owner persistence must fail closed");
    assert_eq!(
        malformed.code,
        "DATA_QUALITY_PRIVACY_SCOPE_STORED_STATE_INVALID"
    );
    assert_eq!(write_surface_counts(&admin).await, malformed_baseline);
    set_record_data_class(&admin, TENANT_A, &unrelated.remediation, "personal").await;

    delete_record(
        &admin,
        TENANT_A,
        "data_quality.finding_observation",
        &alias.observation,
    )
    .await;
    let association_baseline = write_surface_counts(&admin).await;
    let orphaned = adapter
        .execute(
            &definition,
            scope_request(
                TENANT_A,
                "party-canonical",
                generation_a,
                20,
                "",
                "data-quality-orphaned",
            ),
        )
        .await
        .expect_err("missing current observation must fail closed");
    assert_eq!(
        orphaned.code,
        "DATA_QUALITY_PRIVACY_SCOPE_ASSOCIATION_STATE_INVALID"
    );
    assert_eq!(write_surface_counts(&admin).await, association_baseline);
}

fn definitions() -> (PartyRuleSetVersion, PartyCompletenessProfileVersion) {
    let rule_key = RuleKey::try_new("display_name_placeholder").unwrap();
    let rule = PartyQualityRule::try_new(
        rule_key.clone(),
        QualitySeverity::Warning,
        PartyQualityEvaluator::display_name_placeholder_exact_ascii_casefold(vec![
            "private bad name alpha".to_owned(),
            "private bad name beta".to_owned(),
            "private bad name gamma".to_owned(),
            "private bad name delta".to_owned(),
        ])
        .unwrap(),
        "Private Data Quality rule title",
        "Private Data Quality remediation guidance",
    )
    .unwrap();
    let rule_set = PartyRuleSetVersion::publish(vec![rule]).unwrap();
    let profile = PartyCompletenessProfileVersion::publish(
        &rule_set,
        vec![
            PartyCompletenessComponent::try_new(
                ComponentKey::try_new("display_name").unwrap(),
                rule_key,
                10_000,
            )
            .unwrap(),
        ],
    )
    .unwrap();
    (rule_set, profile)
}

async fn insert_definitions(
    admin: &PgPool,
    tenant: &str,
    rule_set: &PartyRuleSetVersion,
    profile: &PartyCompletenessProfileVersion,
) {
    insert_record(
        admin,
        tenant,
        "data_quality.party_rule_set_version",
        rule_set.version_id().as_str(),
        1,
        party_rule_set_persisted_contract(),
        DataClass::Confidential,
        encode_party_rule_set_version_state(rule_set).unwrap(),
    )
    .await;
    insert_record(
        admin,
        tenant,
        "data_quality.party_completeness_profile_version",
        profile.version_id().as_str(),
        1,
        party_completeness_profile_persisted_contract(),
        DataClass::Confidential,
        encode_party_completeness_profile_version_state(profile).unwrap(),
    )
    .await;
}

async fn insert_complete_graph(
    admin: &PgPool,
    tenant: &str,
    job_id: &str,
    party_id: &str,
    private_display_name: &str,
    private_corrected_name: &str,
    seed: i64,
    rule_set: &PartyRuleSetVersion,
    profile: &PartyCompletenessProfileVersion,
) -> GraphIds {
    let created = PartyEvaluationJob::create(
        RecordId::try_new(job_id).unwrap(),
        RecordId::try_new(party_id).unwrap(),
        rule_set,
        profile,
        seed,
    )
    .unwrap();
    let (staged, input): (PartyEvaluationJob, PartyEvaluationInputSnapshot) = created
        .stage(
            EvaluatedPartyKind::Person,
            private_display_name,
            1,
            seed + 1,
        )
        .unwrap();
    let quality_input =
        PartyQualityInput::try_new(EvaluatedPartyKind::Person, private_display_name).unwrap();
    let evaluations = rule_set.evaluate(&quality_input);
    let outcomes = evaluations
        .iter()
        .map(|evaluation| PartyRuleOutcome::evaluate(&staged, evaluation, seed + 2).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(outcomes.len(), 1);
    assert!(!outcomes[0].passed());
    let completeness =
        PartyCompletenessResult::compute(&staged, profile, &outcomes, seed + 2).unwrap();
    let materialized = staged.record_materialized_outcomes(1, 1, seed + 2).unwrap();
    let completed = materialized.complete(1, 1, seed + 3).unwrap();
    let rule = rule_set.rule(outcomes[0].rule_key()).unwrap();
    let observation = PartyFindingObservation::observe_failure(
        TenantId::try_new(tenant).unwrap(),
        rule,
        &outcomes[0],
    )
    .unwrap();
    let finding = PartyFinding::open(rule, &observation).unwrap();
    let caller = IdempotencyKey::try_new(format!("private-caller-{job_id}")).unwrap();
    let remediation_identity = PartyDisplayNameRemediationIdentity::derive(
        &TenantId::try_new(tenant).unwrap(),
        &caller,
        &finding,
        1,
        observation.observation_id(),
        1,
        private_corrected_name,
    )
    .unwrap();
    let remediation = PartyDisplayNameRemediationAttempt::complete(
        TenantId::try_new(tenant).unwrap(),
        remediation_identity,
        &finding,
        1,
        observation.observation_id(),
        1,
        private_corrected_name,
        2,
        seed + 4,
    )
    .unwrap();

    insert_record(
        admin,
        tenant,
        "data_quality.party_evaluation_job",
        completed.job_id().as_str(),
        3,
        party_evaluation_job_persisted_contract(),
        DataClass::Personal,
        encode_party_evaluation_job_state(&completed).unwrap(),
    )
    .await;
    insert_record(
        admin,
        tenant,
        "data_quality.party_evaluation_input",
        input.job_id().as_str(),
        1,
        party_evaluation_input_persisted_contract(),
        DataClass::Personal,
        encode_party_evaluation_input_state(&input).unwrap(),
    )
    .await;
    insert_record(
        admin,
        tenant,
        "data_quality.rule_outcome",
        outcomes[0].outcome_id(),
        1,
        party_rule_outcome_persisted_contract(),
        DataClass::Personal,
        encode_rule_outcome_state(&outcomes[0]).unwrap(),
    )
    .await;
    insert_record(
        admin,
        tenant,
        "data_quality.finding",
        finding.finding_id(),
        1,
        party_finding_persisted_contract(),
        DataClass::Personal,
        encode_finding_state(&finding).unwrap(),
    )
    .await;
    insert_record(
        admin,
        tenant,
        "data_quality.finding_observation",
        observation.observation_id(),
        1,
        party_finding_observation_persisted_contract(),
        DataClass::Personal,
        encode_finding_observation_state(&observation).unwrap(),
    )
    .await;
    insert_record(
        admin,
        tenant,
        "data_quality.party_completeness_result",
        completeness.result_id(),
        1,
        party_completeness_result_persisted_contract(),
        DataClass::Personal,
        encode_party_completeness_result_state(&completeness).unwrap(),
    )
    .await;
    insert_record(
        admin,
        tenant,
        "data_quality.remediation_attempt",
        remediation.attempt_id(),
        1,
        remediation_attempt_persisted_contract(),
        DataClass::Personal,
        encode_remediation_attempt_state(&remediation).unwrap(),
    )
    .await;

    GraphIds {
        job: completed.job_id().as_str().to_owned(),
        input: input.job_id().as_str().to_owned(),
        outcome: outcomes[0].outcome_id().to_owned(),
        finding: finding.finding_id().to_owned(),
        observation: observation.observation_id().to_owned(),
        completeness: completeness.result_id().to_owned(),
        remediation: remediation.attempt_id().to_owned(),
    }
}

async fn insert_record(
    admin: &PgPool,
    tenant: &str,
    record_type: &str,
    record_id: &str,
    version: i64,
    contract: PersistedPayloadContract<'_>,
    data_class: DataClass,
    payload_bytes: Vec<u8>,
) {
    let mut transaction = admin
        .begin()
        .await
        .expect("begin isolated Data Quality fixture transaction");
    sqlx::query("SET LOCAL session_replication_role = 'replica'")
        .execute(&mut *transaction)
        .await
        .expect("disable production triggers for isolated Data Quality fixture");
    sqlx::query(
        r#"
        INSERT INTO crm.records (
          tenant_id, record_type, record_id, version, owner_module_id,
          schema_id, schema_version, descriptor_hash, data_class, payload_encoding,
          maximum_payload_size, retention_policy_id, payload_bytes,
          last_business_transaction_id
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, 'json', $10, $11, $12, $13)
        "#,
    )
    .bind(tenant)
    .bind(record_type)
    .bind(record_id)
    .bind(version)
    .bind(contract.owner)
    .bind(contract.schema_id)
    .bind(contract.schema_version)
    .bind(contract.descriptor_hash.as_slice())
    .bind(match data_class {
        DataClass::Confidential => "confidential",
        DataClass::Personal => "personal",
        _ => panic!("unsupported fixture data class"),
    })
    .bind(i64::try_from(contract.maximum_size_bytes).unwrap())
    .bind(contract.retention_policy_id)
    .bind(payload_bytes)
    .bind(format!("fixture-{record_id}"))
    .execute(&mut *transaction)
    .await
    .expect("insert exact authoritative Data Quality fixture");
    sqlx::query("SET LOCAL session_replication_role = 'origin'")
        .execute(&mut *transaction)
        .await
        .expect("restore production trigger mode after Data Quality fixture");
    transaction
        .commit()
        .await
        .expect("commit isolated Data Quality fixture transaction");
}

async fn create_party(
    executor: &Arc<dyn TransactionalCapabilityExecutor>,
    definition: &CapabilityDefinition,
    tenant: &str,
    party_id: &str,
    seed: u8,
) {
    executor
        .execute(
            definition,
            capability_request(
                crm_parties::MODULE_ID,
                CREATE_PARTY_CAPABILITY,
                crm_parties_capability_adapter::CREATE_REQUEST_SCHEMA,
                tenant,
                &format!("party-{party_id}"),
                100_000_000 + i64::from(seed),
                &parties::CreatePartyRequest {
                    party_ref: Some(customer::PartyRef {
                        party_id: party_id.to_owned(),
                    }),
                    kind: parties::PartyKind::Person as i32,
                    display_name: format!("Data Quality Privacy Subject {party_id}"),
                },
            ),
        )
        .await
        .expect("create authoritative Party fixture");
}

async fn merge_party(
    executor: &Arc<dyn TransactionalCapabilityExecutor>,
    definition: &CapabilityDefinition,
    tenant: &str,
    operation_id: &str,
    source_party_id: &str,
    survivor_party_id: &str,
    seed: u8,
) {
    executor
        .execute(
            definition,
            capability_request(
                crm_identity_resolution::MODULE_ID,
                MERGE_CAPABILITY,
                crm_identity_resolution_capability_adapter::MERGE_REQUEST_SCHEMA,
                tenant,
                &format!("merge-{operation_id}"),
                200_000_000 + i64::from(seed),
                &identity::MergePartyRequest {
                    merge_operation_ref: Some(identity::MergeOperationRef {
                        merge_operation_id: operation_id.to_owned(),
                    }),
                    source_party_ref: Some(customer::PartyRef {
                        party_id: source_party_id.to_owned(),
                    }),
                    source_party_version: 1,
                    survivor_party_ref: Some(customer::PartyRef {
                        party_id: survivor_party_id.to_owned(),
                    }),
                    survivor_party_version: 1,
                    decision_ref: format!("approval://{operation_id}"),
                    reason: "duplicate.confirmed".to_owned(),
                    survivorship: vec![identity::SurvivorshipSelection {
                        field_path: "display_name".to_owned(),
                        provenance_party_ref: Some(customer::PartyRef {
                            party_id: source_party_id.to_owned(),
                        }),
                        provenance_party_version: 1,
                        source_value_sha256: [seed; 32].to_vec(),
                        evidence_ref: format!("evidence://{operation_id}"),
                    }],
                },
            ),
        )
        .await
        .expect("create authoritative alias merge fixture");
}

fn capability_request<M: Message>(
    module_id: &str,
    capability_id: &str,
    input_schema: &str,
    tenant: &str,
    identity: &str,
    started_at: i64,
    command: &M,
) -> CapabilityRequest {
    let bytes = command.encode_to_vec();
    let input = TypedPayload {
        owner: ModuleId::try_new(module_id).unwrap(),
        schema_id: SchemaId::try_new(input_schema).unwrap(),
        schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
        descriptor_hash: message_descriptor_hash(input_schema),
        data_class: DataClass::Personal,
        encoding: PayloadEncoding::Protobuf,
        maximum_size_bytes: support::MAX_PROTOBUF_BYTES,
        retention_policy_id: RetentionPolicyId::try_new("standard").unwrap(),
        bytes,
    };
    CapabilityRequest {
        context: ModuleExecutionContext {
            module_id: ModuleId::try_new(module_id).unwrap(),
            execution: ExecutionContext {
                tenant_id: TenantId::try_new(tenant).unwrap(),
                actor_id: ActorId::try_new("actor-a").unwrap(),
                request_id: RequestId::try_new(format!("request-{identity}")).unwrap(),
                correlation_id: CorrelationId::try_new(format!("correlation-{identity}")).unwrap(),
                causation_id: CausationId::try_new(format!("causation-{identity}")).unwrap(),
                trace_id: TraceId::try_new(format!("trace-{identity}")).unwrap(),
                capability_id: CapabilityId::try_new(capability_id).unwrap(),
                capability_version: CapabilityVersion::try_new("1.0.0").unwrap(),
                idempotency_key: IdempotencyKey::try_new(format!("{identity}-key")).unwrap(),
                business_transaction_id: BusinessTransactionId::try_new(format!("{identity}-tx"))
                    .unwrap(),
                schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
                request_started_at_unix_nanos: started_at,
            },
        },
        input_hash: Sha256::digest(&input.bytes).into(),
        input,
        approval: None,
    }
}

fn scope_request(
    tenant: &str,
    party_id: &str,
    generation: u64,
    page_size: u32,
    cursor: &str,
    identity: &str,
) -> QueryRequest {
    let registry = OwnerScopeRegistry::canonical_v1().unwrap();
    let wire = privacy::DataQualityPrivacyScopeContributionRequest {
        contribution: Some(privacy::PrivacyScopeContributionRequestEnvelope {
            lineage: Some(privacy::PrivacyScopeContributionLineage {
                privacy_case_id: format!("privacy-case-{identity}"),
                tenant_id: tenant.to_owned(),
                canonical_party_ref: Some(customer::PartyRef {
                    party_id: party_id.to_owned(),
                }),
                identity_resolution_generation: generation,
                registry_version: CANONICAL_SCOPE_REGISTRY_VERSION.to_owned(),
                registry_digest_sha256: registry.digest().to_vec(),
                purpose_code: "PRIVACY_ERASURE_SCOPE".to_owned(),
                effective_request_at_unix_ms: 1_000,
            }),
            page_size,
            cursor: cursor.to_owned(),
        }),
    };
    let bytes = wire.encode_to_vec();
    QueryRequest {
        owner_module_id: ModuleId::try_new(crm_data_quality::MODULE_ID).unwrap(),
        context: QueryExecutionContext {
            tenant_id: TenantId::try_new(tenant).unwrap(),
            actor_id: ActorId::try_new(ACTOR).unwrap(),
            request_id: RequestId::try_new(format!("request-{identity}")).unwrap(),
            correlation_id: CorrelationId::try_new(format!("correlation-{identity}")).unwrap(),
            trace_id: TraceId::try_new(format!("trace-{identity}")).unwrap(),
            capability_id: CapabilityId::try_new(CAPABILITY_ID).unwrap(),
            capability_version: CapabilityVersion::try_new(CAPABILITY_VERSION).unwrap(),
            schema_version: SchemaVersion::try_new(CONTRACT_SCHEMA_VERSION).unwrap(),
            request_started_at_unix_nanos: 2_000_000_000,
        },
        input: TypedPayload {
            owner: ModuleId::try_new(crm_data_quality::MODULE_ID).unwrap(),
            schema_id: SchemaId::try_new(INPUT_SCHEMA_ID).unwrap(),
            schema_version: SchemaVersion::try_new(CONTRACT_SCHEMA_VERSION).unwrap(),
            descriptor_hash: message_descriptor_hash(INPUT_SCHEMA_ID),
            data_class: DataClass::Confidential,
            encoding: PayloadEncoding::Protobuf,
            maximum_size_bytes: INPUT_MAXIMUM_BYTES,
            retention_policy_id: RetentionPolicyId::try_new(INPUT_RETENTION_POLICY_ID).unwrap(),
            bytes: bytes.clone(),
        },
        input_hash: Sha256::digest(&bytes).into(),
    }
}

async fn current_generation(admin: &PgPool, tenant: &str) -> u64 {
    let mut transaction = admin.begin().await.unwrap();
    sqlx::query("SELECT set_config('app.tenant_id', $1, true)")
        .bind(tenant)
        .execute(&mut *transaction)
        .await
        .unwrap();
    let generation: i64 =
        sqlx::query_scalar("SELECT crm.current_identity_resolution_generation($1)")
            .bind(tenant)
            .fetch_one(&mut *transaction)
            .await
            .unwrap();
    transaction.commit().await.unwrap();
    u64::try_from(generation).unwrap()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct WriteSurfaceCounts {
    records: i64,
    relationships: i64,
    business_transactions: i64,
    idempotency_records: i64,
    outbox_events: i64,
    outbox_delivery: i64,
    audit_heads: i64,
    audit_records: i64,
}

async fn write_surface_counts(pool: &PgPool) -> WriteSurfaceCounts {
    WriteSurfaceCounts {
        records: count(pool, "crm.records").await,
        relationships: count(pool, "crm.relationships").await,
        business_transactions: count(pool, "crm.business_transactions").await,
        idempotency_records: count(pool, "crm.idempotency_records").await,
        outbox_events: count(pool, "crm.outbox_events").await,
        outbox_delivery: count(pool, "crm.outbox_delivery").await,
        audit_heads: count(pool, "crm.audit_heads").await,
        audit_records: count(pool, "crm.audit_records").await,
    }
}

async fn count(pool: &PgPool, table: &str) -> i64 {
    match table {
        "crm.records" => sqlx::query_scalar("SELECT count(*)::bigint FROM crm.records"),
        "crm.relationships" => sqlx::query_scalar("SELECT count(*)::bigint FROM crm.relationships"),
        "crm.business_transactions" => {
            sqlx::query_scalar("SELECT count(*)::bigint FROM crm.business_transactions")
        }
        "crm.idempotency_records" => {
            sqlx::query_scalar("SELECT count(*)::bigint FROM crm.idempotency_records")
        }
        "crm.outbox_events" => sqlx::query_scalar("SELECT count(*)::bigint FROM crm.outbox_events"),
        "crm.outbox_delivery" => {
            sqlx::query_scalar("SELECT count(*)::bigint FROM crm.outbox_delivery")
        }
        "crm.audit_heads" => sqlx::query_scalar("SELECT count(*)::bigint FROM crm.audit_heads"),
        "crm.audit_records" => sqlx::query_scalar("SELECT count(*)::bigint FROM crm.audit_records"),
        _ => panic!("unsupported write-surface table"),
    }
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn prove_records_primary_key_scan(admin: &PgPool, tenant: &str) {
    let mut transaction = admin.begin().await.unwrap();
    sqlx::query("SET LOCAL enable_seqscan = off")
        .execute(&mut *transaction)
        .await
        .unwrap();
    let plan = sqlx::query_scalar::<_, String>(
        r#"
        EXPLAIN (COSTS OFF)
        SELECT record_id, version
          FROM crm.records
         WHERE tenant_id = $1
           AND owner_module_id = 'crm.data-quality'
           AND record_type = 'data_quality.party_evaluation_job'
           AND record_id > ''
           AND deleted_at IS NULL
         ORDER BY record_id ASC
         LIMIT 512
        "#,
    )
    .bind(tenant)
    .fetch_all(&mut *transaction)
    .await
    .unwrap()
    .join("\n");
    assert!(plan.contains("records_pkey"), "unexpected plan: {plan}");
    assert!(!plan.contains("Seq Scan"), "unbounded scan plan: {plan}");
    transaction.rollback().await.unwrap();
}

async fn set_record_data_class(admin: &PgPool, tenant: &str, record_id: &str, value: &str) {
    let mut transaction = admin.begin().await.unwrap();
    sqlx::query("SET LOCAL session_replication_role = 'replica'")
        .execute(&mut *transaction)
        .await
        .unwrap();
    sqlx::query("UPDATE crm.records SET data_class = $3 WHERE tenant_id = $1 AND record_id = $2")
        .bind(tenant)
        .bind(record_id)
        .bind(value)
        .execute(&mut *transaction)
        .await
        .unwrap();
    transaction.commit().await.unwrap();
}

async fn delete_record(admin: &PgPool, tenant: &str, record_type: &str, record_id: &str) {
    let mut transaction = admin.begin().await.unwrap();
    sqlx::query("SET LOCAL session_replication_role = 'replica'")
        .execute(&mut *transaction)
        .await
        .unwrap();
    sqlx::query(
        "DELETE FROM crm.records WHERE tenant_id = $1 AND record_type = $2 AND record_id = $3",
    )
    .bind(tenant)
    .bind(record_type)
    .bind(record_id)
    .execute(&mut *transaction)
    .await
    .unwrap();
    transaction.commit().await.unwrap();
}

fn decode(bytes: &[u8]) -> privacy::DataQualityPrivacyScopeContributionResponse {
    privacy::DataQualityPrivacyScopeContributionResponse::decode(bytes)
        .expect("decode Data Quality privacy scope response")
}

fn expected_two_graphs(left: &GraphIds, right: &GraphIds) -> BTreeMap<String, Vec<String>> {
    let mut map = BTreeMap::new();
    for (record_type, left_id, right_id) in [
        ("data_quality.party_evaluation_job", &left.job, &right.job),
        (
            "data_quality.party_evaluation_input",
            &left.input,
            &right.input,
        ),
        ("data_quality.rule_outcome", &left.outcome, &right.outcome),
        ("data_quality.finding", &left.finding, &right.finding),
        (
            "data_quality.finding_observation",
            &left.observation,
            &right.observation,
        ),
        (
            "data_quality.party_completeness_result",
            &left.completeness,
            &right.completeness,
        ),
        (
            "data_quality.remediation_attempt",
            &left.remediation,
            &right.remediation,
        ),
    ] {
        let mut ids = vec![left_id.clone(), right_id.clone()];
        ids.sort();
        map.insert(record_type.to_owned(), ids);
    }
    map
}

fn graph_values(graph: &GraphIds) -> [&str; 7] {
    [
        &graph.job,
        &graph.input,
        &graph.outcome,
        &graph.finding,
        &graph.observation,
        &graph.completeness,
        &graph.remediation,
    ]
}

fn assert_response_omits_private_data_quality_values(bytes: &[u8]) {
    for forbidden in [
        "party-alias",
        "party-unrelated",
        "Private Bad Name Alpha",
        "Private Bad Name Beta",
        "Private Bad Name Gamma",
        "Private Bad Name Delta",
        "Private Corrected Name Alpha",
        "Private Corrected Name Beta",
        "Private Corrected Name Gamma",
        "Private Corrected Name Delta",
        "Private Data Quality rule title",
        "Private Data Quality remediation guidance",
        "private-caller-dq-job-001",
        "DATA_QUALITY_PARTY_DISPLAY_NAME_PLACEHOLDER",
    ] {
        assert!(
            !contains(bytes, forbidden),
            "response leaked forbidden Data Quality value: {forbidden}"
        );
    }
}

fn contains(bytes: &[u8], value: &str) -> bool {
    bytes
        .windows(value.len())
        .any(|candidate| candidate == value.as_bytes())
}
