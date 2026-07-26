use crm_capability_plan_support as support;
use crm_capability_runtime::{
    CapabilityDefinition, CapabilityRequest, TransactionalCapabilityExecutor,
};
use crm_customer_privacy::{CANONICAL_SCOPE_REGISTRY_VERSION, OwnerScopeRegistry};
use crm_identity_resolution_capability_adapter::{
    CONFIRM_CAPABILITY, CONFIRM_REQUEST_SCHEMA, DISMISS_CAPABILITY, DISMISS_REQUEST_SCHEMA,
    MERGE_CAPABILITY, MERGE_REQUEST_SCHEMA, REFRESH_CAPABILITY, REFRESH_REQUEST_SCHEMA,
    REGISTER_CAPABILITY, REGISTER_REQUEST_SCHEMA, UNMERGE_CAPABILITY, UNMERGE_REQUEST_SCHEMA,
};
use crm_identity_resolution_privacy_scope_adapter::{
    CAPABILITY_ID, CAPABILITY_VERSION, CONTRACT_SCHEMA_VERSION, INPUT_MAXIMUM_BYTES,
    INPUT_RETENTION_POLICY_ID, INPUT_SCHEMA_ID,
};
use crm_module_sdk::{
    ActorId, BusinessTransactionId, CapabilityId, CapabilityVersion, CausationId, CorrelationId,
    DataClass, ExecutionContext, IdempotencyKey, ModuleExecutionContext, ModuleId, PayloadEncoding,
    RequestId, RetentionPolicyId, SchemaId, SchemaVersion, TenantId, TraceId, TypedPayload,
};
use crm_parties_capability_adapter::{
    CREATE_CAPABILITY as CREATE_PARTY_CAPABILITY, CREATE_REQUEST_SCHEMA as CREATE_PARTY_SCHEMA,
};
use crm_proto_contracts::{
    crm::{
        core::v1 as core, customer::v1 as customer, customer_privacy::v1 as privacy,
        identity_resolution::v1 as identity, parties::v1 as parties,
    },
    message_descriptor_hash,
};
use crm_query_runtime::{QueryExecutionContext, QueryRequest};
use prost::Message;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use std::sync::Arc;

pub(crate) const TENANT_A: &str = "tenant-a";
pub(crate) const TENANT_B: &str = "tenant-b";
const ACTOR: &str = "privacy-worker";

pub(crate) async fn create_party(
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
                CREATE_PARTY_SCHEMA,
                tenant,
                &format!("party-{party_id}"),
                100_000_000 + i64::from(seed),
                seed,
                &parties::CreatePartyRequest {
                    party_ref: Some(customer::PartyRef {
                        party_id: party_id.to_owned(),
                    }),
                    kind: parties::PartyKind::Person as i32,
                    display_name: format!("Identity Privacy Subject {party_id}"),
                },
            ),
        )
        .await
        .expect("create authoritative Party fixture");
}

pub(crate) async fn register_candidate(
    executor: &Arc<dyn TransactionalCapabilityExecutor>,
    definition: &CapabilityDefinition,
    tenant: &str,
    first_party_id: &str,
    second_party_id: &str,
    matcher: &str,
    signal: &str,
    evidence_ref: &str,
    seed: u8,
) {
    executor
        .execute(
            definition,
            capability_request(
                crm_identity_resolution::MODULE_ID,
                REGISTER_CAPABILITY,
                REGISTER_REQUEST_SCHEMA,
                tenant,
                &format!("candidate-{seed}"),
                200_000_000 + i64::from(seed),
                seed,
                &identity::RegisterDuplicateCandidateRequest {
                    evidence: Some(identity::MatchEvidenceSnapshot {
                        first_party_ref: Some(customer::PartyRef {
                            party_id: first_party_id.to_owned(),
                        }),
                        first_party_version: 1,
                        second_party_ref: Some(customer::PartyRef {
                            party_id: second_party_id.to_owned(),
                        }),
                        second_party_version: 1,
                        matcher_profile: matcher.to_owned(),
                        score_basis_points: 8_500,
                        signals: vec![identity::MatchSignal {
                            kind: signal.to_owned(),
                            source: "party.normalized".to_owned(),
                            evidence_ref: evidence_ref.to_owned(),
                            contribution_basis_points: 8_500,
                        }],
                        generated_at: Some(core::UnixTime {
                            unix_nanos: 190_000_000 + i64::from(seed),
                        }),
                    }),
                },
            ),
        )
        .await
        .expect("register authoritative duplicate candidate fixture");
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn merge_party(
    executor: &Arc<dyn TransactionalCapabilityExecutor>,
    definition: &CapabilityDefinition,
    tenant: &str,
    operation_id: &str,
    source_party_id: &str,
    survivor_party_id: &str,
    provenance_party_id: &str,
    field_path: &str,
    evidence_ref: &str,
    seed: u8,
) {
    executor
        .execute(
            definition,
            capability_request(
                crm_identity_resolution::MODULE_ID,
                MERGE_CAPABILITY,
                MERGE_REQUEST_SCHEMA,
                tenant,
                &format!("merge-{operation_id}"),
                300_000_000 + i64::from(seed),
                seed,
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
                        field_path: field_path.to_owned(),
                        provenance_party_ref: Some(customer::PartyRef {
                            party_id: provenance_party_id.to_owned(),
                        }),
                        provenance_party_version: 1,
                        source_value_sha256: [seed; 32].to_vec(),
                        evidence_ref: evidence_ref.to_owned(),
                    }],
                },
            ),
        )
        .await
        .expect("create authoritative merge operation fixture");
}

pub(crate) async fn unmerge_party(
    executor: &Arc<dyn TransactionalCapabilityExecutor>,
    definition: &CapabilityDefinition,
    tenant: &str,
    operation_id: &str,
    source_version: i64,
    survivor_version: i64,
    seed: u8,
) {
    executor
        .execute(
            definition,
            capability_request(
                crm_identity_resolution::MODULE_ID,
                UNMERGE_CAPABILITY,
                UNMERGE_REQUEST_SCHEMA,
                tenant,
                &format!("unmerge-{operation_id}"),
                400_000_000 + i64::from(seed),
                seed,
                &identity::UnmergePartyRequest {
                    merge_operation_ref: Some(identity::MergeOperationRef {
                        merge_operation_id: operation_id.to_owned(),
                    }),
                    expected_version: 1,
                    decision_ref: format!("approval://unmerge/{operation_id}"),
                    reason: "duplicate.reversed".to_owned(),
                    expected_source_party_version: source_version,
                    expected_survivor_party_version: survivor_version,
                },
            ),
        )
        .await
        .expect("unmerge authoritative operation fixture");
}

#[allow(dead_code)]
pub(crate) async fn refresh_candidate(
    executor: &Arc<dyn TransactionalCapabilityExecutor>,
    definition: &CapabilityDefinition,
    tenant: &str,
    case_id: &str,
    first_party_id: &str,
    second_party_id: &str,
    seed: u8,
) {
    executor
        .execute(
            definition,
            capability_request(
                crm_identity_resolution::MODULE_ID,
                REFRESH_CAPABILITY,
                REFRESH_REQUEST_SCHEMA,
                tenant,
                &format!("refresh-{case_id}"),
                250_000_000 + i64::from(seed),
                seed,
                &identity::RefreshDuplicateCandidateEvidenceRequest {
                    case_ref: Some(identity::DuplicateCandidateCaseRef {
                        case_id: case_id.to_owned(),
                    }),
                    expected_version: 1,
                    evidence: Some(identity::MatchEvidenceSnapshot {
                        first_party_ref: Some(customer::PartyRef {
                            party_id: first_party_id.to_owned(),
                        }),
                        first_party_version: 2,
                        second_party_ref: Some(customer::PartyRef {
                            party_id: second_party_id.to_owned(),
                        }),
                        second_party_version: 1,
                        matcher_profile: "deterministic.v2".to_owned(),
                        score_basis_points: 8_700,
                        signals: vec![identity::MatchSignal {
                            kind: "name.exact".to_owned(),
                            source: "party.normalized".to_owned(),
                            evidence_ref: "evidence://candidate/refresh".to_owned(),
                            contribution_basis_points: 8_700,
                        }],
                        generated_at: Some(core::UnixTime {
                            unix_nanos: 240_000_000 + i64::from(seed),
                        }),
                    }),
                },
            ),
        )
        .await
        .expect("refresh authoritative duplicate candidate fixture");
}

#[allow(dead_code)]
pub(crate) async fn decide_candidate(
    executor: &Arc<dyn TransactionalCapabilityExecutor>,
    definition: &CapabilityDefinition,
    tenant: &str,
    case_id: &str,
    confirm: bool,
    seed: u8,
) {
    let (capability, schema, payload) = if confirm {
        (
            CONFIRM_CAPABILITY,
            CONFIRM_REQUEST_SCHEMA,
            identity::ConfirmDuplicateCandidateRequest {
                case_ref: Some(identity::DuplicateCandidateCaseRef {
                    case_id: case_id.to_owned(),
                }),
                expected_version: 1,
                reason: "review.confirmed".to_owned(),
            }
            .encode_to_vec(),
        )
    } else {
        (
            DISMISS_CAPABILITY,
            DISMISS_REQUEST_SCHEMA,
            identity::DismissDuplicateCandidateRequest {
                case_ref: Some(identity::DuplicateCandidateCaseRef {
                    case_id: case_id.to_owned(),
                }),
                expected_version: 1,
                reason: "review.dismissed".to_owned(),
            }
            .encode_to_vec(),
        )
    };
    executor
        .execute(
            definition,
            raw_capability_request(
                crm_identity_resolution::MODULE_ID,
                capability,
                schema,
                tenant,
                &format!("decision-{case_id}"),
                260_000_000 + i64::from(seed),
                seed,
                payload,
            ),
        )
        .await
        .expect("decide authoritative duplicate candidate fixture");
}

#[allow(clippy::too_many_arguments)]
fn capability_request<M: Message>(
    module_id: &str,
    capability_id: &str,
    input_schema: &str,
    tenant: &str,
    identity: &str,
    started_at: i64,
    hash: u8,
    command: &M,
) -> CapabilityRequest {
    raw_capability_request(
        module_id,
        capability_id,
        input_schema,
        tenant,
        identity,
        started_at,
        hash,
        command.encode_to_vec(),
    )
}

#[allow(clippy::too_many_arguments)]
fn raw_capability_request(
    module_id: &str,
    capability_id: &str,
    input_schema: &str,
    tenant: &str,
    identity: &str,
    started_at: i64,
    hash: u8,
    bytes: Vec<u8>,
) -> CapabilityRequest {
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

pub(crate) fn scope_request(
    tenant: &str,
    party_id: &str,
    generation: u64,
    page_size: u32,
    cursor: &str,
    identity: &str,
) -> QueryRequest {
    let registry = OwnerScopeRegistry::canonical_v1().unwrap();
    let wire = privacy::IdentityResolutionPrivacyScopeContributionRequest {
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
    let input_hash: [u8; 32] = Sha256::digest(&bytes).into();
    QueryRequest {
        owner_module_id: ModuleId::try_new(crm_identity_resolution::MODULE_ID).unwrap(),
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
            owner: ModuleId::try_new(crm_identity_resolution::MODULE_ID).unwrap(),
            schema_id: SchemaId::try_new(INPUT_SCHEMA_ID).unwrap(),
            schema_version: SchemaVersion::try_new(CONTRACT_SCHEMA_VERSION).unwrap(),
            descriptor_hash: message_descriptor_hash(INPUT_SCHEMA_ID),
            data_class: DataClass::Confidential,
            encoding: PayloadEncoding::Protobuf,
            maximum_size_bytes: INPUT_MAXIMUM_BYTES,
            retention_policy_id: RetentionPolicyId::try_new(INPUT_RETENTION_POLICY_ID).unwrap(),
            bytes,
        },
        input_hash,
    }
}

pub(crate) async fn current_generation(admin: &PgPool, tenant: &str) -> u64 {
    let generation: i64 =
        sqlx::query_scalar("SELECT crm.current_identity_resolution_generation($1)")
            .bind(tenant)
            .fetch_one(admin)
            .await
            .expect("read current Identity Resolution generation");
    u64::try_from(generation).expect("Identity Resolution generation is positive")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct WriteSurfaceCounts {
    records: i64,
    relationships: i64,
    business_transactions: i64,
    idempotency_records: i64,
    outbox_events: i64,
    outbox_delivery: i64,
    audit_heads: i64,
    audit_records: i64,
}

pub(crate) async fn write_surface_counts(pool: &PgPool) -> WriteSurfaceCounts {
    WriteSurfaceCounts {
        records: sqlx::query_scalar("SELECT count(*)::bigint FROM crm.records")
            .fetch_one(pool)
            .await
            .expect("count CRM records"),
        relationships: sqlx::query_scalar("SELECT count(*)::bigint FROM crm.relationships")
            .fetch_one(pool)
            .await
            .expect("count CRM relationships"),
        business_transactions: sqlx::query_scalar(
            "SELECT count(*)::bigint FROM crm.business_transactions",
        )
        .fetch_one(pool)
        .await
        .expect("count CRM business transactions"),
        idempotency_records: sqlx::query_scalar(
            "SELECT count(*)::bigint FROM crm.idempotency_records",
        )
        .fetch_one(pool)
        .await
        .expect("count CRM idempotency records"),
        outbox_events: sqlx::query_scalar("SELECT count(*)::bigint FROM crm.outbox_events")
            .fetch_one(pool)
            .await
            .expect("count CRM outbox events"),
        outbox_delivery: sqlx::query_scalar("SELECT count(*)::bigint FROM crm.outbox_delivery")
            .fetch_one(pool)
            .await
            .expect("count CRM outbox delivery rows"),
        audit_heads: sqlx::query_scalar("SELECT count(*)::bigint FROM crm.audit_heads")
            .fetch_one(pool)
            .await
            .expect("count CRM audit heads"),
        audit_records: sqlx::query_scalar("SELECT count(*)::bigint FROM crm.audit_records")
            .fetch_one(pool)
            .await
            .expect("count CRM audit records"),
    }
}

pub(crate) async fn corrupt_candidate_metadata(admin: &PgPool, candidate_id: &str) {
    sqlx::query(
        r#"
        UPDATE crm.records
        SET schema_id = 'crm.identity_resolution.candidate_case.state.invalid'
        WHERE tenant_id = $1
          AND owner_module_id = $2
          AND record_type = $3
          AND record_id = $4
        "#,
    )
    .bind(TENANT_A)
    .bind(crm_identity_resolution::MODULE_ID)
    .bind(crm_identity_resolution::CANDIDATE_CASE_RECORD_TYPE)
    .bind(candidate_id)
    .execute(admin)
    .await
    .expect("corrupt isolated candidate metadata fixture");
}
