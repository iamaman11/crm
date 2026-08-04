#!/usr/bin/env python3
"""One-shot correction for the remaining Step 18 product-plan drift."""

from __future__ import annotations

from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


def replace_once(path: str, old: str, new: str, label: str) -> None:
    target = ROOT / path
    content = target.read_text(encoding="utf-8")
    count = content.count(old)
    if count != 1:
        raise RuntimeError(f"{label}: expected 1 match, found {count}")
    target.write_text(content.replace(old, new, 1), encoding="utf-8")


replace_once(
    "docs/PRODUCT_DEVELOPMENT_10_OF_10_PLAN.md",
    "The only next permitted implementation packet remains:\n\n> Repository Step 15 — Party tombstone, no-orphan proof and projection/search/cache convergence.\n\nAfter Steps 15–21 complete Phase 8A and Step 22 resolves architecture/runtime/gate decisions, Step 23 begins the first measured product-extension wave. No Phase 8B–11 implementation may bypass that order.",
    "The only next permitted implementation packet is:\n\n> Repository Step 19 — real Customer Privacy worker lifecycle and complete process/end-to-end acceptance.\n\nAfter Steps 19–21 complete Phase 8A and Step 22 resolves architecture/runtime/gate decisions, Step 23 begins the first measured product-extension wave. No Phase 8B–11 implementation may bypass that order.",
    "product-plan immediate continuation",
)
replace_once(
    "tests/test_architecture_documentation_consistency.py",
    '''        self.assertNotIn(
            "Repository Step 15 remains the next implementation packet",
            self.product_plan,
        )
''',
    '''        for stale in (
            "Repository Step 15 remains the next implementation packet",
            "Repository Step 15 — Party tombstone",
            "After Steps 15–21 complete Phase 8A",
        ):
            self.assertNotIn(stale, self.product_plan)
        self.assertIn(
            "Repository Step 19 — real Customer Privacy worker lifecycle",
            self.product_plan,
        )
        self.assertIn("After Steps 19–21 complete Phase 8A", self.product_plan)
''',
    "product-plan continuation guard",
)
