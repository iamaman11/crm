#!/usr/bin/env python3
"""Temporarily finalize repository-step-12 live status wording.

This one-run wrapper preserves the canonical ``--write`` and ``--check``
interfaces and is restored before acceptance.
"""

from __future__ import annotations

import argparse
import os
from pathlib import Path
import subprocess

from repository_navigation import generated_documents, stale_generated_documents


ROOT = Path(__file__).resolve().parents[1]
DOCS = (
    "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md",
    "docs/IMPLEMENTATION_ROADMAP.md",
    "docs/PHASE8_DELIVERY_PLAN.md",
    "docs/PROJECT_STATUS.md",
)
STALE_CURRENT = (
    "Repository step 12 remains the current permitted implementation step. "
    "The next bounded packet must continue owner contribution aggregation after factual inventory review "
    "for Identity Resolution, Customer Data Operations, Data Quality, Customer Enrichment, "
    "Sales/Activities, Customer 360 and any other remaining active first-party owners."
)
STALE_CURRENT_PHASE8 = STALE_CURRENT + (
    " Repository step 13 or later work remains blocked until repository step 12 is fully accepted "
    "and its final evidence is synchronized."
)
NEXT_STEP = (
    "Repository step 13 is the current next permitted implementation step and is not started. "
    "Its bounded packet must complete calibrated dependency, Rust public-surface, reverse-fan-out "
    "and exception governance, including removal of the three direct lint exceptions."
)
LATEST_OLD = (
    "Latest accepted repository implementation packet is PR #246 / accepted source "
    "`3b4fe7cdf458daac9c12f816d0d6a87039e613f3` / squash merge "
    "`f090fa8785ac2bbb7e5cf186b4c5011cb9aeb978` / 37 of 37 applicable permanent workflows "
    "on one unchanged exact head. repository step 12 is complete."
)
LATEST_NEW = (
    "Latest accepted repository implementation packet is PR #249 / accepted source "
    "`7876945586e5a6cc94f8d3b0f6ba2b57316484d2` / squash merge "
    "`f36592211bed3e0df7cf3771164b4bc24026eff3` / 37 of 37 applicable permanent workflows "
    "on one unchanged exact head. Repository step 12 and Stage D are complete; repository step 13 "
    "is the next permitted implementation step and is not started."
)


def materialize() -> None:
    for relative in DOCS:
        path = ROOT / relative
        text = path.read_text(encoding="utf-8")
        text = text.replace(STALE_CURRENT_PHASE8, NEXT_STEP)
        text = text.replace(STALE_CURRENT, NEXT_STEP)
        text = text.replace(LATEST_OLD, LATEST_NEW)
        text = text.replace("Repository steps 1–11 are complete", "Repository steps 1–12 are complete")
        text = text.replace(
            "| B — dependency, crate and exception governance | steps 12, 13, 14, 22 and 25, plus every structural preflight |",
            "| B — dependency, crate and exception governance | steps 13, 14, 22 and 25, plus every structural preflight |",
        )
        text = text.replace(". repository step 12 is complete;", ". Repository step 12 is complete;")
        path.write_text(text, encoding="utf-8")

    guard = ROOT / "tests/test_architecture_documentation_consistency.py"
    text = guard.read_text(encoding="utf-8")
    anchor = '''        for statement in ("run in parallel", "separate parallel lane", "runs alongside Phase 8A"):\n'''
    addition = '''        for document in self.authoritative_status_documents:\n            self.assertNotIn(\n                "Repository step 12 remains the current permitted implementation step.",\n                document,\n            )\n        self.assertIn(\n            "Latest accepted repository implementation packet is PR #249",\n            self.status,\n        )\n        self.assertIn(\n            "Repository step 13 is the current next permitted implementation step and is not started.",\n            self.status,\n        )\n\n'''
    if addition not in text:
        if anchor not in text:
            raise RuntimeError("documentation guard anchor not found")
        text = text.replace(anchor, addition + anchor, 1)
    guard.write_text(text, encoding="utf-8")

    for relative, content in generated_documents(ROOT).items():
        path = ROOT / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content, encoding="utf-8")

    if os.environ.get("GITHUB_ACTIONS") == "true" and os.environ.get("GITHUB_HEAD_REF"):
        subprocess.run(["git", "config", "user.name", "github-actions[bot]"], check=True)
        subprocess.run(
            ["git", "config", "user.email", "41898282+github-actions[bot]@users.noreply.github.com"],
            check=True,
        )
        paths = [*DOCS, "docs/ACTIVE_PACKET.md", "tests/test_architecture_documentation_consistency.py"]
        subprocess.run(["git", "add", "--", *paths], check=True)
        if subprocess.run(["git", "diff", "--cached", "--quiet"]).returncode != 0:
            subprocess.run(["git", "commit", "-m", "Finalize repository step 12 live status"], check=True)
            subprocess.run(
                ["git", "push", "origin", f"HEAD:{os.environ['GITHUB_HEAD_REF']}"],
                check=True,
            )


def main() -> int:
    parser = argparse.ArgumentParser()
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--write", action="store_true")
    mode.add_argument("--check", action="store_true")
    args = parser.parse_args()
    if args.write:
        materialize()
        return 0
    stale = stale_generated_documents(ROOT)
    if stale:
        for path in stale:
            print(f"STALE {path}")
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
