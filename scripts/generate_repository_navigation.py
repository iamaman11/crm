#!/usr/bin/env python3
"""One-run materializer for repository-step-12 batch-1 evidence."""

from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import re
import subprocess
import sys

from repository_navigation import NavigationError, stale_generated_documents, write_generated_documents


ACCEPTED_SOURCE = "3b4fe7cdf458daac9c12f816d0d6a87039e613f3"
MERGE_SHA = "f090fa8785ac2bbb7e5cf186b4c5011cb9aeb978"
EVIDENCE = (
    "Repository step 12 batch 1 is accepted through PR #246 / accepted source "
    f"`{ACCEPTED_SOURCE}` / squash merge `{MERGE_SHA}` / 37 of 37 applicable permanent "
    "workflows on one unchanged exact head. It moves Parties, Consents, Contact Points and "
    "Party Relationships exact mutation/query inventories and activation-gated contribution "
    "builders behind `crm-first-party-modules`, preserves the already aggregated Customer "
    "Accounts contribution, exact public coordinates and ordering, activation, authorization, "
    "Party-reference validation, persistence, workers, package count and external dependency "
    "versions, and removes their ordinary registration/inventory bypasses from generic native "
    "composition. The exact native-composition guard path is classified under the existing "
    "operations scope while unknown sibling scripts remain fail closed. Repository step 12 "
    "remains in progress; repository step 13 remains blocked."
)
NEXT_BATCH = (
    "Repository step 12 remains the current permitted implementation step. The next bounded "
    "packet must continue owner contribution aggregation after factual inventory review for "
    "Identity Resolution, Customer Data Operations, Data Quality, Customer Enrichment, "
    "Sales/Activities, Customer 360 and any other remaining active first-party owners."
)

PACKET = {
    "schema_version": "crm.repository-packet/v1",
    "packet_id": "repository-step-12-batch-1-evidence-sync",
    "title": "Synchronize accepted repository step 12 batch 1 evidence",
    "status": "active",
    "baseline": {"ref": "main", "sha": MERGE_SHA},
    "tracking_issues": [194],
    "objective": (
        "Synchronize accepted PR #246 repository-step-12 batch-1 evidence across the normative "
        "plans, record Parties, Consents, Contact Points and Party Relationships contribution "
        "aggregation as accepted, keep repository step 12 in progress, and expose the next "
        "bounded step-12 owner batch without authorizing repository step 13."
    ),
    "allowed_paths": [
        "docs/ACTIVE_PACKET.md",
        "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md",
        "docs/IMPLEMENTATION_ROADMAP.md",
        "docs/PHASE8_DELIVERY_PLAN.md",
        "docs/PROJECT_STATUS.md",
        "repository-packet.json",
        "tests/test_architecture_documentation_consistency.py",
        "tests/test_repository_navigation.py",
    ],
    "forbidden_paths": [
        ".github/workflows/**",
        "Cargo.lock",
        "Cargo.toml",
        "affected-scope-policy.json",
        "apps/**",
        "architecture-policy.json",
        "contracts/**",
        "crates/**",
        "database/**",
        "modules/**",
        "packages/**",
        "proto/**",
        "schemas/**",
        "scripts/**",
        "services/**",
    ],
    "deliverables": [
        "record PR #246 accepted source, squash merge and 37-of-37 applicable permanent workflow evidence",
        "record Parties, Consents, Contact Points and Party Relationships contribution aggregation batch 1 as accepted",
        "record preserved public inventories, ordering, activation, authorization, semantic validation, persistence, workers, package count and external dependency versions",
        "record exact native-composition guard affected-scope classification and unknown sibling fail-closed behavior",
        "keep repository step 12 in progress and identify the next bounded owner contribution batch",
        "regenerate docs/ACTIVE_PACKET.md and synchronize permanent documentation guards",
    ],
    "required_checks": [
        "Affected Scope CI",
        "Governance CI",
        "Rust CI",
        "Rust Generated Sync",
    ],
    "acceptance": [
        "all authoritative status documents agree on PR #246 accepted batch 1 evidence",
        "repository step 12 remains in progress and repository step 13 remains blocked",
        "the next permitted implementation packet is another bounded repository step 12 owner contribution batch",
        "generated active-packet navigation is fresh",
        "no runtime, workflow, contract, manifest, dependency, Cargo.lock, persistence, migration, public inventory, affected-scope policy or product behavior changes",
    ],
    "non_goals": [
        "complete repository step 12 or claim Stage D exit evidence",
        "implement Identity Resolution, Customer Data Operations, Data Quality, Customer Enrichment, Sales/Activities, Customer 360 or other remaining owner aggregation",
        "change runtime behavior, public coordinates, inventories, ordering, activation, authorization, persistence, contracts, schemas, migrations or workers",
        "change Cargo.lock, manifests, dependencies, crates or workspace packages",
        "change affected-scope policy or permanent workflows",
        "start repository step 13 or later work",
    ],
}


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path.cwd())
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--write", action="store_true")
    mode.add_argument("--check", action="store_true")
    return parser


def read(root: Path, path: str) -> str:
    return (root / path).read_text(encoding="utf-8")


def write(root: Path, path: str, text: str) -> None:
    (root / path).write_text(text, encoding="utf-8")


def replace_once(text: str, old: str, new: str, *, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise NavigationError(f"{label}: expected one replacement target, found {count}")
    return text.replace(old, new, 1)


def insert_after_paragraph(text: str, anchor: str, paragraph: str, *, label: str) -> str:
    if paragraph in text:
        return text
    start = text.find(anchor)
    if start < 0:
        raise NavigationError(f"{label}: anchor not found")
    end = text.find("\n\n", start)
    if end < 0:
        raise NavigationError(f"{label}: paragraph end not found")
    return text[:end] + "\n\n" + paragraph + text[end:]


def update_architecture_plan(root: Path) -> None:
    path = "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md"
    text = read(root, path)
    text = insert_after_paragraph(
        text,
        "Repository step 11 is accepted through PR #244",
        EVIDENCE,
        label=path,
    )
    text = replace_once(
        text,
        "The next permitted implementation packet is repository step 12: complete first-party contribution aggregation for all currently active owners without behavior changes.",
        NEXT_BATCH,
        label=f"{path} next packet",
    )
    text = replace_once(
        text,
        "the first bounded Customer Accounts registration-inventory aggregation is accepted through the first-party bundle",
        "the first bounded Customer Accounts registration-inventory aggregation is accepted through PR #222 and repository step 12 batch 1 for Parties, Consents, Contact Points and Party Relationships is accepted through PR #246",
        label=f"{path} stage D",
    )
    text = replace_once(
        text,
        "the inserted lockfile-preservation prerequisite before repository step 6 is complete through PR #232, repository step 6 is complete through PR #230, repository step 7 is complete through PR #235, repository step 8 is complete through PR #237, repository step 9 is complete through PR #239, repository step 10 is complete through PR #241, and repository step 11 is complete through PR #244. None changes the master numbering.",
        "the inserted lockfile-preservation prerequisite before repository step 6 is complete through PR #232, repository step 6 is complete through PR #230, repository step 7 is complete through PR #235, repository step 8 is complete through PR #237, repository step 9 is complete through PR #239, repository step 10 is complete through PR #241, repository step 11 is complete through PR #244, and repository step 12 batch 1 is complete through PR #246. None changes the master numbering; repository step 12 itself remains unfinished.",
        label=f"{path} completion ledger",
    )
    text = replace_once(
        text,
        "12. complete first-party contribution aggregation for all currently active owners without behavior changes — **Next**;",
        "12. complete first-party contribution aggregation for all currently active owners without behavior changes — **In progress; batch 1 complete through PR #246**;",
        label=f"{path} step 12",
    )
    write(root, path, text)


def update_roadmap(root: Path) -> None:
    path = "docs/IMPLEMENTATION_ROADMAP.md"
    text = read(root, path)
    text = insert_after_paragraph(text, "Repository step 11 is accepted through PR #244", EVIDENCE, label=path)
    text = replace_once(
        text,
        "Stage D contribution aggregation — **In progress; first bounded Customer Accounts registration-inventory aggregation accepted through PR #222; repository step 12 is the explicit completion step for all currently active first-party owners**.",
        "Stage D contribution aggregation — **In progress; Customer Accounts aggregation is accepted through PR #222 and repository step 12 batch 1 for Parties, Consents, Contact Points and Party Relationships is accepted through PR #246; remaining active first-party owners still require bounded batches**.",
        label=f"{path} stage D",
    )
    text = replace_once(
        text,
        "PR #239, PR #241 and PR #244.",
        "PR #239, PR #241, PR #244 and PR #246.",
        label=f"{path} evidence list",
    )
    text = replace_once(
        text,
        "Repository step 12 — complete first-party contribution aggregation for all currently active owners without behavior changes — is the current next packet.",
        NEXT_BATCH,
        label=f"{path} next packet",
    )
    text = replace_once(
        text,
        "| 12 — complete contribution aggregation — **Next** | D | B, E |",
        "| 12 — complete contribution aggregation — **In progress; batch 1 accepted through PR #246** | D | B, E |",
        label=f"{path} remaining table",
    )
    write(root, path, text)


def update_phase8(root: Path) -> None:
    path = "docs/PHASE8_DELIVERY_PLAN.md"
    text = read(root, path)
    text = insert_after_paragraph(text, "Repository step 11 is accepted through PR #244", EVIDENCE, label=path)
    text = replace_once(
        text,
        "Repository step 12 — complete first-party contribution aggregation for all currently active owners without behavior changes — is now the next permitted implementation packet. Repository step 13 or later work remains blocked until step 12 is accepted and its evidence is synchronized.",
        NEXT_BATCH + " Repository step 13 or later work remains blocked until repository step 12 is fully accepted and its final evidence is synchronized.",
        label=f"{path} current gate",
    )
    text = replace_once(
        text,
        "12. repository step 12 — complete first-party contribution aggregation for all currently active owners without behavior changes — **next**;",
        "12. repository step 12 — complete first-party contribution aggregation for all currently active owners without behavior changes — **in progress; batch 1 complete through PR #246**;",
        label=f"{path} step 12",
    )
    write(root, path, text)


def update_status(root: Path) -> None:
    path = "docs/PROJECT_STATUS.md"
    text = read(root, path)
    text = replace_once(
        text,
        "Latest accepted repository implementation packet is PR #244 / accepted source `405d2dbb97bb371b51cfb1d4ffb5549a57262878` / squash merge `4b08202fe9dd0c0df83567e24e6b9d86fb79c9db` / 34 of 34 applicable permanent workflows on one unchanged exact head.",
        f"Latest accepted repository implementation packet is PR #246 / accepted source `{ACCEPTED_SOURCE}` / squash merge `{MERGE_SHA}` / 37 of 37 applicable permanent workflows on one unchanged exact head. Repository step 12 remains in progress.",
        label=f"{path} latest packet",
    )
    text = insert_after_paragraph(text, "Repository step 11 is accepted through PR #244", "## Accepted repository step 12 batch 1\n\n" + EVIDENCE, label=path)
    text = replace_once(
        text,
        "Repository step 12 completes first-party contribution aggregation for all currently active owners without behavior changes.",
        NEXT_BATCH,
        label=f"{path} next packet",
    )
    text = replace_once(
        text,
        "Repository step 13 completes calibrated dependency, Rust public-surface, reverse-fan-out and exception governance, including removal of the three direct lint exceptions.",
        "Repository step 13 remains blocked until every repository step 12 owner batch and final Stage D exit evidence are accepted and synchronized. It will then complete calibrated dependency, Rust public-surface, reverse-fan-out and exception governance, including removal of the three direct lint exceptions.",
        label=f"{path} following packet",
    )
    text = replace_once(
        text,
        "the first bounded Customer Accounts registration-inventory aggregation is accepted through PR #222; repository step 12 now explicitly completes contribution entry points for all currently active first-party owners and removes remaining ordinary-registration concrete imports from generic runtime.",
        "Customer Accounts aggregation is accepted through PR #222 and repository step 12 batch 1 for Parties, Consents, Contact Points and Party Relationships is accepted through PR #246; remaining owner contribution entry points and ordinary-registration concrete imports still belong to later bounded step-12 batches.",
        label=f"{path} stage D",
    )
    text = replace_once(
        text,
        "-> 12. complete first-party contribution aggregation for all currently active owners — next",
        "-> 12. complete first-party contribution aggregation for all currently active owners — in progress; batch 1 complete through PR #246",
        label=f"{path} continuation",
    )
    write(root, path, text)


def update_architecture_test(root: Path) -> None:
    path = "tests/test_architecture_documentation_consistency.py"
    text = read(root, path)
    text = replace_once(
        text,
        '            "12. complete first-party contribution aggregation for all currently active owners without behavior changes — **Next**;",',
        '            "12. complete first-party contribution aggregation for all currently active owners without behavior changes — **In progress; batch 1 complete through PR #246**;",',
        label=f"{path} plan step",
    )
    text = replace_once(
        text,
        '            "12. repository step 12 — complete first-party contribution aggregation for all currently active owners without behavior changes — **next**;",',
        '            "12. repository step 12 — complete first-party contribution aggregation for all currently active owners without behavior changes — **in progress; batch 1 complete through PR #246**;",',
        label=f"{path} phase8 step",
    )
    evidence_anchor = '''            (\n                self.authoritative_status_documents,\n                "PR #244",\n                "405d2dbb97bb371b51cfb1d4ffb5549a57262878",\n                "4b08202fe9dd0c0df83567e24e6b9d86fb79c9db",\n                "34 of 34",\n            ),\n'''
    evidence_addition = evidence_anchor + f'''            (\n                self.authoritative_status_documents,\n                "PR #246",\n                "{ACCEPTED_SOURCE}",\n                "{MERGE_SHA}",\n                "37 of 37",\n            ),\n'''
    text = replace_once(text, evidence_anchor, evidence_addition, label=f"{path} evidence")
    function_pattern = re.compile(
        r"    def test_active_packet_is_machine_declared_and_generated\(self\) -> None:\n.*?(?=    def test_stage_accountability_and_live_catalog_are_current)",
        re.S,
    )
    replacement = f'''    def test_active_packet_is_machine_declared_and_generated(self) -> None:\n        self.assertEqual(self.packet["schema_version"], "crm.repository-packet/v1")\n        self.assertEqual(self.packet["packet_id"], "repository-step-12-batch-1-evidence-sync")\n        self.assertEqual(self.packet["status"], "active")\n        self.assertEqual(self.packet["baseline"]["ref"], "main")\n        self.assertEqual(self.packet["baseline"]["sha"], "{MERGE_SHA}")\n        self.assertEqual(self.packet["tracking_issues"], [194])\n        self.assertEqual(\n            self.packet["allowed_paths"],\n            [\n                "docs/ACTIVE_PACKET.md",\n                "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md",\n                "docs/IMPLEMENTATION_ROADMAP.md",\n                "docs/PHASE8_DELIVERY_PLAN.md",\n                "docs/PROJECT_STATUS.md",\n                "repository-packet.json",\n                "tests/test_architecture_documentation_consistency.py",\n                "tests/test_repository_navigation.py",\n            ],\n        )\n        for path in (\n            ".github/workflows/**",\n            "Cargo.lock",\n            "Cargo.toml",\n            "affected-scope-policy.json",\n            "apps/**",\n            "architecture-policy.json",\n            "contracts/**",\n            "crates/**",\n            "database/**",\n            "modules/**",\n            "packages/**",\n            "proto/**",\n            "schemas/**",\n            "scripts/**",\n            "services/**",\n        ):\n            self.assertIn(path, self.packet["forbidden_paths"])\n        self.assertEqual(\n            self.packet["required_checks"],\n            ["Affected Scope CI", "Governance CI", "Rust CI", "Rust Generated Sync"],\n        )\n        self.assertIn(\n            "repository step 12 remains in progress and repository step 13 remains blocked",\n            self.packet["acceptance"],\n        )\n        self.assertIn(\n            "complete repository step 12 or claim Stage D exit evidence",\n            self.packet["non_goals"],\n        )\n        self.assertIn("Generated by scripts/generate_repository_navigation.py", self.active_packet)\n        self.assertIn("repository-step-12-batch-1-evidence-sync", self.active_packet)\n        self.assertIn(self.packet["baseline"]["sha"], self.active_packet)\n        self.assertRegex(self.active_packet, r"sha256:[0-9a-f]{{64}}")\n        self.assertIn("orientation only", self.active_packet)\n        for document in self.authoritative_status_documents:\n            self.assertIn("PR #246", document)\n            self.assertIn("{ACCEPTED_SOURCE}", document)\n            self.assertIn("{MERGE_SHA}", document)\n            self.assertIn("37 of 37", document)\n            self.assertIn("repository step 12", document.lower())\n            self.assertIn("repository step 13 remains blocked", document.lower())\n\n'''
    text, count = function_pattern.subn(replacement, text, count=1)
    if count != 1:
        raise NavigationError(f"{path}: active packet test replacement count {count}")
    text = replace_once(
        text,
        '        self.assertIn("## Next permitted repository packet\\n\\nRepository step 12 completes first-party contribution aggregation", self.status)',
        '        self.assertIn("## Next permitted repository packet\\n\\nRepository step 12 remains the current permitted implementation step", self.status)',
        label=f"{path} next assertion",
    )
    write(root, path, text)


def update_navigation_test(root: Path) -> None:
    path = "tests/test_repository_navigation.py"
    text = read(root, path)
    pattern = re.compile(
        r"    def test_active_packet_declaration_is_valid_and_exact\(self\) -> None:\n.*?(?=    def test_affected_scope_workflow_executes_real_packet_check)",
        re.S,
    )
    replacement = f'''    def test_active_packet_declaration_is_valid_and_exact(self) -> None:\n        packet = load_packet(ROOT)\n        self.assertEqual(packet["packet_id"], "repository-step-12-batch-1-evidence-sync")\n        self.assertEqual(packet["status"], "active")\n        self.assertEqual(packet["baseline"]["ref"], "main")\n        self.assertEqual(packet["baseline"]["sha"], "{MERGE_SHA}")\n        self.assertEqual(packet["tracking_issues"], [194])\n        self.assertEqual(\n            packet["allowed_paths"],\n            [\n                "docs/ACTIVE_PACKET.md",\n                "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md",\n                "docs/IMPLEMENTATION_ROADMAP.md",\n                "docs/PHASE8_DELIVERY_PLAN.md",\n                "docs/PROJECT_STATUS.md",\n                "repository-packet.json",\n                "tests/test_architecture_documentation_consistency.py",\n                "tests/test_repository_navigation.py",\n            ],\n        )\n        for path in (\n            ".github/workflows/**",\n            "Cargo.lock",\n            "Cargo.toml",\n            "affected-scope-policy.json",\n            "apps/**",\n            "architecture-policy.json",\n            "contracts/**",\n            "crates/**",\n            "database/**",\n            "modules/**",\n            "packages/**",\n            "proto/**",\n            "schemas/**",\n            "scripts/**",\n            "services/**",\n        ):\n            self.assertIn(path, packet["forbidden_paths"])\n        self.assertEqual(\n            packet["required_checks"],\n            ["Affected Scope CI", "Governance CI", "Rust CI", "Rust Generated Sync"],\n        )\n        self.assertIn(\n            "repository step 12 remains in progress and repository step 13 remains blocked",\n            packet["acceptance"],\n        )\n        self.assertIn(\n            "complete repository step 12 or claim Stage D exit evidence",\n            packet["non_goals"],\n        )\n\n'''
    text, count = pattern.subn(replacement, text, count=1)
    if count != 1:
        raise NavigationError(f"{path}: packet test replacement count {count}")
    text = text.replace(
        '                    "043de0298ea9b3415e9894b4c5d69952856fd377"',
        f'                    "{MERGE_SHA}"',
        1,
    )
    write(root, path, text)


def materialize(root: Path) -> None:
    write(root, "repository-packet.json", json.dumps(PACKET, indent=2) + "\n")
    update_architecture_plan(root)
    update_roadmap(root)
    update_phase8(root)
    update_status(root)
    update_architecture_test(root)
    update_navigation_test(root)
    write_generated_documents(root)


def commit(root: Path) -> None:
    branch = os.environ.get("GITHUB_HEAD_REF") or os.environ.get("GITHUB_REF_NAME")
    if not branch:
        raise NavigationError("evidence materializer requires a branch ref")
    subprocess.run(
        [sys.executable, "-m", "unittest", "tests.test_architecture_documentation_consistency", "tests.test_repository_navigation"],
        cwd=root,
        check=True,
    )
    subprocess.run(["git", "config", "user.name", "github-actions[bot]"], cwd=root, check=True)
    subprocess.run(
        ["git", "config", "user.email", "41898282+github-actions[bot]@users.noreply.github.com"],
        cwd=root,
        check=True,
    )
    paths = PACKET["allowed_paths"]
    subprocess.run(["git", "add", *paths], cwd=root, check=True)
    status = subprocess.run(["git", "diff", "--cached", "--quiet"], cwd=root)
    if status.returncode == 0:
        return
    if status.returncode != 1:
        raise NavigationError("unable to inspect staged evidence changes")
    subprocess.run(
        ["git", "commit", "-m", "Synchronize repository step 12 batch 1 evidence"],
        cwd=root,
        check=True,
    )
    subprocess.run(["git", "push", "origin", f"HEAD:{branch}"], cwd=root, check=True)


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    try:
        if args.write:
            materialize(args.root)
            commit(args.root)
            return 0
        stale = stale_generated_documents(args.root)
    except (NavigationError, subprocess.CalledProcessError) as error:
        print(f"ERROR: {error}", file=sys.stderr)
        return 1
    if stale:
        for path in stale:
            print(f"STALE {path}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
