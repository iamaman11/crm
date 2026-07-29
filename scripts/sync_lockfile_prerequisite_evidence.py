#!/usr/bin/env python3
from pathlib import Path

SOURCE = "3f09dcc595f79d633915e4a67117aedc59ed2499"
MERGE = "3eab6dcd1d03a15ef0ce148d7f74137d2e1d10ed"


def replace_once(path: str, old: str, new: str, label: str) -> None:
    file = Path(path)
    text = file.read_text(encoding="utf-8")
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{path}: {label}: expected one occurrence, found {count}")
    file.write_text(text.replace(old, new, 1), encoding="utf-8")


def replace_block(path: str, start_marker: str, end_marker: str, replacement: str) -> None:
    file = Path(path)
    text = file.read_text(encoding="utf-8")
    start = text.index(start_marker)
    end = text.index(end_marker, start)
    file.write_text(text[:start] + replacement + text[end:], encoding="utf-8")


accepted = (
    f"PR #232 / accepted source `{SOURCE}` / squash merge `{MERGE}` / "
    "5 of 5 applicable permanent workflows accepts the smallest repository-step-6 "
    "architecture prerequisite. Rust Generated Sync and Rust CI now verify the committed "
    "dependency graph with locked Cargo commands, preserve `Cargo.lock` byte-for-byte on "
    "ordinary packets and cannot auto-commit registry drift. Intentional lockfile refresh "
    "remains explicit through `python scripts/repo.py lock` inside a bounded packet. The "
    "six-file change adds no product behavior, contract, manifest, dependency, package, "
    "persistence or migration change."
)

# Architecture plan.
replace_once(
    "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md",
    "| B — dependency, crate and exception governance | **In progress** | reproducible baseline, crate justification, calibrated inheritance cohorts, root-family no-growth, exact Rust `1.97.1` toolchain/workspace `rust-version`, measured zero-warning Rust/Clippy baseline and three exact expiring legacy lint exceptions | additional homogeneous dependency cohorts, removal of the three direct-lint exceptions, public-surface/fan-out calibration |",
    "| B — dependency, crate and exception governance | **In progress** | reproducible baseline, crate justification, calibrated inheritance cohorts, root-family no-growth, exact Rust `1.97.1` toolchain/workspace `rust-version`, measured zero-warning Rust/Clippy baseline, lockfile-preserving Rust workflows and three exact expiring legacy lint exceptions | additional homogeneous dependency cohorts, removal of the three direct-lint exceptions, public-surface/fan-out calibration |",
    "stage B evidence",
)
replace_once(
    "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md",
    "Repository step 5 is accepted through PR #228 / accepted source `a9aa0bef028d906b61e83803436167bf6f91e634` / squash merge `727a244fcf174dc517dec6fdbb6b8997eb205f14` / 5 of 5 applicable permanent workflows. It adds deterministic explanation, fail-closed packet validation, real-diff Affected Scope enforcement and generated navigation without changing product behavior, contracts, persistence, dependencies, `Cargo.lock` or package count.\n\nThe next permitted implementation packet is repository step 6: Customer Privacy legal-hold and mandatory-retention precedence.",
    "Repository step 5 is accepted through PR #228 / accepted source `a9aa0bef028d906b61e83803436167bf6f91e634` / squash merge `727a244fcf174dc517dec6fdbb6b8997eb205f14` / 5 of 5 applicable permanent workflows. It adds deterministic explanation, fail-closed packet validation, real-diff Affected Scope enforcement and generated navigation without changing product behavior, contracts, persistence, dependencies, `Cargo.lock` or package count.\n\n" + accepted + "\n\nThe next permitted implementation packet is repository step 6: Customer Privacy legal-hold and mandatory-retention precedence.",
    "accepted prerequisite paragraph",
)
replace_once(
    "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md",
    "The inserted repository step 4 prerequisite is complete through PR #224, repository step 4 is complete through PR #226, and repository step 5 is complete through PR #228. None changes the master numbering.",
    "The inserted repository step 4 prerequisite is complete through PR #224, repository step 4 is complete through PR #226, repository step 5 is complete through PR #228, and the inserted lockfile-preservation prerequisite before repository step 6 is complete through PR #232. None changes the master numbering.",
    "master-order prerequisite note",
)

# Project status.
replace_once(
    "docs/PROJECT_STATUS.md",
    "Latest accepted repository architecture/developer-experience packet is PR #228 / accepted source `a9aa0bef028d906b61e83803436167bf6f91e634` / squash merge `727a244fcf174dc517dec6fdbb6b8997eb205f14` / 5 of 5 applicable permanent workflows.",
    f"Latest accepted repository architecture/developer-experience packet is PR #232 / accepted source `{SOURCE}` / squash merge `{MERGE}` / 5 of 5 applicable permanent workflows.",
    "latest architecture packet",
)
replace_once(
    "docs/PROJECT_STATUS.md",
    "The accepted generated inventory is 113 workspace packages, 14 business manifests, 119 published capability coordinates, 70 published event coordinates, 7 platform runtime routes, 5 worker runtime routes, 17 non-runtime contract routes and one route-less module. Product runtime, contracts, manifests, migrations, dependencies, `Cargo.lock`, package count and Customer Privacy behavior are unchanged.\n\n## Next permitted repository packet",
    "The accepted generated inventory is 113 workspace packages, 14 business manifests, 119 published capability coordinates, 70 published event coordinates, 7 platform runtime routes, 5 worker runtime routes, 17 non-runtime contract routes and one route-less module. Product runtime, contracts, manifests, migrations, dependencies, `Cargo.lock`, package count and Customer Privacy behavior are unchanged.\n\n## Accepted lockfile-preserving Rust workflow prerequisite\n\n" + accepted + "\n\n## Next permitted repository packet",
    "status accepted section",
)
replace_once(
    "docs/PROJECT_STATUS.md",
    "- Stage B dependency/crate/exception governance is in progress: reproducible metrics, calibrated inheritance cohorts, root-family no-growth, exact Rust `1.97.1`, root `rust-version`, measured zero-warning Rust/Clippy governance and three exact expiring legacy lint exceptions are accepted; broader dependency/public-surface calibration and exception removal remain.",
    "- Stage B dependency/crate/exception governance is in progress: reproducible metrics, calibrated inheritance cohorts, root-family no-growth, exact Rust `1.97.1`, root `rust-version`, measured zero-warning Rust/Clippy governance, lockfile-preserving Rust workflows and three exact expiring legacy lint exceptions are accepted; broader dependency/public-surface calibration and exception removal remain.",
    "status stage B",
)
replace_once(
    "docs/PROJECT_STATUS.md",
    "-> 5. explain / packet-check / generated active packet and repository map — complete through PR #228\n-> 6. legal-hold and mandatory-retention precedence — next",
    "-> 5. explain / packet-check / generated active packet and repository map — complete through PR #228\n-> 5a. lockfile-preserving Rust workflow prerequisite — complete through PR #232\n-> 6. legal-hold and mandatory-retention precedence — next",
    "status continuation",
)

# Implementation roadmap.
replace_once(
    "docs/IMPLEMENTATION_ROADMAP.md",
    "- Stage B dependency, crate and exception governance — **In progress; Rust toolchain and measured lint prerequisite accepted through PR #218**.",
    "- Stage B dependency, crate and exception governance — **In progress; Rust toolchain/lint prerequisite accepted through PR #218 and lockfile-preserving Rust workflows accepted through PR #232**.",
    "roadmap stage B",
)
replace_once(
    "docs/IMPLEMENTATION_ROADMAP.md",
    "Accepted architecture evidence includes PR #197, PR #199, PR #200, PR #203, PR #204, PR #205, PR #218, PR #222, PR #224, PR #226 and PR #228.",
    "Accepted architecture evidence includes PR #197, PR #199, PR #200, PR #203, PR #204, PR #205, PR #218, PR #222, PR #224, PR #226, PR #228 and PR #232.",
    "roadmap evidence list",
)
replace_once(
    "docs/IMPLEMENTATION_ROADMAP.md",
    "Repository steps 1–5 are complete through PR #218, PR #220, PR #222, PR #226 and PR #228; PR #224 accepted the smallest inserted prerequisite required by step 4. Repository step 6 — legal-hold and mandatory-retention precedence — is the current next packet.",
    "Repository steps 1–5 are complete through PR #218, PR #220, PR #222, PR #226 and PR #228; PR #224 accepted the smallest inserted prerequisite required by step 4, and PR #232 accepted the smallest lockfile-preservation prerequisite required before step 6. Repository step 6 — legal-hold and mandatory-retention precedence — is the current next packet.",
    "roadmap sequence summary",
)
replace_once(
    "docs/IMPLEMENTATION_ROADMAP.md",
    "The packet adds deterministic exact module/capability explanation, fail-closed packet validation, real-diff Affected Scope enforcement, generated active-packet/repository-map navigation and permanent freshness tests. It records 113 packages, 14 manifests, 119 capabilities and 70 events without changing product runtime, contracts, persistence, migrations, dependencies, `Cargo.lock` or workspace package count.\n\n## 6. Binding active sequence",
    "The packet adds deterministic exact module/capability explanation, fail-closed packet validation, real-diff Affected Scope enforcement, generated active-packet/repository-map navigation and permanent freshness tests. It records 113 packages, 14 manifests, 119 capabilities and 70 events without changing product runtime, contracts, persistence, migrations, dependencies, `Cargo.lock` or workspace package count.\n\n### 5.10 Accepted lockfile-preserving Rust workflow prerequisite\n\n" + accepted + "\n\n## 6. Binding active sequence",
    "roadmap accepted section",
)
replace_once(
    "docs/IMPLEMENTATION_ROADMAP.md",
    "5. **Repository step 5 — `explain`, `packet-check` and generated navigation — Complete through PR #228.**\n6. **Repository step 6 — legal-hold and mandatory-retention precedence — Next.**",
    "5. **Repository step 5 — `explain`, `packet-check` and generated navigation — Complete through PR #228.**\n5a. **Inserted lockfile-preservation prerequisite before repository step 6 — Complete through PR #232.**\n6. **Repository step 6 — legal-hold and mandatory-retention precedence — Next.**",
    "roadmap binding sequence",
)

# Phase 8 plan.
replace_once(
    "docs/PHASE8_DELIVERY_PLAN.md",
    "PR #228 / accepted source `a9aa0bef028d906b61e83803436167bf6f91e634` / squash merge `727a244fcf174dc517dec6fdbb6b8997eb205f14` / 5 of 5 applicable permanent workflows completes repository step 5. Deterministic explanation, packet validation, real-diff affected enforcement and generated navigation are accepted; repository step 6 is now the next permitted implementation packet.",
    "PR #228 / accepted source `a9aa0bef028d906b61e83803436167bf6f91e634` / squash merge `727a244fcf174dc517dec6fdbb6b8997eb205f14` / 5 of 5 applicable permanent workflows completes repository step 5. Deterministic explanation, packet validation, real-diff affected enforcement and generated navigation are accepted.\n\n" + accepted + " Repository step 6 remains the next permitted implementation packet.",
    "phase8 prerequisite evidence",
)
replace_once(
    "docs/PHASE8_DELIVERY_PLAN.md",
    "5. repository step 5 — `explain`, `packet-check` and generated navigation — **complete through PR #228**;\n6. repository step 6 — legal-hold and mandatory-retention precedence — **next**;",
    "5. repository step 5 — `explain`, `packet-check` and generated navigation — **complete through PR #228**;\n5a. inserted lockfile-preservation prerequisite before repository step 6 — **complete through PR #232**;\n6. repository step 6 — legal-hold and mandatory-retention precedence — **next**;",
    "phase8 sequence",
)
replace_once(
    "docs/PHASE8_DELIVERY_PLAN.md",
    "The inserted prerequisite did not renumber the normative master sequence.",
    "The inserted prerequisites did not renumber the normative master sequence.",
    "phase8 inserted prerequisites",
)

# Active packet consistency guard.
replace_block(
    "tests/test_architecture_documentation_consistency.py",
    "    def test_active_packet_is_machine_declared_and_generated(self) -> None:\n",
    "    def test_repository_map_matches_authoritative_inventory(self) -> None:\n",
    '''    def test_active_packet_is_machine_declared_and_generated(self) -> None:
        self.assertEqual(self.packet["schema_version"], "crm.repository-packet/v1")
        self.assertEqual(
            self.packet["packet_id"],
            "repository-step-6-lockfile-prerequisite-evidence-sync",
        )
        self.assertEqual(self.packet["status"], "active")
        self.assertEqual(
            self.packet["baseline"]["sha"],
            "3eab6dcd1d03a15ef0ce148d7f74137d2e1d10ed",
        )
        self.assertEqual(self.packet["tracking_issues"], [194, 231])
        for path in (
            "docs/ACTIVE_PACKET.md",
            "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md",
            "docs/IMPLEMENTATION_ROADMAP.md",
            "docs/PHASE8_DELIVERY_PLAN.md",
            "docs/PROJECT_STATUS.md",
            "tests/test_architecture_documentation_consistency.py",
            "tests/test_repository_navigation.py",
        ):
            self.assertIn(path, self.packet["allowed_paths"])
        self.assertIn(".github/workflows/**", self.packet["forbidden_paths"])
        self.assertIn("Cargo.lock", self.packet["forbidden_paths"])
        self.assertIn("Rust CI", self.packet["required_checks"])
        self.assertIn("Rust Generated Sync", self.packet["required_checks"])

        self.assertIn("Generated by scripts/generate_repository_navigation.py", self.active_packet)
        self.assertIn("repository-step-6-lockfile-prerequisite-evidence-sync", self.active_packet)
        self.assertIn(self.packet["baseline"]["sha"], self.active_packet)
        self.assertRegex(self.active_packet, r"sha256:[0-9a-f]{64}")
        self.assertIn("orientation only", self.active_packet)

        for document in self.authoritative_status_documents:
            self.assertIn("PR #232", document)
            self.assertIn("3f09dcc595f79d633915e4a67117aedc59ed2499", document)
            self.assertIn("3eab6dcd1d03a15ef0ce148d7f74137d2e1d10ed", document)
            self.assertIn("5 of 5", document)

''',
)

# Repository navigation guard methods.
replace_block(
    "tests/test_repository_navigation.py",
    "    def test_active_packet_declaration_is_valid_and_exact(self) -> None:\n",
    "    def test_affected_scope_workflow_executes_real_packet_check(self) -> None:\n",
    '''    def test_active_packet_declaration_is_valid_and_exact(self) -> None:
        packet = load_packet(ROOT)
        self.assertEqual(
            packet["packet_id"],
            "repository-step-6-lockfile-prerequisite-evidence-sync",
        )
        self.assertEqual(packet["status"], "active")
        self.assertEqual(packet["baseline"]["ref"], "main")
        self.assertEqual(
            packet["baseline"]["sha"],
            "3eab6dcd1d03a15ef0ce148d7f74137d2e1d10ed",
        )
        self.assertEqual(packet["tracking_issues"], [194, 231])
        for path in (
            "docs/ACTIVE_PACKET.md",
            "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md",
            "docs/IMPLEMENTATION_ROADMAP.md",
            "docs/PHASE8_DELIVERY_PLAN.md",
            "docs/PROJECT_STATUS.md",
            "tests/test_architecture_documentation_consistency.py",
            "tests/test_repository_navigation.py",
        ):
            self.assertIn(path, packet["allowed_paths"])
        self.assertIn("Cargo.lock", packet["forbidden_paths"])
        self.assertIn("Rust CI", packet["required_checks"])
        self.assertIn(
            "repository step 6 remains the only next implementation packet",
            packet["acceptance"],
        )

''',
)
replace_block(
    "tests/test_repository_navigation.py",
    "    def test_packet_check_reports_affected_scope_without_running_git_or_cargo(self) -> None:\n",
    "    def test_repo_parser_exposes_exact_step_5_commands(self) -> None:\n",
    '''    def test_packet_check_reports_affected_scope_without_running_git_or_cargo(self) -> None:
        changed_paths = [
            "docs/ACTIVE_PACKET.md",
            "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md",
            "docs/IMPLEMENTATION_ROADMAP.md",
            "docs/PHASE8_DELIVERY_PLAN.md",
            "docs/PROJECT_STATUS.md",
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
                return_value="3eab6dcd1d03a15ef0ce148d7f74137d2e1d10ed",
            ),
            patch("scripts.repository_navigation.build_report", return_value=affected),
            patch("scripts.repository_navigation.stale_generated_documents", return_value=[]),
        ):
            report = packet_check(ROOT, "origin/main")
        self.assertTrue(report["ok"])
        self.assertEqual(report["changed_paths"], changed_paths)
        self.assertEqual(report["blockers"], [])
        self.assertEqual(report["selected_workflows"][0]["name"], "Governance CI")

''',
)
