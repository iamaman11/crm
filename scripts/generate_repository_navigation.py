#!/usr/bin/env python3
"""Correct the temporary final-step-12 materializer ownership boundaries.

The executable interface intentionally retains the permanent ``--write`` and
``--check`` markers. The canonical generator is restored before acceptance.
"""

from __future__ import annotations

from pathlib import Path
import subprocess

MATERIALIZER_COMMIT = "224caa6a91cf3cfaafb31dcaf5cb19ba6d67f84f"

source = subprocess.check_output(
    ["git", "show", f"{MATERIALIZER_COMMIT}:scripts/generate_repository_navigation.py"],
    text=True,
)

manifest_start = source.index(
    '    replace_once(\n        root,\n        "crates/crm-customer-360-composition/Cargo.toml",'
)
manifest_end = source.index(
    '    replace_once(\n        root,\n        "crates/crm-customer-enrichment-capability-composition/Cargo.toml",',
    manifest_start,
)
query_manifest = '''    replace_once(
        root,
        "crates/crm-customer-360-query-adapter/Cargo.toml",
        "[dependencies]\\ncrm-capability-plan-support",
        "[dependencies]\\ncrm-application-composition = { path = \\\"../crm-application-composition\\\" }\\ncrm-capability-plan-support",
    )
'''
source = source[:manifest_start] + query_manifest + source[manifest_end:]

replacements = (
    (
        "crm-customer-360-composition = { path = \\\"../crm-customer-360-composition\\\" }",
        "crm-customer-360-query-adapter = { path = \\\"../crm-customer-360-query-adapter\\\" }",
    ),
    ("crm_customer_360_composition", "crm_customer_360_query_adapter"),
    (
        "crates/crm-customer-360-composition/Cargo.toml",
        "crates/crm-customer-360-query-adapter/Cargo.toml",
    ),
    (
        "crates/crm-customer-360-composition/src/lib.rs",
        "crates/crm-customer-360-query-adapter/src/lib.rs",
    ),
    (
        "crates/crm-customer-360-composition/src/production_contribution.rs",
        "crates/crm-customer-360-query-adapter/src/production_contribution.rs",
    ),
    (
        '        self.assertIn("pub fn query_capability_definitions()", customer_360)\n',
        '        self.assertIn("pub fn production_query_capability_definitions()", customer_360)\n',
    ),
)
for before, after in replacements:
    if before not in source:
        raise RuntimeError(f"temporary Customer 360 correction anchor missing: {before}")
    source = source.replace(before, after)

sales_reexport_before = (
    '        "mod production_contribution;\\npub use production_contribution::*;\\n\\nmod link_event_processor;\\n",\n'
)
sales_reexport_after = (
    '        "mod production_contribution;\\npub use production_contribution::{\\n'
    '    build_contribution, mutation_capability_definitions, SalesActivitiesProductionDependencies,\\n'
    '};\\n\\nmod link_event_processor;\\n",\n'
)
if source.count(sales_reexport_before) != 1:
    raise RuntimeError("temporary Sales/Activities re-export anchor is not unique")
source = source.replace(sales_reexport_before, sales_reexport_after, 1)

sales_test_before = '''        for owner in (sales, enrichment):
            self.assertIn("pub fn mutation_capability_definitions()", owner)
            self.assertIn("pub fn query_capability_definitions()", owner)
            self.assertIn("pub fn build_contribution(", owner)
            self.assertIn("ActivationGatedMutationValidator::new", owner)
            self.assertIn("ActivationGatedQueryValidator::new", owner)
'''
sales_test_after = '''        self.assertIn("pub fn mutation_capability_definitions()", sales)
        self.assertIn("pub fn production_query_capability_definitions()", sales)
        self.assertIn("pub fn build_contribution(", sales)
        self.assertIn("ActivationGatedMutationValidator::new", sales)
        self.assertIn("ActivationGatedQueryValidator::new", sales)
        self.assertIn("pub fn mutation_capability_definitions()", enrichment)
        self.assertIn("pub fn query_capability_definitions()", enrichment)
        self.assertIn("pub fn build_contribution(", enrichment)
        self.assertIn("ActivationGatedMutationValidator::new", enrichment)
        self.assertIn("ActivationGatedQueryValidator::new", enrichment)
'''
if source.count(sales_test_before) != 1:
    raise RuntimeError("temporary Sales/Activities test anchor is not unique")
source = source.replace(sales_test_before, sales_test_after, 1)

namespace = {"__name__": "__main__", "__file__": str(Path(__file__).resolve())}
exec(compile(source, namespace["__file__"], "exec"), namespace)
