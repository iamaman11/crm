#!/usr/bin/env python3
"""One-shot fail-closed synchronization for accepted Step 18 dev-up/dev-reset evidence."""

from __future__ import annotations

import json
from pathlib import Path
import re
import subprocess

ROOT = Path(__file__).resolve().parents[1]

PR281_SOURCE = "76a0d93d594b6ffbb890f90d3cb9037febf4c3f8"
PR281_MERGE = "87bc0d33befc4525b62aa7e0e1884abc07e12abf"
PR283_SOURCE = "5f4480fafd37cc8c89df60f3688e756d7f881af8"
PR283_MERGE = "21e2f73b57d2c35c16eccc15ee3e075e818f488a"
PACKET_ID = "repository-step-18-dev-up-reset-evidence-sync"

NORMATIVE = (
    "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md",
    "docs/IMPLEMENTATION_ROADMAP.md",
    "docs/MODULE_CATALOG.md",
    "docs/PHASE8_DELIVERY_PLAN.md",
    "docs/PROJECT_STATUS.md",
)

OLD_PARAGRAPH = (
    "Repository Step 18 is in progress through accepted PR #281 / source "
    "`76a0d93d594b6ffbb890f90d3cb9037febf4c3f8` / squash merge "
    "`87bc0d33befc4525b62aa7e0e1884abc07e12abf` / 7 of 7 applicable permanent "
    "workflows on one unchanged exact head. The accepted first slice delivers deterministic "
    "repository-pinned `doctor` and locked isolated `bootstrap`; the next permitted bounded "
    "implementation packet is `dev-up` and `dev-reset`, while `seed-demo` and `smoke` remain "
    "later Step 18 slices."
)

NEW_PARAGRAPH = (
    "Repository Step 18 is in progress through accepted PRs #281 and #283: PR #281 / source "
    f"`{PR281_SOURCE}` / squash merge `{PR281_MERGE}` / 7 of 7; PR #283 / source "
    f"`{PR283_SOURCE}` / squash merge `{PR283_MERGE}` / 7 of 7 applicable permanent "
    "workflows, each on one unchanged exact head. The accepted slices deliver deterministic "
    "repository-pinned `doctor`, locked isolated `bootstrap`, and checkout-owned PostgreSQL "
    "`dev-up` / `dev-reset` with immutable image pinning, schema-digest reuse, fail-closed "
    "ownership checks and permanent real-Docker create/reuse/reset acceptance. The next "
    "permitted bounded implementation packet is `seed-demo` and `smoke`; Repository Step 19 "
    "remains blocked."
)


def read(path: str) -> str:
    return (ROOT / path).read_text(encoding="utf-8")


def write(path: str, content: str) -> None:
    (ROOT / path).write_text(content, encoding="utf-8")


def replace_exact(
    content: str,
    old: str,
    new: str,
    *,
    label: str,
    minimum: int = 1,
    maximum: int | None = None,
) -> str:
    count = content.count(old)
    if count < minimum or (maximum is not None and count > maximum):
        raise RuntimeError(
            f"{label}: expected {minimum}..{maximum or 'many'} matches, found {count}"
        )
    return content.replace(old, new)


def replace_method(content: str, name: str, replacement: str) -> str:
    pattern = re.compile(
        rf"(?ms)^    def {re.escape(name)}\(self\).*?(?=^    def |^\nif __name__)"
    )
    updated, count = pattern.subn(lambda _: replacement.rstrip() + "\n\n", content)
    if count != 1:
        raise RuntimeError(f"method {name}: expected 1 match, found {count}")
    return updated


def synchronize_normative_documents() -> None:
    for path in NORMATIVE:
        content = replace_exact(
            read(path),
            OLD_PARAGRAPH,
            NEW_PARAGRAPH,
            label=f"{path} accepted paragraph",
        )
        write(path, content)

    path = "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md"
    content = read(path)
    content = replace_exact(
        content,
        "Current blocking baseline after the accepted Step 18 doctor/bootstrap slice:",
        "Current blocking baseline after the accepted Step 18 doctor/bootstrap and dev-up/dev-reset slices:",
        label="architecture baseline heading",
        maximum=1,
    )
    content = replace_exact(
        content,
        "| H — reproducible environment and navigation | **In progress** | `affected`, `check-affected`, `explain`, fail-closed `packet-check`, generated active packet/repository map, plus accepted deterministic `doctor` and locked `bootstrap` through PR #281 | deterministic `dev-up`, `dev-reset`, `seed-demo` and `smoke` in the remaining Step 18 slices |",
        "| H — reproducible environment and navigation | **In progress** | `affected`, `check-affected`, `explain`, fail-closed `packet-check`, generated active packet/repository map, plus accepted deterministic `doctor`, locked `bootstrap` and checkout-owned PostgreSQL `dev-up` / `dev-reset` through PRs #281 and #283 | deterministic `seed-demo` and `smoke` in the remaining Step 18 slice |",
        label="architecture Stage H row",
        maximum=1,
    )
    content = replace_exact(
        content,
        "18. deterministic local lifecycle commands: `doctor`, `bootstrap`, `dev-up`, `dev-reset`, `seed-demo` and `smoke` — **in progress through PR #281; doctor/bootstrap accepted, dev-up/dev-reset next**;",
        "18. deterministic local lifecycle commands: `doctor`, `bootstrap`, `dev-up`, `dev-reset`, `seed-demo` and `smoke` — **in progress through PRs #281 and #283; doctor/bootstrap/dev-up/dev-reset accepted, seed-demo/smoke next**;",
        label="architecture Step 18 row",
        maximum=1,
    )
    content = replace_exact(
        content,
        "Step 18 must add deterministic clean-machine commands:\n\n- `python scripts/repo.py doctor`;\n- `python scripts/repo.py bootstrap`;\n- `python scripts/repo.py dev-up`;\n- `python scripts/repo.py dev-reset`;\n- `python scripts/repo.py seed-demo`;\n- `python scripts/repo.py smoke`.\n\nThey must be pinned, repeatable, safe, production-aligned and proven on a clean environment.",
        "Repository Step 18 has accepted these deterministic clean-machine commands through PRs #281 and #283:\n\n- `python scripts/repo.py doctor`;\n- `python scripts/repo.py bootstrap`;\n- `python scripts/repo.py dev-up`;\n- `python scripts/repo.py dev-reset`.\n\nThe remaining bounded Step 18 packet must add:\n\n- `python scripts/repo.py seed-demo`;\n- `python scripts/repo.py smoke`.\n\nEvery lifecycle command must remain pinned, repeatable, safe, production-aligned and proven on a clean environment.",
        label="architecture lifecycle command ledger",
        maximum=1,
    )
    content = replace_exact(
        content,
        "The combined evidence keeps `activities.task.create@1.1.0` as the sole live create coordinate, preserves the ordinary production zero-usage path for released contracts and fabricates no production history. Repository Step 18 is the next permitted implementation packet and is not started.",
        "The combined evidence keeps `activities.task.create@1.1.0` as the sole live create coordinate, preserves the ordinary production zero-usage path for released contracts and fabricates no production history. " + NEW_PARAGRAPH,
        label="architecture Step 17 transition",
        maximum=1,
    )
    write(path, content)

    path = "docs/IMPLEMENTATION_ROADMAP.md"
    content = read(path)
    content = replace_exact(
        content,
        "Repository Steps 1–17 are complete. Repository Step 18 is **in progress through accepted PR #281**; doctor/bootstrap are accepted and dev-up/dev-reset are the next permitted bounded implementation packet.",
        "Repository Steps 1–17 are complete. Repository Step 18 is **in progress through accepted PRs #281 and #283**; doctor/bootstrap/dev-up/dev-reset are accepted and seed-demo/smoke are the next permitted bounded implementation packet.",
        label="roadmap current Step 18",
        maximum=1,
    )
    content = replace_exact(
        content,
        "### 3.5 Accepted Repository Step 18 doctor/bootstrap slice",
        "### 3.5 Accepted Repository Step 18 lifecycle slices",
        label="roadmap Step 18 heading",
        maximum=1,
    )
    content = replace_exact(
        content,
        "The slice changes no product behavior or runtime ownership. Step 18 remains open until dev-up, dev-reset, seed-demo and smoke are separately accepted.",
        "The accepted slices change no product behavior or runtime ownership. Step 18 remains open until seed-demo and smoke are accepted on a clean environment.",
        label="roadmap Step 18 closure sentence",
        maximum=1,
    )
    content = replace_exact(
        content,
        "18. deterministic local lifecycle commands — **in progress through PR #281; doctor/bootstrap accepted, dev-up/dev-reset next**;",
        "18. deterministic local lifecycle commands — **in progress through PRs #281 and #283; doctor/bootstrap/dev-up/dev-reset accepted, seed-demo/smoke next**;",
        label="roadmap sequence Step 18",
        maximum=1,
    )
    write(path, content)

    path = "docs/MODULE_CATALOG.md"
    content = read(path)
    content = replace_exact(
        content,
        f"- PR #281 / accepted source `{PR281_SOURCE}` / merge `{PR281_MERGE}` / 7 of 7 permanent workflows — deterministic repository-pinned doctor and locked isolated bootstrap, with no module readiness or product behavior change.",
        f"- PR #281 / accepted source `{PR281_SOURCE}` / merge `{PR281_MERGE}` / 7 of 7 permanent workflows — deterministic repository-pinned doctor and locked isolated bootstrap, with no module readiness or product behavior change.\n- PR #283 / accepted source `{PR283_SOURCE}` / merge `{PR283_MERGE}` / 7 of 7 permanent workflows — checkout-owned PostgreSQL dev-up/dev-reset, immutable image and schema digest, fail-closed ownership/reset semantics and real-Docker create/reuse/reset acceptance, with no module readiness or product behavior change.",
        label="catalog Step 18 evidence bullets",
        maximum=1,
    )
    content = replace_exact(
        content,
        "Repository Step 17 is complete through PR #279; Repository Step 18 is in progress through PR #281, with doctor/bootstrap accepted and dev-up/dev-reset next.",
        "Repository Step 17 is complete through PR #279; Repository Step 18 is in progress through PRs #281 and #283, with doctor/bootstrap/dev-up/dev-reset accepted and seed-demo/smoke next.",
        label="catalog current Step 18 summary",
        maximum=1,
    )
    write(path, content)

    path = "docs/PHASE8_DELIVERY_PLAN.md"
    content = read(path)
    content = replace_exact(
        content,
        "### Repository Step 18 — in progress through PR #281\n\n- accepted through PR #281: deterministic repository-pinned `doctor` and locked isolated `bootstrap`;\n- next bounded packet: deterministic `dev-up` and `dev-reset`;\n- later Step 18 slices: `seed-demo` and end-to-end `smoke` on a clean environment.",
        "### Repository Step 18 — in progress through PRs #281 and #283\n\n- accepted through PR #281: deterministic repository-pinned `doctor` and locked isolated `bootstrap`;\n- accepted through PR #283: checkout-owned PostgreSQL `dev-up` and `dev-reset` with immutable image pinning, schema-digest reuse, fail-closed ownership/reset semantics and permanent real-Docker acceptance;\n- next bounded packet: deterministic `seed-demo` and end-to-end `smoke` on a clean environment;\n- Repository Step 19 remains blocked until Step 18 closes.",
        label="phase8 Step 18 section",
        maximum=1,
    )
    write(path, content)

    path = "docs/PROJECT_STATUS.md"
    content = read(path)
    content = replace_exact(
        content,
        "Status date: 2026-08-03",
        "Status date: 2026-08-04",
        label="project status date",
        maximum=1,
    )
    content = replace_exact(
        content,
        "Repository Steps 1–17 are complete. Repository Step 18 is in progress through the accepted doctor/bootstrap slice in PR #281.",
        "Repository Steps 1–17 are complete. Repository Step 18 is in progress through accepted PRs #281 and #283; doctor/bootstrap/dev-up/dev-reset are accepted and seed-demo/smoke are next.",
        label="project current position",
        maximum=1,
    )
    content = replace_exact(
        content,
        "## Repository Step 18 accepted doctor/bootstrap slice",
        "## Repository Step 18 accepted lifecycle slices",
        label="project Step 18 heading",
        maximum=1,
    )
    old_behavior = """Accepted behavior:

- `repo.py doctor` provides deterministic human and JSON output, repository-pinned Rust/Node/pnpm validation, Python venv validation and actionable fail-closed remediation;
- the bootstrap profile excludes Docker, while the full profile additionally validates Docker CLI, Compose v2 and daemon reachability;
- `repo.py bootstrap` creates an isolated `.venv`, installs committed Python constraints, uses Cargo `--locked` and pnpm `--frozen-lockfile`, and verifies locked metadata plus generated navigation;
- dry-run executes no command and reports the exact ordered argument-array plan;
- no runtime, owner, route, worker, contract, schema, migration, dependency, lockfile or product behavior changed."""
    new_behavior = """Accepted behavior:

- `repo.py doctor` provides deterministic human and JSON output, repository-pinned Rust/Node/pnpm validation, Python venv validation and actionable fail-closed remediation;
- the bootstrap profile excludes Docker, while the full profile additionally validates Docker CLI, Compose v2 and daemon reachability;
- `repo.py bootstrap` creates an isolated `.venv`, installs committed Python constraints, uses Cargo `--locked` and pnpm `--frozen-lockfile`, and verifies locked metadata plus generated navigation;
- `repo.py dev-up` creates or reuses an immutable PostgreSQL 17 dependency plane with checkout-scoped ownership labels, loopback-only publishing, ordered migrations/fixtures and a deterministic schema digest;
- `repo.py dev-reset` verifies ownership before removing only the owned container and volume, then recreates clean state;
- permanent real-Docker acceptance proves create, marker persistence, unchanged reuse, destructive reset, pre-reset probe removal and CRM schema restoration;
- dry-run executes no command and reports the exact ordered argument-array plan;
- no runtime, owner, route, worker, contract, schema, migration, dependency, lockfile or product behavior changed."""
    content = replace_exact(
        content,
        old_behavior,
        new_behavior,
        label="project accepted behavior",
        maximum=1,
    )
    content = replace_exact(
        content,
        f"| 18 (doctor/bootstrap slice) | PR #281 / source `{PR281_SOURCE}` / merge `{PR281_MERGE}` / 7 of 7 | Deterministic repository-pinned doctor and locked isolated bootstrap; Step 18 remains in progress with dev-up/dev-reset next |",
        f"| 18 (lifecycle slices) | PR #281 / source `{PR281_SOURCE}` / merge `{PR281_MERGE}` / 7 of 7; PR #283 / source `{PR283_SOURCE}` / merge `{PR283_MERGE}` / 7 of 7 | Deterministic doctor/bootstrap and checkout-owned PostgreSQL dev-up/dev-reset; Step 18 remains in progress with seed-demo/smoke next |",
        label="project evidence table Step 18",
        maximum=1,
    )
    content = replace_exact(
        content,
        "- **Stage H — in progress:** explanation, packet checking and generated navigation exist; deterministic local lifecycle commands remain.",
        "- **Stage H — in progress:** explanation, packet checking, generated navigation, doctor, bootstrap, dev-up and dev-reset are accepted; deterministic seed-demo and smoke remain.",
        label="project Stage H",
        maximum=1,
    )
    content = replace_exact(
        content,
        "Repository Step 18 is the next permitted implementation packet and is **not started**. Its bounded scope is deterministic local lifecycle commands: `doctor`, `bootstrap`, `dev-up`, `dev-reset`, `seed-demo` and `smoke` on a clean environment.",
        "The next permitted implementation packet is the remaining bounded Repository Step 18 slice: deterministic `seed-demo` and end-to-end `smoke` on a clean environment. Doctor, bootstrap, dev-up and dev-reset are already accepted through PRs #281 and #283.",
        label="project next packet",
        maximum=1,
    )
    old_order = """```text
1–16. accepted and complete
-> 17. contract compatibility, deprecation, consumer migration and retirement enforcement — next, not started
-> 18. deterministic local lifecycle commands
-> 19. Customer Privacy worker and complete process/end-to-end acceptance"""
    new_order = """```text
1–17. accepted and complete
-> 18. deterministic local lifecycle commands — in progress; doctor/bootstrap/dev-up/dev-reset accepted, seed-demo/smoke next
-> 19. Customer Privacy worker and complete process/end-to-end acceptance"""
    content = replace_exact(
        content,
        old_order,
        new_order,
        label="project continuation order",
        maximum=1,
    )
    write(path, content)


def write_packet() -> None:
    packet = {
        "schema_version": "crm.repository-packet/v1",
        "packet_id": PACKET_ID,
        "title": "Synchronize accepted Step 18 dev-up and dev-reset evidence",
        "status": "active",
        "baseline": {"ref": "main", "sha": PR283_MERGE},
        "tracking_issues": [194],
        "objective": (
            "Record the exact accepted PR #283 source, squash merge and 7-of-7 workflow evidence "
            "across every live normative source; keep Repository Step 18 in progress rather than "
            "complete; and make seed-demo/smoke the only next permitted implementation packet "
            "without changing runtime or product behavior."
        ),
        "allowed_paths": [
            "docs/ACTIVE_PACKET.md",
            "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md",
            "docs/IMPLEMENTATION_ROADMAP.md",
            "docs/MODULE_CATALOG.md",
            "docs/PHASE8_DELIVERY_PLAN.md",
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
            "database/**",
            "evidence/**",
            "modules/**",
            "package.json",
            "packages/**",
            "pnpm-lock.yaml",
            "proto/**",
            "requirements-dev.txt",
            "rust-toolchain.toml",
            "schemas/**",
            "scripts/**",
            "services/**",
        ],
        "deliverables": [
            "record exact PR #283 source, squash merge and 7-of-7 workflow evidence in all five normative documents",
            "mark Repository Step 18 in progress with doctor/bootstrap/dev-up/dev-reset accepted and seed-demo/smoke next",
            "keep Repository Step 19 blocked until the remaining Step 18 slice is accepted",
            "update active-packet and documentation-consistency guards to the accepted evidence-sync packet",
            "record the accepted dev-up/dev-reset zero-impact result in the historical workspace complexity baseline",
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
            f"the branch is based exactly on merge commit {PR283_MERGE}",
            f"all five normative documents contain PR #283, source {PR283_SOURCE}, merge {PR283_MERGE} and 7 of 7",
            "Repository Step 18 is described as in progress rather than complete",
            "seed-demo/smoke are the only next permitted implementation packet and Repository Step 19 remains blocked",
            "the final diff contains only declared documentation, packet, generated navigation and permanent guard files",
            "Complexity Baseline CI is applicable and succeeds on the same final exact head through the declared complexity-baseline evidence path",
            "one unchanged exact head passes every applicable permanent workflow with zero unresolved comments, reviews or review threads",
            "the exact final head passes both Customer Privacy process gates selected by the permanent historical-evidence preflight",
        ],
        "non_goals": [
            "implement seed-demo or smoke",
            "start Repository Step 19 or change Customer Privacy worker behavior",
            "change runtime, routes, contracts, schemas, migrations, dependencies, lockfiles or product behavior",
            "complete Repository Step 18, Phase 8A, an expert module or architecture 10/10",
        ],
    }
    write("repository-packet.json", json.dumps(packet, indent=2) + "\n")


def update_complexity_evidence() -> None:
    path = "docs/WORKSPACE_COMPLEXITY_BASELINE.md"
    content = read(path)
    old = (
        "## Accepted Repository Step 18 doctor/bootstrap complexity non-effect\n\n"
        f"PR #281 / accepted source `{PR281_SOURCE}` / squash merge `{PR281_MERGE}` / 7 of 7 applicable permanent workflows added deterministic repository-pinned `doctor` and locked isolated `bootstrap`. The accepted slice changed no workspace package, dependency declaration, feature set, internal edge, public Rust surface, Cargo manifest or `Cargo.lock`; this file remains a historical Stage B measurement baseline rather than a replacement for current generated complexity artifacts."
    )
    new = (
        "## Accepted Repository Step 18 lifecycle complexity non-effect\n\n"
        f"PR #281 / accepted source `{PR281_SOURCE}` / squash merge `{PR281_MERGE}` / 7 of 7 applicable permanent workflows added deterministic repository-pinned `doctor` and locked isolated `bootstrap`. PR #283 / accepted source `{PR283_SOURCE}` / squash merge `{PR283_MERGE}` / 7 of 7 applicable permanent workflows added checkout-owned PostgreSQL `dev-up` / `dev-reset`, immutable image and schema-digest checks, fail-closed ownership/reset semantics and permanent real-Docker create/reuse/reset acceptance. The accepted slices changed no workspace package, dependency declaration, feature set, internal edge, public Rust surface, Cargo manifest or `Cargo.lock`; this file remains a historical Stage B measurement baseline rather than a replacement for current generated complexity artifacts."
    )
    content = replace_exact(
        content,
        old,
        new,
        label="complexity Step 18 evidence",
        maximum=1,
    )
    write(path, content)


def update_architecture_guard() -> None:
    path = "tests/test_architecture_documentation_consistency.py"
    content = read(path)
    content = replace_exact(
        content,
        '            "ea3d3894d05e3a8d814aff69824b593843763d03",\n',
        '            "ea3d3894d05e3a8d814aff69824b593843763d03",\n'
        '            "PR #283",\n'
        f'            "{PR283_SOURCE}",\n'
        f'            "{PR283_MERGE}",\n'
        '            "7 of 7",\n',
        label="architecture guard exact evidence tuple",
        maximum=1,
    )
    content = replace_exact(
        content,
        '        self.assertIn("Repository Step 18 is in progress through accepted PR #281", self.status)\n',
        '        self.assertIn("Repository Step 18 is in progress through accepted PRs #281 and #283", self.status)\n',
        label="architecture guard status assertion",
        maximum=1,
    )
    content = replace_exact(
        content,
        '        self.assertIn("**in progress through PR #281; doctor/bootstrap accepted, dev-up/dev-reset next**", self.plan)\n',
        '        self.assertIn("**in progress through PRs #281 and #283; doctor/bootstrap/dev-up/dev-reset accepted, seed-demo/smoke next**", self.plan)\n',
        label="architecture guard plan assertion",
        maximum=1,
    )
    replacement = f'''    def test_active_step_18_dev_up_reset_evidence_sync_packet_is_exact(self) -> None:
        self.assertEqual(self.packet["schema_version"], "crm.repository-packet/v1")
        self.assertEqual(self.packet["packet_id"], "{PACKET_ID}")
        self.assertEqual(self.packet["status"], "active")
        self.assertEqual(
            self.packet["baseline"],
            {{"ref": "main", "sha": "{PR283_MERGE}"}},
        )
        self.assertEqual(self.packet["tracking_issues"], [194])
        self.assertEqual(
            set(self.packet["allowed_paths"]),
            {{
                "docs/ACTIVE_PACKET.md",
                "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md",
                "docs/IMPLEMENTATION_ROADMAP.md",
                "docs/MODULE_CATALOG.md",
                "docs/PHASE8_DELIVERY_PLAN.md",
                "docs/PROJECT_STATUS.md",
                "docs/WORKSPACE_COMPLEXITY_BASELINE.md",
                "repository-packet.json",
                "tests/test_architecture_documentation_consistency.py",
                "tests/test_repository_navigation.py",
            }},
        )
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
        self.assertIn(
            "record exact PR #283 source, squash merge and 7-of-7 workflow evidence in all five normative documents",
            self.packet["deliverables"],
        )
        self.assertIn("implement seed-demo or smoke", self.packet["non_goals"])
'''
    content = replace_method(
        content,
        "test_active_step_18_dev_up_reset_packet_is_exact",
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
            {{"ref": "main", "sha": "{PR283_MERGE}"}},
        )
        self.assertEqual(packet["tracking_issues"], [194])
        self.assertEqual(
            set(packet["allowed_paths"]),
            {{
                "docs/ACTIVE_PACKET.md",
                "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md",
                "docs/IMPLEMENTATION_ROADMAP.md",
                "docs/MODULE_CATALOG.md",
                "docs/PHASE8_DELIVERY_PLAN.md",
                "docs/PROJECT_STATUS.md",
                "docs/WORKSPACE_COMPLEXITY_BASELINE.md",
                "repository-packet.json",
                "tests/test_architecture_documentation_consistency.py",
                "tests/test_repository_navigation.py",
            }},
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
        self.assertIn(
            "record exact PR #283 source, squash merge and 7-of-7 workflow evidence in all five normative documents",
            packet["deliverables"],
        )
        self.assertIn("implement seed-demo or smoke", packet["non_goals"])
'''
    content = replace_method(content, "test_active_packet_declaration_is_exact", replacement)
    content = replace_exact(
        content,
        '            "repository-step-18-dev-up-reset",\n',
        f'            "{PACKET_ID}",\n',
        label="navigation generated packet id",
        maximum=1,
    )
    content = replace_exact(
        content,
        f'            "dbb3e08097ff9439161ce94ff91b8912c8a0b249",\n',
        f'            "{PR283_MERGE}",\n',
        label="navigation generated baseline",
        maximum=1,
    )
    content = replace_exact(
        content,
        '                return_value="dbb3e08097ff9439161ce94ff91b8912c8a0b249",\n',
        f'                return_value="{PR283_MERGE}",\n',
        label="navigation mocked baseline",
        maximum=1,
    )
    content = replace_exact(
        content,
        '                    "reasons": ["Step 18 doctor/bootstrap local lifecycle"],\n',
        '                    "reasons": ["Step 18 dev-up/dev-reset evidence synchronization"],\n',
        label="navigation workflow reason",
        maximum=1,
    )
    write(path, content)


def verify() -> None:
    required = (
        "PR #281",
        PR281_SOURCE,
        PR281_MERGE,
        "PR #283",
        PR283_SOURCE,
        PR283_MERGE,
        "7 of 7",
        "seed-demo",
        "smoke",
    )
    for path in NORMATIVE:
        content = read(path)
        for marker in required:
            if marker not in content:
                raise RuntimeError(f"{path}: missing {marker}")
    stale = (
        "dev-up/dev-reset next",
        "Repository Step 18 is the next permitted implementation packet and is **not started**",
        "17. contract compatibility, deprecation, consumer migration and retirement enforcement — next, not started",
    )
    combined = "\n".join(read(path) for path in NORMATIVE)
    for marker in stale:
        if marker in combined:
            raise RuntimeError(f"stale Step 18 claim remains: {marker}")


def main() -> None:
    synchronize_normative_documents()
    write_packet()
    update_complexity_evidence()
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
