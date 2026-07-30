from __future__ import annotations

from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


def replace_exact(path: str, old: str, new: str) -> None:
    target = ROOT / path
    text = target.read_text(encoding="utf-8")
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{path}: expected one exact match, found {count}")
    target.write_text(text.replace(old, new), encoding="utf-8")


replace_exact(
    "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md",
    "additional homogeneous dependency cohorts, removal of the three direct-lint exceptions, public-surface/fan-out calibration and removal of the three direct lint exceptions at step 13, measured consolidation at step 14, and remeasurement at steps 22 and 25",
    "additional homogeneous dependency cohorts, public-surface/fan-out calibration and removal of the three direct lint exceptions at step 13, measured consolidation at step 14, and remeasurement at steps 22 and 25",
)
replace_exact(
    "docs/PROJECT_STATUS.md",
    "- Stage B dependency/crate/exception governance is in progress: reproducible metrics, calibrated inheritance cohorts, root-family no-growth, exact Rust `1.97.1`, root `rust-version`, measured zero-warning Rust/Clippy governance, lockfile-preserving Rust workflows and three exact expiring legacy lint exceptions are accepted; broader dependency/public-surface calibration and exception removal remain.",
    "- Stage B dependency/crate/exception governance is in progress: reproducible metrics, calibrated inheritance cohorts, root-family no-growth, exact Rust `1.97.1`, root `rust-version`, measured zero-warning Rust/Clippy governance, lockfile-preserving Rust workflows and three exact expiring legacy lint exceptions are accepted; repository step 13 owns the remaining dependency/public-surface/reverse-fan-out calibration and removal of those three direct lint exceptions.",
)
replace_exact(
    "docs/PROJECT_STATUS.md",
    "Repository step 12 completes first-party contribution aggregation for all currently active owners without behavior changes. Repository step 13 completes calibrated dependency, Rust public-surface, reverse-fan-out and exception governance, including removal of the three direct lint exceptions. Repository step 14 is the first measured behavior-neutral transitional domain-cluster consolidation.",
    "Repository step 12 completes first-party contribution aggregation for all currently active owners without behavior changes.",
)
replace_exact(
    "docs/MODULE_CATALOG.md",
    "Phase 8B / issue #29 remains planned and blocked on completed Phase 8A. Product Catalog, Pricing, CPQ, Quotes, Orders, Contracts, Subscriptions/Entitlements/Usage and governed billing/ERP/payment/tax/fulfillment remain independent owner domains.",
    "Phase 8B / issue #29 remains planned and blocked on completed Phase 8A plus the repository-step-22 measurement checkpoint. Repository step 23 is the first later expert-domain wave. Product Catalog, Pricing, CPQ, Quotes, Orders, Contracts, Subscriptions/Entitlements/Usage and governed billing/ERP/payment/tax/fulfillment remain independent owner domains.",
)
replace_exact(
    "tests/test_architecture_documentation_consistency.py",
    '        self.assertIn("repository step 25", self.plan.lower())\n        self.assertIn("seven mutations, four permission-aware public queries and zero Customer Privacy workers through PR #241", self.catalog)',
    '        self.assertIn("repository step 25", self.plan.lower())\n        self.assertIn("repository step 13 owns the remaining dependency/public-surface/reverse-fan-out calibration", self.status.lower())\n        self.assertIn("## Following permitted repository packet\\n\\nRepository step 12 completes first-party contribution aggregation", self.status)\n        self.assertNotIn("## Following permitted repository packet\\n\\nRepository step 12 completes first-party contribution aggregation for all currently active owners without behavior changes. Repository step 13", self.status)\n        self.assertIn("seven mutations, four permission-aware public queries and zero Customer Privacy workers through PR #241", self.catalog)',
)
