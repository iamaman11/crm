#!/usr/bin/env python3
from pathlib import Path


def replace_once(path: str, old: str, new: str) -> None:
    target = Path(path)
    text = target.read_text(encoding="utf-8")
    if text.count(old) != 1:
        raise SystemExit(f"{path}: expected exactly one patch anchor, found {text.count(old)}")
    target.write_text(text.replace(old, new, 1), encoding="utf-8")


replace_once(
    "crates/crm-application-composition/src/lib.rs",
    '''    pub fn add_empty_module(&mut self, module_id: ModuleId) -> Result<&mut Self, CompositionError> {
        let key = module_id.as_str().to_owned();
        if self.modules.contains_key(&key) {
            return Ok(self);
        }
        self.modules
            .insert(key, ModuleRuntimeContribution::new(module_id));
        Ok(self)
    }

    pub fn build(self) -> Result<ApplicationComposition, CompositionError> {
''',
    '''    pub fn add_empty_module(&mut self, module_id: ModuleId) -> Result<&mut Self, CompositionError> {
        let key = module_id.as_str().to_owned();
        if self.modules.contains_key(&key) {
            return Ok(self);
        }
        self.modules
            .insert(key, ModuleRuntimeContribution::new(module_id));
        Ok(self)
    }

    /// Merges independently built module-owned contribution fragments.
    /// Duplicate routes and owner mismatches remain fail-closed at final assembly.
    pub fn merge(&mut self, other: Self) -> &mut Self {
        for contribution in other.modules.into_values() {
            let key = contribution.module_id.as_str().to_owned();
            let target = self
                .modules
                .entry(key)
                .or_insert_with(|| ModuleRuntimeContribution::new(contribution.module_id.clone()));
            target.mutations.extend(contribution.mutations);
            target.queries.extend(contribution.queries);
        }
        self
    }

    pub fn build(self) -> Result<ApplicationComposition, CompositionError> {
''',
)

replace_once(
    "crates/crm-application-composition/src/lib.rs",
    '''    #[test]
    fn duplicate_and_owner_mismatched_routes_fail_assembly() {
''',
    '''    #[test]
    fn independently_built_contribution_sets_merge_by_owner() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let handler = Arc::new(MutationHandler {
            calls: Arc::clone(&calls),
        });
        let mut first = ModuleContributionSet::new();
        first
            .add_mutations(
                [definition("crm.alpha", "alpha.record.create", true)],
                handler.clone(),
                handler.clone(),
            )
            .unwrap();
        let mut second = ModuleContributionSet::new();
        second
            .add_mutations(
                [definition("crm.alpha", "alpha.record.update", true)],
                handler.clone(),
                handler,
            )
            .unwrap();

        first.merge(second);
        let composition = first.build().unwrap();
        assert_eq!(
            composition.module_ids(),
            &BTreeSet::from(["crm.alpha".to_owned()])
        );
        assert_eq!(composition.mutation_definitions().len(), 2);
    }

    #[test]
    fn duplicate_and_owner_mismatched_routes_fail_assembly() {
''',
)

Path("crates/crm-customer-accounts-capability-composition/Cargo.toml").write_text(
    '''[package]
name = "crm-customer-accounts-capability-composition"
version = "0.1.0"
edition = "2024"
publish = false

[dependencies]
crm-application-composition = { path = "../crm-application-composition" }
crm-capability-runtime = { path = "../crm-capability-runtime" }
crm-core-data = { path = "../crm-core-data" }
crm-customer-accounts-capability-adapter = { path = "../crm-customer-accounts-capability-adapter" }
crm-customer-accounts-query-adapter = { path = "../crm-customer-accounts-query-adapter" }
crm-module-sdk = { path = "../crm-module-sdk" }
crm-party-reference-composition = { path = "../crm-party-reference-composition" }
crm-query-runtime = { path = "../crm-query-runtime" }
''',
    encoding="utf-8",
)

replace_once(
    "crates/crm-customer-accounts-capability-composition/src/lib.rs",
    '''use crm_capability_runtime::{
    CapabilityDefinition, CapabilityRequest, CapabilitySemanticValidator,
};
use crm_customer_accounts_capability_adapter::{
    CREATE_CAPABILITY, MUTATION_CAPABILITY_IDS, UPDATE_CAPABILITY,
    referenced_party_ids_from_create, referenced_party_ids_from_update,
};
use crm_module_sdk::{ErrorCategory, PortFuture, SdkError};
use crm_party_reference_composition::PartyReferenceReader;
use std::collections::BTreeSet;
use std::fmt;
use std::sync::Arc;
''',
    '''use crm_application_composition::{
    ActivationGatedMutationValidator, ActivationGatedQueryValidator, ModuleActivationPort,
    ModuleContributionSet,
};
use crm_capability_runtime::{
    CapabilityDefinition, CapabilityRequest, CapabilitySemanticValidator,
    TransactionalCapabilityExecutor,
};
use crm_core_data::{PostgresDataStore, PostgresTransactionalAggregateExecutor};
use crm_customer_accounts_capability_adapter::{
    CREATE_CAPABILITY, CustomerAccountCapabilityPlanner, MUTATION_CAPABILITY_IDS,
    UPDATE_CAPABILITY, capability_definitions, referenced_party_ids_from_create,
    referenced_party_ids_from_update,
};
use crm_customer_accounts_query_adapter::{
    AccountQueryAdapter, query_capability_definitions,
};
use crm_module_sdk::{ErrorCategory, PortFuture, SdkError};
use crm_party_reference_composition::PartyReferenceReader;
use crm_query_runtime::{
    CursorCodec, QueryExecutor, QuerySemanticValidator, QueryVisibilityAuthorizer,
};
use std::collections::BTreeSet;
use std::fmt;
use std::sync::Arc;
''',
)

replace_once(
    "crates/crm-customer-accounts-capability-composition/src/lib.rs",
    'pub const CRATE_NAME: &str = "crm-customer-accounts-capability-composition";\n',
    '''#[derive(Clone)]
pub struct CustomerAccountsProductionDependencies {
    pub store: PostgresDataStore,
    pub parties: Arc<dyn PartyReferenceReader>,
    pub activation: Arc<dyn ModuleActivationPort>,
    pub visibility_authorizer: Arc<dyn QueryVisibilityAuthorizer>,
    pub cursor_key: [u8; 32],
}

/// Builds the complete Customer Accounts mutation/query contribution inside
/// the authoritative owner package rather than the generic process host.
pub fn build_contribution(
    dependencies: CustomerAccountsProductionDependencies,
) -> Result<ModuleContributionSet, SdkError> {
    let CustomerAccountsProductionDependencies {
        store,
        parties,
        activation,
        visibility_authorizer,
        cursor_key,
    } = dependencies;
    let mut contributions = ModuleContributionSet::new();

    let mutation_executor: Arc<dyn TransactionalCapabilityExecutor> = Arc::new(
        PostgresTransactionalAggregateExecutor::new(
            store.clone(),
            Arc::new(CustomerAccountCapabilityPlanner),
        ),
    );
    let mutation_validator: Arc<dyn CapabilitySemanticValidator> = Arc::new(
        ActivationGatedMutationValidator::new(
            activation.clone(),
            Arc::new(AccountPartyReferenceSemanticValidator::new(parties)),
        ),
    );
    contributions
        .add_mutations(
            capability_definitions()?,
            mutation_validator,
            mutation_executor,
        )
        .map_err(production_composition_error)?;

    let query_adapter = Arc::new(AccountQueryAdapter::new(
        store,
        customer_accounts_cursor(cursor_key)?,
        visibility_authorizer,
    )?);
    let query_validator: Arc<dyn QuerySemanticValidator> = Arc::new(
        ActivationGatedQueryValidator::new(activation, query_adapter.clone()),
    );
    let query_executor: Arc<dyn QueryExecutor> = query_adapter;
    contributions
        .add_queries(
            query_capability_definitions()?,
            query_validator,
            query_executor,
        )
        .map_err(production_composition_error)?;

    Ok(contributions)
}

fn customer_accounts_cursor(key: [u8; 32]) -> Result<CursorCodec, SdkError> {
    CursorCodec::new(key).map_err(|error| {
        SdkError::new(
            "CUSTOMER_ACCOUNTS_CURSOR_CONFIGURATION_INVALID",
            ErrorCategory::Internal,
            false,
            "The Customer Accounts cursor configuration is invalid.",
        )
        .with_internal_reference(error.to_string())
    })
}

fn production_composition_error(error: impl fmt::Display) -> SdkError {
    SdkError::new(
        "CUSTOMER_ACCOUNTS_PRODUCTION_COMPOSITION_INVALID",
        ErrorCategory::Internal,
        false,
        "The Customer Accounts production contribution is invalid.",
    )
    .with_internal_reference(error.to_string())
}

pub const CRATE_NAME: &str = "crm-customer-accounts-capability-composition";
''',
)

replace_once(
    "crates/crm-application-runtime/src/native_composition.rs",
    '''use crm_customer_accounts_capability_adapter::{
    CustomerAccountCapabilityPlanner, capability_definitions as account_capability_definitions,
};
use crm_customer_accounts_capability_composition::AccountPartyReferenceSemanticValidator;
use crm_customer_accounts_query_adapter::{
    AccountQueryAdapter, query_capability_definitions as account_query_capability_definitions,
};
''',
    '''use crm_customer_accounts_capability_adapter::capability_definitions as account_capability_definitions;
use crm_customer_accounts_capability_composition::{
    CustomerAccountsProductionDependencies,
    build_contribution as build_customer_accounts_contribution,
};
use crm_customer_accounts_query_adapter::query_capability_definitions as account_query_capability_definitions;
''',
)

replace_once(
    "crates/crm-application-runtime/src/native_composition.rs",
    '''    let account_executor = aggregate_executor(store.clone(), CustomerAccountCapabilityPlanner);
    add_activated_mutations(
        &mut contributions,
        account_capability_definitions()?,
        Arc::new(AccountPartyReferenceSemanticValidator::new(parties.clone())),
        account_executor,
        activation.clone(),
    )?;

''',
    '''    contributions.merge(build_customer_accounts_contribution(
        CustomerAccountsProductionDependencies {
            store: store.clone(),
            parties: parties.clone(),
            activation: activation.clone(),
            visibility_authorizer: visibility_authorizer.clone(),
            cursor_key,
        },
    )?);

''',
)

replace_once(
    "crates/crm-application-runtime/src/native_composition.rs",
    '''    let account_queries = Arc::new(AccountQueryAdapter::new(
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

''',
    "",
)

replace_once(
    "scripts/check_native_module_composition.py",
    '''    LegacyMarker(
        "crates/crm-application-runtime/src/runtime.rs",
        "pub export_selection_worker:",
        "background work is still represented by fixed process fields",
    ),
)
''',
    '''    LegacyMarker(
        "crates/crm-application-runtime/src/runtime.rs",
        "pub export_selection_worker:",
        "background work is still represented by fixed process fields",
    ),
    LegacyMarker(
        "crates/crm-application-runtime/src/native_composition.rs",
        "CustomerAccountCapabilityPlanner",
        "Customer Accounts mutation construction returned to the generic process host",
    ),
    LegacyMarker(
        "crates/crm-application-runtime/src/native_composition.rs",
        "AccountPartyReferenceSemanticValidator",
        "Customer Accounts semantic validation returned to the generic process host",
    ),
    LegacyMarker(
        "crates/crm-application-runtime/src/native_composition.rs",
        "AccountQueryAdapter::new",
        "Customer Accounts query construction returned to the generic process host",
    ),
)
''',
)

Path("docs/GOLDEN_MODULE_CONTRIBUTION.md").write_text(
    '''# Golden Module Contribution — Customer Accounts

Status: first bounded production-contribution pilot

Customer Accounts is the first owner migrated from concrete construction in the generic application runtime to a module-owned contribution entry point.

## Why this owner

Customer Accounts has a stable but representative production surface:

- mutation definitions and an aggregate planner;
- live Party reference validation;
- permission-aware get/list queries;
- activation gating;
- PostgreSQL persistence and cursor configuration.

It is complex enough to prove the boundary while avoiding the exceptional orchestration of Identity Resolution, Data Operations or Customer Enrichment.

## Accepted shape

The existing `crm-customer-accounts-capability-composition` package now exposes:

```rust
pub fn build_contribution(
    dependencies: CustomerAccountsProductionDependencies,
) -> Result<ModuleContributionSet, SdkError>;
```

The owner package constructs its planner, semantic validator, query adapter and activation gates internally. The generic application runtime supplies only production context and merges the resulting contribution set.

No new crate is introduced. The transitional composition package is used as the owner production boundary for this pilot.

## Invariants

- public capability coordinates and contracts remain unchanged;
- data-only definition inventory remains available for grants and parity checks;
- final assembly still rejects duplicate routes, owner mismatch and route-kind mismatch;
- the application runtime no longer constructs Customer Accounts planners, validators or query adapters;
- full exact-head acceptance remains mandatory;
- this pilot does not yet introduce the first-party aggregate package or affected-scope skipping.

## Mechanical guard

`check_native_module_composition.py` rejects reintroduction of concrete Customer Accounts planner, validator or query-adapter construction in `crm-application-runtime/src/native_composition.rs`.

## Next comparison

After this pilot is accepted, migrate a second contrasting owner through the same generic merge seam. Only then stabilize a reusable first-party aggregate and decide whether naming/package consolidation is warranted.
''',
    encoding="utf-8",
)
