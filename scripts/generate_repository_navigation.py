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

identity_insertion = '''    replace_once(
        root,
        "crates/crm-identity-resolution-capability-composition/src/lib.rs",
        "#![forbid(unsafe_code)]\\n",
        "#![forbid(unsafe_code)]\\n\\nmod production_contribution;\\npub use production_contribution::*;\\n",
    )
'''
customer_data_insertion = '''    replace_once(
        root,
        "crates/crm-customer-data-operations-execution-composition/src/lib.rs",
        "#![forbid(unsafe_code)]\\n",
        "#![forbid(unsafe_code)]\\n\\nmod production_contribution;\\npub use production_contribution::*;\\n",
    )
'''
materialize_anchor = "def materialize(root: Path) -> None:\n"
materialize_replacement = '''def materialize(root: Path) -> None:
    contribution_declaration = "mod production_contribution;\\npub use production_contribution::*;"
    duplicate_declaration = (
        contribution_declaration + "\\n\\n" + contribution_declaration
    )
    for relative in (
        "crates/crm-identity-resolution-capability-composition/src/lib.rs",
        "crates/crm-customer-data-operations-execution-composition/src/lib.rs",
    ):
        path = root / relative
        content = path.read_text(encoding="utf-8")
        while duplicate_declaration in content:
            content = content.replace(duplicate_declaration, contribution_declaration, 1)
        path.write_text(content, encoding="utf-8")
'''
resolution_before = '''    subprocess.run(["cargo", "metadata", "--format-version", "1", "--no-deps"], cwd=root, check=True, stdout=subprocess.DEVNULL)
'''
resolution_after = '''    subprocess.run(
        ["cargo", "check", "--workspace", "--all-targets", "--all-features"],
        cwd=root,
        check=True,
    )
'''

for before, after, label in (
    (identity_insertion, "", "Identity Resolution repeated module insertion"),
    (customer_data_insertion, "", "Customer Data Operations repeated module insertion"),
    (materialize_anchor, materialize_replacement, "idempotent declaration normalization"),
    (resolution_before, resolution_after, "all-target lockfile and compilation resolution"),
):
    if source.count(before) != 1:
        raise RuntimeError(f"temporary {label} anchor is not unique")
    source = source.replace(before, after, 1)

namespace = {"__name__": "__main__", "__file__": str(Path(__file__).resolve())}
exec(compile(source, namespace["__file__"], "exec"), namespace)
