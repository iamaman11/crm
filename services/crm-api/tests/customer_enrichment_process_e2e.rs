#![cfg(unix)]

#[path = "support/customer_enrichment_process/mod.rs"]
mod support;

use crm_application_runtime::gateway_v1::application_gateway_service_client::ApplicationGatewayServiceClient;
use reqwest::Client as HttpClient;
use serde_json::json;
use sqlx::PgPool;
use tonic::{Code, Status};

use support::*;

const HIDDEN_PROFILE_DEFINITION: &str = "customer_enrichment.provider_profile.get|crm.customer-enrichment|customer_enrichment.provider_profile_version|definition";

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn customer_enrichment_real_process_denials_are_bounded_and_side_effect_free() {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!(
            "skipping Customer Enrichment crm-api process test because DATABASE_URL is absent"
        );
        return;
    };
    let admin_database_url = std::env::var("ADMIN_DATABASE_URL")
        .expect("ADMIN_DATABASE_URL must accompany DATABASE_URL");
    let admin = PgPool::connect(&admin_database_url)
        .await
        .expect("connect Customer Enrichment process evidence reader");

    let profile_definition = mutation_definition(PUBLISH_PROFILE);
    let mapping_definition = mutation_definition(PUBLISH_MAPPING);
    let request_definition = mutation_definition(CREATE_REQUEST);
    let party_definition = mutation_definition(PARTY_CREATE);
    let profile_query_definition = query_definition(GET_PROFILE);
    for definition in [
        &profile_definition,
        &mapping_definition,
        &request_definition,
        &profile_query_definition,
    ] {
        assert_customer_enrichment_owner(definition);
    }

    let http_port = free_port();
    let grpc_port = free_port();
    assert_ne!(http_port, grpc_port);
    let http_addr = format!("127.0.0.1:{http_port}");
    let grpc_addr = format!("127.0.0.1:{grpc_port}");
    let http = HttpClient::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .expect("build Customer Enrichment HTTP client");
    let mut process = spawn_crm_api(
        &database_url,
        &http_addr,
        &grpc_addr,
        true,
        Some(HIDDEN_PROFILE_DEFINITION),
    );
    wait_until_ready(&http, &mut process, &http_addr, true).await;
    let mut grpc = connect_grpc(&grpc_addr).await;

    let initial = evidence_counts(&admin).await;
    let unauthenticated = http_mutate(
        &http,
        &http_addr,
        &profile_definition,
        &profile_payload(&profile_definition, "unauthenticated-profile"),
        TENANT_A,
        "crm-api-enrichment-unauthenticated",
        false,
    )
    .await;
    assert_eq!(unauthenticated.status(), reqwest::StatusCode::UNAUTHORIZED);
    let unauthenticated_body: serde_json::Value = unauthenticated
        .json()
        .await
        .expect("decode unauthenticated HTTP response");
    assert_eq!(unauthenticated_body, json!({"error": "request_failed"}));
    assert_safe_text(&unauthenticated_body.to_string());
    assert_eq!(evidence_counts(&admin).await, initial);

    mutate(
        &mut grpc,
        &party_definition,
        party_payload(&party_definition),
        TENANT_A,
        "crm-api-enrichment-party-create",
        true,
    )
    .await
    .expect("create governed Party through crm-api");
    let profile_response = mutate(
        &mut grpc,
        &profile_definition,
        profile_payload(&profile_definition, "crm-api-process-profile"),
        TENANT_A,
        "crm-api-enrichment-profile-publish",
        true,
    )
    .await
    .expect("publish provider profile through crm-api");
    let profile_id = decode_profile_id(&profile_response);
    let mapping_response = mutate(
        &mut grpc,
        &mapping_definition,
        mapping_payload(&mapping_definition, &profile_id),
        TENANT_A,
        "crm-api-enrichment-mapping-publish",
        true,
    )
    .await
    .expect("publish mapping through crm-api");
    let mapping_id = decode_mapping_id(&mapping_response);
    let baseline = evidence_counts(&admin).await;

    let visible_profile = query(
        &mut grpc,
        &profile_query_definition,
        get_profile_payload(&profile_query_definition, &profile_id),
        TENANT_A,
        true,
    )
    .await
    .expect("query provider profile through crm-api");
    let visible_profile = decode_profile_query(visible_profile);
    assert!(
        visible_profile.definition.is_none(),
        "deployment field ceiling must redact the confidential definition"
    );
    assert_eq!(evidence_counts(&admin).await, baseline);

    let cross_tenant = query(
        &mut grpc,
        &profile_query_definition,
        get_profile_payload(&profile_query_definition, &profile_id),
        TENANT_B,
        true,
    )
    .await
    .expect_err("cross-tenant provider profile must be concealed");
    assert_safe_status(
        &cross_tenant,
        Code::NotFound,
        "CUSTOMER_ENRICHMENT_PROVIDER_PROFILE_NOT_FOUND",
    );
    assert_eq!(evidence_counts(&admin).await, baseline);

    let forbidden_tenant = query(
        &mut grpc,
        &profile_query_definition,
        get_profile_payload(&profile_query_definition, &profile_id),
        TENANT_OUTSIDE_TOKEN,
        true,
    )
    .await
    .expect_err("token must not cross its tenant set");
    assert_safe_status(
        &forbidden_tenant,
        Code::PermissionDenied,
        "TENANT_FORBIDDEN",
    );
    assert_eq!(evidence_counts(&admin).await, baseline);

    let missing_consent = mutate(
        &mut grpc,
        &request_definition,
        missing_consent_request_payload(
            &request_definition,
            profile_id.as_str(),
            mapping_id.as_str(),
        ),
        TENANT_A,
        "crm-api-enrichment-missing-consent",
        true,
    )
    .await
    .expect_err("consent legal basis must require exact evidence");
    assert_safe_status(
        &missing_consent,
        Code::PermissionDenied,
        "CUSTOMER_ENRICHMENT_REQUEST_CONSENT_DENIED",
    );
    assert_eq!(evidence_counts(&admin).await, baseline);

    set_customer_enrichment_status(&admin, "suspended").await;
    let inactive = mutate(
        &mut grpc,
        &profile_definition,
        profile_payload(&profile_definition, "inactive-profile"),
        TENANT_A,
        "crm-api-enrichment-module-inactive",
        true,
    )
    .await
    .expect_err("suspended Customer Enrichment module must reject mutations");
    assert_safe_status(&inactive, Code::Aborted, "MODULE_NOT_ACTIVE");
    assert_eq!(evidence_counts(&admin).await, baseline);
    set_customer_enrichment_status(&admin, "active").await;
    stop_process(&mut process).await;

    let denied_http_port = free_port();
    let denied_grpc_port = free_port();
    let denied_http_addr = format!("127.0.0.1:{denied_http_port}");
    let denied_grpc_addr = format!("127.0.0.1:{denied_grpc_port}");
    let mut denied_process = spawn_crm_api(
        &database_url,
        &denied_http_addr,
        &denied_grpc_addr,
        false,
        None,
    );
    wait_until_ready(&http, &mut denied_process, &denied_http_addr, false).await;
    let mut denied_grpc: ApplicationGatewayServiceClient<tonic::transport::Channel> =
        connect_grpc(&denied_grpc_addr).await;
    let authorization_denied = mutate(
        &mut denied_grpc,
        &profile_definition,
        profile_payload(&profile_definition, "authorization-denied-profile"),
        TENANT_A,
        "crm-api-enrichment-authorization-denied",
        true,
    )
    .await
    .expect_err("authenticated request without live grant must be denied");
    assert_safe_status(
        &authorization_denied,
        Code::PermissionDenied,
        "CAPABILITY_PERMISSION_DENIED",
    );
    assert_eq!(evidence_counts(&admin).await, baseline);
    stop_process(&mut denied_process).await;
}

fn assert_safe_status(status: &Status, expected_code: Code, expected_error_code: &str) {
    assert_eq!(status.code(), expected_code);
    assert_eq!(
        status
            .metadata()
            .get("x-error-code")
            .expect("typed gRPC error code")
            .to_str()
            .expect("ASCII gRPC error code"),
        expected_error_code
    );
    assert_eq!(
        status
            .metadata()
            .get("x-error-retryable")
            .expect("retryability metadata")
            .to_str()
            .expect("ASCII retryability metadata"),
        "false"
    );
    assert_safe_text(status.message());
    assert_safe_text(&format!("{:?}", status.metadata()));
}

fn assert_safe_text(value: &str) {
    for forbidden in [
        SECRET_MARKER,
        "internal_reference",
        "provider_response_body",
        "authorization-denied-profile",
        "crm-api-process-profile",
        "credential_handle",
    ] {
        assert!(
            !value.contains(forbidden),
            "safe transport surface leaked {forbidden}: {value}"
        );
    }
}
