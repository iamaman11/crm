#!/usr/bin/env python3
"""Run the accepted Step 18 synchronizer and close product-plan drift."""

from __future__ import annotations

from pathlib import Path
import subprocess


ROOT = Path(__file__).resolve().parents[1]
ORIGINAL_BLOB = "117a5625e201152ed9f98026a405d06c7f988c87"
PR285_SOURCE = "a522b8b11a0c6f143694f516e7a7f9d522c18ce3"
PR285_MERGE = "a906f2c514285974113749b8b8ad9446202a5fa1"
content = subprocess.check_output(
    ["git", "cat-file", "blob", ORIGINAL_BLOB],
    cwd=ROOT,
    text=True,
)


def replace_source_once(old: str, new: str, label: str) -> None:
    global content
    count = content.count(old)
    if count != 1:
        raise RuntimeError(f"{label}: expected 1 match, found {count}")
    content = content.replace(old, new, 1)


def replace_file_once(path: str, old: str, new: str, label: str) -> None:
    target = ROOT / path
    current = target.read_text(encoding="utf-8")
    count = current.count(old)
    if count != 1:
        raise RuntimeError(f"{label}: expected 1 match, found {count}")
    target.write_text(current.replace(old, new, 1), encoding="utf-8")


replace_source_once(
    '    "docs/PROJECT_STATUS.md",\n    "docs/WORKSPACE_COMPLEXITY_BASELINE.md",',
    '    "docs/PROJECT_STATUS.md",\n    "docs/PRODUCT_DEVELOPMENT_10_OF_10_PLAN.md",\n    "docs/WORKSPACE_COMPLEXITY_BASELINE.md",',
    "allowed product plan",
)
architecture_old = (
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
replace_source_once(
    '            "record exact PR #285 source, squash merge and 19-of-19 workflow evidence in all five normative documents",',
    '            "record exact PR #285 source, squash merge and 19-of-19 workflow evidence in all six normative documents",',
    "packet six-document deliverable",
)
replace_source_once(
    '            f"all five normative documents contain PR #285, source {PR285_SOURCE}, merge {PR285_MERGE} and 19 of 19",',
    '            f"all six normative documents contain PR #285, source {PR285_SOURCE}, merge {PR285_MERGE} and 19 of 19",',
    "packet six-document acceptance",
)
replace_source_once(
    '''    combined = "\\n".join(read(path) for path in NORMATIVE)
    for stale in (
        "seed-demo/smoke next",
        "next permitted bounded implementation packet is `seed-demo` and `smoke`",
        "Repository Step 19 remains blocked",
    ):
        if stale in combined:
            raise RuntimeError(f"stale Step 18 claim remains: {stale}")
''',
    '''    for document_path in NORMATIVE:
        document = read(document_path)
        for stale in (
            "seed-demo/smoke next",
            "next permitted bounded implementation packet is `seed-demo` and `smoke`",
            "Repository Step 19 remains blocked",
        ):
            if stale in document:
                raise RuntimeError(
                    f"{document_path}: stale Step 18 claim remains: {stale}"
                )
''',
    "per-document stale verification",
)

namespace = {
    "__name__": "__main__",
    "__file__": str(Path(__file__).resolve()),
}
exec(compile(content, str(Path(__file__).resolve()), "exec"), namespace)

product_path = "docs/PRODUCT_DEVELOPMENT_10_OF_10_PLAN.md"
replace_file_once(
    product_path,
    "This plan does not change the single repository execution order. Repository Step 15 remains the next implementation packet. Product waves below begin only when their stated architecture and prior-product dependencies are accepted.",
    "This plan does not change the single repository execution order. Repository Steps 1–18 are complete; Repository Step 19 is the next implementation packet. Product waves below begin only when their stated architecture and prior-product dependencies are accepted.",
    "product plan current repository step",
)
replace_file_once(
    product_path,
    "| Repository Steps 15–21 | Complete Phase 8A product/runtime/UX/operations evidence | Sequential architecture order | **Next through planned** |",
    "| Repository Steps 19–21 | Complete remaining Phase 8A product/runtime/UX/operations evidence | Sequential architecture order | **Next through planned** |",
    "product plan remaining repository steps",
)
replace_file_once(
    product_path,
    "It must finish:\n\n- Party tombstone and no-orphan semantics;\n- projection/search/cache convergence;\n- generic worker conformance and a real Customer Privacy worker;\n- contract compatibility/deprecation/retirement lifecycle;\n- deterministic local lifecycle commands;\n- customer privacy frontend/browser/accessibility acceptance;\n- restore, SLO, observability, performance, security and supply-chain evidence.",
    f"Accepted prerequisites now include Party tombstone/no-orphan and projection convergence, generic worker conformance, contract lifecycle enforcement, and the complete deterministic local lifecycle through PR #285 / source `{PR285_SOURCE}` / merge `{PR285_MERGE}` / 19 of 19 applicable permanent workflows.\n\nPhase 8A must still finish:\n\n- a real Customer Privacy worker lifecycle and complete process/end-to-end acceptance;\n- customer privacy frontend/browser/accessibility acceptance;\n- restore, SLO, observability, performance, security and supply-chain evidence.",
    "product plan Phase 8A remaining work",
)

guard_path = "tests/test_architecture_documentation_consistency.py"
replace_file_once(
    guard_path,
    '        cls.catalog = read("docs/MODULE_CATALOG.md")\n        cls.delivery = read("docs/DELIVERY_GOVERNANCE.md")',
    '        cls.catalog = read("docs/MODULE_CATALOG.md")\n        cls.product_plan = read("docs/PRODUCT_DEVELOPMENT_10_OF_10_PLAN.md")\n        cls.delivery = read("docs/DELIVERY_GOVERNANCE.md")',
    "architecture guard product-plan fixture",
)
replace_file_once(
    guard_path,
    '        self.assertIn("Repository Step 19", self.status)\n',
    f'''        self.assertIn("Repository Step 19", self.status)
        for marker in (
            "PR #285",
            "{PR285_SOURCE}",
            "{PR285_MERGE}",
            "19 of 19",
            "Repository Step 19",
        ):
            self.assertIn(marker, self.product_plan)
        self.assertNotIn(
            "Repository Step 15 remains the next implementation packet",
            self.product_plan,
        )
''',
    "product plan consistency assertions",
)

product = (ROOT / product_path).read_text(encoding="utf-8")
for marker in ("PR #285", PR285_SOURCE, PR285_MERGE, "19 of 19", "Repository Step 19"):
    if marker not in product:
        raise RuntimeError(f"{product_path}: missing {marker}")
if "Repository Step 15 remains the next implementation packet" in product:
    raise RuntimeError(f"{product_path}: stale Step 15 execution pointer remains")

subprocess.run(["git", "add", product_path], cwd=ROOT, check=True)
