mod support;

use crm_application_runtime::gateway_v1::application_gateway_service_client::ApplicationGatewayServiceClient;
use crm_customer_360_composition::{
    CUSTOMER_360_CONTRIBUTION_RESOURCE_TYPE, CUSTOMER_360_PROJECTION_ID,
};
use crm_proto_contracts::crm::{customer::v1 as customer, parties::v1 as parties};
use reqwest::Client as HttpClient;
use serde_json::{Value, json};
use sqlx::{PgPool, Row};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use support::customer_enrichment_process::*;
use tokio::time::sleep;

const LEGACY_CUSTOMER_360_PROJECTION_ID: &str = "customer.customer-360.v1";
const ORIGINAL_NAME: &str = "CRM API Privacy Orphan Company";
const OWNER_ACTION_EVENT_DESCRIPTOR_HASH: [u8; 32] = [
    213, 213, 180, 13, 242, 156, 208, 33, 85, 11, 63, 152, 114, 199, 7, 154, 115, 225, 181, 172,
    233, 62, 83, 56, 65, 78, 35, 3, 176, 230, 31, 123,
];

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn crm_api_background_worker_repairs_party_privacy_orphan_into_v2() {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!("skipping crm-api Customer 360 no-orphan process test without DATABASE_URL");
        return;
    };
    let admin_database_url = std::env::var("ADMIN_DATABASE_URL")
        .expect("ADMIN_DATABASE_URL must accompany DATABASE_URL");
    let admin = PgPool::connect(&admin_database_url)
        .await
        .expect("connect no-orphan admin pool");
    let run_id = unique_id();
    let party_id = format!("party-process-no-orphan-{run_id}");
    let event_id = format!("event-process-no-orphan-{run_id}");
    let business_transaction_id = format!("tx-process-no-orphan-{run_id}");

    let http_port = free_port();
    let grpc_port = free_port();
    let http_addr = format!("127.0.0.1:{http_port}");
    let grpc_addr = format!("127.0.0.1:{grpc_port}");
    let http = HttpClient::new();
    let mut first_process = spawn_crm_api(
        &database_url,
        &http_addr,
        &grpc_addr,
        true,
        None,
    );
    wait_until_ready(&http, &mut first_process, &http_addr, true).await;
    let mut gateway = connect_grpc(&grpc_addr).await;
    create_party(&mut gateway, &party_id).await;
    wait_for_party_record(&admin, &party_id).await;
    stop_process(&mut first_process).await;

    seed_privacy_orphan(
        &admin,
        &party_id,
        &event_id,
        &business_transaction_id,
    )
    .await;
    assert_no_v2_document(&admin, &party_id).await;
    assert_legacy_v1_stale(&admin, &party_id).await;

    let restart_http_port = free_port();
    let restart_grpc_port = free_port();
    let restart_http_addr = format!("127.0.0.1:{restart_http_port}");
    let restart_grpc_addr = format!("127.0.0.1:{restart_grpc_port}");
    let mut restarted_process = spawn_crm_api(
        &database_url,
        &restart_http_addr,
        &restart_grpc_addr,
        false,
        None,
    );
    wait_until_ready(
        &http,
        &mut restarted_process,
        &restart_http_addr,
        true,
    )
    .await;
    wait_for_v2_tombstone(&admin, &party_id).await;
    assert_legacy_v1_stale(&admin, &party_id).await;
    assert_authoritative_party_minimized(&admin, &party_id).await;
    stop_process(&mut restarted_process).await;
}

async fn create_party(
    gateway: &mut ApplicationGatewayServiceClient<tonic::transport::Channel>,
    party_id: &str,
) {
    let definition = mutation_definition(PARTY_CREATE);
    let input = payload(
        &definition,
        parties::CreatePartyRequest {
            party_ref: Some(customer::PartyRef {
                party_id: party_id.to_owned(),
            }),
            kind: parties::PartyKind::Organization as i32,
            display_name: ORIGINAL_NAME.to_owned(),
        },
    );
    mutate(
        gateway,
        &definition,
        input,
        TENANT_A,
        &format!("party-process-no-orphan-create-{party_id}"),
        true,
    )
    .await
    .expect("create Party through real crm-api gateway");
}

async fn wait_for_party_record(admin: &PgPool, party_id: &str) {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let count: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM crm.records WHERE tenant_id = $1 AND record_type = 'parties.party' AND record_id = $2 AND version = 1",
        )
        .bind(TENANT_A)
        .bind(party_id)
        .fetch_one(admin)
        .await
        .unwrap();
        if count == 1 {
            return;
        }
        assert!(Instant::now() < deadline, "Party record did not become visible");
        sleep(Duration::from_millis(100)).await;
    }
}

async fn seed_privacy_orphan(
    admin: &PgPool,
    party_id: &str,
    event_id: &str,
    business_transaction_id: &str,
) {
    let payload = json!({
        "action_code": "anonymize",
        "owner_capability_id": "parties.privacy.action.apply",
        "owner_capability_version": "1.0.0",
        "owner_module_id": "crm.parties",
        "resource_id": party_id,
        "resource_type": "parties.party",
        "resource_version": "1",
        "tenant_id": TENANT_A
    })
    .to_string()
    .into_bytes();
    let minimized_state = serde_json::to_vec(&json!({
        "party_id": party_id,
        "kind": "organization",
        "display_name": format!("minimized organization {party_id}"),
        "created_at_unix_nanos": 1,
        "updated_at_unix_nanos": 2,
        "version": 2
    }))
    .unwrap();

    let mut transaction = admin.begin().await.unwrap();
    sqlx::query("SET LOCAL session_replication_role = replica")
        .execute(&mut *transaction)
        .await
        .unwrap();
    sqlx::query(
        r#"
        INSERT INTO crm.capability_registry (
          capability_id, capability_version, owner_module_id, owner_module_version,
          service_name, method_name, input_descriptor_hash, output_descriptor_hash,
          risk_level, idempotency_required, audit_required, approval_required,
          ai_callable, marketplace_callable, bulk_allowed, export_allowed,
          data_classes_touched
        ) VALUES (
          'parties.privacy.action.apply', '1.0.0', 'crm.parties', '0.3.0',
          'crm.parties.internal.PrivacyOwnerAction', 'Apply', $1, $1,
          'critical', true, true, false, false, false, false, false,
          ARRAY['personal', 'restricted']::text[]
        ) ON CONFLICT (capability_id, capability_version) DO NOTHING
        "#,
    )
    .bind(OWNER_ACTION_EVENT_DESCRIPTOR_HASH.as_slice())
    .execute(&mut *transaction)
    .await
    .unwrap();
    sqlx::query(
        r#"
        INSERT INTO crm.business_transactions (
          tenant_id, business_transaction_id, actor_id, request_id,
          correlation_id, trace_id, capability_id, capability_version,
          expected_outbox_events, expected_audit_records, expected_idempotency_records
        ) VALUES ($1, $2, $3, $4, $5, $6,
                  'parties.privacy.action.apply', '1.0.0', 1, 1, 1)
        "#,
    )
    .bind(TENANT_A)
    .bind(business_transaction_id)
    .bind(ACTOR)
    .bind(format!("request-{business_transaction_id}"))
    .bind(format!("correlation-{business_transaction_id}"))
    .bind(format!("trace-{business_transaction_id}"))
    .execute(&mut *transaction)
    .await
    .unwrap();
    sqlx::query(
        r#"
        UPDATE crm.records
        SET version = 2,
            payload_bytes = $3,
            last_business_transaction_id = $4,
            updated_at = clock_timestamp()
        WHERE tenant_id = $1 AND record_type = 'parties.party' AND record_id = $2
        "#,
    )
    .bind(TENANT_A)
    .bind(party_id)
    .bind(minimized_state)
    .bind(business_transaction_id)
    .execute(&mut *transaction)
    .await
    .unwrap();
    sqlx::query(
        r#"
        INSERT INTO crm.outbox_events (
          tenant_id, event_id, business_transaction_id,
          aggregate_type, aggregate_id, aggregate_version, event_sequence,
          event_type, schema_id, schema_version, descriptor_hash,
          data_class, maximum_payload_size, retention_policy_id,
          payload_bytes, occurred_at
        ) VALUES (
          $1, $2, $3, 'parties.party', $4, 2, 2,
          'parties.privacy.action.apply.completed',
          'crm.customer-privacy.owner_action.event', '1.0.0', $5,
          'restricted', 32768, 'crm.customer_privacy.owner_action_command',
          $6, clock_timestamp()
        )
        "#,
    )
    .bind(TENANT_A)
    .bind(event_id)
    .bind(business_transaction_id)
    .bind(party_id)
    .bind(OWNER_ACTION_EVENT_DESCRIPTOR_HASH.as_slice())
    .bind(payload)
    .execute(&mut *transaction)
    .await
    .unwrap();
    sqlx::query(
        "DELETE FROM crm.projection_documents WHERE tenant_id = $1 AND projection_id = $2",
    )
    .bind(TENANT_A)
    .bind(CUSTOMER_360_PROJECTION_ID)
    .execute(&mut *transaction)
    .await
    .unwrap();
    sqlx::query(
        "DELETE FROM crm.projection_checkpoints WHERE tenant_id = $1 AND projection_id = $2",
    )
    .bind(TENANT_A)
    .bind(CUSTOMER_360_PROJECTION_ID)
    .execute(&mut *transaction)
    .await
    .unwrap();
    sqlx::query(
        r#"
        INSERT INTO crm.projection_checkpoints (
          tenant_id, projection_id, last_occurred_at, last_event_id,
          applied_event_count, status
        ) VALUES ($1, $2, TIMESTAMPTZ 'epoch', $3, 1, 'active')
        ON CONFLICT (tenant_id, projection_id) DO UPDATE
        SET last_occurred_at = EXCLUDED.last_occurred_at,
            last_event_id = EXCLUDED.last_event_id,
            applied_event_count = 1,
            status = 'active'
        "#,
    )
    .bind(TENANT_A)
    .bind(LEGACY_CUSTOMER_360_PROJECTION_ID)
    .bind(event_id)
    .execute(&mut *transaction)
    .await
    .unwrap();
    sqlx::query(
        r#"
        INSERT INTO crm.projection_documents (
          tenant_id, projection_id, resource_type, resource_id,
          source_version, source_event_id, document
        ) VALUES (
          $1, $2, $3, 'party:' || $4, 1, $5,
          jsonb_build_object(
            'projection_schema_version', '1',
            'contribution_kind', 'party',
            'root_party_ids', jsonb_build_array($4::text),
            'source_owner_module_id', 'crm.parties',
            'source_resource_type', 'parties.party',
            'source_resource_id', $4::text,
            'source_version', 1,
            'source_event_id', $5::text,
            'snapshot', jsonb_build_object(
              'snapshot_kind', 'party',
              'kind', 'organization',
              'display_name', $6::text,
              'privacy_lifecycle', 'active'
            )
          )
        )
        ON CONFLICT (tenant_id, projection_id, resource_type, resource_id) DO UPDATE
        SET source_version = EXCLUDED.source_version,
            source_event_id = EXCLUDED.source_event_id,
            document = EXCLUDED.document
        "#,
    )
    .bind(TENANT_A)
    .bind(LEGACY_CUSTOMER_360_PROJECTION_ID)
    .bind(CUSTOMER_360_CONTRIBUTION_RESOURCE_TYPE)
    .bind(party_id)
    .bind(event_id)
    .bind(ORIGINAL_NAME)
    .execute(&mut *transaction)
    .await
    .unwrap();
    transaction.commit().await.unwrap();
}

async fn assert_no_v2_document(admin: &PgPool, party_id: &str) {
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM crm.projection_documents WHERE tenant_id = $1 AND projection_id = $2 AND resource_type = $3 AND resource_id = 'party:' || $4",
    )
    .bind(TENANT_A)
    .bind(CUSTOMER_360_PROJECTION_ID)
    .bind(CUSTOMER_360_CONTRIBUTION_RESOURCE_TYPE)
    .bind(party_id)
    .fetch_one(admin)
    .await
    .unwrap();
    assert_eq!(count, 0);
}

async fn wait_for_v2_tombstone(admin: &PgPool, party_id: &str) {
    let deadline = Instant::now() + Duration::from_secs(35);
    loop {
        let row = sqlx::query(
            r#"
            SELECT source_version,
                   document -> 'root_party_ids' = '[]'::jsonb AS roots_removed,
                   document #>> '{snapshot,privacy_lifecycle}' AS lifecycle,
                   document::text LIKE '%' || $5 || '%' AS leaks_original
            FROM crm.projection_documents
            WHERE tenant_id = $1 AND projection_id = $2
              AND resource_type = $3 AND resource_id = 'party:' || $4
            "#,
        )
        .bind(TENANT_A)
        .bind(CUSTOMER_360_PROJECTION_ID)
        .bind(CUSTOMER_360_CONTRIBUTION_RESOURCE_TYPE)
        .bind(party_id)
        .bind(ORIGINAL_NAME)
        .fetch_optional(admin)
        .await
        .unwrap();
        if let Some(row) = row
            && row.try_get::<i64, _>("source_version").unwrap() == 2
            && row.try_get::<bool, _>("roots_removed").unwrap()
            && row.try_get::<String, _>("lifecycle").unwrap() == "privacy_minimized"
            && !row.try_get::<bool, _>("leaks_original").unwrap()
        {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "real crm-api process did not repair Customer 360 privacy orphan"
        );
        sleep(Duration::from_millis(200)).await;
    }
}

async fn assert_legacy_v1_stale(admin: &PgPool, party_id: &str) {
    let row = sqlx::query(
        r#"
        SELECT source_version,
               document #>> '{snapshot,display_name}' AS display_name,
               document -> 'root_party_ids' @> jsonb_build_array($4::text) AS has_root
        FROM crm.projection_documents
        WHERE tenant_id = $1 AND projection_id = $2
          AND resource_type = $3 AND resource_id = 'party:' || $4
        "#,
    )
    .bind(TENANT_A)
    .bind(LEGACY_CUSTOMER_360_PROJECTION_ID)
    .bind(CUSTOMER_360_CONTRIBUTION_RESOURCE_TYPE)
    .bind(party_id)
    .fetch_one(admin)
    .await
    .unwrap();
    assert_eq!(row.try_get::<i64, _>("source_version").unwrap(), 1);
    assert_eq!(row.try_get::<String, _>("display_name").unwrap(), ORIGINAL_NAME);
    assert!(row.try_get::<bool, _>("has_root").unwrap());
}

async fn assert_authoritative_party_minimized(admin: &PgPool, party_id: &str) {
    let row = sqlx::query(
        "SELECT version, payload_bytes FROM crm.records WHERE tenant_id = $1 AND record_type = 'parties.party' AND record_id = $2",
    )
    .bind(TENANT_A)
    .bind(party_id)
    .fetch_one(admin)
    .await
    .unwrap();
    let state: Value = serde_json::from_slice(&row.try_get::<Vec<u8>, _>("payload_bytes").unwrap())
        .unwrap();
    assert_eq!(row.try_get::<i64, _>("version").unwrap(), 2);
    assert_eq!(state["version"], 2);
    assert!(!state["display_name"]
        .as_str()
        .unwrap()
        .contains(ORIGINAL_NAME));
}

fn unique_id() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock after unix epoch")
        .as_nanos()
}
