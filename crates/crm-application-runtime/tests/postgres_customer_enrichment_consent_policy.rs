#[path = "support/customer_enrichment_consent_policy.rs"]
mod consent_fixture;
#[path = "support/customer_enrichment_suggestion_get.rs"]
mod customer_enrichment_suggestion_get;

use consent_fixture::{
    ConsentFixtureKind, DefinitionFixture, LEGAL_BASIS, PARTY_ID, PURPOSE, seed_consent,
    seed_definitions, seed_party,
};
use crm_application_runtime::{
    PostgresModuleActivation, ProductionCompositionDependencies, build_production_composition,
};
use crm_capability_adapters::{
    LiveAuthorizationStore, LiveCapabilityAuthorizer, LiveQueryVisibilityAuthorizer,
    LiveQueryVisibilityStore,
};
use crm_capability_ingress::{
    AccessTokenGrant, AccessTokenStore, BearerTokenAuthenticator, CapabilityIngress,
    ExecutionContextResolver, HttpCapabilityMiddleware, TimeoutPolicy,
};
use crm_capability_runtime::{
    ApprovalEvidence, CapabilityApprovalVerifier, CapabilityDefinition, CapabilityGateway,
    CapabilityRateLimiter, CapabilityRequest, RateLimitDecision,
};
use crm_consents_capability_adapter::RECORD_TYPE as CONSENT_RECORD_TYPE;
use crm_consents_query_adapter::{
    GET_CAPABILITY as CONSENT_GET_CAPABILITY,
    query_capability_definition as consent_query_definition,
};
use crm_core_data::PostgresDataStore;
use crm_customer_enrichment_capability_adapter::{MODULE_ID, request_create_capability_definition};
use crm_customer_enrichment_capability_composition::REQUEST_POLICY_VERSION;
use crm_module_sdk::testing::{DeterministicRandom, FixedClock};
use crm_module_sdk::{Clock, PortFuture, SdkError};
use crm_parties_query_adapter::{
    GET_CAPABILITY as PARTY_GET_CAPABILITY, query_capability_definition as party_query_definition,
};
use crm_proto_contracts::crm::{customer::v1 as customer_wire, customer_enrichment::v1 as wire};
use http::StatusCode;
use sqlx::PgPool;
use std::collections::BTreeSet;
use std::sync::Arc;

use customer_enrichment_suggestion_get::*;

#[tokio::test(flavor = "current_thread")]
async fn production_request_creation_denies_invalid_consent_before_persistence() {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!("skipping Customer Enrichment Consent policy because DATABASE_URL is absent");
        return;
    };
    let admin_database_url = std::env::var("ADMIN_DATABASE_URL")
        .expect("ADMIN_DATABASE_URL must accompany DATABASE_URL");
    let store = PostgresDataStore::connect(&database_url, 6)
        .await
        .expect("connect Consent policy store");
    let admin = PgPool::connect(&admin_database_url)
        .await
        .expect("connect Consent policy evidence reader");

    seed_party(&store).await.expect("seed governed Party");
    let definitions = seed_definitions(&store)
        .await
        .expect("seed immutable profile and mapping");
    let valid_consent = seed_consent(&store, ConsentFixtureKind::Valid)
        .await
        .expect("seed valid Consent");
    let invalid_consents = [
        ConsentFixtureKind::WrongParty,
        ConsentFixtureKind::WrongPurpose,
        ConsentFixtureKind::WrongLegalBasis,
        ConsentFixtureKind::Deny,
        ConsentFixtureKind::Withdrawn,
        ConsentFixtureKind::NotYetEffective,
        ConsentFixtureKind::Expired,
    ];
    for kind in invalid_consents {
        seed_consent(&store, kind)
            .await
            .unwrap_or_else(|error| panic!("seed {} Consent: {error}", kind.suffix()));
    }
    activate_customer_enrichment(&store).await;

    let request_definition =
        request_create_capability_definition().expect("valid request-create definition");
    let party_definition =
        party_query_definition(PARTY_GET_CAPABILITY).expect("valid Party get definition");
    let consent_definition =
        consent_query_definition(CONSENT_GET_CAPABILITY).expect("valid Consent get definition");
    let clock: Arc<dyn Clock> = Arc::new(FixedClock::new(NOW));
    let authorization_store = LiveAuthorizationStore::default();
    let request_authorization = authorization_grant(&request_definition);
    let party_authorization = authorization_grant(&party_definition);
    let consent_authorization = authorization_grant(&consent_definition);
    for grant in [
        request_authorization,
        party_authorization,
        consent_authorization.clone(),
    ] {
        authorization_store
            .upsert(grant)
            .expect("grant Consent policy authorization");
    }

    let visibility_store = LiveQueryVisibilityStore::default();
    for grant in [
        visibility_grant(
            &party_definition,
            PARTY_RECORD_TYPE,
            BTreeSet::from(["display_name".to_owned()]),
        ),
        visibility_grant(&consent_definition, CONSENT_RECORD_TYPE, consent_fields()),
    ] {
        visibility_store
            .upsert(grant)
            .expect("grant Consent policy visibility");
    }

    let live_authorizer = Arc::new(LiveCapabilityAuthorizer::new(
        authorization_store.clone(),
        Arc::clone(&clock),
    ));
    let composition = build_production_composition(ProductionCompositionDependencies {
        store: store.clone(),
        activation: Arc::new(PostgresModuleActivation::new(store)),
        capability_authorizer: live_authorizer.clone(),
        query_authorizer: live_authorizer,
        visibility_authorizer: Arc::new(LiveQueryVisibilityAuthorizer::new(
            visibility_store,
            Arc::clone(&clock),
        )),
        cursor_key: [0x51; 32],
    })
    .expect("assemble production Consent policy composition");
    assert!(composition.mutation_definitions().iter().any(|candidate| {
        candidate.capability_id == request_definition.capability_id
            && candidate.capability_version == request_definition.capability_version
            && candidate.owner_module_id.as_str() == MODULE_ID
    }));

    let capability_gateway = Arc::new(CapabilityGateway::new(
        composition.mutation_registry(),
        composition.mutation_validator(),
        Arc::new(AllowRateLimiter),
        Arc::new(NoopApprovalVerifier),
        Arc::new(LiveCapabilityAuthorizer::new(
            authorization_store.clone(),
            Arc::clone(&clock),
        )),
        composition.mutation_executor(),
        Arc::clone(&clock),
    ));
    let token_store = AccessTokenStore::default();
    token_store
        .issue(
            access_token().as_bytes(),
            AccessTokenGrant {
                actor_id: actor(),
                tenant_ids: BTreeSet::from([tenant(TENANT)]),
                authentication_id: "consent-policy-session".to_owned(),
                expires_at_unix_nanos: NOW + 10_000_000_000_000,
            },
        )
        .expect("issue Consent policy access token");
    let mutation_http = HttpCapabilityMiddleware::new(CapabilityIngress::new(
        Arc::new(BearerTokenAuthenticator::new(
            token_store,
            Arc::clone(&clock),
        )),
        ExecutionContextResolver::new(
            Arc::clone(&clock),
            Arc::new(DeterministicRandom::from_bytes(
                (0_u8..=255).cycle().take(8_192),
            )),
            TimeoutPolicy {
                default_millis: 5_000,
                maximum_millis: 30_000,
            },
        )
        .expect("valid Consent policy context resolver"),
        capability_gateway,
    ));

    let baseline = evidence_counts(&admin).await;
    assert_eq!(request_record_count(&admin).await, 0);

    let cases = [
        ("missing", None),
        (
            ConsentFixtureKind::WrongParty.suffix(),
            Some(ConsentFixtureKind::WrongParty.authorization_id()),
        ),
        (
            ConsentFixtureKind::WrongPurpose.suffix(),
            Some(ConsentFixtureKind::WrongPurpose.authorization_id()),
        ),
        (
            ConsentFixtureKind::WrongLegalBasis.suffix(),
            Some(ConsentFixtureKind::WrongLegalBasis.authorization_id()),
        ),
        (
            ConsentFixtureKind::Deny.suffix(),
            Some(ConsentFixtureKind::Deny.authorization_id()),
        ),
        (
            ConsentFixtureKind::Withdrawn.suffix(),
            Some(ConsentFixtureKind::Withdrawn.authorization_id()),
        ),
        (
            ConsentFixtureKind::NotYetEffective.suffix(),
            Some(ConsentFixtureKind::NotYetEffective.authorization_id()),
        ),
        (
            ConsentFixtureKind::Expired.suffix(),
            Some(ConsentFixtureKind::Expired.authorization_id()),
        ),
    ];
    for (suffix, consent_reference) in cases {
        let denied = execute_mutation(
            &mutation_http,
            &request_definition,
            &request_command(&definitions, consent_reference),
            TENANT,
            Box::leak(format!("consent-policy-{suffix}").into_boxed_str()),
        )
        .await;
        assert_eq!(denied.status, StatusCode::FORBIDDEN, "case {suffix}");
        assert_mutation_error_code(denied.body, "CUSTOMER_ENRICHMENT_REQUEST_CONSENT_DENIED");
        assert_eq!(evidence_counts(&admin).await, baseline, "case {suffix}");
        assert_eq!(request_record_count(&admin).await, 0, "case {suffix}");
    }

    authorization_store
        .revoke(
            &consent_authorization.tenant_id,
            &consent_authorization.actor_id,
            &consent_authorization.policy_id,
        )
        .expect("revoke governed Consent query authorization");
    let permission_denied = execute_mutation(
        &mutation_http,
        &request_definition,
        &request_command(&definitions, Some(valid_consent)),
        TENANT,
        "consent-policy-query-permission-denied",
    )
    .await;
    assert_eq!(permission_denied.status, StatusCode::FORBIDDEN);
    assert_mutation_error_code(
        permission_denied.body,
        "CUSTOMER_ENRICHMENT_CONSENT_PERMISSION_DENIED",
    );
    assert_eq!(evidence_counts(&admin).await, baseline);
    assert_eq!(request_record_count(&admin).await, 0);
}

fn request_command(
    definitions: &DefinitionFixture,
    consent_reference: Option<String>,
) -> wire::CreateEnrichmentRequestRequest {
    let now_ms = NOW / 1_000_000;
    wire::CreateEnrichmentRequestRequest {
        target: Some(wire::EnrichmentTargetSnapshot {
            party_ref: Some(customer_wire::PartyRef {
                party_id: PARTY_ID.to_owned(),
            }),
            party_resource_version: 1,
            target_field: wire::EnrichmentTargetField::PartyDisplayName as i32,
        }),
        provider_profile_version_ref: Some(wire::ProviderProfileVersionRef {
            provider_profile_version_id: definitions.profile.version_id().as_str().to_owned(),
        }),
        mapping_version_ref: Some(wire::MappingVersionRef {
            mapping_version_id: definitions.mapping.version_id().as_str().to_owned(),
        }),
        requested_fields: vec![wire::EnrichmentTargetField::PartyDisplayName as i32],
        policy_evidence: Some(wire::EnrichmentRequestPolicyEvidence {
            purpose_code: PURPOSE.to_owned(),
            legal_basis_code: LEGAL_BASIS.to_owned(),
            consent_evidence_reference: consent_reference,
            policy_version: REQUEST_POLICY_VERSION.to_owned(),
        }),
        deadline_at_unix_ms: now_ms + 100_000,
        expires_at_unix_ms: now_ms + 200_000,
    }
}

fn consent_fields() -> BTreeSet<String> {
    [
        "party_ref",
        "contact_point_ref",
        "purpose",
        "channel",
        "effect",
        "legal_basis",
        "jurisdiction",
        "source",
        "evidence_ref",
        "validity",
        "status",
        "resource_version",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}

async fn request_record_count(pool: &PgPool) -> i64 {
    sqlx::query_scalar(
        "SELECT count(*) FROM crm.records WHERE tenant_id = $1 AND owner_module_id = $2 AND record_type = 'customer_enrichment.request'",
    )
    .bind(TENANT)
    .bind(MODULE_ID)
    .fetch_one(pool)
    .await
    .expect("read enrichment-request record count")
}

#[derive(Debug, Clone, Copy)]
struct AllowRateLimiter;

impl CapabilityRateLimiter for AllowRateLimiter {
    fn check<'a>(
        &'a self,
        _definition: &'a CapabilityDefinition,
        _request: &'a CapabilityRequest,
    ) -> PortFuture<'a, Result<RateLimitDecision, SdkError>> {
        Box::pin(async {
            Ok(RateLimitDecision {
                allowed: true,
                decision_id: "consent-policy-rate-allowed".to_owned(),
                retry_after_millis: None,
            })
        })
    }
}

#[derive(Debug, Clone, Copy)]
struct NoopApprovalVerifier;

impl CapabilityApprovalVerifier for NoopApprovalVerifier {
    fn verify<'a>(
        &'a self,
        _definition: &'a CapabilityDefinition,
        _request: &'a CapabilityRequest,
        _approval: &'a ApprovalEvidence,
    ) -> PortFuture<'a, Result<(), SdkError>> {
        Box::pin(async { Ok(()) })
    }
}
