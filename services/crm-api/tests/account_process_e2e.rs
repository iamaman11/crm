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
    accounts::v1 as accounts, core::v1 as core, customer::v1 as customer, parties::v1 as parties,
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
const TOKEN: &str = "account-process-bearer-token-0123456789abcdef0123456789abcdef";
const PARTY_CREATE: &str = "parties.party.create";
const ACCOUNT_CREATE: &str = "accounts.account.create";
const ACCOUNT_UPDATE: &str = "accounts.account.update";
const ACCOUNT_GET: &str = "accounts.account.get";
const ACCOUNT_LIST: &str = "accounts.account.list";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct EvidenceCounts {
    records: i64,
    events: i64,
    audits: i64,
    idempotency: i64,
    transactions: i64,
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn crm_api_process_serves_governed_account_lifecycle_and_party_reference_integrity() {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!("skipping Account process acceptance because DATABASE_URL is absent");
        return;
    };
    let admin_database_url = std::env::var("ADMIN_DATABASE_URL")
        .expect("ADMIN_DATABASE_URL must accompany DATABASE_URL");
    let admin = PgPool::connect(&admin_database_url)
        .await
        .expect("connect Account process evidence reader");
    admin
        .execute(sqlx::raw_sql(include_str!(
            "../../../database/tests/0005_party_adapter.sql"
        )))
        .await
        .expect("publish Party module/capability registry fixture");
    admin
        .execute(sqlx::raw_sql(include_str!(
            "../../../database/tests/0006_account_adapter.sql"
        )))
        .await
        .expect("publish Account module/capability registry fixture");

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
            "account-process-cursor-signing-key-0123456789abcdef",
        )
        .env(
            "CRM_APPROVAL_SIGNING_KEY",
            "account-process-approval-signing-key-0123456789abcdef",
        )
        .env("CRM_BOOTSTRAP_ALLOW_PHASE6", "true")
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .kill_on_drop(true)
        .spawn()
        .expect("spawn production crm-api process for Account acceptance");

    let http = reqwest::Client::new();
    wait_until_ready(&http, &mut child, &http_addr).await;
    let mut grpc = connect_grpc(&grpc_addr).await;

    let party_create = mutation_definition(PARTY_CREATE);
    let primary_party_id = unique_id("party-primary");
    let member_party_id = unique_id("party-member");
    let tenant_b_party_id = unique_id("party-tenant-b");
    create_party(
        &mut grpc,
        &party_create,
        TENANT_A,
        &primary_party_id,
        parties::PartyKind::Organization,
        "Northwind Organization",
        "account-process-party-primary",
    )
    .await;
    create_party(
        &mut grpc,
        &party_create,
        TENANT_A,
        &member_party_id,
        parties::PartyKind::Person,
        "Ada Buyer",
        "account-process-party-member",
    )
    .await;
    create_party(
        &mut grpc,
        &party_create,
        TENANT_B,
        &tenant_b_party_id,
        parties::PartyKind::Organization,
        "Tenant B Organization",
        "account-process-party-tenant-b",
    )
    .await;

    let baseline = account_evidence_counts(&admin, TENANT_A).await;
    let account_create = mutation_definition(ACCOUNT_CREATE);
    let account_update = mutation_definition(ACCOUNT_UPDATE);
    let account_get = query_definition(ACCOUNT_GET);
    let account_list = query_definition(ACCOUNT_LIST);
    let account_id = unique_id("account-primary");
    let second_account_id = unique_id("account-secondary");

    let unavailable_party = mutate(
        &mut grpc,
        &account_create,
        create_account_payload(
            &account_create,
            &unique_id("account-missing-ref"),
            "Missing Reference Account",
            vec![association(
                &unique_id("party-missing"),
                accounts::AccountPartyRole::Primary,
            )],
        ),
        TENANT_A,
        "account-process-missing-party",
        true,
    )
    .await
    .expect_err("missing Party reference must be rejected");
    assert_eq!(unavailable_party.code(), Code::InvalidArgument);
    assert_eq!(account_evidence_counts(&admin, TENANT_A).await, baseline);

    let cross_tenant_party = mutate(
        &mut grpc,
        &account_create,
        create_account_payload(
            &account_create,
            &unique_id("account-cross-tenant-ref"),
            "Cross Tenant Reference Account",
            vec![association(
                &tenant_b_party_id,
                accounts::AccountPartyRole::Primary,
            )],
        ),
        TENANT_A,
        "account-process-cross-tenant-party",
        true,
    )
    .await
    .expect_err("tenant A must not reference tenant B Party");
    assert_eq!(cross_tenant_party.code(), Code::InvalidArgument);
    assert_eq!(account_evidence_counts(&admin, TENANT_A).await, baseline);

    let create_payload = create_account_payload(
        &account_create,
        &account_id,
        "  Northwind   Customer   Group  ",
        vec![
            association(&member_party_id, accounts::AccountPartyRole::Member),
            association(&primary_party_id, accounts::AccountPartyRole::Primary),
        ],
    );
    let created = mutate(
        &mut grpc,
        &account_create,
        create_payload.clone(),
        TENANT_A,
        "account-process-create-primary",
        true,
    )
    .await
    .expect("create Account through production gateway");
    assert!(!created.replayed);
    assert_eq!(created.affected_resources.len(), 1);
    assert_eq!(
        created.affected_resources[0].resource_type,
        "accounts.account"
    );
    assert_eq!(created.affected_resources[0].resource_id, account_id);
    assert_eq!(created.affected_resources[0].version, Some(1));
    let created_output = created.output.expect("Account create output");
    let created_account = decode_create_account(&created_output.payload);
    assert_eq!(account_id_of(&created_account), account_id);
    assert_eq!(created_account.name, "Northwind Customer Group");
    assert_eq!(
        created_account.status,
        accounts::AccountStatus::Active as i32
    );
    assert_eq!(created_account.party_associations.len(), 2);
    assert_eq!(resource_version(&created_account), 1);
    assert_primary_party(&created_account, &primary_party_id);

    let after_create = account_evidence_counts(&admin, TENANT_A).await;
    assert_eq!(after_create.records, baseline.records + 1);
    assert_eq!(after_create.events, baseline.events + 1);
    assert_eq!(after_create.audits, baseline.audits + 1);
    assert_eq!(after_create.idempotency, baseline.idempotency + 1);
    assert_eq!(after_create.transactions, baseline.transactions + 1);

    let replay = mutate(
        &mut grpc,
        &account_create,
        create_payload,
        TENANT_A,
        "account-process-create-primary",
        true,
    )
    .await
    .expect("replay Account create through production gateway");
    assert!(replay.replayed);
    assert_eq!(
        replay.output.expect("Account replay output").payload,
        created_output.payload
    );
    assert_eq!(
        account_evidence_counts(&admin, TENANT_A).await,
        after_create
    );

    let queried = query(
        &mut grpc,
        &account_get,
        get_account_payload(&account_get, &account_id),
        TENANT_A,
        true,
    )
    .await
    .expect("get Account through production query gateway");
    let queried_account = decode_get_account(queried);
    assert_eq!(queried_account.name, "Northwind Customer Group");
    assert_eq!(resource_version(&queried_account), 1);

    let update_payload = payload(
        &account_update,
        accounts::UpdateAccountRequest {
            account_ref: Some(customer::AccountRef {
                account_id: account_id.clone(),
            }),
            expected_version: 1,
            name: "Northwind Strategic Account".to_owned(),
            status: accounts::AccountStatus::Inactive as i32,
            party_associations: vec![association(
                &primary_party_id,
                accounts::AccountPartyRole::Primary,
            )],
        },
    );
    let updated = mutate(
        &mut grpc,
        &account_update,
        update_payload.clone(),
        TENANT_A,
        "account-process-update-primary-v1",
        true,
    )
    .await
    .expect("update Account through production gateway");
    assert!(!updated.replayed);
    assert_eq!(updated.affected_resources[0].version, Some(2));
    let updated_output = updated.output.expect("Account update output");
    let updated_account = decode_update_account(&updated_output.payload);
    assert_eq!(updated_account.name, "Northwind Strategic Account");
    assert_eq!(
        updated_account.status,
        accounts::AccountStatus::Inactive as i32
    );
    assert_eq!(updated_account.party_associations.len(), 1);
    assert_primary_party(&updated_account, &primary_party_id);
    assert_eq!(resource_version(&updated_account), 2);

    let after_update = account_evidence_counts(&admin, TENANT_A).await;
    assert_eq!(after_update.records, after_create.records);
    assert_eq!(after_update.events, after_create.events + 1);
    assert_eq!(after_update.audits, after_create.audits + 1);
    assert_eq!(after_update.idempotency, after_create.idempotency + 1);
    assert_eq!(after_update.transactions, after_create.transactions + 1);

    let update_replay = mutate(
        &mut grpc,
        &account_update,
        update_payload,
        TENANT_A,
        "account-process-update-primary-v1",
        true,
    )
    .await
    .expect("replay Account update through production gateway");
    assert!(update_replay.replayed);
    assert_eq!(
        update_replay
            .output
            .expect("Account update replay output")
            .payload,
        updated_output.payload
    );
    assert_eq!(
        account_evidence_counts(&admin, TENANT_A).await,
        after_update
    );

    let stale = mutate(
        &mut grpc,
        &account_update,
        payload(
            &account_update,
            accounts::UpdateAccountRequest {
                account_ref: Some(customer::AccountRef {
                    account_id: account_id.clone(),
                }),
                expected_version: 1,
                name: "Stale Account Update".to_owned(),
                status: accounts::AccountStatus::Active as i32,
                party_associations: vec![association(
                    &primary_party_id,
                    accounts::AccountPartyRole::Primary,
                )],
            },
        ),
        TENANT_A,
        "account-process-stale-update",
        true,
    )
    .await
    .expect_err("stale Account version must fail");
    assert_eq!(stale.code(), Code::Aborted);
    assert_eq!(
        account_evidence_counts(&admin, TENANT_A).await,
        after_update
    );

    let second_created = mutate(
        &mut grpc,
        &account_create,
        create_account_payload(
            &account_create,
            &second_account_id,
            "Northwind Active Account",
            vec![association(
                &primary_party_id,
                accounts::AccountPartyRole::Primary,
            )],
        ),
        TENANT_A,
        "account-process-create-secondary",
        true,
    )
    .await
    .expect("create second Account through production gateway");
    assert!(!second_created.replayed);

    let after_second_create = account_evidence_counts(&admin, TENANT_A).await;
    assert_eq!(after_second_create.records, after_update.records + 1);
    assert_eq!(after_second_create.events, after_update.events + 1);
    assert_eq!(after_second_create.audits, after_update.audits + 1);
    assert_eq!(
        after_second_create.idempotency,
        after_update.idempotency + 1
    );
    assert_eq!(
        after_second_create.transactions,
        after_update.transactions + 1
    );

    let first_page = query(
        &mut grpc,
        &account_list,
        list_accounts_payload(&account_list, 1, "", None),
        TENANT_A,
        true,
    )
    .await
    .expect("list first Account page");
    let first_page = decode_list_accounts(first_page);
    assert_eq!(first_page.accounts.len(), 1);
    let next_page_token = first_page
        .page
        .expect("first Account page info")
        .next_page_token;
    assert!(!next_page_token.is_empty());

    let second_page = query(
        &mut grpc,
        &account_list,
        list_accounts_payload(&account_list, 1, &next_page_token, None),
        TENANT_A,
        true,
    )
    .await
    .expect("list second Account page");
    let second_page = decode_list_accounts(second_page);
    assert_eq!(second_page.accounts.len(), 1);
    assert!(
        second_page
            .page
            .expect("second Account page info")
            .next_page_token
            .is_empty()
    );
    let listed_ids = BTreeSet::from([
        account_id_of(&first_page.accounts[0]).to_owned(),
        account_id_of(&second_page.accounts[0]).to_owned(),
    ]);
    assert_eq!(
        listed_ids,
        BTreeSet::from([account_id.clone(), second_account_id.clone()])
    );

    let inactive = query(
        &mut grpc,
        &account_list,
        list_accounts_payload(
            &account_list,
            10,
            "",
            Some(accounts::AccountStatus::Inactive),
        ),
        TENANT_A,
        true,
    )
    .await
    .expect("list inactive Accounts");
    let inactive = decode_list_accounts(inactive);
    assert_eq!(inactive.accounts.len(), 1);
    assert_eq!(account_id_of(&inactive.accounts[0]), account_id);
    assert_eq!(resource_version(&inactive.accounts[0]), 2);

    let unauthenticated = query(
        &mut grpc,
        &account_get,
        get_account_payload(&account_get, &account_id),
        TENANT_A,
        false,
    )
    .await
    .expect_err("unauthenticated Account query must fail");
    assert_eq!(unauthenticated.code(), Code::Unauthenticated);

    let cross_tenant_get = query(
        &mut grpc,
        &account_get,
        get_account_payload(&account_get, &account_id),
        TENANT_B,
        true,
    )
    .await
    .expect_err("tenant B must not discover tenant A Account");
    assert_eq!(cross_tenant_get.code(), Code::NotFound);

    let cross_tenant_list = query(
        &mut grpc,
        &account_list,
        list_accounts_payload(&account_list, 10, "", None),
        TENANT_B,
        true,
    )
    .await
    .expect("tenant B Account list must not leak tenant A resources");
    assert!(decode_list_accounts(cross_tenant_list).accounts.is_empty());
    assert_eq!(
        account_evidence_counts(&admin, TENANT_A).await,
        after_second_create
    );

    send_sigint(&child).await;
    let exit = timeout(Duration::from_secs(15), child.wait())
        .await
        .expect("crm-api must stop within graceful-shutdown budget")
        .expect("wait for Account acceptance crm-api process");
    assert!(exit.success(), "crm-api exited unsuccessfully: {exit}");
}

async fn create_party(
    client: &mut ApplicationGatewayServiceClient<tonic::transport::Channel>,
    definition: &CapabilityDefinition,
    tenant_id: &str,
    party_id: &str,
    kind: parties::PartyKind,
    display_name: &str,
    idempotency_key: &str,
) {
    let response = mutate(
        client,
        definition,
        payload(
            definition,
            parties::CreatePartyRequest {
                party_ref: Some(customer::PartyRef {
                    party_id: party_id.to_owned(),
                }),
                kind: kind as i32,
                display_name: display_name.to_owned(),
            },
        ),
        tenant_id,
        idempotency_key,
        true,
    )
    .await
    .expect("create Party prerequisite through production gateway");
    assert!(!response.replayed);
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

fn association(
    party_id: &str,
    role: accounts::AccountPartyRole,
) -> accounts::AccountPartyAssociation {
    accounts::AccountPartyAssociation {
        party_ref: Some(customer::PartyRef {
            party_id: party_id.to_owned(),
        }),
        role: role as i32,
    }
}

fn create_account_payload(
    definition: &CapabilityDefinition,
    account_id: &str,
    name: &str,
    party_associations: Vec<accounts::AccountPartyAssociation>,
) -> TypedPayload {
    payload(
        definition,
        accounts::CreateAccountRequest {
            account_ref: Some(customer::AccountRef {
                account_id: account_id.to_owned(),
            }),
            name: name.to_owned(),
            party_associations,
        },
    )
}

fn get_account_payload(definition: &CapabilityDefinition, account_id: &str) -> TypedPayload {
    payload(
        definition,
        accounts::GetAccountRequest {
            account_ref: Some(customer::AccountRef {
                account_id: account_id.to_owned(),
            }),
        },
    )
}

fn list_accounts_payload(
    definition: &CapabilityDefinition,
    page_size: i32,
    page_token: &str,
    status: Option<accounts::AccountStatus>,
) -> TypedPayload {
    payload(
        definition,
        accounts::ListAccountsRequest {
            page: Some(core::PageRequest {
                page_size,
                page_token: page_token.to_owned(),
            }),
            status: status.map(|value| value as i32),
            sort: accounts::AccountSort::UpdatedAtDescending as i32,
        },
    )
}

fn decode_create_account(bytes: &[u8]) -> accounts::Account {
    accounts::CreateAccountResponse::decode(bytes)
        .expect("decode Account create response")
        .account
        .expect("created Account exists")
}

fn decode_update_account(bytes: &[u8]) -> accounts::Account {
    accounts::UpdateAccountResponse::decode(bytes)
        .expect("decode Account update response")
        .account
        .expect("updated Account exists")
}

fn decode_get_account(
    response: crm_application_runtime::gateway_v1::QueryResponse,
) -> accounts::Account {
    accounts::GetAccountResponse::decode(
        response
            .output
            .expect("Account query output")
            .payload
            .as_slice(),
    )
    .expect("decode Account query response")
    .account
    .expect("queried Account exists")
}

fn decode_list_accounts(
    response: crm_application_runtime::gateway_v1::QueryResponse,
) -> accounts::ListAccountsResponse {
    accounts::ListAccountsResponse::decode(
        response
            .output
            .expect("Account list output")
            .payload
            .as_slice(),
    )
    .expect("decode Account list response")
}

fn account_id_of(account: &accounts::Account) -> &str {
    account
        .account_ref
        .as_ref()
        .expect("Account reference")
        .account_id
        .as_str()
}

fn resource_version(account: &accounts::Account) -> i64 {
    account
        .resource_version
        .as_ref()
        .expect("Account resource version")
        .version
}

fn assert_primary_party(account: &accounts::Account, expected_party_id: &str) {
    let primary = account
        .party_associations
        .iter()
        .find(|association| association.role == accounts::AccountPartyRole::Primary as i32)
        .expect("Account primary Party association");
    assert_eq!(
        primary
            .party_ref
            .as_ref()
            .expect("primary Party reference")
            .party_id,
        expected_party_id
    );
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

async fn account_evidence_counts(admin: &PgPool, tenant_id: &str) -> EvidenceCounts {
    let records = sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM crm.records WHERE tenant_id = $1 AND owner_module_id = 'crm.customer-accounts' AND record_type = 'accounts.account' AND deleted_at IS NULL",
    )
    .bind(tenant_id)
    .fetch_one(admin)
    .await
    .expect("count Account records");
    let events = sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM crm.outbox_events WHERE tenant_id = $1 AND event_type IN ('accounts.account.created', 'accounts.account.updated')",
    )
    .bind(tenant_id)
    .fetch_one(admin)
    .await
    .expect("count Account outbox events");
    let audits =
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM crm.audit_records WHERE tenant_id = $1")
            .bind(tenant_id)
            .fetch_one(admin)
            .await
            .expect("count Account audit evidence");
    let idempotency = sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM crm.idempotency_records WHERE tenant_id = $1",
    )
    .bind(tenant_id)
    .fetch_one(admin)
    .await
    .expect("count Account idempotency evidence");
    let transactions = sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM crm.business_transactions WHERE tenant_id = $1",
    )
    .bind(tenant_id)
    .fetch_one(admin)
    .await
    .expect("count Account business transactions");
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
            panic!("crm-api exited before Account acceptance readiness: {status}");
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
            "Account acceptance crm-api readiness timed out"
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
                    "Account acceptance gRPC listener timed out: {error}"
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
        .expect("send SIGINT to Account acceptance crm-api");
    assert!(status.success(), "kill -INT failed: {status}");
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("bind ephemeral Account acceptance port")
        .local_addr()
        .expect("read ephemeral Account acceptance port")
        .port()
}

fn unique_id(prefix: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time after Unix epoch")
        .as_nanos();
    format!("{prefix}-{}-{nanos}", std::process::id())
}
