#![forbid(unsafe_code)]

//! Owner-owned production contribution for `crm.customer-privacy`.
//!
//! This package is the only supported process-composition entry point for
//! Customer Privacy. It promotes five mutations and preserves four permission-aware queries.
//! Scope discovery and deterministic planning remain owner-owned internal services:
//! neither is a public route or a generic-runtime worker.

use crm_application_composition::{
    ActivationGatedMutationValidator, ActivationGatedQueryValidator, ModuleActivationPort,
    ModuleContributionSet, NoopMutationSemanticValidator,
};
use crm_capability_runtime::CapabilitySemanticValidator;
use crm_consents_privacy_scope_adapter::{
    ConsentsPrivacyScopeQueryAdapter, consents_privacy_scope_definition,
};
use crm_contact_points_privacy_scope_adapter::{
    ContactPointsPrivacyScopeQueryAdapter, contact_points_privacy_scope_definition,
};
use crm_core_data::PostgresDataStore;
use crm_customer_accounts_privacy_scope_adapter::{
    CustomerAccountsPrivacyScopeQueryAdapter, customer_accounts_privacy_scope_definition,
};
use crm_customer_data_operations_privacy_scope_adapter::{
    CustomerDataOperationsPrivacyScopeQueryAdapter, customer_data_privacy_scope_definition,
};
use crm_customer_enrichment_privacy_scope_adapter::{
    CustomerEnrichmentPrivacyScopeQueryAdapter, customer_enrichment_privacy_scope_definition,
};
use crm_customer_privacy::{SCOPE_SNAPSHOT_RECORD_TYPE, discovery_sha256};
pub use crm_customer_privacy_application::{
    APPROVE_PRIVACY_CASE_CAPABILITY, DiscoveryInvocation, DiscoverySnapshotReader,
    GET_PRIVACY_ACTION_PLAN_CAPABILITY, LIST_PRIVACY_OWNER_OUTCOMES_CAPABILITY, PlanningInvocation,
    PrivacyPlanningService, ScopeDiscoveryService, SnapshotReadContext,
    mutation_capability_definitions, plan_read_visibility_resources, query_capability_definitions,
};
use crm_customer_privacy_application::{
    CustomerPrivacyPlanReadAdapter, DiscoverySnapshotVisibilityPort, OwnerContributionEndpoint,
    OwnerContributionEndpoints, SnapshotVisibilityDecision, plan_read_query_capability_definitions,
};
use crm_customer_privacy_postgres::{
    PostgresDiscoveryPersistence, PostgresPlanningPersistence, PostgresPrivacyReadPersistence,
    postgres_case_approval_executor, postgres_case_cancel_executor, postgres_case_create_executor,
    postgres_case_subject_verify_executor, postgres_case_submit_executor,
};
use crm_customer_privacy_query_adapter::{
    CustomerPrivacyQueryAdapter, query_capability_definitions as case_query_capability_definitions,
};
pub use crm_customer_privacy_query_adapter::{
    GET_PRIVACY_CASE_CAPABILITY, LIST_PRIVACY_CASES_CAPABILITY, query_visibility_resources,
};
use crm_data_quality_privacy_scope_adapter::{
    DataQualityPrivacyScopeQueryAdapter, data_quality_privacy_scope_definition,
};
use crm_identity_resolution_privacy_scope_adapter::{
    IdentityResolutionPrivacyScopeQueryAdapter, identity_resolution_privacy_scope_definition,
};
use crm_module_sdk::{
    CapabilityId, CapabilityVersion, DataClass, ErrorCategory, ModuleId, PayloadEncoding,
    PortFuture, RecordId, RecordRef, RecordType, RetentionPolicyId, SchemaId, SchemaVersion,
    SdkError, TypedPayload,
};
use crm_parties_privacy_scope_adapter::{
    PartiesPrivacyScopeQueryAdapter, parties_privacy_scope_definition,
};
use crm_party_relationships_privacy_scope_adapter::{
    PartyRelationshipsPrivacyScopeQueryAdapter, party_relationships_privacy_scope_definition,
};
use crm_query_runtime::{
    CursorCodec, QueryExecutionContext, QueryExecutor, QueryRequest, QuerySemanticValidator,
    QueryVisibilityAuthorizer,
};
use std::sync::Arc;

const INTERNAL_SNAPSHOT_READ_CAPABILITY: &str = "customer_privacy.scope.snapshot.read";
const INTERNAL_SNAPSHOT_READ_VERSION: &str = "1.0.0";
const INTERNAL_SNAPSHOT_READ_SCHEMA: &str = "crm.customer-privacy.discovery_scope_snapshot.read";
const INTERNAL_SNAPSHOT_READ_RETENTION: &str = "crm.customer_privacy.discovery_scope_snapshot.read";

#[derive(Clone)]
pub struct CustomerPrivacyProductionDependencies {
    pub store: PostgresDataStore,
    pub activation: Arc<dyn ModuleActivationPort>,
    pub visibility_authorizer: Arc<dyn QueryVisibilityAuthorizer>,
    pub cursor_key: [u8; 32],
}

pub struct CustomerPrivacyProduction {
    pub contribution: ModuleContributionSet,
    pub discovery: ScopeDiscoveryService,
    pub snapshot_reader: DiscoverySnapshotReader,
    pub planning: PrivacyPlanningService,
}

pub fn build_production(
    dependencies: CustomerPrivacyProductionDependencies,
) -> Result<CustomerPrivacyProduction, SdkError> {
    let contribution = build_contribution(dependencies.clone())?;
    let (discovery, snapshot_reader) = build_internal_discovery(&dependencies)?;
    let planning = build_internal_planning(&dependencies);
    Ok(CustomerPrivacyProduction {
        contribution,
        discovery,
        snapshot_reader,
        planning,
    })
}

pub fn build_internal_discovery(
    dependencies: &CustomerPrivacyProductionDependencies,
) -> Result<(ScopeDiscoveryService, DiscoverySnapshotReader), SdkError> {
    let endpoints = OwnerContributionEndpoints::exact_canonical([
        OwnerContributionEndpoint {
            definition: consents_privacy_scope_definition()?,
            executor: Arc::new(ConsentsPrivacyScopeQueryAdapter::new(
                dependencies.store.clone(),
            )),
        },
        OwnerContributionEndpoint {
            definition: contact_points_privacy_scope_definition()?,
            executor: Arc::new(ContactPointsPrivacyScopeQueryAdapter::new(
                dependencies.store.clone(),
            )),
        },
        OwnerContributionEndpoint {
            definition: customer_accounts_privacy_scope_definition()?,
            executor: Arc::new(CustomerAccountsPrivacyScopeQueryAdapter::new(
                dependencies.store.clone(),
            )),
        },
        OwnerContributionEndpoint {
            definition: customer_data_privacy_scope_definition()?,
            executor: Arc::new(CustomerDataOperationsPrivacyScopeQueryAdapter::new(
                dependencies.store.clone(),
            )),
        },
        OwnerContributionEndpoint {
            definition: customer_enrichment_privacy_scope_definition()?,
            executor: Arc::new(CustomerEnrichmentPrivacyScopeQueryAdapter::new(
                dependencies.store.clone(),
            )),
        },
        OwnerContributionEndpoint {
            definition: data_quality_privacy_scope_definition()?,
            executor: Arc::new(DataQualityPrivacyScopeQueryAdapter::new(
                dependencies.store.clone(),
            )),
        },
        OwnerContributionEndpoint {
            definition: identity_resolution_privacy_scope_definition()?,
            executor: Arc::new(IdentityResolutionPrivacyScopeQueryAdapter::new(
                dependencies.store.clone(),
            )),
        },
        OwnerContributionEndpoint {
            definition: parties_privacy_scope_definition()?,
            executor: Arc::new(PartiesPrivacyScopeQueryAdapter::new(
                dependencies.store.clone(),
            )),
        },
        OwnerContributionEndpoint {
            definition: party_relationships_privacy_scope_definition()?,
            executor: Arc::new(PartyRelationshipsPrivacyScopeQueryAdapter::new(
                dependencies.store.clone(),
            )),
        },
    ])?;
    let persistence = Arc::new(PostgresDiscoveryPersistence::new(Arc::new(
        dependencies.store.clone(),
    )));
    let discovery = ScopeDiscoveryService::new(
        dependencies.activation.clone(),
        persistence.clone(),
        endpoints,
    );
    let snapshot_reader = DiscoverySnapshotReader::new(
        persistence,
        Arc::new(ProductionSnapshotVisibility {
            inner: dependencies.visibility_authorizer.clone(),
        }),
    );
    Ok((discovery, snapshot_reader))
}

pub fn build_internal_planning(
    dependencies: &CustomerPrivacyProductionDependencies,
) -> PrivacyPlanningService {
    PrivacyPlanningService::new(
        dependencies.activation.clone(),
        Arc::new(PostgresPlanningPersistence::new(Arc::new(
            dependencies.store.clone(),
        ))),
    )
}

pub fn build_contribution(
    dependencies: CustomerPrivacyProductionDependencies,
) -> Result<ModuleContributionSet, SdkError> {
    let mutations = mutation_capability_definitions()?;
    if mutations.len() != 5 {
        return Err(configuration_error(
            "Customer Privacy production inventory must contain exactly five mutations",
        ));
    }
    let all_queries = query_capability_definitions()?;
    if all_queries.len() != 4 {
        return Err(configuration_error(
            "Customer Privacy production inventory must contain exactly four queries",
        ));
    }
    let case_queries = case_query_capability_definitions()?;
    let plan_queries = plan_read_query_capability_definitions()?;
    if case_queries.len() != 2 || plan_queries.len() != 2 {
        return Err(configuration_error(
            "Customer Privacy query ownership must remain split into two case reads and two plan reads",
        ));
    }

    let mut contributions = ModuleContributionSet::new();
    let executors = [
        postgres_case_create_executor(dependencies.store.clone()),
        postgres_case_submit_executor(dependencies.store.clone()),
        postgres_case_subject_verify_executor(dependencies.store.clone()),
        postgres_case_approval_executor(dependencies.store.clone()),
        postgres_case_cancel_executor(dependencies.store.clone()),
    ];
    for (definition, executor) in mutations.into_iter().zip(executors) {
        let validator: Arc<dyn CapabilitySemanticValidator> =
            Arc::new(ActivationGatedMutationValidator::new(
                dependencies.activation.clone(),
                Arc::new(NoopMutationSemanticValidator),
            ));
        contributions
            .add_mutations([definition], validator, executor)
            .map_err(composition_error)?;
    }

    let case_query_adapter = Arc::new(CustomerPrivacyQueryAdapter::new_with_cursor(
        dependencies.store.clone(),
        cursor(dependencies.cursor_key)?,
        dependencies.visibility_authorizer.clone(),
    ));
    let case_query_validator: Arc<dyn QuerySemanticValidator> =
        Arc::new(ActivationGatedQueryValidator::new(
            dependencies.activation.clone(),
            case_query_adapter.clone(),
        ));
    let case_query_executor: Arc<dyn QueryExecutor> = case_query_adapter;
    contributions
        .add_queries(case_queries, case_query_validator, case_query_executor)
        .map_err(composition_error)?;

    let plan_query_adapter = Arc::new(CustomerPrivacyPlanReadAdapter::new(
        dependencies.activation,
        Arc::new(PostgresPrivacyReadPersistence::new(Arc::new(
            dependencies.store,
        ))),
        dependencies.visibility_authorizer,
    ));
    let plan_query_validator: Arc<dyn QuerySemanticValidator> = plan_query_adapter.clone();
    let plan_query_executor: Arc<dyn QueryExecutor> = plan_query_adapter;
    contributions
        .add_queries(plan_queries, plan_query_validator, plan_query_executor)
        .map_err(composition_error)?;

    Ok(contributions)
}

#[derive(Clone)]
struct ProductionSnapshotVisibility {
    inner: Arc<dyn QueryVisibilityAuthorizer>,
}

impl DiscoverySnapshotVisibilityPort for ProductionSnapshotVisibility {
    fn authorize<'a>(
        &'a self,
        context: &'a SnapshotReadContext,
        snapshot_id: &'a RecordId,
        required_field: &'a str,
    ) -> PortFuture<'a, Result<SnapshotVisibilityDecision, SdkError>> {
        Box::pin(async move {
            let bytes = snapshot_id.as_str().as_bytes().to_vec();
            let descriptor_hash = discovery_sha256(
                b"crm.customer-privacy.discovery_scope_snapshot.read/v1:snapshot_id",
            );
            let request = QueryRequest {
                owner_module_id: id::<ModuleId>("crm.customer-privacy")?,
                context: QueryExecutionContext {
                    tenant_id: context.tenant_id.clone(),
                    actor_id: context.actor_id.clone(),
                    request_id: context.request_id.clone(),
                    correlation_id: context.correlation_id.clone(),
                    trace_id: context.trace_id.clone(),
                    capability_id: id::<CapabilityId>(INTERNAL_SNAPSHOT_READ_CAPABILITY)?,
                    capability_version: id::<CapabilityVersion>(INTERNAL_SNAPSHOT_READ_VERSION)?,
                    schema_version: id::<SchemaVersion>(INTERNAL_SNAPSHOT_READ_VERSION)?,
                    request_started_at_unix_nanos: context.request_started_at_unix_nanos,
                },
                input: TypedPayload {
                    owner: id::<ModuleId>("crm.customer-privacy")?,
                    schema_id: id::<SchemaId>(INTERNAL_SNAPSHOT_READ_SCHEMA)?,
                    schema_version: id::<SchemaVersion>(INTERNAL_SNAPSHOT_READ_VERSION)?,
                    descriptor_hash,
                    data_class: DataClass::Confidential,
                    encoding: PayloadEncoding::Utf8Text,
                    maximum_size_bytes: 180,
                    retention_policy_id: id::<RetentionPolicyId>(INTERNAL_SNAPSHOT_READ_RETENTION)?,
                    bytes: bytes.clone(),
                },
                input_hash: discovery_sha256(&bytes),
            };
            let resource = RecordRef {
                record_type: id::<RecordType>(SCOPE_SNAPSHOT_RECORD_TYPE)?,
                record_id: snapshot_id.clone(),
            };
            let decision = self.inner.authorize_visibility(&request, &resource).await?;
            Ok(SnapshotVisibilityDecision {
                allowed: decision.resource_visible && decision.allows_field(required_field),
                decision_id: decision.decision_id,
                policy_version: decision.policy_version,
            })
        })
    }
}

trait Identifier: Sized {
    fn make(value: &str) -> Result<Self, crm_module_sdk::IdentifierError>;
}

macro_rules! identifier {
    ($type:ty) => {
        impl Identifier for $type {
            fn make(value: &str) -> Result<Self, crm_module_sdk::IdentifierError> {
                <$type>::try_new(value)
            }
        }
    };
}

identifier!(ModuleId);
identifier!(CapabilityId);
identifier!(CapabilityVersion);
identifier!(SchemaId);
identifier!(SchemaVersion);
identifier!(RetentionPolicyId);
identifier!(RecordType);

fn id<T: Identifier>(value: &str) -> Result<T, SdkError> {
    T::make(value).map_err(|error| configuration_error(error.to_string()))
}

fn cursor(key: [u8; 32]) -> Result<CursorCodec, SdkError> {
    CursorCodec::new(key).map_err(|error| configuration_error(error.to_string()))
}

fn composition_error(error: impl std::fmt::Display) -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_PRODUCTION_COMPOSITION_INVALID",
        ErrorCategory::Internal,
        false,
        "The Customer Privacy production contribution is invalid.",
    )
    .with_internal_reference(error.to_string())
}

fn configuration_error(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_PRODUCTION_CONFIGURATION_INVALID",
        ErrorCategory::Internal,
        false,
        "The Customer Privacy production package is misconfigured.",
    )
    .with_internal_reference(reference)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn approval_is_public_while_internal_discovery_and_planning_remain_non_public() {
        let mutations = mutation_capability_definitions().unwrap();
        let queries = query_capability_definitions().unwrap();
        assert_eq!(mutations.len(), 5);
        assert_eq!(queries.len(), 4);
        assert!(mutations.iter().any(|definition| {
            definition.capability_id.as_str() == APPROVE_PRIVACY_CASE_CAPABILITY
        }));
        for forbidden in [
            "customer_privacy.scope.discover",
            "customer_privacy.plan.build",
        ] {
            assert!(
                mutations
                    .iter()
                    .all(|definition| definition.capability_id.as_str() != forbidden)
            );
            assert!(
                queries
                    .iter()
                    .all(|definition| definition.capability_id.as_str() != forbidden)
            );
        }
        assert!(queries.iter().any(|definition| {
            definition.capability_id.as_str() == GET_PRIVACY_ACTION_PLAN_CAPABILITY
        }));
        assert!(queries.iter().any(|definition| {
            definition.capability_id.as_str() == LIST_PRIVACY_OWNER_OUTCOMES_CAPABILITY
        }));
    }

    #[test]
    fn snapshot_visibility_requires_the_exact_internal_field() {
        let mut allowed = BTreeSet::new();
        allowed.insert("discovery_scope_snapshot".to_owned());
        assert!(allowed.contains("discovery_scope_snapshot"));
        assert!(!allowed.contains("resource_payload"));
    }
}
