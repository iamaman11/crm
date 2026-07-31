#!/usr/bin/env python3
"""Correct the temporary repository-step-12 batch-2 materializer.

The executable interface intentionally retains the permanent ``--write`` and
``--check`` markers. The canonical generator is restored before acceptance.
"""

from __future__ import annotations

from pathlib import Path
import subprocess

MATERIALIZER_COMMIT = "d77822675fb106b81305c6e98c7add916b00d7ba"

source = subprocess.check_output(
    [
        "git",
        "show",
        f"{MATERIALIZER_COMMIT}:scripts/generate_repository_navigation.py",
    ],
    text=True,
)

identity_before = '''    replace_once(
        root,
        "crates/crm-identity-resolution-capability-composition/src/lib.rs",
        "#![forbid(unsafe_code)]\\n",
        "#![forbid(unsafe_code)]\\n\\nmod production_contribution;\\npub use production_contribution::*;\\n",
    )
'''
identity_after = '''    replace_once(
        root,
        "crates/crm-identity-resolution-capability-composition/src/lib.rs",
        "use crm_capability_plan_support as support;\\n",
        "mod production_contribution;\\npub use production_contribution::*;\\n\\nuse crm_capability_plan_support as support;\\n",
    )
'''
customer_data_before = '''    replace_once(
        root,
        "crates/crm-customer-data-operations-execution-composition/src/lib.rs",
        "#![forbid(unsafe_code)]\\n",
        "#![forbid(unsafe_code)]\\n\\nmod production_contribution;\\npub use production_contribution::*;\\n",
    )
'''
customer_data_after = '''    replace_once(
        root,
        "crates/crm-customer-data-operations-execution-composition/src/lib.rs",
        "pub use crm_core_files::{\\n",
        "mod production_contribution;\\npub use production_contribution::*;\\n\\npub use crm_core_files::{\\n",
    )
'''

for before, after, label in (
    (identity_before, identity_after, "Identity Resolution"),
    (customer_data_before, customer_data_after, "Customer Data Operations"),
):
    if source.count(before) != 1:
        raise RuntimeError(f"temporary {label} insertion anchor is not unique")
    source = source.replace(before, after, 1)

namespace = {"__name__": "__main__", "__file__": str(Path(__file__).resolve())}
exec(compile(source, namespace["__file__"], "exec"), namespace)
