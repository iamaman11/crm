#!/usr/bin/env python3
"""One-shot staged correction for the remaining product-plan drift."""

from __future__ import annotations

from pathlib import Path
import subprocess


ROOT = Path(__file__).resolve().parents[1]
PATH = "docs/PRODUCT_DEVELOPMENT_10_OF_10_PLAN.md"
target = ROOT / PATH
content = target.read_text(encoding="utf-8")
old = "The only next permitted implementation packet remains:\n\n> Repository Step 15 — Party tombstone, no-orphan proof and projection/search/cache convergence.\n\nAfter Steps 15–21 complete Phase 8A and Step 22 resolves architecture/runtime/gate decisions, Step 23 begins the first measured product-extension wave. No Phase 8B–11 implementation may bypass that order."
new = "The only next permitted implementation packet is:\n\n> Repository Step 19 — real Customer Privacy worker lifecycle and complete process/end-to-end acceptance.\n\nAfter Steps 19–21 complete Phase 8A and Step 22 resolves architecture/runtime/gate decisions, Step 23 begins the first measured product-extension wave. No Phase 8B–11 implementation may bypass that order."
count = content.count(old)
if count != 1:
    raise RuntimeError(f"product-plan immediate continuation: expected 1 match, found {count}")
target.write_text(content.replace(old, new, 1), encoding="utf-8")
subprocess.run(["git", "add", PATH], cwd=ROOT, check=True)
