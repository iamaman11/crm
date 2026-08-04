from __future__ import annotations

import json
from pathlib import Path
import re

from scripts.repository_navigation import generated_documents

ROOT = Path(__file__).resolve().parents[1]
SOURCE = "f9c5faa667f4d5483335ec2cb5bac31596d818c8"
MERGE = "ef3457c11646b1069e5e65683d3618b3d470136e"
BACKUP_SHA256 = "700b8ae13a71af30010b11877f70b6a4b3efe1b0ec3beddaf0f3e3bc19533d3c"

NORMATIVE = (
    "docs/PROJECT_STATUS.md",
    "docs/IMPLEMENTATION_ROADMAP.md",
    "docs/PHASE8_DELIVERY_PLAN.md",
    "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md",
    "docs/MODULE_CATALOG.md",
    "docs/PRODUCT_DEVELOPMENT_10_OF_10_PLAN.md",
    "docs/WORKSPACE_COMPLEXITY_BASELINE.md",
)
SPECIALIZED = (
    "docs/PHASE8A_CUSTOMER_PRIVACY_PRODUCT_PLANE.md",
    "docs/CUSTOMER_PRIVACY_OPERATIONS_READINESS.md",
)

EVIDENCE_HEADING = "## Repository Step 20 accepted closure"
EVIDENCE_BLOCK = f"""

{EVIDENCE_HEADING}

Repository Step 20 is complete through PR #292 / source `938cebed1e78bf7debf40dc544431bfe819970f4` / squash merge `fffd6baf35544eea736d183af0a5ba38518cce9a` / 17 of 17 applicable permanent workflows and PR #294 / source `{SOURCE}` / squash merge `{MERGE}` / 8 of 8 applicable permanent workflows, each accepted on one unchanged exact head with zero unresolved comments, reviews or review threads.

Step 20A proves the typed governed Customer Privacy browser product plane against real PostgreSQL, assembled `crm-api`, Vite and Chromium. Step 20B proves independent PostgreSQL logical backup and restore, restored-process startup and readiness, active `customer_privacy.case.list` and `customer_privacy.case.get` metrics, cross-tenant and expired-session concealment, startup `0.101` seconds, nearest-rank readiness p95 `2.977` milliseconds, backup SHA-256 `{BACKUP_SHA256}`, backup size `1,118,941` bytes and Chromium 3 of 3.

Repository Steps 1–20 are complete. Repository Step 21 Phase 8A closure is the only next permitted implementation packet. Phase 8A.11, Phase 8A, Customer Privacy as a complete product capability, product-complete expert modules, architecture 10/10 and the Universal CRM product remain incomplete.
"""


def read(path: str) -> str:
    return (ROOT / path).read_text(encoding="utf-8")


def write(path: str, content: str) -> None:
    (ROOT / path).write_text(content, encoding="utf-8")


def update_document(path: str) -> None:
    text = read(path)
    replacements = (
        ("Repository Steps 1–19 are complete", "Repository Steps 1–20 are complete"),
        ("Repository Step 20 remains in progress", "Repository Step 20 is complete"),
        (
            "Repository Step 20B restore, SLO, observability, performance, security and supply-chain operations evidence is the only next permitted implementation packet",
            "Repository Step 21 Phase 8A closure is the only next permitted implementation packet",
        ),
        (
            "Repository Step 20B restore, SLO, observability, performance, security and supply-chain operations evidence is the sole next permitted implementation packet",
            "Repository Step 21 Phase 8A closure is the sole next permitted implementation packet",
        ),
        (
            "Repository Step 20B operations evidence is the only next permitted implementation packet",
            "Repository Step 21 Phase 8A closure is the only next permitted implementation packet",
        ),
        (
            "Repository Step 20B operations evidence is the sole next permitted implementation packet",
            "Repository Step 21 Phase 8A closure is the sole next permitted implementation packet",
        ),
        (
            "Repository Step 20B is the only next permitted implementation packet",
            "Repository Step 21 Phase 8A closure is the only next permitted implementation packet",
        ),
        (
            "Repository Step 20B is the sole next permitted implementation packet",
            "Repository Step 21 Phase 8A closure is the sole next permitted implementation packet",
        ),
        ("After Steps 20–21 complete Phase 8A", "After Step 21 complete Phase 8A"),
        ("Step 21 remains blocked", "Step 21 is the next permitted packet"),
    )
    for old, new in replacements:
        text = text.replace(old, new)
    text = re.sub(
        r"(?m)^20\. Phase 8A frontend[^\n]*$",
        "20. Phase 8A frontend, accessibility, browser and operations parity — **complete through PRs #292 and #294**.",
        text,
    )
    text = re.sub(
        r"(?m)^Status date: \d{4}-\d{2}-\d{2}$",
        "Status date: 2026-08-05",
        text,
    )
    if EVIDENCE_HEADING not in text:
        text = text.rstrip() + EVIDENCE_BLOCK + "\n"
    write(path, text)


def replace_method(text: str, current: str, following: str, replacement: str) -> str:
    pattern = rf"(?ms)^    def {re.escape(current)}\(.*?(?=^    def {re.escape(following)}\()"
    updated, count = re.subn(pattern, replacement.rstrip() + "\n\n", text)
    if count != 1:
        raise RuntimeError(f"expected one method {current}, found {count}")
    return updated


def update_architecture_test() -> None:
    path = "tests/test_architecture_documentation_consistency.py"
    text = read(path)
    step_method = '''    def test_step_20_is_complete_and_step_21_is_next(self) -> None:
        required = (
            "PR #292",
            "938cebed1e78bf7debf40dc544431bfe819970f4",
            "fffd6baf35544eea736d183af0a5ba38518cce9a",
            "17 of 17",
            "PR #294",
            "f9c5faa667f4d5483335ec2cb5bac31596d818c8",
            "ef3457c11646b1069e5e65683d3618b3d470136e",
            "8 of 8",
        )
        for document in self.normative_documents:
            lowered = document.lower()
            for marker in required:
                self.assertIn(marker, document)
            self.assertIn("step 20", lowered)
            self.assertIn("step 21", lowered)
            self.assertNotRegex(
                lowered,
                r"step 20[^\\n.;]{0,120}(?:not started|in progress|next)",
            )
            self.assertNotRegex(document, r"(?m)^\\s*-\\s*;\\s*$")
        self.assertIn("Repository Steps 1–20 are complete", self.status)
        self.assertIn("Repository Step 20 is complete through PR #292", self.status)
        self.assertIn("Repository Step 21 Phase 8A closure", self.status)
        self.assertIn(
            "20. Phase 8A frontend, accessibility, browser and operations parity — **complete through PRs #292 and #294**.",
            self.plan,
        )
        for marker in required:
            self.assertIn(marker, self.product_plan)
        self.assertIn("After Step 21 complete Phase 8A", self.product_plan)
        self.assertNotIn("After Steps 20–21 complete Phase 8A", self.product_plan)
        self.assertIn("Chromium 3 of 3", self.operations)
        self.assertIn("2.977", self.operations)
        self.assertIn("0.101", self.operations)
        self.assertIn("700b8ae13a71af30010b11877f70b6a4b3efe1b0ec3beddaf0f3e3bc19533d3c", self.operations)
'''
    text = replace_method(
        text,
        "test_step_19_is_complete_step_20a_is_accepted_and_step_20b_is_next",
        "test_product_readiness_is_not_overstated",
        step_method,
    )
    packet_method = '''    def test_active_step_20_evidence_sync_packet_is_exact(self) -> None:
        self.assertEqual(self.packet["schema_version"], "crm.repository-packet/v1")
        self.assertEqual(self.packet["packet_id"], "repository-step-20-evidence-sync")
        self.assertEqual(
            self.packet["baseline"],
            {"ref": "main", "sha": "ef3457c11646b1069e5e65683d3618b3d470136e"},
        )
        self.assertEqual(len(self.packet["allowed_paths"]), 13)
        self.assertEqual(
            self.packet["required_checks"],
            [
                "Affected Scope CI",
                "Complexity Baseline CI",
                "Customer Privacy Access Export CI",
                "Customer Privacy Owner Execution CI",
                "Governance CI",
                "Rust Generated Sync",
                "Rust CI",
            ],
        )
        self.assertIn(self.packet["packet_id"], self.active_packet)
        self.assertIn(self.packet["baseline"]["sha"], self.active_packet)
        self.assertIn("PR #294", self.operations)
        self.assertIn("Chromium 3 of 3", self.operations)
        self.assertIn("Repository Step 21", self.operations)
'''
    text = replace_method(
        text,
        "test_active_step_20b_operations_packet_is_exact",
        "test_repository_map_and_product_inventory_remain_exact",
        packet_method,
    )
    write(path, text)


def update_navigation_test() -> None:
    path = "tests/test_repository_navigation.py"
    text = read(path)
    declaration = '''    def test_active_step_20_evidence_sync_packet_declaration_is_exact(self) -> None:
        packet = load_packet(ROOT)
        self.assertEqual(packet["schema_version"], "crm.repository-packet/v1")
        self.assertEqual(packet["packet_id"], "repository-step-20-evidence-sync")
        self.assertEqual(packet["status"], "active")
        self.assertEqual(
            packet["baseline"],
            {"ref": "main", "sha": "ef3457c11646b1069e5e65683d3618b3d470136e"},
        )
        self.assertEqual(packet["tracking_issues"], [194, 126])
        self.assertEqual(
            set(packet["allowed_paths"]),
            {
                "docs/ACTIVE_PACKET.md",
                "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md",
                "docs/CUSTOMER_PRIVACY_OPERATIONS_READINESS.md",
                "docs/IMPLEMENTATION_ROADMAP.md",
                "docs/MODULE_CATALOG.md",
                "docs/PHASE8A_CUSTOMER_PRIVACY_PRODUCT_PLANE.md",
                "docs/PHASE8_DELIVERY_PLAN.md",
                "docs/PRODUCT_DEVELOPMENT_10_OF_10_PLAN.md",
                "docs/PROJECT_STATUS.md",
                "docs/WORKSPACE_COMPLEXITY_BASELINE.md",
                "repository-packet.json",
                "tests/test_architecture_documentation_consistency.py",
                "tests/test_repository_navigation.py",
            },
        )
        self.assertEqual(
            packet["required_checks"],
            [
                "Affected Scope CI",
                "Complexity Baseline CI",
                "Customer Privacy Access Export CI",
                "Customer Privacy Owner Execution CI",
                "Governance CI",
                "Rust Generated Sync",
                "Rust CI",
            ],
        )
        self.assertIn("PR #294", " ".join(packet["deliverables"]))
'''
    text = replace_method(
        text,
        "test_active_step_20b_operations_packet_declaration_is_exact",
        "test_generated_navigation_is_deterministic_and_current",
        declaration,
    )
    text = text.replace(
        '"repository-step-20b-customer-privacy-operations",\n            first[ACTIVE_PACKET_PATH],',
        '"repository-step-20-evidence-sync",\n            first[ACTIVE_PACKET_PATH],',
    )
    text = text.replace(
        '"d3d066d0446a4936bd61574506e729c9fd9104dc", first[ACTIVE_PACKET_PATH]',
        '"ef3457c11646b1069e5e65683d3618b3d470136e", first[ACTIVE_PACKET_PATH]',
    )
    packet_check = '''    def test_packet_check_uses_exact_planning_baseline(self) -> None:
        packet = load_packet(ROOT)
        workflow_paths = {
            "Affected Scope CI": ".github/workflows/affected-scope.yml",
            "Complexity Baseline CI": ".github/workflows/complexity-baseline.yml",
            "Customer Privacy Access Export CI": ".github/workflows/customer-privacy-access-export.yml",
            "Customer Privacy Owner Execution CI": ".github/workflows/customer-privacy-owner-execution.yml",
            "Governance CI": ".github/workflows/governance.yml",
            "Rust Generated Sync": ".github/workflows/rust-generated-sync.yml",
            "Rust CI": ".github/workflows/rust.yml",
        }
        affected = {
            "head_sha": "b" * 40,
            "changed_paths": packet["allowed_paths"],
            "affected_packages": [],
            "selected_workflows": [
                {
                    "name": name,
                    "path": workflow_paths[name],
                    "selected": True,
                    "reasons": ["Step 20 evidence synchronization"],
                }
                for name in packet["required_checks"]
            ],
        }
        with (
            patch(
                "scripts.repository_navigation._git",
                return_value="ef3457c11646b1069e5e65683d3618b3d470136e",
            ),
            patch("scripts.repository_navigation.build_report", return_value=affected),
            patch(
                "scripts.repository_navigation.stale_generated_documents",
                return_value=[],
            ),
        ):
            report = packet_check(ROOT, "origin/main")
        self.assertTrue(report["ok"])
        self.assertEqual(report["changed_paths"], packet["allowed_paths"])
        self.assertEqual(report["blockers"], [])
'''
    text = replace_method(
        text,
        "test_packet_check_uses_exact_planning_baseline",
        "test_repository_workflow_and_parser_contracts_remain_intact",
        packet_check,
    )
    write(path, text)


def update_packet() -> None:
    packet = {
        "schema_version": "crm.repository-packet/v1",
        "packet_id": "repository-step-20-evidence-sync",
        "title": "Synchronize accepted Repository Step 20 evidence",
        "status": "active",
        "baseline": {"ref": "main", "sha": MERGE},
        "tracking_issues": [194, 126],
        "objective": "Record exact accepted Step 20A and Step 20B evidence across every live normative source close Repository Step 20 and make Repository Step 21 Phase 8A closure the sole next implementation packet without changing product or runtime behavior.",
        "allowed_paths": [
            "docs/ACTIVE_PACKET.md",
            "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md",
            "docs/CUSTOMER_PRIVACY_OPERATIONS_READINESS.md",
            "docs/IMPLEMENTATION_ROADMAP.md",
            "docs/MODULE_CATALOG.md",
            "docs/PHASE8A_CUSTOMER_PRIVACY_PRODUCT_PLANE.md",
            "docs/PHASE8_DELIVERY_PLAN.md",
            "docs/PRODUCT_DEVELOPMENT_10_OF_10_PLAN.md",
            "docs/PROJECT_STATUS.md",
            "docs/WORKSPACE_COMPLEXITY_BASELINE.md",
            "repository-packet.json",
            "tests/test_architecture_documentation_consistency.py",
            "tests/test_repository_navigation.py",
        ],
        "forbidden_paths": [
            ".github/workflows/**",
            "AGENTS.md",
            "Cargo.lock",
            "Cargo.toml",
            "README.md",
            "affected-scope-policy.json",
            "apps/**",
            "contracts/**",
            "crates/**",
            "customer-privacy-operations-policy.json",
            "database/**",
            "evidence/**",
            "modules/**",
            "package.json",
            "packages/**",
            "pnpm-lock.yaml",
            "pnpm-workspace.yaml",
            "proto/**",
            "requirements-dev.txt",
            "rust-toolchain.toml",
            "schemas/**",
            "scripts/**",
            "services/**",
            "tsconfig.base.json",
        ],
        "deliverables": [
            f"record PR #294 source {SOURCE} squash merge {MERGE} and 8 of 8 exact-head permanent workflow evidence in every live normative source",
            "record startup 0.101 seconds readiness p95 2.977 milliseconds backup SHA-256 and 1118941-byte backup evidence plus Chromium 3 of 3",
            "mark Repository Step 20 complete while keeping Phase 8A.11 Phase 8A Customer Privacy product completion architecture 10/10 and Universal CRM incomplete",
            "make Repository Step 21 Phase 8A closure the sole next permitted implementation packet",
            "preserve the accepted one-worker seven-mutation four-query inventory and frozen Rust complexity baselines",
            "update generated navigation and permanent documentation guards without product runtime workflow contract schema migration dependency or lockfile changes",
        ],
        "required_checks": [
            "Affected Scope CI",
            "Complexity Baseline CI",
            "Customer Privacy Access Export CI",
            "Customer Privacy Owner Execution CI",
            "Governance CI",
            "Rust Generated Sync",
            "Rust CI",
        ],
        "acceptance": [
            f"the branch is based exactly on main squash merge {MERGE}",
            f"all live normative sources contain PR #294 source {SOURCE} merge {MERGE} and 8 of 8 applicable permanent workflows",
            "the accepted evidence records independent restore restored crm-api active list and get metrics startup and p95 objectives backup digest and size and Chromium 3 of 3",
            "Repository Steps 1 through 20 are complete and Repository Step 21 Phase 8A closure is the only next permitted implementation packet",
            "Phase 8A.11 Phase 8A Customer Privacy product completion product-complete expert modules architecture 10/10 and Universal CRM remain incomplete",
            "the final diff contains only the thirteen declared documentation generated packet and permanent guard paths",
            "one unchanged exact head passes every applicable permanent workflow with zero unresolved comments reviews or review threads",
        ],
        "non_goals": [
            "change Customer Privacy product runtime authorization tenant isolation persistence workers capabilities contracts manifests schemas migrations dependencies or lockfiles",
            "change the accepted operations SLO policy or implementation gate",
            "start Repository Step 21 implementation or close Phase 8A.11 Phase 8A architecture 10/10 or Universal CRM",
        ],
    }
    write("repository-packet.json", json.dumps(packet, indent=2) + "\n")


def main() -> None:
    for path in NORMATIVE + SPECIALIZED:
        update_document(path)
    update_packet()
    update_architecture_test()
    update_navigation_test()
    for path, content in generated_documents(ROOT).items():
        path.write_text(content, encoding="utf-8")
    for path in NORMATIVE + SPECIALIZED:
        text = read(path)
        if SOURCE not in text or MERGE not in text or "Repository Step 21" not in text:
            raise RuntimeError(f"missing accepted Step 20 evidence in {path}")
    print("Repository Step 20 evidence synchronized")


if __name__ == "__main__":
    main()
