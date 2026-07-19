use crate::{
    ApplicationConfig, ApplicationGatewayService, BootstrapVisibilityResource,
    CustomerEnrichmentApplicationWorkerDependencies, GovernedPartyExportSelectionSource,
    PartyExportArtifactDownloadService, PostgresModuleActivation, ProcessIdentitySource,
    ProductionBackgroundWorkerDependencies, ProductionCompositionDependencies, SystemClock,
    bootstrap_export_selection_worker_access, build_bootstrap_visibility_registry,
    build_customer_enrichment_application_worker, build_production_background_workers,
    build_production_composition,
};
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use crm_application_composition::BackgroundWorkerRegistry;
use crm_capability_adapters::{
    AuthorizationGrant, FixedWindowRateLimiter, GatewayCapabilityClient,
    HmacSha256ApprovalVerifier, LiveAuthorizationStore, LiveCapabilityAuthorizer,
    LiveQueryVisibilityAuthorizer, LiveQueryVisibilityStore, QueryVisibilityGrant,
    RateLimitPolicyStore,
};
use crm_capability_ingress::{
    AccessTokenGrant, AccessTokenStore, BearerTokenAuthenticator, CapabilityIngress,
    CapabilityRoute, ExecutionContextResolver, GrpcCapabilityMiddleware, GrpcQueryMiddleware,
    HttpCapabilityBody, HttpCapabilityMiddleware, HttpCapabilityRequest, HttpQueryBody,
    HttpQueryMiddleware, HttpQueryRequest, QueryContextResolver, QueryIngress, TimeoutPolicy,
};
use crm_capability_runtime::{ApprovalEvidence, CapabilityDefinition, CapabilityGateway};
use crm_consents_query_adapter::GET_CAPABILITY as CONSENT_GET_CAPABILITY;
use crm_core_data::{PostgresDataStore, PostgresImmutableFileArtifactStore};
use crm_customer_360_composition::Customer360ProjectionWorker;
use crm_customer_data_operations_capability_adapter::internal_export_selection_capability_definitions;
use crm_customer_data_operations_execution_composition::{
    EXPORT_SELECTION_WORKER_ACTOR_ID, IMPORT_EXECUTION_WORKER_ACTOR_ID, PartyExportSelectionWorker,
    PartyImportExecutionCoordinator, PartyImportExecutionWorker,
    PostgresImportExecutionOutcomeSink, PostgresImportExecutionSnapshotReader,
    PostgresPartyExportSelectionReader, PostgresPartyExportSelectionSink,
    internal_capability_definitions,
};
use crm_customer_data_operations_query_adapter::{
    PartyExportArtifactDownloadResolver, artifact_download_capability_definition,
};
use crm_customer_enrichment_application_composition::PARTY_DISPLAY_NAME_APPLICATION_WORKER_ACTOR_ID;
use crm_global_search_composition::GlobalSearchWorker;
use crm_module_sdk::{
    ActorId, CapabilityId, CapabilityVersion, Clock, ModuleId, RandomSource, RecordType,
    SchemaVersion, TenantId, TypedPayload,
};
use crm_parties_capability_adapter::{
    CREATE_CAPABILITY as PARTY_CREATE_CAPABILITY, MODULE_ID as PARTIES_MODULE_ID,
    UPDATE_CAPABILITY as PARTY_UPDATE_CAPABILITY,
};
use crm_parties_query_adapter::{GET_CAPABILITY as PARTY_GET_CAPABILITY, PartyQueryAdapter};
use crm_query_runtime::{CursorCodec, QueryGateway};
use crm_sales_activities_capability_composition::{
    Phase6ProjectionWorker, SalesActivitiesLinkEventProcessor,
    SalesActivitiesLinkEventProcessorConfig,
};
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

#[derive(Clone)]
pub struct ApplicationComponents {
    pub module_ids: BTreeSet<String>,
    pub mutation_http: Arc<HttpCapabilityMiddleware>,
    pub mutation_grpc: Arc<GrpcCapabilityMiddleware>,
    pub query_http: Arc<HttpQueryMiddleware>,
    pub query_grpc: Arc<GrpcQueryMiddleware>,
    pub background_workers: BackgroundWorkerRegistry,
    pub export_artifact_download: Arc<PartyExportArtifactDownloadService>,
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
            .field("module_count", &self.module_ids.len())
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
        let authorizer = Arc::new(LiveCapabilityAuthorizer::new(
            authorization_store.clone(),
            Arc::clone(&clock),
        ));
        let visibility_authorizer = Arc::new(LiveQueryVisibilityAuthorizer::new(
            visibility_store.clone(),
            Arc::clone(&clock),
        ));
        let cursor_key: [u8; 32] = config.cursor_signing_key[..32]
            .try_into()
            .map_err(|_| ApplicationRuntimeError::Assembly("cursor key is invalid".to_owned()))?;
        let activation: Arc<dyn crm_application_composition::ModuleActivationPort> =
            Arc::new(PostgresModuleActivation::new(store.clone()));
        let capability_authorizer: Arc<dyn crm_capability_runtime::CapabilityAuthorizer> =
            authorizer.clone();
        let query_authorizer: Arc<dyn crm_query_runtime::QueryAuthorizer> = authorizer.clone();
        let query_visibility: Arc<dyn crm_query_runtime::QueryVisibilityAuthorizer> =
            visibility_authorizer.clone();
        let composition = build_production_composition(ProductionCompositionDependencies {
            store: store.clone(),
            activation: activation.clone(),
            capability_authorizer,
            query_authorizer: query_authorizer.clone(),
            visibility_authorizer: query_visibility.clone(),
            cursor_key,
        })
        .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?;
        let module_ids = composition.module_ids().clone();
        if config.bootstrap_allow_phase6 {
            store
                .bootstrap_activate_published_modules(&config.tenant_ids, &module_ids)
                .await
                .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?;
        }
        let mutation_definitions = composition.mutation_definitions().to_vec();
        let query_definitions = composition.query_definitions().to_vec();
        let internal_import_outcome_definitions = internal_capability_definitions()
            .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?;
        let internal_export_selection_definitions =
            internal_export_selection_capability_definitions()
                .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?;
        let artifact_download_definition = artifact_download_capability_definition()
            .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?;
        let import_execution_worker_actor_id =
            ActorId::try_new(IMPORT_EXECUTION_WORKER_ACTOR_ID)
                .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?;
        let export_selection_worker_actor_id =
            ActorId::try_new(EXPORT_SELECTION_WORKER_ACTOR_ID)
                .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?;
        let customer_enrichment_application_worker_actor_id =
            ActorId::try_new(PARTY_DISPLAY_NAME_APPLICATION_WORKER_ACTOR_ID)
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
            bootstrap_import_execution_worker_access(
                &config,
                now,
                &authorization_store,
                &mutation_definitions,
                &internal_import_outcome_definitions,
                &import_execution_worker_actor_id,
            )?;
            bootstrap_export_selection_worker_access(
                &config,
                now,
                &authorization_store,
                &visibility_store,
                &query_definitions,
                &artifact_download_definition,
                &internal_export_selection_definitions,
                &export_selection_worker_actor_id,
            )?;
            bootstrap_customer_enrichment_application_worker_access(
                &config,
                now,
                &authorization_store,
                &visibility_store,
                &mutation_definitions,
                &query_definitions,
                &customer_enrichment_application_worker_actor_id,
            )?;
        }

        let mutation_gateway = Arc::new(CapabilityGateway::new(
            composition.mutation_registry(),
            composition.mutation_validator(),
            Arc::new(FixedWindowRateLimiter::new(
                RateLimitPolicyStore::default(),
                Arc::clone(&clock),
            )),
            Arc::new(
                HmacSha256ApprovalVerifier::try_new(config.approval_signing_key.clone())
                    .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?,
            ),
            authorizer.clone(),
            composition.mutation_executor(),
            Arc::clone(&clock),
        ));

        let customer_enrichment_application_worker = build_customer_enrichment_application_worker(
            CustomerEnrichmentApplicationWorkerDependencies {
                store: store.clone(),
                capabilities: Arc::new(GatewayCapabilityClient::new(Arc::clone(&mutation_gateway))),
                query_authorizer,
                visibility_authorizer: query_visibility,
                clock: Arc::clone(&clock),
                cursor_key,
                actor_id: customer_enrichment_application_worker_actor_id,
            },
        )
        .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?;

        let import_execution_reader =
            Arc::new(PostgresImportExecutionSnapshotReader::new(store.clone()));
        let import_execution_outcomes = Arc::new(PostgresImportExecutionOutcomeSink::new(
            store.clone(),
            authorizer.clone(),
        ));
        let import_execution_coordinator = Arc::new(PartyImportExecutionCoordinator::new(
            Arc::new(GatewayCapabilityClient::new(Arc::clone(&mutation_gateway))),
            import_execution_outcomes,
        ));
        let import_execution_worker = Arc::new(
            PartyImportExecutionWorker::new(
                store.clone(),
                import_execution_reader,
                import_execution_coordinator,
                Arc::clone(&clock),
            )
            .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?,
        );

        let artifact_download_resolver = Arc::new(PartyExportArtifactDownloadResolver::new(
            store.clone(),
            visibility_authorizer.clone(),
        ));
        let export_party_query_adapter = PartyQueryAdapter::new(
            store.clone(),
            CursorCodec::new(cursor_key)
                .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?,
            visibility_authorizer.clone(),
        )
        .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?;
        let export_selection_source = Arc::new(GovernedPartyExportSelectionSource::new(
            Arc::new(export_party_query_adapter),
            authorizer.clone(),
        ));
        let export_selection_reader =
            Arc::new(PostgresPartyExportSelectionReader::new(store.clone()));
        let export_selection_sink = Arc::new(PostgresPartyExportSelectionSink::new(
            store.clone(),
            authorizer.clone(),
        ));
        let export_selection_worker = Arc::new(
            PartyExportSelectionWorker::new(
                store.clone(),
                export_selection_reader,
                export_selection_sink,
                export_selection_source,
                Arc::clone(&clock),
            )
            .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?,
        );
        let query_gateway = Arc::new(QueryGateway::new(
            composition.query_registry(),
            composition.query_validator(),
            authorizer.clone(),
            composition.query_executor(),
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
        let query_context_resolver =
            QueryContextResolver::new(Arc::clone(&clock), Arc::clone(&random), timeout_policy)
                .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?;
        let export_artifact_download = Arc::new(
            PartyExportArtifactDownloadService::new(
                authenticator.clone(),
                query_context_resolver.clone(),
                authorizer.clone(),
                artifact_download_resolver,
                Arc::new(PostgresImmutableFileArtifactStore::new(store.clone())),
                store.clone(),
                config.export_retention_policies.clone(),
            )
            .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?,
        );
        let query_ingress = QueryIngress::new(authenticator, query_context_resolver, query_gateway);

        let link_processor = Arc::new(
            SalesActivitiesLinkEventProcessor::new(
                store.clone(),
                Arc::clone(&mutation_gateway),
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
        let background_workers =
            build_production_background_workers(ProductionBackgroundWorkerDependencies {
                module_ids: module_ids.clone(),
                activation,
                store,
                import_execution_worker,
                export_selection_worker,
                customer_enrichment_application_worker,
                link_processor,
                projection_worker,
                customer_360_worker,
                search_worker,
            })
            .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?;
        let components = Arc::new(ApplicationComponents {
            module_ids,
            mutation_http: Arc::new(HttpCapabilityMiddleware::new(mutation_ingress.clone())),
            mutation_grpc: Arc::new(GrpcCapabilityMiddleware::new(mutation_ingress)),
            query_http: Arc::new(HttpQueryMiddleware::new(query_ingress.clone())),
            query_grpc: Arc::new(GrpcQueryMiddleware::new(query_ingress)),
            background_workers,
            export_artifact_download,
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
    let download_router =
        crate::export_artifact_download_router(Arc::clone(&components.export_artifact_download));
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
        .with_state(HttpState { components })
        .merge(download_router);
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
    let now_unix_nanos = components.clock.now_unix_nanos();
    for tenant_id in &components.tenant_ids {
        components
            .background_workers
            .run_tenant_cycle(tenant_id.clone(), now_unix_nanos)
            .await
            .map_err(|error| ApplicationRuntimeError::Server(error.to_string()))?;
    }
    Ok(())
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
    let visibility = build_bootstrap_visibility_registry()
        .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?;
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
            let resources = visibility
                .resources_for(definition)
                .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?;
            for resource in resources {
                upsert_bootstrap_visibility(
                    visibility_store,
                    &config.actor_id,
                    tenant_id,
                    definition,
                    resource,
                    expires_at,
                )?;
            }
        }
    }
    Ok(())
}

fn bootstrap_import_execution_worker_access(
    config: &ApplicationConfig,
    now_unix_nanos: i64,
    authorization_store: &LiveAuthorizationStore,
    mutation_definitions: &[CapabilityDefinition],
    internal_definitions: &[CapabilityDefinition],
    worker_actor_id: &ActorId,
) -> Result<(), ApplicationRuntimeError> {
    let expires_at = expiry(now_unix_nanos)?;
    let party_create = mutation_definitions
        .iter()
        .find(|definition| {
            definition.owner_module_id.as_str() == PARTIES_MODULE_ID
                && definition.capability_id.as_str() == PARTY_CREATE_CAPABILITY
        })
        .ok_or_else(|| {
            ApplicationRuntimeError::Assembly(
                "Party create capability is missing from the production catalog".to_owned(),
            )
        })?;
    for tenant_id in &config.tenant_ids {
        for definition in std::iter::once(party_create).chain(internal_definitions.iter()) {
            authorization_store
                .upsert(AuthorizationGrant {
                    tenant_id: tenant_id.clone(),
                    actor_id: worker_actor_id.clone(),
                    policy_id: definition.authorization_policy_id.clone(),
                    capability_id: definition.capability_id.clone(),
                    capability_version: definition.capability_version.clone(),
                    owner_module_id: definition.owner_module_id.clone(),
                    policy_version: BOOTSTRAP_POLICY_VERSION.to_owned(),
                    expires_at_unix_nanos: Some(expires_at),
                })
                .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?;
        }
    }
    Ok(())
}

fn bootstrap_customer_enrichment_application_worker_access(
    config: &ApplicationConfig,
    now_unix_nanos: i64,
    authorization_store: &LiveAuthorizationStore,
    visibility_store: &LiveQueryVisibilityStore,
    mutation_definitions: &[CapabilityDefinition],
    query_definitions: &[CapabilityDefinition],
    worker_actor_id: &ActorId,
) -> Result<(), ApplicationRuntimeError> {
    let expires_at = expiry(now_unix_nanos)?;
    let party_update = mutation_definitions
        .iter()
        .find(|definition| {
            definition.owner_module_id.as_str() == PARTIES_MODULE_ID
                && definition.capability_id.as_str() == PARTY_UPDATE_CAPABILITY
        })
        .ok_or_else(|| {
            ApplicationRuntimeError::Assembly(
                "Party update capability is missing from the production catalog".to_owned(),
            )
        })?;
    let party_get = query_definitions
        .iter()
        .find(|definition| definition.capability_id.as_str() == PARTY_GET_CAPABILITY)
        .ok_or_else(|| {
            ApplicationRuntimeError::Assembly(
                "Party get capability is missing from the production catalog".to_owned(),
            )
        })?;
    let consent_get = query_definitions
        .iter()
        .find(|definition| definition.capability_id.as_str() == CONSENT_GET_CAPABILITY)
        .ok_or_else(|| {
            ApplicationRuntimeError::Assembly(
                "Consent get capability is missing from the production catalog".to_owned(),
            )
        })?;
    let visibility = build_bootstrap_visibility_registry()
        .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?;
    for tenant_id in &config.tenant_ids {
        for definition in [party_update, party_get, consent_get] {
            authorization_store
                .upsert(AuthorizationGrant {
                    tenant_id: tenant_id.clone(),
                    actor_id: worker_actor_id.clone(),
                    policy_id: definition.authorization_policy_id.clone(),
                    capability_id: definition.capability_id.clone(),
                    capability_version: definition.capability_version.clone(),
                    owner_module_id: definition.owner_module_id.clone(),
                    policy_version: BOOTSTRAP_POLICY_VERSION.to_owned(),
                    expires_at_unix_nanos: Some(expires_at),
                })
                .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?;
        }
        for definition in [party_get, consent_get] {
            let resources = visibility
                .resources_for(definition)
                .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?;
            for resource in resources {
                upsert_bootstrap_visibility(
                    visibility_store,
                    worker_actor_id,
                    tenant_id,
                    definition,
                    resource,
                    expires_at,
                )?;
            }
        }
    }
    Ok(())
}

fn upsert_bootstrap_visibility(
    visibility_store: &LiveQueryVisibilityStore,
    actor_id: &ActorId,
    tenant_id: &TenantId,
    definition: &CapabilityDefinition,
    resource: BootstrapVisibilityResource,
    expires_at: i64,
) -> Result<(), ApplicationRuntimeError> {
    visibility_store
        .upsert(QueryVisibilityGrant {
            tenant_id: tenant_id.clone(),
            actor_id: actor_id.clone(),
            capability_id: definition.capability_id.clone(),
            capability_version: definition.capability_version.clone(),
            owner_module_id: ModuleId::try_new(resource.owner_module_id)
                .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?,
            record_type: RecordType::try_new(resource.resource_type)
                .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?,
            record_id: None,
            allowed_fields: resource.allowed_fields,
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
    fn expiry_is_strictly_after_now() {
        assert!(expiry(100).unwrap() > 100);
    }
}
