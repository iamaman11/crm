#![forbid(unsafe_code)]

//! Deterministic first-party module contribution runtime.
//!
//! This crate contains composition mechanics only. It knows no concrete CRM
//! module, database, transport or process host. Exact versioned routes and
//! background workers are contributed explicitly and fail closed at assembly.

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

#[derive(Debug, Clone)]
pub struct ModuleRuntimeContribution {
    pub module_id: ModuleId,
    pub mutations: Vec<MutationRoute>,
    pub queries: Vec<QueryRoute>,
}

impl ModuleRuntimeContribution {
    pub fn new(module_id: ModuleId) -> Self {
        Self {
            module_id,
            mutations: Vec::new(),
            queries: Vec::new(),
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
}

/// Collects route fragments by their declared owner and emits one deterministic
/// contribution per module. Production composition roots use this when module
/// constructors become available in dependency order while preserving the
/// invariant that `ApplicationCompositionBuilder` sees each module exactly once.
#[derive(Default)]
pub struct ModuleContributionSet {
    modules: BTreeMap<String, ModuleRuntimeContribution>,
}

impl fmt::Debug for ModuleContributionSet {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ModuleContributionSet")
            .field("module_ids", &self.modules.keys().collect::<Vec<_>>())
            .finish()
    }
}

impl ModuleContributionSet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_mutations(
        &mut self,
        definitions: impl IntoIterator<Item = CapabilityDefinition>,
        validator: Arc<dyn CapabilitySemanticValidator>,
        executor: Arc<dyn TransactionalCapabilityExecutor>,
    ) -> Result<&mut Self, CompositionError> {
        for definition in definitions {
            let module_id = definition.owner_module_id.clone();
            let key = module_id.as_str().to_owned();
            let contribution = self
                .modules
                .entry(key)
                .or_insert_with(|| ModuleRuntimeContribution::new(module_id));
            contribution.mutations.push(MutationRoute {
                definition,
                validator: validator.clone(),
                executor: executor.clone(),
            });
        }
        Ok(self)
    }

    pub fn add_queries(
        &mut self,
        definitions: impl IntoIterator<Item = CapabilityDefinition>,
        validator: Arc<dyn QuerySemanticValidator>,
        executor: Arc<dyn QueryExecutor>,
    ) -> Result<&mut Self, CompositionError> {
        for definition in definitions {
            let module_id = definition.owner_module_id.clone();
            let key = module_id.as_str().to_owned();
            let contribution = self
                .modules
                .entry(key)
                .or_insert_with(|| ModuleRuntimeContribution::new(module_id));
            contribution.queries.push(QueryRoute {
                definition,
                validator: validator.clone(),
                executor: executor.clone(),
            });
        }
        Ok(self)
    }

    pub fn add_empty_module(&mut self, module_id: ModuleId) -> Result<&mut Self, CompositionError> {
        let key = module_id.as_str().to_owned();
        if self.modules.contains_key(&key) {
            return Ok(self);
        }
        self.modules
            .insert(key, ModuleRuntimeContribution::new(module_id));
        Ok(self)
    }

    pub fn build(self) -> Result<ApplicationComposition, CompositionError> {
        let mut builder = ApplicationCompositionBuilder::new();
        for contribution in self.modules.into_values() {
            builder.add_module(contribution)?;
        }
        builder.build()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompositionError {
    DuplicateModule(String),
    DuplicateMutation(Coordinate),
    DuplicateQuery(Coordinate),
    OwnerMismatch {
        module_id: String,
        capability_id: String,
        capability_version: String,
        owner_module_id: String,
    },
    MutationKindMismatch(Coordinate),
    QueryKindMismatch(Coordinate),
    Empty,
}

impl fmt::Display for CompositionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateModule(module_id) => {
                write!(formatter, "duplicate module contribution {module_id}")
            }
            Self::DuplicateMutation((id, version)) => {
                write!(formatter, "duplicate mutation route {id}@{version}")
            }
            Self::DuplicateQuery((id, version)) => {
                write!(formatter, "duplicate query route {id}@{version}")
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
                write!(
                    formatter,
                    "mutation route {id}@{version} has a query definition"
                )
            }
            Self::QueryKindMismatch((id, version)) => {
                write!(
                    formatter,
                    "query route {id}@{version} has a mutation definition"
                )
            }
            Self::Empty => formatter.write_str("application composition declares no modules"),
        }
    }
}

impl Error for CompositionError {}

#[derive(Default)]
pub struct ApplicationCompositionBuilder {
    modules: BTreeSet<String>,
    mutations: BTreeMap<Coordinate, MutationRoute>,
    queries: BTreeMap<Coordinate, QueryRoute>,
}

impl fmt::Debug for ApplicationCompositionBuilder {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ApplicationCompositionBuilder")
            .field("modules", &self.modules)
            .field("mutation_count", &self.mutations.len())
            .field("query_count", &self.queries.len())
            .finish()
    }
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
            .collect();
        let query_definitions = self
            .queries
            .values()
            .map(|route| route.definition.clone())
            .collect();
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
        })
    }
}

fn validate_owner(
    module_id: &str,
    definition: &CapabilityDefinition,
) -> Result<(), CompositionError> {
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
}

impl fmt::Debug for ApplicationComposition {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ApplicationComposition")
            .field("module_ids", &self.module_ids)
            .field("mutation_count", &self.mutation_definitions.len())
            .field("query_count", &self.query_definitions.len())
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
                    .map(|(key, route)| (key.clone(), route.definition.clone()))
                    .collect(),
            ),
        }
    }

    fn from_queries(routes: &BTreeMap<Coordinate, QueryRoute>) -> Self {
        Self {
            definitions: Arc::new(
                routes
                    .iter()
                    .map(|(key, route)| (key.clone(), route.definition.clone()))
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
            .field("route_count", &self.routes.len())
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
            .field("route_count", &self.routes.len())
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
            .field("route_count", &self.routes.len())
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
            .field("route_count", &self.routes.len())
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

pub trait ModuleActivationPort: Send + Sync {
    fn is_active<'a>(
        &'a self,
        tenant_id: &'a TenantId,
        module_id: &'a ModuleId,
    ) -> PortFuture<'a, Result<bool, SdkError>>;
}

#[derive(Clone)]
pub struct ActivationGatedMutationValidator {
    activation: Arc<dyn ModuleActivationPort>,
    inner: Arc<dyn CapabilitySemanticValidator>,
}

impl fmt::Debug for ActivationGatedMutationValidator {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ActivationGatedMutationValidator")
            .field("activation", &"dyn ModuleActivationPort")
            .field("inner", &"dyn CapabilitySemanticValidator")
            .finish()
    }
}

impl ActivationGatedMutationValidator {
    pub fn new(
        activation: Arc<dyn ModuleActivationPort>,
        inner: Arc<dyn CapabilitySemanticValidator>,
    ) -> Self {
        Self { activation, inner }
    }
}

impl CapabilitySemanticValidator for ActivationGatedMutationValidator {
    fn validate<'a>(
        &'a self,
        definition: &'a CapabilityDefinition,
        request: &'a CapabilityRequest,
    ) -> PortFuture<'a, Result<(), SdkError>> {
        Box::pin(async move {
            if !self
                .activation
                .is_active(
                    &request.context.execution.tenant_id,
                    &definition.owner_module_id,
                )
                .await?
            {
                return Err(module_not_active(&definition.owner_module_id));
            }
            self.inner.validate(definition, request).await
        })
    }
}

#[derive(Clone)]
pub struct ActivationGatedQueryValidator {
    activation: Arc<dyn ModuleActivationPort>,
    inner: Arc<dyn QuerySemanticValidator>,
}

impl fmt::Debug for ActivationGatedQueryValidator {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ActivationGatedQueryValidator")
            .field("activation", &"dyn ModuleActivationPort")
            .field("inner", &"dyn QuerySemanticValidator")
            .finish()
    }
}

impl ActivationGatedQueryValidator {
    pub fn new(
        activation: Arc<dyn ModuleActivationPort>,
        inner: Arc<dyn QuerySemanticValidator>,
    ) -> Self {
        Self { activation, inner }
    }
}

impl QuerySemanticValidator for ActivationGatedQueryValidator {
    fn validate<'a>(
        &'a self,
        definition: &'a CapabilityDefinition,
        request: &'a QueryRequest,
    ) -> PortFuture<'a, Result<(), SdkError>> {
        Box::pin(async move {
            if !self
                .activation
                .is_active(&request.context.tenant_id, &definition.owner_module_id)
                .await?
            {
                return Err(module_not_active(&definition.owner_module_id));
            }
            self.inner.validate(definition, request).await
        })
    }
}

pub trait TenantBackgroundWorker: Send + Sync {
    fn run_tenant_cycle<'a>(
        &'a self,
        tenant_id: TenantId,
        now_unix_nanos: i64,
    ) -> PortFuture<'a, Result<(), SdkError>>;
}

/// Stable process-wide ordering for background work. Modules choose a phase,
/// while module and worker identifiers provide deterministic ordering inside it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BackgroundWorkerPhase(u16);

impl BackgroundWorkerPhase {
    pub const SOURCE_INGESTION: Self = Self(100);
    pub const DOMAIN_LINKING: Self = Self(200);
    pub const PROJECTION: Self = Self(300);
    pub const DERIVED_VIEW: Self = Self(400);
    pub const SEARCH_INDEX: Self = Self(500);
    pub const DEFAULT: Self = Self(1_000);

    pub const fn new(order: u16) -> Self {
        Self(order)
    }

    pub const fn order(self) -> u16 {
        self.0
    }
}

#[derive(Clone)]
pub struct ActivationGatedBackgroundWorker {
    activation: Arc<dyn ModuleActivationPort>,
    module_id: ModuleId,
    inner: Arc<dyn TenantBackgroundWorker>,
}

impl fmt::Debug for ActivationGatedBackgroundWorker {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ActivationGatedBackgroundWorker")
            .field("module_id", &self.module_id)
            .field("activation", &"dyn ModuleActivationPort")
            .field("inner", &"dyn TenantBackgroundWorker")
            .finish()
    }
}

impl ActivationGatedBackgroundWorker {
    pub fn new(
        activation: Arc<dyn ModuleActivationPort>,
        module_id: ModuleId,
        inner: Arc<dyn TenantBackgroundWorker>,
    ) -> Self {
        Self {
            activation,
            module_id,
            inner,
        }
    }
}

impl TenantBackgroundWorker for ActivationGatedBackgroundWorker {
    fn run_tenant_cycle<'a>(
        &'a self,
        tenant_id: TenantId,
        now_unix_nanos: i64,
    ) -> PortFuture<'a, Result<(), SdkError>> {
        Box::pin(async move {
            if !self
                .activation
                .is_active(&tenant_id, &self.module_id)
                .await?
            {
                return Ok(());
            }
            self.inner.run_tenant_cycle(tenant_id, now_unix_nanos).await
        })
    }
}

type WorkerIdentity = (String, String);
type ScheduledWorkerCoordinate = (BackgroundWorkerPhase, String, String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackgroundCompositionError {
    UndeclaredModule(String),
    DuplicateWorker(WorkerIdentity),
    InvalidWorkerId(WorkerIdentity),
}

impl fmt::Display for BackgroundCompositionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UndeclaredModule(module_id) => {
                write!(
                    formatter,
                    "background worker owner {module_id} is undeclared"
                )
            }
            Self::DuplicateWorker((module_id, worker_id)) => {
                write!(
                    formatter,
                    "duplicate background worker {module_id}/{worker_id}"
                )
            }
            Self::InvalidWorkerId((module_id, worker_id)) => {
                write!(
                    formatter,
                    "invalid background worker id {module_id}/{worker_id}"
                )
            }
        }
    }
}

impl Error for BackgroundCompositionError {}

pub struct BackgroundWorkerRegistryBuilder {
    modules: BTreeSet<String>,
    worker_identities: BTreeSet<WorkerIdentity>,
    workers: BTreeMap<ScheduledWorkerCoordinate, Arc<dyn TenantBackgroundWorker>>,
}

impl fmt::Debug for BackgroundWorkerRegistryBuilder {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BackgroundWorkerRegistryBuilder")
            .field("modules", &self.modules)
            .field("workers", &self.workers.keys().collect::<Vec<_>>())
            .finish()
    }
}

impl BackgroundWorkerRegistryBuilder {
    pub fn new(modules: impl IntoIterator<Item = String>) -> Self {
        Self {
            modules: modules.into_iter().collect(),
            worker_identities: BTreeSet::new(),
            workers: BTreeMap::new(),
        }
    }

    pub fn add(
        &mut self,
        module_id: ModuleId,
        worker_id: impl Into<String>,
        worker: Arc<dyn TenantBackgroundWorker>,
    ) -> Result<&mut Self, BackgroundCompositionError> {
        self.add_in_phase(BackgroundWorkerPhase::DEFAULT, module_id, worker_id, worker)
    }

    pub fn add_in_phase(
        &mut self,
        phase: BackgroundWorkerPhase,
        module_id: ModuleId,
        worker_id: impl Into<String>,
        worker: Arc<dyn TenantBackgroundWorker>,
    ) -> Result<&mut Self, BackgroundCompositionError> {
        let module_id = module_id.as_str().to_owned();
        let worker_id = worker_id.into();
        if !self.modules.contains(&module_id) {
            return Err(BackgroundCompositionError::UndeclaredModule(module_id));
        }
        let identity = (module_id.clone(), worker_id.clone());
        if !valid_worker_id(&worker_id) {
            return Err(BackgroundCompositionError::InvalidWorkerId(identity));
        }
        if !self.worker_identities.insert(identity.clone()) {
            return Err(BackgroundCompositionError::DuplicateWorker(identity));
        }
        self.workers.insert((phase, module_id, worker_id), worker);
        Ok(self)
    }

    pub fn build(self) -> BackgroundWorkerRegistry {
        BackgroundWorkerRegistry {
            workers: Arc::new(self.workers),
        }
    }
}

fn valid_worker_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 180
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-')
        })
}

#[derive(Clone)]
pub struct BackgroundWorkerRegistry {
    workers: Arc<BTreeMap<ScheduledWorkerCoordinate, Arc<dyn TenantBackgroundWorker>>>,
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
            .map(|(_, module_id, worker_id)| (module_id.as_str(), worker_id.as_str()))
    }

    pub fn scheduled_coordinates(
        &self,
    ) -> impl Iterator<Item = (BackgroundWorkerPhase, &str, &str)> {
        self.workers
            .keys()
            .map(|(phase, module_id, worker_id)| (*phase, module_id.as_str(), worker_id.as_str()))
    }

    pub async fn run_tenant_cycle(
        &self,
        tenant_id: TenantId,
        now_unix_nanos: i64,
    ) -> Result<(), SdkError> {
        if now_unix_nanos <= 0 {
            return Err(composition_invalid("background cycle time is invalid"));
        }
        for ((phase, module_id, worker_id), worker) in self.workers.iter() {
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
                        "phase={};module={module_id};worker={worker_id};error={}",
                        phase.order(),
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

fn module_not_active(module_id: &ModuleId) -> SdkError {
    SdkError::new(
        "MODULE_NOT_ACTIVE",
        ErrorCategory::Conflict,
        false,
        "The requested module is not active for this tenant.",
    )
    .with_internal_reference(module_id.as_str())
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
        ActorId, BusinessTransactionId, CausationId, CorrelationId, DataClass, ExecutionContext,
        IdempotencyKey, ModuleExecutionContext, PayloadEncoding, RequestId, RetentionPolicyId,
        SchemaId, SchemaVersion, TraceId, TypedPayload,
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
                Ok(CapabilityExecutionResult {
                    output: None,
                    affected_resources: Vec::new(),
                    replayed: false,
                })
            })
        }
    }

    #[derive(Debug)]
    struct Activation(bool);

    impl ModuleActivationPort for Activation {
        fn is_active<'a>(
            &'a self,
            _tenant_id: &'a TenantId,
            _module_id: &'a ModuleId,
        ) -> PortFuture<'a, Result<bool, SdkError>> {
            Box::pin(async move { Ok(self.0) })
        }
    }

    #[derive(Debug)]
    struct Worker {
        order: Arc<Mutex<Vec<String>>>,
        value: &'static str,
    }

    impl TenantBackgroundWorker for Worker {
        fn run_tenant_cycle<'a>(
            &'a self,
            _tenant_id: TenantId,
            _now_unix_nanos: i64,
        ) -> PortFuture<'a, Result<(), SdkError>> {
            Box::pin(async move {
                self.order.lock().unwrap().push(self.value.to_owned());
                Ok(())
            })
        }
    }

    #[tokio::test]
    async fn exact_routes_do_not_need_central_capability_switches() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let handler = Arc::new(MutationHandler {
            calls: Arc::clone(&calls),
        });
        let definition = definition("crm.alpha", "alpha.record.create", true);
        let mut builder = ApplicationCompositionBuilder::new();
        builder
            .add_module(
                ModuleRuntimeContribution::new(module_id("crm.alpha")).with_mutation(
                    MutationRoute {
                        definition: definition.clone(),
                        validator: handler.clone(),
                        executor: handler,
                    },
                ),
            )
            .unwrap();
        let composition = builder.build().unwrap();
        let request = request(&definition);
        composition
            .mutation_validator()
            .validate(&definition, &request)
            .await
            .unwrap();
        composition
            .mutation_executor()
            .execute(&definition, request)
            .await
            .unwrap();
        assert_eq!(*calls.lock().unwrap(), vec!["validate", "execute"]);
    }

    #[tokio::test]
    async fn inactive_module_fails_before_inner_semantic_validation() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let inner = Arc::new(MutationHandler {
            calls: Arc::clone(&calls),
        });
        let validator = ActivationGatedMutationValidator::new(Arc::new(Activation(false)), inner);
        let definition = definition("crm.alpha", "alpha.record.create", true);
        let error = validator
            .validate(&definition, &request(&definition))
            .await
            .unwrap_err();
        assert_eq!(error.code, "MODULE_NOT_ACTIVE");
        assert!(calls.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn background_workers_run_in_stable_coordinate_order() {
        let order = Arc::new(Mutex::new(Vec::new()));
        let modules = BTreeSet::from(["crm.alpha".to_owned(), "crm.zeta".to_owned()]);
        let mut builder = BackgroundWorkerRegistryBuilder::new(modules);
        builder
            .add(
                module_id("crm.zeta"),
                "worker-b",
                Arc::new(Worker {
                    order: Arc::clone(&order),
                    value: "zeta/b",
                }),
            )
            .unwrap();
        builder
            .add(
                module_id("crm.alpha"),
                "worker-a",
                Arc::new(Worker {
                    order: Arc::clone(&order),
                    value: "alpha/a",
                }),
            )
            .unwrap();
        builder
            .build()
            .run_tenant_cycle(TenantId::try_new("tenant-a").unwrap(), 1)
            .await
            .unwrap();
        assert_eq!(*order.lock().unwrap(), vec!["alpha/a", "zeta/b"]);
    }

    #[tokio::test]
    async fn background_worker_phases_precede_module_sort_order() {
        let order = Arc::new(Mutex::new(Vec::new()));
        let modules = BTreeSet::from(["crm.alpha".to_owned(), "crm.zeta".to_owned()]);
        let mut builder = BackgroundWorkerRegistryBuilder::new(modules);
        builder
            .add_in_phase(
                BackgroundWorkerPhase::SEARCH_INDEX,
                module_id("crm.alpha"),
                "search",
                Arc::new(Worker {
                    order: Arc::clone(&order),
                    value: "search",
                }),
            )
            .unwrap();
        builder
            .add_in_phase(
                BackgroundWorkerPhase::SOURCE_INGESTION,
                module_id("crm.zeta"),
                "ingestion",
                Arc::new(Worker {
                    order: Arc::clone(&order),
                    value: "ingestion",
                }),
            )
            .unwrap();
        builder
            .build()
            .run_tenant_cycle(TenantId::try_new("tenant-a").unwrap(), 1)
            .await
            .unwrap();
        assert_eq!(*order.lock().unwrap(), vec!["ingestion", "search"]);
    }

    #[tokio::test]
    async fn inactive_background_worker_is_skipped_before_inner_execution() {
        let order = Arc::new(Mutex::new(Vec::new()));
        let gated = ActivationGatedBackgroundWorker::new(
            Arc::new(Activation(false)),
            module_id("crm.alpha"),
            Arc::new(Worker {
                order: Arc::clone(&order),
                value: "should-not-run",
            }),
        );
        gated
            .run_tenant_cycle(TenantId::try_new("tenant-a").unwrap(), 1)
            .await
            .unwrap();
        assert!(order.lock().unwrap().is_empty());
    }

    #[test]
    fn contribution_set_merges_route_fragments_without_duplicate_module_registration() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let handler = Arc::new(MutationHandler {
            calls: Arc::clone(&calls),
        });
        let mut set = ModuleContributionSet::new();
        set.add_mutations(
            [definition("crm.alpha", "alpha.record.create", true)],
            handler.clone(),
            handler.clone(),
        )
        .unwrap();
        set.add_mutations(
            [definition("crm.alpha", "alpha.record.update", true)],
            handler.clone(),
            handler,
        )
        .unwrap();
        let composition = set.build().unwrap();
        assert_eq!(
            composition.module_ids(),
            &BTreeSet::from(["crm.alpha".to_owned()])
        );
        assert_eq!(composition.mutation_definitions().len(), 2);
    }

    #[test]
    fn duplicate_and_owner_mismatched_routes_fail_assembly() {
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
        let error = builder
            .add_module(
                ModuleRuntimeContribution::new(module_id("crm.beta")).with_mutation(route.clone()),
            )
            .unwrap_err();
        assert!(matches!(error, CompositionError::OwnerMismatch { .. }));

        let mut builder = ApplicationCompositionBuilder::new();
        let error = builder
            .add_module(
                ModuleRuntimeContribution::new(module_id("crm.alpha"))
                    .with_mutation(route.clone())
                    .with_mutation(route),
            )
            .unwrap_err();
        assert!(matches!(error, CompositionError::DuplicateMutation(_)));
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
            output_contract: None,
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

    fn payload(definition: &CapabilityDefinition) -> TypedPayload {
        TypedPayload {
            owner: definition.input_contract.owner.clone(),
            schema_id: definition.input_contract.schema_id.clone(),
            schema_version: definition.input_contract.schema_version.clone(),
            descriptor_hash: definition.input_contract.descriptor_hash,
            data_class: DataClass::Internal,
            encoding: PayloadEncoding::Protobuf,
            maximum_size_bytes: definition.input_contract.maximum_size_bytes,
            retention_policy_id: RetentionPolicyId::try_new("standard").unwrap(),
            bytes: Vec::new(),
        }
    }

    fn request(definition: &CapabilityDefinition) -> CapabilityRequest {
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
            input: payload(definition),
            input_hash: [1; 32],
            approval: None,
        }
    }
}
