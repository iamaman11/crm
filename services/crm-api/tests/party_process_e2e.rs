#![cfg(unix)]

use crm_application_runtime::{
    application_mutation_definitions, application_query_definitions,
    gateway_v1::{
        MutateRequest as GatewayMutateRequest, QueryRequest as GatewayQueryRequest,
        TypedPayload as GatewayTypedPayload,
        application_gateway_service_client::ApplicationGatewayServiceClient,
    },
};
use crm_capability_runtime::CapabilityDefinition;
use crm_module_sdk::{DataClass, PayloadEncoding, RetentionPolicyId, TypedPayload};
use crm_proto_contracts::crm::{
    core::v1 as core, customer::v1 as customer, parties::v1 as parties, search::v1 as search,
};
use prost::Message;
use sqlx::{Executor, PgPool};
use std::collections::BTreeSet;
use std::net::TcpListener;
use std::process::Stdio;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::process::{Child, Command};
use tokio::time::{sleep, timeout};
use tonic::{Code, Request, Status};

const TENANT_A: &str = "tenant-a";
const TENANT_B: &str = "tenant-b";
const ACTOR: &str = "actor-a";
const TOKEN: &str = "party-process-bearer-token-0123456789abcdef0123456789abcdef";
const PARTY_CREATE: &str = "parties.party.create";
const PARTY_UPDATE: &str = "parties.party.update";
const PARTY_GET: &str = "parties.party.get";
const PARTY_LIST: &str = "parties.party.list";
const GLOBAL_SEARCH: &str = "search.global.query";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct EvidenceCounts {
    records: i64,
    events: i64,
    audits: i64,
    idempotency: i64,
    transactions: i64,
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn crm_api_process_serves_governed_party_lifecycle_listing_search_and_tenant_isolation() {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!("skipping Party process acceptance because DATABASE_URL is absent");
        return;
    };
    let admin_database_url = std::env::var("ADMIN_DATABASE_URL")
        .expect("ADMIN_DATABASE_URL must accompany DATABASE_URL");
    let admin = PgPool::connect(&admin_database_url)
        .await
        .expect("connect Party process evidence reader");
    admin
        .execute(sqlx::raw_sql(include_str!(
            "../../../database/tests/0005_party_adapter.sql"
        )))
        .await
        .expect("publish Party module/capability registry fixture");

    let http_port = free_port();
    let mut grpc_port = free_port();
    while grpc_port == http_port {
        grpc_port = free_port();
    }
    let http_addr = format!("127.0.0.1:{http_port}");
    let grpc_addr = format!("127.0.0.1:{grpc_port}");

    let mut child = Command::new(env!("CARGO_BIN_EXE_crm-api"))
        .env("CRM_DATABASE_URL", &database_url)
        .env("CRM_HTTP_BIND", &http_addr)
        .env("CRM_GRPC_BIND", &grpc_addr)
        .env("CRM_API_BEARER_TOKEN", TOKEN)
        .env("CRM_API_ACTOR_ID", ACTOR)
        .env("CRM_API_TENANTS", format!("{TENANT_A},{TENANT_B}"))
        .env(
            "CRM_CURSOR_SIGNING_KEY",
            "party-process-cursor-signing-key-0123456789abcdef",
        )
        .env(
            "CRM_APPROVAL_SIGNING_KEY",
            "party-process-approval-signing-key-0123456789abcdef",
        )
        .env("CRM_BOOTSTRAP_ALLOW_PHASE6", "true")
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .kill_on_drop(true)
        .spawn()
        .expect("spawn production crm-api process for Party acceptance");

    let http = reqwest::Client::new();
    wait_until_ready(&http, &mut child, &http_addr).await;
    let mut grpc = connect_grpc(&grpc_addr).await;

    let baseline = evidence_counts(&admin, TENANT_A).await;
    let person_id = unique_party_id("person");
    let organization_id = unique_party_id("organization");
    let initial_person_name = unique_display_name("Legacy Party Search Name");
    let updated_person_name = unique_display_name("Current Party Search Name");
    let organization_name = unique_display_name("Organization Search Name");
    let create_definition = mutation_definition(PARTY_CREATE);
    let create_person_payload = payload(
        &create_definition,
        parties::CreatePartyRequest {
            party_ref: Some(customer::PartyRef {
                party_id: person_id.clone(),
            }),
            kind: parties::PartyKind::Person as i32,
            display_name: initial_person_name.clone(),
        },
    );

    let unauthorized = mutate(
        &mut grpc,
        &create_definition,
        create_person_payload.clone(),
        TENANT_A,
        "party-process-create-person",
        false,
    )
    .await
    .expect_err("unauthenticated Party mutation must fail");
    assert_eq!(unauthorized.code(), Code::Unauthenticated);
    assert_eq!(evidence_counts(&admin, TENANT_A).await, baseline);

    let created = mutate(
        &mut grpc,
        &create_definition,
        create_person_payload.clone(),
        TENANT_A,
        "party-process-create-person",
        true,
    )
    .await
    .expect("create Party through production gRPC gateway");
    assert!(!created.replayed);
    assert_eq!(created.affected_resources.len(), 1);
    assert_eq!(created.affected_resources[0].resource_type, "parties.party");
    assert_eq!(created.affected_resources[0].resource_id, person_id);
    assert_eq!(created.affected_resources[0].version, Some(1));
    let created_output = created.output.expect("Party create output");
    let created_party = parties::CreatePartyResponse::decode(created_output.payload.as_slice())
        .expect("decode Party create response")
        .party
        .expect("created Party exists");
    assert_eq!(
        created_party
            .party_ref
            .as_ref()
            .expect("created Party reference")
            .party_id,
        person_id
    );
    assert_eq!(created_party.kind, parties::PartyKind::Person as i32);
    assert_eq!(created_party.display_name, initial_person_name);
    assert_eq!(resource_version(&created_party), 1);

    let after_create = evidence_counts(&admin, TENANT_A).await;
    assert_eq!(after_create.records, baseline.records + 1);
    assert_eq!(after_create.events, baseline.events + 1);
    assert_eq!(after_create.audits, baseline.audits + 1);
    assert_eq!(after_create.idempotency, baseline.idempotency + 1);
    assert_eq!(after_create.transactions, baseline.transactions + 1);

    let replay = mutate(
        &mut grpc,
        &create_definition,
        create_person_payload.clone(),
        TENANT_A,
        "party-process-create-person",
        true,
    )
    .await
    .expect("replay Party create through production gateway");
    assert!(replay.replayed);
    assert_eq!(
        replay.output.expect("replay output").payload,
        created_output.payload
    );
    assert_eq!(evidence_counts(&admin, TENANT_A).await, after_create);

    let conflicting_payload = payload(
        &create_definition,
        parties::CreatePartyRequest {
            party_ref: Some(customer::PartyRef {
                party_id: person_id.clone(),
            }),
            kind: parties::PartyKind::Person as i32,
            display_name: unique_display_name("Conflicting Party Name"),
        },
    );
    let conflict = mutate(
        &mut grpc,
        &create_definition,
        conflicting_payload,
        TENANT_A,
        "party-process-create-person",
        true,
    )
    .await
    .expect_err("conflicting Party idempotency replay must fail");
    assert_eq!(conflict.code(), Code::Aborted);
    assert_eq!(evidence_counts(&admin, TENANT_A).await, after_create);

    let get_definition = query_definition(PARTY_GET);
    let get_person_payload = party_get_payload(&get_definition, &person_id);
    let queried = query(
        &mut grpc,
        &get_definition,
        get_person_payload.clone(),
        TENANT_A,
        true,
    )
    .await
    .expect("query Party through production gRPC gateway");
    let queried_party = decode_get_party(queried);
    assert_eq!(party_id(&queried_party), person_id);
    assert_eq!(queried_party.kind, parties::PartyKind::Person as i32);
    assert_eq!(queried_party.display_name, initial_person_name);
    assert_eq!(resource_version(&queried_party), 1);
    assert_eq!(evidence_counts(&admin, TENANT_A).await, after_create);

    let update_definition = mutation_definition(PARTY_UPDATE);
    let update_payload = payload(
        &update_definition,
        parties::UpdatePartyRequest {
            party_ref: Some(customer::PartyRef {
                party_id: person_id.clone(),
            }),
            expected_version: 1,
            display_name: updated_person_name.clone(),
        },
    );
    let updated = mutate(
        &mut grpc,
        &update_definition,
        update_payload.clone(),
        TENANT_A,
        "party-process-update-person-v1",
        true,
    )
    .await
    .expect("update Party through production gRPC gateway");
    assert!(!updated.replayed);
    assert_eq!(updated.affected_resources.len(), 1);
    assert_eq!(updated.affected_resources[0].resource_id, person_id);
    assert_eq!(updated.affected_resources[0].version, Some(2));
    let updated_output = updated.output.expect("Party update output");
    let updated_party = parties::UpdatePartyResponse::decode(updated_output.payload.as_slice())
        .expect("decode Party update response")
        .party
        .expect("updated Party exists");
    assert_eq!(party_id(&updated_party), person_id);
    assert_eq!(updated_party.kind, parties::PartyKind::Person as i32);
    assert_eq!(updated_party.display_name, updated_person_name);
    assert_eq!(resource_version(&updated_party), 2);

    let after_update = evidence_counts(&admin, TENANT_A).await;
    assert_eq!(after_update.records, after_create.records);
    assert_eq!(after_update.events, after_create.events + 1);
    assert_eq!(after_update.audits, after_create.audits + 1);
    assert_eq!(after_update.idempotency, after_create.idempotency + 1);
    assert_eq!(after_update.transactions, after_create.transactions + 1);

    let update_replay = mutate(
        &mut grpc,
        &update_definition,
        update_payload.clone(),
        TENANT_A,
        "party-process-update-person-v1",
        true,
    )
    .await
    .expect("replay Party update through production gateway");
    assert!(update_replay.replayed);
    assert_eq!(
        update_replay.output.expect("update replay output").payload,
        updated_output.payload
    );
    assert_eq!(evidence_counts(&admin, TENANT_A).await, after_update);

    let stale_update = mutate(
        &mut grpc,
        &update_definition,
        payload(
            &update_definition,
            parties::UpdatePartyRequest {
                party_ref: Some(customer::PartyRef {
                    party_id: person_id.clone(),
                }),
                expected_version: 1,
                display_name: unique_display_name("Stale Party Name"),
            },
        ),
        TENANT_A,
        "party-process-stale-update-person",
        true,
    )
    .await
    .expect_err("stale Party version must fail");
    assert_eq!(stale_update.code(), Code::Aborted);
    assert_eq!(evidence_counts(&admin, TENANT_A).await, after_update);

    let queried_after_update = query(
        &mut grpc,
        &get_definition,
        get_person_payload.clone(),
        TENANT_A,
        true,
    )
    .await
    .expect("query updated Party through production gRPC gateway");
    let queried_after_update = decode_get_party(queried_after_update);
    assert_eq!(queried_after_update.display_name, updated_person_name);
    assert_eq!(resource_version(&queried_after_update), 2);

    let create_organization = mutate(
        &mut grpc,
        &create_definition,
        payload(
            &create_definition,
            parties::CreatePartyRequest {
                party_ref: Some(customer::PartyRef {
                    party_id: organization_id.clone(),
                }),
                kind: parties::PartyKind::Organization as i32,
                display_name: organization_name.clone(),
            },
        ),
        TENANT_A,
        "party-process-create-organization",
        true,
    )
    .await
    .expect("create Organization Party through production gRPC gateway");
    assert!(!create_organization.replayed);
    assert_eq!(create_organization.affected_resources[0].version, Some(1));

    let after_organization = evidence_counts(&admin, TENANT_A).await;
    assert_eq!(after_organization.records, after_update.records + 1);
    assert_eq!(after_organization.events, after_update.events + 1);
    assert_eq!(after_organization.audits, after_update.audits + 1);
    assert_eq!(after_organization.idempotency, after_update.idempotency + 1);
    assert_eq!(
        after_organization.transactions,
        after_update.transactions + 1
    );

    let list_definition = query_definition(PARTY_LIST);
    let first_page = query(
        &mut grpc,
        &list_definition,
        party_list_payload(&list_definition, 1, "", None),
        TENANT_A,
        true,
    )
    .await
    .expect("list first Party page through production gRPC gateway");
    let first_page = decode_list_parties(first_page);
    assert_eq!(first_page.parties.len(), 1);
    let first_page_token = first_page
        .page
        .expect("first Party page info")
        .next_page_token;
    assert!(!first_page_token.is_empty());

    let second_page = query(
        &mut grpc,
        &list_definition,
        party_list_payload(&list_definition, 1, &first_page_token, None),
        TENANT_A,
        true,
    )
    .await
    .expect("list second Party page through production gRPC gateway");
    let second_page = decode_list_parties(second_page);
    assert_eq!(second_page.parties.len(), 1);
    assert!(
        second_page
            .page
            .as_ref()
            .expect("second Party page info")
            .next_page_token
            .is_empty()
    );

    let listed_ids = BTreeSet::from([
        party_id(&first_page.parties[0]).to_owned(),
        party_id(&second_page.parties[0]).to_owned(),
    ]);
    assert_eq!(
        listed_ids,
        BTreeSet::from([person_id.clone(), organization_id.clone()])
    );

    let people = query(
        &mut grpc,
        &list_definition,
        party_list_payload(&list_definition, 10, "", Some(parties::PartyKind::Person)),
        TENANT_A,
        true,
    )
    .await
    .expect("list Person Parties through production gRPC gateway");
    let people = decode_list_parties(people);
    assert_eq!(people.parties.len(), 1);
    assert_eq!(party_id(&people.parties[0]), person_id);
    assert_eq!(people.parties[0].kind, parties::PartyKind::Person as i32);
    assert_eq!(people.parties[0].display_name, updated_person_name);
    assert_eq!(resource_version(&people.parties[0]), 2);

    let search_definition = query_definition(GLOBAL_SEARCH);
    let search_response = wait_for_party_search_hit(
        &mut grpc,
        &search_definition,
        &updated_person_name,
        TENANT_A,
        &person_id,
        2,
    )
    .await;
    let search_hit = search_response
        .hits
        .iter()
        .find(|hit| hit.resource_id == person_id)
        .expect("updated Party search hit");
    assert_eq!(search_hit.owner_module_id, "crm.parties");
    assert_eq!(search_hit.resource_type, "parties.party");
    assert_eq!(search_hit.source_version, 2);
    assert_eq!(
        search_hit.fields.get("display_name").map(String::as_str),
        Some(updated_person_name.as_str())
    );
    assert_eq!(
        search_hit.fields.get("kind").map(String::as_str),
        Some("person")
    );
    assert_eq!(search_hit.matched_fields, vec!["display_name"]);

    let old_name_search = query(
        &mut grpc,
        &search_definition,
        party_search_payload(&search_definition, &initial_person_name),
        TENANT_A,
        true,
    )
    .await
    .expect("search old Party display name after update");
    assert!(
        decode_search(old_name_search)
            .hits
            .iter()
            .all(|hit| hit.resource_id != person_id),
        "active search generation must not retain the superseded Party display name"
    );

    let organization_search = wait_for_party_search_hit(
        &mut grpc,
        &search_definition,
        &organization_name,
        TENANT_A,
        &organization_id,
        1,
    )
    .await;
    let organization_hit = organization_search
        .hits
        .iter()
        .find(|hit| hit.resource_id == organization_id)
        .expect("Organization Party search hit");
    assert_eq!(
        organization_hit.fields.get("kind").map(String::as_str),
        Some("organization")
    );

    let unauthorized_query = query(
        &mut grpc,
        &get_definition,
        get_person_payload.clone(),
        TENANT_A,
        false,
    )
    .await
    .expect_err("unauthenticated Party query must fail");
    assert_eq!(unauthorized_query.code(), Code::Unauthenticated);

    let cross_tenant_get = query(
        &mut grpc,
        &get_definition,
        get_person_payload,
        TENANT_B,
        true,
    )
    .await
    .expect_err("tenant B must not discover tenant A Party by id");
    assert_eq!(cross_tenant_get.code(), Code::NotFound);

    let cross_tenant_list = query(
        &mut grpc,
        &list_definition,
        party_list_payload(&list_definition, 10, "", None),
        TENANT_B,
        true,
    )
    .await
    .expect("tenant B Party list must succeed without leaking tenant A resources");
    assert!(decode_list_parties(cross_tenant_list).parties.is_empty());

    let cross_tenant_search = query(
        &mut grpc,
        &search_definition,
        party_search_payload(&search_definition, &updated_person_name),
        TENANT_B,
        true,
    )
    .await
    .expect("tenant B Party search must succeed without leaking tenant A resources");
    assert!(decode_search(cross_tenant_search).hits.is_empty());
    assert_eq!(evidence_counts(&admin, TENANT_A).await, after_organization);

    send_sigint(&child).await;
    let exit = timeout(Duration::from_secs(15), child.wait())
        .await
        .expect("crm-api must stop within graceful-shutdown budget")
        .expect("wait for Party acceptance crm-api process");
    assert!(exit.success(), "crm-api exited unsuccessfully: {exit}");
}

async fn mutate(
    client: &mut ApplicationGatewayServiceClient<tonic::transport::Channel>,
    definition: &CapabilityDefinition,
    input: TypedPayload,
    tenant_id: &str,
    idempotency_key: &str,
    authenticated: bool,
) -> Result<crm_application_runtime::gateway_v1::MutateResponse, Status> {
    let mut request = Request::new(GatewayMutateRequest {
        owner_module_id: definition.owner_module_id.as_str().to_owned(),
        capability_id: definition.capability_id.as_str().to_owned(),
        capability_version: definition.capability_version.as_str().to_owned(),
        input: Some(wire_payload(input)),
        approval: None,
    });
    request.metadata_mut().insert(
        "x-tenant-id",
        tenant_id.parse().expect("valid tenant metadata"),
    );
    request.metadata_mut().insert(
        "idempotency-key",
        idempotency_key.parse().expect("valid idempotency metadata"),
    );
    if authenticated {
        request.metadata_mut().insert(
            "authorization",
            format!("Bearer {TOKEN}")
                .parse()
                .expect("valid authorization metadata"),
        );
    }
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
    authenticated: bool,
) -> Result<crm_application_runtime::gateway_v1::QueryResponse, Status> {
    let mut request = Request::new(GatewayQueryRequest {
        owner_module_id: definition.owner_module_id.as_str().to_owned(),
        capability_id: definition.capability_id.as_str().to_owned(),
        capability_version: definition.capability_version.as_str().to_owned(),
        input: Some(wire_payload(input)),
    });
    request.metadata_mut().insert(
        "x-tenant-id",
        tenant_id.parse().expect("valid tenant metadata"),
    );
    if authenticated {
        request.metadata_mut().insert(
            "authorization",
            format!("Bearer {TOKEN}")
                .parse()
                .expect("valid authorization metadata"),
        );
    }
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

fn party_get_payload(definition: &CapabilityDefinition, value: &str) -> TypedPayload {
    payload(
        definition,
        parties::GetPartyRequest {
            party_ref: Some(customer::PartyRef {
                party_id: value.to_owned(),
            }),
        },
    )
}

fn party_list_payload(
    definition: &CapabilityDefinition,
    page_size: i32,
    page_token: &str,
    kind: Option<parties::PartyKind>,
) -> TypedPayload {
    payload(
        definition,
        parties::ListPartiesRequest {
            page: Some(core::PageRequest {
                page_size,
                page_token: page_token.to_owned(),
            }),
            kind: kind.map(|value| value as i32),
            sort: parties::PartySort::UpdatedAtDescending as i32,
        },
    )
}

fn party_search_payload(definition: &CapabilityDefinition, text: &str) -> TypedPayload {
    payload(
        definition,
        search::SearchRequest {
            text: text.to_owned(),
            resource_types: vec!["parties.party".to_owned()],
            page_size: 25,
            cursor: String::new(),
        },
    )
}

fn decode_get_party(
    response: crm_application_runtime::gateway_v1::QueryResponse,
) -> parties::Party {
    parties::GetPartyResponse::decode(
        response
            .output
            .expect("Party query output")
            .payload
            .as_slice(),
    )
    .expect("decode Party query response")
    .party
    .expect("queried Party exists")
}

fn decode_list_parties(
    response: crm_application_runtime::gateway_v1::QueryResponse,
) -> parties::ListPartiesResponse {
    parties::ListPartiesResponse::decode(
        response
            .output
            .expect("Party list output")
            .payload
            .as_slice(),
    )
    .expect("decode Party list response")
}

fn decode_search(
    response: crm_application_runtime::gateway_v1::QueryResponse,
) -> search::SearchResponse {
    search::SearchResponse::decode(
        response
            .output
            .expect("search query output")
            .payload
            .as_slice(),
    )
    .expect("decode search response")
}

async fn wait_for_party_search_hit(
    client: &mut ApplicationGatewayServiceClient<tonic::transport::Channel>,
    definition: &CapabilityDefinition,
    text: &str,
    tenant_id: &str,
    expected_resource_id: &str,
    expected_version: i64,
) -> search::SearchResponse {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let response = query(
            client,
            definition,
            party_search_payload(definition, text),
            tenant_id,
            true,
        )
        .await
        .expect("query Party through governed global search");
        let response = decode_search(response);
        if response.hits.iter().any(|hit| {
            hit.owner_module_id == "crm.parties"
                && hit.resource_type == "parties.party"
                && hit.resource_id == expected_resource_id
                && hit.source_version == expected_version
        }) {
            return response;
        }
        assert!(
            Instant::now() < deadline,
            "Party search projection did not converge before the acceptance deadline"
        );
        sleep(Duration::from_millis(200)).await;
    }
}

fn party_id(party: &parties::Party) -> &str {
    party
        .party_ref
        .as_ref()
        .expect("Party reference")
        .party_id
        .as_str()
}

fn resource_version(party: &parties::Party) -> i64 {
    party
        .resource_version
        .as_ref()
        .expect("Party resource version")
        .version
}

fn payload<M: Message>(definition: &CapabilityDefinition, message: M) -> TypedPayload {
    let data_class = *definition
        .input_contract
        .allowed_data_classes
        .first()
        .expect("governed input contract must declare a data class");
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

async fn evidence_counts(admin: &PgPool, tenant_id: &str) -> EvidenceCounts {
    let records = sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM crm.records WHERE tenant_id = $1 AND owner_module_id = 'crm.parties' AND record_type = 'parties.party' AND deleted_at IS NULL",
    )
    .bind(tenant_id)
    .fetch_one(admin)
    .await
    .expect("count Party records");
    let events = sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM crm.outbox_events WHERE tenant_id = $1 AND event_type IN ('parties.party.created', 'parties.party.updated')",
    )
    .bind(tenant_id)
    .fetch_one(admin)
    .await
    .expect("count Party outbox events");
    let audits =
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM crm.audit_records WHERE tenant_id = $1")
            .bind(tenant_id)
            .fetch_one(admin)
            .await
            .expect("count Party tenant audit evidence");
    let idempotency = sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM crm.idempotency_records WHERE tenant_id = $1",
    )
    .bind(tenant_id)
    .fetch_one(admin)
    .await
    .expect("count Party tenant idempotency evidence");
    let transactions = sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM crm.business_transactions WHERE tenant_id = $1",
    )
    .bind(tenant_id)
    .fetch_one(admin)
    .await
    .expect("count Party tenant business transactions");
    EvidenceCounts {
        records,
        events,
        audits,
        idempotency,
        transactions,
    }
}

async fn wait_until_ready(client: &reqwest::Client, child: &mut Child, http_addr: &str) {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if let Some(status) = child.try_wait().expect("poll crm-api process") {
            panic!("crm-api exited before Party acceptance readiness: {status}");
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
            "Party acceptance crm-api readiness timed out"
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
                    "Party acceptance gRPC listener timed out: {error}"
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
        .expect("send SIGINT to Party acceptance crm-api");
    assert!(status.success(), "kill -INT failed: {status}");
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("bind ephemeral Party acceptance port")
        .local_addr()
        .expect("read ephemeral Party acceptance port")
        .port()
}

fn unique_party_id(kind: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time after Unix epoch")
        .as_nanos();
    format!("party-{kind}-{}-{nanos}", std::process::id())
}

fn unique_display_name(prefix: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time after Unix epoch")
        .as_nanos();
    format!("{prefix} {} {nanos}", std::process::id())
}
