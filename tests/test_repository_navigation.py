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

ALLOWED_PACKET_PATHS = [
    "docs/ACTIVE_PACKET.md",
    "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md",
    "docs/IMPLEMENTATION_ROADMAP.md",
    "docs/MODULE_CATALOG.md",
    "docs/PHASE8_DELIVERY_PLAN.md",
    "docs/PROJECT_STATUS.md",
    "repository-packet.json",
    "tests/test_architecture_documentation_consistency.py",
    "tests/test_repository_navigation.py",
]


class RepositoryNavigationTests(unittest.TestCase):
    def test_active_packet_declaration_is_valid_and_exact(self) -> None:
        packet = load_packet(ROOT)
        self.assertEqual(packet["packet_id"], "repository-step-13-measurement-evidence-sync")
        self.assertEqual(packet["status"], "active")
        self.assertEqual(packet["baseline"]["ref"], "main")
        self.assertEqual(
            packet["baseline"]["sha"],
            "7dcda204be07209d9e4996fdc9c5fd364cea179e",
        )
        self.assertEqual(packet["tracking_issues"], [194, 126])
        self.assertEqual(packet["allowed_paths"], ALLOWED_PACKET_PATHS)
        self.assertEqual(
            packet["required_checks"],
            [
                "Affected Scope CI",
                "Customer Privacy Access Export CI",
                "Customer Privacy Owner Execution CI",
                "Governance CI",
                "Rust CI",
                "Rust Generated Sync",
            ],
        )
        self.assertIn(
            "the architecture plan, project status, roadmap, Phase 8 plan, module catalog and issues agree on accepted PR #253 evidence",
            packet["acceptance"],
        )

    def test_affected_scope_workflow_executes_real_packet_check(self) -> None:
        workflow = (
            ROOT / ".github/workflows/affected-scope.yml"
        ).read_text(encoding="utf-8")
        self.assertIn(
            "Test affected-scope policy and analyzer",
            workflow,
        )
        self.assertIn("Explain and enforce affected scope", workflow)
        self.assertIn(
            "Upload deterministic affected-scope report",
            workflow,
        )
        self.assertIn(
            "ref: ${{ github.event.pull_request.head.sha }}",
            workflow,
        )
        self.assertIn("Validate active repository packet", workflow)
        self.assertIn(
            'python scripts/repo.py packet-check --base "${BASE_REF}"',
            workflow,
        )
        self.assertLess(
            workflow.index("Validate active repository packet"),
            workflow.index("Run affected structural preflight"),
        )

    def test_rust_workflows_preserve_the_committed_lockfile(self) -> None:
        generated_sync = (
            ROOT / ".github/workflows/rust-generated-sync.yml"
        ).read_text(encoding="utf-8")
        rust_ci = (
            ROOT / ".github/workflows/rust.yml"
        ).read_text(encoding="utf-8")
        repo_runner = (
            ROOT / "scripts/repo.py"
        ).read_text(encoding="utf-8")

        for workflow in (generated_sync, rust_ci):
            self.assertNotIn("cargo generate-lockfile", workflow)
            self.assertIn(
                "cargo metadata --locked --format-version 1 --no-deps",
                workflow,
            )
            self.assertIn("lockfile_before=", workflow)
            self.assertIn("lockfile_after=", workflow)
            self.assertIn(
                "git diff --exit-code -- Cargo.lock",
                workflow,
            )

        self.assertNotIn("git add Cargo.lock", generated_sync)
        self.assertIn(
            "cargo run --locked -p crm-proto-contracts",
            generated_sync,
        )
        self.assertIn(
            "cargo clippy --locked --fix",
            generated_sync,
        )
        self.assertIn(
            "cargo clippy --locked --workspace",
            generated_sync,
        )

        self.assertNotIn(
            "Upload resolved Cargo lockfile",
            rust_ci,
        )
        self.assertNotIn("name: cargo-lockfile", rust_ci)
        self.assertIn(
            "cargo check --locked --workspace",
            rust_ci,
        )
        self.assertIn(
            "cargo clippy --locked --workspace",
            rust_ci,
        )
        self.assertIn(
            "cargo test --locked --workspace",
            rust_ci,
        )

        self.assertIn(
            'run(["cargo", "generate-lockfile"])',
            repo_runner,
        )
        self.assertIn(
            '"lock", help="regenerate the committed Cargo lockfile"',
            repo_runner,
        )

    def test_module_explanation_traces_customer_privacy_owner(self) -> None:
        explanation = explain_target(ROOT, "crm.customer-privacy")
        self.assertEqual(
            explanation["schema_version"],
            NAVIGATION_SCHEMA,
        )
        self.assertEqual(explanation["kind"], "module")
        self.assertEqual(explanation["version"], "0.3.0")
        self.assertEqual(
            explanation["owner"]["team"],
            "customer-platform",
        )
        self.assertEqual(
            explanation["manifest_path"],
            "modules/crm-customer-privacy/module.yaml",
        )
        coordinates = {
            capability["coordinate"]
            for capability in explanation["capabilities"]
        }
        self.assertIn(
            "customer_privacy.case.submit@1.0.0",
            coordinates,
        )
        self.assertIn(
            "customer_privacy.restriction.place@1.0.0",
            coordinates,
        )
        self.assertIn(
            "customer_privacy.legal_hold.place@1.0.0",
            coordinates,
        )
        self.assertTrue(explanation["references"])

    def test_capability_explanation_resolves_exact_binding_and_runtime(
        self,
    ) -> None:
        explanation = explain_target(
            ROOT,
            "customer_privacy.case.submit@1.0.0",
        )
        self.assertEqual(explanation["kind"], "capability")
        self.assertEqual(
            explanation["owner_module_id"],
            "crm.customer-privacy",
        )
        self.assertEqual(
            explanation["route"]["classification"],
            "public_runtime",
        )
        self.assertEqual(
            explanation["binding"]["rpc"],
            "crm.customer_privacy.v1.CustomerPrivacyCaseService.SubmitPrivacyCase",
        )
        self.assertTrue(explanation["references"])

    def test_unknown_explanation_target_fails_closed(self) -> None:
        with self.assertRaisesRegex(
            NavigationError,
            "unknown module or capability",
        ):
            explain_target(
                ROOT,
                "customer_privacy.unknown@1.0.0",
            )

    def test_generated_navigation_is_deterministic_and_current(self) -> None:
        first = generated_documents(ROOT)
        second = generated_documents(ROOT)
        self.assertEqual(first, second)
        self.assertEqual(
            set(first),
            {ACTIVE_PACKET_PATH, REPOSITORY_MAP_PATH},
        )
        for content in first.values():
            self.assertIn(
                "Generated by scripts/generate_repository_navigation.py",
                content,
            )
            self.assertIn(
                "source-digest: sha256:",
                content,
            )
        self.assertIn(
            "**Workspace packages:** 113",
            first[REPOSITORY_MAP_PATH],
        )
        self.assertIn(
            "`crm.customer-privacy`",
            first[REPOSITORY_MAP_PATH],
        )
        self.assertEqual(stale_generated_documents(ROOT), [])

    def test_path_policy_rejects_forbidden_and_unscoped_changes(
        self,
    ) -> None:
        forbidden, disallowed = evaluate_path_policy(
            [
                "docs/README.md",
                "scripts/repository_navigation.py",
                "proto/crm/customer/v1/customer.proto",
                "unowned.txt",
            ],
            [
                "docs/**",
                "scripts/repository_navigation.py",
            ],
            ["proto/**"],
        )
        self.assertEqual(
            forbidden,
            ["proto/crm/customer/v1/customer.proto"],
        )
        self.assertEqual(disallowed, ["unowned.txt"])

    def test_packet_check_reports_affected_scope_without_running_git_or_cargo(
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
                    ("Customer Privacy Access Export CI", ".github/workflows/customer-privacy-access-export.yml"),
                    ("Customer Privacy Owner Execution CI", ".github/workflows/customer-privacy-owner-execution.yml"),
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
                    "7dcda204be07209d9e4996fdc9c5fd364cea179e"
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
        )

    def test_repo_parser_exposes_navigation_commands(self) -> None:
        parser = build_parser()
        explain = parser.parse_args(
            [
                "explain",
                "customer_privacy.legal_hold.place@1.0.0",
                "--json",
            ]
        )
        self.assertEqual(explain.command, "explain")
        self.assertTrue(explain.json)
        packet = parser.parse_args(
            [
                "packet-check",
                "--base",
                "origin/main",
                "--write-generated",
            ]
        )
        self.assertEqual(packet.command, "packet-check")
        self.assertEqual(packet.base, "origin/main")
        self.assertTrue(packet.write_generated)


if __name__ == "__main__":
    unittest.main()
