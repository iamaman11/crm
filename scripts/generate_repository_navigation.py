#!/usr/bin/env python3
"""Temporarily materialize repository step 12 batch 2, then write navigation."""

from __future__ import annotations

import argparse
import os
from pathlib import Path
import subprocess
import sys

from repository_navigation import (
    NavigationError,
    stale_generated_documents,
    write_generated_documents,
)


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path.cwd())
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--write", action="store_true")
    mode.add_argument("--check", action="store_true")
    return parser


def replace_once(root: Path, relative: str, old: str, new: str) -> None:
    path = root / relative
    content = path.read_text(encoding="utf-8")
    if new in content and old not in content:
        return
    count = content.count(old)
    if count != 1:
        raise RuntimeError(f"{relative}: expected one anchor, found {count}: {old[:120]!r}")
    path.write_text(content.replace(old, new, 1), encoding="utf-8")


def write_exact(root: Path, relative: str, content: str) -> None:
    path = root / relative
    path.parent.mkdir(parents=True, exist_ok=True)
    if path.exists() and path.read_text(encoding="utf-8") == content:
        return
    path.write_text(content, encoding="utf-8")


def move_data_quality_sources(root: Path) -> None:
    moves = (
        (
            "crates/crm-application-runtime/src/data_quality_capability_execution.rs",
            "crates/crm-data-quality-source-composition/src/capability_execution.rs",
        ),
        (
            "crates/crm-application-runtime/src/data_quality_registration.rs",
            "crates/crm-data-quality-source-composition/src/registration.rs",
        ),
    )
    for source_relative, target_relative in moves:
        source = root / source_relative
        target = root / target_relative
        if source.exists():
            content = source.read_text(encoding="utf-8")
            if source_relative.endswith("data_quality_capability_execution.rs"):
                content = content.replace(
                    "use crm_data_quality_source_composition::{\n"
                    "    GovernedPartyQualitySource, PartyQualitySource, PartyQualitySourceRequest,\n"
                    "};",
                    "use crate::{\n"
                    "    GovernedPartyQualitySource, PartyQualitySource, PartyQualitySourceRequest,\n"
                    "};",
                )
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_text(content, encoding="utf-8")
            source.unlink()
        elif not target.exists():
            raise RuntimeError(f"missing both {source_relative} and {target_relative}")


def identity_production_source() -> str:
    return '''#![forbid(unsafe_code)]

use crate::{
    IdentityResolutionCapabilityExecutor, IdentityResolutionCapabilitySemanticValidator,
    PostgresIdentityResolutionReferenceReader,
};
use crm_application_composition::{
    ActivationGatedMutationValidator, ActivationGatedQueryValidator, ModuleActivationPort,
    ModuleContributionSet,
};
use crm_capability_runtime::{
    CapabilityDefinition, CapabilitySemanticValidator, TransactionalCapabilityExecutor,
};
use crm_core_data::{PostgresDataStore, PostgresTransactionalAggregateExecutor};
use crm_identity_resolution_capability_adapter::{
    CANDIDATE_MUTATION_CAPABILITY_IDS, IdentityResolutionCapabilityPlanner,
    MERGE_MUTATION_CAPABILITY_IDS,
    capability_definitions as adapter_mutation_capability_definitions,
};
use crm_identity_resolution_merge_composition::{
    MergeLineageCapabilityExecutor, MergeLineageCapabilitySemanticValidator,
    PostgresMergeLineageReferenceReader,
};
use crm_identity_resolution_merge_query_adapter::{
    IdentityResolutionMergeQueryAdapter,
    query_capability_definitions as merge_query_capability_definitions,
};
use crm_identity_resolution_query_adapter::{
    IdentityResolutionQueryAdapter,
    query_capability_definitions as candidate_query_capability_definitions,
};
use crm_module_sdk::{ErrorCategory, SdkError};
use crm_query_runtime::{
    CursorCodec, QueryExecutor, QuerySemanticValidator, QueryVisibilityAuthorizer,
};
use std::fmt;
use std::sync::Arc;

#[derive(Clone)]
pub struct IdentityResolutionProductionDependencies {
    pub store: PostgresDataStore,
    pub activation: Arc<dyn ModuleActivationPort>,
    pub visibility_authorizer: Arc<dyn QueryVisibilityAuthorizer>,
    pub cursor_key: [u8; 32],
}

pub fn mutation_capability_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {
    adapter_mutation_capability_definitions()
}

pub fn query_capability_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {
    let mut definitions = candidate_query_capability_definitions()?;
    definitions.extend(merge_query_capability_definitions()?);
    Ok(definitions)
}

pub fn build_contribution(
    dependencies: IdentityResolutionProductionDependencies,
) -> Result<ModuleContributionSet, SdkError> {
    let IdentityResolutionProductionDependencies {
        store,
        activation,
        visibility_authorizer,
        cursor_key,
    } = dependencies;
    let mut contributions = ModuleContributionSet::new();
    let identity_aggregate: Arc<dyn TransactionalCapabilityExecutor> =
        Arc::new(PostgresTransactionalAggregateExecutor::new(
            store.clone(),
            Arc::new(IdentityResolutionCapabilityPlanner),
        ));
    let definitions = mutation_capability_definitions()?;

    let candidate_validator: Arc<dyn CapabilitySemanticValidator> = Arc::new(
        ActivationGatedMutationValidator::new(
            activation.clone(),
            Arc::new(IdentityResolutionCapabilitySemanticValidator::new(Arc::new(
                PostgresIdentityResolutionReferenceReader::new(store.clone()),
            ))),
        ),
    );
    contributions
        .add_mutations(
            select_definitions(&definitions, CANDIDATE_MUTATION_CAPABILITY_IDS),
            candidate_validator,
            Arc::new(IdentityResolutionCapabilityExecutor::new(
                identity_aggregate.clone(),
            )),
        )
        .map_err(production_composition_error)?;

    let merge_validator: Arc<dyn CapabilitySemanticValidator> = Arc::new(
        ActivationGatedMutationValidator::new(
            activation.clone(),
            Arc::new(MergeLineageCapabilitySemanticValidator::new(Arc::new(
                PostgresMergeLineageReferenceReader::new(store.clone()),
            ))),
        ),
    );
    contributions
        .add_mutations(
            select_definitions(&definitions, MERGE_MUTATION_CAPABILITY_IDS),
            merge_validator,
            Arc::new(MergeLineageCapabilityExecutor::new(identity_aggregate)),
        )
        .map_err(production_composition_error)?;

    let candidate_queries = Arc::new(IdentityResolutionQueryAdapter::new(
        store.clone(),
        cursor(cursor_key)?,
        visibility_authorizer.clone(),
    )?);
    let candidate_query_validator: Arc<dyn QuerySemanticValidator> = Arc::new(
        ActivationGatedQueryValidator::new(activation.clone(), candidate_queries.clone()),
    );
    let candidate_query_executor: Arc<dyn QueryExecutor> = candidate_queries;
    contributions
        .add_queries(
            candidate_query_capability_definitions()?,
            candidate_query_validator,
            candidate_query_executor,
        )
        .map_err(production_composition_error)?;

    let merge_queries = Arc::new(IdentityResolutionMergeQueryAdapter::new(
        store,
        cursor(cursor_key)?,
        visibility_authorizer,
    )?);
    let merge_query_validator: Arc<dyn QuerySemanticValidator> = Arc::new(
        ActivationGatedQueryValidator::new(activation, merge_queries.clone()),
    );
    let merge_query_executor: Arc<dyn QueryExecutor> = merge_queries;
    contributions
        .add_queries(
            merge_query_capability_definitions()?,
            merge_query_validator,
            merge_query_executor,
        )
        .map_err(production_composition_error)?;

    Ok(contributions)
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
            "IDENTITY_RESOLUTION_CURSOR_CONFIGURATION_INVALID",
            ErrorCategory::Internal,
            false,
            "The Identity Resolution cursor configuration is invalid.",
        )
        .with_internal_reference(error.to_string())
    })
}

fn production_composition_error(error: impl fmt::Display) -> SdkError {
    SdkError::new(
        "IDENTITY_RESOLUTION_PRODUCTION_COMPOSITION_INVALID",
        ErrorCategory::Internal,
        false,
        "The Identity Resolution production contribution is invalid.",
    )
    .with_internal_reference(error.to_string())
}
'''


def customer_data_operations_production_source() -> str:
    return '''#![forbid(unsafe_code)]

use crm_application_composition::{
    ActivationGatedMutationValidator, ActivationGatedQueryValidator, ModuleActivationPort,
    ModuleContributionSet, NoopMutationSemanticValidator,
};
use crm_capability_runtime::{
    CapabilityAuthorizer, CapabilityDefinition, CapabilitySemanticValidator,
    TransactionalCapabilityExecutor,
};
use crm_core_data::{
    PostgresDataStore, PostgresImmutableFileArtifactStore,
    PostgresTransactionalAggregateExecutor,
};
use crm_customer_data_operations_capability_adapter::{
    CREATE_PARTY_IMPORT_JOB_CAPABILITY, CustomerDataOperationsCapabilityPlanner,
    VALIDATE_PARTY_IMPORT_ROWS_CAPABILITY,
    capability_definitions as adapter_mutation_capability_definitions,
};
use crm_customer_data_operations_query_adapter::{
    CustomerDataOperationsQueryAdapter,
    query_capability_definitions as adapter_query_capability_definitions,
};
use crm_customer_data_operations_source_composition::{
    CustomerDataOperationsSourceExecutor,
    source_capability_definitions as source_mutation_capability_definitions,
};
use crm_module_sdk::{ErrorCategory, SdkError};
use crm_query_runtime::{
    CursorCodec, QueryExecutor, QuerySemanticValidator, QueryVisibilityAuthorizer,
};
use std::fmt;
use std::sync::Arc;

#[derive(Clone)]
pub struct CustomerDataOperationsProductionDependencies {
    pub store: PostgresDataStore,
    pub activation: Arc<dyn ModuleActivationPort>,
    pub capability_authorizer: Arc<dyn CapabilityAuthorizer>,
    pub visibility_authorizer: Arc<dyn QueryVisibilityAuthorizer>,
    pub cursor_key: [u8; 32],
}

pub fn mutation_capability_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {
    let mut definitions = ordinary_mutation_capability_definitions()?;
    definitions.extend(source_mutation_capability_definitions()?);
    Ok(definitions)
}

pub fn query_capability_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {
    adapter_query_capability_definitions()
}

pub fn build_contribution(
    dependencies: CustomerDataOperationsProductionDependencies,
) -> Result<ModuleContributionSet, SdkError> {
    let CustomerDataOperationsProductionDependencies {
        store,
        activation,
        capability_authorizer,
        visibility_authorizer,
        cursor_key,
    } = dependencies;
    let mut contributions = ModuleContributionSet::new();

    let ordinary_executor: Arc<dyn TransactionalCapabilityExecutor> =
        Arc::new(PostgresTransactionalAggregateExecutor::new(
            store.clone(),
            Arc::new(CustomerDataOperationsCapabilityPlanner),
        ));
    let ordinary_validator: Arc<dyn CapabilitySemanticValidator> = Arc::new(
        ActivationGatedMutationValidator::new(
            activation.clone(),
            Arc::new(NoopMutationSemanticValidator),
        ),
    );
    contributions
        .add_mutations(
            ordinary_mutation_capability_definitions()?,
            ordinary_validator,
            ordinary_executor,
        )
        .map_err(production_composition_error)?;

    let source_executor: Arc<dyn TransactionalCapabilityExecutor> =
        Arc::new(CustomerDataOperationsSourceExecutor::new(
            store.clone(),
            Arc::new(PostgresImmutableFileArtifactStore::new(store.clone())),
            capability_authorizer,
        ));
    let source_validator: Arc<dyn CapabilitySemanticValidator> = Arc::new(
        ActivationGatedMutationValidator::new(
            activation.clone(),
            Arc::new(NoopMutationSemanticValidator),
        ),
    );
    contributions
        .add_mutations(
            source_mutation_capability_definitions()?,
            source_validator,
            source_executor,
        )
        .map_err(production_composition_error)?;

    let queries = Arc::new(CustomerDataOperationsQueryAdapter::new(
        store,
        cursor(cursor_key)?,
        visibility_authorizer,
    )?);
    let query_validator: Arc<dyn QuerySemanticValidator> = Arc::new(
        ActivationGatedQueryValidator::new(activation, queries.clone()),
    );
    let query_executor: Arc<dyn QueryExecutor> = queries;
    contributions
        .add_queries(
            query_capability_definitions()?,
            query_validator,
            query_executor,
        )
        .map_err(production_composition_error)?;

    Ok(contributions)
}

fn ordinary_mutation_capability_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {
    Ok(adapter_mutation_capability_definitions()?
        .into_iter()
        .filter(|definition| {
            !matches!(
                definition.capability_id.as_str(),
                CREATE_PARTY_IMPORT_JOB_CAPABILITY | VALIDATE_PARTY_IMPORT_ROWS_CAPABILITY
            )
        })
        .collect())
}

fn cursor(key: [u8; 32]) -> Result<CursorCodec, SdkError> {
    CursorCodec::new(key).map_err(|error| {
        SdkError::new(
            "CUSTOMER_DATA_OPERATIONS_CURSOR_CONFIGURATION_INVALID",
            ErrorCategory::Internal,
            false,
            "The Customer Data Operations cursor configuration is invalid.",
        )
        .with_internal_reference(error.to_string())
    })
}

fn production_composition_error(error: impl fmt::Display) -> SdkError {
    SdkError::new(
        "CUSTOMER_DATA_OPERATIONS_PRODUCTION_COMPOSITION_INVALID",
        ErrorCategory::Internal,
        false,
        "The Customer Data Operations production contribution is invalid.",
    )
    .with_internal_reference(error.to_string())
}
'''


def data_quality_production_source() -> str:
    return '''#![forbid(unsafe_code)]

use crate::{DataQualityAggregatePlanner, DataQualityCapabilityExecutor};
use crm_application_composition::{
    ActivationGatedMutationValidator, ActivationGatedQueryValidator, ModuleActivationPort,
    ModuleContributionSet, NoopMutationSemanticValidator,
};
use crm_capability_runtime::{
    CapabilityAuthorizer, CapabilityDefinition, CapabilitySemanticValidator,
    TransactionalCapabilityExecutor,
};
use crm_core_data::{PostgresDataStore, PostgresTransactionalAggregateExecutor};
use crm_data_quality_capability_adapter::{
    capability_definitions as adapter_mutation_capability_definitions,
};
use crm_data_quality_query_adapter::{
    DataQualityQueryAdapter, query_capability_definitions as adapter_query_capability_definitions,
};
use crm_module_sdk::{ErrorCategory, SdkError};
use crm_parties_capability_adapter::PartyCapabilityPlanner;
use crm_query_runtime::{
    QueryAuthorizer, QueryExecutor, QuerySemanticValidator, QueryVisibilityAuthorizer,
};
use std::fmt;
use std::sync::Arc;

#[derive(Clone)]
pub struct DataQualityProductionDependencies {
    pub store: PostgresDataStore,
    pub activation: Arc<dyn ModuleActivationPort>,
    pub capability_authorizer: Arc<dyn CapabilityAuthorizer>,
    pub query_authorizer: Arc<dyn QueryAuthorizer>,
    pub visibility_authorizer: Arc<dyn QueryVisibilityAuthorizer>,
}

pub fn mutation_capability_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {
    adapter_mutation_capability_definitions()
}

pub fn query_capability_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {
    adapter_query_capability_definitions()
}

pub fn build_contribution(
    dependencies: DataQualityProductionDependencies,
) -> Result<ModuleContributionSet, SdkError> {
    let DataQualityProductionDependencies {
        store,
        activation,
        capability_authorizer,
        query_authorizer,
        visibility_authorizer,
    } = dependencies;
    let mut contributions = ModuleContributionSet::new();

    let fallback: Arc<dyn TransactionalCapabilityExecutor> =
        Arc::new(PostgresTransactionalAggregateExecutor::new(
            store.clone(),
            Arc::new(DataQualityAggregatePlanner),
        ));
    let party_update_executor: Arc<dyn TransactionalCapabilityExecutor> =
        Arc::new(PostgresTransactionalAggregateExecutor::new(
            store.clone(),
            Arc::new(PartyCapabilityPlanner),
        ));
    let mutation_executor: Arc<dyn TransactionalCapabilityExecutor> =
        Arc::new(DataQualityCapabilityExecutor::new(
            store.clone(),
            fallback,
            party_update_executor,
            activation.clone(),
            capability_authorizer,
            query_authorizer,
        ));
    let mutation_validator: Arc<dyn CapabilitySemanticValidator> = Arc::new(
        ActivationGatedMutationValidator::new(
            activation.clone(),
            Arc::new(NoopMutationSemanticValidator),
        ),
    );
    contributions
        .add_mutations(
            mutation_capability_definitions()?,
            mutation_validator,
            mutation_executor,
        )
        .map_err(production_composition_error)?;

    let queries = Arc::new(DataQualityQueryAdapter::new(store, visibility_authorizer));
    let query_validator: Arc<dyn QuerySemanticValidator> = Arc::new(
        ActivationGatedQueryValidator::new(activation, queries.clone()),
    );
    let query_executor: Arc<dyn QueryExecutor> = queries;
    contributions
        .add_queries(
            query_capability_definitions()?,
            query_validator,
            query_executor,
        )
        .map_err(production_composition_error)?;

    Ok(contributions)
}

fn production_composition_error(error: impl fmt::Display) -> SdkError {
    SdkError::new(
        "DATA_QUALITY_PRODUCTION_COMPOSITION_INVALID",
        ErrorCategory::Internal,
        false,
        "The Data Quality production contribution is invalid.",
    )
    .with_internal_reference(error.to_string())
}
'''


def materialize(root: Path) -> None:
    write_exact(
        root,
        "crates/crm-identity-resolution-capability-composition/src/production_contribution.rs",
        identity_production_source(),
    )
    write_exact(
        root,
        "crates/crm-customer-data-operations-execution-composition/src/production_contribution.rs",
        customer_data_operations_production_source(),
    )
    move_data_quality_sources(root)
    write_exact(
        root,
        "crates/crm-data-quality-source-composition/src/production_contribution.rs",
        data_quality_production_source(),
    )

    replace_once(
        root,
        "crates/crm-identity-resolution-capability-composition/src/lib.rs",
        "#![forbid(unsafe_code)]\n",
        "#![forbid(unsafe_code)]\n\nmod production_contribution;\npub use production_contribution::*;\n",
    )
    replace_once(
        root,
        "crates/crm-customer-data-operations-execution-composition/src/lib.rs",
        "#![forbid(unsafe_code)]\n",
        "#![forbid(unsafe_code)]\n\nmod production_contribution;\npub use production_contribution::*;\n",
    )
    replace_once(
        root,
        "crates/crm-data-quality-source-composition/src/lib.rs",
        "mod materialization_sink;\n",
        "mod capability_execution;\nmod registration;\nmod production_contribution;\npub use capability_execution::DataQualityCapabilityExecutor;\npub use production_contribution::*;\npub use registration::DataQualityAggregatePlanner;\n\nmod materialization_sink;\n",
    )

    replace_once(
        root,
        "crates/crm-identity-resolution-capability-composition/Cargo.toml",
        "[dependencies]\ncrm-capability-plan-support",
        "[dependencies]\ncrm-application-composition = { path = \"../crm-application-composition\" }\ncrm-capability-plan-support",
    )
    replace_once(
        root,
        "crates/crm-identity-resolution-capability-composition/Cargo.toml",
        "crm-identity-resolution-capability-adapter = { path = \"../crm-identity-resolution-capability-adapter\" }\ncrm-module-sdk",
        "crm-identity-resolution-capability-adapter = { path = \"../crm-identity-resolution-capability-adapter\" }\ncrm-identity-resolution-merge-composition = { path = \"../crm-identity-resolution-merge-composition\" }\ncrm-identity-resolution-merge-query-adapter = { path = \"../crm-identity-resolution-merge-query-adapter\" }\ncrm-identity-resolution-query-adapter = { path = \"../crm-identity-resolution-query-adapter\" }\ncrm-module-sdk",
    )
    replace_once(
        root,
        "crates/crm-identity-resolution-capability-composition/Cargo.toml",
        "crm-proto-contracts = { path = \"../crm-proto-contracts\" }\n",
        "crm-proto-contracts = { path = \"../crm-proto-contracts\" }\ncrm-query-runtime = { path = \"../crm-query-runtime\" }\n",
    )

    replace_once(
        root,
        "crates/crm-customer-data-operations-execution-composition/Cargo.toml",
        "[dependencies]\ncrm-capability-adapters",
        "[dependencies]\ncrm-application-composition = { path = \"../crm-application-composition\" }\ncrm-capability-adapters",
    )
    replace_once(
        root,
        "crates/crm-customer-data-operations-execution-composition/Cargo.toml",
        "crm-customer-data-operations-capability-adapter = { path = \"../crm-customer-data-operations-capability-adapter\" }\ncrm-module-sdk",
        "crm-customer-data-operations-capability-adapter = { path = \"../crm-customer-data-operations-capability-adapter\" }\ncrm-customer-data-operations-query-adapter = { path = \"../crm-customer-data-operations-query-adapter\" }\ncrm-customer-data-operations-source-composition = { path = \"../crm-customer-data-operations-source-composition\" }\ncrm-module-sdk",
    )
    replace_once(
        root,
        "crates/crm-customer-data-operations-execution-composition/Cargo.toml",
        "crm-proto-contracts = { path = \"../crm-proto-contracts\" }\nprost",
        "crm-proto-contracts = { path = \"../crm-proto-contracts\" }\ncrm-query-runtime = { path = \"../crm-query-runtime\" }\nprost",
    )

    replace_once(
        root,
        "crates/crm-data-quality-source-composition/Cargo.toml",
        "[dependencies]\ncrm-capability-adapters",
        "[dependencies]\ncrm-application-composition = { path = \"../crm-application-composition\" }\ncrm-capability-adapters",
    )
    replace_once(
        root,
        "crates/crm-data-quality-source-composition/Cargo.toml",
        "crm-data-quality-capability-adapter = { path = \"../crm-data-quality-capability-adapter\" }\ncrm-module-sdk",
        "crm-data-quality-capability-adapter = { path = \"../crm-data-quality-capability-adapter\" }\ncrm-data-quality-query-adapter = { path = \"../crm-data-quality-query-adapter\" }\ncrm-module-sdk",
    )
    replace_once(
        root,
        "crates/crm-data-quality-source-composition/Cargo.toml",
        "crm-module-sdk = { path = \"../crm-module-sdk\" }\ncrm-parties-query-adapter",
        "crm-module-sdk = { path = \"../crm-module-sdk\" }\ncrm-parties-capability-adapter = { path = \"../crm-parties-capability-adapter\" }\ncrm-parties-query-adapter",
    )

    replace_once(
        root,
        "crates/crm-first-party-modules/Cargo.toml",
        "crm-application-composition = { path = \"../crm-application-composition\" }\ncrm-core-data",
        "crm-application-composition = { path = \"../crm-application-composition\" }\ncrm-capability-runtime = { path = \"../crm-capability-runtime\" }\ncrm-core-data",
    )
    replace_once(
        root,
        "crates/crm-first-party-modules/Cargo.toml",
        "crm-contact-points-capability-composition = { path = \"../crm-contact-points-capability-composition\" }\ncrm-module-sdk",
        "crm-contact-points-capability-composition = { path = \"../crm-contact-points-capability-composition\" }\ncrm-customer-data-operations-execution-composition = { path = \"../crm-customer-data-operations-execution-composition\" }\ncrm-data-quality-source-composition = { path = \"../crm-data-quality-source-composition\" }\ncrm-identity-resolution-capability-composition = { path = \"../crm-identity-resolution-capability-composition\" }\ncrm-module-sdk",
    )

    first_party = root / "crates/crm-first-party-modules/src/lib.rs"
    content = first_party.read_text(encoding="utf-8")
    if "IdentityResolutionProductionDependencies" not in content:
        content = content.replace(
            "use crm_application_composition::{ModuleActivationPort, ModuleContributionSet};\n",
            "use crm_application_composition::{ModuleActivationPort, ModuleContributionSet};\n"
            "use crm_capability_runtime::CapabilityAuthorizer;\n",
            1,
        )
        content = content.replace(
            "use crm_core_data::PostgresDataStore;\n",
            "use crm_core_data::PostgresDataStore;\n"
            "use crm_customer_data_operations_execution_composition::{\n"
            "    CustomerDataOperationsProductionDependencies,\n"
            "    build_contribution as build_customer_data_operations_contribution,\n"
            "};\n"
            "pub use crm_customer_data_operations_execution_composition::{\n"
            "    mutation_capability_definitions as customer_data_operations_mutation_capability_definitions,\n"
            "    query_capability_definitions as customer_data_operations_query_capability_definitions,\n"
            "};\n"
            "use crm_data_quality_source_composition::{\n"
            "    DataQualityProductionDependencies,\n"
            "    build_contribution as build_data_quality_contribution,\n"
            "};\n"
            "pub use crm_data_quality_source_composition::{\n"
            "    mutation_capability_definitions as data_quality_mutation_capability_definitions,\n"
            "    query_capability_definitions as data_quality_query_capability_definitions,\n"
            "};\n"
            "use crm_identity_resolution_capability_composition::{\n"
            "    IdentityResolutionProductionDependencies,\n"
            "    build_contribution as build_identity_resolution_contribution,\n"
            "};\n"
            "pub use crm_identity_resolution_capability_composition::{\n"
            "    mutation_capability_definitions as identity_resolution_mutation_capability_definitions,\n"
            "    query_capability_definitions as identity_resolution_query_capability_definitions,\n"
            "};\n",
            1,
        )
        content = content.replace(
            "use crm_query_runtime::QueryVisibilityAuthorizer;\n",
            "use crm_query_runtime::{QueryAuthorizer, QueryVisibilityAuthorizer};\n",
            1,
        )
        content = content.replace(
            "    pub activation: Arc<dyn ModuleActivationPort>,\n    pub visibility_authorizer: Arc<dyn QueryVisibilityAuthorizer>,\n",
            "    pub activation: Arc<dyn ModuleActivationPort>,\n"
            "    pub capability_authorizer: Arc<dyn CapabilityAuthorizer>,\n"
            "    pub query_authorizer: Arc<dyn QueryAuthorizer>,\n"
            "    pub visibility_authorizer: Arc<dyn QueryVisibilityAuthorizer>,\n",
            1,
        )
        content = content.replace(
            "        activation,\n        visibility_authorizer,\n        cursor_key,\n",
            "        activation,\n        capability_authorizer,\n        query_authorizer,\n        visibility_authorizer,\n        cursor_key,\n",
            1,
        )
        content = content.replace(
            "        PartyRelationshipsProductionDependencies {\n            store,\n            parties,\n            activation,\n            visibility_authorizer,\n            cursor_key,\n        },\n    )?);\n\n    Ok(contributions)\n",
            "        PartyRelationshipsProductionDependencies {\n"
            "            store: store.clone(),\n"
            "            parties: parties.clone(),\n"
            "            activation: activation.clone(),\n"
            "            visibility_authorizer: visibility_authorizer.clone(),\n"
            "            cursor_key,\n"
            "        },\n"
            "    )?);\n"
            "    contributions.merge(build_identity_resolution_contribution(\n"
            "        IdentityResolutionProductionDependencies {\n"
            "            store: store.clone(),\n"
            "            activation: activation.clone(),\n"
            "            visibility_authorizer: visibility_authorizer.clone(),\n"
            "            cursor_key,\n"
            "        },\n"
            "    )?);\n"
            "    contributions.merge(build_customer_data_operations_contribution(\n"
            "        CustomerDataOperationsProductionDependencies {\n"
            "            store: store.clone(),\n"
            "            activation: activation.clone(),\n"
            "            capability_authorizer: capability_authorizer.clone(),\n"
            "            visibility_authorizer: visibility_authorizer.clone(),\n"
            "            cursor_key,\n"
            "        },\n"
            "    )?);\n"
            "    contributions.merge(build_data_quality_contribution(\n"
            "        DataQualityProductionDependencies {\n"
            "            store,\n"
            "            activation,\n"
            "            capability_authorizer,\n"
            "            query_authorizer,\n"
            "            visibility_authorizer,\n"
            "        },\n"
            "    )?);\n\n"
            "    Ok(contributions)\n",
            1,
        )
        first_party.write_text(content, encoding="utf-8")

    app_lib = root / "crates/crm-application-runtime/src/lib.rs"
    content = app_lib.read_text(encoding="utf-8")
    content = content.replace("mod data_quality_capability_execution;\n", "")
    content = content.replace("mod data_quality_registration;\n", "")
    content = content.replace(
        "pub use data_quality_capability_execution::DataQualityCapabilityExecutor;\n"
        "pub use data_quality_registration::*;\n",
        "pub use crm_data_quality_source_composition::{\n"
        "    DataQualityAggregatePlanner, DataQualityCapabilityExecutor,\n"
        "};\n",
    )
    app_lib.write_text(content, encoding="utf-8")

    native = root / "crates/crm-application-runtime/src/native_composition.rs"
    content = native.read_text(encoding="utf-8")
    if "IdentityResolutionCapabilityPlanner" in content:
        content = content.replace(
            "use crate::{DataQualityAggregatePlanner, DataQualityCapabilityExecutor};\n",
            "",
        )
        content = content.replace(
            "    PostgresDataStore, PostgresImmutableFileArtifactStore, PostgresMetadataCapabilityExecutor,\n",
            "    PostgresDataStore, PostgresMetadataCapabilityExecutor,\n",
        )
        start = content.index("use crm_customer_data_operations_capability_adapter::{")
        end = content.index("use crm_customer_enrichment_capability_adapter::{")
        content = content[:start] + content[end:]
        content = content.replace(
            "use crm_data_quality_capability_adapter::capability_definitions as data_quality_capability_definitions;\n"
            "use crm_data_quality_query_adapter::{\n"
            "    DataQualityQueryAdapter,\n"
            "    query_capability_definitions as data_quality_query_capability_definitions,\n"
            "};\n",
            "",
        )
        start = content.index("use crm_identity_resolution_capability_adapter::{")
        end = content.index("use crm_metadata_api_adapter::{")
        content = content[:start] + content[end:]
        content = content.replace(
            "use crm_parties_capability_adapter::PartyCapabilityPlanner;\n",
            "",
        )
        old_first_party = '''use crm_first_party_modules::{
    FirstPartyProductionDependencies, build_all as build_first_party_modules,
    consents_mutation_capability_definitions as consent_capability_definitions,
    consents_query_capability_definitions as consent_query_capability_definitions,
    contact_points_mutation_capability_definitions as contact_point_capability_definitions,
    contact_points_query_capability_definitions as contact_point_query_capability_definitions,
    customer_accounts_mutation_capability_definitions as account_capability_definitions,
    customer_accounts_query_capability_definitions as account_query_capability_definitions,
    parties_mutation_capability_definitions as party_capability_definitions,
    parties_query_capability_definitions as party_query_capability_definitions,
    party_relationships_mutation_capability_definitions as party_relationship_capability_definitions,
    party_relationships_query_capability_definitions as party_relationship_query_capability_definitions,
};
'''
        new_first_party = '''use crm_first_party_modules::{
    FirstPartyProductionDependencies, build_all as build_first_party_modules,
    consents_mutation_capability_definitions as consent_capability_definitions,
    consents_query_capability_definitions as consent_query_capability_definitions,
    contact_points_mutation_capability_definitions as contact_point_capability_definitions,
    contact_points_query_capability_definitions as contact_point_query_capability_definitions,
    customer_accounts_mutation_capability_definitions as account_capability_definitions,
    customer_accounts_query_capability_definitions as account_query_capability_definitions,
    customer_data_operations_mutation_capability_definitions as customer_data_operations_capability_definitions,
    customer_data_operations_query_capability_definitions as customer_data_operations_query_capability_definitions,
    data_quality_mutation_capability_definitions as data_quality_capability_definitions,
    data_quality_query_capability_definitions as data_quality_query_capability_definitions,
    identity_resolution_mutation_capability_definitions as identity_resolution_capability_definitions,
    identity_resolution_query_capability_definitions as identity_resolution_query_capability_definitions,
    parties_mutation_capability_definitions as party_capability_definitions,
    parties_query_capability_definitions as party_query_capability_definitions,
    party_relationships_mutation_capability_definitions as party_relationship_capability_definitions,
    party_relationships_query_capability_definitions as party_relationship_query_capability_definitions,
};
'''
        if old_first_party not in content:
            raise RuntimeError("native composition first-party import anchor missing")
        content = content.replace(old_first_party, new_first_party, 1)
        content = content.replace(
            "    definitions.extend(\n"
            "        customer_data_operations_capability_definitions()?\n"
            "            .into_iter()\n"
            "            .filter(|definition| {\n"
            "                !matches!(\n"
            "                    definition.capability_id.as_str(),\n"
            "                    CREATE_PARTY_IMPORT_JOB_CAPABILITY | VALIDATE_PARTY_IMPORT_ROWS_CAPABILITY\n"
            "                )\n"
            "            }),\n"
            "    );\n"
            "    definitions.extend(customer_data_operations_source_capability_definitions()?);\n",
            "    definitions.extend(customer_data_operations_capability_definitions()?);\n",
            1,
        )
        content = content.replace(
            "    definitions.extend(identity_resolution_query_capability_definitions()?);\n"
            "    definitions.extend(identity_resolution_merge_query_capability_definitions()?);\n",
            "    definitions.extend(identity_resolution_query_capability_definitions()?);\n",
            1,
        )
        content = content.replace(
            "    let party_executor = aggregate_executor(store.clone(), PartyCapabilityPlanner);\n\n",
            "",
            1,
        )
        content = content.replace(
            "            activation: activation.clone(),\n"
            "            visibility_authorizer: visibility_authorizer.clone(),\n"
            "            cursor_key,\n",
            "            activation: activation.clone(),\n"
            "            capability_authorizer: capability_authorizer.clone(),\n"
            "            query_authorizer: query_authorizer.clone(),\n"
            "            visibility_authorizer: visibility_authorizer.clone(),\n"
            "            cursor_key,\n",
            1,
        )
        start = content.index("    let identity_aggregate = aggregate_executor(")
        end = content.index("    let customer_enrichment_fallback:")
        content = content[:start] + content[end:]
        start = content.index("    let identity_queries = Arc::new(")
        end = content.index("    let customer_enrichment_queries = Arc::new(")
        content = content[:start] + content[end:]
        start = content.index("fn select_definitions(")
        end = content.index("fn cursor(", start)
        content = content[:start] + content[end:]
        native.write_text(content, encoding="utf-8")

    guard = root / "scripts/check_native_module_composition.py"
    content = guard.read_text(encoding="utf-8")
    if "Identity Resolution mutation construction returned" not in content:
        marker_block = '''    LegacyMarker(
        "crates/crm-application-runtime/src/native_composition.rs",
        "IdentityResolutionCapabilityPlanner",
        "Identity Resolution mutation construction returned to the generic process host",
    ),
    LegacyMarker(
        "crates/crm-application-runtime/src/native_composition.rs",
        "IdentityResolutionQueryAdapter::new",
        "Identity Resolution query construction returned to the generic process host",
    ),
    LegacyMarker(
        "crates/crm-application-runtime/src/native_composition.rs",
        "CustomerDataOperationsCapabilityPlanner",
        "Customer Data Operations mutation construction returned to the generic process host",
    ),
    LegacyMarker(
        "crates/crm-application-runtime/src/native_composition.rs",
        "CustomerDataOperationsQueryAdapter::new",
        "Customer Data Operations query construction returned to the generic process host",
    ),
    LegacyMarker(
        "crates/crm-application-runtime/src/native_composition.rs",
        "DataQualityCapabilityExecutor::new",
        "Data Quality mutation construction returned to the generic process host",
    ),
    LegacyMarker(
        "crates/crm-application-runtime/src/native_composition.rs",
        "DataQualityQueryAdapter::new",
        "Data Quality query construction returned to the generic process host",
    ),
    LegacyMarker(
        "crates/crm-application-runtime/src/native_composition.rs",
        "IdentityResolutionProductionDependencies",
        "Identity Resolution owner contribution bypassed the first-party aggregate",
    ),
    LegacyMarker(
        "crates/crm-application-runtime/src/native_composition.rs",
        "CustomerDataOperationsProductionDependencies",
        "Customer Data Operations owner contribution bypassed the first-party aggregate",
    ),
    LegacyMarker(
        "crates/crm-application-runtime/src/native_composition.rs",
        "DataQualityProductionDependencies",
        "Data Quality owner contribution bypassed the first-party aggregate",
    ),
'''
        content = content.replace(
            ")\n\n\ndef find_legacy_composition_violations",
            marker_block + ")\n\n\ndef find_legacy_composition_violations",
            1,
        )
        guard.write_text(content, encoding="utf-8")

    tests = root / "tests/test_native_module_composition.py"
    content = tests.read_text(encoding="utf-8")
    if "test_identity_data_operations_and_quality_batch_is_owner_aggregated" not in content:
        test_block = '''    def test_identity_data_operations_and_quality_batch_is_owner_aggregated(self) -> None:
        identity = (
            ROOT
            / "crates/crm-identity-resolution-capability-composition/src/production_contribution.rs"
        ).read_text(encoding="utf-8")
        data_operations = (
            ROOT
            / "crates/crm-customer-data-operations-execution-composition/src/production_contribution.rs"
        ).read_text(encoding="utf-8")
        data_quality = (
            ROOT
            / "crates/crm-data-quality-source-composition/src/production_contribution.rs"
        ).read_text(encoding="utf-8")
        aggregate = (ROOT / "crates/crm-first-party-modules/src/lib.rs").read_text(
            encoding="utf-8"
        )
        runtime = (
            ROOT / "crates/crm-application-runtime/src/native_composition.rs"
        ).read_text(encoding="utf-8")

        for owner in (identity, data_operations, data_quality):
            self.assertIn("pub fn mutation_capability_definitions()", owner)
            self.assertIn("pub fn query_capability_definitions()", owner)
            self.assertIn("pub fn build_contribution(", owner)
            self.assertIn("ActivationGatedMutationValidator::new", owner)
            self.assertIn("ActivationGatedQueryValidator::new", owner)

        order = [
            "build_party_relationships_contribution(",
            "build_identity_resolution_contribution(",
            "build_customer_data_operations_contribution(",
            "build_data_quality_contribution(",
        ]
        positions = [aggregate.index(marker) for marker in order]
        self.assertEqual(positions, sorted(positions))

        for marker in (
            "IdentityResolutionCapabilityPlanner",
            "IdentityResolutionQueryAdapter::new",
            "CustomerDataOperationsCapabilityPlanner",
            "CustomerDataOperationsQueryAdapter::new",
            "DataQualityCapabilityExecutor::new",
            "DataQualityQueryAdapter::new",
        ):
            self.assertNotIn(marker, runtime)

        self.assertFalse(
            (ROOT / "crates/crm-application-runtime/src/data_quality_capability_execution.rs").exists()
        )
        self.assertFalse(
            (ROOT / "crates/crm-application-runtime/src/data_quality_registration.rs").exists()
        )
        self.assertTrue(
            (ROOT / "crates/crm-data-quality-source-composition/src/capability_execution.rs").exists()
        )
        self.assertTrue(
            (ROOT / "crates/crm-data-quality-source-composition/src/registration.rs").exists()
        )

'''
        content = content.replace(
            "\n\nif __name__ == \"__main__\":",
            "\n\n" + test_block + "if __name__ == \"__main__\":",
            1,
        )
        tests.write_text(content, encoding="utf-8")


def synchronize_and_commit(root: Path) -> None:
    subprocess.run(["cargo", "metadata", "--format-version", "1", "--no-deps"], cwd=root, check=True, stdout=subprocess.DEVNULL)
    subprocess.run(["cargo", "fmt", "--all"], cwd=root, check=True)
    write_generated_documents(root)
    if os.environ.get("GITHUB_ACTIONS") != "true":
        return
    status = subprocess.run(
        ["git", "status", "--porcelain"], cwd=root, check=True, capture_output=True, text=True
    ).stdout
    if not status.strip():
        return
    branch = os.environ.get("GITHUB_HEAD_REF")
    if not branch:
        raise RuntimeError("GITHUB_HEAD_REF is unavailable")
    subprocess.run(["git", "config", "user.name", "github-actions[bot]"], cwd=root, check=True)
    subprocess.run(
        ["git", "config", "user.email", "41898282+github-actions[bot]@users.noreply.github.com"],
        cwd=root,
        check=True,
    )
    subprocess.run(["git", "add", "Cargo.lock", "crates", "docs/ACTIVE_PACKET.md", "docs/generated/REPOSITORY_MAP.md", "scripts/check_native_module_composition.py", "tests/test_native_module_composition.py"], cwd=root, check=True)
    subprocess.run(["git", "commit", "-m", "Materialize repository step 12 contribution aggregation batch 2"], cwd=root, check=True)
    subprocess.run(["git", "push", "origin", f"HEAD:{branch}"], cwd=root, check=True)


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    root = args.root.resolve()
    try:
        if args.write:
            materialize(root)
            synchronize_and_commit(root)
            print("Repository step 12 batch 2 is materialized and navigation is synchronized.")
            return 0
        stale = stale_generated_documents(root)
    except (NavigationError, OSError, RuntimeError, subprocess.CalledProcessError) as error:
        print(f"ERROR: {error}", file=sys.stderr)
        return 1
    if stale:
        for path in stale:
            print(f"STALE {path}", file=sys.stderr)
        print(
            "ERROR: run python scripts/generate_repository_navigation.py --write",
            file=sys.stderr,
        )
        return 1
    print("Repository navigation is synchronized.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
