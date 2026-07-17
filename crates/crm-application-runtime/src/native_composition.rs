use crate::{DataQualityAggregatePlanner, DataQualityCapabilityExecutor};
use crm_application_composition::{
    ActivationGatedMutationValidator, ActivationGatedQueryValidator, ApplicationComposition,
    ModuleActivationPort, ModuleContributionSet, NoopMutationSemanticValidator,
};
use crm_capability_runtime::{
    CapabilityAuthorizer, CapabilityDefinition, CapabilitySemanticValidator,
    TransactionalCapabilityExecutor,
};
use crm_consents_capability_adapter::{
    ConsentCapabilityPlanner, capability_definitions as consent_capability_definitions,
};
use crm_consents_capability_composition::{
    ConsentCapabilityExecutor, ConsentCapabilitySemanticValidator, PostgresConsentReferenceReader,
};
use crm_consents_query_adapter::{
    ConsentQueryAdapter, query_capability_definitions as consent_query_capability_definitions,
};
use crm_contact_points_capability_adapter::{
    ContactPointCapabilityPlanner, capability_definitions as contact_point_capability_definitions,
};
use crm_contact_points_capability_composition::ContactPointPartyReferenceSemanticValidator;
use crm_contact_points_query_adapter::{
    ContactPointQueryAdapter,
    query_capability_definitions as contact_point_query_capability_definitions,
};
use crm_core_data::{
    PostgresDataStore, PostgresImmutableFileArtifactStore, PostgresMetadataCapabilityExecutor,
    PostgresMetadataQueryStore, PostgresTransactionalAggregateExecutor,
    TransactionalAggregatePlanner,
};
use crm_customer_360_query_adapter::{
    Customer360QueryAdapter,
    query_capability_definitions as customer_360_query_capability_definitions,
};
use crm_customer_accounts_capability_adapter::{
    CustomerAccountCapabilityPlanner, capability_definitions as account_capability_definitions,
};
use crm_customer_accounts_capability_composition::AccountPartyReferenceSemanticValidator;
use crm_customer_accounts_query_adapter::{
    AccountQueryAdapter, query_capability_definitions as account_query_capability_definitions,
};
use crm_customer_data_operations_capability_adapter::{
    CREATE_PARTY_IMPORT_JOB_CAPABILITY, CustomerDataOperationsCapabilityPlanner,
    VALIDATE_PARTY_IMPORT_ROWS_CAPABILITY,
    capability_definitions as customer_data_operations_capability_definitions,
};
use crm_customer_data_operations_query_adapter::{
    CustomerDataOperationsQueryAdapter,
    query_capability_definitions as customer_data_operations_query_capability_definitions,
};
use crm_customer_data_operations_source_composition::{
    CustomerDataOperationsSourceExecutor,
    source_capability_definitions as customer_data_operations_source_capability_definitions,
};
use crm_data_quality_capability_adapter::capability_definitions as data_quality_capability_definitions;
use crm_data_quality_query_adapter::{
    DataQualityQueryAdapter,
    query_capability_definitions as data_quality_query_capability_definitions,
};
use crm_global_search_composition::GLOBAL_SEARCH_INDEX_ID;
use crm_identity_resolution_capability_adapter::{
    CANDIDATE_MUTATION_CAPABILITY_IDS, IdentityResolutionCapabilityPlanner,
    MERGE_MUTATION_CAPABILITY_IDS,
    capability_definitions as identity_resolution_capability_definitions,
};
use crm_identity_resolution_capability_composition::{
    IdentityResolutionCapabilityExecutor, IdentityResolutionCapabilitySemanticValidator,
    PostgresIdentityResolutionReferenceReader,
};
use crm_identity_resolution_merge_composition::{
    MergeLineageCapabilityExecutor, MergeLineageCapabilitySemanticValidator,
    PostgresMergeLineageReferenceReader,
};
use crm_identity_resolution_merge_query_adapter::{
    IdentityResolutionMergeQueryAdapter,
    query_capability_definitions as identity_resolution_merge_query_capability_definitions,
};
use crm_identity_resolution_query_adapter::{
    IdentityResolutionQueryAdapter,
    query_capability_definitions as identity_resolution_query_capability_definitions,
};
use crm_metadata_api_adapter::{
    metadata_mutation_capability_definitions, metadata_query_capability_definitions,
};
use crm_metadata_query_adapter::MetadataQueryAdapter;
use crm_module_sdk::{ErrorCategory, ModuleId, PortFuture, SdkError, TenantId};
use crm_parties_capability_adapter::{
    PartyCapabilityPlanner, capability_definitions as party_capability_definitions,
};
use crm_parties_query_adapter::{
    PartyQueryAdapter, query_capability_definitions as party_query_capability_definitions,
};
use crm_party_reference_composition::PostgresPartyReferenceReader;
use crm_party_relationships_capability_adapter::{
    PartyRelationshipCapabilityPlanner,
    capability_definitions as party_relationship_capability_definitions,
};
use crm_party_relationships_capability_composition::PartyRelationshipReferenceSemanticValidator;
use crm_party_relationships_query_adapter::{
    PartyRelationshipQueryAdapter,
    query_capability_definitions as party_relationship_query_capability_definitions,
};
use crm_query_runtime::{
    CursorCodec, QueryAuthorizer, QueryExecutor, QuerySemanticValidator, QueryVisibilityAuthorizer,
};
use crm_sales_activities_capability_composition::{
    SalesActivitiesCapabilityPlannerRouter,
    capability_definitions as sales_activities_capability_definitions,
};
use crm_sales_activities_link::MODULE_ID as LINK_MODULE_ID;
use crm_sales_activities_query_adapter::{
    SalesActivitiesQueryAdapter,
    query_capability_definitions as sales_activities_query_capability_definitions,
};
use crm_search_query_adapter::{SearchQueryAdapter, search_query_capability_definition};
use crm_search_runtime::SearchIndexId;
use std::collections::BTreeSet;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct PostgresModuleActivation {
    store: PostgresDataStore,
    bootstrap_active: bool,
}

impl PostgresModuleActivation {
    pub fn new(store: PostgresDataStore, bootstrap_active: bool) -> Self {
        Self {
            store,
            bootstrap_active,
        }
    }
}

impl ModuleActivationPort for PostgresModuleActivation {
    fn is_active<'a>(
        &'a self,
        tenant_id: &'a TenantId,
        module_id: &'a ModuleId,
    ) -> PortFuture<'a, Result<bool, SdkError>> {
        Box::pin(async move {
            if self.bootstrap_active {
                return Ok(true);
            }
            self.store.is_module_active(tenant_id, module_id).await
        })
    }
}

pub struct ProductionCompositionDependencies {
    pub store: PostgresDataStore,
    pub activation: Arc<dyn ModuleActivationPort>,
    pub capability_authorizer: Arc<dyn CapabilityAuthorizer>,
    pub query_authorizer: Arc<dyn QueryAuthorizer>,
    pub visibility_authorizer: Arc<dyn QueryVisibilityAuthorizer>,
    pub cursor_key: [u8; 32],
}

/// Returns the exact public mutation inventory assembled from module-owned
/// definition factories. This compatibility API is intentionally data-only:
/// production dispatch is owned exclusively by `ApplicationComposition`.
pub fn application_mutation_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {
    let mut definitions = sales_activities_capability_definitions()?;
    definitions.extend(party_capability_definitions()?);
    definitions.extend(account_capability_definitions()?);
    definitions.extend(contact_point_capability_definitions()?);
    definitions.extend(party_relationship_capability_definitions()?);
    definitions.extend(consent_capability_definitions()?);
    definitions.extend(identity_resolution_capability_definitions()?);
    definitions.extend(
        customer_data_operations_capability_definitions()?
            .into_iter()
            .filter(|definition| {
                !matches!(
                    definition.capability_id.as_str(),
                    CREATE_PARTY_IMPORT_JOB_CAPABILITY | VALIDATE_PARTY_IMPORT_ROWS_CAPABILITY
                )
            }),
    );
    definitions.extend(customer_data_operations_source_capability_definitions()?);
    definitions.extend(data_quality_capability_definitions()?);
    definitions.extend(metadata_mutation_capability_definitions()?);
    Ok(definitions)
}

/// Returns the exact public query inventory assembled from module-owned
/// definition factories. It exists for tests, bootstrap grants and parity
/// checks; it is not a router and performs no runtime dispatch.
pub fn application_query_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {
    let mut definitions = sales_activities_query_capability_definitions()?;
    definitions.extend(party_query_capability_definitions()?);
    definitions.extend(account_query_capability_definitions()?);
    definitions.extend(contact_point_query_capability_definitions()?);
    definitions.extend(party_relationship_query_capability_definitions()?);
    definitions.extend(customer_360_query_capability_definitions()?);
    definitions.extend(consent_query_capability_definitions()?);
    definitions.extend(identity_resolution_query_capability_definitions()?);
    definitions.extend(identity_resolution_merge_query_capability_definitions()?);
    definitions.extend(customer_data_operations_query_capability_definitions()?);
    definitions.extend(data_quality_query_capability_definitions()?);
    definitions.push(search_query_capability_definition()?);
    definitions.extend(metadata_query_capability_definitions()?);
    Ok(definitions)
}

pub fn build_production_composition(
    dependencies: ProductionCompositionDependencies,
) -> Result<ApplicationComposition, SdkError> {
    let ProductionCompositionDependencies {
        store,
        activation,
        capability_authorizer,
        query_authorizer,
        visibility_authorizer,
        cursor_key,
    } = dependencies;
    let mut contributions = ModuleContributionSet::new();
    let parties = Arc::new(PostgresPartyReferenceReader::new(store.clone()));

    let sales_activities_executor =
        aggregate_executor(store.clone(), SalesActivitiesCapabilityPlannerRouter);
    add_activated_mutations(
        &mut contributions,
        sales_activities_capability_definitions()?,
        Arc::new(NoopMutationSemanticValidator),
        sales_activities_executor,
        activation.clone(),
    )?;

    let party_executor = aggregate_executor(store.clone(), PartyCapabilityPlanner);
    add_activated_mutations(
        &mut contributions,
        party_capability_definitions()?,
        Arc::new(NoopMutationSemanticValidator),
        party_executor,
        activation.clone(),
    )?;

    let account_executor = aggregate_executor(store.clone(), CustomerAccountCapabilityPlanner);
    add_activated_mutations(
        &mut contributions,
        account_capability_definitions()?,
        Arc::new(AccountPartyReferenceSemanticValidator::new(parties.clone())),
        account_executor,
        activation.clone(),
    )?;

    let contact_point_executor = aggregate_executor(store.clone(), ContactPointCapabilityPlanner);
    add_activated_mutations(
        &mut contributions,
        contact_point_capability_definitions()?,
        Arc::new(ContactPointPartyReferenceSemanticValidator::new(
            parties.clone(),
        )),
        contact_point_executor,
        activation.clone(),
    )?;

    let party_relationship_executor =
        aggregate_executor(store.clone(), PartyRelationshipCapabilityPlanner);
    add_activated_mutations(
        &mut contributions,
        party_relationship_capability_definitions()?,
        Arc::new(PartyRelationshipReferenceSemanticValidator::new(
            parties.clone(),
        )),
        party_relationship_executor,
        activation.clone(),
    )?;

    let consent_aggregate = aggregate_executor(store.clone(), ConsentCapabilityPlanner);
    add_activated_mutations(
        &mut contributions,
        consent_capability_definitions()?,
        Arc::new(ConsentCapabilitySemanticValidator::new(Arc::new(
            PostgresConsentReferenceReader::new(store.clone()),
        ))),
        Arc::new(ConsentCapabilityExecutor::new(consent_aggregate)),
        activation.clone(),
    )?;

    let identity_aggregate = aggregate_executor(store.clone(), IdentityResolutionCapabilityPlanner);
    let identity_definitions = identity_resolution_capability_definitions()?;
    let candidate_definitions =
        select_definitions(&identity_definitions, &CANDIDATE_MUTATION_CAPABILITY_IDS);
    let merge_definitions =
        select_definitions(&identity_definitions, &MERGE_MUTATION_CAPABILITY_IDS);
    add_activated_mutations(
        &mut contributions,
        candidate_definitions,
        Arc::new(IdentityResolutionCapabilitySemanticValidator::new(
            Arc::new(PostgresIdentityResolutionReferenceReader::new(
                store.clone(),
            )),
        )),
        Arc::new(IdentityResolutionCapabilityExecutor::new(
            identity_aggregate.clone(),
        )),
        activation.clone(),
    )?;
    add_activated_mutations(
        &mut contributions,
        merge_definitions,
        Arc::new(MergeLineageCapabilitySemanticValidator::new(Arc::new(
            PostgresMergeLineageReferenceReader::new(store.clone()),
        ))),
        Arc::new(MergeLineageCapabilityExecutor::new(identity_aggregate)),
        activation.clone(),
    )?;

    let customer_data_operations_executor =
        aggregate_executor(store.clone(), CustomerDataOperationsCapabilityPlanner);
    let customer_data_operations_definitions = customer_data_operations_capability_definitions()?
        .into_iter()
        .filter(|definition| {
            !matches!(
                definition.capability_id.as_str(),
                CREATE_PARTY_IMPORT_JOB_CAPABILITY | VALIDATE_PARTY_IMPORT_ROWS_CAPABILITY
            )
        })
        .collect::<Vec<_>>();
    add_activated_mutations(
        &mut contributions,
        customer_data_operations_definitions,
        Arc::new(NoopMutationSemanticValidator),
        customer_data_operations_executor,
        activation.clone(),
    )?;
    let source_executor: Arc<dyn TransactionalCapabilityExecutor> =
        Arc::new(CustomerDataOperationsSourceExecutor::new(
            store.clone(),
            Arc::new(PostgresImmutableFileArtifactStore::new(store.clone())),
            capability_authorizer.clone(),
        ));
    add_activated_mutations(
        &mut contributions,
        customer_data_operations_source_capability_definitions()?,
        Arc::new(NoopMutationSemanticValidator),
        source_executor,
        activation.clone(),
    )?;

    let data_quality_fallback = aggregate_executor(store.clone(), DataQualityAggregatePlanner);
    let data_quality_executor: Arc<dyn TransactionalCapabilityExecutor> =
        Arc::new(DataQualityCapabilityExecutor::new(
            store.clone(),
            data_quality_fallback,
            capability_authorizer.clone(),
            query_authorizer.clone(),
        ));
    add_activated_mutations(
        &mut contributions,
        data_quality_capability_definitions()?,
        Arc::new(NoopMutationSemanticValidator),
        data_quality_executor,
        activation.clone(),
    )?;

    contributions
        .add_mutations(
            metadata_mutation_capability_definitions()?,
            Arc::new(NoopMutationSemanticValidator),
            Arc::new(PostgresMetadataCapabilityExecutor::new(store.clone())),
        )
        .map_err(composition_error)?;

    let sales_activities_queries = Arc::new(SalesActivitiesQueryAdapter::new(
        store.clone(),
        cursor(cursor_key)?,
        visibility_authorizer.clone(),
    )?);
    add_activated_queries(
        &mut contributions,
        sales_activities_query_capability_definitions()?,
        sales_activities_queries,
        activation.clone(),
    )?;

    let party_queries = Arc::new(PartyQueryAdapter::new(
        store.clone(),
        cursor(cursor_key)?,
        visibility_authorizer.clone(),
    )?);
    add_activated_queries(
        &mut contributions,
        party_query_capability_definitions()?,
        party_queries,
        activation.clone(),
    )?;

    let account_queries = Arc::new(AccountQueryAdapter::new(
        store.clone(),
        cursor(cursor_key)?,
        visibility_authorizer.clone(),
    )?);
    add_activated_queries(
        &mut contributions,
        account_query_capability_definitions()?,
        account_queries,
        activation.clone(),
    )?;

    let contact_point_queries = Arc::new(ContactPointQueryAdapter::new(
        store.clone(),
        cursor(cursor_key)?,
        visibility_authorizer.clone(),
    )?);
    add_activated_queries(
        &mut contributions,
        contact_point_query_capability_definitions()?,
        contact_point_queries,
        activation.clone(),
    )?;

    let relationship_queries = Arc::new(PartyRelationshipQueryAdapter::new(
        store.clone(),
        cursor(cursor_key)?,
        visibility_authorizer.clone(),
    )?);
    add_activated_queries(
        &mut contributions,
        party_relationship_query_capability_definitions()?,
        relationship_queries,
        activation.clone(),
    )?;

    let customer_360_queries = Arc::new(Customer360QueryAdapter::new(
        store.clone(),
        visibility_authorizer.clone(),
    ));
    add_activated_queries(
        &mut contributions,
        customer_360_query_capability_definitions()?,
        customer_360_queries,
        activation.clone(),
    )?;

    let consent_queries = Arc::new(ConsentQueryAdapter::new(
        store.clone(),
        cursor(cursor_key)?,
        visibility_authorizer.clone(),
    )?);
    add_activated_queries(
        &mut contributions,
        consent_query_capability_definitions()?,
        consent_queries,
        activation.clone(),
    )?;

    let identity_queries = Arc::new(IdentityResolutionQueryAdapter::new(
        store.clone(),
        cursor(cursor_key)?,
        visibility_authorizer.clone(),
    )?);
    add_activated_queries(
        &mut contributions,
        identity_resolution_query_capability_definitions()?,
        identity_queries,
        activation.clone(),
    )?;

    let identity_merge_queries = Arc::new(IdentityResolutionMergeQueryAdapter::new(
        store.clone(),
        cursor(cursor_key)?,
        visibility_authorizer.clone(),
    )?);
    add_activated_queries(
        &mut contributions,
        identity_resolution_merge_query_capability_definitions()?,
        identity_merge_queries,
        activation.clone(),
    )?;

    let customer_data_queries = Arc::new(CustomerDataOperationsQueryAdapter::new(
        store.clone(),
        cursor(cursor_key)?,
        visibility_authorizer.clone(),
    )?);
    add_activated_queries(
        &mut contributions,
        customer_data_operations_query_capability_definitions()?,
        customer_data_queries,
        activation.clone(),
    )?;

    let data_quality_queries = Arc::new(DataQualityQueryAdapter::new(
        store.clone(),
        visibility_authorizer.clone(),
    ));
    add_activated_queries(
        &mut contributions,
        data_quality_query_capability_definitions()?,
        data_quality_queries,
        activation,
    )?;

    let search_queries = Arc::new(SearchQueryAdapter::new(
        SearchIndexId::try_new(GLOBAL_SEARCH_INDEX_ID).map_err(configuration_error)?,
        Arc::new(store.clone()),
        visibility_authorizer,
        cursor(cursor_key)?,
    )?);
    contributions
        .add_queries(
            [search_query_capability_definition()?],
            search_queries.clone(),
            search_queries,
        )
        .map_err(composition_error)?;

    let metadata_queries = Arc::new(MetadataQueryAdapter::new(Arc::new(
        PostgresMetadataQueryStore::new(store),
    )));
    contributions
        .add_queries(
            metadata_query_capability_definitions()?,
            metadata_queries.clone(),
            metadata_queries,
        )
        .map_err(composition_error)?;

    contributions
        .add_empty_module(ModuleId::try_new(LINK_MODULE_ID).map_err(configuration_error)?)
        .map_err(composition_error)?;
    contributions.build().map_err(composition_error)
}

fn aggregate_executor<P>(
    store: PostgresDataStore,
    planner: P,
) -> Arc<dyn TransactionalCapabilityExecutor>
where
    P: TransactionalAggregatePlanner + 'static,
{
    Arc::new(PostgresTransactionalAggregateExecutor::new(
        store,
        Arc::new(planner),
    ))
}

fn add_activated_mutations(
    contributions: &mut ModuleContributionSet,
    definitions: Vec<CapabilityDefinition>,
    validator: Arc<dyn CapabilitySemanticValidator>,
    executor: Arc<dyn TransactionalCapabilityExecutor>,
    activation: Arc<dyn ModuleActivationPort>,
) -> Result<(), SdkError> {
    let validator: Arc<dyn CapabilitySemanticValidator> =
        Arc::new(ActivationGatedMutationValidator::new(activation, validator));
    contributions
        .add_mutations(definitions, validator, executor)
        .map(|_| ())
        .map_err(composition_error)
}

fn add_activated_queries<T>(
    contributions: &mut ModuleContributionSet,
    definitions: Vec<CapabilityDefinition>,
    adapter: Arc<T>,
    activation: Arc<dyn ModuleActivationPort>,
) -> Result<(), SdkError>
where
    T: QuerySemanticValidator + QueryExecutor + 'static,
{
    let validator: Arc<dyn QuerySemanticValidator> = Arc::new(ActivationGatedQueryValidator::new(
        activation,
        adapter.clone(),
    ));
    let executor: Arc<dyn QueryExecutor> = adapter;
    contributions
        .add_queries(definitions, validator, executor)
        .map(|_| ())
        .map_err(composition_error)
}

fn select_definitions(
    definitions: &[CapabilityDefinition],
    capability_ids: &[&str],
) -> Vec<CapabilityDefinition> {
    definitions
        .iter()
        .filter(|definition| capability_ids.contains(&definition.capability_id.as_str()))
        .cloned()
        .collect()
}

fn cursor(key: [u8; 32]) -> Result<CursorCodec, SdkError> {
    CursorCodec::new(key).map_err(|error| {
        SdkError::new(
            "APPLICATION_CURSOR_CONFIGURATION_INVALID",
            ErrorCategory::Internal,
            false,
            "The application cursor configuration is invalid.",
        )
        .with_internal_reference(error.to_string())
    })
}

fn composition_error(error: impl std::fmt::Display) -> SdkError {
    SdkError::new(
        "APPLICATION_COMPOSITION_INVALID",
        ErrorCategory::Internal,
        false,
        "The production application composition is invalid.",
    )
    .with_internal_reference(error.to_string())
}

fn configuration_error(error: impl std::fmt::Display) -> SdkError {
    SdkError::new(
        "APPLICATION_COMPOSITION_CONFIGURATION_INVALID",
        ErrorCategory::Internal,
        false,
        "The production application composition configuration is invalid.",
    )
    .with_internal_reference(error.to_string())
}

pub fn declared_business_module_ids() -> BTreeSet<String> {
    [
        "crm.activities",
        "crm.consents",
        "crm.contact-points",
        "crm.customer-accounts",
        "crm.customer-data-operations",
        "crm.customer360",
        "crm.data-quality",
        "crm.identity-resolution",
        "crm.parties",
        "crm.party-relationships",
        "crm.sales",
        LINK_MODULE_ID,
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}
