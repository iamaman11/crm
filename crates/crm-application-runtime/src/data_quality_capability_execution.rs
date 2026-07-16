use crate::governed_metadata::ApplicationCapabilityExecutorRouter as BaseApplicationCapabilityExecutorRouter;
use crm_capability_runtime::{
    CapabilityAuthorizer, CapabilityDefinition, CapabilityExecutionResult, CapabilityRequest,
    TransactionalCapabilityExecutor,
};
use crm_core_data::{
    PostgresDataStore, PostgresMetadataCapabilityExecutor, PostgresTransactionalAggregateExecutor,
    RecordGetQuery,
};
use crm_data_quality_capability_adapter::{
    DataQualityCompletenessProfileCapabilityPlanner, DataQualityEvaluationJobCapabilityPlanner,
    MODULE_ID, PARTY_COMPLETENESS_PROFILE_VERSION_RECORD_TYPE,
    PARTY_RULE_SET_VERSION_RECORD_TYPE, PUBLISH_PARTY_COMPLETENESS_PROFILE_CAPABILITY,
    REQUEST_PARTY_EVALUATION_CAPABILITY, completeness_profile_reference_scope_from_request,
    evaluation_reference_scope_from_request, party_completeness_profile_from_immutable_snapshot,
    party_rule_set_from_snapshot,
};
use crm_module_sdk::{ErrorCategory, ModuleId, PortFuture, RecordId, RecordType, SdkError};
use std::fmt;
use std::sync::Arc;

#[derive(Clone)]
pub struct ApplicationCapabilityExecutorRouter {
    store: PostgresDataStore,
    base: BaseApplicationCapabilityExecutorRouter,
}

impl ApplicationCapabilityExecutorRouter {
    pub fn new(
        store: PostgresDataStore,
        aggregate: Arc<PostgresTransactionalAggregateExecutor>,
        metadata: Arc<PostgresMetadataCapabilityExecutor>,
        authorizer: Arc<dyn CapabilityAuthorizer>,
    ) -> Self {
        let base = BaseApplicationCapabilityExecutorRouter::new(
            store.clone(),
            aggregate,
            metadata,
            authorizer,
        );
        Self { store, base }
    }
}

impl fmt::Debug for ApplicationCapabilityExecutorRouter {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ApplicationCapabilityExecutorRouter")
            .field("store", &self.store)
            .field("base", &self.base)
            .finish()
    }
}

impl TransactionalCapabilityExecutor for ApplicationCapabilityExecutorRouter {
    fn execute<'a>(
        &'a self,
        definition: &'a CapabilityDefinition,
        request: CapabilityRequest,
    ) -> PortFuture<'a, Result<CapabilityExecutionResult, SdkError>> {
        match definition.capability_id.as_str() {
            PUBLISH_PARTY_COMPLETENESS_PROFILE_CAPABILITY => Box::pin(async move {
                let scope = completeness_profile_reference_scope_from_request(&request)?;
                let snapshot = self
                    .store
                    .get_record_for_query(&RecordGetQuery {
                        tenant_id: request.context.execution.tenant_id.clone(),
                        owner_module_id: module_id()?,
                        record_type: record_type(PARTY_RULE_SET_VERSION_RECORD_TYPE)?,
                        record_id: RecordId::try_new(scope.rule_set_version_id)
                            .map_err(|_| rule_set_unavailable())?,
                    })
                    .await?
                    .ok_or_else(rule_set_unavailable)?;
                let rule_set = party_rule_set_from_snapshot(&snapshot)?;
                PostgresTransactionalAggregateExecutor::new(
                    self.store.clone(),
                    Arc::new(DataQualityCompletenessProfileCapabilityPlanner::new(
                        rule_set,
                    )),
                )
                .execute(definition, request)
                .await
            }),
            REQUEST_PARTY_EVALUATION_CAPABILITY => Box::pin(async move {
                let scope = evaluation_reference_scope_from_request(&request)?;
                let rule_set_snapshot = self
                    .store
                    .get_record_for_query(&RecordGetQuery {
                        tenant_id: request.context.execution.tenant_id.clone(),
                        owner_module_id: module_id()?,
                        record_type: record_type(PARTY_RULE_SET_VERSION_RECORD_TYPE)?,
                        record_id: scope.rule_set_version_id,
                    })
                    .await?
                    .ok_or_else(evaluation_definitions_unavailable)?;
                let rule_set = party_rule_set_from_snapshot(&rule_set_snapshot)?;
                let profile_snapshot = self
                    .store
                    .get_record_for_query(&RecordGetQuery {
                        tenant_id: request.context.execution.tenant_id.clone(),
                        owner_module_id: module_id()?,
                        record_type: record_type(
                            PARTY_COMPLETENESS_PROFILE_VERSION_RECORD_TYPE,
                        )?,
                        record_id: scope.profile_version_id,
                    })
                    .await?
                    .ok_or_else(evaluation_definitions_unavailable)?;
                let profile = party_completeness_profile_from_immutable_snapshot(
                    &profile_snapshot,
                    &rule_set,
                )?;
                PostgresTransactionalAggregateExecutor::new(
                    self.store.clone(),
                    Arc::new(DataQualityEvaluationJobCapabilityPlanner::new(
                        rule_set, profile,
                    )?),
                )
                .execute(definition, request)
                .await
            }),
            _ => self.base.execute(definition, request),
        }
    }
}

fn module_id() -> Result<ModuleId, SdkError> {
    ModuleId::try_new(MODULE_ID).map_err(reference_configuration_error)
}

fn record_type(value: &str) -> Result<RecordType, SdkError> {
    RecordType::try_new(value).map_err(reference_configuration_error)
}

fn rule_set_unavailable() -> SdkError {
    SdkError::new(
        "DATA_QUALITY_COMPLETENESS_RULE_SET_UNAVAILABLE",
        ErrorCategory::InvalidArgument,
        false,
        "The referenced Party rule-set version is unavailable.",
    )
}

fn evaluation_definitions_unavailable() -> SdkError {
    SdkError::new(
        "DATA_QUALITY_EVALUATION_DEFINITIONS_UNAVAILABLE",
        ErrorCategory::InvalidArgument,
        false,
        "The referenced Party evaluation definitions are unavailable.",
    )
}

fn reference_configuration_error(error: crm_module_sdk::IdentifierError) -> SdkError {
    SdkError::new(
        "DATA_QUALITY_REFERENCE_CONFIGURATION_INVALID",
        ErrorCategory::Internal,
        false,
        "The Data Quality reference boundary is not configured safely.",
    )
    .with_internal_reference(error.to_string())
}
