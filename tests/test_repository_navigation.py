from __future__ import annotations

from pathlib import Path
import unittest
from unittest.mock import patch

from scripts.repository_navigation import (
    ACTIVE_PACKET_PATH,
    NAVIGATION_SCHEMA,
    REPOSITORY_MAP_PATH,
    NavigationError,
    evaluate_path_policy,
    explain_target,
    generated_documents,
    load_packet,
    packet_check,
    stale_generated_documents,
)
from scripts.repo import build_parser


ROOT = Path(__file__).resolve().parents[1]


class RepositoryNavigationTests(unittest.TestCase):
    def test_active_packet_declaration_is_valid_and_exact(self) -> None:
        packet = load_packet(ROOT)
        self.assertEqual(
            packet["packet_id"],
            "repository-step-6-generated-sync-prerequisite",
        )
        self.assertEqual(packet["status"], "active")
        self.assertEqual(packet["baseline"]["ref"], "main")
        self.assertEqual(
            packet["baseline"]["sha"],
            "59ab9242a5d38aa143313c120d3e076adad9b851",
        )
        self.assertEqual(packet["tracking_issues"], [194, 231])
        self.assertIn(
            ".github/workflows/rust-generated-sync.yml", packet["allowed_paths"]
        )
        self.assertIn(".github/workflows/rust.yml", packet["allowed_paths"])
        self.assertIn("docs/ACTIVE_PACKET.md", packet["allowed_paths"])
        self.assertIn(
            "tests/test_architecture_documentation_consistency.py",
            packet["allowed_paths"],
        )
        self.assertIn("tests/test_repository_navigation.py", packet["allowed_paths"])
        self.assertIn("Cargo.lock", packet["forbidden_paths"])
        self.assertIn("Rust CI", packet["required_checks"])
        self.assertIn(
            "Cargo.lock remains byte-identical throughout Rust Generated Sync and Rust CI",
            packet["acceptance"],
        )
        self.assertIn(
            "repository step 6 remains blocked and unchanged until this prerequisite is accepted and evidence-synchronized",
            packet["acceptance"],
        )

    def test_affected_scope_workflow_executes_real_packet_check(self) -> None:
        workflow = (ROOT / ".github/workflows/affected-scope.yml").read_text(
            encoding="utf-8"
        )
        self.assertIn("Validate active repository packet", workflow)
        self.assertIn(
            'python scripts/repo.py packet-check --base "${BASE_REF}"', workflow
        )
        self.assertLess(
            workflow.index("Validate active repository packet"),
            workflow.index("Run affected structural preflight"),
        )

    def test_rust_workflows_preserve_the_committed_lockfile(self) -> None:
        generated_sync = (
            ROOT / ".github/workflows/rust-generated-sync.yml"
        ).read_text(encoding="utf-8")
        rust_ci = (ROOT / ".github/workflows/rust.yml").read_text(encoding="utf-8")
        repo_runner = (ROOT / "scripts/repo.py").read_text(encoding="utf-8")

        for workflow in (generated_sync, rust_ci):
            self.assertNotIn("cargo generate-lockfile", workflow)
            self.assertIn(
                "cargo metadata --locked --format-version 1 --no-deps", workflow
            )
            self.assertIn("lockfile_before=", workflow)
            self.assertIn("lockfile_after=", workflow)
            self.assertIn("git diff --exit-code -- Cargo.lock", workflow)

        self.assertNotIn("git add Cargo.lock", generated_sync)
        self.assertIn(
            "cargo run --locked -p crm-proto-contracts", generated_sync
        )
        self.assertIn("cargo clippy --locked --fix", generated_sync)
        self.assertIn("cargo clippy --locked --workspace", generated_sync)

        self.assertNotIn("Upload resolved Cargo lockfile", rust_ci)
        self.assertNotIn("name: cargo-lockfile", rust_ci)
        self.assertIn("cargo check --locked --workspace", rust_ci)
        self.assertIn("cargo clippy --locked --workspace", rust_ci)
        self.assertIn("cargo test --locked --workspace", rust_ci)

        self.assertIn('run(["cargo", "generate-lockfile"])', repo_runner)
        self.assertIn('"lock", help="regenerate the committed Cargo lockfile"', repo_runner)

    def test_module_explanation_traces_customer_privacy_owner(self) -> None:
        explanation = explain_target(ROOT, "crm.customer-privacy")
        self.assertEqual(explanation["schema_version"], NAVIGATION_SCHEMA)
        self.assertEqual(explanation["kind"], "module")
        self.assertEqual(explanation["version"], "0.2.0")
        self.assertEqual(explanation["owner"]["team"], "customer-platform")
        self.assertEqual(
            explanation["manifest_path"], "modules/crm-customer-privacy/module.yaml"
        )
        coordinates = {
            capability["coordinate"] for capability in explanation["capabilities"]
        }
        self.assertIn("customer_privacy.case.submit@1.0.0", coordinates)
        self.assertIn("customer_privacy.restriction.place@1.0.0", coordinates)
        self.assertTrue(explanation["references"])

    def test_capability_explanation_resolves_exact_binding_and_runtime(self) -> None:
        explanation = explain_target(
            ROOT, "customer_privacy.case.submit@1.0.0"
        )
        self.assertEqual(explanation["kind"], "capability")
        self.assertEqual(explanation["owner_module_id"], "crm.customer-privacy")
        self.assertEqual(explanation["route"]["classification"], "public_runtime")
        self.assertEqual(
            explanation["binding"]["rpc"],
            "crm.customer_privacy.v1.CustomerPrivacyCaseService.SubmitPrivacyCase",
        )
        self.assertTrue(explanation["references"])

    def test_unknown_explanation_target_fails_closed(self) -> None:
        with self.assertRaisesRegex(NavigationError, "unknown module or capability"):
            explain_target(ROOT, "customer_privacy.unknown@1.0.0")

    def test_generated_navigation_is_deterministic_and_current(self) -> None:
        first = generated_documents(ROOT)
        second = generated_documents(ROOT)
        self.assertEqual(first, second)
        self.assertEqual(
            set(first), {ACTIVE_PACKET_PATH, REPOSITORY_MAP_PATH}
        )
        for content in first.values():
            self.assertIn("Generated by scripts/generate_repository_navigation.py", content)
            self.assertIn("source-digest: sha256:", content)
        self.assertIn("**Workspace packages:** 113", first[REPOSITORY_MAP_PATH])
        self.assertIn("`crm.customer-privacy`", first[REPOSITORY_MAP_PATH])
        self.assertEqual(stale_generated_documents(ROOT), [])

    def test_path_policy_rejects_forbidden_and_unscoped_changes(self) -> None:
        forbidden, disallowed = evaluate_path_policy(
            [
                "docs/README.md",
                "scripts/repository_navigation.py",
                "proto/crm/customer/v1/customer.proto",
                "unowned.txt",
            ],
            ["docs/**", "scripts/repository_navigation.py"],
            ["proto/**"],
        )
        self.assertEqual(forbidden, ["proto/crm/customer/v1/customer.proto"])
        self.assertEqual(disallowed, ["unowned.txt"])

    def test_packet_check_reports_affected_scope_without_running_git_or_cargo(self) -> None:
        changed_paths = [
            ".github/workflows/rust-generated-sync.yml",
            ".github/workflows/rust.yml",
            "docs/ACTIVE_PACKET.md",
            "repository-packet.json",
            "tests/test_architecture_documentation_consistency.py",
            "tests/test_repository_navigation.py",
        ]
        affected = {
            "head_sha": "b" * 40,
            "changed_paths": changed_paths,
            "affected_packages": [],
            "selected_workflows": [
                {
                    "name": "Governance CI",
                    "path": ".github/workflows/governance.yml",
                    "selected": True,
                    "reasons": ["test fixture"],
                }
            ],
        }
        with (
            patch(
                "scripts.repository_navigation._git",
                return_value="59ab9242a5d38aa143313c120d3e076adad9b851",
            ),
            patch("scripts.repository_navigation.build_report", return_value=affected),
            patch("scripts.repository_navigation.stale_generated_documents", return_value=[]),
        ):
            report = packet_check(ROOT, "origin/main")
        self.assertTrue(report["ok"])
        self.assertEqual(report["changed_paths"], changed_paths)
        self.assertEqual(report["blockers"], [])
        self.assertEqual(report["selected_workflows"][0]["name"], "Governance CI")

    def test_repo_parser_exposes_exact_step_5_commands(self) -> None:
        parser = build_parser()
        explain = parser.parse_args(
            ["explain", "customer_privacy.case.submit@1.0.0", "--json"]
        )
        self.assertEqual(explain.command, "explain")
        self.assertTrue(explain.json)
        packet = parser.parse_args(
            ["packet-check", "--base", "origin/main", "--write-generated"]
        )
        self.assertEqual(packet.command, "packet-check")
        self.assertEqual(packet.base, "origin/main")
        self.assertTrue(packet.write_generated)


if __name__ == "__main__":
    unittest.main()
