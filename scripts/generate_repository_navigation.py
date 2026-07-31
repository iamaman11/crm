#!/usr/bin/env python3
"""Temporarily synchronize repository-step-12 batch-2 packet guards.

The canonical generator is restored before exact-head acceptance. The permanent
``--write`` and ``--check`` interface remains intact during materialization.
"""

from __future__ import annotations

import argparse
import os
from pathlib import Path
import subprocess
import sys

from repository_navigation import (
    NavigationError,
    stale_generated_documents,
    write_generated_documents,
)


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path.cwd())
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--write", action="store_true")
    mode.add_argument("--check", action="store_true")
    return parser


def replace_method(
    root: Path,
    relative: str,
    method_name: str,
    next_method_name: str,
    replacement: str,
) -> None:
    path = root / relative
    content = path.read_text(encoding="utf-8")
    start_marker = f"    def {method_name}("
    end_marker = f"    def {next_method_name}("
    start = content.find(start_marker)
    end = content.find(end_marker, start + len(start_marker))
    if start < 0 or end < 0:
        raise RuntimeError(f"{relative}: packet guard method anchors are missing")
    updated = content[:start] + replacement.rstrip() + "\n\n" + content[end:]
    path.write_text(updated, encoding="utf-8")


def materialize_packet_guards(root: Path) -> None:
    expected_paths = [
        "Cargo.lock",
        "crates/crm-application-runtime/Cargo.toml",
        "crates/crm-application-runtime/src/data_quality_capability_execution.rs",
        "crates/crm-application-runtime/src/data_quality_registration.rs",
        "crates/crm-application-runtime/src/lib.rs",
        "crates/crm-application-runtime/src/native_composition.rs",
        "crates/crm-customer-data-operations-execution-composition/Cargo.toml",
        "crates/crm-customer-data-operations-execution-composition/src/lib.rs",
        "crates/crm-customer-data-operations-execution-composition/src/production_contribution.rs",
        "crates/crm-data-quality-source-composition/Cargo.toml",
        "crates/crm-data-quality-source-composition/src/capability_execution.rs",
        "crates/crm-data-quality-source-composition/src/lib.rs",
        "crates/crm-data-quality-source-composition/src/production_contribution.rs",
        "crates/crm-data-quality-source-composition/src/registration.rs",
        "crates/crm-first-party-modules/Cargo.toml",
        "crates/crm-first-party-modules/src/lib.rs",
        "crates/crm-identity-resolution-capability-composition/Cargo.toml",
        "crates/crm-identity-resolution-capability-composition/src/lib.rs",
        "crates/crm-identity-resolution-capability-composition/src/production_contribution.rs",
        "docs/ACTIVE_PACKET.md",
        "docs/generated/REPOSITORY_MAP.md",
        "repository-packet.json",
        "scripts/check_native_module_composition.py",
        "scripts/generate_repository_navigation.py",
        "tests/test_architecture_documentation_consistency.py",
        "tests/test_native_module_composition.py",
        "tests/test_repository_navigation.py",
    ]
    expected_paths_literal = repr(expected_paths)

    architecture_method = f'''    def test_active_packet_is_machine_declared_and_generated(self) -> None:
        self.assertEqual(self.packet["schema_version"], "crm.repository-packet/v1")
        self.assertEqual(
            self.packet["packet_id"],
            "repository-step-12-contribution-aggregation-batch-2",
        )
        self.assertEqual(self.packet["status"], "active")
        self.assertEqual(self.packet["baseline"]["ref"], "main")
        self.assertEqual(
            self.packet["baseline"]["sha"],
            "10be9a128ed1f8fbc6967d82baba648ba52f1d12",
        )
        self.assertEqual(self.packet["tracking_issues"], [194])
        self.assertEqual(self.packet["allowed_paths"], {expected_paths_literal})
        for path in (
            ".github/workflows/**",
            "affected-scope-policy.json",
            "apps/**",
            "architecture-policy.json",
            "contracts/**",
            "database/**",
            "modules/**",
            "packages/**",
            "proto/**",
            "schemas/**",
            "services/**",
        ):
            self.assertIn(path, self.packet["forbidden_paths"])
        self.assertEqual(
            self.packet["required_checks"],
            ["Affected Scope CI", "Governance CI", "Rust CI", "Rust Generated Sync"],
        )
        self.assertIn(
            "repository step 12 remains in progress and repository step 13 is not started",
            self.packet["acceptance"],
        )
        self.assertIn(
            "complete repository step 12 for Customer Enrichment, Sales/Activities, Customer 360 or any other remaining owner in this batch",
            self.packet["non_goals"],
        )
        self.assertIn("Generated by scripts/generate_repository_navigation.py", self.active_packet)
        self.assertIn(
            "repository-step-12-contribution-aggregation-batch-2",
            self.active_packet,
        )
        self.assertIn(self.packet["baseline"]["sha"], self.active_packet)
        self.assertRegex(self.active_packet, r"sha256:[0-9a-f]{{64}}")
        self.assertIn("orientation only", self.active_packet)
        for document in self.authoritative_status_documents:
            self.assertIn("PR #246", document)
            self.assertIn("3b4fe7cdf458daac9c12f816d0d6a87039e613f3", document)
            self.assertIn("f090fa8785ac2bbb7e5cf186b4c5011cb9aeb978", document)
            self.assertIn("37 of 37", document)
            self.assertIn("repository step 12", document.lower())
            self.assertIn("repository step 13 remains blocked", document.lower())'''
    replace_method(
        root,
        "tests/test_architecture_documentation_consistency.py",
        "test_active_packet_is_machine_declared_and_generated",
        "test_stage_accountability_and_live_catalog_are_current",
        architecture_method,
    )

    repository_method = f'''    def test_active_packet_declaration_is_valid_and_exact(self) -> None:
        packet = load_packet(ROOT)
        self.assertEqual(
            packet["packet_id"],
            "repository-step-12-contribution-aggregation-batch-2",
        )
        self.assertEqual(packet["status"], "active")
        self.assertEqual(packet["baseline"]["ref"], "main")
        self.assertEqual(
            packet["baseline"]["sha"],
            "10be9a128ed1f8fbc6967d82baba648ba52f1d12",
        )
        self.assertEqual(packet["tracking_issues"], [194])
        self.assertEqual(packet["allowed_paths"], {expected_paths_literal})
        for path in (
            ".github/workflows/**",
            "affected-scope-policy.json",
            "apps/**",
            "architecture-policy.json",
            "contracts/**",
            "database/**",
            "modules/**",
            "packages/**",
            "proto/**",
            "schemas/**",
            "services/**",
        ):
            self.assertIn(path, packet["forbidden_paths"])
        self.assertEqual(
            packet["required_checks"],
            ["Affected Scope CI", "Governance CI", "Rust CI", "Rust Generated Sync"],
        )
        self.assertIn(
            "repository step 12 remains in progress and repository step 13 is not started",
            packet["acceptance"],
        )
        self.assertIn(
            "complete repository step 12 for Customer Enrichment, Sales/Activities, Customer 360 or any other remaining owner in this batch",
            packet["non_goals"],
        )'''
    replace_method(
        root,
        "tests/test_repository_navigation.py",
        "test_active_packet_declaration_is_valid_and_exact",
        "test_affected_scope_workflow_executes_real_packet_check",
        repository_method,
    )

    navigation_test = root / "tests/test_repository_navigation.py"
    content = navigation_test.read_text(encoding="utf-8")
    content = content.replace(
        '"f090fa8785ac2bbb7e5cf186b4c5011cb9aeb978"',
        '"10be9a128ed1f8fbc6967d82baba648ba52f1d12"',
    )
    navigation_test.write_text(content, encoding="utf-8")


def commit_materialization(root: Path) -> None:
    if os.environ.get("GITHUB_ACTIONS") != "true":
        return
    status = subprocess.run(
        ["git", "status", "--porcelain"],
        cwd=root,
        check=True,
        capture_output=True,
        text=True,
    ).stdout
    if not status.strip():
        return
    branch = os.environ.get("GITHUB_HEAD_REF")
    if not branch:
        raise RuntimeError("GITHUB_HEAD_REF is unavailable")
    subprocess.run(
        ["git", "config", "user.name", "github-actions[bot]"],
        cwd=root,
        check=True,
    )
    subprocess.run(
        [
            "git",
            "config",
            "user.email",
            "41898282+github-actions[bot]@users.noreply.github.com",
        ],
        cwd=root,
        check=True,
    )
    subprocess.run(
        [
            "git",
            "add",
            "docs/ACTIVE_PACKET.md",
            "docs/generated/REPOSITORY_MAP.md",
            "tests/test_architecture_documentation_consistency.py",
            "tests/test_repository_navigation.py",
        ],
        cwd=root,
        check=True,
    )
    subprocess.run(
        ["git", "commit", "-m", "Synchronize batch 2 packet consistency guards"],
        cwd=root,
        check=True,
    )
    subprocess.run(
        ["git", "push", "origin", f"HEAD:{branch}"],
        cwd=root,
        check=True,
    )


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    root = args.root.resolve()
    try:
        if args.write:
            materialize_packet_guards(root)
            write_generated_documents(root)
            commit_materialization(root)
            print("Batch 2 packet guards and repository navigation are synchronized.")
            return 0
        stale = stale_generated_documents(root)
    except (NavigationError, OSError, RuntimeError, subprocess.CalledProcessError) as error:
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
