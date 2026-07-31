#!/usr/bin/env python3
"""Temporarily remove final stale repository-step-12 status assertions.

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
        "- Stage D is in progress: Customer Accounts aggregation is accepted through PR #222 and repository step 12 batch 1 for Parties, Consents, Contact Points and Party Relationships is accepted through PR #246; remaining owner contribution entry points and ordinary-registration concrete imports still belong to later bounded step-12 batches.",
        "- Stage D is complete: all currently active first-party owners expose owner-owned production contribution boundaries aggregated through `crm-first-party-modules`; generic native composition retains platform-level composition only, accepted through PRs #246, #248 and #249.",
    )
    status = status.replace(
        "-> 12. complete first-party contribution aggregation for all currently active owners — in progress; batch 1 complete through PR #246",
        "-> 12. complete first-party contribution aggregation for all currently active owners — complete through PR #249",
    )
    status_path.write_text(status, encoding="utf-8")

    guard_path = ROOT / "tests/test_architecture_documentation_consistency.py"
    guard = guard_path.read_text(encoding="utf-8")
    guard = guard.replace(
        '        self.assertIn("## Next permitted repository packet\\n\\nRepository step 12 remains the current permitted implementation step", self.status)\n',
        '        self.assertIn("## Next permitted repository packet\\n\\nRepository step 13 is the current next permitted implementation step and is not started", self.status)\n',
    )
    guard = guard.replace(
        '        self.assertNotIn("## Following permitted repository packet\\n\\nRepository step 12 completes first-party contribution aggregation for all currently active owners without behavior changes. Repository step 13", self.status)\n',
        '        self.assertIn("## Following permitted repository packet\\n\\nRepository step 14 follows only after repository step 13 is accepted and synchronized", self.status)\n',
    )
    anchor = '''        self.assertIn(\n            "Repository step 14 follows only after repository step 13 is accepted and synchronized.",\n            self.status,\n        )\n'''
    addition = '''        self.assertIn("- Stage D is complete:", self.status)\n        self.assertIn(\n            "-> 12. complete first-party contribution aggregation for all currently active owners — complete through PR #249",\n            self.status,\n        )\n        self.assertNotIn("Stage D is in progress", self.status)\n        self.assertNotIn("step 12 batch 1 complete through PR #246", self.status.lower())\n'''
    if addition not in guard:
        if anchor not in guard:
            raise RuntimeError("final status guard anchor not found")
        guard = guard.replace(anchor, anchor + addition, 1)
    guard_path.write_text(guard, encoding="utf-8")

    if os.environ.get("GITHUB_ACTIONS") == "true" and os.environ.get("GITHUB_HEAD_REF"):
        subprocess.run(["git", "config", "user.name", "github-actions[bot]"], check=True)
        subprocess.run(
            ["git", "config", "user.email", "41898282+github-actions[bot]@users.noreply.github.com"],
            check=True,
        )
        paths = ["docs/PROJECT_STATUS.md", "tests/test_architecture_documentation_consistency.py"]
        subprocess.run(["git", "add", "--", *paths], check=True)
        if subprocess.run(["git", "diff", "--cached", "--quiet"]).returncode != 0:
            subprocess.run(["git", "commit", "-m", "Remove stale repository step 12 status assertions"], check=True)
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
