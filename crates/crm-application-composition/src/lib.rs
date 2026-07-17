#![forbid(unsafe_code)]

//! Deterministic first-party module contribution runtime.
//!
//! This crate owns application-composition mechanics only. It knows no business
//! module, transport, database or process-host implementation. A host contributes
//! exact versioned mutation/query routes and background workers; assembly rejects
//! duplicate, owner-mismatched or incomplete coordinates before serving traffic.

use crm_capability_runtime::{
    CapabilityDefinition, CapabilityExecutionResult, CapabilityRegistryPort, CapabilityRequest,
    CapabilitySemanticValidator, TransactionalCapabilityExecutor,
};
use crm_module_sdk::{
    CapabilityId, CapabilityVersion, ErrorCategory, ModuleId, PortFuture, SdkError, TenantId,
};
use crm_query_runtime::{
    QueryExecutionResult, QueryExecutor, QueryRequest, QuerySemanticValidator,
};
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::sync::Arc;

pub const CRATE_NAME: &str = "crm-application-composition";

type Coordinate = (String, String);
type WorkerCoordinate = (String, String);

fn coordinate(definition: &CapabilityDefinition) -> Coordinate {
    (
        definition.capability_id.as_str().to_owned(),
        definition.capability_version.as_str().to_owned(),
    )
}

#[derive(Clone)]
pub struct MutationRoute {
    pub definition: CapabilityDefinition,
    pub validator: Arc<dyn CapabilitySemanticValidator>,
    pub executor: Arc<dyn TransactionalCapabilityExecutor>,
}

impl fmt::Debug for MutationRoute {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MutationRoute")
            .field("definition", &self.definition)
            .field("validator", &"dyn CapabilitySemanticValidator")
            .field("executor", &"dyn TransactionalCapabilityExecutor")
            .finish()
    }
}

#[derive(Clone)]
pub struct QueryRoute {
    pub definition: CapabilityDefinition,
    pub validator: Arc<dyn QuerySemanticValidator>,
    pub executor: Arc<dyn QueryExecutor>,
}

impl fmt::Debug for QueryRoute {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("QueryRoute")
            .field("definition", &self.definition)
            .field("validator", &"dyn QuerySemanticValidator")
            .field("executor", &"dyn QueryExecutor")
            .finish()
    }
}

pub trait TenantBackgroundWorker: Send + Sync {
    fn run_tenant_cycle<'a>(
        &'a self,
        tenant_id: TenantId,
        now_unix_nanos: i64,
    ) -> PortFuture<'a, Result<(), SdkError>>;
}

#[derive(Clone)]
pub struct BackgroundWorkerContribution {
    pub worker_id: String,
    pub worker: Arc<dyn TenantBackgroundWorker>,
}

impl fmt::Debug for BackgroundWorkerContribution {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BackgroundWorkerContribution")
            .field("worker_id", &self.worker_id)
            .field("worker", &"dyn TenantBackgroundWorker")
            .finish()
    }
}

#[derive(Debug, Clone)]
pub struct ModuleRuntimeContribution {
    pub module_id: ModuleId,
    pub mutations: Vec<MutationRoute>,
    pub queries: Vec<QueryRoute>,
    pub background_workers: Vec<BackgroundWorkerContribution>,
}

impl ModuleRuntimeContribution {
    pub fn new(module_id: ModuleId) -> Self {
        Self {
            module_id,
            mutations: Vec::new(),
            queries: Vec::new(),
            background_workers: Vec::new(),
        }
    }

    pub fn with_mutation(mut self, route: MutationRoute) -> Self {
        self.mutations.push(route);
        self
    }

    pub fn with_query(mut self, route: QueryRoute) -> Self {
        self.queries.push(route);
        self
    }

    pub fn with_background_worker(
        mut self,
        worker_id: impl Into<String>,
        worker: Arc<dyn TenantBackgroundWorker>,
    ) -> Self {
        self.background_workers.push(BackgroundWorkerContribution {
            worker_id: worker_id.into(),
            worker,
        });
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompositionError {
    DuplicateModule(String),
    DuplicateMutation(Coordinate),
    DuplicateQuery(Coordinate),
    DuplicateWorker(WorkerCoordinate),
    OwnerMismatch {
        module_id: String,
        capability_id: String,
        capability_version: String,
        owner_module_id: String,
    },
    MutationKindMismatch(Coordinate),
    QueryKindMismatch(Coordinate),
    InvalidWorkerId {
        module_id: String,
        worker_id: String,
    },
    Empty,
}

impl fmt::Display for CompositionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateModule(module_id) => write!(formatter, "duplicate module contribution {module_id}"),
            Self::DuplicateMutation((id, version)) => {
                write!(formatter, "duplicate mutation route {id}@{version}")
            }
            Self::DuplicateQuery((id, version)) => {
                write!(formatter, "duplicate query route {id}@{version}")
            }
            Self::DuplicateWorker((module_id, worker_id)) => {
                write!(formatter, "duplicate background worker {module_id}/{worker_id}")
            }
            Self::OwnerMismatch {
                module_id,
                capability_id,
                capability_version,
                owner_module_id,
            } => write!(
                formatter,
                "module {module_id} cannot contribute {capability_id}@{capability_version} owned by {owner_module_id}"
            ),
            Self::MutationKindMismatch((id, version)) => {
                write!(formatter, "mutation route {id}@{version} has a query definition")
            }
            Self::QueryKindMismatch((id, version)) => {
                write!(formatter, "query route {id}@{version} has a mutation definition")
            }
            Self::InvalidWorkerId {
                module_id,
                worker_id,
            } => write!(formatter, "invalid background worker id {module_id}/{worker_id}"),
            Self::Empty => formatter.write_str("application composition must declare at least one module"),
        }
    }
}

impl Error for CompositionError {}

#[derive(Debug, Default)]
pub struct ApplicationCompositionBuilder {
    modules: BTreeSet<String>,
    mutations: BTreeMap<Coordinate, MutationRoute>,
    queries: BTreeMap<Coordinate, QueryRoute>,
    workers: BTreeMap<WorkerCoordinate, Arc<dyn TenantBackgroundWorker>>,
}

impl ApplicationCompositionBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_module(
        &mut self,
        contribution: ModuleRuntimeContribution,
    ) -> Result<&mut Self, CompositionError> {
        let module_id = contribution.module_id.as_str().to_owned();
        if !self.modules.insert(module_id.clone()) {
            return Err(CompositionError::DuplicateModule(module_id));
        }

        for route in contribution.mutations {
            validate_owner(&module_id, &route.definition)?;
            let key = coordinate(&route.definition);
            if !route.definition.mutation {
                return Err(CompositionError::MutationKindMismatch(key));
            }
            if self.mutations.insert(key.clone(), route).is_some() {
                return Err(CompositionError::DuplicateMutation(key));
            }
        }

        for route in contribution.queries {
            validate_owner(&module_id, &route.definition)?;
            let key = coordinate(&route.definition);
            if route.definition.mutation
                || route.definition.requires_idempotency
                || route.definition.requires_approval
            {
                return Err(CompositionError::QueryKindMismatch(key));
            }
            if self.queries.insert(key.clone(), route).is_some() {
                return Err(CompositionError::DuplicateQuery(key));
            }
        }

        for contribution in contribution.background_workers {
            if !valid_worker_id(&contribution.worker_id) {
                return Err(CompositionError::InvalidWorkerId {
                    module_id: module_id.clone(),
                    worker_id: contribution.worker_id,
                });
            }
            let key = (module_id.clone(), contribution.worker_id);
            if self.workers.insert(key.clone(), contribution.worker).is_some() {
                return Err(CompositionError::DuplicateWorker(key));
            }
        }
        Ok(self)
    }

    pub fn build(self) -> Result<ApplicationComposition, CompositionError> {
        if self.modules.is_empty() {
            return Err(CompositionError::Empty);
        }

        let mutation_definitions = self
            .mutations
            .values()
            .map(|route| route.definition.clone())
            .collect::<Vec<_>>();
        let query_definitions = self
            .queries
            .values()
            .map(|route| route.definition.clone())
            .collect::<Vec<_>>();

        Ok(ApplicationComposition {
            module_ids: self.modules,
            mutation_definitions,
            query_definitions,
            mutation_registry: Arc::new(DefinitionRegistry::from_mutations(&self.mutations)),
            query_registry: Arc::new(DefinitionRegistry::from_queries(&self.queries)),
            mutation_validator: Arc::new(MutationValidatorRouter {
                routes: Arc::new(self.mutations.clone()),
            }),
            mutation_executor: Arc::new(MutationExecutorRouter {
                routes: Arc::new(self.mutations),
            }),
            query_validator: Arc::new(QueryValidatorRouter {
                routes: Arc::new(self.queries.clone()),
            }),
            query_executor: Arc::new(QueryExecutorRouter {
                routes: Arc::new(self.queries),
            }),
            background_workers: BackgroundWorkerRegistry {
                workers: Arc::new(self.workers),
            },
        })
    }
}

fn validate_owner(module_id: &str, definition: &CapabilityDefinition) -> Result<(), CompositionError> {
    if definition.owner_module_id.as_str() != module_id {
        return Err(CompositionError::OwnerMismatch {
            module_id: module_id.to_owned(),
            capability_id: definition.capability_id.as_str().to_owned(),
            capability_version: definition.capability_version.as_str().to_owned(),
            owner_module_id: definition.owner_module_id.as_str().to_owned(),
        });
    }
    Ok(())
}

fn valid_worker_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 180
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-'))
}

#[derive(Clone)]
pub struct ApplicationComposition {
    module_ids: BTreeSet<String>,
    mutation_definitions: Vec<CapabilityDefinition>,
    query_definitions: Vec<CapabilityDefinition>,
    mutation_registry: Arc<DefinitionRegistry>,
    query_registry: Arc<DefinitionRegistry>,
    mutation_validator: Arc<MutationValidatorRouter>,
    mutation_executor: Arc<MutationExecutorRouter>,
    query_validator: Arc<QueryValidatorRouter>,
    query_executor: Arc<QueryExecutorRouter>,
    background_workers: BackgroundWorkerRegistry,
}

impl fmt::Debug for ApplicationComposition {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ApplicationComposition")
            .field("module_ids", &self.module_ids)
            .field("mutation_definitions", &self.mutation_definitions.len())
            .field("query_definitions", &self.query_definitions.len())
            .field("background_workers", &self.background_workers.len())
            .finish()
    }
}

impl ApplicationComposition {
    pub fn module_ids(&self) -> &BTreeSet<String> {
        &self.module_ids
    }

    pub fn mutation_definitions(&self) -> &[CapabilityDefinition] {
        &self.mutation_definitions
    }

    pub fn query_definitions(&self) -> &[CapabilityDefinition] {
        &self.query_definitions
    }

    pub fn mutation_registry(&self) -> Arc<dyn CapabilityRegistryPort> {
        self.mutation_registry.clone()
    }

    pub fn query_registry(&self) -> Arc<dyn CapabilityRegistryPort> {
        self.query_registry.clone()
    }

    pub fn mutation_validator(&self) -> Arc<dyn CapabilitySemanticValidator> {
        self.mutation_validator.clone()
    }

    pub fn mutation_executor(&self) -> Arc<dyn TransactionalCapabilityExecutor> {
        self.mutation_executor.clone()
    }

    pub fn query_validator(&self) -> Arc<dyn QuerySemanticValidator> {
        self.query_validator.clone()
    }

    pub fn query_executor(&self) -> Arc<dyn QueryExecutor> {
        self.query_executor.clone()
    }

    pub fn background_workers(&self) -> &BackgroundWorkerRegistry {
        &self.background_workers
    }
}

#[derive(Debug, Clone)]
struct DefinitionRegistry {
    definitions: Arc<BTreeMap<Coordinate, CapabilityDefinition>>,
}

impl DefinitionRegistry {
    fn from_mutations(routes: &BTreeMap<Coordinate, MutationRoute>) -> Self {
        Self {
            definitions: Arc::new(
                routes
                    .iter()
                    .map(|(coordinate, route)| (coordinate.clone(), route.definition.clone()))
                    .collect(),
            ),
        }
    }

    fn from_queries(routes: &BTreeMap<Coordinate, QueryRoute>) -> Self {
        Self {
            definitions: Arc::new(
                routes
                    .iter()
                    .map(|(coordinate, route)| (coordinate.clone(), route.definition.clone()))
                    .collect(),
            ),
        }
    }
}

impl CapabilityRegistryPort for DefinitionRegistry {
    fn resolve<'a>(
        &'a self,
        capability_id: &'a CapabilityId,
        capability_version: &'a CapabilityVersion,
    ) -> PortFuture<'a, Result<Option<CapabilityDefinition>, SdkError>> {
        Box::pin(async move {
            Ok(self
                .definitions
                .get(&(
                    capability_id.as_str().to_owned(),
                    capability_version.as_str().to_owned(),
                ))
                .cloned())
        })
    }
}

#[derive(Clone)]
struct MutationValidatorRouter {
    routes: Arc<BTreeMap<Coordinate, MutationRoute>>,
}

impl fmt::Debug for MutationValidatorRouter {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MutationValidatorRouter")
            .field("routes", &self.routes.len())
            .finish()
    }
}

impl CapabilitySemanticValidator for MutationValidatorRouter {
    fn validate<'a>(
        &'a self,
        definition: &'a CapabilityDefinition,
        request: &'a CapabilityRequest,
    ) -> PortFuture<'a, Result<(), SdkError>> {
        let route = self.routes.get(&coordinate(definition)).cloned();
        Box::pin(async move {
            route
                .ok_or_else(route_unavailable)?
                .validator
                .validate(definition, request)
                .await
        })
    }
}

#[derive(Clone)]
struct MutationExecutorRouter {
    routes: Arc<BTreeMap<Coordinate, MutationRoute>>,
}

impl fmt::Debug for MutationExecutorRouter {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MutationExecutorRouter")
            .field("routes", &self.routes.len())
            .finish()
    }
}

impl TransactionalCapabilityExecutor for MutationExecutorRouter {
    fn execute<'a>(
        &'a self,
        definition: &'a CapabilityDefinition,
        request: CapabilityRequest,
    ) -> PortFuture<'a, Result<CapabilityExecutionResult, SdkError>> {
        let route = self.routes.get(&coordinate(definition)).cloned();
        Box::pin(async move {
            route
                .ok_or_else(route_unavailable)?
                .executor
                .execute(definition, request)
                .await
        })
    }
}

#[derive(Clone)]
struct QueryValidatorRouter {
    routes: Arc<BTreeMap<Coordinate, QueryRoute>>,
}

impl fmt::Debug for QueryValidatorRouter {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("QueryValidatorRouter")
            .field("routes", &self.routes.len())
            .finish()
    }
}

impl QuerySemanticValidator for QueryValidatorRouter {
    fn validate<'a>(
        &'a self,
        definition: &'a CapabilityDefinition,
        request: &'a QueryRequest,
    ) -> PortFuture<'a, Result<(), SdkError>> {
        let route = self.routes.get(&coordinate(definition)).cloned();
        Box::pin(async move {
            route
                .ok_or_else(route_unavailable)?
                .validator
                .validate(definition, request)
                .await
        })
    }
}

#[derive(Clone)]
struct QueryExecutorRouter {
    routes: Arc<BTreeMap<Coordinate, QueryRoute>>,
}

impl fmt::Debug for QueryExecutorRouter {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("QueryExecutorRouter")
            .field("routes", &self.routes.len())
            .finish()
    }
}

impl QueryExecutor for QueryExecutorRouter {
    fn execute<'a>(
        &'a self,
        definition: &'a CapabilityDefinition,
        request: QueryRequest,
    ) -> PortFuture<'a, Result<QueryExecutionResult, SdkError>> {
        let route = self.routes.get(&coordinate(definition)).cloned();
        Box::pin(async move {
            route
                .ok_or_else(route_unavailable)?
                .executor
                .execute(definition, request)
                .await
        })
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct NoopMutationSemanticValidator;

impl CapabilitySemanticValidator for NoopMutationSemanticValidator {
    fn validate<'a>(
        &'a self,
        _definition: &'a CapabilityDefinition,
        _request: &'a CapabilityRequest,
    ) -> PortFuture<'a, Result<(), SdkError>> {
        Box::pin(async { Ok(()) })
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct NoopQuerySemanticValidator;

impl QuerySemanticValidator for NoopQuerySemanticValidator {
    fn validate<'a>(
        &'a self,
        _definition: &'a CapabilityDefinition,
        _request: &'a QueryRequest,
    ) -> PortFuture<'a, Result<(), SdkError>> {
        Box::pin(async { Ok(()) })
    }
}

#[derive(Clone)]
pub struct BackgroundWorkerRegistry {
    workers: Arc<BTreeMap<WorkerCoordinate, Arc<dyn TenantBackgroundWorker>>>,
}

impl fmt::Debug for BackgroundWorkerRegistry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BackgroundWorkerRegistry")
            .field("workers", &self.workers.keys().collect::<Vec<_>>())
            .finish()
    }
}

impl BackgroundWorkerRegistry {
    pub fn len(&self) -> usize {
        self.workers.len()
    }

    pub fn is_empty(&self) -> bool {
        self.workers.is_empty()
    }

    pub fn coordinates(&self) -> impl Iterator<Item = (&str, &str)> {
        self.workers
            .keys()
            .map(|(module_id, worker_id)| (module_id.as_str(), worker_id.as_str()))
    }

    pub async fn run_tenant_cycle(
        &self,
        tenant_id: TenantId,
        now_unix_nanos: i64,
    ) -> Result<(), SdkError> {
        if now_unix_nanos <= 0 {
            return Err(composition_invalid("background cycle time is invalid"));
        }
        for ((module_id, worker_id), worker) in self.workers.iter() {
            worker
                .run_tenant_cycle(tenant_id.clone(), now_unix_nanos)
                .await
                .map_err(|error| {
                    SdkError::new(
                        "APPLICATION_BACKGROUND_WORKER_FAILED",
                        ErrorCategory::Unavailable,
                        true,
                        "A background module worker failed.",
                    )
                    .with_internal_reference(format!(
                        "module={module_id};worker={worker_id};error={}",
                        error.code
                    ))
                })?;
        }
        Ok(())
    }
}

fn route_unavailable() -> SdkError {
    composition_invalid("runtime route is unavailable")
}

fn composition_invalid(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "APPLICATION_COMPOSITION_INVALID",
        ErrorCategory::Internal,
        false,
        "The application module composition is invalid.",
    )
    .with_internal_reference(reference)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crm_capability_runtime::{CapabilityRisk, PayloadContract};
    use crm_module_sdk::{
        DataClass, PayloadEncoding, RetentionPolicyId, SchemaId, SchemaVersion, TypedPayload,
    };
    use std::sync::Mutex;

    #[derive(Debug)]
    struct MutationHandler {
        calls: Arc<Mutex<Vec<&'static str>>>,
    }

    impl CapabilitySemanticValidator for MutationHandler {
        fn validate<'a>(
            &'a self,
            _definition: &'a CapabilityDefinition,
            _request: &'a CapabilityRequest,
        ) -> PortFuture<'a, Result<(), SdkError>> {
            Box::pin(async move {
                self.calls.lock().unwrap().push("validate");
                Ok(())
            })
        }
    }

    impl TransactionalCapabilityExecutor for MutationHandler {
        fn execute<'a>(
            &'a self,
            _definition: &'a CapabilityDefinition,
            _request: CapabilityRequest,
        ) -> PortFuture<'a, Result<CapabilityExecutionResult, SdkError>> {
            Box::pin(async move {
                self.calls.lock().unwrap().push("execute");
                Ok(CapabilityExecutionResult { output: None })
            })
        }
    }

    #[derive(Debug)]
    struct QueryHandler;

    impl QuerySemanticValidator for QueryHandler {
        fn validate<'a>(
            &'a self,
            _definition: &'a CapabilityDefinition,
            _request: &'a QueryRequest,
        ) -> PortFuture<'a, Result<(), SdkError>> {
            Box::pin(async { Ok(()) })
        }
    }

    impl QueryExecutor for QueryHandler {
        fn execute<'a>(
            &'a self,
            definition: &'a CapabilityDefinition,
            _request: QueryRequest,
        ) -> PortFuture<'a, Result<QueryExecutionResult, SdkError>> {
            Box::pin(async move {
                Ok(QueryExecutionResult {
                    output: payload(
                        definition.owner_module_id.as_str(),
                        definition.output_contract.as_ref().unwrap(),
                    ),
                })
            })
        }
    }

    #[derive(Debug)]
    struct Worker {
        order: Arc<Mutex<Vec<String>>>,
        name: &'static str,
    }

    impl TenantBackgroundWorker for Worker {
        fn run_tenant_cycle<'a>(
            &'a self,
            _tenant_id: TenantId,
            _now_unix_nanos: i64,
        ) -> PortFuture<'a, Result<(), SdkError>> {
            Box::pin(async move {
                self.order.lock().unwrap().push(self.name.to_owned());
                Ok(())
            })
        }
    }

    #[tokio::test]
    async fn routes_exact_coordinates_without_central_switches() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let handler = Arc::new(MutationHandler {
            calls: Arc::clone(&calls),
        });
        let definition = definition("crm.alpha", "alpha.record.create", true);
        let contribution = ModuleRuntimeContribution::new(module_id("crm.alpha"))
            .with_mutation(MutationRoute {
                definition: definition.clone(),
                validator: handler.clone(),
                executor: handler,
            });
        let mut builder = ApplicationCompositionBuilder::new();
        builder.add_module(contribution).unwrap();
        let composition = builder.build().unwrap();
        composition
            .mutation_validator()
            .validate(&definition, &mutation_request(&definition))
            .await
            .unwrap();
        composition
            .mutation_executor()
            .execute(&definition, mutation_request(&definition))
            .await
            .unwrap();
        assert_eq!(*calls.lock().unwrap(), vec!["validate", "execute"]);
    }

    #[test]
    fn rejects_duplicate_and_owner_mismatched_routes() {
        let handler = Arc::new(MutationHandler {
            calls: Arc::new(Mutex::new(Vec::new())),
        });
        let definition = definition("crm.alpha", "alpha.record.create", true);
        let route = MutationRoute {
            definition: definition.clone(),
            validator: handler.clone(),
            executor: handler.clone(),
        };
        let mut builder = ApplicationCompositionBuilder::new();
        builder
            .add_module(
                ModuleRuntimeContribution::new(module_id("crm.alpha"))
                    .with_mutation(route.clone()),
            )
            .unwrap();
        let duplicate = builder.add_module(
            ModuleRuntimeContribution::new(module_id("crm.beta")).with_mutation(route),
        );
        assert!(matches!(duplicate, Err(CompositionError::OwnerMismatch { .. })));

        let mut builder = ApplicationCompositionBuilder::new();
        let result = builder.add_module(
            ModuleRuntimeContribution::new(module_id("crm.alpha"))
                .with_mutation(MutationRoute {
                    definition: definition.clone(),
                    validator: handler.clone(),
                    executor: handler.clone(),
                })
                .with_mutation(MutationRoute {
                    definition,
                    validator: handler.clone(),
                    executor: handler,
                }),
        );
        assert!(matches!(result, Err(CompositionError::DuplicateMutation(_))));
    }

    #[tokio::test]
    async fn workers_run_in_stable_module_and_worker_order() {
        let order = Arc::new(Mutex::new(Vec::new()));
        let mut builder = ApplicationCompositionBuilder::new();
        builder
            .add_module(
                ModuleRuntimeContribution::new(module_id("crm.zeta"))
                    .with_background_worker(
                        "worker-b",
                        Arc::new(Worker {
                            order: Arc::clone(&order),
                            name: "zeta/b",
                        }),
                    ),
            )
            .unwrap();
        builder
            .add_module(
                ModuleRuntimeContribution::new(module_id("crm.alpha"))
                    .with_background_worker(
                        "worker-a",
                        Arc::new(Worker {
                            order: Arc::clone(&order),
                            name: "alpha/a",
                        }),
                    ),
            )
            .unwrap();
        let composition = builder.build().unwrap();
        composition
            .background_workers()
            .run_tenant_cycle(TenantId::try_new("tenant-a").unwrap(), 1)
            .await
            .unwrap();
        assert_eq!(*order.lock().unwrap(), vec!["alpha/a", "zeta/b"]);
    }

    #[test]
    fn query_and_empty_module_contributions_are_explicit() {
        let definition = definition("crm.read", "read.record.get", false);
        let handler = Arc::new(QueryHandler);
        let mut builder = ApplicationCompositionBuilder::new();
        builder
            .add_module(
                ModuleRuntimeContribution::new(module_id("crm.read")).with_query(QueryRoute {
                    definition,
                    validator: handler.clone(),
                    executor: handler,
                }),
            )
            .unwrap();
        builder
            .add_module(ModuleRuntimeContribution::new(module_id("crm.link")))
            .unwrap();
        let composition = builder.build().unwrap();
        assert_eq!(
            composition.module_ids(),
            &BTreeSet::from(["crm.link".to_owned(), "crm.read".to_owned()])
        );
        assert_eq!(composition.query_definitions().len(), 1);
    }

    fn module_id(value: &str) -> ModuleId {
        ModuleId::try_new(value).unwrap()
    }

    fn definition(owner: &str, id: &str, mutation: bool) -> CapabilityDefinition {
        CapabilityDefinition {
            capability_id: CapabilityId::try_new(id).unwrap(),
            capability_version: CapabilityVersion::try_new("1.0.0").unwrap(),
            owner_module_id: module_id(owner),
            input_contract: contract(owner, format!("{id}.request")),
            output_contract: (!mutation).then(|| contract(owner, format!("{id}.response"))),
            risk: CapabilityRisk::Low,
            mutation,
            requires_idempotency: mutation,
            requires_approval: false,
            authorization_policy_id: id.to_owned(),
            rate_limit_policy_id: None,
        }
    }

    fn contract(owner: &str, schema: String) -> PayloadContract {
        PayloadContract {
            owner: module_id(owner),
            schema_id: SchemaId::try_new(schema).unwrap(),
            schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
            descriptor_hash: [7; 32],
            allowed_data_classes: vec![DataClass::Internal],
            allowed_encodings: vec![PayloadEncoding::Protobuf],
            maximum_size_bytes: 4096,
        }
    }

    fn payload(owner: &str, contract: &PayloadContract) -> TypedPayload {
        TypedPayload {
            owner: module_id(owner),
            schema_id: contract.schema_id.clone(),
            schema_version: contract.schema_version.clone(),
            descriptor_hash: contract.descriptor_hash,
            data_class: DataClass::Internal,
            encoding: PayloadEncoding::Protobuf,
            maximum_size_bytes: contract.maximum_size_bytes,
            retention_policy_id: RetentionPolicyId::try_new("standard").unwrap(),
            bytes: Vec::new(),
        }
    }

    fn mutation_request(definition: &CapabilityDefinition) -> CapabilityRequest {
        use crm_module_sdk::{
            ActorId, BusinessTransactionId, CausationId, CorrelationId, ExecutionContext,
            IdempotencyKey, ModuleExecutionContext, RequestId, TraceId,
        };
        CapabilityRequest {
            context: ModuleExecutionContext {
                module_id: definition.owner_module_id.clone(),
                execution: ExecutionContext {
                    tenant_id: TenantId::try_new("tenant-a").unwrap(),
                    actor_id: ActorId::try_new("actor-a").unwrap(),
                    request_id: RequestId::try_new("request-a").unwrap(),
                    correlation_id: CorrelationId::try_new("correlation-a").unwrap(),
                    causation_id: CausationId::try_new("causation-a").unwrap(),
                    trace_id: TraceId::try_new("trace-a").unwrap(),
                    capability_id: definition.capability_id.clone(),
                    capability_version: definition.capability_version.clone(),
                    idempotency_key: IdempotencyKey::try_new("idem-a").unwrap(),
                    business_transaction_id: BusinessTransactionId::try_new("tx-a").unwrap(),
                    schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
                    request_started_at_unix_nanos: 1,
                },
            },
            input: payload(&definition.owner_module_id.to_string(), &definition.input_contract),
            input_hash: [1; 32],
            approval: None,
        }
    }
}
