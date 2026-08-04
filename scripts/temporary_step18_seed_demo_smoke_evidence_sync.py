#!/usr/bin/env python3
"""Run the corrected materializer with final permanent-guard fixes."""

from __future__ import annotations

from pathlib import Path
import subprocess


ROOT = Path(__file__).resolve().parents[1]
WRAPPER_BLOB = "96de096512cca13d803c56d83ba5c385544d8507"
content = subprocess.check_output(
    ["git", "cat-file", "blob", WRAPPER_BLOB],
    cwd=ROOT,
    text=True,
)
old = 'subprocess.run(["git", "add", product_path], cwd=ROOT, check=True)\n'
new = '''replace_file_once(
    "docs/PHASE8_DELIVERY_PLAN.md",
    "18. Repository Step 18 — deterministic local lifecycle — **next, not started**;",
    "18. Repository Step 18 — deterministic local lifecycle — **complete through PR #285**;",
    "Phase 8 binding Step 18 closure",
)
replace_file_once(
    guard_path,
    '            for match in re.finditer(r"113", document):',
    '            for match in re.finditer(r"\\b113\\b", document):',
    "standalone historical package-count guard",
)
replace_file_once(
    "tests/test_repository_navigation.py",
    "repository-step-18-seed-demo-smoke-evidence-sync-evidence-sync",
    "repository-step-18-seed-demo-smoke-evidence-sync",
    "navigation packet id",
)
subprocess.run(["git", "add", product_path], cwd=ROOT, check=True)
'''
count = content.count(old)
if count != 1:
    raise RuntimeError(f"final guard patch: expected 1 match, found {count}")
content = content.replace(old, new, 1)
namespace = {
    "__name__": "__main__",
    "__file__": str(Path(__file__).resolve()),
}
exec(compile(content, str(Path(__file__).resolve()), "exec"), namespace)
