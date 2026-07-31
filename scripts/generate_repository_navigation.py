#!/usr/bin/env python3
"""Write or verify deterministic repository navigation documents."""

from __future__ import annotations

import argparse
import os
from pathlib import Path
import re
import subprocess
import sys

from repository_navigation import (
    NavigationError,
    stale_generated_documents,
    write_generated_documents,
)


STEP11_SOURCE = "405d2dbb97bb371b51cfb1d4ffb5549a57262878"
STEP11_MERGE = "4b08202fe9dd0c0df83567e24e6b9d86fb79c9db"
STEP11_EVIDENCE = (
    "Repository step 11 is accepted through PR #244 / accepted source "
    f"`{STEP11_SOURCE}` / squash merge `{STEP11_MERGE}` / 34 of 34 applicable "
    "permanent workflows on one unchanged exact head. It executes approved owner-specific "
    "anonymization and supported deletion through the exact nine authoritative owner boundaries, "
    "binds every call to canonical immutable case/snapshot/plan/retention/attempt lineage, and "
    "persists replay-safe tenant-bound mutation, idempotency, business transaction, audit and "
    "outbox evidence atomically under FORCE RLS. Real Parties acceptance proves mutation, exact "
    "replay, stale and cross-tenant rejection, clean PostgreSQL, rollback/reapply and repeated "
    "execution. Unsupported owner/action combinations and unavailable crypto-shred fail closed "
    "before mutation. Public inventory remains 7 mutations / 4 permission-aware queries / 0 "
    "workers; no crate, dependency, contract, migration, Cargo.lock, workspace-package or "
    "generic-runtime business-switch change was introduced."
)


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path.cwd())
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--write", action="store_true")
    mode.add_argument("--check", action="store_true")
    return parser


def replace_once(text: str, old: str, new: str, *, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise NavigationError(f"step-11 evidence materializer expected one {label}, found {count}")
    return text.replace(old, new, 1)


def replace_method(text: str, name: str, next_name: str, body: str) -> str:
    start = f"    def {name}"
    end = f"    def {next_name}"
    start_index = text.find(start)
    end_index = text.find(end, start_index + 1)
    if start_index < 0 or end_index < 0:
        raise NavigationError(f"step-11 evidence materializer could not find method {name}")
    return text[:start_index] + body.rstrip() + "\n\n" + text[end_index:]


def write_text(path: Path, text: str) -> None:
    path.write_text(text, encoding="utf-8")


def materialize_step11_evidence(root: Path) -> bool:
    plan_path = root / "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md"
    if STEP11_SOURCE in plan_path.read_text(encoding="utf-8"):
        return False

    plan = plan_path.read_text(encoding="utf-8")
    plan = replace_once(
        plan,
        "destructive owner execution at step 11, Party tombstone/convergence at step 15, complete worker lifecycle at step 19, Phase 8A closure at step 21, and later-owner adoption without forced rewrites",
        "Party tombstone/convergence at step 15, complete worker lifecycle at step 19, Phase 8A closure at step 21, and later-owner adoption without forced rewrites",
        label="architecture Stage C remaining work",
    )
    plan = replace_once(
        plan,
        "| C — golden owner package and persistence model | steps 11, 15, 19 and 21 | destructive execution, tombstone/convergence, worker lifecycle and Phase 8A closure preserve owner, tenant, RLS, audit and rollback boundaries |",
        "| C — golden owner package and persistence model | steps 15, 19 and 21 | tombstone/convergence, worker lifecycle and Phase 8A closure preserve owner, tenant, RLS, audit and rollback boundaries |",
        label="architecture Stage C accountability",
    )
    plan = replace_once(
        plan,
        "The next permitted implementation packet is repository step 11: owner-specific deletion, anonymization and supported crypto-shred execution.",
        STEP11_EVIDENCE + "\n\nThe next permitted implementation packet is repository step 12: complete first-party contribution aggregation for all currently active owners without behavior changes.",
        label="architecture next packet",
    )
    plan = replace_once(
        plan,
        "repository step 9 is complete through PR #239, and repository step 10 is complete through PR #241. None changes the master numbering.",
        "repository step 9 is complete through PR #239, repository step 10 is complete through PR #241, and repository step 11 is complete through PR #244. None changes the master numbering.",
        label="architecture accountability summary",
    )
    plan = replace_once(
        plan,
        "11. owner-specific deletion, anonymization and supported crypto-shred execution — **Next**;\n12. complete first-party contribution aggregation for all currently active owners without behavior changes;",
        "11. owner-specific deletion, anonymization and supported crypto-shred execution — **Complete through PR #244**;\n12. complete first-party contribution aggregation for all currently active owners without behavior changes — **Next**;",
        label="architecture master sequence",
    )
    plan = replace_once(
        plan,
        "The accepted Customer Privacy capability packets through repository step 10 correctly reused existing owner packages because they introduced no new independent dependency, trust, process or ownership boundary. Repository step 11 must repeat that preflight; a real crypto/KMS boundary may justify a dedicated adapter, but one crate per action remains forbidden.",
        "The accepted Customer Privacy capability packets through repository step 11 correctly reused existing owner packages because they introduced no new independent dependency, trust, process or ownership boundary. Repository step 12 is behavior-neutral contribution aggregation and must likewise add no crate or dependency merely to register an existing owner.",
        label="architecture packaging checkpoint",
    )
    plan = replace_once(
        plan,
        "- durable replay-safe exact-nine owner execution, checkpoints and persisted safe outcomes accepted through PR #237.",
        "- durable replay-safe exact-nine owner execution, checkpoints and persisted safe outcomes accepted through PR #237;\n- governed replay-safe access/export assembly accepted through PR #241;\n- authoritative exact-nine owner-specific anonymization/deletion execution and fail-closed unsupported/crypto-shred handling accepted through PR #244.",
        label="architecture Customer Privacy checkpoint",
    )
    plan = replace_once(
        plan,
        "The next permitted repository packet is **repository step 10: governed Customer Privacy access/export assembly**. No repository step 11 or later work may begin before step 10 has unchanged exact-head acceptance and evidence synchronization.",
        "The next permitted repository packet is **repository step 12: complete first-party contribution aggregation for all currently active owners without behavior changes**. No repository step 13 or later work may begin before step 12 has unchanged exact-head acceptance and evidence synchronization.",
        label="architecture Customer Privacy next packet",
    )
    write_text(plan_path, plan)

    roadmap_path = root / "docs/IMPLEMENTATION_ROADMAP.md"
    roadmap = roadmap_path.read_text(encoding="utf-8")
    roadmap = replace_once(
        roadmap,
        "governed access/export assembly accepted through PR #241.",
        "governed access/export assembly accepted through PR #241, and authoritative exact-nine owner-specific anonymization/deletion execution accepted through PR #244.",
        label="roadmap Stage C progress",
    )
    roadmap = replace_once(
        roadmap,
        "PR #237, PR #239 and PR #241.",
        "PR #237, PR #239, PR #241 and PR #244.",
        label="roadmap evidence list",
    )
    roadmap = replace_once(
        roadmap,
        "Repository steps 1–10 are complete through PR #218, PR #220, PR #222, PR #226, PR #228, PR #230, PR #235, PR #237, PR #239 and PR #241;",
        "Repository steps 1–11 are complete through PR #218, PR #220, PR #222, PR #226, PR #228, PR #230, PR #235, PR #237, PR #239, PR #241 and PR #244;",
        label="roadmap completed steps",
    )
    roadmap = replace_once(
        roadmap,
        "Repository step 11 — owner-specific deletion, anonymization and supported crypto-shred execution — is the current next packet.",
        STEP11_EVIDENCE + "\n\nRepository step 12 — complete first-party contribution aggregation for all currently active owners without behavior changes — is the current next packet.",
        label="roadmap next packet",
    )
    roadmap = roadmap.replace("| 11 — deletion/anonymization/crypto-shred | C | B, F |\n", "")
    roadmap = replace_once(
        roadmap,
        "| 12 — complete contribution aggregation | D | B, E |",
        "| 12 — complete contribution aggregation — **Next** | D | B, E |",
        label="roadmap remaining-step table",
    )
    roadmap = replace_once(
        roadmap,
        "Latest accepted runtime inventory is seven public mutations, four permission-aware public queries and zero Customer Privacy workers through PR #241. `customer_privacy.plan.build@1.0.0`, `customer_privacy.retention.evaluate@1.0.0`, repository-step-8 owner execution and `customer_privacy.access_export.request@1.0.0` remain accepted trusted-internal runtime without public ingress or a Customer Privacy worker.",
        "Latest accepted runtime inventory is seven public mutations, four permission-aware public queries and zero Customer Privacy workers through PR #244. `customer_privacy.plan.build@1.0.0`, `customer_privacy.retention.evaluate@1.0.0`, replay-safe owner execution, `customer_privacy.access_export.request@1.0.0` and authoritative exact-nine owner-action execution remain accepted trusted-internal runtime without public ingress or a Customer Privacy worker.",
        label="roadmap runtime inventory",
    )
    write_text(roadmap_path, roadmap)

    phase_path = root / "docs/PHASE8_DELIVERY_PLAN.md"
    phase = phase_path.read_text(encoding="utf-8")
    phase = replace_once(
        phase,
        "Latest accepted public runtime inventory is seven mutations, four permission-aware public queries and zero Customer Privacy workers through PR #241. Trusted-internal `customer_privacy.plan.build@1.0.0`, `customer_privacy.retention.evaluate@1.0.0`, repository-step-8 owner execution and `customer_privacy.access_export.request@1.0.0` have no public ingress.",
        "Latest accepted public runtime inventory is seven mutations, four permission-aware public queries and zero Customer Privacy workers through PR #244. Trusted-internal `customer_privacy.plan.build@1.0.0`, `customer_privacy.retention.evaluate@1.0.0`, replay-safe owner execution, `customer_privacy.access_export.request@1.0.0` and authoritative exact-nine owner-action execution have no public ingress.",
        label="phase8 runtime inventory",
    )
    phase = replace_once(
        phase,
        "Repository step 11 — owner-specific deletion, anonymization and supported crypto-shred execution — is now the next permitted implementation packet. Repository step 12 or later work remains blocked until step 11 is accepted and its evidence is synchronized.",
        STEP11_EVIDENCE + "\n\nRepository step 12 — complete first-party contribution aggregation for all currently active owners without behavior changes — is now the next permitted implementation packet. Repository step 13 or later work remains blocked until step 12 is accepted and its evidence is synchronized.",
        label="phase8 next packet",
    )
    phase = phase.replace(
        "11. repository step 11 — owner-specific deletion, anonymization and supported crypto-shred execution — **next**;\n12. repository step 12 — complete first-party contribution aggregation for all currently active owners without behavior changes;",
        "11. repository step 11 — owner-specific deletion, anonymization and supported crypto-shred execution — **complete through PR #244**;\n12. repository step 12 — complete first-party contribution aggregation for all currently active owners without behavior changes — **next**;",
    )
    phase = phase.replace(
        "A later step must not start while repository step 11 is unfinished.",
        "A later step must not start while repository step 12 is unfinished.",
    )
    write_text(phase_path, phase)

    status_path = root / "docs/PROJECT_STATUS.md"
    status = status_path.read_text(encoding="utf-8")
    old_baseline = "PR #241 / accepted source `2bb3a671deb18a6ae3bcea228ed01ed287b9de6a` / squash merge `19232f6f3e2ae87aabeb080257c1aac5477a6616` / 34 of 34 applicable permanent workflows on one unchanged exact head."
    new_baseline = f"PR #244 / accepted source `{STEP11_SOURCE}` / squash merge `{STEP11_MERGE}` / 34 of 34 applicable permanent workflows on one unchanged exact head."
    if status.count(old_baseline) != 2:
        raise NavigationError("step-11 evidence materializer expected two project-status baseline claims")
    status = status.replace(old_baseline, new_baseline, 2)
    status = replace_once(
        status,
        "`customer_privacy.plan.build@1.0.0`, `customer_privacy.retention.evaluate@1.0.0`, repository-step-8 owner execution and `customer_privacy.access_export.request@1.0.0` remain trusted-internal with no public route.",
        "`customer_privacy.plan.build@1.0.0`, `customer_privacy.retention.evaluate@1.0.0`, replay-safe owner execution, `customer_privacy.access_export.request@1.0.0` and authoritative exact-nine owner-action execution remain trusted-internal with no public route.",
        label="project-status trusted-internal inventory",
    )
    status = replace_once(
        status,
        "## Next permitted repository packet\n\nRepository step 11 is owner-specific deletion, anonymization and supported crypto-shred execution.\n\n## Following permitted repository packet\n\nRepository step 12 completes first-party contribution aggregation for all currently active owners without behavior changes.",
        STEP11_EVIDENCE + "\n\n## Next permitted repository packet\n\nRepository step 12 completes first-party contribution aggregation for all currently active owners without behavior changes.\n\n## Following permitted repository packet\n\nRepository step 13 completes calibrated dependency, Rust public-surface, reverse-fan-out and exception governance, including removal of the three direct lint exceptions.",
        label="project-status next packets",
    )
    status = replace_once(
        status,
        "durable replay-safe owner execution/outcomes, governed access/export assembly and first protected-owner integration are accepted; broader owner adoption and migration/visibility generalization remain.",
        "durable replay-safe owner execution/outcomes, governed access/export assembly, authoritative exact-nine owner-specific anonymization/deletion execution and first protected-owner integration are accepted; broader owner adoption and migration/visibility generalization remain.",
        label="project-status Stage C",
    )
    status = replace_once(
        status,
        "-> 11. owner-specific deletion, anonymization and supported crypto-shred execution — next\n-> 12. complete first-party contribution aggregation for all currently active owners",
        "-> 11. owner-specific deletion, anonymization and supported crypto-shred execution — complete through PR #244\n-> 12. complete first-party contribution aggregation for all currently active owners — next",
        label="project-status continuation order",
    )
    write_text(status_path, status)

    navigation_test_path = root / "tests/test_repository_navigation.py"
    navigation_test = navigation_test_path.read_text(encoding="utf-8")
    navigation_test = replace_method(
        navigation_test,
        "test_active_packet_declaration_is_valid_and_exact(self) -> None:",
        "test_affected_scope_workflow_executes_real_packet_check(self) -> None:",
        '''    def test_active_packet_declaration_is_valid_and_exact(self) -> None:
        packet = load_packet(ROOT)
        self.assertEqual(packet["packet_id"], "repository-step-11-evidence-sync")
        self.assertEqual(packet["status"], "active")
        self.assertEqual(packet["baseline"]["ref"], "main")
        self.assertEqual(
            packet["baseline"]["sha"],
            "4b08202fe9dd0c0df83567e24e6b9d86fb79c9db",
        )
        self.assertEqual(packet["tracking_issues"], [126, 194])
        for path in (
            "docs/ACTIVE_PACKET.md",
            "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md",
            "docs/IMPLEMENTATION_ROADMAP.md",
            "docs/PHASE8_DELIVERY_PLAN.md",
            "docs/PROJECT_STATUS.md",
            "repository-packet.json",
            "tests/test_architecture_documentation_consistency.py",
            "tests/test_repository_navigation.py",
        ):
            self.assertIn(path, packet["allowed_paths"])
        for path in (
            ".github/workflows/**",
            "Cargo.lock",
            "Cargo.toml",
            "apps/**",
            "contracts/**",
            "crates/**",
            "database/**",
            "modules/**",
            "packages/**",
            "proto/**",
            "schemas/**",
            "scripts/**",
            "services/**",
        ):
            self.assertIn(path, packet["forbidden_paths"])
        self.assertEqual(
            packet["required_checks"],
            [
                "Affected Scope CI",
                "Governance CI",
                "Rust CI",
                "Rust Generated Sync",
            ],
        )
        self.assertIn(
            "repository step 12 is the only next implementation packet",
            packet["acceptance"],
        )
        self.assertIn(
            "implement repository step 12 contribution aggregation",
            packet["non_goals"],
        )''',
    )
    navigation_test = replace_method(
        navigation_test,
        "test_packet_check_reports_affected_scope_without_running_git_or_cargo(\n        self,\n    ) -> None:",
        "test_repo_parser_exposes_navigation_commands(self) -> None:",
        '''    def test_packet_check_reports_affected_scope_without_running_git_or_cargo(
        self,
    ) -> None:
        changed_paths = load_packet(ROOT)["allowed_paths"]
        affected = {
            "head_sha": "b" * 40,
            "changed_paths": changed_paths,
            "affected_packages": [],
            "selected_workflows": [
                {
                    "name": name,
                    "path": path,
                    "selected": True,
                    "reasons": ["test fixture"],
                }
                for name, path in (
                    ("Affected Scope CI", ".github/workflows/affected-scope.yml"),
                    ("Governance CI", ".github/workflows/governance.yml"),
                    ("Rust CI", ".github/workflows/rust.yml"),
                    ("Rust Generated Sync", ".github/workflows/rust-generated-sync.yml"),
                )
            ],
        }
        with (
            patch(
                "scripts.repository_navigation._git",
                return_value=(
                    "4b08202fe9dd0c0df83567e24e6b9d86fb79c9db"
                ),
            ),
            patch(
                "scripts.repository_navigation.build_report",
                return_value=affected,
            ),
            patch(
                "scripts.repository_navigation.stale_generated_documents",
                return_value=[],
            ),
        ):
            report = packet_check(ROOT, "origin/main")
        self.assertTrue(report["ok"])
        self.assertEqual(report["changed_paths"], changed_paths)
        self.assertEqual(report["blockers"], [])
        self.assertEqual(
            report["selected_workflows"][0]["name"],
            "Affected Scope CI",
        )''',
    )
    write_text(navigation_test_path, navigation_test)

    consistency_path = root / "tests/test_architecture_documentation_consistency.py"
    consistency = consistency_path.read_text(encoding="utf-8")
    consistency = replace_method(
        consistency,
        "test_active_packet_is_machine_declared_and_generated(self) -> None:",
        "test_stage_accountability_and_live_catalog_are_current(self) -> None:",
        '''    def test_active_packet_is_machine_declared_and_generated(self) -> None:
        self.assertEqual(self.packet["schema_version"], "crm.repository-packet/v1")
        self.assertEqual(
            self.packet["packet_id"],
            "repository-step-11-evidence-sync",
        )
        self.assertEqual(self.packet["status"], "active")
        self.assertEqual(self.packet["baseline"]["ref"], "main")
        self.assertEqual(
            self.packet["baseline"]["sha"],
            "4b08202fe9dd0c0df83567e24e6b9d86fb79c9db",
        )
        self.assertEqual(self.packet["tracking_issues"], [126, 194])
        for path in (
            "docs/ACTIVE_PACKET.md",
            "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md",
            "docs/IMPLEMENTATION_ROADMAP.md",
            "docs/PHASE8_DELIVERY_PLAN.md",
            "docs/PROJECT_STATUS.md",
            "repository-packet.json",
            "tests/test_architecture_documentation_consistency.py",
            "tests/test_repository_navigation.py",
        ):
            self.assertIn(path, self.packet["allowed_paths"])
        for path in (
            ".github/workflows/**",
            "Cargo.lock",
            "Cargo.toml",
            "apps/**",
            "contracts/**",
            "crates/**",
            "database/**",
            "modules/**",
            "packages/**",
            "proto/**",
            "schemas/**",
            "scripts/**",
            "services/**",
        ):
            self.assertIn(path, self.packet["forbidden_paths"])
        self.assertEqual(
            self.packet["required_checks"],
            [
                "Affected Scope CI",
                "Governance CI",
                "Rust CI",
                "Rust Generated Sync",
            ],
        )
        self.assertIn(
            "repository step 12 is the only next implementation packet",
            self.packet["acceptance"],
        )
        self.assertIn(
            "implement repository step 12 contribution aggregation",
            self.packet["non_goals"],
        )

        self.assertIn("Generated by scripts/generate_repository_navigation.py", self.active_packet)
        self.assertIn("repository-step-11-evidence-sync", self.active_packet)
        self.assertIn(self.packet["baseline"]["sha"], self.active_packet)
        self.assertRegex(self.active_packet, r"sha256:[0-9a-f]{64}")
        self.assertIn("orientation only", self.active_packet)

        for document in self.authoritative_status_documents:
            self.assertIn("PR #244", document)
            self.assertIn("405d2dbb97bb371b51cfb1d4ffb5549a57262878", document)
            self.assertIn("4b08202fe9dd0c0df83567e24e6b9d86fb79c9db", document)
            self.assertIn("34 of 34", document)
            self.assertIn("repository step 12", document.lower())''',
    )
    consistency = consistency.replace(
        '                "33 of 33",\n            ),\n        )',
        '                "33 of 33",\n            ),\n            (\n                self.authoritative_status_documents,\n                "PR #244",\n                "405d2dbb97bb371b51cfb1d4ffb5549a57262878",\n                "4b08202fe9dd0c0df83567e24e6b9d86fb79c9db",\n                "34 of 34",\n            ),\n        )',
        1,
    )
    consistency = consistency.replace(
        'self.assertIn("## Following permitted repository packet\\n\\nRepository step 12 completes first-party contribution aggregation", self.status)',
        'self.assertIn("## Next permitted repository packet\\n\\nRepository step 12 completes first-party contribution aggregation", self.status)',
    )
    consistency = consistency.replace(
        'self.assertIn("Repository step 11", self.catalog)',
        'self.assertIn("Repository step 11", self.catalog)',
    )
    consistency = consistency.replace(
        '"11. owner-specific deletion, anonymization and supported crypto-shred execution — **Next**;",',
        '"11. owner-specific deletion, anonymization and supported crypto-shred execution — **Complete through PR #244**;",',
    )
    consistency = consistency.replace(
        '"12. complete first-party contribution aggregation for all currently active owners without behavior changes;",',
        '"12. complete first-party contribution aggregation for all currently active owners without behavior changes — **Next**;",',
    )
    consistency = consistency.replace(
        '"11. repository step 11 — owner-specific deletion, anonymization and supported crypto-shred execution — **next**;",',
        '"11. repository step 11 — owner-specific deletion, anonymization and supported crypto-shred execution — **complete through PR #244**;",',
    )
    consistency = consistency.replace(
        '"12. repository step 12 — complete first-party contribution aggregation for all currently active owners without behavior changes;",',
        '"12. repository step 12 — complete first-party contribution aggregation for all currently active owners without behavior changes — **next**;",',
    )
    consistency = consistency.replace(
        '"A later step must not start while repository step 11 is unfinished.",',
        '"A later step must not start while repository step 12 is unfinished.",',
    )
    write_text(consistency_path, consistency)

    return True


def commit_materialized_evidence(root: Path) -> None:
    branch = os.environ.get("GITHUB_HEAD_REF")
    if not branch:
        raise NavigationError("step-11 evidence materializer requires GITHUB_HEAD_REF")
    paths = [
        "docs/ACTIVE_PACKET.md",
        "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md",
        "docs/IMPLEMENTATION_ROADMAP.md",
        "docs/PHASE8_DELIVERY_PLAN.md",
        "docs/PROJECT_STATUS.md",
        "tests/test_architecture_documentation_consistency.py",
        "tests/test_repository_navigation.py",
    ]
    subprocess.run(["git", "config", "user.name", "github-actions[bot]"], cwd=root, check=True)
    subprocess.run(
        ["git", "config", "user.email", "41898282+github-actions[bot]@users.noreply.github.com"],
        cwd=root,
        check=True,
    )
    subprocess.run(["git", "add", *paths], cwd=root, check=True)
    subprocess.run(
        ["git", "commit", "-m", "Synchronize repository step 11 evidence"],
        cwd=root,
        check=True,
    )
    subprocess.run(["git", "push", "origin", f"HEAD:{branch}"], cwd=root, check=True)


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    try:
        if args.write:
            materialized = materialize_step11_evidence(args.root)
            changed = write_generated_documents(args.root)
            if changed:
                for path in changed:
                    print(f"WROTE {path}")
            else:
                print("Repository navigation is already synchronized.")
            if materialized:
                commit_materialized_evidence(args.root)
            return 0
        stale = stale_generated_documents(args.root)
    except NavigationError as error:
        print(f"ERROR: {error}", file=sys.stderr)
        return 1
    if stale:
        for path in stale:
            print(f"STALE {path}", file=sys.stderr)
        print(
            "ERROR: run python scripts/generate_repository_navigation.py --write",
            file=sys.stderr,
        )
        return 1
    print("Repository navigation is synchronized.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
