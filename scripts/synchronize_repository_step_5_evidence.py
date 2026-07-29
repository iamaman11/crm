#!/usr/bin/env python3
"""One-shot deterministic synchronization for accepted repository step 5 evidence."""

from __future__ import annotations

import json
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[1]
SOURCE = "a9aa0bef028d906b61e83803436167bf6f91e634"
MERGE = "727a244fcf174dc517dec6fdbb6b8997eb205f14"
BASELINE = MERGE


def read(path: str) -> str:
    return (ROOT / path).read_text(encoding="utf-8")


def write(path: str, content: str) -> None:
    (ROOT / path).write_text(content, encoding="utf-8")


def replace_once(path: str, old: str, new: str) -> None:
    content = read(path)
    count = content.count(old)
    if count != 1:
        raise RuntimeError(f"{path}: expected one occurrence, found {count}: {old!r}")
    write(path, content.replace(old, new, 1))


def regex_once(path: str, pattern: str, replacement: str) -> None:
    content = read(path)
    updated, count = re.subn(pattern, replacement, content, count=1, flags=re.MULTILINE)
    if count != 1:
        raise RuntimeError(f"{path}: expected one regex match, found {count}: {pattern!r}")
    write(path, updated)


def synchronize_status() -> None:
    path = "docs/PROJECT_STATUS.md"
    replace_once(
        path,
        "Latest accepted repository architecture prerequisite remains PR #224 / accepted source `e57307fcb1b5192d5e6340247cb6633f32b7ba34` / squash merge `67804d9478b2bbaf342a398b649e23bd5ead6c08` / 28 of 28 permanent workflows.",
        f"Latest accepted repository architecture/developer-experience packet is PR #228 / accepted source `{SOURCE}` / squash merge `{MERGE}` / 5 of 5 applicable permanent workflows.",
    )
    marker = "Restriction release/reads, legal-hold and mandatory-retention adjudication, owner execution, access/export assembly, destructive actions and Customer Privacy workers remain non-runtime.\n\n"
    section = (
        "## Accepted repository explanation and generated navigation\n\n"
        f"Repository step 5 is accepted through PR #228 / accepted source `{SOURCE}` / squash merge `{MERGE}` / 5 of 5 applicable permanent workflows on one unchanged source-authored head.\n\n"
        "The repository now has deterministic `repo.py explain` for exact module/capability ownership and route classification, fail-closed `repo.py packet-check` for exact baseline/path/affected/workflow/freshness validation, generated `docs/ACTIVE_PACKET.md`, and generated `docs/generated/REPOSITORY_MAP.md` with source digests. Affected Scope CI executes packet-check against the real pull-request diff before structural, Clippy and test closures.\n\n"
        "The accepted generated inventory is 113 workspace packages, 14 business manifests, 119 published capability coordinates, 70 published event coordinates, 7 platform runtime routes, 5 worker runtime routes, 17 non-runtime contract routes and one route-less module. Product runtime, contracts, manifests, migrations, dependencies, `Cargo.lock`, package count and Customer Privacy behavior are unchanged.\n\n"
    )
    replace_once(path, marker, marker + section)
    replace_once(
        path,
        "## Next permitted repository packet\n\nRepository step 5 is `repo.py explain`, `repo.py packet-check`, generated `docs/ACTIVE_PACKET.md` and the generated repository map. This packet is behavior-neutral developer tooling and navigation; it must not change routes, contracts, persistence or product semantics.\n\n## Following permitted repository packet\n\nRepository step 6 is Customer Privacy legal-hold and mandatory-retention precedence.",
        "## Next permitted repository packet\n\nRepository step 6 is Customer Privacy legal-hold and mandatory-retention precedence.\n\n## Following permitted repository packet\n\nRepository step 7 is reusable generic mutation and query conformance.",
    )
    replace_once(
        path,
        "- Stage E has a working foundation but is incomplete: affected-scope selection is not yet complete across every database/process/product/frontend/operations dimension.\n- Stages F–I remain foundation-only or unstarted; generic conformance/lifecycle, measured consolidation, reproducible local environment, generated navigation, frontend and operations parity are not complete.",
        "- Stage E has a working foundation but is incomplete: real-diff packet validation and Rust broadening are accepted, while database/process/product/frontend/operations selection remains incomplete.\n- Stage H is in progress: deterministic explain, packet-check and generated navigation are accepted through PR #228; local lifecycle commands remain repository step 15.\n- Stages F, G and I remain foundation-only or unstarted; generic conformance/lifecycle, measured consolidation, frontend and operations parity are not complete.",
    )
    replace_once(
        path,
        "-> 5. explain / packet-check / generated active packet and repository map — next\n-> 6. legal-hold and mandatory-retention precedence",
        "-> 5. explain / packet-check / generated active packet and repository map — complete through PR #228\n-> 6. legal-hold and mandatory-retention precedence — next",
    )


def synchronize_roadmap() -> None:
    path = "docs/IMPLEMENTATION_ROADMAP.md"
    replace_once(
        path,
        "- Stages E–I — affected-scope expansion, conformance, consolidation, reproducible environment, frontend and operations parity remain open.",
        "- Stage E — **In progress**: real-diff packet-check and broadened Rust closure are accepted; database/process/product/frontend/operations selection remains open.\n- Stage H — **In progress**: deterministic explain, packet-check and generated navigation are accepted through PR #228; local lifecycle commands remain open.\n- Stages F, G and I — conformance/lifecycle, measured consolidation, frontend and operations parity remain open.",
    )
    replace_once(
        path,
        "Accepted architecture evidence includes PR #197, PR #199, PR #200, PR #203, PR #204, PR #205, PR #218, PR #222, PR #224 and PR #226.",
        "Accepted architecture evidence includes PR #197, PR #199, PR #200, PR #203, PR #204, PR #205, PR #218, PR #222, PR #224, PR #226 and PR #228.",
    )
    replace_once(
        path,
        "The architecture stage table is completion accounting, not a set of independent work queues. The binding repository order is section 2.4 of the architecture plan. Repository steps 1–4 are complete through PR #218, PR #220, PR #222 and PR #226; PR #224 accepted the smallest inserted prerequisite required by step 4. Repository step 5 — `explain`, `packet-check` and generated navigation — is the current next packet.",
        "The architecture stage table is completion accounting, not a set of independent work queues. The binding repository order is section 2.4 of the architecture plan. Repository steps 1–5 are complete through PR #218, PR #220, PR #222, PR #226 and PR #228; PR #224 accepted the smallest inserted prerequisite required by step 4. Repository step 6 — legal-hold and mandatory-retention precedence — is the current next packet.",
    )
    marker = "Permanent PostgreSQL and real-process acceptance proves pre-restriction owner success, public placement, active denial without side effects, unrelated-Party isolation, malformed/cross-tenant fail-closed behavior, retained lock, complete rollback/reapply and repeated acceptance. Restriction release/reads, legal holds, retention decisions, owner execution, destructive behavior and workers remain non-runtime.\n\n"
    section = (
        "### 5.9 Accepted repository explanation and generated navigation\n\n"
        f"PR #228 / accepted source `{SOURCE}` / squash merge `{MERGE}` / 5 of 5 applicable permanent workflows completes repository step 5 on one unchanged source-authored head.\n\n"
        "The packet adds deterministic exact module/capability explanation, fail-closed packet validation, real-diff Affected Scope enforcement, generated active-packet/repository-map navigation and permanent freshness tests. It records 113 packages, 14 manifests, 119 capabilities and 70 events without changing product runtime, contracts, persistence, migrations, dependencies, `Cargo.lock` or workspace package count.\n\n"
    )
    replace_once(path, marker, marker + section)
    replace_once(
        path,
        "5. **Repository step 5 — `explain`, `packet-check` and generated navigation — Next.**\n6. **Repository step 6 — legal-hold and mandatory-retention precedence.**",
        "5. **Repository step 5 — `explain`, `packet-check` and generated navigation — Complete through PR #228.**\n6. **Repository step 6 — legal-hold and mandatory-retention precedence — Next.**",
    )
    replace_once(
        path,
        "The inserted prerequisite did not renumber the master sequence. Repository step 5 is now the next permitted behavior-neutral architecture/developer-experience packet.",
        "The inserted prerequisite did not renumber the master sequence. Repository step 6 is now the next permitted Customer Privacy packet.",
    )


def synchronize_phase8() -> None:
    path = "docs/PHASE8_DELIVERY_PLAN.md"
    replace_once(
        path,
        "PR #226 / accepted source `ad08a691ec759b8b3b523fa66a034cecf4138ff0` / squash merge `a46460623e90c5649d36bedba055fb55023d9349` / 34 of 34 permanent workflows completes repository step 4. Immediate deny-only restriction placement and the first complete protected-owner boundary are accepted; repository step 5 is now the next permitted implementation packet.",
        f"PR #226 / accepted source `ad08a691ec759b8b3b523fa66a034cecf4138ff0` / squash merge `a46460623e90c5649d36bedba055fb55023d9349` / 34 of 34 permanent workflows completes repository step 4. Immediate deny-only restriction placement and the first complete protected-owner boundary are accepted.\n\nPR #228 / accepted source `{SOURCE}` / squash merge `{MERGE}` / 5 of 5 applicable permanent workflows completes repository step 5. Deterministic explanation, packet validation, real-diff affected enforcement and generated navigation are accepted; repository step 6 is now the next permitted implementation packet.",
    )
    marker = "`contact-points.contact-point.create@1.0.0` is the first complete protected-owner path. Real-process acceptance proves placement, active denial immediately before persistence, zero denied side effects, unrelated-Party isolation, rollback/reapply and repeated acceptance. Restriction release/reads, legal holds, retention decisions, owner execution, access/export, destructive actions and workers remain later work.\n\n"
    section = (
        "### 8.5 Accepted repository explanation and generated navigation\n\n"
        f"PR #228 / accepted source `{SOURCE}` / squash merge `{MERGE}` / 5 of 5 applicable permanent workflows accepts repository step 5.\n\n"
        "The packet provides deterministic module/capability explanation, exact baseline and path-policy packet checking, real pull-request diff enforcement, generated active-packet/repository-map navigation and freshness-checked conformance. It changes no Customer Privacy runtime, route, contract, manifest, persistence, migration, dependency, `Cargo.lock`, package or worker.\n\n"
    )
    replace_once(path, marker, marker + section)
    replace_once(
        path,
        "5. repository step 5 — `explain`, `packet-check` and generated navigation — **next**;\n6. repository step 6 — legal-hold and mandatory-retention precedence;",
        "5. repository step 5 — `explain`, `packet-check` and generated navigation — **complete through PR #228**;\n6. repository step 6 — legal-hold and mandatory-retention precedence — **next**;",
    )
    replace_once(
        path,
        "A later step must not start while repository step 5 is unfinished.",
        "A later step must not start while repository step 6 is unfinished.",
    )


def synchronize_plan() -> None:
    path = "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md"
    replace_once(
        path,
        "- incomplete local-environment automation;\n- incomplete repository navigation and packet explanation tooling;",
        "- incomplete local-environment lifecycle automation;\n- remaining affected-scope coverage outside the accepted real-diff Rust/tooling closure;",
    )
    replace_once(
        path,
        "| E — affected-scope CI | **In progress** | changed paths, Rust reverse closure, structural preflight and explainable broadening exist | complete contract, migration, process, product, frontend and operations scope selection with safe fallback |",
        "| E — affected-scope CI | **In progress** | changed paths, Rust reverse closure, structural preflight, explainable broadening and real-diff packet-check enforcement are accepted | complete contract, migration, process, product, frontend and operations scope selection with safe fallback |",
    )
    replace_once(
        path,
        "| H — reproducible environment and navigation | **Partially started** | stable docs index plus `repo.py affected` and `check-affected` exist | `doctor`, `bootstrap`, `dev-up`, `dev-reset`, `seed-demo`, `smoke`, `explain`, `packet-check`, generated active packet and repository map |",
        "| H — reproducible environment and navigation | **In progress** | stable docs index, `affected`, `check-affected`, deterministic `explain`, fail-closed `packet-check`, generated active packet and repository map are accepted through PR #228 | `doctor`, `bootstrap`, `dev-up`, `dev-reset`, `seed-demo` and `smoke` |",
    )
    replace_once(
        path,
        "The next permitted implementation packet is repository step 5: `repo.py explain`, `repo.py packet-check`, generated `docs/ACTIVE_PACKET.md` and generated repository map.",
        f"Repository step 5 is accepted through PR #228 / accepted source `{SOURCE}` / squash merge `{MERGE}` / 5 of 5 applicable permanent workflows. It adds deterministic explanation, fail-closed packet validation, real-diff Affected Scope enforcement and generated navigation without changing product behavior, contracts, persistence, dependencies, `Cargo.lock` or package count.\n\nThe next permitted implementation packet is repository step 6: Customer Privacy legal-hold and mandatory-retention precedence.",
    )
    replace_once(
        path,
        "The inserted repository step 4 prerequisite is complete through PR #224, and repository step 4 itself is complete through PR #226. Neither changes the master numbering.",
        "The inserted repository step 4 prerequisite is complete through PR #224, repository step 4 is complete through PR #226, and repository step 5 is complete through PR #228. None changes the master numbering.",
    )
    replace_once(
        path,
        "5. `repo.py explain`, `repo.py packet-check`, generated `docs/ACTIVE_PACKET.md` and generated repository map — **Next**;\n6. Customer Privacy legal-hold and mandatory-retention precedence;",
        "5. `repo.py explain`, `repo.py packet-check`, generated `docs/ACTIVE_PACKET.md` and generated repository map — **Complete through PR #228**;\n6. Customer Privacy legal-hold and mandatory-retention precedence — **Next**;",
    )
    replace_once(
        path,
        "The next permitted repository packet is **repository step 5: `repo.py explain`, `repo.py packet-check`, generated `docs/ACTIVE_PACKET.md` and generated repository map**. It is behavior-neutral navigation/tooling work and must not add legal-hold/mandatory-retention adjudication, owner execution, destructive action, worker, dependency upgrade, crate consolidation or product runtime behavior.",
        "The next permitted repository packet is **repository step 6: Customer Privacy legal-hold and mandatory-retention precedence**. It must preserve shared subject locking and immediate deny-only behavior while adding authoritative legal-hold-over-retention-over-approved-action adjudication; it must not include owner execution, access/export assembly, destructive action, workers, dependency upgrades or crate consolidation.",
    )


def synchronize_test() -> None:
    path = "tests/test_architecture_documentation_consistency.py"
    replace_once(
        path,
        '            "5. `repo.py explain`, `repo.py packet-check`, generated `docs/ACTIVE_PACKET.md` and generated repository map — **Next**;",',
        '            "5. `repo.py explain`, `repo.py packet-check`, generated `docs/ACTIVE_PACKET.md` and generated repository map — **Complete through PR #228**;",\n            "6. Customer Privacy legal-hold and mandatory-retention precedence — **Next**;",',
    )
    replace_once(
        path,
        '            (\n                self.privacy_status_documents,\n                "PR #226",\n                "ad08a691ec759b8b3b523fa66a034cecf4138ff0",\n                "a46460623e90c5649d36bedba055fb55023d9349",\n                "34 of 34",\n            ),',
        '            (\n                self.privacy_status_documents,\n                "PR #226",\n                "ad08a691ec759b8b3b523fa66a034cecf4138ff0",\n                "a46460623e90c5649d36bedba055fb55023d9349",\n                "34 of 34",\n            ),\n            (\n                self.authoritative_status_documents,\n                "PR #228",\n                "a9aa0bef028d906b61e83803436167bf6f91e634",\n                "727a244fcf174dc517dec6fdbb6b8997eb205f14",\n                "5 of 5",\n            ),',
    )
    replace_once(path, '        self.assertEqual(self.packet["packet_id"], "repository-step-5")', '        self.assertEqual(self.packet["packet_id"], "repository-step-5-evidence-sync")')
    replace_once(path, '        self.assertEqual(self.packet["status"], "active")', '        self.assertEqual(self.packet["status"], "active")')
    replace_once(path, '            "f7e6dc51cbe09add8025174640fca539b8327a25",', f'            "{BASELINE}",')
    replace_once(path, '        self.assertIn("scripts/repo.py", self.packet["allowed_paths"])', '        self.assertIn("docs/PROJECT_STATUS.md", self.packet["allowed_paths"])')
    replace_once(path, '        self.assertIn("crates/**/src/**", self.packet["forbidden_paths"])', '        self.assertIn("crates/**/src/**", self.packet["forbidden_paths"])')
    replace_once(path, '        self.assertIn("repository-step-5", self.active_packet)', '        self.assertIn("repository-step-5-evidence-sync", self.active_packet)')
    replace_once(path, '            "4. immediate deny-only Customer Privacy processing restrictions using final subject locks — **Next**",', '            "4. immediate deny-only Customer Privacy processing restrictions using final subject locks — **Next**",\n            "5. `repo.py explain`, `repo.py packet-check`, generated `docs/ACTIVE_PACKET.md` and generated repository map — **Next**",')


def write_packet() -> None:
    packet = {
        "schema_version": "crm.repository-packet/v1",
        "packet_id": "repository-step-5-evidence-sync",
        "title": "Synchronize accepted repository step 5 evidence",
        "status": "active",
        "baseline": {"ref": "main", "sha": BASELINE},
        "tracking_issues": [194],
        "objective": "Synchronize accepted repository step 5 evidence and expose repository step 6 as the only next implementation packet.",
        "allowed_paths": [
            "docs/ACTIVE_PACKET.md",
            "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md",
            "docs/IMPLEMENTATION_ROADMAP.md",
            "docs/PHASE8_DELIVERY_PLAN.md",
            "docs/PROJECT_STATUS.md",
            "repository-packet.json",
            "tests/test_architecture_documentation_consistency.py",
        ],
        "forbidden_paths": [
            "Cargo.lock",
            "contracts/**",
            "crates/**/src/**",
            "database/migrations/**",
            "modules/**/module.yaml",
            "modules/**/src/**",
            "proto/**",
            "services/**/src/**",
        ],
        "deliverables": [
            "record PR #228 accepted source, merge and 5-of-5 evidence",
            "mark repository step 5 complete",
            "mark repository step 6 next",
            "regenerate docs/ACTIVE_PACKET.md",
        ],
        "required_checks": ["Governance CI", "Affected Scope CI", "Rust CI", "Rust Generated Sync"],
        "acceptance": [
            "all authoritative status documents agree on accepted step 5 evidence",
            "repository step 6 is the only next implementation packet",
            "generated active-packet navigation is fresh",
            "no runtime, contract, manifest, persistence, migration, dependency, Cargo.lock or product behavior changes",
        ],
        "non_goals": [
            "implement repository step 6",
            "change Customer Privacy runtime",
            "change public contracts or route classifications",
            "change workspace packages or dependencies",
        ],
    }
    write("repository-packet.json", json.dumps(packet, indent=2) + "\n")


def main() -> int:
    synchronize_status()
    synchronize_roadmap()
    synchronize_phase8()
    synchronize_plan()
    synchronize_test()
    write_packet()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
