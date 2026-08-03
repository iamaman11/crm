#!/usr/bin/env python3
"""One-shot fail-closed synchronization for accepted Step 18 closure evidence."""

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
PR285_SOURCE = "a522b8b11a0c6f143694f516e7a7f9d522c18ce3"
PR285_MERGE = "a906f2c514285974113749b8b8ad9446202a5fa1"
PACKET_ID = "repository-step-18-seed-demo-smoke-evidence-sync"

NORMATIVE = (
    "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md",
    "docs/IMPLEMENTATION_ROADMAP.md",
    "docs/MODULE_CATALOG.md",
    "docs/PHASE8_DELIVERY_PLAN.md",
    "docs/PROJECT_STATUS.md",
)

OLD_PARAGRAPH = (
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

NEW_PARAGRAPH = (
    "Repository Step 18 is complete through accepted PRs #281, #283 and #285: PR #281 / source "
    f"`{PR281_SOURCE}` / squash merge `{PR281_MERGE}` / 7 of 7; PR #283 / source "
    f"`{PR283_SOURCE}` / squash merge `{PR283_MERGE}` / 7 of 7; PR #285 / source "
    f"`{PR285_SOURCE}` / squash merge `{PR285_MERGE}` / 19 of 19 applicable permanent "
    "workflows, each on one unchanged exact head. The accepted lifecycle delivers deterministic "
    "repository-pinned `doctor`, locked isolated `bootstrap`, checkout-owned PostgreSQL `dev-up` / "
    "`dev-reset`, versioned idempotent `seed-demo` through the governed Party mutation gateway and "
    "real-process `smoke` proving readiness, permission, authentication and tenant boundaries. The "
    "next permitted bounded implementation packet is Repository Step 19: the real Customer Privacy "
    "worker lifecycle and complete process/end-to-end acceptance."
)

ALLOWED = [
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
]
REQUIRED = [
    "Affected Scope CI",
    "Complexity Baseline CI",
    "Customer Privacy Access Export CI",
    "Customer Privacy Owner Execution CI",
    "Governance CI",
    "Rust Generated Sync",
    "Rust CI",
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


def synchronize_common_evidence() -> None:
    for path in NORMATIVE:
        content = read(path)
        count = content.count(OLD_PARAGRAPH)
        if count < 1:
            raise RuntimeError(f"{path}: accepted Step 18 paragraph not found")
        write(path, content.replace(OLD_PARAGRAPH, NEW_PARAGRAPH))


def update_architecture_plan() -> None:
    path = "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md"
    content = read(path)
    content = replace_exact(
        content,
        "Repository Step 18 has accepted these deterministic clean-machine commands through PRs #281 and #283:\n\n- `python scripts/repo.py doctor`;\n- `python scripts/repo.py bootstrap`;\n- `python scripts/repo.py dev-up`;\n- `python scripts/repo.py dev-reset`.\n\nThe remaining bounded Step 18 packet must add:\n\n- `python scripts/repo.py seed-demo`;\n- `python scripts/repo.py smoke`.\n\nEvery lifecycle command must remain pinned, repeatable, safe, production-aligned and proven on a clean environment.",
        "Repository Step 18 has accepted the complete deterministic clean-machine command surface through PRs #281, #283 and #285:\n\n- `python scripts/repo.py doctor`;\n- `python scripts/repo.py bootstrap`;\n- `python scripts/repo.py dev-up`;\n- `python scripts/repo.py dev-reset`;\n- `python scripts/repo.py seed-demo`;\n- `python scripts/repo.py smoke`.\n\nThe commands are repository-pinned, repeatable, fail closed, production-aligned and permanently proven on clean environments. `seed-demo` uses the governed Party gateway and `smoke` proves readiness plus permission, authentication and tenant negative paths through a real `crm-api` process.",
        "architecture lifecycle command closure",
    )
    content = content.replace(
        "18. deterministic local lifecycle commands — **in progress through PRs #281 and #283; doctor/bootstrap/dev-up/dev-reset accepted, seed-demo/smoke next**;",
        "18. deterministic local lifecycle commands — **complete through PR #285**;",
    )
    content = content.replace(
        "| H — reproducible environment and navigation | **In progress** |",
        "| H — reproducible environment and navigation | **Complete through PR #285** |",
    )
    content = content.replace(
        "deterministic `seed-demo` and `smoke` in the remaining Step 18 slice",
        "complete deterministic doctor/bootstrap/dev-up/dev-reset/seed-demo/smoke lifecycle accepted through PR #285",
    )
    write(path, content)


def update_roadmap() -> None:
    path = "docs/IMPLEMENTATION_ROADMAP.md"
    content = read(path)
    content = replace_exact(
        content,
        "- Stage H reproducible environment and navigation — **In progress**;",
        "- Stage H reproducible environment and navigation — **Complete through PR #285**;",
        "roadmap Stage H",
    )
    content = replace_exact(
        content,
        "Repository Steps 1–17 are complete. Repository Step 18 is **in progress through accepted PRs #281 and #283**; doctor/bootstrap/dev-up/dev-reset are accepted and seed-demo/smoke are the next permitted bounded implementation packet.",
        "Repository Steps 1–18 are complete. Repository Step 19 is the next permitted bounded implementation packet: the real Customer Privacy worker lifecycle and complete process/end-to-end acceptance.",
        "roadmap current repository position",
    )
    content = replace_exact(
        content,
        "### 3.5 Accepted Repository Step 18 lifecycle slices",
        "### 3.5 Accepted Repository Step 18 closure",
        "roadmap Step 18 heading",
    )
    content = replace_exact(
        content,
        "The accepted slices change no product behavior or runtime ownership. Step 18 remains open until seed-demo and smoke are accepted on a clean environment.",
        "The accepted Step 18 lifecycle changes no product ownership, public contract, schema, migration, dependency or lockfile. Step 18 is complete; Step 19 is next and Customer Privacy still publishes zero workers until that packet is accepted.",
        "roadmap Step 18 closure sentence",
    )
    content = content.replace(
        "18. deterministic local lifecycle commands — **in progress through PRs #281 and #283; doctor/bootstrap/dev-up/dev-reset accepted, seed-demo/smoke next**;",
        "18. deterministic local lifecycle commands — **complete through PR #285**;",
    )
    write(path, content)


def update_catalog() -> None:
    path = "docs/MODULE_CATALOG.md"
    content = read(path)
    anchor = (
        f"- PR #283 / accepted source `{PR283_SOURCE}` / merge `{PR283_MERGE}` / 7 of 7 permanent workflows — "
        "checkout-owned PostgreSQL dev-up/dev-reset, immutable image and schema digest, fail-closed ownership/reset semantics and real-Docker create/reuse/reset acceptance, with no module readiness or product behavior change."
    )
    addition = (
        anchor
        + "\n"
        + f"- PR #285 / accepted source `{PR285_SOURCE}` / merge `{PR285_MERGE}` / 19 of 19 permanent workflows — "
        "versioned idempotent local-demo-acme seeding through the governed Party gateway and real crm-api smoke proof for readiness, permission grants, authentication denial and tenant non-disclosure, with no module-readiness advancement."
    )
    content = replace_exact(content, anchor, addition, "catalog PR #285 bullet")
    content = content.replace(
        "Repository Step 17 is complete through PR #279; Repository Step 18 is in progress through PRs #281 and #283, with doctor/bootstrap/dev-up/dev-reset accepted and seed-demo/smoke next.",
        "Repository Steps 17 and 18 are complete through PRs #279 and #285 respectively; Repository Step 19, the real Customer Privacy worker lifecycle, is next.",
    )
    content = content.replace(
        "Repository Step 18 is in progress through PRs #281 and #283, with doctor/bootstrap/dev-up/dev-reset accepted and seed-demo/smoke next.",
        "Repository Step 18 is complete through PR #285; Repository Step 19 is next.",
    )
    write(path, content)


def update_phase8() -> None:
    path = "docs/PHASE8_DELIVERY_PLAN.md"
    content = read(path)
    content = content.replace(
        "### Repository Step 18 — in progress through PRs #281 and #283",
        "### Repository Step 18 — complete through PR #285",
    )
    content = content.replace(
        "- next bounded packet: deterministic `seed-demo` and end-to-end `smoke` on a clean environment;\n- Repository Step 19 remains blocked until Step 18 closes.",
        f"- accepted through PR #285 / source `{PR285_SOURCE}` / merge `{PR285_MERGE}` / 19 of 19: versioned idempotent `seed-demo` through the governed Party gateway and real-process `smoke` with permission, authentication and tenant negative proof;\n- Repository Step 19 is now the next permitted packet.",
    )
    write(path, content)


def update_project_status() -> None:
    path = "docs/PROJECT_STATUS.md"
    content = read(path)
    content = replace_exact(
        content,
        "Repository Steps 1–17 are complete. Repository Step 18 is in progress through accepted PRs #281 and #283; doctor/bootstrap/dev-up/dev-reset are accepted and seed-demo/smoke are next. Architecture Stages A, B, D, E and G are complete. Stages C, F, H and I remain incomplete or in progress according to the architecture plan.",
        "Repository Steps 1–18 are complete. Repository Step 19 is next: the real Customer Privacy worker lifecycle and complete process/end-to-end acceptance. Architecture Stages A, B, D, E, G and H are complete. Stages C, F and I remain incomplete or in progress according to the architecture plan.",
        "project current position",
    )
    content = replace_exact(
        content,
        "## Repository Step 18 accepted lifecycle slices",
        "## Repository Step 18 accepted closure",
        "project Step 18 heading",
    )
    content = replace_exact(
        content,
        "- permanent real-Docker acceptance proves create, marker persistence, unchanged reuse, destructive reset, pre-reset probe removal and CRM schema restoration;\n- dry-run executes no command and reports the exact ordered argument-array plan;",
        "- permanent real-Docker acceptance proves create, marker persistence, unchanged reuse, destructive reset, pre-reset probe removal and CRM schema restoration;\n- `repo.py seed-demo` creates or idempotently replays the versioned `local-demo-acme` organization only through the governed Party mutation gateway;\n- `repo.py smoke` starts the real `crm-api`, proves denial without a live query grant, verifies the explicit grant, rejects missing authentication and conceals tenant-A data from tenant B;\n- dry-run executes no Docker mutation or process command and reports the exact ordered argument-array plan without exposing admin credentials;",
        "project Step 18 accepted behavior",
    )
    content = content.replace(
        "| 18 (lifecycle slices) | PR #281 / source `76a0d93d594b6ffbb890f90d3cb9037febf4c3f8` / merge `87bc0d33befc4525b62aa7e0e1884abc07e12abf` / 7 of 7; PR #283 / source `5f4480fafd37cc8c89df60f3688e756d7f881af8` / merge `21e2f73b57d2c35c16eccc15ee3e075e818f488a` / 7 of 7 | Deterministic doctor/bootstrap and checkout-owned PostgreSQL dev-up/dev-reset; Step 18 remains in progress with seed-demo/smoke next |",
        f"| 18 | PR #281 / source `{PR281_SOURCE}` / merge `{PR281_MERGE}` / 7 of 7; PR #283 / source `{PR283_SOURCE}` / merge `{PR283_MERGE}` / 7 of 7; PR #285 / source `{PR285_SOURCE}` / merge `{PR285_MERGE}` / 19 of 19 | Complete deterministic doctor/bootstrap/dev-up/dev-reset/seed-demo/smoke lifecycle; Step 19 is next |",
    )
    content = content.replace(
        "- **Stage H — in progress:** explanation, packet checking, generated navigation, doctor, bootstrap, dev-up and dev-reset are accepted; deterministic seed-demo and smoke remain.",
        "- **Stage H — complete through PR #285:** explanation, packet checking, generated navigation and the full doctor/bootstrap/dev-up/dev-reset/seed-demo/smoke lifecycle are accepted.",
    )
    content = content.replace(
        "The next permitted implementation packet is the remaining bounded Repository Step 18 slice: deterministic `seed-demo` and end-to-end `smoke` on a clean environment. Doctor, bootstrap, dev-up and dev-reset are already accepted through PRs #281 and #283.",
        "The next permitted implementation packet is Repository Step 19: the real Customer Privacy worker lifecycle and complete process/end-to-end acceptance. Repository Step 18 is complete through PR #285.",
    )
    content = content.replace(
        "1–17. accepted and complete\n-> 18. deterministic local lifecycle commands — in progress; doctor/bootstrap/dev-up/dev-reset accepted, seed-demo/smoke next\n-> 19. Customer Privacy worker and complete process/end-to-end acceptance",
        "1–18. accepted and complete\n-> 19. Customer Privacy worker and complete process/end-to-end acceptance",
    )
    write(path, content)


def update_complexity() -> None:
    path = "docs/WORKSPACE_COMPLEXITY_BASELINE.md"
    content = read(path)
    old = (
        "PR #281 / accepted source `76a0d93d594b6ffbb890f90d3cb9037febf4c3f8` / squash merge `87bc0d33befc4525b62aa7e0e1884abc07e12abf` / 7 of 7 applicable permanent workflows added deterministic repository-pinned `doctor` and locked isolated `bootstrap`. PR #283 / accepted source `5f4480fafd37cc8c89df60f3688e756d7f881af8` / squash merge `21e2f73b57d2c35c16eccc15ee3e075e818f488a` / 7 of 7 applicable permanent workflows added checkout-owned PostgreSQL `dev-up` / `dev-reset`, immutable image and schema-digest checks, fail-closed ownership/reset semantics and permanent real-Docker create/reuse/reset acceptance. The accepted slices changed no workspace package, dependency declaration, feature set, internal edge, public Rust surface, Cargo manifest or `Cargo.lock`; this file remains a historical Stage B measurement baseline rather than a replacement for current generated complexity artifacts."
    )
    new = (
        old[:-len(" The accepted slices changed no workspace package, dependency declaration, feature set, internal edge, public Rust surface, Cargo manifest or `Cargo.lock`; this file remains a historical Stage B measurement baseline rather than a replacement for current generated complexity artifacts.")]
        + f" PR #285 / accepted source `{PR285_SOURCE}` / squash merge `{PR285_MERGE}` / 19 of 19 applicable permanent workflows added governed versioned demo seeding, real-process permission/authentication/tenant smoke acceptance and permanent clean reset/seed/replay/smoke Governance coverage. The accepted Step 18 slices changed no workspace package, dependency declaration, feature set, internal edge, public Rust surface, Cargo manifest or `Cargo.lock`; this file remains a historical Stage B measurement baseline rather than a replacement for current generated complexity artifacts."
    )
    content = replace_exact(content, old, new, "complexity Step 18 closure")
    write(path, content)


def write_packet() -> None:
    packet = {
        "schema_version": "crm.repository-packet/v1",
        "packet_id": PACKET_ID,
        "title": "Synchronize accepted Repository Step 18 closure evidence",
        "status": "active",
        "baseline": {"ref": "main", "sha": PR285_MERGE},
        "tracking_issues": [194],
        "objective": (
            "Record exact PR #285 source, squash merge and 19-of-19 acceptance evidence across every "
            "normative source, close Repository Step 18 and make Repository Step 19 the only next permitted "
            "implementation packet without changing runtime or product behavior."
        ),
        "allowed_paths": ALLOWED,
        "forbidden_paths": [
            ".github/workflows/**", "AGENTS.md", "Cargo.lock", "Cargo.toml", "README.md",
            "affected-scope-policy.json", "apps/**", "contracts/**", "crates/**", "database/**",
            "evidence/**", "modules/**", "package.json", "packages/**", "pnpm-lock.yaml", "proto/**",
            "requirements-dev.txt", "rust-toolchain.toml", "schemas/**", "scripts/**", "services/**",
        ],
        "deliverables": [
            "record exact PR #285 source, squash merge and 19-of-19 workflow evidence in all five normative documents",
            "mark Repository Step 18 and architecture Stage H complete while keeping Phase 8A, Customer Privacy and architecture 10/10 incomplete",
            "make Repository Step 19 the only next permitted implementation packet",
            "update active-packet and documentation-consistency guards to the accepted closure evidence-sync packet",
            "record the accepted seed-demo/smoke zero-impact result in the historical workspace complexity baseline",
        ],
        "required_checks": REQUIRED,
        "acceptance": [
            f"the branch is based exactly on merge commit {PR285_MERGE}",
            f"all five normative documents contain PR #285, source {PR285_SOURCE}, merge {PR285_MERGE} and 19 of 19",
            "Repository Step 18 and Stage H are complete while Step 19 is the next permitted packet",
            "Customer Privacy, Phase 8A and architecture 10/10 remain explicitly incomplete",
            "the final diff contains only declared documentation, packet, generated navigation and permanent guard files",
            "one unchanged exact head passes every applicable permanent workflow with zero unresolved comments, reviews or review threads",
        ],
        "non_goals": [
            "implement or start Repository Step 19",
            "change Customer Privacy worker behavior",
            "change runtime, routes, contracts, schemas, migrations, dependencies, lockfiles or product behavior",
            "complete Phase 8A, an expert module or architecture 10/10",
        ],
    }
    write("repository-packet.json", json.dumps(packet, indent=2) + "\n")


def update_architecture_guard() -> None:
    path = "tests/test_architecture_documentation_consistency.py"
    content = read(path)
    replacement = f'''    def test_step_18_is_complete_and_step_19_is_next(self) -> None:
        exact = (
            "PR #275",
            "1c6366b557e255a14a677758fd87f7fe63184a89",
            "60d5974349a04c462475aadd4af0a37bada9713b",
            "PR #278",
            "228f459963e571a830aee6c43cddd15e3c9b0d5f",
            "996d634a33945e618be5ff81c297f0f617ce19d5",
            "PR #279",
            "0dce0895edec56508df1b4fc880d09ab27fc00df",
            "ea3d3894d05e3a8d814aff69824b593843763d03",
            "PR #285",
            "{PR285_SOURCE}",
            "{PR285_MERGE}",
            "19 of 19",
        )
        for document in self.normative_documents:
            lowered = document.lower()
            for marker in exact:
                self.assertIn(marker, document)
            self.assertIn("step 18", lowered)
            self.assertIn("step 19", lowered)
            self.assertNotRegex(lowered, r"step 18[^\\n.;]{{0,100}}(?:not started|in progress)")
        self.assertIn("Repository Steps 1–18 are complete", self.status)
        self.assertIn("18. deterministic local lifecycle commands", self.plan)
        self.assertIn("**complete through PR #285**", self.plan)
        self.assertIn("Repository Step 19", self.status)
'''
    content = replace_method(content, "test_step_17_is_complete_and_step_18_is_in_progress", replacement)
    packet_replacement = f'''    def test_active_step_18_closure_evidence_sync_packet_is_exact(self) -> None:
        self.assertEqual(self.packet["schema_version"], "crm.repository-packet/v1")
        self.assertEqual(self.packet["packet_id"], "{PACKET_ID}")
        self.assertEqual(self.packet["status"], "active")
        self.assertEqual(self.packet["baseline"], {{"ref": "main", "sha": "{PR285_MERGE}"}})
        self.assertEqual(self.packet["tracking_issues"], [194])
        self.assertEqual(set(self.packet["allowed_paths"]), {set(ALLOWED)!r})
        self.assertEqual(self.packet["required_checks"], {REQUIRED!r})
        self.assertIn(self.packet["packet_id"], self.active_packet)
        self.assertIn(self.packet["baseline"]["sha"], self.active_packet)
        self.assertIn(
            "make Repository Step 19 the only next permitted implementation packet",
            self.packet["deliverables"],
        )
        self.assertIn("implement or start Repository Step 19", self.packet["non_goals"])
'''
    content = replace_method(content, "test_active_step_18_seed_demo_smoke_packet_is_exact", packet_replacement)
    write(path, content)


def update_navigation_guard() -> None:
    path = "tests/test_repository_navigation.py"
    content = read(path)
    replacement = f'''    def test_active_packet_declaration_is_exact(self) -> None:
        packet = load_packet(ROOT)
        self.assertEqual(packet["schema_version"], "crm.repository-packet/v1")
        self.assertEqual(packet["packet_id"], "{PACKET_ID}")
        self.assertEqual(packet["status"], "active")
        self.assertEqual(packet["baseline"], {{"ref": "main", "sha": "{PR285_MERGE}"}})
        self.assertEqual(packet["tracking_issues"], [194])
        self.assertEqual(set(packet["allowed_paths"]), {set(ALLOWED)!r})
        self.assertEqual(packet["required_checks"], {REQUIRED!r})
        self.assertIn(
            "make Repository Step 19 the only next permitted implementation packet",
            packet["deliverables"],
        )
        self.assertIn("implement or start Repository Step 19", packet["non_goals"])
'''
    content = replace_method(content, "test_active_packet_declaration_is_exact", replacement)
    content = content.replace("repository-step-18-seed-demo-smoke", PACKET_ID)
    content = content.replace("953b9f929f8e89c351c59ebc01461fffab3689ff", PR285_MERGE)
    content = content.replace("Step 18 seed-demo/smoke implementation", "Step 18 closure evidence synchronization")
    write(path, content)


def verify() -> None:
    required = ("PR #285", PR285_SOURCE, PR285_MERGE, "19 of 19", "Repository Step 19")
    for path in NORMATIVE:
        content = read(path)
        for marker in required:
            if marker not in content:
                raise RuntimeError(f"{path}: missing {marker}")
    combined = "\n".join(read(path) for path in NORMATIVE)
    for stale in (
        "seed-demo/smoke next",
        "next permitted bounded implementation packet is `seed-demo` and `smoke`",
        "Repository Step 19 remains blocked",
    ):
        if stale in combined:
            raise RuntimeError(f"stale Step 18 claim remains: {stale}")
    if "Architecture 10/10 is **not declared**" not in read("docs/PROJECT_STATUS.md"):
        raise RuntimeError("architecture non-completion guard disappeared")


def main() -> None:
    synchronize_common_evidence()
    update_architecture_plan()
    update_roadmap()
    update_catalog()
    update_phase8()
    update_project_status()
    update_complexity()
    write_packet()
    update_architecture_guard()
    update_navigation_guard()
    verify()
    subprocess.run(["python", "scripts/generate_repository_navigation.py", "--write"], cwd=ROOT, check=True)


if __name__ == "__main__":
    main()
