#!/usr/bin/env python3
"""One-run materializer for repository step 12 contribution aggregation batch 1."""

from __future__ import annotations

import argparse
import os
from pathlib import Path
import subprocess
import sys

from repository_navigation import NavigationError, stale_generated_documents, write_generated_documents


SENTINEL = "pub struct PartiesProductionDependencies"


def parser() -> argparse.ArgumentParser:
    value = argparse.ArgumentParser()
    value.add_argument("--root", type=Path, default=Path.cwd())
    mode = value.add_mutually_exclusive_group(required=True)
    mode.add_argument("--write", action="store_true")
    mode.add_argument("--check", action="store_true")
    return value


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise NavigationError(f"step-12 batch-1 expected one {label}, found {count}")
    return text.replace(old, new, 1)


def replace_method(text: str, name: str, next_name: str, body: str) -> str:
    start = f"    def {name}"
    end = f"    def {next_name}"
    start_index = text.find(start)
    end_index = text.find(end, start_index + 1)
    if start_index < 0 or end_index < 0:
        raise NavigationError(f"step-12 batch-1 could not find method {name}")
    return text[:start_index] + body.rstrip() + "\n\n" + text[end_index:]


def write(path: Path, content: str) -> None:
    path.write_text(content, encoding="utf-8")


def materialize(root: Path) -> bool:
    parties_path = root / "crates/crm-party-reference-composition/src/lib.rs"
    if SENTINEL in parties_path.read_text(encoding="utf-8"):
        return False

    # Internal path-edge changes only; no external dependency family/version changes.
    cargo_updates = {
        "crates/crm-party-reference-composition/Cargo.toml": (
            "[dependencies]\ncrm-core-data = { path = \"../crm-core-data\" }\ncrm-module-sdk = { path = \"../crm-module-sdk\" }\ncrm-parties-capability-adapter = { path = \"../crm-parties-capability-adapter\" }\nsqlx = { version = \"0.9\", default-features = false, features = [\"postgres\", \"runtime-tokio\"] }\n",
            "[dependencies]\ncrm-application-composition = { path = \"../crm-application-composition\" }\ncrm-capability-runtime = { path = \"../crm-capability-runtime\" }\ncrm-core-data = { path = \"../crm-core-data\" }\ncrm-module-sdk = { path = \"../crm-module-sdk\" }\ncrm-parties-capability-adapter = { path = \"../crm-parties-capability-adapter\" }\ncrm-parties-query-adapter = { path = \"../crm-parties-query-adapter\" }\ncrm-query-runtime = { path = \"../crm-query-runtime\" }\nsqlx = { version = \"0.9\", default-features = false, features = [\"postgres\", \"runtime-tokio\"] }\n",
        ),
        "crates/crm-contact-points-capability-composition/Cargo.toml": (
            "[dependencies]\ncrm-capability-runtime = { path = \"../crm-capability-runtime\" }\ncrm-contact-points-capability-adapter = { path = \"../crm-contact-points-capability-adapter\" }\ncrm-core-data = { path = \"../crm-core-data\" }\ncrm-identity-resolution-topology-composition = { path = \"../crm-identity-resolution-topology-composition\" }\ncrm-module-sdk = { path = \"../crm-module-sdk\" }\ncrm-party-reference-composition = { path = \"../crm-party-reference-composition\" }\n",
            "[dependencies]\ncrm-application-composition = { path = \"../crm-application-composition\" }\ncrm-capability-runtime = { path = \"../crm-capability-runtime\" }\ncrm-contact-points-capability-adapter = { path = \"../crm-contact-points-capability-adapter\" }\ncrm-contact-points-query-adapter = { path = \"../crm-contact-points-query-adapter\" }\ncrm-core-data = { path = \"../crm-core-data\" }\ncrm-identity-resolution-topology-composition = { path = \"../crm-identity-resolution-topology-composition\" }\ncrm-module-sdk = { path = \"../crm-module-sdk\" }\ncrm-party-reference-composition = { path = \"../crm-party-reference-composition\" }\ncrm-query-runtime = { path = \"../crm-query-runtime\" }\n",
        ),
        "crates/crm-party-relationships-capability-composition/Cargo.toml": (
            "[dependencies]\ncrm-capability-runtime = { path = \"../crm-capability-runtime\" }\ncrm-module-sdk = { path = \"../crm-module-sdk\" }\ncrm-party-reference-composition = { path = \"../crm-party-reference-composition\" }\ncrm-party-relationships-capability-adapter = { path = \"../crm-party-relationships-capability-adapter\" }\n",
            "[dependencies]\ncrm-application-composition = { path = \"../crm-application-composition\" }\ncrm-capability-runtime = { path = \"../crm-capability-runtime\" }\ncrm-core-data = { path = \"../crm-core-data\" }\ncrm-module-sdk = { path = \"../crm-module-sdk\" }\ncrm-party-reference-composition = { path = \"../crm-party-reference-composition\" }\ncrm-party-relationships-capability-adapter = { path = \"../crm-party-relationships-capability-adapter\" }\ncrm-party-relationships-query-adapter = { path = \"../crm-party-relationships-query-adapter\" }\ncrm-query-runtime = { path = \"../crm-query-runtime\" }\n",
        ),
        "crates/crm-first-party-modules/Cargo.toml": (
            "crm-consents-capability-composition = { path = \"../crm-consents-capability-composition\" }\ncrm-module-sdk = { path = \"../crm-module-sdk\" }\ncrm-party-reference-composition = { path = \"../crm-party-reference-composition\" }\ncrm-query-runtime = { path = \"../crm-query-runtime\" }\n",
            "crm-consents-capability-composition = { path = \"../crm-consents-capability-composition\" }\ncrm-contact-points-capability-composition = { path = \"../crm-contact-points-capability-composition\" }\ncrm-module-sdk = { path = \"../crm-module-sdk\" }\ncrm-party-reference-composition = { path = \"../crm-party-reference-composition\" }\ncrm-party-relationships-capability-composition = { path = \"../crm-party-relationships-capability-composition\" }\ncrm-query-runtime = { path = \"../crm-query-runtime\" }\n",
        ),
    }
    for relative, (old, new) in cargo_updates.items():
        path = root / relative
        write(path, replace_once(path.read_text(encoding="utf-8"), old, new, relative))

    # Parties owner contribution lives in the existing Party composition boundary.
    parties = parties_path.read_text(encoding="utf-8")
    parties = replace_once(
        parties,
        "use crm_core_data::{PostgresDataStore, RecordGetQuery};\nuse crm_module_sdk::{\n    ErrorCategory, ModuleId, PortFuture, RecordId, RecordType, SdkError, TenantId,\n};\nuse crm_parties_capability_adapter::{MODULE_ID as PARTIES_MODULE_ID, RECORD_TYPE};\nuse sqlx::{Postgres, Row, Transaction};\n",
        "use crm_application_composition::{\n    ActivationGatedMutationValidator, ActivationGatedQueryValidator, ModuleActivationPort,\n    ModuleContributionSet, NoopMutationSemanticValidator,\n};\nuse crm_capability_runtime::{\n    CapabilityDefinition, CapabilitySemanticValidator, TransactionalCapabilityExecutor,\n};\nuse crm_core_data::{\n    PostgresDataStore, PostgresTransactionalAggregateExecutor, RecordGetQuery,\n};\nuse crm_module_sdk::{\n    ErrorCategory, ModuleId, PortFuture, RecordId, RecordType, SdkError, TenantId,\n};\nuse crm_parties_capability_adapter::{\n    MODULE_ID as PARTIES_MODULE_ID, PartyCapabilityPlanner, RECORD_TYPE,\n    capability_definitions as adapter_mutation_capability_definitions,\n};\nuse crm_parties_query_adapter::{\n    PartyQueryAdapter, query_capability_definitions as adapter_query_capability_definitions,\n};\nuse crm_query_runtime::{\n    CursorCodec, QueryExecutor, QuerySemanticValidator, QueryVisibilityAuthorizer,\n};\nuse sqlx::{Postgres, Row, Transaction};\nuse std::fmt;\nuse std::sync::Arc;\n",
        "Parties imports",
    )
    parties_builder = '''#[derive(Clone)]
pub struct PartiesProductionDependencies {
    pub store: PostgresDataStore,
    pub activation: Arc<dyn ModuleActivationPort>,
    pub visibility_authorizer: Arc<dyn QueryVisibilityAuthorizer>,
    pub cursor_key: [u8; 32],
}

/// Returns the exact Parties mutation inventory owned by this production
/// composition package.
pub fn mutation_capability_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {
    adapter_mutation_capability_definitions()
}

/// Returns the exact Parties query inventory owned by this production
/// composition package.
pub fn query_capability_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {
    adapter_query_capability_definitions()
}

/// Builds the complete Parties mutation/query contribution inside an existing
/// owner composition package rather than the generic process host.
pub fn build_contribution(
    dependencies: PartiesProductionDependencies,
) -> Result<ModuleContributionSet, SdkError> {
    let PartiesProductionDependencies {
        store,
        activation,
        visibility_authorizer,
        cursor_key,
    } = dependencies;
    let mut contributions = ModuleContributionSet::new();

    let mutation_executor: Arc<dyn TransactionalCapabilityExecutor> =
        Arc::new(PostgresTransactionalAggregateExecutor::new(
            store.clone(),
            Arc::new(PartyCapabilityPlanner),
        ));
    let mutation_validator: Arc<dyn CapabilitySemanticValidator> =
        Arc::new(ActivationGatedMutationValidator::new(
            activation.clone(),
            Arc::new(NoopMutationSemanticValidator),
        ));
    contributions
        .add_mutations(
            mutation_capability_definitions()?,
            mutation_validator,
            mutation_executor,
        )
        .map_err(production_composition_error)?;

    let query_adapter = Arc::new(PartyQueryAdapter::new(
        store,
        parties_cursor(cursor_key)?,
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

fn parties_cursor(key: [u8; 32]) -> Result<CursorCodec, SdkError> {
    CursorCodec::new(key).map_err(|error| {
        SdkError::new(
            "PARTIES_CURSOR_CONFIGURATION_INVALID",
            ErrorCategory::Internal,
            false,
            "The Parties cursor configuration is invalid.",
        )
        .with_internal_reference(error.to_string())
    })
}

fn production_composition_error(error: impl fmt::Display) -> SdkError {
    SdkError::new(
        "PARTIES_PRODUCTION_COMPOSITION_INVALID",
        ErrorCategory::Internal,
        false,
        "The Parties production contribution is invalid.",
    )
    .with_internal_reference(error.to_string())
}

'''
    parties = replace_once(
        parties,
        "/// Locks and proves one authoritative Party row inside the caller's PostgreSQL\n",
        parties_builder + "/// Locks and proves one authoritative Party row inside the caller's PostgreSQL\n",
        "Parties production builder insertion",
    )
    write(parties_path, parties)

    # Consents already owns its builder; expose exact inventory factories through it.
    consents_path = root / "crates/crm-consents-capability-composition/src/lib.rs"
    consents = consents_path.read_text(encoding="utf-8")
    consents = replace_once(
        consents,
        "    MUTATION_CAPABILITY_IDS, capability_definitions, referenced_scope_from_create,\n};\nuse crm_consents_query_adapter::{ConsentQueryAdapter, query_capability_definitions};\n",
        "    MUTATION_CAPABILITY_IDS,\n    capability_definitions as adapter_mutation_capability_definitions,\n    referenced_scope_from_create,\n};\nuse crm_consents_query_adapter::{\n    ConsentQueryAdapter, query_capability_definitions as adapter_query_capability_definitions,\n};\n",
        "Consents definition aliases",
    )
    consents = replace_once(
        consents,
        "/// Builds the complete Consents mutation/query contribution inside the owner\n",
        "/// Returns the exact Consents mutation inventory owned by this production\n/// composition package.\npub fn mutation_capability_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {\n    adapter_mutation_capability_definitions()\n}\n\n/// Returns the exact Consents query inventory owned by this production\n/// composition package.\npub fn query_capability_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {\n    adapter_query_capability_definitions()\n}\n\n/// Builds the complete Consents mutation/query contribution inside the owner\n",
        "Consents inventory factory insertion",
    )
    consents = replace_once(
        consents,
        "            capability_definitions()?,\n",
        "            mutation_capability_definitions()?,\n",
        "Consents mutation builder inventory",
    )
    consents = replace_once(
        consents,
        "            query_capability_definitions()?,\n",
        "            query_capability_definitions()?,\n",
        "Consents query builder inventory",
    )
    write(consents_path, consents)

    # Contact Points full owner production contribution.
    contact_path = root / "crates/crm-contact-points-capability-composition/src/lib.rs"
    contact = contact_path.read_text(encoding="utf-8")
    contact = replace_once(
        contact,
        "use crm_capability_runtime::{\n    CapabilityDefinition, CapabilityRequest, CapabilitySemanticValidator,\n};\nuse crm_contact_points_capability_adapter::{\n    CREATE_CAPABILITY, MUTATION_CAPABILITY_IDS, referenced_party_id_from_create,\n};\nuse crm_module_sdk::{ErrorCategory, PortFuture, SdkError};\nuse crm_party_reference_composition::PartyReferenceReader;\nuse std::fmt;\nuse std::sync::Arc;\n",
        "use crm_application_composition::{\n    ActivationGatedMutationValidator, ActivationGatedQueryValidator, ModuleActivationPort,\n    ModuleContributionSet,\n};\nuse crm_capability_runtime::{\n    CapabilityDefinition, CapabilityRequest, CapabilitySemanticValidator,\n    TransactionalCapabilityExecutor,\n};\nuse crm_contact_points_capability_adapter::{\n    CREATE_CAPABILITY, ContactPointCapabilityPlanner, MUTATION_CAPABILITY_IDS,\n    capability_definitions as adapter_mutation_capability_definitions,\n    referenced_party_id_from_create,\n};\nuse crm_contact_points_query_adapter::{\n    ContactPointQueryAdapter, query_capability_definitions as adapter_query_capability_definitions,\n};\nuse crm_core_data::{PostgresDataStore, PostgresTransactionalAggregateExecutor};\nuse crm_module_sdk::{ErrorCategory, PortFuture, SdkError};\nuse crm_party_reference_composition::PartyReferenceReader;\nuse crm_query_runtime::{\n    CursorCodec, QueryExecutor, QuerySemanticValidator, QueryVisibilityAuthorizer,\n};\nuse std::fmt;\nuse std::sync::Arc;\n",
        "Contact Points imports",
    )
    contact_builder = '''#[derive(Clone)]
pub struct ContactPointsProductionDependencies {
    pub store: PostgresDataStore,
    pub parties: Arc<dyn PartyReferenceReader>,
    pub activation: Arc<dyn ModuleActivationPort>,
    pub visibility_authorizer: Arc<dyn QueryVisibilityAuthorizer>,
    pub cursor_key: [u8; 32],
}

/// Returns the exact Contact Points mutation inventory owned by this production
/// composition package.
pub fn mutation_capability_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {
    adapter_mutation_capability_definitions()
}

/// Returns the exact Contact Points query inventory owned by this production
/// composition package.
pub fn query_capability_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {
    adapter_query_capability_definitions()
}

/// Builds the complete Contact Points mutation/query contribution inside the
/// owner composition package while preserving Party reference validation.
pub fn build_contribution(
    dependencies: ContactPointsProductionDependencies,
) -> Result<ModuleContributionSet, SdkError> {
    let ContactPointsProductionDependencies {
        store,
        parties,
        activation,
        visibility_authorizer,
        cursor_key,
    } = dependencies;
    let mut contributions = ModuleContributionSet::new();

    let mutation_executor: Arc<dyn TransactionalCapabilityExecutor> =
        Arc::new(PostgresTransactionalAggregateExecutor::new(
            store.clone(),
            Arc::new(ContactPointCapabilityPlanner),
        ));
    let mutation_validator: Arc<dyn CapabilitySemanticValidator> =
        Arc::new(ActivationGatedMutationValidator::new(
            activation.clone(),
            Arc::new(ContactPointPartyReferenceSemanticValidator::new(parties)),
        ));
    contributions
        .add_mutations(
            mutation_capability_definitions()?,
            mutation_validator,
            mutation_executor,
        )
        .map_err(production_composition_error)?;

    let query_adapter = Arc::new(ContactPointQueryAdapter::new(
        store,
        contact_points_cursor(cursor_key)?,
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

fn contact_points_cursor(key: [u8; 32]) -> Result<CursorCodec, SdkError> {
    CursorCodec::new(key).map_err(|error| {
        SdkError::new(
            "CONTACT_POINTS_CURSOR_CONFIGURATION_INVALID",
            ErrorCategory::Internal,
            false,
            "The Contact Points cursor configuration is invalid.",
        )
        .with_internal_reference(error.to_string())
    })
}

fn production_composition_error(error: impl fmt::Display) -> SdkError {
    SdkError::new(
        "CONTACT_POINTS_PRODUCTION_COMPOSITION_INVALID",
        ErrorCategory::Internal,
        false,
        "The Contact Points production contribution is invalid.",
    )
    .with_internal_reference(error.to_string())
}

'''
    contact = replace_once(
        contact,
        "fn reference_unavailable() -> SdkError {\n",
        contact_builder + "fn reference_unavailable() -> SdkError {\n",
        "Contact Points production builder insertion",
    )
    write(contact_path, contact)

    # Party Relationships full owner production contribution.
    relationships_path = root / "crates/crm-party-relationships-capability-composition/src/lib.rs"
    relationships = relationships_path.read_text(encoding="utf-8")
    relationships = replace_once(
        relationships,
        "use crm_capability_runtime::{\n    CapabilityDefinition, CapabilityRequest, CapabilitySemanticValidator,\n};\nuse crm_module_sdk::{ErrorCategory, PortFuture, SdkError};\nuse crm_party_reference_composition::PartyReferenceReader;\nuse crm_party_relationships_capability_adapter::{\n    CREATE_CAPABILITY, MUTATION_CAPABILITY_IDS, referenced_party_ids_from_create,\n};\nuse std::collections::BTreeSet;\nuse std::fmt;\nuse std::sync::Arc;\n",
        "use crm_application_composition::{\n    ActivationGatedMutationValidator, ActivationGatedQueryValidator, ModuleActivationPort,\n    ModuleContributionSet,\n};\nuse crm_capability_runtime::{\n    CapabilityDefinition, CapabilityRequest, CapabilitySemanticValidator,\n    TransactionalCapabilityExecutor,\n};\nuse crm_core_data::{PostgresDataStore, PostgresTransactionalAggregateExecutor};\nuse crm_module_sdk::{ErrorCategory, PortFuture, SdkError};\nuse crm_party_reference_composition::PartyReferenceReader;\nuse crm_party_relationships_capability_adapter::{\n    CREATE_CAPABILITY, MUTATION_CAPABILITY_IDS, PartyRelationshipCapabilityPlanner,\n    capability_definitions as adapter_mutation_capability_definitions,\n    referenced_party_ids_from_create,\n};\nuse crm_party_relationships_query_adapter::{\n    PartyRelationshipQueryAdapter,\n    query_capability_definitions as adapter_query_capability_definitions,\n};\nuse crm_query_runtime::{\n    CursorCodec, QueryExecutor, QuerySemanticValidator, QueryVisibilityAuthorizer,\n};\nuse std::collections::BTreeSet;\nuse std::fmt;\nuse std::sync::Arc;\n",
        "Party Relationships imports",
    )
    relationships_builder = '''#[derive(Clone)]
pub struct PartyRelationshipsProductionDependencies {
    pub store: PostgresDataStore,
    pub parties: Arc<dyn PartyReferenceReader>,
    pub activation: Arc<dyn ModuleActivationPort>,
    pub visibility_authorizer: Arc<dyn QueryVisibilityAuthorizer>,
    pub cursor_key: [u8; 32],
}

/// Returns the exact Party Relationships mutation inventory owned by this
/// production composition package.
pub fn mutation_capability_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {
    adapter_mutation_capability_definitions()
}

/// Returns the exact Party Relationships query inventory owned by this
/// production composition package.
pub fn query_capability_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {
    adapter_query_capability_definitions()
}

/// Builds the complete Party Relationships mutation/query contribution inside
/// the owner package while preserving Party reference validation.
pub fn build_contribution(
    dependencies: PartyRelationshipsProductionDependencies,
) -> Result<ModuleContributionSet, SdkError> {
    let PartyRelationshipsProductionDependencies {
        store,
        parties,
        activation,
        visibility_authorizer,
        cursor_key,
    } = dependencies;
    let mut contributions = ModuleContributionSet::new();

    let mutation_executor: Arc<dyn TransactionalCapabilityExecutor> =
        Arc::new(PostgresTransactionalAggregateExecutor::new(
            store.clone(),
            Arc::new(PartyRelationshipCapabilityPlanner),
        ));
    let mutation_validator: Arc<dyn CapabilitySemanticValidator> =
        Arc::new(ActivationGatedMutationValidator::new(
            activation.clone(),
            Arc::new(PartyRelationshipReferenceSemanticValidator::new(parties)),
        ));
    contributions
        .add_mutations(
            mutation_capability_definitions()?,
            mutation_validator,
            mutation_executor,
        )
        .map_err(production_composition_error)?;

    let query_adapter = Arc::new(PartyRelationshipQueryAdapter::new(
        store,
        party_relationships_cursor(cursor_key)?,
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

fn party_relationships_cursor(key: [u8; 32]) -> Result<CursorCodec, SdkError> {
    CursorCodec::new(key).map_err(|error| {
        SdkError::new(
            "PARTY_RELATIONSHIPS_CURSOR_CONFIGURATION_INVALID",
            ErrorCategory::Internal,
            false,
            "The Party Relationships cursor configuration is invalid.",
        )
        .with_internal_reference(error.to_string())
    })
}

fn production_composition_error(error: impl fmt::Display) -> SdkError {
    SdkError::new(
        "PARTY_RELATIONSHIPS_PRODUCTION_COMPOSITION_INVALID",
        ErrorCategory::Internal,
        false,
        "The Party Relationships production contribution is invalid.",
    )
    .with_internal_reference(error.to_string())
}

'''
    relationships = replace_once(
        relationships,
        "fn reference_unavailable() -> SdkError {\n",
        relationships_builder + "fn reference_unavailable() -> SdkError {\n",
        "Party Relationships production builder insertion",
    )
    write(relationships_path, relationships)

    # The aggregate owns shared Party reference construction and exact accepted order.
    first_party_path = root / "crates/crm-first-party-modules/src/lib.rs"
    write(
        first_party_path,
        '''#![forbid(unsafe_code)]

//! Mechanically narrow aggregation of proven first-party owner contributions.
//!
//! This crate contains no route catalog and no business dispatch. Exact routes
//! remain defined and built by owner packages; this boundary only combines
//! their contribution entry points for the generic process host.

use crm_application_composition::{ModuleActivationPort, ModuleContributionSet};
use crm_consents_capability_composition::{
    ConsentsProductionDependencies, build_contribution as build_consents_contribution,
};
pub use crm_consents_capability_composition::{
    mutation_capability_definitions as consents_mutation_capability_definitions,
    query_capability_definitions as consents_query_capability_definitions,
};
use crm_contact_points_capability_composition::{
    ContactPointsProductionDependencies, build_contribution as build_contact_points_contribution,
};
pub use crm_contact_points_capability_composition::{
    mutation_capability_definitions as contact_points_mutation_capability_definitions,
    query_capability_definitions as contact_points_query_capability_definitions,
};
use crm_core_data::PostgresDataStore;
use crm_customer_accounts_capability_composition::{
    CustomerAccountsProductionDependencies,
    build_contribution as build_customer_accounts_contribution,
};
pub use crm_customer_accounts_capability_composition::{
    mutation_capability_definitions as customer_accounts_mutation_capability_definitions,
    query_capability_definitions as customer_accounts_query_capability_definitions,
};
use crm_module_sdk::SdkError;
use crm_party_reference_composition::{
    PartiesProductionDependencies, PostgresPartyReferenceReader,
    build_contribution as build_parties_contribution,
};
pub use crm_party_reference_composition::{
    mutation_capability_definitions as parties_mutation_capability_definitions,
    query_capability_definitions as parties_query_capability_definitions,
};
use crm_party_relationships_capability_composition::{
    PartyRelationshipsProductionDependencies,
    build_contribution as build_party_relationships_contribution,
};
pub use crm_party_relationships_capability_composition::{
    mutation_capability_definitions as party_relationships_mutation_capability_definitions,
    query_capability_definitions as party_relationships_query_capability_definitions,
};
use crm_query_runtime::QueryVisibilityAuthorizer;
use std::sync::Arc;

#[derive(Clone)]
pub struct FirstPartyProductionDependencies {
    pub store: PostgresDataStore,
    pub activation: Arc<dyn ModuleActivationPort>,
    pub visibility_authorizer: Arc<dyn QueryVisibilityAuthorizer>,
    pub cursor_key: [u8; 32],
}

/// Builds the accepted first-party contribution sequence without repeating any
/// module identifier, capability coordinate or owner-specific dispatch rule.
pub fn build_all(
    dependencies: FirstPartyProductionDependencies,
) -> Result<ModuleContributionSet, SdkError> {
    let FirstPartyProductionDependencies {
        store,
        activation,
        visibility_authorizer,
        cursor_key,
    } = dependencies;
    let parties = Arc::new(PostgresPartyReferenceReader::new(store.clone()));
    let mut contributions = ModuleContributionSet::new();

    contributions.merge(build_parties_contribution(PartiesProductionDependencies {
        store: store.clone(),
        activation: activation.clone(),
        visibility_authorizer: visibility_authorizer.clone(),
        cursor_key,
    })?)?;
    contributions.merge(build_customer_accounts_contribution(
        CustomerAccountsProductionDependencies {
            store: store.clone(),
            parties: parties.clone(),
            activation: activation.clone(),
            visibility_authorizer: visibility_authorizer.clone(),
            cursor_key,
        },
    )?)?;
    contributions.merge(build_consents_contribution(ConsentsProductionDependencies {
        store: store.clone(),
        activation: activation.clone(),
        visibility_authorizer: visibility_authorizer.clone(),
        cursor_key,
    })?)?;
    contributions.merge(build_contact_points_contribution(
        ContactPointsProductionDependencies {
            store: store.clone(),
            parties: parties.clone(),
            activation: activation.clone(),
            visibility_authorizer: visibility_authorizer.clone(),
            cursor_key,
        },
    )?)?;
    contributions.merge(build_party_relationships_contribution(
        PartyRelationshipsProductionDependencies {
            store,
            parties,
            activation,
            visibility_authorizer,
            cursor_key,
        },
    )?)?;

    Ok(contributions)
}

pub const CRATE_NAME: &str = "crm-first-party-modules";
''',
    )

    # Generic runtime keeps compatibility inventory order but delegates factories
    # and ordinary registration through the first-party aggregate.
    runtime_path = root / "crates/crm-application-runtime/src/native_composition.rs"
    runtime = runtime_path.read_text(encoding="utf-8")
    for old, label in (
        ("use crm_consents_capability_adapter::capability_definitions as consent_capability_definitions;\n", "Consents mutation import"),
        ("use crm_contact_points_capability_adapter::{\n    ContactPointCapabilityPlanner, capability_definitions as contact_point_capability_definitions,\n};\n", "Contact Points mutation import"),
        ("use crm_contact_points_capability_composition::ContactPointPartyReferenceSemanticValidator;\n", "Contact Points validator import"),
        ("use crm_contact_points_query_adapter::{\n    ContactPointQueryAdapter,\n    query_capability_definitions as contact_point_query_capability_definitions,\n};\n", "Contact Points query import"),
        ("use crm_party_reference_composition::PostgresPartyReferenceReader;\n", "Party reader import"),
        ("use crm_party_relationships_capability_adapter::{\n    PartyRelationshipCapabilityPlanner,\n    capability_definitions as party_relationship_capability_definitions,\n};\n", "Party Relationships mutation import"),
        ("use crm_party_relationships_capability_composition::PartyRelationshipReferenceSemanticValidator;\n", "Party Relationships validator import"),
        ("use crm_party_relationships_query_adapter::{\n    PartyRelationshipQueryAdapter,\n    query_capability_definitions as party_relationship_query_capability_definitions,\n};\n", "Party Relationships query import"),
    ):
        runtime = replace_once(runtime, old, "", label)
    runtime = replace_once(
        runtime,
        "use crm_consents_query_adapter::{\n    ConsentQueryAdapter, query_capability_definitions as consent_query_capability_definitions,\n};\n",
        "use crm_consents_query_adapter::ConsentQueryAdapter;\n",
        "Consents query definition import",
    )
    runtime = replace_once(
        runtime,
        "use crm_first_party_modules::{\n    FirstPartyProductionDependencies, build_all as build_first_party_modules,\n    customer_accounts_mutation_capability_definitions as account_capability_definitions,\n    customer_accounts_query_capability_definitions as account_query_capability_definitions,\n};\n",
        "use crm_first_party_modules::{\n    FirstPartyProductionDependencies, build_all as build_first_party_modules,\n    consents_mutation_capability_definitions as consent_capability_definitions,\n    consents_query_capability_definitions as consent_query_capability_definitions,\n    contact_points_mutation_capability_definitions as contact_point_capability_definitions,\n    contact_points_query_capability_definitions as contact_point_query_capability_definitions,\n    customer_accounts_mutation_capability_definitions as account_capability_definitions,\n    customer_accounts_query_capability_definitions as account_query_capability_definitions,\n    parties_mutation_capability_definitions as party_capability_definitions,\n    parties_query_capability_definitions as party_query_capability_definitions,\n    party_relationships_mutation_capability_definitions as party_relationship_capability_definitions,\n    party_relationships_query_capability_definitions as party_relationship_query_capability_definitions,\n};\n",
        "first-party inventory imports",
    )
    runtime = replace_once(
        runtime,
        "use crm_parties_capability_adapter::{\n    PartyCapabilityPlanner, capability_definitions as party_capability_definitions,\n};\nuse crm_parties_query_adapter::{\n    PartyQueryAdapter, query_capability_definitions as party_query_capability_definitions,\n};\n",
        "use crm_parties_capability_adapter::PartyCapabilityPlanner;\nuse crm_parties_query_adapter::PartyQueryAdapter;\n",
        "Parties direct definition imports",
    )
    runtime = replace_once(
        runtime,
        "    let mut contributions = ModuleContributionSet::new();\n    let parties = Arc::new(PostgresPartyReferenceReader::new(store.clone()));\n",
        "    let mut contributions = ModuleContributionSet::new();\n",
        "generic Party reader construction",
    )
    runtime = replace_once(
        runtime,
        "    let party_executor = aggregate_executor(store.clone(), PartyCapabilityPlanner);\n    add_activated_mutations(\n        &mut contributions,\n        party_capability_definitions()?,\n        Arc::new(NoopMutationSemanticValidator),\n        party_executor.clone(),\n        activation.clone(),\n    )?;\n\n",
        "    let party_executor = aggregate_executor(store.clone(), PartyCapabilityPlanner);\n\n",
        "Parties mutation registration",
    )
    runtime = replace_once(
        runtime,
        "            store: store.clone(),\n            parties: parties.clone(),\n            activation: activation.clone(),\n",
        "            store: store.clone(),\n            activation: activation.clone(),\n",
        "first-party dependency call",
    )
    contact_block = '''    let contact_point_executor = aggregate_executor(store.clone(), ContactPointCapabilityPlanner);
    add_activated_mutations(
        &mut contributions,
        contact_point_capability_definitions()?,
        Arc::new(ContactPointPartyReferenceSemanticValidator::new(
            parties.clone(),
        )),
        contact_point_executor,
        activation.clone(),
    )?;

'''
    runtime = replace_once(runtime, contact_block, "", "Contact Points mutation registration")
    relationship_block = '''    let party_relationship_executor =
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

'''
    runtime = replace_once(runtime, relationship_block, "", "Party Relationships mutation registration")
    runtime = replace_once(
        runtime,
        '''    let party_queries = Arc::new(PartyQueryAdapter::new(
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

''',
        '''    let party_queries = Arc::new(PartyQueryAdapter::new(
        store.clone(),
        cursor(cursor_key)?,
        visibility_authorizer.clone(),
    )?);

''',
        "Parties query registration",
    )
    runtime = replace_once(
        runtime,
        '''    let contact_point_queries = Arc::new(ContactPointQueryAdapter::new(
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

''',
        "",
        "Contact Points query registration",
    )
    runtime = replace_once(
        runtime,
        '''    let relationship_queries = Arc::new(PartyRelationshipQueryAdapter::new(
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

''',
        "",
        "Party Relationships query registration",
    )
    write(runtime_path, runtime)

    runtime_cargo_path = root / "crates/crm-application-runtime/Cargo.toml"
    runtime_cargo = runtime_cargo_path.read_text(encoding="utf-8")
    for dependency in (
        "crm-consents-capability-adapter = { path = \"../crm-consents-capability-adapter\" }\n",
        "crm-contact-points-capability-adapter = { path = \"../crm-contact-points-capability-adapter\" }\n",
        "crm-contact-points-capability-composition = { path = \"../crm-contact-points-capability-composition\" }\n",
        "crm-contact-points-query-adapter = { path = \"../crm-contact-points-query-adapter\" }\n",
        "crm-customer-accounts-capability-adapter = { path = \"../crm-customer-accounts-capability-adapter\" }\n",
        "crm-customer-accounts-query-adapter = { path = \"../crm-customer-accounts-query-adapter\" }\n",
        "crm-party-reference-composition = { path = \"../crm-party-reference-composition\" }\n",
        "crm-party-relationships-capability-adapter = { path = \"../crm-party-relationships-capability-adapter\" }\n",
        "crm-party-relationships-capability-composition = { path = \"../crm-party-relationships-capability-composition\" }\n",
        "crm-party-relationships-query-adapter = { path = \"../crm-party-relationships-query-adapter\" }\n",
    ):
        runtime_cargo = replace_once(runtime_cargo, dependency, "", f"runtime dependency {dependency.split(' =')[0]}")
    write(runtime_cargo_path, runtime_cargo)

    # Fail closed against renewed central registration bypasses.
    guard_path = root / "scripts/check_native_module_composition.py"
    guard = guard_path.read_text(encoding="utf-8")
    guard_insert = '''    LegacyMarker(
        "crates/crm-application-runtime/src/native_composition.rs",
        "use crm_consents_capability_adapter",
        "Consents inventory bypassed the first-party aggregate",
    ),
    LegacyMarker(
        "crates/crm-application-runtime/src/native_composition.rs",
        "ContactPointCapabilityPlanner",
        "Contact Points mutation construction returned to the generic process host",
    ),
    LegacyMarker(
        "crates/crm-application-runtime/src/native_composition.rs",
        "ContactPointPartyReferenceSemanticValidator",
        "Contact Points semantic validation returned to the generic process host",
    ),
    LegacyMarker(
        "crates/crm-application-runtime/src/native_composition.rs",
        "ContactPointQueryAdapter::new",
        "Contact Points query construction returned to the generic process host",
    ),
    LegacyMarker(
        "crates/crm-application-runtime/src/native_composition.rs",
        "PartyRelationshipCapabilityPlanner",
        "Party Relationships mutation construction returned to the generic process host",
    ),
    LegacyMarker(
        "crates/crm-application-runtime/src/native_composition.rs",
        "PartyRelationshipReferenceSemanticValidator",
        "Party Relationships semantic validation returned to the generic process host",
    ),
    LegacyMarker(
        "crates/crm-application-runtime/src/native_composition.rs",
        "PartyRelationshipQueryAdapter::new",
        "Party Relationships query construction returned to the generic process host",
    ),
    LegacyMarker(
        "crates/crm-application-runtime/src/native_composition.rs",
        "add_activated_mutations(\n        &mut contributions,\n        party_capability_definitions()?",
        "Parties mutation registration returned to the generic process host",
    ),
    LegacyMarker(
        "crates/crm-application-runtime/src/native_composition.rs",
        "add_activated_queries(\n        &mut contributions,\n        party_query_capability_definitions()?",
        "Parties query registration returned to the generic process host",
    ),
'''
    guard = replace_once(
        guard,
        ")\n\n\ndef find_legacy_composition_violations",
        guard_insert + ")\n\n\ndef find_legacy_composition_violations",
        "native composition guard insertion",
    )
    write(guard_path, guard)

    native_test_path = root / "tests/test_native_module_composition.py"
    native_tests = native_test_path.read_text(encoding="utf-8")
    native_test = '''
    def test_relationship_owner_batch_is_aggregated_without_behavior_reordering(self) -> None:
        parties = (ROOT / "crates/crm-party-reference-composition/src/lib.rs").read_text(
            encoding="utf-8"
        )
        consents = (
            ROOT / "crates/crm-consents-capability-composition/src/lib.rs"
        ).read_text(encoding="utf-8")
        contact_points = (
            ROOT / "crates/crm-contact-points-capability-composition/src/lib.rs"
        ).read_text(encoding="utf-8")
        relationships = (
            ROOT / "crates/crm-party-relationships-capability-composition/src/lib.rs"
        ).read_text(encoding="utf-8")
        aggregate = (ROOT / "crates/crm-first-party-modules/src/lib.rs").read_text(
            encoding="utf-8"
        )
        runtime = (
            ROOT / "crates/crm-application-runtime/src/native_composition.rs"
        ).read_text(encoding="utf-8")
        runtime_cargo = (ROOT / "crates/crm-application-runtime/Cargo.toml").read_text(
            encoding="utf-8"
        )

        for owner in (parties, consents, contact_points, relationships):
            self.assertIn("pub fn mutation_capability_definitions()", owner)
            self.assertIn("pub fn query_capability_definitions()", owner)
            self.assertIn("pub fn build_contribution(", owner)
            self.assertIn("ActivationGatedMutationValidator::new", owner)
            self.assertIn("ActivationGatedQueryValidator::new", owner)

        order = [
            "build_parties_contribution(",
            "build_customer_accounts_contribution(",
            "build_consents_contribution(",
            "build_contact_points_contribution(",
            "build_party_relationships_contribution(",
        ]
        positions = [aggregate.index(marker) for marker in order]
        self.assertEqual(positions, sorted(positions))
        self.assertIn("PostgresPartyReferenceReader::new(store.clone())", aggregate)
        self.assertNotIn("@1.0.0", aggregate)
        self.assertNotIn("crm.parties", aggregate)
        self.assertNotIn("crm.contact-points", aggregate)
        self.assertNotIn("crm.party-relationships", aggregate)

        self.assertNotIn("ContactPointCapabilityPlanner", runtime)
        self.assertNotIn("ContactPointPartyReferenceSemanticValidator", runtime)
        self.assertNotIn("ContactPointQueryAdapter::new", runtime)
        self.assertNotIn("PartyRelationshipCapabilityPlanner", runtime)
        self.assertNotIn("PartyRelationshipReferenceSemanticValidator", runtime)
        self.assertNotIn("PartyRelationshipQueryAdapter::new", runtime)
        self.assertNotIn("use crm_consents_capability_adapter", runtime)
        self.assertNotIn("PostgresPartyReferenceReader::new", runtime)
        self.assertIn(
            "parties_mutation_capability_definitions as party_capability_definitions",
            runtime,
        )
        self.assertIn(
            "contact_points_mutation_capability_definitions as contact_point_capability_definitions",
            runtime,
        )
        self.assertIn(
            "party_relationships_query_capability_definitions as party_relationship_query_capability_definitions",
            runtime,
        )

        for dependency in (
            "crm-contact-points-capability-adapter",
            "crm-contact-points-capability-composition",
            "crm-contact-points-query-adapter",
            "crm-customer-accounts-capability-adapter",
            "crm-customer-accounts-query-adapter",
            "crm-party-reference-composition",
            "crm-party-relationships-capability-adapter",
            "crm-party-relationships-capability-composition",
            "crm-party-relationships-query-adapter",
        ):
            self.assertNotIn(dependency, runtime_cargo)
'''
    native_tests = replace_once(
        native_tests,
        "\n\nif __name__ == \"__main__\":\n",
        native_test + "\n\nif __name__ == \"__main__\":\n",
        "native composition batch test insertion",
    )
    write(native_test_path, native_tests)

    # Synchronize exact packet guards.
    navigation_path = root / "tests/test_repository_navigation.py"
    navigation = navigation_path.read_text(encoding="utf-8")
    navigation = replace_method(
        navigation,
        "test_active_packet_declaration_is_valid_and_exact(self) -> None:",
        "test_affected_scope_workflow_executes_real_packet_check(self) -> None:",
        '''    def test_active_packet_declaration_is_valid_and_exact(self) -> None:
        packet = load_packet(ROOT)
        self.assertEqual(
            packet["packet_id"],
            "repository-step-12-contribution-aggregation-batch-1",
        )
        self.assertEqual(packet["status"], "active")
        self.assertEqual(packet["baseline"]["ref"], "main")
        self.assertEqual(
            packet["baseline"]["sha"],
            "043de0298ea9b3415e9894b4c5d69952856fd377",
        )
        self.assertEqual(packet["tracking_issues"], [194])
        for path in (
            "Cargo.lock",
            "crates/crm-application-runtime/Cargo.toml",
            "crates/crm-application-runtime/src/native_composition.rs",
            "crates/crm-consents-capability-composition/src/lib.rs",
            "crates/crm-contact-points-capability-composition/Cargo.toml",
            "crates/crm-contact-points-capability-composition/src/lib.rs",
            "crates/crm-first-party-modules/Cargo.toml",
            "crates/crm-first-party-modules/src/lib.rs",
            "crates/crm-party-reference-composition/Cargo.toml",
            "crates/crm-party-reference-composition/src/lib.rs",
            "crates/crm-party-relationships-capability-composition/Cargo.toml",
            "crates/crm-party-relationships-capability-composition/src/lib.rs",
            "docs/ACTIVE_PACKET.md",
            "repository-packet.json",
            "scripts/check_native_module_composition.py",
            "tests/test_architecture_documentation_consistency.py",
            "tests/test_native_module_composition.py",
            "tests/test_repository_navigation.py",
        ):
            self.assertIn(path, packet["allowed_paths"])
        for path in (
            ".github/workflows/**",
            "affected-scope-policy.json",
            "apps/**",
            "contracts/**",
            "database/**",
            "modules/**",
            "packages/**",
            "proto/**",
            "schemas/**",
            "services/**",
        ):
            self.assertIn(path, packet["forbidden_paths"])
        self.assertEqual(
            packet["required_checks"],
            [
                "Affected Scope CI",
                "Governance CI",
                "Rust CI",
                "Rust Generated Sync",
            ],
        )
        self.assertIn(
            "repository step 12 remains in progress and repository step 13 is not started",
            packet["acceptance"],
        )
        self.assertIn(
            "complete repository step 12 for Identity Resolution",
            packet["non_goals"],
        )''',
    )
    navigation = replace_method(
        navigation,
        "test_packet_check_reports_affected_scope_without_running_git_or_cargo(self) -> None:",
        "test_repo_parser_exposes_navigation_commands(self) -> None:",
        '''    def test_packet_check_reports_affected_scope_without_running_git_or_cargo(
        self,
    ) -> None:
        changed_paths = load_packet(ROOT)["allowed_paths"]
        affected = {
            "head_sha": "b" * 40,
            "changed_paths": changed_paths,
            "affected_packages": [],
            "selected_workflows": [
                {
                    "name": name,
                    "path": path,
                    "selected": True,
                    "reasons": ["test fixture"],
                }
                for name, path in (
                    ("Affected Scope CI", ".github/workflows/affected-scope.yml"),
                    ("Governance CI", ".github/workflows/governance.yml"),
                    ("Rust CI", ".github/workflows/rust.yml"),
                    ("Rust Generated Sync", ".github/workflows/rust-generated-sync.yml"),
                )
            ],
        }
        with (
            patch(
                "scripts.repository_navigation._git",
                return_value=(
                    "043de0298ea9b3415e9894b4c5d69952856fd377"
                ),
            ),
            patch(
                "scripts.repository_navigation.build_report",
                return_value=affected,
            ),
            patch(
                "scripts.repository_navigation.stale_generated_documents",
                return_value=[],
            ),
        ):
            report = packet_check(ROOT, "origin/main")
        self.assertTrue(report["ok"])
        self.assertEqual(report["changed_paths"], changed_paths)
        self.assertEqual(report["blockers"], [])
        self.assertEqual(
            report["selected_workflows"][0]["name"],
            "Affected Scope CI",
        )''',
    )
    write(navigation_path, navigation)

    consistency_path = root / "tests/test_architecture_documentation_consistency.py"
    consistency = consistency_path.read_text(encoding="utf-8")
    consistency = replace_method(
        consistency,
        "test_active_packet_is_machine_declared_and_generated(self) -> None:",
        "test_stage_accountability_and_live_catalog_are_current(self) -> None:",
        '''    def test_active_packet_is_machine_declared_and_generated(self) -> None:
        self.assertEqual(self.packet["schema_version"], "crm.repository-packet/v1")
        self.assertEqual(
            self.packet["packet_id"],
            "repository-step-12-contribution-aggregation-batch-1",
        )
        self.assertEqual(self.packet["status"], "active")
        self.assertEqual(self.packet["baseline"]["ref"], "main")
        self.assertEqual(
            self.packet["baseline"]["sha"],
            "043de0298ea9b3415e9894b4c5d69952856fd377",
        )
        self.assertEqual(self.packet["tracking_issues"], [194])
        for path in (
            "Cargo.lock",
            "crates/crm-application-runtime/Cargo.toml",
            "crates/crm-application-runtime/src/native_composition.rs",
            "crates/crm-consents-capability-composition/src/lib.rs",
            "crates/crm-contact-points-capability-composition/Cargo.toml",
            "crates/crm-contact-points-capability-composition/src/lib.rs",
            "crates/crm-first-party-modules/Cargo.toml",
            "crates/crm-first-party-modules/src/lib.rs",
            "crates/crm-party-reference-composition/Cargo.toml",
            "crates/crm-party-reference-composition/src/lib.rs",
            "crates/crm-party-relationships-capability-composition/Cargo.toml",
            "crates/crm-party-relationships-capability-composition/src/lib.rs",
            "docs/ACTIVE_PACKET.md",
            "repository-packet.json",
            "scripts/check_native_module_composition.py",
            "tests/test_architecture_documentation_consistency.py",
            "tests/test_native_module_composition.py",
            "tests/test_repository_navigation.py",
        ):
            self.assertIn(path, self.packet["allowed_paths"])
        for path in (
            ".github/workflows/**",
            "affected-scope-policy.json",
            "apps/**",
            "contracts/**",
            "database/**",
            "modules/**",
            "packages/**",
            "proto/**",
            "schemas/**",
            "services/**",
        ):
            self.assertIn(path, self.packet["forbidden_paths"])
        self.assertEqual(
            self.packet["required_checks"],
            [
                "Affected Scope CI",
                "Governance CI",
                "Rust CI",
                "Rust Generated Sync",
            ],
        )
        self.assertIn(
            "repository step 12 remains in progress and repository step 13 is not started",
            self.packet["acceptance"],
        )
        self.assertIn(
            "complete repository step 12 for Identity Resolution",
            self.packet["non_goals"],
        )

        self.assertIn("Generated by scripts/generate_repository_navigation.py", self.active_packet)
        self.assertIn(
            "repository-step-12-contribution-aggregation-batch-1",
            self.active_packet,
        )
        self.assertIn(self.packet["baseline"]["sha"], self.active_packet)
        self.assertRegex(self.active_packet, r"sha256:[0-9a-f]{64}")
        self.assertIn("orientation only", self.active_packet)

        for document in self.authoritative_status_documents:
            self.assertIn("PR #244", document)
            self.assertIn("405d2dbb97bb371b51cfb1d4ffb5549a57262878", document)
            self.assertIn("4b08202fe9dd0c0df83567e24e6b9d86fb79c9db", document)
            self.assertIn("34 of 34", document)
            self.assertIn("repository step 12", document.lower())''',
    )
    write(consistency_path, consistency)

    write_generated_documents(root)
    subprocess.run([sys.executable, "scripts/repo.py", "lock"], cwd=root, check=True)
    subprocess.run(["cargo", "fmt", "--all"], cwd=root, check=True)
    return True


def commit(root: Path) -> None:
    branch = os.environ.get("GITHUB_HEAD_REF") or os.environ.get("GITHUB_REF_NAME")
    if not branch:
        raise NavigationError("step-12 batch-1 materializer requires a branch ref")
    subprocess.run(["git", "config", "user.name", "github-actions[bot]"], cwd=root, check=True)
    subprocess.run(
        ["git", "config", "user.email", "41898282+github-actions[bot]@users.noreply.github.com"],
        cwd=root,
        check=True,
    )
    paths = [
        "Cargo.lock",
        "crates/crm-application-runtime/Cargo.toml",
        "crates/crm-application-runtime/src/native_composition.rs",
        "crates/crm-consents-capability-composition/src/lib.rs",
        "crates/crm-contact-points-capability-composition/Cargo.toml",
        "crates/crm-contact-points-capability-composition/src/lib.rs",
        "crates/crm-first-party-modules/Cargo.toml",
        "crates/crm-first-party-modules/src/lib.rs",
        "crates/crm-party-reference-composition/Cargo.toml",
        "crates/crm-party-reference-composition/src/lib.rs",
        "crates/crm-party-relationships-capability-composition/Cargo.toml",
        "crates/crm-party-relationships-capability-composition/src/lib.rs",
        "docs/ACTIVE_PACKET.md",
        "scripts/check_native_module_composition.py",
        "tests/test_architecture_documentation_consistency.py",
        "tests/test_native_module_composition.py",
        "tests/test_repository_navigation.py",
    ]
    subprocess.run(["git", "add", *paths], cwd=root, check=True)
    subprocess.run(
        ["git", "commit", "-m", "Aggregate first relationship owner contributions"],
        cwd=root,
        check=True,
    )
    subprocess.run(["git", "push", "origin", f"HEAD:{branch}"], cwd=root, check=True)


def main() -> int:
    args = parser().parse_args()
    try:
        if args.write:
            if materialize(args.root):
                commit(args.root)
            else:
                write_generated_documents(args.root)
            return 0
        stale = stale_generated_documents(args.root)
    except (NavigationError, subprocess.CalledProcessError) as error:
        print(f"ERROR: {error}", file=sys.stderr)
        return 1
    if stale:
        for path in stale:
            print(f"STALE {path}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
