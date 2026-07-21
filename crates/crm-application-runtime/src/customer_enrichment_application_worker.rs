use crm_capability_ingress::semantic_input_hash;
use crm_capability_plan_support as support;
use crm_capability_runtime::{CapabilityAuthorizer, CapabilityRequest};
use crm_consents_capability_adapter::MODULE_ID as CONSENTS_MODULE_ID;
use crm_consents_query_adapter::{
    ConsentQueryAdapter, GET_CAPABILITY as CONSENT_GET_CAPABILITY,
    GET_REQUEST_SCHEMA as CONSENT_GET_REQUEST_SCHEMA,
    query_capability_definition as consent_query_definition,
};
use crm_core_data::PostgresDataStore;
use crm_customer_enrichment::{
    EnrichmentPolicyDecision, EnrichmentPolicyPort, EnrichmentPolicyRequest,
    PartyDisplayNameApplicationPort, PartyDisplayNameApplicationRequest,
    PartyDisplayNameApplicationResult, PartySnapshot, PartySnapshotPort, PartySnapshotRequest,
    PolicyEvaluationPhase, TargetField,
};
use crm_customer_enrichment_application_adapter::{
    RECORD_APPLICATION_OUTCOME_REQUEST_SCHEMA, record_application_outcome_capability_definition,
};
use crm_customer_enrichment_application_composition::{
    CustomerEnrichmentPartyApplicationOrchestrator, CustomerEnrichmentPartyApplicationWorker,
    GatewayPartyDisplayNameApplicationPort,
};
use crm_customer_enrichment_capability_adapter::MODULE_ID;
use crm_module_sdk::{
    ActorId, BusinessTransactionId, CapabilityClient, CapabilityId, CapabilityVersion, CausationId,
    Clock, CorrelationId, DataClass, ErrorCategory, ExecutionContext, IdempotencyKey,
    ModuleExecutionContext, ModuleId, PortFuture, RecordId, RequestId, SchemaVersion, SdkError,
    TenantId, TraceId,
};
use crm_parties_query_adapter::{
    GET_CAPABILITY as PARTY_GET_CAPABILITY, PartyQueryAdapter, export_execution_query_request,
    query_capability_definition as party_query_definition,
};
use crm_proto_contracts::crm::{
    consents::v1 as consent_wire, customer_enrichment::v1 as enrichment_wire,
    parties::v1 as party_wire,
};
use crm_query_runtime::{
    CursorCodec, QueryAuthorizer, QueryExecutionContext, QueryExecutor, QueryRequest,
    QuerySemanticValidator, QueryVisibilityAuthorizer, normalized_filter_hash,
};
use prost::Message;
use sha2::{Digest, Sha256};
use std::fmt;
use std::sync::Arc;

pub const OWNER_APPLICATION_POLICY_VERSION: &str = "owner-application-policy-v1";
const CONSENT_LEGAL_BASIS_CODE: &str = "consent";
const LEGITIMATE_INTEREST_LEGAL_BASIS_CODE: &str = "legitimate_interest";

pub struct CustomerEnrichmentApplicationWorkerDependencies {
    pub store: PostgresDataStore,
    pub capabilities: Arc<dyn CapabilityClient>,
    pub capability_authorizer: Arc<dyn CapabilityAuthorizer>,
    pub query_authorizer: Arc<dyn QueryAuthorizer>,
    pub visibility_authorizer: Arc<dyn QueryVisibilityAuthorizer>,
    pub clock: Arc<dyn Clock>,
    pub cursor_key: [u8; 32],
    pub actor_id: ActorId,
}

impl fmt::Debug for CustomerEnrichmentApplicationWorkerDependencies {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CustomerEnrichmentApplicationWorkerDependencies")
            .field("store", &self.store)
            .field("capabilities", &"dyn CapabilityClient")
            .field("capability_authorizer", &"dyn CapabilityAuthorizer")
            .field("query_authorizer", &"dyn QueryAuthorizer")
            .field("visibility_authorizer", &"dyn QueryVisibilityAuthorizer")
            .field("clock", &"dyn Clock")
            .field("actor_id", &self.actor_id)
            .finish_non_exhaustive()
    }
}

pub fn build_customer_enrichment_application_worker(
    dependencies: CustomerEnrichmentApplicationWorkerDependencies,
) -> Result<Arc<CustomerEnrichmentPartyApplicationWorker>, SdkError> {
    let party_queries = Arc::new(PartyQueryAdapter::new(
        dependencies.store.clone(),
        CursorCodec::new(dependencies.cursor_key).map_err(configuration_error)?,
        dependencies.visibility_authorizer.clone(),
    )?);
    let consent_queries = Arc::new(ConsentQueryAdapter::new(
        dependencies.store.clone(),
        CursorCodec::new(dependencies.cursor_key).map_err(configuration_error)?,
        dependencies.visibility_authorizer,
    )?);
    let party_snapshots: Arc<dyn PartySnapshotPort> = Arc::new(GovernedPartySnapshotPort::new(
        party_queries.clone(),
        dependencies.query_authorizer.clone(),
    ));
    let outcome_authorization = Arc::new(ProductionApplicationOutcomeAuthorization::new(
        dependencies.capability_authorizer,
    ));
    let policy: Arc<dyn EnrichmentPolicyPort> = Arc::new(OutcomeAuthorizingPolicy::new(
        Arc::new(ProductionOwnerApplicationPolicy::new(
            party_queries,
            consent_queries,
            dependencies.query_authorizer,
        )),
        outcome_authorization.clone(),
    ));
    let owner: Arc<dyn PartyDisplayNameApplicationPort> =
        Arc::new(OutcomeAuthorizingPartyDisplayNameApplicationPort::new(
            Arc::new(GatewayPartyDisplayNameApplicationPort::new(
                dependencies.capabilities,
                party_snapshots,
                dependencies.clock.clone(),
            )?),
            outcome_authorization,
            dependencies.clock.clone(),
        ));
    let orchestrator = Arc::new(CustomerEnrichmentPartyApplicationOrchestrator::postgres(
        dependencies.store.clone(),
        policy,
        owner,
        dependencies.clock,
    )?);
    Ok(Arc::new(CustomerEnrichmentPartyApplicationWorker::new(
        dependencies.store,
        orchestrator,
        dependencies.actor_id,
    )?))
}

#[derive(Clone)]
struct ProductionApplicationOutcomeAuthorization {
    authorizer: Arc<dyn CapabilityAuthorizer>,
}

impl ProductionApplicationOutcomeAuthorization {
    fn new(authorizer: Arc<dyn CapabilityAuthorizer>) -> Self {
        Self { authorizer }
    }

    async fn authorize(
        &self,
        tenant_id: &TenantId,
        actor_id: &ActorId,
        application_attempt_id: &str,
        causation_identity: &str,
        decided_at_unix_nanos: i64,
    ) -> Result<(), SdkError> {
        if decided_at_unix_nanos <= 0 {
            return Err(configuration_invalid(
                "application outcome authorization time is invalid",
            ));
        }
        let definition = record_application_outcome_capability_definition()?;
        let recorded_at_unix_ms = decided_at_unix_nanos / 1_000_000;
        let input = support::protobuf_payload(
            MODULE_ID,
            RECORD_APPLICATION_OUTCOME_REQUEST_SCHEMA,
            DataClass::Personal,
            &enrichment_wire::RecordApplicationOutcomeRequest {
                application_attempt_ref: Some(enrichment_wire::ApplicationAttemptRef {
                    application_attempt_id: application_attempt_id.to_owned(),
                }),
                outcome: None,
                recorded_at_unix_ms,
            },
        )?;
        let identity = outcome_authorization_identity(
            tenant_id,
            actor_id,
            application_attempt_id,
            causation_identity,
        );
        let request = CapabilityRequest {
            context: ModuleExecutionContext {
                module_id: ModuleId::try_new(MODULE_ID).map_err(configuration_error)?,
                execution: ExecutionContext {
                    tenant_id: tenant_id.clone(),
                    actor_id: actor_id.clone(),
                    request_id: RequestId::try_new(identity.clone())
                        .map_err(configuration_error)?,
                    correlation_id: CorrelationId::try_new(identity.clone())
                        .map_err(configuration_error)?,
                    causation_id: CausationId::try_new(identity.clone())
                        .map_err(configuration_error)?,
                    trace_id: TraceId::try_new(identity.clone()).map_err(configuration_error)?,
                    capability_id: definition.capability_id.clone(),
                    capability_version: definition.capability_version.clone(),
                    idempotency_key: IdempotencyKey::try_new(identity.clone())
                        .map_err(configuration_error)?,
                    business_transaction_id: BusinessTransactionId::try_new(identity)
                        .map_err(configuration_error)?,
                    schema_version: SchemaVersion::try_new(support::CONTRACT_VERSION)
                        .map_err(configuration_error)?,
                    request_started_at_unix_nanos: decided_at_unix_nanos,
                },
            },
            input_hash: semantic_input_hash(&input),
            input,
            approval: None,
        };
        let decision = self.authorizer.authorize(&definition, &request).await?;
        if decision.allowed {
            return Ok(());
        }
        Err(SdkError::new(
            "CUSTOMER_ENRICHMENT_APPLICATION_OUTCOME_PERMISSION_DENIED",
            ErrorCategory::Authorization,
            false,
            "The application worker is not authorized to persist the application outcome.",
        )
        .with_internal_reference(format!(
            "decision_id={};reason_code={};policy_version={}",
            decision.decision_id, decision.reason_code, decision.policy_version
        )))
    }
}

impl fmt::Debug for ProductionApplicationOutcomeAuthorization {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProductionApplicationOutcomeAuthorization")
            .field("authorizer", &"dyn CapabilityAuthorizer")
            .finish()
    }
}

#[derive(Clone)]
struct OutcomeAuthorizingPolicy {
    inner: Arc<dyn EnrichmentPolicyPort>,
    outcome_authorization: Arc<ProductionApplicationOutcomeAuthorization>,
}

impl OutcomeAuthorizingPolicy {
    fn new(
        inner: Arc<dyn EnrichmentPolicyPort>,
        outcome_authorization: Arc<ProductionApplicationOutcomeAuthorization>,
    ) -> Self {
        Self {
            inner,
            outcome_authorization,
        }
    }
}

impl fmt::Debug for OutcomeAuthorizingPolicy {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OutcomeAuthorizingPolicy")
            .field("inner", &"dyn EnrichmentPolicyPort")
            .field("outcome_authorization", &self.outcome_authorization)
            .finish()
    }
}

impl EnrichmentPolicyPort for OutcomeAuthorizingPolicy {
    fn evaluate<'a>(
        &'a self,
        request: EnrichmentPolicyRequest,
    ) -> PortFuture<'a, Result<EnrichmentPolicyDecision, SdkError>> {
        Box::pin(async move {
            let decision = self.inner.evaluate(request.clone()).await?;
            if matches!(decision, EnrichmentPolicyDecision::Denied { .. }) {
                let decided_at_unix_nanos = request
                    .evaluated_at_unix_ms
                    .checked_mul(1_000_000)
                    .ok_or_else(|| configuration_invalid("outcome policy time overflow"))?;
                self.outcome_authorization
                    .authorize(
                        &request.tenant_id,
                        &request.actor_id,
                        &request.request_identity,
                        decision.decision_id(),
                        decided_at_unix_nanos,
                    )
                    .await?;
            }
            Ok(decision)
        })
    }
}

#[derive(Clone)]
struct OutcomeAuthorizingPartyDisplayNameApplicationPort {
    inner: Arc<dyn PartyDisplayNameApplicationPort>,
    outcome_authorization: Arc<ProductionApplicationOutcomeAuthorization>,
    clock: Arc<dyn Clock>,
}

impl OutcomeAuthorizingPartyDisplayNameApplicationPort {
    fn new(
        inner: Arc<dyn PartyDisplayNameApplicationPort>,
        outcome_authorization: Arc<ProductionApplicationOutcomeAuthorization>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            inner,
            outcome_authorization,
            clock,
        }
    }
}

impl fmt::Debug for OutcomeAuthorizingPartyDisplayNameApplicationPort {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OutcomeAuthorizingPartyDisplayNameApplicationPort")
            .field("inner", &"dyn PartyDisplayNameApplicationPort")
            .field("outcome_authorization", &self.outcome_authorization)
            .field("clock", &"dyn Clock")
            .finish()
    }
}

impl PartyDisplayNameApplicationPort for OutcomeAuthorizingPartyDisplayNameApplicationPort {
    fn apply<'a>(
        &'a self,
        request: PartyDisplayNameApplicationRequest,
    ) -> PortFuture<'a, Result<PartyDisplayNameApplicationResult, SdkError>> {
        Box::pin(async move {
            let result = self.inner.apply(request.clone()).await?;
            self.outcome_authorization
                .authorize(
                    &request.tenant_id,
                    &request.actor_id,
                    request.application_attempt_id.as_str(),
                    &request.final_authorization_decision_id,
                    self.clock.now_unix_nanos(),
                )
                .await?;
            Ok(result)
        })
    }
}

fn outcome_authorization_identity(
    tenant_id: &TenantId,
    actor_id: &ActorId,
    application_attempt_id: &str,
    causation_identity: &str,
) -> String {
    let mut hasher = Sha256::new();
    for value in [
        "crm.customer-enrichment.application-outcome-authorization/v1",
        tenant_id.as_str(),
        actor_id.as_str(),
        application_attempt_id,
        causation_identity,
    ] {
        hasher.update((value.len() as u64).to_be_bytes());
        hasher.update(value.as_bytes());
    }
    let digest = hasher.finalize();
    let mut encoded = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write as _;
        let _ = write!(encoded, "{byte:02x}");
    }
    format!("application-outcome-authorization-{encoded}")
}

#[derive(Clone)]
struct GovernedPartySnapshotPort {
    party_queries: Arc<PartyQueryAdapter>,
    query_authorizer: Arc<dyn QueryAuthorizer>,
}

impl GovernedPartySnapshotPort {
    fn new(
        party_queries: Arc<PartyQueryAdapter>,
        query_authorizer: Arc<dyn QueryAuthorizer>,
    ) -> Self {
        Self {
            party_queries,
            query_authorizer,
        }
    }
}

impl fmt::Debug for GovernedPartySnapshotPort {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GovernedPartySnapshotPort")
            .field("party_queries", &"PartyQueryAdapter")
            .field("query_authorizer", &"dyn QueryAuthorizer")
            .finish()
    }
}

impl PartySnapshotPort for GovernedPartySnapshotPort {
    fn get<'a>(
        &'a self,
        request: PartySnapshotRequest,
    ) -> PortFuture<'a, Result<PartySnapshot, SdkError>> {
        Box::pin(async move {
            let requested_at_unix_nanos = request
                .requested_at_unix_ms
                .checked_mul(1_000_000)
                .ok_or_else(|| configuration_invalid("Party snapshot timestamp overflow"))?;
            let query = export_execution_query_request(
                &request.tenant_id,
                &request.actor_id,
                &request.request_identity,
                &request.party_id,
                requested_at_unix_nanos,
            )?;
            let definition = party_query_definition(PARTY_GET_CAPABILITY)?;
            authorize_query(
                self.query_authorizer.as_ref(),
                &definition,
                &query,
                "CUSTOMER_ENRICHMENT_APPLICATION_PARTY_PERMISSION_DENIED",
            )
            .await?;
            self.party_queries.validate(&definition, &query).await?;
            let result = self.party_queries.execute(&definition, query).await?;
            let response = party_wire::GetPartyResponse::decode(result.output.bytes.as_slice())
                .map_err(|error| {
                    party_snapshot_invalid().with_internal_reference(error.to_string())
                })?;
            let party = response.party.ok_or_else(party_snapshot_unavailable)?;
            let party_id = party.party_ref.ok_or_else(party_snapshot_invalid)?.party_id;
            let resource_version = party
                .resource_version
                .ok_or_else(party_snapshot_invalid)?
                .version;
            let party_id = RecordId::try_new(party_id).map_err(configuration_error)?;
            if party_id != request.party_id
                || resource_version <= 0
                || party.display_name.is_empty()
            {
                return Err(party_snapshot_invalid());
            }
            Ok(PartySnapshot {
                party_id,
                display_name: party.display_name,
                resource_version,
                observed_at_unix_ms: request.requested_at_unix_ms,
            })
        })
    }
}

#[derive(Clone)]
struct ProductionOwnerApplicationPolicy {
    party_queries: Arc<PartyQueryAdapter>,
    consent_queries: Arc<ConsentQueryAdapter>,
    query_authorizer: Arc<dyn QueryAuthorizer>,
}

impl ProductionOwnerApplicationPolicy {
    fn new(
        party_queries: Arc<PartyQueryAdapter>,
        consent_queries: Arc<ConsentQueryAdapter>,
        query_authorizer: Arc<dyn QueryAuthorizer>,
    ) -> Self {
        Self {
            party_queries,
            consent_queries,
            query_authorizer,
        }
    }

    async fn validate_party(&self, request: &EnrichmentPolicyRequest) -> Result<(), SdkError> {
        let evaluated_at_unix_nanos = request
            .evaluated_at_unix_ms
            .checked_mul(1_000_000)
            .ok_or_else(|| policy_invalid("policy timestamp overflow"))?;
        let query = export_execution_query_request(
            &request.tenant_id,
            &request.actor_id,
            &request.request_identity,
            &request.party_id,
            evaluated_at_unix_nanos,
        )?;
        let definition = party_query_definition(PARTY_GET_CAPABILITY)?;
        authorize_query(
            self.query_authorizer.as_ref(),
            &definition,
            &query,
            "CUSTOMER_ENRICHMENT_APPLICATION_PARTY_PERMISSION_DENIED",
        )
        .await?;
        self.party_queries.validate(&definition, &query).await?;
        let result = self.party_queries.execute(&definition, query).await?;
        let response = party_wire::GetPartyResponse::decode(result.output.bytes.as_slice())
            .map_err(|error| policy_invalid(error.to_string()))?;
        let party = response
            .party
            .ok_or_else(|| policy_denied_error("party_unavailable"))?;
        if party
            .party_ref
            .as_ref()
            .is_none_or(|value| value.party_id != request.party_id.as_str())
            || party.display_name.is_empty()
        {
            return Err(policy_denied_error("party_not_visible"));
        }
        Ok(())
    }

    async fn validate_consent(&self, request: &EnrichmentPolicyRequest) -> Result<(), SdkError> {
        let Some(authorization_id) = request.consent_evidence_reference.as_deref() else {
            return Err(policy_denied_error("consent_evidence_required"));
        };
        let query = consent_get_query_request(request, authorization_id)?;
        let definition = consent_query_definition(CONSENT_GET_CAPABILITY)?;
        authorize_query(
            self.query_authorizer.as_ref(),
            &definition,
            &query,
            "CUSTOMER_ENRICHMENT_APPLICATION_CONSENT_PERMISSION_DENIED",
        )
        .await?;
        self.consent_queries.validate(&definition, &query).await?;
        let result = self.consent_queries.execute(&definition, query).await?;
        let response =
            consent_wire::GetConsentAuthorizationResponse::decode(result.output.bytes.as_slice())
                .map_err(|error| policy_invalid(error.to_string()))?;
        let authorization = response
            .authorization
            .ok_or_else(|| policy_denied_error("consent_authorization_missing"))?;
        validate_consent_authorization(&authorization, authorization_id, request)
    }
}

impl fmt::Debug for ProductionOwnerApplicationPolicy {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProductionOwnerApplicationPolicy")
            .field("party_queries", &"PartyQueryAdapter")
            .field("consent_queries", &"ConsentQueryAdapter")
            .field("query_authorizer", &"dyn QueryAuthorizer")
            .finish()
    }
}

impl EnrichmentPolicyPort for ProductionOwnerApplicationPolicy {
    fn evaluate<'a>(
        &'a self,
        request: EnrichmentPolicyRequest,
    ) -> PortFuture<'a, Result<EnrichmentPolicyDecision, SdkError>> {
        Box::pin(async move {
            if request.phase != PolicyEvaluationPhase::OwnerApplication
                || request.target_field != TargetField::PartyDisplayName
                || !canonical_token(&request.purpose_code)
                || !canonical_token(&request.legal_basis_code)
                || request.evaluated_at_unix_ms < 0
            {
                return Err(policy_invalid("owner application policy input is invalid"));
            }
            self.validate_party(&request).await?;
            match request.legal_basis_code.as_str() {
                CONSENT_LEGAL_BASIS_CODE => self.validate_consent(&request).await?,
                LEGITIMATE_INTEREST_LEGAL_BASIS_CODE => {}
                _ => return Ok(policy_denied(&request, "legal_basis_not_permitted")),
            }
            Ok(EnrichmentPolicyDecision::Allowed {
                decision_id: policy_decision_id(&request, "allowed"),
                policy_version: OWNER_APPLICATION_POLICY_VERSION.to_owned(),
            })
        })
    }
}

async fn authorize_query(
    authorizer: &dyn QueryAuthorizer,
    definition: &crm_capability_runtime::CapabilityDefinition,
    request: &QueryRequest,
    error_code: &'static str,
) -> Result<(), SdkError> {
    let decision = authorizer.authorize(definition, request).await?;
    if decision.allowed {
        return Ok(());
    }
    Err(SdkError::new(
        error_code,
        ErrorCategory::Authorization,
        false,
        "The application worker is not authorized to inspect required policy evidence.",
    )
    .with_internal_reference(format!(
        "decision_id={};reason_code={};policy_version={}",
        decision.decision_id, decision.reason_code, decision.policy_version
    )))
}

fn consent_get_query_request(
    request: &EnrichmentPolicyRequest,
    authorization_id: &str,
) -> Result<QueryRequest, SdkError> {
    let authorization_id = RecordId::try_new(authorization_id)
        .map_err(|_| policy_denied_error("consent_reference_invalid"))?;
    let command = consent_wire::GetConsentAuthorizationRequest {
        authorization_ref: Some(consent_wire::ConsentAuthorizationRef {
            authorization_id: authorization_id.as_str().to_owned(),
        }),
    };
    let input = support::protobuf_payload(
        CONSENTS_MODULE_ID,
        CONSENT_GET_REQUEST_SCHEMA,
        DataClass::Personal,
        &command,
    )?;
    let request_id = format!("application-consent-{}", request.request_identity);
    Ok(QueryRequest {
        owner_module_id: ModuleId::try_new(CONSENTS_MODULE_ID).map_err(configuration_error)?,
        context: QueryExecutionContext {
            tenant_id: request.tenant_id.clone(),
            actor_id: request.actor_id.clone(),
            request_id: crm_module_sdk::RequestId::try_new(request_id.clone())
                .map_err(configuration_error)?,
            correlation_id: crm_module_sdk::CorrelationId::try_new(request_id.clone())
                .map_err(configuration_error)?,
            trace_id: crm_module_sdk::TraceId::try_new(request_id).map_err(configuration_error)?,
            capability_id: CapabilityId::try_new(CONSENT_GET_CAPABILITY)
                .map_err(configuration_error)?,
            capability_version: CapabilityVersion::try_new(support::CONTRACT_VERSION)
                .map_err(configuration_error)?,
            schema_version: SchemaVersion::try_new(support::CONTRACT_VERSION)
                .map_err(configuration_error)?,
            request_started_at_unix_nanos: request
                .evaluated_at_unix_ms
                .checked_mul(1_000_000)
                .ok_or_else(|| policy_invalid("consent policy timestamp overflow"))?,
        },
        input,
        input_hash: normalized_filter_hash([(
            "authorization_id",
            authorization_id.as_str().as_bytes(),
        )]),
    })
}

fn validate_consent_authorization(
    authorization: &consent_wire::ConsentAuthorization,
    expected_authorization_id: &str,
    request: &EnrichmentPolicyRequest,
) -> Result<(), SdkError> {
    let evaluated_at_unix_nanos = request
        .evaluated_at_unix_ms
        .checked_mul(1_000_000)
        .ok_or_else(|| policy_invalid("consent evaluation timestamp overflow"))?;
    let identity_matches = authorization
        .authorization_ref
        .as_ref()
        .is_some_and(|value| value.authorization_id == expected_authorization_id);
    let party_matches = authorization
        .party_ref
        .as_ref()
        .is_some_and(|value| value.party_id == request.party_id.as_str());
    let effect = consent_wire::ConsentEffect::try_from(authorization.effect).ok();
    let status = consent_wire::ConsentAuthorizationStatus::try_from(authorization.status).ok();
    let effective_from = authorization
        .effective_from
        .as_ref()
        .map(|value| value.unix_nanos)
        .ok_or_else(|| policy_denied_error("consent_effective_time_missing"))?;
    let not_expired = authorization
        .expires_at
        .as_ref()
        .is_none_or(|value| evaluated_at_unix_nanos < value.unix_nanos);
    if !identity_matches
        || !party_matches
        || authorization.purpose != request.purpose_code
        || authorization.legal_basis != request.legal_basis_code
        || effect != Some(consent_wire::ConsentEffect::Grant)
        || status != Some(consent_wire::ConsentAuthorizationStatus::Active)
        || effective_from > evaluated_at_unix_nanos
        || !not_expired
        || authorization.evidence_ref.is_empty()
    {
        return Err(policy_denied_error("consent_evidence_not_applicable"));
    }
    Ok(())
}

fn policy_denied(request: &EnrichmentPolicyRequest, reason: &str) -> EnrichmentPolicyDecision {
    EnrichmentPolicyDecision::Denied {
        decision_id: policy_decision_id(request, reason),
        policy_version: OWNER_APPLICATION_POLICY_VERSION.to_owned(),
        safe_reason_code: reason.to_owned(),
    }
}

fn policy_decision_id(request: &EnrichmentPolicyRequest, evidence: &str) -> String {
    let mut hasher = Sha256::new();
    for value in [
        "crm.customer-enrichment.owner-application-policy/v1",
        request.tenant_id.as_str(),
        request.actor_id.as_str(),
        request.request_identity.as_str(),
        request.enrichment_request_id.as_str(),
        request.party_id.as_str(),
        request.purpose_code.as_str(),
        request.legal_basis_code.as_str(),
        request.consent_evidence_reference.as_deref().unwrap_or(""),
        evidence,
    ] {
        hasher.update((value.len() as u64).to_be_bytes());
        hasher.update(value.as_bytes());
    }
    let digest = hasher.finalize();
    let mut encoded = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write as _;
        let _ = write!(encoded, "{byte:02x}");
    }
    format!("owner-application-policy-{encoded}")
}

fn canonical_token(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 120
        && value.trim() == value
        && !value.chars().any(char::is_control)
}

fn configuration_error(error: impl fmt::Display) -> SdkError {
    configuration_invalid(error.to_string())
}

fn configuration_invalid(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_APPLICATION_CONFIGURATION_INVALID",
        ErrorCategory::Internal,
        false,
        "The Customer Enrichment application worker is not configured safely.",
    )
    .with_internal_reference(reference.into())
}

fn party_snapshot_unavailable() -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_APPLICATION_PARTY_UNAVAILABLE",
        ErrorCategory::NotFound,
        false,
        "The target Party is unavailable.",
    )
}

fn party_snapshot_invalid() -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_APPLICATION_PARTY_STATE_INVALID",
        ErrorCategory::Internal,
        false,
        "The target Party state is invalid.",
    )
}

fn policy_denied_error(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_APPLICATION_POLICY_DENIED",
        ErrorCategory::Authorization,
        false,
        "The accepted suggestion is not permitted for owner application.",
    )
    .with_internal_reference(reference.into())
}

fn policy_invalid(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_APPLICATION_POLICY_INVALID",
        ErrorCategory::Internal,
        false,
        "The owner application policy could not be evaluated safely.",
    )
    .with_internal_reference(reference.into())
}
