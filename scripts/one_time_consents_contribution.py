#!/usr/bin/env python3
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def replace_once(path: Path, old: str, new: str) -> None:
    content = path.read_text(encoding="utf-8")
    if content.count(old) != 1:
        raise SystemExit(f"expected exactly one match in {path}: {old!r}")
    path.write_text(content.replace(old, new, 1), encoding="utf-8")


cargo = ROOT / "crates/crm-consents-capability-composition/Cargo.toml"
replace_once(
    cargo,
    "[dependencies]\ncrm-capability-runtime = { path = \"../crm-capability-runtime\" }\n",
    "[dependencies]\n"
    "crm-application-composition = { path = \"../crm-application-composition\" }\n"
    "crm-capability-runtime = { path = \"../crm-capability-runtime\" }\n",
)
replace_once(
    cargo,
    "crm-consents-capability-adapter = { path = \"../crm-consents-capability-adapter\" }\n",
    "crm-consents-capability-adapter = { path = \"../crm-consents-capability-adapter\" }\n"
    "crm-consents-query-adapter = { path = \"../crm-consents-query-adapter\" }\n",
)
replace_once(
    cargo,
    "crm-parties-capability-adapter = { path = \"../crm-parties-capability-adapter\" }\n",
    "crm-parties-capability-adapter = { path = \"../crm-parties-capability-adapter\" }\n"
    "crm-query-runtime = { path = \"../crm-query-runtime\" }\n",
)

lib = ROOT / "crates/crm-consents-capability-composition/src/lib.rs"
replace_once(
    lib,
    "use crm_capability_runtime::{\n",
    "use crm_application_composition::{\n"
    "    ActivationGatedMutationValidator, ActivationGatedQueryValidator, ModuleActivationPort,\n"
    "    ModuleContributionSet,\n"
    "};\n"
    "use crm_capability_runtime::{\n",
)
replace_once(
    lib,
    "use crm_consents_capability_adapter::{\n"
    "    CREATE_CAPABILITY, CreateConsentReferenceScope, MUTATION_CAPABILITY_IDS,\n"
    "    referenced_scope_from_create,\n"
    "};\n",
    "use crm_consents_capability_adapter::{\n"
    "    CREATE_CAPABILITY, ConsentCapabilityPlanner, CreateConsentReferenceScope,\n"
    "    MUTATION_CAPABILITY_IDS, capability_definitions, referenced_scope_from_create,\n"
    "};\n"
    "use crm_consents_query_adapter::{ConsentQueryAdapter, query_capability_definitions};\n",
)
replace_once(
    lib,
    "use crm_core_data::{PostgresDataStore, RecordGetQuery};\n",
    "use crm_core_data::{\n"
    "    PostgresDataStore, PostgresTransactionalAggregateExecutor, RecordGetQuery,\n"
    "};\n",
)
replace_once(
    lib,
    "use crm_parties_capability_adapter::{\n"
    "    MODULE_ID as PARTIES_MODULE_ID, RECORD_TYPE as PARTY_RECORD_TYPE,\n"
    "};\n",
    "use crm_parties_capability_adapter::{\n"
    "    MODULE_ID as PARTIES_MODULE_ID, RECORD_TYPE as PARTY_RECORD_TYPE,\n"
    "};\n"
    "use crm_query_runtime::{\n"
    "    CursorCodec, QueryExecutor, QuerySemanticValidator, QueryVisibilityAuthorizer,\n"
    "};\n",
)

production_code = r'''
#[derive(Clone)]
pub struct ConsentsProductionDependencies {
    pub store: PostgresDataStore,
    pub activation: Arc<dyn ModuleActivationPort>,
    pub visibility_authorizer: Arc<dyn QueryVisibilityAuthorizer>,
    pub cursor_key: [u8; 32],
}

/// Builds the complete Consents mutation/query contribution inside the owner
/// composition package while preserving the module's richer reference checks.
pub fn build_contribution(
    dependencies: ConsentsProductionDependencies,
) -> Result<ModuleContributionSet, SdkError> {
    let ConsentsProductionDependencies {
        store,
        activation,
        visibility_authorizer,
        cursor_key,
    } = dependencies;
    let mut contributions = ModuleContributionSet::new();

    let aggregate: Arc<dyn TransactionalCapabilityExecutor> =
        Arc::new(PostgresTransactionalAggregateExecutor::new(
            store.clone(),
            Arc::new(ConsentCapabilityPlanner),
        ));
    let mutation_executor: Arc<dyn TransactionalCapabilityExecutor> =
        Arc::new(ConsentCapabilityExecutor::new(aggregate));
    let mutation_validator: Arc<dyn CapabilitySemanticValidator> =
        Arc::new(ActivationGatedMutationValidator::new(
            activation.clone(),
            Arc::new(ConsentCapabilitySemanticValidator::new(Arc::new(
                PostgresConsentReferenceReader::new(store.clone()),
            ))),
        ));
    contributions
        .add_mutations(
            capability_definitions()?,
            mutation_validator,
            mutation_executor,
        )
        .map_err(production_composition_error)?;

    let query_adapter = Arc::new(ConsentQueryAdapter::new(
        store,
        consents_cursor(cursor_key)?,
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

fn consents_cursor(key: [u8; 32]) -> Result<CursorCodec, SdkError> {
    CursorCodec::new(key).map_err(|error| {
        SdkError::new(
            "CONSENTS_CURSOR_CONFIGURATION_INVALID",
            ErrorCategory::Internal,
            false,
            "The Consents cursor configuration is invalid.",
        )
        .with_internal_reference(error.to_string())
    })
}

fn production_composition_error(error: impl fmt::Display) -> SdkError {
    SdkError::new(
        "CONSENTS_PRODUCTION_COMPOSITION_INVALID",
        ErrorCategory::Internal,
        false,
        "The Consents production contribution is invalid.",
    )
    .with_internal_reference(error.to_string())
}

'''
replace_once(lib, "fn reference_unavailable() -> SdkError {\n", production_code + "fn reference_unavailable() -> SdkError {\n")

runtime = ROOT / "crates/crm-application-runtime/src/native_composition.rs"
replace_once(
    runtime,
    "use crm_consents_capability_adapter::{\n"
    "    ConsentCapabilityPlanner, capability_definitions as consent_capability_definitions,\n"
    "};\n"
    "use crm_consents_capability_composition::{\n"
    "    ConsentCapabilityExecutor, ConsentCapabilitySemanticValidator, PostgresConsentReferenceReader,\n"
    "};\n",
    "use crm_consents_capability_adapter::capability_definitions as consent_capability_definitions;\n"
    "use crm_consents_capability_composition::{\n"
    "    ConsentsProductionDependencies, build_contribution as build_consents_contribution,\n"
    "};\n",
)
replace_once(
    runtime,
    "    let consent_aggregate = aggregate_executor(store.clone(), ConsentCapabilityPlanner);\n"
    "    add_activated_mutations(\n"
    "        &mut contributions,\n"
    "        consent_capability_definitions()?,\n"
    "        Arc::new(ConsentCapabilitySemanticValidator::new(Arc::new(\n"
    "            PostgresConsentReferenceReader::new(store.clone()),\n"
    "        ))),\n"
    "        Arc::new(ConsentCapabilityExecutor::new(consent_aggregate)),\n"
    "        activation.clone(),\n"
    "    )?;\n",
    "    contributions.merge(build_consents_contribution(ConsentsProductionDependencies {\n"
    "        store: store.clone(),\n"
    "        activation: activation.clone(),\n"
    "        visibility_authorizer: visibility_authorizer.clone(),\n"
    "        cursor_key,\n"
    "    })?);\n",
)
replace_once(
    runtime,
    "    let consent_queries = Arc::new(ConsentQueryAdapter::new(\n"
    "        store.clone(),\n"
    "        cursor(cursor_key)?,\n"
    "        visibility_authorizer.clone(),\n"
    "    )?);\n"
    "    add_activated_queries(\n"
    "        &mut contributions,\n"
    "        consent_query_capability_definitions()?,\n"
    "        consent_queries,\n"
    "        activation.clone(),\n"
    "    )?;\n\n",
    "",
)

guard = ROOT / "scripts/check_native_module_composition.py"
replace_once(
    guard,
    "    LegacyMarker(\n"
    "        \"crates/crm-application-runtime/src/native_composition.rs\",\n"
    "        \"AccountQueryAdapter::new\",\n"
    "        \"Customer Accounts query construction returned to the generic process host\",\n"
    "    ),\n"
    ")\n",
    "    LegacyMarker(\n"
    "        \"crates/crm-application-runtime/src/native_composition.rs\",\n"
    "        \"AccountQueryAdapter::new\",\n"
    "        \"Customer Accounts query construction returned to the generic process host\",\n"
    "    ),\n"
    "    LegacyMarker(\n"
    "        \"crates/crm-application-runtime/src/native_composition.rs\",\n"
    "        \"ConsentCapabilityPlanner\",\n"
    "        \"Consents mutation construction returned to the generic process host\",\n"
    "    ),\n"
    "    LegacyMarker(\n"
    "        \"crates/crm-application-runtime/src/native_composition.rs\",\n"
    "        \"ConsentCapabilitySemanticValidator\",\n"
    "        \"Consents semantic validation returned to the generic process host\",\n"
    "    ),\n"
    "    LegacyMarker(\n"
    "        \"crates/crm-application-runtime/src/native_composition.rs\",\n"
    "        \"PostgresConsentReferenceReader\",\n"
    "        \"Consents reference-reader construction returned to the generic process host\",\n"
    "    ),\n"
    "    LegacyMarker(\n"
    "        \"crates/crm-application-runtime/src/native_composition.rs\",\n"
    "        \"let consent_queries = Arc::new(ConsentQueryAdapter::new(\",\n"
    "        \"Consents route registration returned to the generic process host\",\n"
    "    ),\n"
    ")\n",
)

comparison = ROOT / "docs/CONSENTS_MODULE_CONTRIBUTION.md"
comparison.write_text(
    """# Second Module-Owned Production Contribution — Consents\n\n"
    "Status: bounded comparison candidate\n\n"
    "Consents is the second stable owner moved from concrete construction in the generic application runtime to an owner-built `ModuleContributionSet`.\n\n"
    "## Why this owner contrasts with Customer Accounts\n\n"
    "Customer Accounts validates Party references through one shared reader. Consents validates a richer scope across Party and optional Contact Point records, checks ownership and communication-channel compatibility, wraps the aggregate executor with owner-specific capability validation, and exposes permission-aware queries.\n\n"
    "## Boundary\n\n"
    "The existing `crm-consents-capability-composition` package owns mutation planner/executor construction, PostgreSQL reference reading, semantic validation, query adapter construction and activation gates. The generic application runtime supplies only production context and merges the contribution.\n\n"
    "Customer Enrichment may still construct a `ConsentQueryAdapter` as an explicit cross-owner query dependency. The mechanical guard therefore forbids only central Consents route registration and owner mutation/reference construction, not every use of the query-adapter type.\n\n"
    "## Decision boundary\n\n"
    "This packet introduces no new crate and does not yet add a first-party aggregate package. After exact-head acceptance, Customer Accounts and Consents provide two contrasting examples from which the common production-contribution shape can be stabilized without inventing a framework from one owner.\n"
    """,
    encoding="utf-8",
)
