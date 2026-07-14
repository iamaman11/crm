#![cfg(unix)]

use crm_application_runtime::{
    application_mutation_definitions, application_query_definitions,
    gateway_v1::{
        ApprovalEvidence as GatewayApprovalEvidence, MutateRequest as GatewayMutateRequest,
        QueryRequest as GatewayQueryRequest, TypedPayload as GatewayTypedPayload,
        application_gateway_service_client::ApplicationGatewayServiceClient,
    },
};
use crm_capability_adapters::HmacSha256ApprovalVerifier;
use crm_capability_ingress::semantic_input_hash;
use crm_capability_runtime::{ApprovalEvidence, CapabilityDefinition};
use crm_module_sdk::{ActorId, DataClass, PayloadEncoding, RetentionPolicyId, TypedPayload};
use crm_proto_contracts::crm::{
    customer::v1 as customer, identity_resolution::v1 as identity, parties::v1 as parties,
};
use prost::Message;
use sqlx::{Executor, PgPool};
use std::net::TcpListener;
use std::process::Stdio;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::process::{Child, Command};
use tokio::time::{sleep, timeout};
use tonic::{Code, Request, Status};

const TENANT_A: &str = "tenant-a";
const TENANT_B: &str = "tenant-b";
const ACTOR: &str = "actor-a";
const TOKEN: &str = "identity-resolution-process-bearer-token-0123456789abcdef";
const APPROVAL_KEY: &str = "identity-resolution-approval-signing-key-0123456789abcdef";

const PARTY_CREATE: &str = "parties.party.create";
const PARTY_GET: &str = "parties.party.get";
const MERGE_EXECUTE: &str = "identity_resolution.merge.execute";
const MERGE_UNMERGE: &str = "identity_resolution.merge.unmerge";
const MERGE_GET: &str = "identity_resolution.merge.get";
const MERGE_LIST: &str = "identity_resolution.merge.list_by_party";
const RESOLVE_CANONICAL: &str = "identity_resolution.party.resolve_canonical";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct EvidenceCounts {
    operations: i64,
    lineage_relationships: i64,
    redirects: i64,
    events: i64,
    audits: i64,
    idempotency: i64,
    transactions: i64,
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn crm_api_process_proves_approved_reversible_merge_lineage() {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!("skipping merge/unmerge process acceptance because DATABASE_URL is absent");
        return;
    };
    let admin_database_url = std::env::var("ADMIN_DATABASE_URL")
        .expect("ADMIN_DATABASE_URL must accompany DATABASE_URL");
    let admin = PgPool::connect(&admin_database_url)
        .await
        .expect("connect merge/unmerge process evidence reader");
    ensure_canonical_redirect_migration(&admin).await;
    for fixture in [
        include_str!("../../../database/tests/0005_party_adapter.sql"),
        include_str!("../../../database/tests/0010_identity_resolution_adapter.sql"),
    ] {
        admin
            .execute(sqlx::raw_sql(fixture))
            .await
            .expect("publish merge/unmerge production adapter registry fixture");
    }

    let http_port = free_port();
    let mut grpc_port = free_port();
    while grpc_port == http_port {
        grpc_port = free_port();
    }
    let http_addr = format!("127.0.0.1:{http_port}");
    let grpc_addr = format!("127.0.0.1:{grpc_port}");
    let mut child = spawn_crm_api(&database_url, &http_addr, &grpc_addr);
    let http = reqwest::Client::new();
    wait_until_ready(&http, &mut child, &http_addr).await;
    let mut grpc = connect_grpc(&grpc_addr).await;

    let party_create = mutation_definition(PARTY_CREATE);
    let party_get = query_definition(PARTY_GET);
    let merge = mutation_definition(MERGE_EXECUTE);
    let unmerge = mutation_definition(MERGE_UNMERGE);
    let merge_get = query_definition(MERGE_GET);
    let merge_list = query_definition(MERGE_LIST);
    let resolve = query_definition(RESOLVE_CANONICAL);

    let party_a = unique_id("merge-party-a");
    let party_b = unique_id("merge-party-b");
    let party_c = unique_id("merge-party-c");
    let cross_tenant_party = unique_id("merge-party-cross-tenant");
    create_party(
        &mut grpc,
        &party_create,
        TENANT_A,
        &party_a,
        "Merge Subject A",
        "merge-create-a",
    )
    .await;
    create_party(
        &mut grpc,
        &party_create,
        TENANT_A,
        &party_b,
        "Merge Subject B",
        "merge-create-b",
    )
    .await;
    create_party(
        &mut grpc,
        &party_create,
        TENANT_A,
        &party_c,
        "Merge Subject C",
        "merge-create-c",
    )
    .await;
    create_party(
        &mut grpc,
        &party_create,
        TENANT_B,
        &cross_tenant_party,
        "Merge Cross Tenant Subject",
        "merge-create-cross-tenant",
    )
    .await;

    let baseline = evidence_counts(&admin, TENANT_A).await;
    let operation_id = unique_id("merge-operation-c-into-b");
    let exact_merge = merge_payload(&merge, &operation_id, &party_c, 1, &party_b, 1);

    let missing_approval = mutate(
        &mut grpc,
        &merge,
        exact_merge.clone(),
        TENANT_A,
        "merge-missing-approval",
        None,
    )
    .await
    .expect_err("merge must reject missing approval");
    assert_eq!(missing_approval.code(), Code::PermissionDenied);

    let mut invalid_approval = signed_gateway_approval(&merge, &exact_merge);
    invalid_approval.opaque_proof[0] ^= 0xff;
    let invalid_signature = mutate(
        &mut grpc,
        &merge,
        exact_merge.clone(),
        TENANT_A,
        "merge-invalid-approval",
        Some(invalid_approval),
    )
    .await
    .expect_err("merge must reject tampered approval proof");
    assert_eq!(invalid_signature.code(), Code::PermissionDenied);

    let missing_party = unique_id("merge-missing-party");
    let missing_payload = merge_payload(
        &merge,
        &unique_id("merge-missing-operation"),
        &missing_party,
        1,
        &party_b,
        1,
    );
    let missing_reference = mutate(
        &mut grpc,
        &merge,
        missing_payload.clone(),
        TENANT_A,
        "merge-missing-reference",
        Some(signed_gateway_approval(&merge, &missing_payload)),
    )
    .await
    .expect_err("merge must reject missing Party reference");
    assert_eq!(missing_reference.code(), Code::InvalidArgument);

    let cross_payload = merge_payload(
        &merge,
        &unique_id("merge-cross-operation"),
        &cross_tenant_party,
        1,
        &party_b,
        1,
    );
    let cross_reference = mutate(
        &mut grpc,
        &merge,
        cross_payload.clone(),
        TENANT_A,
        "merge-cross-reference",
        Some(signed_gateway_approval(&merge, &cross_payload)),
    )
    .await
    .expect_err("merge must reject cross-tenant Party without disclosure");
    assert_eq!(cross_reference.code(), Code::InvalidArgument);
    assert_eq!(cross_reference.message(), missing_reference.message());

    let stale_payload = merge_payload(
        &merge,
        &unique_id("merge-stale-operation"),
        &party_c,
        2,
        &party_b,
        1,
    );
    let stale = mutate(
        &mut grpc,
        &merge,
        stale_payload.clone(),
        TENANT_A,
        "merge-stale-version",
        Some(signed_gateway_approval(&merge, &stale_payload)),
    )
    .await
    .expect_err("merge must reject stale Party version");
    assert_eq!(stale.code(), Code::Aborted);
    assert_eq!(evidence_counts(&admin, TENANT_A).await, baseline);

    let merged = mutate(
        &mut grpc,
        &merge,
        exact_merge.clone(),
        TENANT_A,
        "merge-c-into-b",
        Some(signed_gateway_approval(&merge, &exact_merge)),
    )
    .await
    .expect("approved exact-version merge must succeed");
    assert!(!merged.replayed);
    let merged_operation = decode_merge(&merged);
    assert_eq!(operation_id_of(&merged_operation), operation_id);
    assert_eq!(operation_version(&merged_operation), 1);
    assert_eq!(
        merged_operation.status,
        identity::MergeOperationStatus::Active as i32
    );
    assert_eq!(merged_operation.decided_by_actor_id, ACTOR);
    assert_eq!(merged_operation.survivorship.len(), 1);
    assert_evidence_delta(
        evidence_counts(&admin, TENANT_A).await,
        baseline,
        1,
        2,
        1,
        1,
    );

    let queried = decode_get_merge(
        query(
            &mut grpc,
            &merge_get,
            get_merge_payload(&merge_get, &operation_id),
            TENANT_A,
        )
        .await
        .expect("query active merge operation"),
    );
    assert_eq!(operation_id_of(&queried), operation_id);
    assert_eq!(
        queried.status,
        identity::MergeOperationStatus::Active as i32
    );

    for party_id in [&party_c, &party_b] {
        let listed = decode_list_merges(
            query(
                &mut grpc,
                &merge_list,
                list_merges_payload(
                    &merge_list,
                    party_id,
                    identity::MergeOperationStatus::Active,
                ),
                TENANT_A,
            )
            .await
            .expect("list active merge lineage by Party"),
        );
        assert_eq!(listed.merge_operations.len(), 1);
        assert_eq!(operation_id_of(&listed.merge_operations[0]), operation_id);
    }

    let resolved = decode_resolution(
        query(
            &mut grpc,
            &resolve,
            resolve_payload(&resolve, &party_c),
            TENANT_A,
        )
        .await
        .expect("resolve source Party to survivor"),
    );
    assert_eq!(
        resolved.canonical_party_ref.as_ref().unwrap().party_id,
        party_b
    );
    assert_eq!(
        resolved
            .party_path
            .iter()
            .map(|party| party.party_id.as_str())
            .collect::<Vec<_>>(),
        vec![party_c.as_str(), party_b.as_str()]
    );
    assert_eq!(resolved.merge_operation_path.len(), 1);
    assert_eq!(
        resolved.merge_operation_path[0].merge_operation_id,
        operation_id
    );

    let duplicate_source_payload = merge_payload(
        &merge,
        &unique_id("merge-duplicate-source-operation"),
        &party_c,
        1,
        &party_a,
        1,
    );
    let duplicate_source = mutate(
        &mut grpc,
        &merge,
        duplicate_source_payload.clone(),
        TENANT_A,
        "merge-duplicate-source",
        Some(signed_gateway_approval(&merge, &duplicate_source_payload)),
    )
    .await
    .expect_err("active redirect source cannot be merged again");
    assert!(matches!(
        duplicate_source.code(),
        Code::Aborted | Code::FailedPrecondition
    ));

    let cycle_payload = merge_payload(
        &merge,
        &unique_id("merge-cycle-operation"),
        &party_b,
        1,
        &party_c,
        1,
    );
    let cycle = mutate(
        &mut grpc,
        &merge,
        cycle_payload.clone(),
        TENANT_A,
        "merge-cycle",
        Some(signed_gateway_approval(&merge, &cycle_payload)),
    )
    .await
    .expect_err("reverse canonical cycle must be rejected");
    assert!(matches!(
        cycle.code(),
        Code::Aborted | Code::FailedPrecondition
    ));
    assert_evidence_delta(
        evidence_counts(&admin, TENANT_A).await,
        baseline,
        1,
        2,
        1,
        1,
    );

    let stale_unmerge = unmerge_payload(&unmerge, &operation_id, 1, 2, 1);
    let stale_unmerge_error = mutate(
        &mut grpc,
        &unmerge,
        stale_unmerge.clone(),
        TENANT_A,
        "unmerge-stale-version",
        Some(signed_gateway_approval(&unmerge, &stale_unmerge)),
    )
    .await
    .expect_err("unmerge must reject stale source Party version");
    assert_eq!(stale_unmerge_error.code(), Code::Aborted);

    let exact_unmerge = unmerge_payload(&unmerge, &operation_id, 1, 1, 1);
    let unmerged = mutate(
        &mut grpc,
        &unmerge,
        exact_unmerge.clone(),
        TENANT_A,
        "unmerge-c-from-b",
        Some(signed_gateway_approval(&unmerge, &exact_unmerge)),
    )
    .await
    .expect("approved exact-version unmerge must succeed");
    let unmerged_operation = decode_unmerge(&unmerged);
    assert_eq!(operation_id_of(&unmerged_operation), operation_id);
    assert_eq!(operation_version(&unmerged_operation), 2);
    assert_eq!(
        unmerged_operation.status,
        identity::MergeOperationStatus::Unmerged as i32
    );
    assert!(unmerged_operation.unmerge_decision.is_some());
    assert_evidence_delta(
        evidence_counts(&admin, TENANT_A).await,
        baseline,
        1,
        2,
        0,
        2,
    );

    let resolved_after = decode_resolution(
        query(
            &mut grpc,
            &resolve,
            resolve_payload(&resolve, &party_c),
            TENANT_A,
        )
        .await
        .expect("resolve source Party after unmerge"),
    );
    assert_eq!(
        resolved_after
            .canonical_party_ref
            .as_ref()
            .unwrap()
            .party_id,
        party_c
    );
    assert_eq!(resolved_after.party_path.len(), 1);
    assert!(resolved_after.merge_operation_path.is_empty());

    let queried_unmerged = decode_get_merge(
        query(
            &mut grpc,
            &merge_get,
            get_merge_payload(&merge_get, &operation_id),
            TENANT_A,
        )
        .await
        .expect("query retained unmerged lineage"),
    );
    assert_eq!(
        queried_unmerged.status,
        identity::MergeOperationStatus::Unmerged as i32
    );

    let listed_unmerged = decode_list_merges(
        query(
            &mut grpc,
            &merge_list,
            list_merges_payload(
                &merge_list,
                &party_c,
                identity::MergeOperationStatus::Unmerged,
            ),
            TENANT_A,
        )
        .await
        .expect("list retained unmerged lineage"),
    );
    assert_eq!(listed_unmerged.merge_operations.len(), 1);
    assert_eq!(
        operation_id_of(&listed_unmerged.merge_operations[0]),
        operation_id
    );

    let party_b_after = decode_party(
        query(
            &mut grpc,
            &party_get,
            party_get_payload(&party_get, &party_b),
            TENANT_A,
        )
        .await
        .expect("survivor Party remains queryable"),
    );
    let party_c_after = decode_party(
        query(
            &mut grpc,
            &party_get,
            party_get_payload(&party_get, &party_c),
            TENANT_A,
        )
        .await
        .expect("source Party remains queryable"),
    );
    assert_eq!(party_version(&party_b_after), 1);
    assert_eq!(party_version(&party_c_after), 1);
    assert_eq!(party_record_count(&admin, TENANT_A).await, 3);

    send_sigint(&child).await;
    let exit = timeout(Duration::from_secs(15), child.wait())
        .await
        .expect("crm-api must stop within graceful-shutdown budget")
        .expect("wait for merge/unmerge acceptance crm-api process");
    assert!(exit.success(), "crm-api exited unsuccessfully: {exit}");
}

async fn ensure_canonical_redirect_migration(admin: &PgPool) {
    let installed = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS (SELECT 1 FROM pg_indexes WHERE schemaname = 'crm' AND indexname = 'identity_resolution_canonical_redirect_source_uq')",
    )
    .fetch_one(admin)
    .await
    .expect("inspect canonical redirect migration state");
    if !installed {
        admin
            .execute(sqlx::raw_sql(include_str!(
                "../../../database/migrations/0011_identity_resolution_canonical_redirect.up.sql"
            )))
            .await
            .expect("apply canonical redirect hard-invariant migration");
    }
}

async fn create_party(
    client: &mut ApplicationGatewayServiceClient<tonic::transport::Channel>,
    definition: &CapabilityDefinition,
    tenant_id: &str,
    party_id: &str,
    display_name: &str,
    idempotency_key: &str,
) {
    mutate(
        client,
        definition,
        payload(
            definition,
            parties::CreatePartyRequest {
                party_ref: Some(customer::PartyRef {
                    party_id: party_id.to_owned(),
                }),
                kind: parties::PartyKind::Person as i32,
                display_name: display_name.to_owned(),
            },
        ),
        tenant_id,
        idempotency_key,
        None,
    )
    .await
    .expect("create Party prerequisite through production gateway");
}

fn merge_payload(
    definition: &CapabilityDefinition,
    operation_id: &str,
    source_party: &str,
    source_version: i64,
    survivor_party: &str,
    survivor_version: i64,
) -> TypedPayload {
    payload(
        definition,
        identity::MergePartyRequest {
            merge_operation_ref: Some(identity::MergeOperationRef {
                merge_operation_id: operation_id.to_owned(),
            }),
            source_party_ref: Some(customer::PartyRef {
                party_id: source_party.to_owned(),
            }),
            source_party_version: source_version,
            survivor_party_ref: Some(customer::PartyRef {
                party_id: survivor_party.to_owned(),
            }),
            survivor_party_version: survivor_version,
            decision_ref: format!("review://identity-resolution/{operation_id}/merge"),
            reason: "review.confirmed_duplicate".to_owned(),
            survivorship: vec![identity::SurvivorshipSelection {
                field_path: "display_name".to_owned(),
                provenance_party_ref: Some(customer::PartyRef {
                    party_id: survivor_party.to_owned(),
                }),
                provenance_party_version: survivor_version,
                source_value_sha256: vec![0x42; 32],
                evidence_ref: format!("evidence://identity-resolution/{operation_id}/display-name"),
            }],
        },
    )
}

fn unmerge_payload(
    definition: &CapabilityDefinition,
    operation_id: &str,
    expected_version: i64,
    expected_source_party_version: i64,
    expected_survivor_party_version: i64,
) -> TypedPayload {
    payload(
        definition,
        identity::UnmergePartyRequest {
            merge_operation_ref: Some(identity::MergeOperationRef {
                merge_operation_id: operation_id.to_owned(),
            }),
            expected_version,
            decision_ref: format!("review://identity-resolution/{operation_id}/unmerge"),
            reason: "review.manual_correction".to_owned(),
            expected_source_party_version,
            expected_survivor_party_version,
        },
    )
}

fn get_merge_payload(definition: &CapabilityDefinition, operation_id: &str) -> TypedPayload {
    payload(
        definition,
        identity::GetMergeOperationRequest {
            merge_operation_ref: Some(identity::MergeOperationRef {
                merge_operation_id: operation_id.to_owned(),
            }),
        },
    )
}

fn list_merges_payload(
    definition: &CapabilityDefinition,
    party_id: &str,
    status: identity::MergeOperationStatus,
) -> TypedPayload {
    payload(
        definition,
        identity::ListMergeOperationsByPartyRequest {
            party_ref: Some(customer::PartyRef {
                party_id: party_id.to_owned(),
            }),
            status: status as i32,
            page_size: 10,
            cursor: String::new(),
        },
    )
}

fn resolve_payload(definition: &CapabilityDefinition, party_id: &str) -> TypedPayload {
    payload(
        definition,
        identity::ResolveCanonicalPartyRequest {
            party_ref: Some(customer::PartyRef {
                party_id: party_id.to_owned(),
            }),
        },
    )
}

fn party_get_payload(definition: &CapabilityDefinition, party_id: &str) -> TypedPayload {
    payload(
        definition,
        parties::GetPartyRequest {
            party_ref: Some(customer::PartyRef {
                party_id: party_id.to_owned(),
            }),
        },
    )
}

fn signed_gateway_approval(
    definition: &CapabilityDefinition,
    input: &TypedPayload,
) -> GatewayApprovalEvidence {
    let verifier = HmacSha256ApprovalVerifier::try_new(APPROVAL_KEY.as_bytes().to_vec())
        .expect("valid acceptance approval signing key");
    let mut approval = ApprovalEvidence {
        approval_id: unique_id("merge-approval"),
        actor_id: ActorId::try_new(ACTOR).expect("valid acceptance actor"),
        capability_id: definition.capability_id.clone(),
        capability_version: definition.capability_version.clone(),
        input_hash: semantic_input_hash(input),
        policy_version: "identity-resolution-merge-approval/v1".to_owned(),
        expires_at_unix_nanos: now_nanos() + 300_000_000_000,
        opaque_proof: Vec::new(),
    };
    approval.opaque_proof = verifier.sign(&approval);
    GatewayApprovalEvidence {
        approval_id: approval.approval_id,
        actor_id: approval.actor_id.as_str().to_owned(),
        capability_id: approval.capability_id.as_str().to_owned(),
        capability_version: approval.capability_version.as_str().to_owned(),
        input_hash: approval.input_hash.to_vec(),
        policy_version: approval.policy_version,
        expires_at_unix_nanos: approval.expires_at_unix_nanos,
        opaque_proof: approval.opaque_proof,
    }
}

async fn mutate(
    client: &mut ApplicationGatewayServiceClient<tonic::transport::Channel>,
    definition: &CapabilityDefinition,
    input: TypedPayload,
    tenant_id: &str,
    idempotency_key: &str,
    approval: Option<GatewayApprovalEvidence>,
) -> Result<crm_application_runtime::gateway_v1::MutateResponse, Status> {
    let mut request = Request::new(GatewayMutateRequest {
        owner_module_id: definition.owner_module_id.as_str().to_owned(),
        capability_id: definition.capability_id.as_str().to_owned(),
        capability_version: definition.capability_version.as_str().to_owned(),
        input: Some(wire_payload(input)),
        approval,
    });
    request
        .metadata_mut()
        .insert("x-tenant-id", tenant_id.parse().unwrap());
    request
        .metadata_mut()
        .insert("idempotency-key", idempotency_key.parse().unwrap());
    request
        .metadata_mut()
        .insert("authorization", format!("Bearer {TOKEN}").parse().unwrap());
    client
        .mutate(request)
        .await
        .map(|response| response.into_inner())
}

async fn query(
    client: &mut ApplicationGatewayServiceClient<tonic::transport::Channel>,
    definition: &CapabilityDefinition,
    input: TypedPayload,
    tenant_id: &str,
) -> Result<crm_application_runtime::gateway_v1::QueryResponse, Status> {
    let mut request = Request::new(GatewayQueryRequest {
        owner_module_id: definition.owner_module_id.as_str().to_owned(),
        capability_id: definition.capability_id.as_str().to_owned(),
        capability_version: definition.capability_version.as_str().to_owned(),
        input: Some(wire_payload(input)),
    });
    request
        .metadata_mut()
        .insert("x-tenant-id", tenant_id.parse().unwrap());
    request
        .metadata_mut()
        .insert("authorization", format!("Bearer {TOKEN}").parse().unwrap());
    client
        .query(request)
        .await
        .map(|response| response.into_inner())
}

fn mutation_definition(capability_id: &str) -> CapabilityDefinition {
    application_mutation_definitions()
        .expect("valid application mutation definitions")
        .into_iter()
        .find(|definition| definition.capability_id.as_str() == capability_id)
        .unwrap_or_else(|| panic!("missing application mutation definition: {capability_id}"))
}

fn query_definition(capability_id: &str) -> CapabilityDefinition {
    application_query_definitions()
        .expect("valid application query definitions")
        .into_iter()
        .find(|definition| definition.capability_id.as_str() == capability_id)
        .unwrap_or_else(|| panic!("missing application query definition: {capability_id}"))
}

fn payload<M: Message>(definition: &CapabilityDefinition, message: M) -> TypedPayload {
    let data_class = *definition
        .input_contract
        .allowed_data_classes
        .first()
        .unwrap();
    let payload = TypedPayload {
        owner: definition.input_contract.owner.clone(),
        schema_id: definition.input_contract.schema_id.clone(),
        schema_version: definition.input_contract.schema_version.clone(),
        descriptor_hash: definition.input_contract.descriptor_hash,
        data_class,
        encoding: PayloadEncoding::Protobuf,
        maximum_size_bytes: definition.input_contract.maximum_size_bytes,
        retention_policy_id: RetentionPolicyId::try_new("standard").unwrap(),
        bytes: message.encode_to_vec(),
    };
    payload.validate().expect("valid governed input payload");
    payload
}

fn wire_payload(payload: TypedPayload) -> GatewayTypedPayload {
    GatewayTypedPayload {
        owner_module_id: payload.owner.as_str().to_owned(),
        schema_id: payload.schema_id.as_str().to_owned(),
        schema_version: payload.schema_version.as_str().to_owned(),
        descriptor_hash: payload.descriptor_hash.to_vec(),
        data_class: data_class_name(payload.data_class).to_owned(),
        encoding: "protobuf".to_owned(),
        maximum_size_bytes: payload.maximum_size_bytes,
        retention_policy_id: payload.retention_policy_id.as_str().to_owned(),
        payload: payload.bytes,
    }
}

fn data_class_name(data_class: DataClass) -> &'static str {
    match data_class {
        DataClass::Public => "public",
        DataClass::Internal => "internal",
        DataClass::Confidential => "confidential",
        DataClass::Restricted => "restricted",
        DataClass::Personal => "personal",
        DataClass::SensitivePersonal => "sensitive_personal",
        DataClass::Biometric => "biometric",
        DataClass::Financial => "financial",
        DataClass::Credential => "credential",
    }
}

fn decode_merge(
    response: &crm_application_runtime::gateway_v1::MutateResponse,
) -> identity::MergeOperation {
    identity::MergePartyResponse::decode(response.output.as_ref().unwrap().payload.as_slice())
        .unwrap()
        .merge_operation
        .unwrap()
}

fn decode_unmerge(
    response: &crm_application_runtime::gateway_v1::MutateResponse,
) -> identity::MergeOperation {
    identity::UnmergePartyResponse::decode(response.output.as_ref().unwrap().payload.as_slice())
        .unwrap()
        .merge_operation
        .unwrap()
}

fn decode_get_merge(
    response: crm_application_runtime::gateway_v1::QueryResponse,
) -> identity::MergeOperation {
    identity::GetMergeOperationResponse::decode(response.output.unwrap().payload.as_slice())
        .unwrap()
        .merge_operation
        .unwrap()
}

fn decode_list_merges(
    response: crm_application_runtime::gateway_v1::QueryResponse,
) -> identity::ListMergeOperationsByPartyResponse {
    identity::ListMergeOperationsByPartyResponse::decode(
        response.output.unwrap().payload.as_slice(),
    )
    .unwrap()
}

fn decode_resolution(
    response: crm_application_runtime::gateway_v1::QueryResponse,
) -> identity::CanonicalPartyResolution {
    identity::ResolveCanonicalPartyResponse::decode(response.output.unwrap().payload.as_slice())
        .unwrap()
        .resolution
        .unwrap()
}

fn decode_party(response: crm_application_runtime::gateway_v1::QueryResponse) -> parties::Party {
    parties::GetPartyResponse::decode(response.output.unwrap().payload.as_slice())
        .unwrap()
        .party
        .unwrap()
}

fn operation_id_of(operation: &identity::MergeOperation) -> &str {
    operation
        .merge_operation_ref
        .as_ref()
        .unwrap()
        .merge_operation_id
        .as_str()
}

fn operation_version(operation: &identity::MergeOperation) -> i64 {
    operation.resource_version.as_ref().unwrap().version
}

fn party_version(party: &parties::Party) -> i64 {
    party.resource_version.as_ref().unwrap().version
}

async fn evidence_counts(admin: &PgPool, tenant_id: &str) -> EvidenceCounts {
    let operations = sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM crm.records WHERE tenant_id = $1 AND owner_module_id = 'crm.identity-resolution' AND record_type = 'identity_resolution.merge_operation' AND deleted_at IS NULL",
    )
    .bind(tenant_id)
    .fetch_one(admin)
    .await
    .unwrap();
    let lineage_relationships = sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM crm.relationships WHERE tenant_id = $1 AND owner_module_id = 'crm.identity-resolution' AND relationship_type = 'identity_resolution.merge.party'",
    )
    .bind(tenant_id)
    .fetch_one(admin)
    .await
    .unwrap();
    let redirects = sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM crm.relationships WHERE tenant_id = $1 AND owner_module_id = 'crm.identity-resolution' AND relationship_type = 'identity_resolution.canonical_redirect'",
    )
    .bind(tenant_id)
    .fetch_one(admin)
    .await
    .unwrap();
    let events = sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM crm.outbox_events WHERE tenant_id = $1 AND event_type IN ('identity_resolution.party.merged', 'identity_resolution.party.unmerged')",
    )
    .bind(tenant_id)
    .fetch_one(admin)
    .await
    .unwrap();
    let audits =
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM crm.audit_records WHERE tenant_id = $1")
            .bind(tenant_id)
            .fetch_one(admin)
            .await
            .unwrap();
    let idempotency = sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM crm.idempotency_records WHERE tenant_id = $1",
    )
    .bind(tenant_id)
    .fetch_one(admin)
    .await
    .unwrap();
    let transactions = sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM crm.business_transactions WHERE tenant_id = $1",
    )
    .bind(tenant_id)
    .fetch_one(admin)
    .await
    .unwrap();
    EvidenceCounts {
        operations,
        lineage_relationships,
        redirects,
        events,
        audits,
        idempotency,
        transactions,
    }
}

fn assert_evidence_delta(
    actual: EvidenceCounts,
    baseline: EvidenceCounts,
    operations: i64,
    lineage_relationships: i64,
    redirects: i64,
    successful_mutations: i64,
) {
    assert_eq!(actual.operations, baseline.operations + operations);
    assert_eq!(
        actual.lineage_relationships,
        baseline.lineage_relationships + lineage_relationships
    );
    assert_eq!(actual.redirects, baseline.redirects + redirects);
    assert_eq!(actual.events, baseline.events + successful_mutations);
    assert_eq!(actual.audits, baseline.audits + successful_mutations);
    assert_eq!(
        actual.idempotency,
        baseline.idempotency + successful_mutations
    );
    assert_eq!(
        actual.transactions,
        baseline.transactions + successful_mutations
    );
}

async fn party_record_count(admin: &PgPool, tenant_id: &str) -> i64 {
    sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM crm.records WHERE tenant_id = $1 AND owner_module_id = 'crm.parties' AND record_type = 'parties.party' AND deleted_at IS NULL",
    )
    .bind(tenant_id)
    .fetch_one(admin)
    .await
    .unwrap()
}

fn spawn_crm_api(database_url: &str, http_addr: &str, grpc_addr: &str) -> Child {
    Command::new(env!("CARGO_BIN_EXE_crm-api"))
        .env("CRM_DATABASE_URL", database_url)
        .env("CRM_HTTP_BIND", http_addr)
        .env("CRM_GRPC_BIND", grpc_addr)
        .env("CRM_API_BEARER_TOKEN", TOKEN)
        .env("CRM_API_ACTOR_ID", ACTOR)
        .env("CRM_API_TENANTS", format!("{TENANT_A},{TENANT_B}"))
        .env(
            "CRM_CURSOR_SIGNING_KEY",
            "identity-resolution-cursor-signing-key-0123456789abcdef",
        )
        .env("CRM_APPROVAL_SIGNING_KEY", APPROVAL_KEY)
        .env("CRM_BOOTSTRAP_ALLOW_PHASE6", "true")
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .kill_on_drop(true)
        .spawn()
        .expect("spawn production crm-api process for merge/unmerge acceptance")
}

async fn wait_until_ready(client: &reqwest::Client, child: &mut Child, http_addr: &str) {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if let Some(status) = child.try_wait().expect("poll crm-api process") {
            panic!("crm-api exited before merge/unmerge acceptance readiness: {status}");
        }
        if let Ok(response) = client
            .get(format!("http://{http_addr}/readyz"))
            .send()
            .await
            && response.status().is_success()
        {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "merge/unmerge acceptance readiness timed out"
        );
        sleep(Duration::from_millis(200)).await;
    }
}

async fn connect_grpc(
    grpc_addr: &str,
) -> ApplicationGatewayServiceClient<tonic::transport::Channel> {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        match ApplicationGatewayServiceClient::connect(format!("http://{grpc_addr}")).await {
            Ok(client) => return client,
            Err(error) => {
                assert!(
                    Instant::now() < deadline,
                    "merge/unmerge gRPC listener timed out: {error}"
                );
                sleep(Duration::from_millis(200)).await;
            }
        }
    }
}

async fn send_sigint(child: &Child) {
    let pid = child.id().expect("running crm-api process has a PID");
    let status = Command::new("kill")
        .arg("-INT")
        .arg(pid.to_string())
        .status()
        .await
        .expect("send SIGINT to merge/unmerge acceptance crm-api");
    assert!(status.success(), "kill -INT failed: {status}");
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("bind ephemeral port")
        .local_addr()
        .expect("read ephemeral port")
        .port()
}

fn unique_id(prefix: &str) -> String {
    format!("{prefix}-{}-{}", std::process::id(), now_nanos())
}

fn now_nanos() -> i64 {
    i64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time after Unix epoch")
            .as_nanos(),
    )
    .expect("current time fits i64 nanoseconds")
}
