use crate::{
    ApplicationAggregatePlannerRouter, ApplicationCapabilityExecutorRouter, ApplicationConfig,
    ApplicationGatewayService, ApplicationQueryRouter, ContractBoundMutationSemanticValidator,
    ProcessIdentitySource, SystemClock, application_capability_catalog,
    application_mutation_definitions, application_query_capability_catalog,
    application_query_definitions,
};
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use crm_capability_adapters::{
    AuthorizationGrant, FixedWindowRateLimiter, HmacSha256ApprovalVerifier, LiveAuthorizationStore,
    LiveCapabilityAuthorizer, LiveQueryVisibilityAuthorizer, LiveQueryVisibilityStore,
    QueryVisibilityGrant, RateLimitPolicyStore,
};
use crm_capability_ingress::{
    AccessTokenGrant, AccessTokenStore, BearerTokenAuthenticator, CapabilityIngress,
    CapabilityRoute, ExecutionContextResolver, GrpcCapabilityMiddleware, GrpcQueryMiddleware,
    HttpCapabilityBody, HttpCapabilityMiddleware, HttpCapabilityRequest, HttpQueryBody,
    HttpQueryMiddleware, HttpQueryRequest, QueryContextResolver, QueryIngress, TimeoutPolicy,
};
use crm_capability_runtime::{ApprovalEvidence, CapabilityDefinition, CapabilityGateway};
use crm_consents_capability_adapter::{
    MODULE_ID as CONSENTS_MODULE_ID, RECORD_TYPE as CONSENT_RECORD_TYPE,
};
use crm_consents_query_adapter::ConsentQueryAdapter;
use crm_contact_points_capability_adapter::{
    MODULE_ID as CONTACT_POINTS_MODULE_ID, RECORD_TYPE as CONTACT_POINT_RECORD_TYPE,
};
use crm_contact_points_query_adapter::ContactPointQueryAdapter;
use crm_core_data::{
    PostgresDataStore, PostgresMetadataCapabilityExecutor, PostgresMetadataQueryStore,
    PostgresTransactionalAggregateExecutor,
};
use crm_core_events::EventHistoryRequest;
use crm_customer_360_composition::Customer360ProjectionWorker;
use crm_customer_360_query_adapter::{
    Customer360QueryAdapter, MODULE_ID as CUSTOMER_360_MODULE_ID,
};
use crm_customer_accounts_capability_adapter::{
    MODULE_ID as ACCOUNTS_MODULE_ID, RECORD_TYPE as ACCOUNT_RECORD_TYPE,
};
use crm_customer_accounts_query_adapter::AccountQueryAdapter;
use crm_customer_data_operations_capability_adapter::{
    IMPORT_JOB_RECORD_TYPE as CUSTOMER_DATA_IMPORT_JOB_RECORD_TYPE,
    IMPORT_ROW_RECORD_TYPE as CUSTOMER_DATA_IMPORT_ROW_RECORD_TYPE,
    MODULE_ID as CUSTOMER_DATA_OPERATIONS_MODULE_ID,
};
use crm_customer_data_operations_query_adapter::{
    CustomerDataOperationsQueryAdapter, LIST_IMPORT_ROWS_CAPABILITY,
};
use crm_global_search_composition::{GLOBAL_SEARCH_INDEX_ID, GlobalSearchWorker};
use crm_identity_resolution_capability_adapter::{
    MERGE_OPERATION_RECORD_TYPE as IDENTITY_RESOLUTION_MERGE_RECORD_TYPE,
    MODULE_ID as IDENTITY_RESOLUTION_MODULE_ID, RECORD_TYPE as IDENTITY_RESOLUTION_RECORD_TYPE,
};
use crm_identity_resolution_merge_query_adapter::IdentityResolutionMergeQueryAdapter;
use crm_identity_resolution_query_adapter::IdentityResolutionQueryAdapter;
use crm_metadata_api_adapter::METADATA_MODULE_ID;
use crm_metadata_query_adapter::MetadataQueryAdapter;
use crm_module_registry::ModuleRegistry;
use crm_module_sdk::{
    ActorId, CapabilityId, CapabilityVersion, Clock, EventType, ModuleId, RandomSource, RecordType,
    SchemaVersion, TenantId, TypedPayload,
};
use crm_parties_capability_adapter::{
    MODULE_ID as PARTIES_MODULE_ID, RECORD_TYPE as PARTY_RECORD_TYPE,
};
use crm_parties_query_adapter::PartyQueryAdapter;
use crm_party_relationships_capability_adapter::{
    MODULE_ID as PARTY_RELATIONSHIPS_MODULE_ID, RECORD_TYPE as PARTY_RELATIONSHIP_RECORD_TYPE,
};
use crm_party_relationships_query_adapter::PartyRelationshipQueryAdapter;
use crm_query_runtime::{CursorCodec, QueryGateway};
use crm_sales_activities_capability_composition::{
    DEAL_TIMELINE_PROJECTION_ID, Phase6ProjectionWorker, ProductionQueryRouter,
    SalesActivitiesLinkDeliveryOutcome, SalesActivitiesLinkEventProcessor,
    SalesActivitiesLinkEventProcessorConfig, TASK_STATUS_PROJECTION_ID,
};
use crm_sales_activities_link::MODULE_ID as LINK_MODULE_ID;
use crm_sales_activities_query_adapter::{
    ACTIVITIES_RECORD_TYPE, SALES_RECORD_TYPE, SalesActivitiesQueryAdapter,
};
use crm_search_query_adapter::{SEARCH_MODULE_ID, SearchQueryAdapter};
use crm_search_runtime::SearchIndexId;
use semver::Version;
use serde::Serialize;
use serde_json::json;
use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::sync::watch;
use tokio_stream::wrappers::TcpListenerStream;
use tonic::transport::Server;
use tonic_health::ServingStatus;

const BOOTSTRAP_POLICY_VERSION: &str = "application-bootstrap/v1";
const BOOTSTRAP_LIFETIME_NANOS: i64 = 365_i64 * 24 * 60 * 60 * 1_000_000_000;
const BACKGROUND_INTERVAL: Duration = Duration::from_secs(1);
const LINK_SCAN_PAGE_SIZE: u32 = 200;
const PROJECTION_PAGE_SIZE: u32 = 200;
const SEARCH_PAGE_SIZE: u32 = 200;

#[derive(Clone)]
pub struct ApplicationComponents {
    pub store: PostgresDataStore,
    pub module_registry: Arc<ModuleRegistry>,
    pub mutation_http: Arc<HttpCapabilityMiddleware>,
    pub mutation_grpc: Arc<GrpcCapabilityMiddleware>,
    pub query_http: Arc<HttpQueryMiddleware>,
    pub query_grpc: Arc<GrpcQueryMiddleware>,
    pub link_processor: Arc<SalesActivitiesLinkEventProcessor>,
    pub projection_worker: Arc<Phase6ProjectionWorker>,
    pub customer_360_worker: Arc<Customer360ProjectionWorker>,
    pub search_worker: Arc<GlobalSearchWorker>,
    readiness: Arc<AtomicBool>,
    workers_healthy: Arc<AtomicBool>,
    last_worker_error: Arc<Mutex<Option<String>>>,
    clock: Arc<dyn Clock>,
    tenant_ids: BTreeSet<TenantId>,
}

impl fmt::Debug for ApplicationComponents {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ApplicationComponents")
            .field("module_registry", &"ModuleRegistry")
            .field("tenant_count", &self.tenant_ids.len())
            .field("ready", &self.is_ready())
            .finish()
    }
}

impl ApplicationComponents {
    pub fn is_ready(&self) -> bool {
        self.readiness.load(Ordering::Acquire) && self.workers_healthy.load(Ordering::Acquire)
    }

    pub fn last_worker_error(&self) -> Option<String> {
        self.last_worker_error
            .lock()
            .ok()
            .and_then(|value| value.clone())
    }
}

pub struct ApplicationRuntime {
    config: ApplicationConfig,
    components: Arc<ApplicationComponents>,
}

impl fmt::Debug for ApplicationRuntime {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ApplicationRuntime")
            .field("http_bind", &self.config.http_bind)
            .field("grpc_bind", &self.config.grpc_bind)
            .field("components", &self.components)
            .finish()
    }
}

impl ApplicationRuntime {
    pub async fn assemble(config: ApplicationConfig) -> Result<Self, ApplicationRuntimeError> {
        config.validate()?;
        let store = PostgresDataStore::connect(&config.database_url, config.maximum_connections)
            .await
            .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?;
        let clock: Arc<dyn Clock> = Arc::new(SystemClock);
        let random: Arc<dyn RandomSource> = Arc::new(ProcessIdentitySource::default());
        let now = clock.now_unix_nanos();
        if now < 0 {
            return Err(ApplicationRuntimeError::Assembly(
                "system clock is before the Unix epoch".to_owned(),
            ));
        }

        let access_tokens = AccessTokenStore::default();
        access_tokens
            .issue(
                config.bearer_token.as_bytes(),
                AccessTokenGrant {
                    actor_id: config.actor_id.clone(),
                    tenant_ids: config.tenant_ids.clone(),
                    authentication_id: "application-bootstrap-token".to_owned(),
                    expires_at_unix_nanos: expiry(now)?,
                },
            )
            .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?;
        let authenticator = Arc::new(BearerTokenAuthenticator::new(
            access_tokens,
            Arc::clone(&clock),
        ));

        let authorization_store = LiveAuthorizationStore::default();
        let visibility_store = LiveQueryVisibilityStore::default();
        let mutation_definitions = application_mutation_definitions()
            .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?;
        let query_definitions = application_query_definitions()
            .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?;
        if config.bootstrap_allow_phase6 {
            bootstrap_application_access(
                &config,
                now,
                &authorization_store,
                &visibility_store,
                &mutation_definitions,
                &query_definitions,
            )?;
        }

        let authorizer = Arc::new(LiveCapabilityAuthorizer::new(
            authorization_store,
            Arc::clone(&clock),
        ));
        let mutation_executor = Arc::new(ApplicationCapabilityExecutorRouter::new(
            store.clone(),
            Arc::new(PostgresTransactionalAggregateExecutor::new(
                store.clone(),
                Arc::new(ApplicationAggregatePlannerRouter),
            )),
            Arc::new(PostgresMetadataCapabilityExecutor::new(store.clone())),
        ));
        let mutation_gateway = Arc::new(CapabilityGateway::new(
            Arc::new(
                application_capability_catalog()
                    .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?,
            ),
            Arc::new(ContractBoundMutationSemanticValidator),
            Arc::new(FixedWindowRateLimiter::new(
                RateLimitPolicyStore::default(),
                Arc::clone(&clock),
            )),
            Arc::new(
                HmacSha256ApprovalVerifier::try_new(config.approval_signing_key.clone())
                    .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?,
            ),
            authorizer.clone(),
            mutation_executor,
            Arc::clone(&clock),
        ));

        let visibility_authorizer = Arc::new(LiveQueryVisibilityAuthorizer::new(
            visibility_store,
            Arc::clone(&clock),
        ));
        let cursor_key: [u8; 32] = config.cursor_signing_key[..32]
            .try_into()
            .map_err(|_| ApplicationRuntimeError::Assembly("cursor key is invalid".to_owned()))?;
        let owner_query_adapter = SalesActivitiesQueryAdapter::new(
            store.clone(),
            CursorCodec::new(cursor_key)
                .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?,
            visibility_authorizer.clone(),
        )
        .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?;
        let party_query_adapter = PartyQueryAdapter::new(
            store.clone(),
            CursorCodec::new(cursor_key)
                .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?,
            visibility_authorizer.clone(),
        )
        .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?;
        let account_query_adapter = AccountQueryAdapter::new(
            store.clone(),
            CursorCodec::new(cursor_key)
                .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?,
            visibility_authorizer.clone(),
        )
        .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?;
        let contact_point_query_adapter = ContactPointQueryAdapter::new(
            store.clone(),
            CursorCodec::new(cursor_key)
                .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?,
            visibility_authorizer.clone(),
        )
        .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?;
        let party_relationship_query_adapter = PartyRelationshipQueryAdapter::new(
            store.clone(),
            CursorCodec::new(cursor_key)
                .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?,
            visibility_authorizer.clone(),
        )
        .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?;
        let customer_360_query_adapter =
            Customer360QueryAdapter::new(store.clone(), visibility_authorizer.clone());
        let consent_query_adapter = ConsentQueryAdapter::new(
            store.clone(),
            CursorCodec::new(cursor_key)
                .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?,
            visibility_authorizer.clone(),
        )
        .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?;
        let identity_resolution_query_adapter = IdentityResolutionQueryAdapter::new(
            store.clone(),
            CursorCodec::new(cursor_key)
                .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?,
            visibility_authorizer.clone(),
        )
        .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?;
        let identity_resolution_merge_query_adapter = IdentityResolutionMergeQueryAdapter::new(
            store.clone(),
            CursorCodec::new(cursor_key)
                .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?,
            visibility_authorizer.clone(),
        )
        .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?;
        let customer_data_operations_query_adapter = CustomerDataOperationsQueryAdapter::new(
            store.clone(),
            CursorCodec::new(cursor_key)
                .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?,
            visibility_authorizer.clone(),
        )
        .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?;
        let search_query_adapter = SearchQueryAdapter::new(
            SearchIndexId::try_new(GLOBAL_SEARCH_INDEX_ID)
                .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?,
            Arc::new(store.clone()),
            visibility_authorizer,
            CursorCodec::new(cursor_key)
                .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?,
        )
        .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?;
        let production_query_router =
            ProductionQueryRouter::new(owner_query_adapter, search_query_adapter);
        let metadata_query_adapter =
            MetadataQueryAdapter::new(Arc::new(PostgresMetadataQueryStore::new(store.clone())));
        let query_router = Arc::new(ApplicationQueryRouter::new(
            production_query_router,
            party_query_adapter,
            account_query_adapter,
            contact_point_query_adapter,
            party_relationship_query_adapter,
            customer_360_query_adapter,
            consent_query_adapter,
            identity_resolution_query_adapter,
            identity_resolution_merge_query_adapter,
            customer_data_operations_query_adapter,
            metadata_query_adapter,
        ));
        let query_gateway = Arc::new(QueryGateway::new(
            Arc::new(
                application_query_capability_catalog()
                    .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?,
            ),
            query_router.clone(),
            authorizer,
            query_router,
        ));

        let timeout_policy = TimeoutPolicy {
            default_millis: config.default_timeout_millis,
            maximum_millis: config.maximum_timeout_millis,
        };
        let mutation_ingress = CapabilityIngress::new(
            authenticator.clone(),
            ExecutionContextResolver::new(Arc::clone(&clock), Arc::clone(&random), timeout_policy)
                .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?,
            Arc::clone(&mutation_gateway),
        );
        let query_ingress = QueryIngress::new(
            authenticator,
            QueryContextResolver::new(Arc::clone(&clock), random, timeout_policy)
                .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?,
            query_gateway,
        );

        let link_processor = Arc::new(
            SalesActivitiesLinkEventProcessor::new(
                store.clone(),
                mutation_gateway,
                SalesActivitiesLinkEventProcessorConfig {
                    worker_id: "crm-api-link-worker".to_owned(),
                    worker_actor_id: config.actor_id.clone(),
                    lease_duration_nanos: 30_000_000_000,
                    retry_delay_nanos: 5_000_000_000,
                },
            )
            .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?,
        );
        let projection_worker = Arc::new(Phase6ProjectionWorker::new(store.clone()));
        let customer_360_worker = Arc::new(
            Customer360ProjectionWorker::new(store.clone())
                .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?,
        );
        let search_worker = Arc::new(
            GlobalSearchWorker::new(store.clone())
                .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?,
        );
        let components = Arc::new(ApplicationComponents {
            store,
            module_registry: Arc::new(ModuleRegistry::new(Version::new(0, 1, 0))),
            mutation_http: Arc::new(HttpCapabilityMiddleware::new(mutation_ingress.clone())),
            mutation_grpc: Arc::new(GrpcCapabilityMiddleware::new(mutation_ingress)),
            query_http: Arc::new(HttpQueryMiddleware::new(query_ingress.clone())),
            query_grpc: Arc::new(GrpcQueryMiddleware::new(query_ingress)),
            link_processor,
            projection_worker,
            customer_360_worker,
            search_worker,
            readiness: Arc::new(AtomicBool::new(false)),
            workers_healthy: Arc::new(AtomicBool::new(true)),
            last_worker_error: Arc::new(Mutex::new(None)),
            clock,
            tenant_ids: config.tenant_ids.clone(),
        });

        Ok(Self { config, components })
    }

    pub fn components(&self) -> Arc<ApplicationComponents> {
        Arc::clone(&self.components)
    }

    pub async fn run_until_signal(self) -> Result<(), ApplicationRuntimeError> {
        let http_listener = TcpListener::bind(self.config.http_bind)
            .await
            .map_err(ApplicationRuntimeError::Io)?;
        let grpc_listener = TcpListener::bind(self.config.grpc_bind)
            .await
            .map_err(ApplicationRuntimeError::Io)?;
        self.components.readiness.store(true, Ordering::Release);

        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let worker = tokio::spawn(background_worker_loop(
            Arc::clone(&self.components),
            shutdown_rx.clone(),
        ));
        let http = tokio::spawn(run_http_server(
            http_listener,
            Arc::clone(&self.components),
            shutdown_rx.clone(),
        ));
        let grpc = tokio::spawn(run_grpc_server(
            grpc_listener,
            Arc::clone(&self.components),
            shutdown_rx,
        ));

        tokio::signal::ctrl_c()
            .await
            .map_err(ApplicationRuntimeError::Io)?;
        self.components.readiness.store(false, Ordering::Release);
        let _ = shutdown_tx.send(true);

        join_task("background worker", worker).await?;
        join_task("HTTP server", http).await?;
        join_task("gRPC server", grpc).await?;
        Ok(())
    }
}

#[derive(Debug)]
pub enum ApplicationRuntimeError {
    Config(crate::ApplicationConfigError),
    Assembly(String),
    Io(std::io::Error),
    Task(String),
    Server(String),
}

impl fmt::Display for ApplicationRuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Config(error) => write!(formatter, "configuration error: {error}"),
            Self::Assembly(message) => write!(formatter, "application assembly failed: {message}"),
            Self::Io(error) => write!(formatter, "application I/O failed: {error}"),
            Self::Task(message) => write!(formatter, "application task failed: {message}"),
            Self::Server(message) => write!(formatter, "application server failed: {message}"),
        }
    }
}

impl Error for ApplicationRuntimeError {}

impl From<crate::ApplicationConfigError> for ApplicationRuntimeError {
    fn from(value: crate::ApplicationConfigError) -> Self {
        Self::Config(value)
    }
}

#[derive(Clone)]
struct HttpState {
    components: Arc<ApplicationComponents>,
}

#[derive(Debug, serde::Deserialize)]
struct PathCoordinate {
    owner_module_id: String,
    capability_id: String,
    capability_version: String,
}

#[derive(Debug, serde::Deserialize)]
#[serde(untagged)]
enum HttpMutationBody {
    Payload(TypedPayload),
    Envelope {
        input: TypedPayload,
        approval: Option<HttpApprovalEvidence>,
    },
}

#[derive(Debug, serde::Deserialize)]
struct HttpApprovalEvidence {
    approval_id: String,
    actor_id: String,
    capability_id: String,
    capability_version: String,
    input_hash: Vec<u8>,
    policy_version: String,
    expires_at_unix_nanos: i64,
    opaque_proof: Vec<u8>,
}

impl HttpMutationBody {
    fn into_parts(self) -> Result<(TypedPayload, Option<ApprovalEvidence>), ()> {
        match self {
            Self::Payload(input) => Ok((input, None)),
            Self::Envelope { input, approval } => {
                Ok((input, approval.map(decode_http_approval).transpose()?))
            }
        }
    }
}

fn decode_http_approval(value: HttpApprovalEvidence) -> Result<ApprovalEvidence, ()> {
    Ok(ApprovalEvidence {
        approval_id: value.approval_id,
        actor_id: ActorId::try_new(value.actor_id).map_err(|_| ())?,
        capability_id: CapabilityId::try_new(value.capability_id).map_err(|_| ())?,
        capability_version: CapabilityVersion::try_new(value.capability_version).map_err(|_| ())?,
        input_hash: value.input_hash.try_into().map_err(|_| ())?,
        policy_version: value.policy_version,
        expires_at_unix_nanos: value.expires_at_unix_nanos,
        opaque_proof: value.opaque_proof,
    })
}

async fn run_http_server(
    listener: TcpListener,
    components: Arc<ApplicationComponents>,
    mut shutdown: watch::Receiver<bool>,
) -> Result<(), ApplicationRuntimeError> {
    let router = Router::new()
        .route("/healthz", get(health))
        .route("/readyz", get(ready))
        .route(
            "/v1/mutations/{owner_module_id}/{capability_id}/{capability_version}",
            post(mutation),
        )
        .route(
            "/v1/queries/{owner_module_id}/{capability_id}/{capability_version}",
            post(query),
        )
        .with_state(HttpState { components });
    axum::serve(listener, router)
        .with_graceful_shutdown(async move {
            let _ = shutdown.wait_for(|value| *value).await;
        })
        .await
        .map_err(ApplicationRuntimeError::Io)
}

async fn run_grpc_server(
    listener: TcpListener,
    components: Arc<ApplicationComponents>,
    mut shutdown: watch::Receiver<bool>,
) -> Result<(), ApplicationRuntimeError> {
    let (reporter, health_service) = tonic_health::server::health_reporter();
    let gateway_service =
        crate::gateway_v1::application_gateway_server::ApplicationGatewayServer::new(
            ApplicationGatewayService::new(components),
        );
    reporter
        .set_service_status("", ServingStatus::Serving)
        .await;
    Server::builder()
        .accept_http1(true)
        .layer(tonic_web::GrpcWebLayer::new())
        .add_service(health_service)
        .add_service(gateway_service)
        .serve_with_incoming_shutdown(TcpListenerStream::new(listener), async move {
            let _ = shutdown.wait_for(|value| *value).await;
        })
        .await
        .map_err(|error| ApplicationRuntimeError::Server(error.to_string()))
}

async fn health() -> impl IntoResponse {
    (StatusCode::OK, Json(json!({"status": "ok"})))
}

async fn ready(State(state): State<HttpState>) -> impl IntoResponse {
    if state.components.is_ready() {
        (StatusCode::OK, Json(json!({"status": "ready"})))
    } else {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({
                "status": "not_ready",
                "worker_error": state.components.last_worker_error(),
            })),
        )
    }
}

async fn mutation(
    State(state): State<HttpState>,
    Path(path): Path<PathCoordinate>,
    headers: HeaderMap,
    Json(body): Json<HttpMutationBody>,
) -> Response {
    let (input, approval) = match body.into_parts() {
        Ok(parts) => parts,
        Err(()) => return bad_approval(),
    };
    let route = match capability_route(&path, &input) {
        Ok(route) => route,
        Err(()) => return bad_route(),
    };
    let response = state
        .components
        .mutation_http
        .handle(HttpCapabilityRequest {
            headers,
            route,
            input,
            approval,
        })
        .await;
    match response.body {
        HttpCapabilityBody::Success(result) => {
            governed_json_response(response.status, response.headers, &result)
        }
        HttpCapabilityBody::Error(_) => governed_error_response(response.status, response.headers),
    }
}

async fn query(
    State(state): State<HttpState>,
    Path(path): Path<PathCoordinate>,
    headers: HeaderMap,
    Json(input): Json<TypedPayload>,
) -> Response {
    let route = match capability_route(&path, &input) {
        Ok(route) => route,
        Err(()) => return bad_route(),
    };
    let response = state
        .components
        .query_http
        .handle(HttpQueryRequest {
            headers,
            route,
            input,
        })
        .await;
    match response.body {
        HttpQueryBody::Success(result) => {
            governed_json_response(response.status, response.headers, &result.output)
        }
        HttpQueryBody::Error(_) => governed_error_response(response.status, response.headers),
    }
}

fn capability_route(path: &PathCoordinate, input: &TypedPayload) -> Result<CapabilityRoute, ()> {
    let owner_module_id = ModuleId::try_new(path.owner_module_id.clone()).map_err(|_| ())?;
    let capability_id = CapabilityId::try_new(path.capability_id.clone()).map_err(|_| ())?;
    let capability_version =
        CapabilityVersion::try_new(path.capability_version.clone()).map_err(|_| ())?;
    let schema_version =
        SchemaVersion::try_new(input.schema_version.as_str().to_owned()).map_err(|_| ())?;
    Ok(CapabilityRoute {
        owner_module_id,
        capability_id,
        capability_version,
        schema_version,
    })
}

fn bad_route() -> Response {
    (
        StatusCode::BAD_REQUEST,
        Json(json!({"error": "invalid_route"})),
    )
        .into_response()
}

fn bad_approval() -> Response {
    (
        StatusCode::BAD_REQUEST,
        Json(json!({"error": "invalid_approval"})),
    )
        .into_response()
}

fn governed_json_response<T>(status: StatusCode, headers: HeaderMap, value: &T) -> Response
where
    T: Serialize,
{
    let mut response = (status, Json(value)).into_response();
    *response.headers_mut() = headers;
    response
}

fn governed_error_response(status: StatusCode, headers: HeaderMap) -> Response {
    let mut response = (status, Json(json!({"error": "request_failed"}))).into_response();
    *response.headers_mut() = headers;
    response
}

async fn background_worker_loop(
    components: Arc<ApplicationComponents>,
    mut shutdown: watch::Receiver<bool>,
) -> Result<(), ApplicationRuntimeError> {
    let mut interval = tokio::time::interval(BACKGROUND_INTERVAL);
    loop {
        tokio::select! {
            _ = interval.tick() => {
                match run_background_cycle(&components).await {
                    Ok(()) => {
                        components.workers_healthy.store(true, Ordering::Release);
                        if let Ok(mut last_error) = components.last_worker_error.lock() {
                            *last_error = None;
                        }
                    }
                    Err(error) => {
                        components.workers_healthy.store(false, Ordering::Release);
                        if let Ok(mut last_error) = components.last_worker_error.lock() {
                            *last_error = Some(error.to_string());
                        }
                    }
                }
            }
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() {
                    return Ok(());
                }
            }
        }
    }
}

async fn run_background_cycle(
    components: &ApplicationComponents,
) -> Result<(), ApplicationRuntimeError> {
    for tenant_id in &components.tenant_ids {
        scan_link_events(components, tenant_id.clone()).await?;
        drain_projection(
            &components.projection_worker,
            tenant_id.clone(),
            DEAL_TIMELINE_PROJECTION_ID,
        )
        .await?;
        drain_projection(
            &components.projection_worker,
            tenant_id.clone(),
            TASK_STATUS_PROJECTION_ID,
        )
        .await?;
        drain_customer_360_projection(&components.customer_360_worker, tenant_id.clone()).await?;
        components
            .search_worker
            .ensure_ready(tenant_id.clone(), SEARCH_PAGE_SIZE)
            .await
            .map_err(|error| ApplicationRuntimeError::Server(error.to_string()))?;
    }
    Ok(())
}

async fn drain_customer_360_projection(
    worker: &Customer360ProjectionWorker,
    tenant_id: TenantId,
) -> Result<(), ApplicationRuntimeError> {
    loop {
        let result = worker
            .run_batch(tenant_id.clone(), PROJECTION_PAGE_SIZE)
            .await
            .map_err(|error| ApplicationRuntimeError::Server(error.to_string()))?;
        if !result.has_more {
            return Ok(());
        }
    }
}

async fn scan_link_events(
    components: &ApplicationComponents,
    tenant_id: TenantId,
) -> Result<(), ApplicationRuntimeError> {
    let event_type = EventType::try_new("sales.deal.stage_changed")
        .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?;
    let consumer_module_id = ModuleId::try_new(LINK_MODULE_ID)
        .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?;
    let mut after = None;
    loop {
        let page = components
            .store
            .list_event_history(&EventHistoryRequest {
                tenant_id: tenant_id.clone(),
                consumer_module_id: consumer_module_id.clone(),
                event_types: vec![event_type.clone()],
                after,
                page_size: LINK_SCAN_PAGE_SIZE,
            })
            .await
            .map_err(|error| ApplicationRuntimeError::Server(error.to_string()))?;
        for delivery in page.deliveries {
            let outcome = components
                .link_processor
                .process(
                    tenant_id.clone(),
                    delivery.event_id,
                    components.clock.now_unix_nanos(),
                )
                .await
                .map_err(|error| ApplicationRuntimeError::Server(error.to_string()))?;
            if let SalesActivitiesLinkDeliveryOutcome::DeadLettered { error_code } = outcome {
                return Err(ApplicationRuntimeError::Server(format!(
                    "link event dead-lettered: {error_code}"
                )));
            }
        }
        let Some(next) = page.next_cursor else {
            return Ok(());
        };
        after = Some(next);
    }
}

async fn drain_projection(
    worker: &Phase6ProjectionWorker,
    tenant_id: TenantId,
    projection_id: &str,
) -> Result<(), ApplicationRuntimeError> {
    loop {
        let result = worker
            .run_batch(tenant_id.clone(), projection_id, PROJECTION_PAGE_SIZE)
            .await
            .map_err(|error| ApplicationRuntimeError::Server(error.to_string()))?;
        if !result.has_more {
            return Ok(());
        }
    }
}

#[derive(Clone, Copy)]
struct BootstrapVisibilityResource<'a> {
    owner_module_id: &'a str,
    resource_type: &'a str,
}

fn bootstrap_application_access(
    config: &ApplicationConfig,
    now_unix_nanos: i64,
    authorization_store: &LiveAuthorizationStore,
    visibility_store: &LiveQueryVisibilityStore,
    mutation_definitions: &[CapabilityDefinition],
    query_definitions: &[CapabilityDefinition],
) -> Result<(), ApplicationRuntimeError> {
    let expires_at = expiry(now_unix_nanos)?;
    for tenant_id in &config.tenant_ids {
        for definition in mutation_definitions.iter().chain(query_definitions) {
            authorization_store
                .upsert(AuthorizationGrant {
                    tenant_id: tenant_id.clone(),
                    actor_id: config.actor_id.clone(),
                    policy_id: definition.authorization_policy_id.clone(),
                    capability_id: definition.capability_id.clone(),
                    capability_version: definition.capability_version.clone(),
                    owner_module_id: definition.owner_module_id.clone(),
                    policy_version: BOOTSTRAP_POLICY_VERSION.to_owned(),
                    expires_at_unix_nanos: Some(expires_at),
                })
                .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?;
        }
        for definition in query_definitions {
            match definition.owner_module_id.as_str() {
                "crm.sales" => upsert_bootstrap_visibility(
                    visibility_store,
                    config,
                    tenant_id,
                    definition,
                    BootstrapVisibilityResource {
                        owner_module_id: "crm.sales",
                        resource_type: SALES_RECORD_TYPE,
                    },
                    sales_fields(),
                    expires_at,
                )?,
                "crm.activities" => upsert_bootstrap_visibility(
                    visibility_store,
                    config,
                    tenant_id,
                    definition,
                    BootstrapVisibilityResource {
                        owner_module_id: "crm.activities",
                        resource_type: ACTIVITIES_RECORD_TYPE,
                    },
                    task_fields(),
                    expires_at,
                )?,
                PARTIES_MODULE_ID => upsert_bootstrap_visibility(
                    visibility_store,
                    config,
                    tenant_id,
                    definition,
                    BootstrapVisibilityResource {
                        owner_module_id: PARTIES_MODULE_ID,
                        resource_type: PARTY_RECORD_TYPE,
                    },
                    party_fields(),
                    expires_at,
                )?,
                ACCOUNTS_MODULE_ID => upsert_bootstrap_visibility(
                    visibility_store,
                    config,
                    tenant_id,
                    definition,
                    BootstrapVisibilityResource {
                        owner_module_id: ACCOUNTS_MODULE_ID,
                        resource_type: ACCOUNT_RECORD_TYPE,
                    },
                    account_fields(),
                    expires_at,
                )?,
                CONTACT_POINTS_MODULE_ID => upsert_bootstrap_visibility(
                    visibility_store,
                    config,
                    tenant_id,
                    definition,
                    BootstrapVisibilityResource {
                        owner_module_id: CONTACT_POINTS_MODULE_ID,
                        resource_type: CONTACT_POINT_RECORD_TYPE,
                    },
                    contact_point_fields(),
                    expires_at,
                )?,
                CONSENTS_MODULE_ID => upsert_bootstrap_visibility(
                    visibility_store,
                    config,
                    tenant_id,
                    definition,
                    BootstrapVisibilityResource {
                        owner_module_id: CONSENTS_MODULE_ID,
                        resource_type: CONSENT_RECORD_TYPE,
                    },
                    consent_fields(),
                    expires_at,
                )?,
                IDENTITY_RESOLUTION_MODULE_ID => {
                    upsert_bootstrap_visibility(
                        visibility_store,
                        config,
                        tenant_id,
                        definition,
                        BootstrapVisibilityResource {
                            owner_module_id: IDENTITY_RESOLUTION_MODULE_ID,
                            resource_type: IDENTITY_RESOLUTION_RECORD_TYPE,
                        },
                        identity_resolution_fields(),
                        expires_at,
                    )?;
                    upsert_bootstrap_visibility(
                        visibility_store,
                        config,
                        tenant_id,
                        definition,
                        BootstrapVisibilityResource {
                            owner_module_id: IDENTITY_RESOLUTION_MODULE_ID,
                            resource_type: IDENTITY_RESOLUTION_MERGE_RECORD_TYPE,
                        },
                        identity_resolution_merge_fields(),
                        expires_at,
                    )?;
                }
                PARTY_RELATIONSHIPS_MODULE_ID => upsert_bootstrap_visibility(
                    visibility_store,
                    config,
                    tenant_id,
                    definition,
                    BootstrapVisibilityResource {
                        owner_module_id: PARTY_RELATIONSHIPS_MODULE_ID,
                        resource_type: PARTY_RELATIONSHIP_RECORD_TYPE,
                    },
                    party_relationship_fields(),
                    expires_at,
                )?,
                CUSTOMER_DATA_OPERATIONS_MODULE_ID => {
                    upsert_bootstrap_visibility(
                        visibility_store,
                        config,
                        tenant_id,
                        definition,
                        BootstrapVisibilityResource {
                            owner_module_id: CUSTOMER_DATA_OPERATIONS_MODULE_ID,
                            resource_type: CUSTOMER_DATA_IMPORT_JOB_RECORD_TYPE,
                        },
                        customer_data_import_job_fields(),
                        expires_at,
                    )?;
                    if definition.capability_id.as_str() == LIST_IMPORT_ROWS_CAPABILITY {
                        upsert_bootstrap_visibility(
                            visibility_store,
                            config,
                            tenant_id,
                            definition,
                            BootstrapVisibilityResource {
                                owner_module_id: CUSTOMER_DATA_OPERATIONS_MODULE_ID,
                                resource_type: CUSTOMER_DATA_IMPORT_ROW_RECORD_TYPE,
                            },
                            customer_data_import_row_fields(),
                            expires_at,
                        )?;
                    }
                }
                METADATA_MODULE_ID => {}
                CUSTOMER_360_MODULE_ID => {
                    upsert_bootstrap_visibility(
                        visibility_store,
                        config,
                        tenant_id,
                        definition,
                        BootstrapVisibilityResource {
                            owner_module_id: PARTIES_MODULE_ID,
                            resource_type: PARTY_RECORD_TYPE,
                        },
                        customer_360_party_fields(),
                        expires_at,
                    )?;
                    upsert_bootstrap_visibility(
                        visibility_store,
                        config,
                        tenant_id,
                        definition,
                        BootstrapVisibilityResource {
                            owner_module_id: ACCOUNTS_MODULE_ID,
                            resource_type: ACCOUNT_RECORD_TYPE,
                        },
                        customer_360_account_fields(),
                        expires_at,
                    )?;
                    upsert_bootstrap_visibility(
                        visibility_store,
                        config,
                        tenant_id,
                        definition,
                        BootstrapVisibilityResource {
                            owner_module_id: CONTACT_POINTS_MODULE_ID,
                            resource_type: CONTACT_POINT_RECORD_TYPE,
                        },
                        customer_360_contact_point_fields(),
                        expires_at,
                    )?;
                    upsert_bootstrap_visibility(
                        visibility_store,
                        config,
                        tenant_id,
                        definition,
                        BootstrapVisibilityResource {
                            owner_module_id: PARTY_RELATIONSHIPS_MODULE_ID,
                            resource_type: PARTY_RELATIONSHIP_RECORD_TYPE,
                        },
                        customer_360_party_relationship_fields(),
                        expires_at,
                    )?;
                }
                SEARCH_MODULE_ID => {
                    upsert_bootstrap_visibility(
                        visibility_store,
                        config,
                        tenant_id,
                        definition,
                        BootstrapVisibilityResource {
                            owner_module_id: "crm.sales",
                            resource_type: SALES_RECORD_TYPE,
                        },
                        ["name"].into_iter().map(str::to_owned).collect(),
                        expires_at,
                    )?;
                    upsert_bootstrap_visibility(
                        visibility_store,
                        config,
                        tenant_id,
                        definition,
                        BootstrapVisibilityResource {
                            owner_module_id: "crm.activities",
                            resource_type: ACTIVITIES_RECORD_TYPE,
                        },
                        ["subject"].into_iter().map(str::to_owned).collect(),
                        expires_at,
                    )?;
                    upsert_bootstrap_visibility(
                        visibility_store,
                        config,
                        tenant_id,
                        definition,
                        BootstrapVisibilityResource {
                            owner_module_id: PARTIES_MODULE_ID,
                            resource_type: PARTY_RECORD_TYPE,
                        },
                        party_fields(),
                        expires_at,
                    )?;
                }
                _ => {
                    return Err(ApplicationRuntimeError::Assembly(
                        "unsupported bootstrap query owner".to_owned(),
                    ));
                }
            }
        }
    }
    Ok(())
}

fn upsert_bootstrap_visibility(
    visibility_store: &LiveQueryVisibilityStore,
    config: &ApplicationConfig,
    tenant_id: &TenantId,
    definition: &CapabilityDefinition,
    resource: BootstrapVisibilityResource<'_>,
    allowed_fields: BTreeSet<String>,
    expires_at: i64,
) -> Result<(), ApplicationRuntimeError> {
    visibility_store
        .upsert(QueryVisibilityGrant {
            tenant_id: tenant_id.clone(),
            actor_id: config.actor_id.clone(),
            capability_id: definition.capability_id.clone(),
            capability_version: definition.capability_version.clone(),
            owner_module_id: ModuleId::try_new(resource.owner_module_id)
                .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?,
            record_type: RecordType::try_new(resource.resource_type)
                .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?,
            record_id: None,
            allowed_fields,
            policy_version: BOOTSTRAP_POLICY_VERSION.to_owned(),
            expires_at_unix_nanos: Some(expires_at),
        })
        .map(|_| ())
        .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))
}

fn expiry(now_unix_nanos: i64) -> Result<i64, ApplicationRuntimeError> {
    now_unix_nanos
        .checked_add(BOOTSTRAP_LIFETIME_NANOS)
        .ok_or_else(|| ApplicationRuntimeError::Assembly("bootstrap expiry overflow".to_owned()))
}

fn sales_fields() -> BTreeSet<String> {
    [
        "name",
        "stage",
        "amount",
        "owner",
        "account",
        "primary_contact",
        "expected_close_date",
        "probability_basis_points",
        "status",
        "close_outcome",
        "created_at",
        "updated_at",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}

fn customer_data_import_job_fields() -> BTreeSet<String> {
    ["source", "mapping", "status", "counters", "checkpoint"]
        .into_iter()
        .map(str::to_owned)
        .collect()
}

fn customer_data_import_row_fields() -> BTreeSet<String> {
    [
        "row_position",
        "source_identity",
        "status",
        "prepared_party",
        "diagnostics",
        "execution",
        "target_party_ref",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}

fn customer_360_party_fields() -> BTreeSet<String> {
    ["display_name"].into_iter().map(str::to_owned).collect()
}

fn customer_360_account_fields() -> BTreeSet<String> {
    ["name", "status"].into_iter().map(str::to_owned).collect()
}

fn customer_360_contact_point_fields() -> BTreeSet<String> {
    [
        "party_ref",
        "kind",
        "normalized_value",
        "status",
        "preferred",
        "validity",
        "verification",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}

fn customer_360_party_relationship_fields() -> BTreeSet<String> {
    ["from_party_ref", "to_party_ref", "status", "validity"]
        .into_iter()
        .map(str::to_owned)
        .collect()
}

fn party_fields() -> BTreeSet<String> {
    ["kind", "display_name"]
        .into_iter()
        .map(str::to_owned)
        .collect()
}

fn account_fields() -> BTreeSet<String> {
    ["name", "status", "party_associations"]
        .into_iter()
        .map(str::to_owned)
        .collect()
}

fn contact_point_fields() -> BTreeSet<String> {
    [
        "party_ref",
        "kind",
        "normalized_value",
        "display_value",
        "status",
        "preferred",
        "validity",
        "verification",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
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

fn identity_resolution_fields() -> BTreeSet<String> {
    [
        "party_pair",
        "evidence_history",
        "status",
        "decision_reason",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}

fn identity_resolution_merge_fields() -> BTreeSet<String> {
    [
        "party_pair",
        "decision",
        "survivorship",
        "status",
        "unmerge_decision",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}

fn party_relationship_fields() -> BTreeSet<String> {
    [
        "from_party_ref",
        "to_party_ref",
        "relationship_type",
        "status",
        "validity",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}

fn task_fields() -> BTreeSet<String> {
    [
        "subject",
        "description",
        "owner",
        "related_resources",
        "priority",
        "status",
        "due_at",
        "reminder_at",
        "completed_at",
        "created_at",
        "updated_at",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}

async fn join_task<T>(
    name: &'static str,
    task: tokio::task::JoinHandle<Result<T, ApplicationRuntimeError>>,
) -> Result<T, ApplicationRuntimeError> {
    task.await
        .map_err(|error| ApplicationRuntimeError::Task(format!("{name}: {error}")))?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn field_bootstrap_sets_are_nonempty_and_stable() {
        assert!(sales_fields().contains("name"));
        assert!(sales_fields().contains("amount"));
        assert!(party_fields().contains("kind"));
        assert!(party_fields().contains("display_name"));
        assert!(account_fields().contains("name"));
        assert!(account_fields().contains("party_associations"));
        assert!(contact_point_fields().contains("party_ref"));
        assert!(contact_point_fields().contains("verification"));
        assert!(consent_fields().contains("purpose"));
        assert!(consent_fields().contains("evidence_ref"));
        assert!(party_relationship_fields().contains("from_party_ref"));
        assert!(party_relationship_fields().contains("relationship_type"));
        assert!(party_relationship_fields().contains("validity"));
        assert!(task_fields().contains("subject"));
        assert!(task_fields().contains("status"));
    }

    #[test]
    fn expiry_is_strictly_after_now() {
        assert!(expiry(100).unwrap() > 100);
    }
}
