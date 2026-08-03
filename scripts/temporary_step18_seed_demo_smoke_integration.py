#!/usr/bin/env python3
"""One-shot fail-closed integration for Repository Step 18 seed-demo/smoke."""

from __future__ import annotations

import json
from pathlib import Path
import re
import subprocess

ROOT = Path(__file__).resolve().parents[1]
BASELINE = "953b9f929f8e89c351c59ebc01461fffab3689ff"
PACKET_ID = "repository-step-18-seed-demo-smoke"
REQUIRED_CHECKS = [
    "Affected Scope CI",
    "Application Runtime CI",
    "Complexity Baseline CI",
    "Customer Privacy Access Export CI",
    "Customer Privacy Approval CI",
    "Customer Privacy Owner Execution CI",
    "Customer Privacy Persistence CI",
    "Customer Privacy Restriction Policy CI",
    "Data Quality Process Runtime CI",
    "Database CI",
    "Export Process Runtime CI",
    "Generic Mutation Query Conformance CI",
    "Governance CI",
    "Import Process Runtime CI",
    "Import Retryable Process Runtime CI",
    "PostgreSQL Process Isolation Pilot",
    "Product Plane CI",
    "Rust Generated Sync",
    "Rust CI",
]
ALLOWED_PATHS = [
    ".github/workflows/governance.yml",
    "AGENTS.md",
    "README.md",
    "affected-scope-policy.json",
    "docs/ACTIVE_PACKET.md",
    "docs/DEVELOPMENT_WORKFLOW.md",
    "repository-packet.json",
    "scripts/local_demo.py",
    "scripts/repo.py",
    "services/crm-api/tests/local_demo_smoke_e2e.rs",
    "tests/test_architecture_documentation_consistency.py",
    "tests/test_local_demo.py",
    "tests/test_repository_navigation.py",
]


def read(path: str) -> str:
    return (ROOT / path).read_text(encoding="utf-8")


def write(path: str, content: str) -> None:
    (ROOT / path).write_text(content, encoding="utf-8")


def replace_exact(content: str, old: str, new: str, label: str, count: int = 1) -> str:
    actual = content.count(old)
    if actual != count:
        raise RuntimeError(f"{label}: expected {count} matches, found {actual}")
    return content.replace(old, new)


def replace_method(content: str, name: str, replacement: str) -> str:
    pattern = re.compile(
        rf"(?ms)^    def {re.escape(name)}\(self\).*?(?=^    def |^\nif __name__)"
    )
    updated, count = pattern.subn(lambda _: replacement.rstrip() + "\n\n", content)
    if count != 1:
        raise RuntimeError(f"method {name}: expected 1 match, found {count}")
    return updated


def update_repo_command_surface() -> None:
    path = "scripts/repo.py"
    content = read(path)
    content = replace_exact(
        content,
        '            "tests/test_local_dev.py",\n            "tests/test_local_lifecycle.py",',
        '            "tests/test_local_demo.py",\n            "tests/test_local_dev.py",\n            "tests/test_local_lifecycle.py",',
        "repo conformance demo test",
    )
    commands = '''
def _command_demo(mode: str, args: argparse.Namespace) -> None:
    try:
        from local_demo import LifecycleError, render_demo, seed_demo, smoke
    except ModuleNotFoundError:
        from scripts.local_demo import LifecycleError, render_demo, seed_demo, smoke

    operation = seed_demo if mode == "seed" else smoke
    try:
        report = operation(ROOT, dry_run=args.dry_run)
    except LifecycleError as error:
        raise CommandError(str(error)) from error
    if args.json:
        print(json.dumps(report, indent=2, sort_keys=True))
    else:
        print(render_demo(report), end="")


def command_seed_demo(args: argparse.Namespace) -> None:
    _command_demo("seed", args)


def command_smoke(args: argparse.Namespace) -> None:
    _command_demo("smoke", args)

'''
    content = replace_exact(
        content,
        "\ndef affected_clippy_command(report: dict) -> list[str] | None:\n",
        "\n" + commands + "def affected_clippy_command(report: dict) -> list[str] | None:\n",
        "repo demo command handlers",
    )
    parsers = '''
    seed_demo_parser = subparsers.add_parser(
        "seed-demo",
        help="create or idempotently replay the governed deterministic local demo dataset",
    )
    seed_demo_parser.add_argument(
        "--dry-run",
        action="store_true",
        help="show the owned database and exact locked process-test command without executing it",
    )
    seed_demo_parser.add_argument("--json", action="store_true")
    seed_demo_parser.set_defaults(handler=command_seed_demo)

    smoke_parser = subparsers.add_parser(
        "smoke",
        help="verify the deterministic local demo through the real crm-api process",
    )
    smoke_parser.add_argument(
        "--dry-run",
        action="store_true",
        help="show the exact locked smoke command without starting crm-api",
    )
    smoke_parser.add_argument("--json", action="store_true")
    smoke_parser.set_defaults(handler=command_smoke)

'''
    content = replace_exact(
        content,
        "\n    check_affected = subparsers.add_parser(\n",
        "\n" + parsers + "    check_affected = subparsers.add_parser(\n",
        "repo demo parsers",
    )
    write(path, content)


def update_scope_policy() -> None:
    path = "affected-scope-policy.json"
    policy = json.loads(read(path))
    operations = next(scope for scope in policy["scopes"] if scope["id"] == "operations")
    patterns = operations["path_patterns"]
    if "scripts/local_demo.py" in patterns:
        raise RuntimeError("operations scope already contains scripts/local_demo.py")
    patterns.append("scripts/local_demo.py")
    patterns.sort()
    write(path, json.dumps(policy, indent=2) + "\n")


def update_orientation_docs() -> None:
    path = "README.md"
    content = read(path)
    content = replace_exact(
        content,
        "python scripts/repo.py dev-reset --dry-run\npython scripts/repo.py dev-reset\npython scripts/repo.py architecture",
        "python scripts/repo.py dev-reset --dry-run\npython scripts/repo.py dev-reset\npython scripts/repo.py seed-demo --dry-run\npython scripts/repo.py seed-demo\npython scripts/repo.py smoke --dry-run\npython scripts/repo.py smoke\npython scripts/repo.py architecture",
        "README demo commands",
    )
    content = replace_exact(
        content,
        "`dev-up` creates or reuses one checkout-scoped PostgreSQL dependency plane from an immutable PostgreSQL 17 image digest, binds it only to `127.0.0.1`, applies every committed up migration and the accepted platform fixtures through Docker-native `psql`, and refuses partial, foreign or drifted resources. `dev-reset` verifies ownership before deleting only that checkout's container and volume and then recreates a clean ready database; use `--dry-run` before destructive reset.\n\nSpecialized contract, database, process, migration and product-plane gates remain mandatory when their scopes are affected. Repository Step 18 remains in progress: `seed-demo` and `smoke` are not yet implemented or accepted, and `dev-up`/`dev-reset` do not start backend or frontend processes.",
        "`dev-up` creates or reuses one checkout-scoped PostgreSQL dependency plane from an immutable PostgreSQL 17 image digest, binds it only to `127.0.0.1`, applies every committed up migration and the accepted platform fixtures through Docker-native `psql`, and refuses partial, foreign or drifted resources. `dev-reset` verifies ownership before deleting only that checkout's container and volume and then recreates a clean ready database; use `--dry-run` before destructive reset.\n\n`seed-demo` reuses that owned database and creates or idempotently replays the versioned `local-demo-acme` organization through the governed production Party mutation gateway. `smoke` starts the real `crm-api`, proves an authenticated query is denied without a live grant, then verifies the bootstrap-granted Party read, missing-authentication denial and tenant-B non-disclosure. Both commands use one exact locked Rust process-test target; neither starts a frontend or writes business tables directly.\n\nSpecialized contract, database, process, migration and product-plane gates remain mandatory when their scopes are affected. This implementation packet completes the command surface required by Repository Step 18, but Step 18 remains in progress until the implementation is merged and its exact acceptance evidence is synchronized separately.",
        "README demo lifecycle explanation",
    )
    write(path, content)

    path = "AGENTS.md"
    content = read(path)
    content = replace_exact(
        content,
        "python scripts/repo.py dev-reset --dry-run\npython scripts/repo.py dev-reset\npython scripts/repo.py conformance",
        "python scripts/repo.py dev-reset --dry-run\npython scripts/repo.py dev-reset\npython scripts/repo.py seed-demo --dry-run\npython scripts/repo.py seed-demo\npython scripts/repo.py smoke --dry-run\npython scripts/repo.py smoke\npython scripts/repo.py conformance",
        "AGENTS demo commands",
    )
    content = replace_exact(
        content,
        "`dev-up` and `dev-reset` manage only the checkout-scoped PostgreSQL dependency plane. They use immutable image and schema-input digests, loopback-only publishing and ownership labels. Never manually relabel a lifecycle resource; when exact configuration or schema inputs change, inspect `dev-reset --dry-run` and reset the owned state.\n\nRepository Step 18 remains in progress. `seed-demo` and `smoke` are not yet implemented or accepted; backend/frontend process startup remains outside this packet.",
        "`dev-up` and `dev-reset` manage only the checkout-scoped PostgreSQL dependency plane. They use immutable image and schema-input digests, loopback-only publishing and ownership labels. Never manually relabel a lifecycle resource; when exact configuration or schema inputs change, inspect `dev-reset --dry-run` and reset the owned state.\n\n`seed-demo` and `smoke` reuse that exact dependency plane and one locked real-process acceptance target. Demo state is created only through the governed Party mutation gateway with a stable versioned identity and idempotency key. Smoke must prove readiness, denial without a live query grant, success with the explicit bootstrap grant, authentication denial and cross-tenant non-disclosure; direct business-table writes or alternate transport paths are forbidden.\n\nThis implementation packet completes the Repository Step 18 command surface, but Step 18 is not complete until merged exact-head evidence is synchronized. Frontend/browser acceptance and Repository Step 19 remain outside this packet.",
        "AGENTS demo lifecycle explanation",
    )
    write(path, content)

    path = "docs/DEVELOPMENT_WORKFLOW.md"
    content = read(path)
    content = replace_exact(
        content,
        "python scripts/repo.py dev-reset --dry-run\npython scripts/repo.py dev-reset\n```",
        "python scripts/repo.py dev-reset --dry-run\npython scripts/repo.py dev-reset\npython scripts/repo.py seed-demo --dry-run\npython scripts/repo.py seed-demo\npython scripts/repo.py smoke --dry-run\npython scripts/repo.py smoke\n```",
        "workflow demo commands",
    )
    content = replace_exact(
        content,
        "`dev-up` creates or reuses the exact checkout-owned PostgreSQL dependency plane from pinned repository migrations and fixtures. It validates ownership, image, port, volume and schema digest before reuse. `dev-reset` first validates immutable ownership labels, removes the container before the volume, and recreates the current clean database; dry-run performs inspection only. Neither command starts product processes or seeds a demo scenario.\n\nRepository Step 18 remains in progress. `seed-demo` and `smoke` remain later bounded Step 18 work and must not be represented as implemented before permanent acceptance.",
        "`dev-up` creates or reuses the exact checkout-owned PostgreSQL dependency plane from pinned repository migrations and fixtures. It validates ownership, image, port, volume and schema digest before reuse. `dev-reset` first validates immutable ownership labels, removes the container before the volume, and recreates the current clean database; dry-run performs inspection only. Neither command starts product processes or seeds a demo scenario.\n\n`seed-demo` starts the real production composition through one locked Rust process target, applies the accepted Party production-adapter fixture, and creates or idempotently replays the versioned `local-demo-acme` organization only through `parties.party.create`. `smoke` starts a fresh real `crm-api` process, proves the authenticated Party query is denied without a live grant, then verifies the explicit bootstrap-granted read, missing-authentication denial and tenant-B non-disclosure. `kill_on_drop` plus graceful SIGINT cleanup prevents orphan local API processes.\n\nThis bounded packet completes the Repository Step 18 command surface. Step 18 remains in progress until merge and a separate exact evidence synchronization; frontend/browser acceptance and Repository Step 19 remain blocked.",
        "workflow demo lifecycle explanation",
    )
    write(path, content)


def write_packet() -> None:
    packet = {
        "schema_version": "crm.repository-packet/v1",
        "packet_id": PACKET_ID,
        "title": "Add deterministic seed-demo and real-process smoke",
        "status": "active",
        "baseline": {"ref": "main", "sha": BASELINE},
        "tracking_issues": [194],
        "objective": (
            "Complete the final bounded Repository Step 18 implementation slice with a versioned, "
            "idempotent demo dataset and a real crm-api smoke path that reuse the accepted checkout-owned "
            "PostgreSQL lifecycle, preserve production authorization/tenant boundaries, and remain permanently "
            "accepted without starting frontend or Repository Step 19 work."
        ),
        "allowed_paths": ALLOWED_PATHS,
        "forbidden_paths": [
            ".github/workflows/temporary-**",
            "Cargo.lock",
            "Cargo.toml",
            "apps/**",
            "contracts/**",
            "crates/**",
            "database/**",
            "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md",
            "docs/IMPLEMENTATION_ROADMAP.md",
            "docs/MODULE_CATALOG.md",
            "docs/PHASE8_DELIVERY_PLAN.md",
            "docs/PROJECT_STATUS.md",
            "docs/WORKSPACE_COMPLEXITY_BASELINE.md",
            "evidence/**",
            "modules/**",
            "package.json",
            "packages/**",
            "pnpm-lock.yaml",
            "proto/**",
            "requirements-dev.txt",
            "rust-toolchain.toml",
            "schemas/**",
            "services/crm-api/src/**",
        ],
        "deliverables": [
            "expose stable seed-demo and smoke commands with human, JSON and mutation-free dry-run output",
            "create or idempotently replay the versioned local-demo-acme Party only through the governed production mutation gateway",
            "prove real crm-api readiness, denial without a live query grant, granted authenticated read, authentication denial and tenant isolation",
            "reuse the accepted checkout-owned PostgreSQL 17 dependency plane and exact Cargo lockfile without direct business-table writes",
            "run clean reset, seed, exact replay and smoke as permanent Governance acceptance",
            "document the command contract without marking Repository Step 18 complete before post-merge evidence synchronization",
        ],
        "required_checks": REQUIRED_CHECKS,
        "acceptance": [
            f"the branch is based exactly on merge commit {BASELINE}",
            "two consecutive seed-demo executions use one stable idempotency key and produce one governed demo Party",
            "smoke starts the real crm-api, verifies readiness and proves permission, authentication and tenant negative paths",
            "all spawned crm-api processes use kill-on-drop and graceful shutdown cleanup",
            "dry-run executes neither Docker mutation nor cargo process execution and does not expose admin credentials",
            "the final diff contains exactly the thirteen declared permanent files and no temporary workflow or synchronizer",
            "one unchanged exact head passes every applicable permanent workflow with zero unresolved comments, reviews or review threads",
        ],
        "non_goals": [
            "mark Repository Step 18 complete or start Repository Step 19",
            "add frontend or browser acceptance",
            "change business contracts, schemas, migrations, dependencies, lockfiles or production runtime source",
            "write Party business tables directly or introduce a second PostgreSQL lifecycle",
            "claim Phase 8A, an expert module or architecture 10/10 complete",
        ],
    }
    write("repository-packet.json", json.dumps(packet, indent=2) + "\n")


def update_local_demo_tests() -> None:
    path = "tests/test_local_demo.py"
    content = read(path)
    content = replace_exact(
        content,
        "from scripts.local_demo import (",
        "from scripts.local_demo import (",
        "local demo import anchor",
    )
    content = replace_exact(
        content,
        "    smoke,\n)\n",
        "    smoke,\n)\nfrom scripts.repo import build_parser\n",
        "local demo parser import",
    )
    parser_test = '''    def test_repository_parser_exposes_demo_commands(self) -> None:
        parser = build_parser()
        seed = parser.parse_args(["seed-demo", "--dry-run", "--json"])
        self.assertEqual(seed.command, "seed-demo")
        self.assertTrue(seed.dry_run)
        self.assertTrue(seed.json)
        verify = parser.parse_args(["smoke", "--dry-run", "--json"])
        self.assertEqual(verify.command, "smoke")
        self.assertTrue(verify.dry_run)
        self.assertTrue(verify.json)

'''
    content = replace_exact(
        content,
        "    def test_invalid_mode_is_rejected_before_preparation(self) -> None:\n",
        parser_test + "    def test_invalid_mode_is_rejected_before_preparation(self) -> None:\n",
        "local demo parser test",
    )
    write(path, content)


def update_architecture_guard() -> None:
    path = "tests/test_architecture_documentation_consistency.py"
    content = read(path)
    replacement = f'''    def test_active_step_18_seed_demo_smoke_packet_is_exact(self) -> None:
        self.assertEqual(self.packet["schema_version"], "crm.repository-packet/v1")
        self.assertEqual(self.packet["packet_id"], "{PACKET_ID}")
        self.assertEqual(self.packet["status"], "active")
        self.assertEqual(
            self.packet["baseline"],
            {{"ref": "main", "sha": "{BASELINE}"}},
        )
        self.assertEqual(self.packet["tracking_issues"], [194])
        self.assertEqual(set(self.packet["allowed_paths"]), {set(ALLOWED_PATHS)!r})
        self.assertEqual(self.packet["required_checks"], {REQUIRED_CHECKS!r})
        self.assertIn(self.packet["packet_id"], self.active_packet)
        self.assertIn(self.packet["baseline"]["sha"], self.active_packet)
        self.assertIn(
            "run clean reset, seed, exact replay and smoke as permanent Governance acceptance",
            self.packet["deliverables"],
        )
        self.assertIn(
            "mark Repository Step 18 complete or start Repository Step 19",
            self.packet["non_goals"],
        )
'''
    content = replace_method(
        content,
        "test_active_step_18_dev_up_reset_evidence_sync_packet_is_exact",
        replacement,
    )
    write(path, content)


def update_navigation_guard() -> None:
    path = "tests/test_repository_navigation.py"
    content = read(path)
    replacement = f'''    def test_active_packet_declaration_is_exact(self) -> None:
        packet = load_packet(ROOT)
        self.assertEqual(packet["schema_version"], "crm.repository-packet/v1")
        self.assertEqual(packet["packet_id"], "{PACKET_ID}")
        self.assertEqual(packet["status"], "active")
        self.assertEqual(
            packet["baseline"],
            {{"ref": "main", "sha": "{BASELINE}"}},
        )
        self.assertEqual(packet["tracking_issues"], [194])
        self.assertEqual(set(packet["allowed_paths"]), {set(ALLOWED_PATHS)!r})
        self.assertEqual(packet["required_checks"], {REQUIRED_CHECKS!r})
        self.assertIn(
            "run clean reset, seed, exact replay and smoke as permanent Governance acceptance",
            packet["deliverables"],
        )
        self.assertIn(
            "mark Repository Step 18 complete or start Repository Step 19",
            packet["non_goals"],
        )
'''
    content = replace_method(content, "test_active_packet_declaration_is_exact", replacement)
    content = content.replace("repository-step-18-dev-up-reset-evidence-sync", PACKET_ID)
    content = content.replace("21e2f73b57d2c35c16eccc15ee3e075e818f488a", BASELINE)
    content = content.replace(
        "Step 18 dev-up/dev-reset evidence synchronization",
        "Step 18 seed-demo/smoke implementation",
    )
    map_anchor = '            "Application Runtime CI": ".github/workflows/application-runtime.yml",\n'
    additions = '''            "Customer Privacy Approval CI": ".github/workflows/customer-privacy-approval.yml",
            "Customer Privacy Persistence CI": ".github/workflows/customer-privacy-persistence.yml",
            "Customer Privacy Restriction Policy CI": ".github/workflows/customer-privacy-restriction-policy.yml",
            "Data Quality Process Runtime CI": ".github/workflows/data-quality-process-runtime.yml",
            "Export Process Runtime CI": ".github/workflows/export-process-runtime.yml",
            "Generic Mutation Query Conformance CI": ".github/workflows/generic-mutation-query-conformance.yaml",
            "Import Process Runtime CI": ".github/workflows/import-process-runtime.yml",
            "Import Retryable Process Runtime CI": ".github/workflows/import-retryable-process-runtime.yml",
'''
    content = replace_exact(
        content,
        map_anchor,
        map_anchor + additions,
        "navigation workflow map additions",
    )
    content = replace_exact(
        content,
        '        self.assertGreaterEqual(governance.count(\'"scripts/local_lifecycle.py"\'), 2)\n',
        '        self.assertGreaterEqual(governance.count(\'"scripts/local_lifecycle.py"\'), 2)\n'
        '        self.assertGreaterEqual(governance.count(\'"scripts/local_demo.py"\'), 2)\n'
        '        self.assertGreaterEqual(governance.count(\'"tests/test_local_demo.py"\'), 2)\n',
        "navigation governance demo paths",
    )
    content = replace_exact(
        content,
        '        self.assertIn("python scripts/repo.py conformance", governance)\n',
        '        self.assertIn("python scripts/repo.py conformance", governance)\n'
        '        self.assertIn("Run deterministic local demo seed replay and smoke acceptance", governance)\n',
        "navigation governance demo acceptance",
    )
    write(path, content)


def verify() -> None:
    packet = json.loads(read("repository-packet.json"))
    if packet["required_checks"] != REQUIRED_CHECKS:
        raise RuntimeError("required check order drifted")
    if set(packet["allowed_paths"]) != set(ALLOWED_PATHS):
        raise RuntimeError("allowed path contract drifted")
    for path in ("README.md", "AGENTS.md", "docs/DEVELOPMENT_WORKFLOW.md"):
        content = read(path)
        for marker in ("seed-demo", "smoke", "local-demo-acme"):
            if marker not in content:
                raise RuntimeError(f"{path}: missing {marker}")
    if "tests/test_local_demo.py" not in read("scripts/repo.py"):
        raise RuntimeError("conformance does not include local demo unit tests")


def main() -> None:
    update_repo_command_surface()
    update_scope_policy()
    update_orientation_docs()
    write_packet()
    update_local_demo_tests()
    update_architecture_guard()
    update_navigation_guard()
    verify()
    subprocess.run(
        ["python", "scripts/generate_repository_navigation.py", "--write"],
        cwd=ROOT,
        check=True,
    )


if __name__ == "__main__":
    main()
