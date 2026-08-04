#!/usr/bin/env python3
"""Run the corrected Step 18 materializer with an exact first-match patch."""

from __future__ import annotations

from pathlib import Path
import subprocess


ROOT = Path(__file__).resolve().parents[1]
WRAPPER_BLOB = "4858d438d52768073e983f08e32f980985c48c00"
content = subprocess.check_output(
    ["git", "cat-file", "blob", WRAPPER_BLOB],
    cwd=ROOT,
    text=True,
)
old = '''replace_once(
    '        "18. deterministic local lifecycle commands — **in progress through PRs #281 and #283; doctor/bootstrap/dev-up/dev-reset accepted, seed-demo/smoke next**;",',
    '        "18. deterministic local lifecycle commands: `doctor`, `bootstrap`, `dev-up`, `dev-reset`, `seed-demo` and `smoke` — **in progress through PRs #281 and #283; doctor/bootstrap/dev-up/dev-reset accepted, seed-demo/smoke next**;",',
    "architecture Step 18 exact source",
)
'''
new = '''architecture_old = (
    '        "18. deterministic local lifecycle commands — **in progress through PRs #281 and #283; doctor/bootstrap/dev-up/dev-reset accepted, seed-demo/smoke next**;",'
)
architecture_new = (
    '        "18. deterministic local lifecycle commands: `doctor`, `bootstrap`, `dev-up`, `dev-reset`, `seed-demo` and `smoke` — **in progress through PRs #281 and #283; doctor/bootstrap/dev-up/dev-reset accepted, seed-demo/smoke next**;",'
)
count = content.count(architecture_old)
if count != 2:
    raise RuntimeError(
        f"architecture Step 18 exact source: expected 2 matches, found {count}"
    )
content = content.replace(architecture_old, architecture_new, 1)
'''
count = content.count(old)
if count != 1:
    raise RuntimeError(f"wrapper architecture patch: expected 1 match, found {count}")
content = content.replace(old, new, 1)
namespace = {
    "__name__": "__main__",
    "__file__": str(Path(__file__).resolve()),
}
exec(compile(content, str(Path(__file__).resolve()), "exec"), namespace)
