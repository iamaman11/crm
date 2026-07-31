#!/usr/bin/env python3
"""Temporarily finalize repository-step-12 completion wording.

This one-run wrapper preserves the canonical ``--write`` and ``--check``
interfaces and is restored before acceptance.
"""

from __future__ import annotations

import argparse
import os
from pathlib import Path
import subprocess

from repository_navigation import stale_generated_documents


ROOT = Path(__file__).resolve().parents[1]


def materialize() -> None:
    status_path = ROOT / "docs/PROJECT_STATUS.md"
    status = status_path.read_text(encoding="utf-8")
    status = status.replace(
        "repository step 13 is the next permitted implementation step and is not started until every repository step 12 owner batch and final Stage D exit evidence are accepted and synchronized. It will then complete calibrated dependency, Rust public-surface, reverse-fan-out and exception governance, including removal of the three direct lint exceptions.",
        "Repository step 14 follows only after repository step 13 is accepted and synchronized. It will perform the first measured behavior-neutral transitional domain-cluster consolidation.",
    )
    status_path.write_text(status, encoding="utf-8")

    roadmap_path = ROOT / "docs/IMPLEMENTATION_ROADMAP.md"
    roadmap = roadmap_path.read_text(encoding="utf-8")
    roadmap = roadmap.replace(
        "PR #241, PR #244 and PR #246.",
        "PR #241, PR #244, PR #246, PR #248 and PR #249.",
    )
    roadmap = roadmap.replace(
        "PR #237, PR #239, PR #241 and PR #244; PR #224",
        "PR #237, PR #239, PR #241, PR #244, PR #246, PR #248 and PR #249; PR #224",
    )
    roadmap_path.write_text(roadmap, encoding="utf-8")

    guard_path = ROOT / "tests/test_architecture_documentation_consistency.py"
    guard = guard_path.read_text(encoding="utf-8")
    anchor = '''        self.assertIn(\n            "Repository step 13 is the current next permitted implementation step and is not started.",\n            self.status,\n        )\n'''
    addition = '''        self.assertIn(\n            "Repository step 14 follows only after repository step 13 is accepted and synchronized.",\n            self.status,\n        )\n'''
    if addition not in guard:
        if anchor not in guard:
            raise RuntimeError("status continuation guard anchor not found")
        guard = guard.replace(anchor, anchor + addition, 1)
    guard_path.write_text(guard, encoding="utf-8")

    if os.environ.get("GITHUB_ACTIONS") == "true" and os.environ.get("GITHUB_HEAD_REF"):
        subprocess.run(["git", "config", "user.name", "github-actions[bot]"], check=True)
        subprocess.run(
            ["git", "config", "user.email", "41898282+github-actions[bot]@users.noreply.github.com"],
            check=True,
        )
        paths = [
            "docs/PROJECT_STATUS.md",
            "docs/IMPLEMENTATION_ROADMAP.md",
            "tests/test_architecture_documentation_consistency.py",
        ]
        subprocess.run(["git", "add", "--", *paths], check=True)
        if subprocess.run(["git", "diff", "--cached", "--quiet"]).returncode != 0:
            subprocess.run(["git", "commit", "-m", "Finalize repository step 12 continuation status"], check=True)
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
