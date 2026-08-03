from __future__ import annotations

import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]

PR275 = "PR #275 / source `1c6366b557e255a14a677758fd87f7fe63184a89` / merge `60d5974349a04c462475aadd4af0a37bada9713b` / 23 of 23"
PR278 = "PR #278 / source `228f459963e571a830aee6c43cddd15e3c9b0d5f` / merge `996d634a33945e618be5ff81c297f0f617ce19d5` / 8 of 8"
PR279 = "PR #279 / source `0dce0895edec56508df1b4fc880d09ab27fc00df` / merge `ea3d3894d05e3a8d814aff69824b593843763d03` / 22 of 22"

EVIDENCE_BULLETS = f"""- {PR275} — published wire-compatible `activities.task.create@1.1.0`, migrated the sole repository-owned consumer and made live authorization overlap exact-version safe;
- {PR278} — made production zero-usage retirement evidence SHA-256-bound, complete, append-only and fail closed;
- {PR279} — proved the coordinate was never externally released through live empty GitHub Releases, tags and deployments, non-publishable packages and no external consumer record, then retired `activities.task.create@1.0.0` while preserving its historical tombstone and keeping `telemetry.zero_since` null.
"""

STEP17_RESULT = """The accepted Step 17 result keeps `activities.task.create@1.1.0` as the sole live create coordinate. `activities.task.create@1.0.0` is absent from the current provider manifest and production capability catalog, rejected before payload decoding, retained only as immutable historical registry/lifecycle evidence and classified `never_externally_released`. The ordinary 30-day/production-zero-usage path remains binding for released contracts; no production history was fabricated. Repository Step 18 is the next permitted implementation step and is not started.
"""


def read(path: str) -> str:
    return (ROOT / path).read_text(encoding="utf-8")


def write(path: str, text: str) -> None:
    (ROOT / path).write_text(text, encoding="utf-8")


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected one occurrence, found {count}: {old!r}")
    return text.replace(old, new, 1)


def insert_before(text: str, anchor: str, insertion: str, label: str) -> str:
    return replace_once(text, anchor, insertion + "\n" + anchor, label)


def sync_project_status() -> None:
    path = "docs/PROJECT_STATUS.md"
    text = read(path)
    text = replace_once(text, "Status date: 2026-08-02", "Status date: 2026-08-03", path)
    text = replace_once(
        text,
        "Repository Steps 1–16 are complete.",
        "Repository Steps 1–17 are complete.",
        path,
    )
    section = f"""## Repository Step 17 accepted closure

Repository Step 17 is complete through three bounded accepted slices:

{EVIDENCE_BULLETS}
{STEP17_RESULT}
"""
    text = insert_before(text, "## Current measured repository baseline", section, path)
    row = f"| 17 | {PR275}; {PR278}; {PR279} | Wire-compatible migration, exact-version authorization overlap, immutable retirement evidence and proven never-externally-released retirement |\n"
    marker = "| 16 | PR #269 / source `74b1d7b0f8764fcd90839b7aab25f8f82fe5e552` / merge `6f82de0a7b2dcd1ab5dd0ae6473d46d7d9d34bdd` / 20 of 20; PR #270 / source `8e2baac0822eefbb6d3c474ffce0cee69e3e4e98` / merge `ce0ca881461d1ee8964a11b28c1fcff46cf145cb` / 17 of 17 | Reusable worker conformance, representative real-worker adoption, retry/restart recovery and exactly-once contention convergence |\n"
    text = replace_once(text, marker, marker + row, path)
    old_next = """## Next permitted repository packet

Repository Step 17 is the next permitted implementation packet and is **not started**. Its bounded scope is contract compatibility, published-version gates, deprecation telemetry, consumer migration evidence and governed retirement enforcement.

No Step 18 or later implementation may start while Step 17 remains unfinished.
"""
    new_next = """## Next permitted repository packet

Repository Step 18 is the next permitted implementation packet and is **not started**. Its bounded scope is deterministic local lifecycle commands: `doctor`, `bootstrap`, `dev-up`, `dev-reset`, `seed-demo` and `smoke` on a clean environment.

No Step 19 or later implementation may start while Step 18 remains unfinished.
"""
    text = replace_once(text, old_next, new_next, path)
    text = text.replace(
        "17. contract compatibility, deprecation, consumer-migration and retirement lifecycle enforcement — **next, not started**;",
        "17. contract compatibility, deprecation, consumer-migration and retirement lifecycle enforcement — **complete through PR #279**;",
    )
    text = text.replace(
        "18. deterministic local lifecycle commands:",
        "18. deterministic local lifecycle commands — **next, not started**:",
        1,
    )
    write(path, text)


def sync_roadmap() -> None:
    path = "docs/IMPLEMENTATION_ROADMAP.md"
    text = read(path)
    text = replace_once(
        text,
        "Repository Steps 1–16 are complete. Repository Step 17 is the next permitted implementation step and is **not started**.",
        "Repository Steps 1–17 are complete. Repository Step 18 is the next permitted implementation step and is **not started**.",
        path,
    )
    section = f"""### 3.4 Accepted Repository Step 17 closure

Step 17 is accepted through three bounded slices:

{EVIDENCE_BULLETS}
{STEP17_RESULT}
"""
    text = replace_once(text, "### 3.4 Binding next repository sequence", section + "\n### 3.5 Binding next repository sequence", path)
    text = replace_once(
        text,
        "17. contract compatibility, deprecation, consumer-migration and retirement enforcement — **next, not started**;",
        "17. contract compatibility, deprecation, consumer-migration and retirement enforcement — **complete through PR #279**;",
        path,
    )
    text = replace_once(
        text,
        "18. deterministic local lifecycle commands;",
        "18. deterministic local lifecycle commands — **next, not started**;",
        path,
    )
    text = text.replace("The remaining product work after Step 16 is:", "The remaining product work after Step 17 is:")
    text = text.replace(
        "Repository Step 17 owns only contract lifecycle enforcement. It must not absorb Steps 18–21 or claim a Customer Privacy worker before Step 19.",
        "Repository Step 17 contract lifecycle enforcement is complete. Repository Step 18 now owns only deterministic local lifecycle commands and must not absorb Steps 19–21 or claim a Customer Privacy worker before Step 19.",
    )
    write(path, text)


def sync_phase8() -> None:
    path = "docs/PHASE8_DELIVERY_PLAN.md"
    text = read(path)
    text = replace_once(text, "## 5. Accepted Repository Steps 14–16 architecture results", "## 5. Accepted Repository Steps 14–17 architecture results", path)
    section = f"""Repository Step 17 is complete through:

{EVIDENCE_BULLETS}
{STEP17_RESULT}
"""
    text = insert_before(text, "Exact measured result remains:", section, path)
    old = """### Repository Step 17 — next, not started

- contract compatibility and published-version gates;
- deprecation telemetry;
- consumer migration evidence;
- governed retirement enforcement.

### Repository Step 18
"""
    new = f"""### Repository Step 17 — complete through PR #279

{EVIDENCE_BULLETS}
{STEP17_RESULT}
### Repository Step 18 — next, not started
"""
    text = replace_once(text, old, new, path)
    text = replace_once(
        text,
        "No Step 18 or later implementation may begin before Step 17 is accepted and synchronized.",
        "No Step 19 or later implementation may begin before Step 18 is accepted and synchronized.",
        path,
    )
    write(path, text)


def sync_architecture_plan() -> None:
    path = "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md"
    text = read(path)
    text = replace_once(text, "Current execution checkpoint: **2026-08-02**", "Current execution checkpoint: **2026-08-03**", path)
    section = f"""Repository Step 17 is accepted through {PR275}, {PR278} and {PR279}.

{STEP17_RESULT}
"""
    text = insert_before(text, "Current blocking baseline after Step 16:", section, path)
    text = replace_once(text, "Current blocking baseline after Step 16:", "Current blocking baseline after Step 17:", path)
    text = text.replace(
        "reusable mutation/query conformance through PR #235 and reusable worker conformance with representative real-worker adoption through PRs #269–#270 | contract lifecycle Step 17 and real Customer Privacy worker adoption Step 19",
        "reusable mutation/query conformance through PR #235, reusable worker conformance through PRs #269–#270 and complete contract lifecycle enforcement through PRs #275, #278 and #279 | real Customer Privacy worker adoption Step 19",
    )
    text = replace_once(
        text,
        "17. contract compatibility, deprecation, consumer-migration and retirement lifecycle enforcement — **next, not started**;",
        "17. contract compatibility, deprecation, consumer-migration and retirement lifecycle enforcement — **complete through PR #279**;",
        path,
    )
    text = replace_once(
        text,
        "18. deterministic local lifecycle commands: `doctor`, `bootstrap`, `dev-up`, `dev-reset`, `seed-demo` and `smoke`;",
        "18. deterministic local lifecycle commands: `doctor`, `bootstrap`, `dev-up`, `dev-reset`, `seed-demo` and `smoke` — **next, not started**;",
        path,
    )
    text = text.replace(
        "| F — generic conformance and contract lifecycle | **In progress** | reusable mutation/query conformance through PR #235 and reusable worker conformance with representative real-worker adoption through PRs #269–#270 | contract lifecycle Step 17 and real Customer Privacy worker adoption Step 19 |",
        "| F — generic conformance and contract lifecycle | **In progress** | reusable mutation/query conformance through PR #235, reusable worker conformance through PRs #269–#270 and complete contract lifecycle enforcement through PRs #275, #278 and #279 | real Customer Privacy worker adoption Step 19 |",
    )
    write(path, text)


def sync_module_catalog() -> None:
    path = "docs/MODULE_CATALOG.md"
    text = read(path)
    section = f"""Repository Step 17 is accepted through:

{EVIDENCE_BULLETS}
{STEP17_RESULT}
"""
    text = insert_before(text, "The accepted Step 15 result preserves", section, path)
    text = replace_once(
        text,
        "Repository Step 17 is next and not started.",
        "Repository Step 17 is complete through PR #279; Repository Step 18 is next and not started.",
        path,
    )
    write(path, text)


def sync_packet() -> None:
    packet = {
        "schema_version": "crm.repository-packet/v1",
        "packet_id": "repository-step-17-accepted-evidence-sync",
        "title": "Synchronize accepted Repository Step 17 closure evidence",
        "status": "active",
        "baseline": {"ref": "main", "sha": "ea3d3894d05e3a8d814aff69824b593843763d03"},
        "tracking_issues": [126, 194],
        "objective": "Synchronize the five normative repository and product documents after accepted PRs #275, #278 and #279. Record exact source, merge and workflow evidence, mark Repository Steps 1–17 complete, preserve the accepted never-externally-released retirement boundary without claiming production history, designate deterministic local lifecycle Step 18 as the sole next packet, and keep Phase 8A, Customer Privacy, later architecture steps and architecture 10/10 explicitly incomplete.",
        "allowed_paths": [
            "docs/ACTIVE_PACKET.md",
            "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md",
            "docs/IMPLEMENTATION_ROADMAP.md",
            "docs/MODULE_CATALOG.md",
            "docs/PHASE8_DELIVERY_PLAN.md",
            "docs/PROJECT_STATUS.md",
            "docs/generated/REPOSITORY_MAP.md",
            "repository-packet.json",
            "tests/test_architecture_documentation_consistency.py",
            "tests/test_repository_navigation.py",
        ],
        "forbidden_paths": [
            ".github/workflows/**",
            "Cargo.lock",
            "Cargo.toml",
            "apps/**",
            "contracts/**",
            "crates/**",
            "database/**",
            "evidence/**",
            "modules/**",
            "packages/**",
            "proto/**",
            "schemas/**",
            "scripts/**",
            "services/**",
        ],
        "deliverables": [
            "record exact PR #275, #278 and #279 source, merge and workflow evidence in all five normative documents",
            "mark Repository Steps 1-17 complete and Step 18 next, not started",
            "record one retired lifecycle tombstone, no deprecated live coordinate and activities.task.create@1.1.0 as the sole live create coordinate",
            "preserve the distinction between never-externally-released evidence and the ordinary production zero-usage path",
            "keep Stage F in progress until real Customer Privacy worker adoption at Step 19",
            "keep Phase 8A.11, Customer Privacy, product completion and architecture 10/10 incomplete",
            "regenerate deterministic active-packet and repository navigation documents",
        ],
        "required_checks": [
            "Affected Scope CI",
            "Complexity Baseline CI",
            "Governance CI",
            "Rust Generated Sync",
            "Rust CI",
        ],
        "acceptance": [
            "the branch is based exactly on main commit ea3d3894d05e3a8d814aff69824b593843763d03 and contains only the ten declared evidence-sync files",
            "all five normative documents state Repository Steps 1-17 are complete and Step 18 is the sole next implementation step",
            "all five normative documents contain exact PR #275, #278 and #279 source and merge evidence",
            "no document calls Step 17 next or not started",
            "no document calls Step 18 complete or starts Step 19 early",
            "the accepted never-externally-released retirement is described without fabricating telemetry.zero_since or production deployment history",
            "Phase 8A.11 remains in progress, Customer Privacy remains incomplete, product-complete expert modules remain zero and architecture 10/10 is not declared",
            "generated navigation is deterministic and current",
            "one unchanged meaningful head passes every applicable permanent workflow and final changed-file, comment, review and thread inspection",
        ],
        "non_goals": [
            "change lifecycle policy, release evidence, runtime code, contracts, schemas, migrations or workflows",
            "start deterministic local lifecycle Step 18 implementation",
            "start the Step 19 Customer Privacy worker lifecycle",
            "complete Phase 8A or any expert module",
            "declare architecture 10/10",
        ],
    }
    (ROOT / "repository-packet.json").write_text(json.dumps(packet, indent=2) + "\n", encoding="utf-8")


def sync_tests() -> None:
    nav_path = ROOT / "tests/test_repository_navigation.py"
    nav = nav_path.read_text(encoding="utf-8")
    nav = nav.replace("repository-step-17-preproduction-retirement", "repository-step-17-accepted-evidence-sync")
    nav = nav.replace("996d634a33945e618be5ff81c297f0f617ce19d5", "ea3d3894d05e3a8d814aff69824b593843763d03")
    start = nav.index("        self.assertEqual(\n            set(packet[\"allowed_paths\"]),")
    end = nav.index("        self.assertEqual(\n            packet[\"required_checks\"],", start)
    allowed = '''        self.assertEqual(\n            set(packet["allowed_paths"]),\n            {\n                "docs/ACTIVE_PACKET.md",\n                "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md",\n                "docs/IMPLEMENTATION_ROADMAP.md",\n                "docs/MODULE_CATALOG.md",\n                "docs/PHASE8_DELIVERY_PLAN.md",\n                "docs/PROJECT_STATUS.md",\n                "docs/generated/REPOSITORY_MAP.md",\n                "repository-packet.json",\n                "tests/test_architecture_documentation_consistency.py",\n                "tests/test_repository_navigation.py",\n            },\n        )\n'''
    nav = nav[:start] + allowed + nav[end:]
    checks_start = nav.index("        self.assertEqual(\n            packet[\"required_checks\"],")
    checks_end = nav.index("        self.assertIn(\n", checks_start)
    checks = '''        self.assertEqual(\n            packet["required_checks"],\n            [\n                "Affected Scope CI",\n                "Complexity Baseline CI",\n                "Governance CI",\n                "Rust Generated Sync",\n                "Rust CI",\n            ],\n        )\n'''
    nav = nav[:checks_start] + checks + nav[checks_end:]
    nav = nav.replace(
        "verify the new evidence against live GitHub Releases, tags and deployments before merge",
        "record exact PR #275, #278 and #279 source, merge and workflow evidence in all five normative documents",
    )
    nav = nav.replace(
        "backdate telemetry.zero_since or fabricate production usage history",
        "start deterministic local lifecycle Step 18 implementation",
    )
    nav = nav.replace(
        '"Step 17 never-released retirement"',
        '"Step 17 accepted evidence synchronization"',
    )
    nav_path.write_text(nav, encoding="utf-8")

    arch_path = ROOT / "tests/test_architecture_documentation_consistency.py"
    arch = arch_path.read_text(encoding="utf-8")
    arch = arch.replace("repository-step-17-preproduction-retirement", "repository-step-17-accepted-evidence-sync")
    arch = arch.replace("996d634a33945e618be5ff81c297f0f617ce19d5", "ea3d3894d05e3a8d814aff69824b593843763d03")
    method_start = arch.index("    def test_step_16_is_complete_and_step_17_is_next")
    method_end = arch.index("    def test_product_readiness_is_not_overstated", method_start)
    method = '''    def test_step_17_is_complete_and_step_18_is_next(self) -> None:\n        exact = (\n            "PR #275",\n            "1c6366b557e255a14a677758fd87f7fe63184a89",\n            "60d5974349a04c462475aadd4af0a37bada9713b",\n            "PR #278",\n            "228f459963e571a830aee6c43cddd15e3c9b0d5f",\n            "996d634a33945e618be5ff81c297f0f617ce19d5",\n            "PR #279",\n            "0dce0895edec56508df1b4fc880d09ab27fc00df",\n            "ea3d3894d05e3a8d814aff69824b593843763d03",\n        )\n        for document in self.normative_documents:\n            lowered = document.lower()\n            for marker in exact:\n                self.assertIn(marker, document)\n            self.assertIn("step 17", lowered)\n            self.assertIn("step 18", lowered)\n            self.assertNotRegex(lowered, r"step 17[^\\n.;]{0,80}not started")\n        self.assertIn("Repository Steps 1–17 are complete", self.status)\n        self.assertIn("Repository Step 18 is the next permitted implementation packet", self.status)\n        self.assertIn(\n            "17. contract compatibility, deprecation, consumer-migration and retirement lifecycle enforcement — **complete through PR #279**",\n            self.plan,\n        )\n        self.assertIn("18. deterministic local lifecycle commands", self.plan)\n        self.assertIn("**next, not started**", self.plan)\n\n'''
    arch = arch[:method_start] + method + arch[method_end:]
    arch = arch.replace("test_active_step_17_preproduction_retirement_packet_is_exact", "test_active_step_17_evidence_sync_packet_is_exact")
    astart = arch.index("        self.assertEqual(\n            set(self.packet[\"allowed_paths\"]),")
    aend = arch.index("        self.assertEqual(\n            self.packet[\"required_checks\"],", astart)
    allowed_arch = '''        self.assertEqual(\n            set(self.packet["allowed_paths"]),\n            {\n                "docs/ACTIVE_PACKET.md",\n                "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md",\n                "docs/IMPLEMENTATION_ROADMAP.md",\n                "docs/MODULE_CATALOG.md",\n                "docs/PHASE8_DELIVERY_PLAN.md",\n                "docs/PROJECT_STATUS.md",\n                "docs/generated/REPOSITORY_MAP.md",\n                "repository-packet.json",\n                "tests/test_architecture_documentation_consistency.py",\n                "tests/test_repository_navigation.py",\n            },\n        )\n'''
    arch = arch[:astart] + allowed_arch + arch[aend:]
    cstart = arch.index("        self.assertEqual(\n            self.packet[\"required_checks\"],")
    cend = arch.index("        for forbidden in (", cstart)
    checks_arch = '''        self.assertEqual(\n            self.packet["required_checks"],\n            [\n                "Affected Scope CI",\n                "Complexity Baseline CI",\n                "Governance CI",\n                "Rust Generated Sync",\n                "Rust CI",\n            ],\n        )\n'''
    arch = arch[:cstart] + checks_arch + arch[cend:]
    arch = arch.replace(
        "verify the new evidence against live GitHub Releases, tags and deployments before merge",
        "record exact PR #275, #278 and #279 source, merge and workflow evidence in all five normative documents",
    )
    arch = arch.replace(
        "backdate telemetry.zero_since or fabricate production usage history",
        "start deterministic local lifecycle Step 18 implementation",
    )
    arch_path.write_text(arch, encoding="utf-8")


def main() -> None:
    sync_project_status()
    sync_roadmap()
    sync_phase8()
    sync_architecture_plan()
    sync_module_catalog()
    sync_packet()
    sync_tests()


if __name__ == "__main__":
    main()
