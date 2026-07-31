#!/usr/bin/env python3
"""Temporarily materialize the final repository-step-12 owner batch.

The canonical generator is restored before exact-head acceptance. The permanent
``--write`` and ``--check`` interface remains intact during materialization.
"""

from __future__ import annotations

import argparse
import os
from pathlib import Path
from pprint import pformat
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
        raise RuntimeError(f"{relative}: expected one anchor, found {count}: {old[:100]!r}")
    path.write_text(content.replace(old, new, 1), encoding="utf-8")


def remove_between(root: Path, relative: str, start: str, end: str) -> None:
    path = root / relative
    content = path.read_text(encoding="utf-8")
    if start not in content:
        return
    start_at = content.index(start)
    end_at = content.index(end, start_at)
    path.write_text(content[:start_at] + content[end_at:], encoding="utf-8")


def replace_method(
    root: Path,
    relative: str,
    method_name: str,
    next_method_name: str,
    replacement: str,
) -> None:
    path = root / relative
    content = path.read_text(encoding="utf-8")
    start = content.find(f"    def {method_name}(")
    end = content.find(f"    def {next_method_name}(", start + 1)
    if start < 0 or end < 0:
        raise RuntimeError(f"{relative}: packet test anchors are missing")
    path.write_text(content[:start] + replacement.rstrip() + "\n\n" + content[end:], encoding="utf-8")


def materialize_manifests(root: Path) -> None:
    replace_once(
        root,
        "crates/crm-sales-activities-capability-composition/Cargo.toml",
        "[dependencies]\ncrm-activities-capability-adapter",
        "[dependencies]\ncrm-application-composition = { path = \"../crm-application-composition\" }\ncrm-activities-capability-adapter",
    )
    replace_once(
        root,
        "crates/crm-customer-360-composition/Cargo.toml",
        "[dependencies]\ncrm-core-data",
        "[dependencies]\ncrm-application-composition = { path = \"../crm-application-composition\" }\ncrm-capability-runtime = { path = \"../crm-capability-runtime\" }\ncrm-core-data",
    )
    replace_once(
        root,
        "crates/crm-customer-360-composition/Cargo.toml",
        "crm-core-events = { path = \"../crm-core-events\" }\ncrm-module-sdk",
        "crm-core-events = { path = \"../crm-core-events\" }\ncrm-customer-360-query-adapter = { path = \"../crm-customer-360-query-adapter\" }\ncrm-module-sdk",
    )
    replace_once(
        root,
        "crates/crm-customer-360-composition/Cargo.toml",
        "crm-proto-contracts = { path = \"../crm-proto-contracts\" }\nprost",
        "crm-proto-contracts = { path = \"../crm-proto-contracts\" }\ncrm-query-runtime = { path = \"../crm-query-runtime\" }\nprost",
    )
    replace_once(
        root,
        "crates/crm-customer-enrichment-capability-composition/Cargo.toml",
        "[dependencies]\ncrm-capability-plan-support",
        "[dependencies]\ncrm-application-composition = { path = \"../crm-application-composition\" }\ncrm-capability-plan-support",
    )
    replace_once(
        root,
        "crates/crm-customer-enrichment-capability-composition/Cargo.toml",
        "crm-customer-enrichment-capability-adapter = { path = \"../crm-customer-enrichment-capability-adapter\" }\ncrm-module-sdk",
        "crm-customer-enrichment-capability-adapter = { path = \"../crm-customer-enrichment-capability-adapter\" }\ncrm-customer-enrichment-query-adapter = { path = \"../crm-customer-enrichment-query-adapter\" }\ncrm-customer-enrichment-request-list-query-adapter = { path = \"../crm-customer-enrichment-request-list-query-adapter\" }\ncrm-customer-enrichment-suggestion-query-adapter = { path = \"../crm-customer-enrichment-suggestion-query-adapter\" }\ncrm-module-sdk",
    )
    replace_once(
        root,
        "crates/crm-customer-enrichment-capability-composition/Cargo.toml",
        "prost = \"0.14\"\n",
        "prost = \"0.14\"\nsqlx = { version = \"0.9.0\", default-features = false, features = [\"runtime-tokio\", \"postgres\"] }\n",
    )
    replace_once(
        root,
        "crates/crm-first-party-modules/Cargo.toml",
        "crm-contact-points-capability-composition = { path = \"../crm-contact-points-capability-composition\" }\n",
        "crm-contact-points-capability-composition = { path = \"../crm-contact-points-capability-composition\" }\ncrm-customer-360-composition = { path = \"../crm-customer-360-composition\" }\ncrm-customer-enrichment-capability-composition = { path = \"../crm-customer-enrichment-capability-composition\" }\n",
    )
    replace_once(
        root,
        "crates/crm-first-party-modules/Cargo.toml",
        "crm-query-runtime = { path = \"../crm-query-runtime\" }\n",
        "crm-query-runtime = { path = \"../crm-query-runtime\" }\ncrm-sales-activities-capability-composition = { path = \"../crm-sales-activities-capability-composition\" }\n",
    )


def materialize_owner_modules(root: Path) -> None:
    replace_once(
        root,
        "crates/crm-sales-activities-capability-composition/src/lib.rs",
        "mod link_event_processor;\n",
        "mod production_contribution;\npub use production_contribution::*;\n\nmod link_event_processor;\n",
    )
    replace_once(
        root,
        "crates/crm-customer-360-composition/src/lib.rs",
        "use crm_core_data::PostgresDataStore;\n",
        "mod production_contribution;\npub use production_contribution::*;\n\nuse crm_core_data::PostgresDataStore;\n",
    )
    replace_once(
        root,
        "crates/crm-customer-enrichment-capability-composition/src/lib.rs",
        "use crm_capability_plan_support as support;\n",
        "mod production_contribution;\nmod reference_guards;\npub use production_contribution::*;\npub use reference_guards::*;\n\nuse crm_capability_plan_support as support;\n",
    )
    old_guard = root / "crates/crm-application-runtime/src/native_composition/customer_enrichment_reference_guards.rs"
    if old_guard.exists():
        old_guard.unlink()


def materialize_first_party(root: Path) -> None:
    path = root / "crates/crm-first-party-modules/src/lib.rs"
    content = path.read_text(encoding="utf-8")
    if "SalesActivitiesProductionDependencies" in content:
        return
    content = content.replace(
        "use crm_core_data::PostgresDataStore;\n",
        "use crm_core_data::PostgresDataStore;\n"
        "use crm_customer_360_composition::{\n"
        "    Customer360ProductionDependencies, build_contribution as build_customer_360_contribution,\n"
        "};\n"
        "pub use crm_customer_360_composition::query_capability_definitions as customer_360_query_capability_definitions;\n"
        "use crm_customer_enrichment_capability_composition::{\n"
        "    CustomerEnrichmentProductionDependencies,\n"
        "    build_contribution as build_customer_enrichment_contribution,\n"
        "};\n"
        "pub use crm_customer_enrichment_capability_composition::{\n"
        "    mutation_capability_definitions as customer_enrichment_mutation_capability_definitions,\n"
        "    query_capability_definitions as customer_enrichment_query_capability_definitions,\n"
        "};\n",
        1,
    )
    content = content.replace(
        "use crm_query_runtime::{QueryAuthorizer, QueryVisibilityAuthorizer};\n",
        "use crm_query_runtime::{QueryAuthorizer, QueryVisibilityAuthorizer};\n"
        "use crm_sales_activities_capability_composition::{\n"
        "    SalesActivitiesProductionDependencies,\n"
        "    build_contribution as build_sales_activities_contribution,\n"
        "};\n"
        "pub use crm_sales_activities_capability_composition::{\n"
        "    mutation_capability_definitions as sales_activities_mutation_capability_definitions,\n"
        "    query_capability_definitions as sales_activities_query_capability_definitions,\n"
        "};\n",
        1,
    )
    content = content.replace(
        "    let mut contributions = ModuleContributionSet::new();\n\n    contributions.merge(build_parties_contribution",
        "    let mut contributions = ModuleContributionSet::new();\n\n"
        "    contributions.merge(build_sales_activities_contribution(\n"
        "        SalesActivitiesProductionDependencies {\n"
        "            store: store.clone(),\n"
        "            activation: activation.clone(),\n"
        "            visibility_authorizer: visibility_authorizer.clone(),\n"
        "            cursor_key,\n"
        "        },\n"
        "    )?);\n\n"
        "    contributions.merge(build_parties_contribution",
        1,
    )
    relationship_anchor = '''    contributions.merge(build_party_relationships_contribution(
        PartyRelationshipsProductionDependencies {
            store: store.clone(),
            parties: parties.clone(),
            activation: activation.clone(),
            visibility_authorizer: visibility_authorizer.clone(),
            cursor_key,
        },
    )?);
'''
    customer360_block = relationship_anchor + '''    contributions.merge(build_customer_360_contribution(
        Customer360ProductionDependencies {
            store: store.clone(),
            activation: activation.clone(),
            visibility_authorizer: visibility_authorizer.clone(),
        },
    )?);
'''
    if relationship_anchor not in content:
        raise RuntimeError("first-party relationship contribution anchor missing")
    content = content.replace(relationship_anchor, customer360_block, 1)
    old_quality = '''    contributions.merge(build_data_quality_contribution(
        DataQualityProductionDependencies {
            store,
            activation,
            capability_authorizer,
            query_authorizer,
            visibility_authorizer,
        },
    )?);
'''
    new_quality = '''    contributions.merge(build_data_quality_contribution(
        DataQualityProductionDependencies {
            store: store.clone(),
            activation: activation.clone(),
            capability_authorizer,
            query_authorizer: query_authorizer.clone(),
            visibility_authorizer: visibility_authorizer.clone(),
        },
    )?);
    contributions.merge(build_customer_enrichment_contribution(
        CustomerEnrichmentProductionDependencies {
            store,
            activation,
            query_authorizer,
            visibility_authorizer,
            cursor_key,
        },
    )?);
'''
    if old_quality not in content:
        raise RuntimeError("first-party Data Quality contribution anchor missing")
    path.write_text(content.replace(old_quality, new_quality, 1), encoding="utf-8")


def materialize_native(root: Path) -> None:
    path = root / "crates/crm-application-runtime/src/native_composition.rs"
    content = path.read_text(encoding="utf-8")
    if "SalesActivitiesCapabilityPlannerRouter" not in content:
        return
    content = content.replace("mod customer_enrichment_reference_guards;\n\n", "", 1)
    content = content.replace(
        "use crm_application_composition::{\n    ActivationGatedMutationValidator, ActivationGatedQueryValidator, ApplicationComposition,\n    ModuleActivationPort, ModuleContributionSet, NoopMutationSemanticValidator,\n};",
        "use crm_application_composition::{\n    ApplicationComposition, ModuleActivationPort, ModuleContributionSet,\n    NoopMutationSemanticValidator,\n};",
        1,
    )
    content = content.replace(
        "use crm_capability_runtime::{\n    CapabilityAuthorizer, CapabilityDefinition, CapabilitySemanticValidator,\n    TransactionalCapabilityExecutor,\n};",
        "use crm_capability_runtime::{CapabilityAuthorizer, CapabilityDefinition};",
        1,
    )
    content = content.replace("use crm_consents_query_adapter::ConsentQueryAdapter;\n", "")
    content = content.replace(
        "use crm_core_data::{\n    PostgresDataStore, PostgresMetadataCapabilityExecutor, PostgresMetadataQueryStore,\n    PostgresTransactionalAggregateExecutor, TransactionalAggregatePlanner,\n};",
        "use crm_core_data::{\n    PostgresDataStore, PostgresMetadataCapabilityExecutor, PostgresMetadataQueryStore,\n};",
        1,
    )
    for start, end in (
        ("use crm_customer_360_query_adapter::{", "use crm_first_party_modules::{"),
        ("use crm_sales_activities_capability_composition::{", "use crm_sales_activities_link::MODULE_ID as LINK_MODULE_ID;"),
        ("use crm_sales_activities_query_adapter::{", "use crm_search_query_adapter::{"),
        ("use customer_enrichment_reference_guards::{", "use std::collections::BTreeSet;"),
    ):
        if start in content:
            start_at = content.index(start)
            end_at = content.index(end, start_at)
            content = content[:start_at] + content[end_at:]
    content = content.replace("use crm_parties_query_adapter::PartyQueryAdapter;\n", "")
    content = content.replace(
        "use crm_query_runtime::{\n    CursorCodec, QueryAuthorizer, QueryExecutor, QuerySemanticValidator, QueryVisibilityAuthorizer,\n};",
        "use crm_query_runtime::{CursorCodec, QueryAuthorizer, QueryVisibilityAuthorizer};",
        1,
    )
    old_first_party = '''use crm_first_party_modules::{
    FirstPartyProductionDependencies, build_all as build_first_party_modules,
    consents_mutation_capability_definitions as consent_capability_definitions,
    consents_query_capability_definitions as consent_query_capability_definitions,
    contact_points_mutation_capability_definitions as contact_point_capability_definitions,
    contact_points_query_capability_definitions as contact_point_query_capability_definitions,
    customer_accounts_mutation_capability_definitions as account_capability_definitions,
    customer_accounts_query_capability_definitions as account_query_capability_definitions,
    customer_data_operations_mutation_capability_definitions as customer_data_operations_capability_definitions,
    customer_data_operations_query_capability_definitions,
    data_quality_mutation_capability_definitions as data_quality_capability_definitions,
    data_quality_query_capability_definitions,
    identity_resolution_mutation_capability_definitions as identity_resolution_capability_definitions,
    identity_resolution_query_capability_definitions,
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
    customer_360_query_capability_definitions,
    customer_accounts_mutation_capability_definitions as account_capability_definitions,
    customer_accounts_query_capability_definitions as account_query_capability_definitions,
    customer_data_operations_mutation_capability_definitions as customer_data_operations_capability_definitions,
    customer_data_operations_query_capability_definitions,
    customer_enrichment_mutation_capability_definitions as customer_enrichment_capability_definitions,
    customer_enrichment_query_capability_definitions,
    data_quality_mutation_capability_definitions as data_quality_capability_definitions,
    data_quality_query_capability_definitions,
    identity_resolution_mutation_capability_definitions as identity_resolution_capability_definitions,
    identity_resolution_query_capability_definitions,
    parties_mutation_capability_definitions as party_capability_definitions,
    parties_query_capability_definitions as party_query_capability_definitions,
    party_relationships_mutation_capability_definitions as party_relationship_capability_definitions,
    party_relationships_query_capability_definitions as party_relationship_query_capability_definitions,
    sales_activities_mutation_capability_definitions as sales_activities_capability_definitions,
    sales_activities_query_capability_definitions,
};
'''
    if old_first_party not in content:
        raise RuntimeError("native first-party import anchor missing")
    content = content.replace(old_first_party, new_first_party, 1)
    content = content.replace(
        "    definitions.extend(customer_enrichment_query_capability_definitions()?);\n    definitions.push(customer_enrichment_request_list_query_capability_definition()?);\n    definitions.push(get_suggestion_capability_definition()?);\n",
        "    definitions.extend(customer_enrichment_query_capability_definitions()?);\n",
        1,
    )
    blocks = (
        ("    let sales_activities_executor =", "    contributions.merge(build_first_party_modules("),
        ("    let customer_enrichment_fallback:", "    contributions\n        .add_mutations(\n            metadata_mutation_capability_definitions()?"),
        ("    let sales_activities_queries =", "    let customer_360_queries ="),
        ("    let customer_360_queries =", "    let customer_enrichment_queries ="),
        ("    let customer_enrichment_queries =", "    let search_queries ="),
        ("fn aggregate_executor<P>(", "fn cursor("),
    )
    for start, end in blocks:
        if start in content:
            start_at = content.index(start)
            end_at = content.index(end, start_at)
            content = content[:start_at] + content[end_at:]
    path.write_text(content, encoding="utf-8")


def materialize_guards(root: Path) -> None:
    guard_path = root / "scripts/check_native_module_composition.py"
    guard = guard_path.read_text(encoding="utf-8")
    if "Sales/Activities mutation construction returned" not in guard:
        markers = '''    LegacyMarker(
        "crates/crm-application-runtime/src/native_composition.rs",
        "SalesActivitiesCapabilityPlannerRouter",
        "Sales/Activities mutation construction returned to the generic process host",
    ),
    LegacyMarker(
        "crates/crm-application-runtime/src/native_composition.rs",
        "SalesActivitiesQueryAdapter::new",
        "Sales/Activities query construction returned to the generic process host",
    ),
    LegacyMarker(
        "crates/crm-application-runtime/src/native_composition.rs",
        "Customer360QueryAdapter::new",
        "Customer 360 query construction returned to the generic process host",
    ),
    LegacyMarker(
        "crates/crm-application-runtime/src/native_composition.rs",
        "CustomerEnrichmentCapabilityExecutor::new",
        "Customer Enrichment mutation construction returned to the generic process host",
    ),
    LegacyMarker(
        "crates/crm-application-runtime/src/native_composition.rs",
        "CustomerEnrichmentQueryAdapter::new",
        "Customer Enrichment query construction returned to the generic process host",
    ),
    LegacyMarker(
        "crates/crm-application-runtime/src/native_composition.rs",
        "SalesActivitiesProductionDependencies",
        "Sales/Activities contribution bypassed the first-party aggregate",
    ),
    LegacyMarker(
        "crates/crm-application-runtime/src/native_composition.rs",
        "Customer360ProductionDependencies",
        "Customer 360 contribution bypassed the first-party aggregate",
    ),
    LegacyMarker(
        "crates/crm-application-runtime/src/native_composition.rs",
        "CustomerEnrichmentProductionDependencies",
        "Customer Enrichment contribution bypassed the first-party aggregate",
    ),
'''
        guard = guard.replace(")\n\n\ndef find_legacy_composition_violations", markers + ")\n\n\ndef find_legacy_composition_violations", 1)
        guard_path.write_text(guard, encoding="utf-8")

    test_path = root / "tests/test_native_module_composition.py"
    tests = test_path.read_text(encoding="utf-8")
    if "test_final_owner_batch_is_aggregated" not in tests:
        method = '''    def test_final_owner_batch_is_aggregated(self) -> None:
        sales = (
            ROOT
            / "crates/crm-sales-activities-capability-composition/src/production_contribution.rs"
        ).read_text(encoding="utf-8")
        customer_360 = (
            ROOT / "crates/crm-customer-360-composition/src/production_contribution.rs"
        ).read_text(encoding="utf-8")
        enrichment = (
            ROOT
            / "crates/crm-customer-enrichment-capability-composition/src/production_contribution.rs"
        ).read_text(encoding="utf-8")
        aggregate = (ROOT / "crates/crm-first-party-modules/src/lib.rs").read_text(
            encoding="utf-8"
        )
        runtime = (
            ROOT / "crates/crm-application-runtime/src/native_composition.rs"
        ).read_text(encoding="utf-8")

        for owner in (sales, enrichment):
            self.assertIn("pub fn mutation_capability_definitions()", owner)
            self.assertIn("pub fn query_capability_definitions()", owner)
            self.assertIn("pub fn build_contribution(", owner)
            self.assertIn("ActivationGatedMutationValidator::new", owner)
            self.assertIn("ActivationGatedQueryValidator::new", owner)
        self.assertIn("pub fn query_capability_definitions()", customer_360)
        self.assertIn("pub fn build_contribution(", customer_360)
        self.assertIn("ActivationGatedQueryValidator::new", customer_360)

        for marker in (
            "build_sales_activities_contribution(",
            "build_customer_360_contribution(",
            "build_customer_enrichment_contribution(",
        ):
            self.assertIn(marker, aggregate)
        for marker in (
            "SalesActivitiesCapabilityPlannerRouter",
            "SalesActivitiesQueryAdapter::new",
            "Customer360QueryAdapter::new",
            "CustomerEnrichmentCapabilityExecutor::new",
            "CustomerEnrichmentQueryAdapter::new",
        ):
            self.assertNotIn(marker, runtime)

        self.assertFalse(
            (
                ROOT
                / "crates/crm-application-runtime/src/native_composition/customer_enrichment_reference_guards.rs"
            ).exists()
        )
        self.assertTrue(
            (
                ROOT
                / "crates/crm-customer-enrichment-capability-composition/src/reference_guards.rs"
            ).exists()
        )

'''
        tests = tests.replace("\n\nif __name__ == \"__main__\":", "\n\n" + method + "if __name__ == \"__main__\":", 1)
        test_path.write_text(tests, encoding="utf-8")


def materialize_packet_tests(root: Path) -> None:
    expected_paths = [
        "Cargo.lock",
        "crates/crm-application-runtime/src/native_composition.rs",
        "crates/crm-application-runtime/src/native_composition/customer_enrichment_reference_guards.rs",
        "crates/crm-customer-360-composition/Cargo.toml",
        "crates/crm-customer-360-composition/src/lib.rs",
        "crates/crm-customer-360-composition/src/production_contribution.rs",
        "crates/crm-customer-enrichment-capability-composition/Cargo.toml",
        "crates/crm-customer-enrichment-capability-composition/src/lib.rs",
        "crates/crm-customer-enrichment-capability-composition/src/production_contribution.rs",
        "crates/crm-customer-enrichment-capability-composition/src/reference_guards.rs",
        "crates/crm-first-party-modules/Cargo.toml",
        "crates/crm-first-party-modules/src/lib.rs",
        "crates/crm-sales-activities-capability-composition/Cargo.toml",
        "crates/crm-sales-activities-capability-composition/src/lib.rs",
        "crates/crm-sales-activities-capability-composition/src/production_contribution.rs",
        "docs/ACTIVE_PACKET.md",
        "docs/generated/REPOSITORY_MAP.md",
        "repository-packet.json",
        "scripts/check_native_module_composition.py",
        "scripts/generate_repository_navigation.py",
        "tests/test_architecture_documentation_consistency.py",
        "tests/test_native_module_composition.py",
        "tests/test_repository_navigation.py",
    ]
    paths_literal = pformat(expected_paths, width=88, sort_dicts=False)
    architecture = f'''    def test_active_packet_is_machine_declared_and_generated(self) -> None:
        self.assertEqual(self.packet["schema_version"], "crm.repository-packet/v1")
        self.assertEqual(self.packet["packet_id"], "repository-step-12-contribution-aggregation-batch-3")
        self.assertEqual(self.packet["status"], "active")
        self.assertEqual(self.packet["baseline"]["ref"], "main")
        self.assertEqual(self.packet["baseline"]["sha"], "b4222364c21cb74127834f5ff4f0739343d26379")
        self.assertEqual(self.packet["tracking_issues"], [194])
        self.assertEqual(self.packet["allowed_paths"], {paths_literal})
        self.assertEqual(
            self.packet["required_checks"],
            ["Affected Scope CI", "Governance CI", "Rust CI", "Rust Generated Sync"],
        )
        self.assertIn(
            "repository step 12 implementation is complete and repository step 13 is not started",
            self.packet["acceptance"],
        )
        self.assertIn("Generated by scripts/generate_repository_navigation.py", self.active_packet)
        self.assertIn("repository-step-12-contribution-aggregation-batch-3", self.active_packet)
        self.assertIn(self.packet["baseline"]["sha"], self.active_packet)
        self.assertRegex(self.active_packet, r"sha256:[0-9a-f]{{64}}")
        self.assertIn("orientation only", self.active_packet)
        for document in self.authoritative_status_documents:
            self.assertIn("PR #246", document)
            self.assertIn("repository step 12", document.lower())
            self.assertIn("repository step 13 remains blocked", document.lower())'''
    replace_method(
        root,
        "tests/test_architecture_documentation_consistency.py",
        "test_active_packet_is_machine_declared_and_generated",
        "test_stage_accountability_and_live_catalog_are_current",
        architecture,
    )
    repository = f'''    def test_active_packet_declaration_is_valid_and_exact(self) -> None:
        packet = load_packet(ROOT)
        self.assertEqual(packet["packet_id"], "repository-step-12-contribution-aggregation-batch-3")
        self.assertEqual(packet["status"], "active")
        self.assertEqual(packet["baseline"]["ref"], "main")
        self.assertEqual(packet["baseline"]["sha"], "b4222364c21cb74127834f5ff4f0739343d26379")
        self.assertEqual(packet["tracking_issues"], [194])
        self.assertEqual(packet["allowed_paths"], {paths_literal})
        self.assertEqual(
            packet["required_checks"],
            ["Affected Scope CI", "Governance CI", "Rust CI", "Rust Generated Sync"],
        )
        self.assertIn(
            "repository step 12 implementation is complete and repository step 13 is not started",
            packet["acceptance"],
        )'''
    replace_method(
        root,
        "tests/test_repository_navigation.py",
        "test_active_packet_declaration_is_valid_and_exact",
        "test_affected_scope_workflow_executes_real_packet_check",
        repository,
    )
    nav = root / "tests/test_repository_navigation.py"
    content = nav.read_text(encoding="utf-8")
    content = content.replace(
        '"10be9a128ed1f8fbc6967d82baba648ba52f1d12"',
        '"b4222364c21cb74127834f5ff4f0739343d26379"',
    )
    nav.write_text(content, encoding="utf-8")


def commit_materialization(root: Path) -> None:
    subprocess.run(
        ["cargo", "check", "--workspace", "--all-targets", "--all-features"],
        cwd=root,
        check=True,
    )
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
    subprocess.run(
        [
            "git",
            "add",
            "Cargo.lock",
            "crates",
            "docs/ACTIVE_PACKET.md",
            "docs/generated/REPOSITORY_MAP.md",
            "scripts/check_native_module_composition.py",
            "tests/test_architecture_documentation_consistency.py",
            "tests/test_native_module_composition.py",
            "tests/test_repository_navigation.py",
        ],
        cwd=root,
        check=True,
    )
    subprocess.run(
        ["git", "commit", "-m", "Materialize final repository step 12 contribution batch"],
        cwd=root,
        check=True,
    )
    subprocess.run(["git", "push", "origin", f"HEAD:{branch}"], cwd=root, check=True)


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    root = args.root.resolve()
    try:
        if args.write:
            materialize_manifests(root)
            materialize_owner_modules(root)
            materialize_first_party(root)
            materialize_native(root)
            materialize_guards(root)
            materialize_packet_tests(root)
            commit_materialization(root)
            print("Final repository step 12 batch is materialized.")
            return 0
        stale = stale_generated_documents(root)
    except (NavigationError, OSError, RuntimeError, subprocess.CalledProcessError) as error:
        print(f"ERROR: {error}", file=sys.stderr)
        return 1
    if stale:
        for path in stale:
            print(f"STALE {path}", file=sys.stderr)
        print("ERROR: run python scripts/generate_repository_navigation.py --write", file=sys.stderr)
        return 1
    print("Repository navigation is synchronized.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
