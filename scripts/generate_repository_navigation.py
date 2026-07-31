#!/usr/bin/env python3
"""Correct the temporary final-step-12 materializer ownership boundary.

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

namespace = {"__name__": "__main__", "__file__": str(Path(__file__).resolve())}
exec(compile(source, namespace["__file__"], "exec"), namespace)
