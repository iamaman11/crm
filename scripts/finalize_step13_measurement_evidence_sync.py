#!/usr/bin/env python3
"""Temporarily advance permanent guards to the PR #253 evidence-sync packet."""

from __future__ import annotations

from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[1]

OLD_PACKET = "repository-step-13-current-main-measurement"
NEW_PACKET = "repository-step-13-measurement-evidence-sync"
OLD_SHA = "222187d988c321aee4d2e7bf81ba01b3205fd14c"
NEW_SHA = "7dcda204be07209d9e4996fdc9c5fd364cea179e"

ALLOWED = '''ALLOWED_PACKET_PATHS = [
    "docs/ACTIVE_PACKET.md",
    "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md",
    "docs/IMPLEMENTATION_ROADMAP.md",
    "docs/MODULE_CATALOG.md",
    "docs/PHASE8_DELIVERY_PLAN.md",
    "docs/PROJECT_STATUS.md",
    "repository-packet.json",
    "tests/test_architecture_documentation_consistency.py",
    "tests/test_repository_navigation.py",
]'''

CHECKS = '''[
                "Affected Scope CI",
                "Customer Privacy Access Export CI",
                "Customer Privacy Owner Execution CI",
                "Governance CI",
                "Rust CI",
                "Rust Generated Sync",
            ]'''


def patch(path: str) -> str:
    target = ROOT / path
    text = target.read_text(encoding="utf-8")
    text, count = re.subn(
        r"ALLOWED_PACKET_PATHS = \[.*?\]\n\n\nclass",
        ALLOWED + "\n\n\nclass",
        text,
        count=1,
        flags=re.DOTALL,
    )
    if count != 1:
        raise RuntimeError(f"could not replace allowed paths in {path}")
    text = text.replace(OLD_PACKET, NEW_PACKET)
    text = text.replace(OLD_SHA, NEW_SHA)
    text = text.replace(
        '''[
                "Affected Scope CI",
                "Complexity Baseline CI",
                "Governance CI",
                "Rust CI",
                "Rust Generated Sync",
            ]''',
        CHECKS,
    )
    text = text.replace(
        "the analyzer runs deterministically on the exact pull-request head with full git history",
        "the architecture plan, project status, roadmap, Phase 8 plan, module catalog and issues agree on accepted PR #253 evidence",
    )
    return text


def patch_navigation() -> None:
    path = "tests/test_repository_navigation.py"
    text = patch(path)
    old_tuple = '''                for name, path in (
                    ("Affected Scope CI", ".github/workflows/affected-scope.yml"),
                    ("Complexity Baseline CI", ".github/workflows/complexity-baseline.yml"),
                    ("Governance CI", ".github/workflows/governance.yml"),
                    ("Rust CI", ".github/workflows/rust.yml"),
                    ("Rust Generated Sync", ".github/workflows/rust-generated-sync.yml"),
                )'''
    new_tuple = '''                for name, path in (
                    ("Affected Scope CI", ".github/workflows/affected-scope.yml"),
                    ("Customer Privacy Access Export CI", ".github/workflows/customer-privacy-access-export.yml"),
                    ("Customer Privacy Owner Execution CI", ".github/workflows/customer-privacy-owner-execution.yml"),
                    ("Governance CI", ".github/workflows/governance.yml"),
                    ("Rust CI", ".github/workflows/rust.yml"),
                    ("Rust Generated Sync", ".github/workflows/rust-generated-sync.yml"),
                )'''
    if old_tuple not in text:
        raise RuntimeError("navigation selected-workflow fixture not found")
    text = text.replace(old_tuple, new_tuple, 1)
    (ROOT / path).write_text(text, encoding="utf-8")


def patch_consistency() -> None:
    path = "tests/test_architecture_documentation_consistency.py"
    text = patch(path)
    replacements = {
        "Repository step 13 is the current next permitted implementation step and is not started.": "Repository step 13 remains in progress.",
        "Repository step 13 is the next permitted implementation step and is not started.": "Repository step 13 remains in progress.",
        "The next permitted packet is repository-step-13 measurement and governance calibration only": "## Accepted repository step 13 current-main measurement",
        "## Next permitted repository packet\\n\\nRepository step 13 is the current next permitted implementation step and is not started": "## Next permitted repository packet\\n\\nRepository step 13 remains in progress",
    }
    for old, new in replacements.items():
        text = text.replace(old, new)
    text = text.replace(
        'self.assertIn("repository step 13 remains the next permitted implementation step", self.adr31)',
        'self.assertIn("repository step 13 remains the next permitted implementation step", self.adr31)',
    )
    (ROOT / path).write_text(text, encoding="utf-8")


def main() -> int:
    patch_navigation()
    patch_consistency()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
