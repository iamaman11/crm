#!/usr/bin/env python3
from __future__ import annotations

import json
from pathlib import Path
import re
import textwrap

SOURCE = "f926ece93dc2b24683f982828e72bf9170dc123a"
MERGE = "9f21a2b40f6af5ce57045fc4c1fbfc1bd6cb5b90"
ACCEPTED = (
    f"Repository step 8 is accepted through PR #237 / accepted source `{SOURCE}` / "
    f"squash merge `{MERGE}` / 33 of 33 applicable permanent workflows on one "
    "unchanged source-authored head. It persists deterministic tenant-bound owner execution "
    "attempts, checkpoints and safe outcomes with immutable action-plan and retention-decision "
    "lineage; composes exactly nine canonical owner endpoints through trusted-internal production "
    "wiring; recovers from pre-invocation, post-owner-result and post-outcome/pre-checkpoint crash "
    "windows without duplicate owner invocation; and makes "
    "`customer_privacy.case.owner_outcomes.list@1.0.0` paginate real persisted payload-safe outcomes. "
    "Activation, registered initiating-capability attribution, FORCE RLS, strict rehydration, "
    "idempotency, audit, clean apply, rollback, reapply and repeated PostgreSQL acceptance are "
    "proven. Public inventory remains 7 mutations / 4 permission-aware queries / 0 workers; no "
    "public route, worker, access/export assembly, destructive owner execution, crate, dependency, "
    "`Cargo.lock`, workspace-package or generic-runtime algorithm change was introduced."
)


def replace_once(path: str, old: str, new: str) -> None:
    target = Path(path)
    content = target.read_text(encoding="utf-8")
    count = content.count(old)
    if count != 1:
        raise SystemExit(f"{path}: expected one exact match, found {count}: {old[:120]!r}")
    target.write_text(content.replace(old, new, 1), encoding="utf-8")


def replace_regex(path: str, pattern: str, replacement: str) -> None:
    target = Path(path)
    content = target.read_text(encoding="utf-8")
    content, count = re.subn(pattern, replacement, content, count=1, flags=re.S)
    if count != 1:
        raise SystemExit(f"{path}: regex replacement count {count}: {pattern[:120]!r}")
    target.write_text(content, encoding="utf-8")


def method(source: str) -> str:
    return textwrap.indent(textwrap.dedent(source).lstrip(), "    ")


packet = {
    "schema_version": "crm.repository-packet/v1",
    "packet_id": "repository-step-8-evidence-sync",
    "title": "Synchronize accepted repository step 8 evidence",
    "status": "active",
    "baseline": {"ref": "main", "sha": MERGE},
    "tracking_issues": [126, 194],
    "objective": (
        "Synchronize accepted PR #237 resumable Customer Privacy owner-execution evidence across "
        "the normative repository plans, mark repository step 8 complete, and expose repository "
        "step 9 as the only next implementation packet."
    ),
    "allowed_paths": [
        "docs/ACTIVE_PACKET.md",
        "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md",
        "docs/IMPLEMENTATION_ROADMAP.md",
        "docs/PHASE8_DELIVERY_PLAN.md",
        "docs/PROJECT_STATUS.md",
        "repository-packet.json",
        "tests/test_architecture_documentation_consistency.py",
        "tests/test_repository_navigation.py",
    ],
    "forbidden_paths": [
        ".github/workflows/**",
        "Cargo.lock",
        "Cargo.toml",
        "contracts/**",
        "crates/**",
        "database/**",
        "modules/**",
        "packages/**",
        "proto/**",
        "scripts/**",
        "services/**",
    ],
    "deliverables": [
        "record PR #237 accepted source, squash merge and 33-of-33 permanent workflow evidence",
        "mark repository step 8 complete and repository step 9 next",
        "record durable replay-safe owner execution and real persisted outcome reads",
        "record unchanged public inventory, worker count, generic runtime, package and dependency budgets",
        "regenerate docs/ACTIVE_PACKET.md and synchronize permanent documentation guards",
    ],
    "required_checks": [
        "Governance CI",
        "Affected Scope CI",
        "Rust CI",
        "Rust Generated Sync",
    ],
    "acceptance": [
        "all authoritative status documents agree on PR #237 accepted evidence",
        "repository step 9 is the only next implementation packet",
        "generated active-packet navigation is fresh",
        "no runtime, contract, manifest, dependency, Cargo.lock, persistence, migration, public inventory or product behavior changes",
    ],
    "non_goals": [
        "implement repository step 9 affected-scope expansion",
        "change Customer Privacy runtime, public coordinates or inventory",
        "change generic runtime algorithms, owner-specific product semantics or worker inventory",
        "change Cargo.lock, manifests, dependencies or workspace packages",
        "change persistence, migrations or unrelated documentation",
    ],
}
Path("repository-packet.json").write_text(
    json.dumps(packet, indent=2, ensure_ascii=False) + "\n", encoding="utf-8"
)

# Normative architecture plan.
replace_once(
    "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md",
    "| C — golden owner package and persistence model | **In progress** | Customer Privacy domain/application/postgres/production pilot, transaction-scoped final policy port, authoritative deny-only restriction decision, public restriction/legal-hold placement, legal-hold/mandatory-retention adjudication and first protected-owner integration are accepted; ordinary Customer Privacy capabilities add zero crates | generalize scaffolding, migration ownership and visibility policy; adopt the model for later owners without forced rewrites |",
    "| C — golden owner package and persistence model | **In progress** | Customer Privacy domain/application/postgres/production pilot, transaction-scoped final policy port, authoritative deny-only restriction decision, public restriction/legal-hold placement, legal-hold/mandatory-retention adjudication, durable replay-safe owner execution/outcomes and first protected-owner integration are accepted; ordinary Customer Privacy capabilities add zero crates | generalize scaffolding, migration ownership and visibility policy; adopt the model for later owners without forced rewrites |",
)
replace_once(
    "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md",
    "The next permitted implementation packet is repository step 8: replay-safe resumable Customer Privacy owner execution and crash-window recovery.",
    f"{ACCEPTED}\n\nThe next permitted implementation packet is repository step 9: affected-scope expansion for contracts, migrations, PostgreSQL/process and product-plane checks.",
)
replace_once(
    "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md",
    "The inserted repository step 4 prerequisite is complete through PR #224, repository step 4 is complete through PR #226, repository step 5 is complete through PR #228, the inserted lockfile-preservation prerequisite before repository step 6 is complete through PR #232, repository step 6 is complete through PR #230, and repository step 7 is complete through PR #235. None changes the master numbering.",
    "The inserted repository step 4 prerequisite is complete through PR #224, repository step 4 is complete through PR #226, repository step 5 is complete through PR #228, the inserted lockfile-preservation prerequisite before repository step 6 is complete through PR #232, repository step 6 is complete through PR #230, repository step 7 is complete through PR #235, and repository step 8 is complete through PR #237. None changes the master numbering.",
)
replace_once(
    "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md",
    "8. replay-safe resumable Customer Privacy owner execution and crash-window recovery — **Next**;\n9. affected-scope expansion for contracts, migrations, PostgreSQL/process and product-plane checks;",
    "8. replay-safe resumable Customer Privacy owner execution and crash-window recovery — **Complete through PR #237**;\n9. affected-scope expansion for contracts, migrations, PostgreSQL/process and product-plane checks — **Next**;",
)
replace_once(
    "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md",
    "- permission-aware plan and future-safe empty outcome reads;",
    "- permission-aware plan and persisted payload-safe outcome reads;",
)
replace_once(
    "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md",
    "- public legal-hold placement and authoritative legal-hold-over-mandatory-retention-over-approved-action adjudication accepted through PR #230.",
    "- public legal-hold placement and authoritative legal-hold-over-mandatory-retention-over-approved-action adjudication accepted through PR #230;\n- durable replay-safe exact-nine owner execution, checkpoints and persisted safe outcomes accepted through PR #237.",
)
replace_once(
    "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md",
    "The next permitted repository packet is **repository step 8: replay-safe resumable Customer Privacy owner execution and crash-window recovery**. No repository step 9 or later work may begin before step 8 has unchanged exact-head acceptance and evidence synchronization.",
    f"{ACCEPTED}\n\nThe next permitted repository packet is **repository step 9: affected-scope expansion for contracts, migrations, PostgreSQL/process and product-plane checks**. No repository step 10 or later work may begin before step 9 has unchanged exact-head acceptance and evidence synchronization.",
)

# Implementation roadmap.
replace_once(
    "docs/IMPLEMENTATION_ROADMAP.md",
    "- Stage C Customer Privacy golden owner packages — **In progress**: package baseline complete through PR #205, final customer-subject policy prerequisite accepted through PR #224, immediate deny-only restriction placement/final owner guard accepted through PR #226, and legal-hold/mandatory-retention precedence accepted through PR #230.",
    "- Stage C Customer Privacy golden owner packages — **In progress**: package baseline complete through PR #205, final customer-subject policy prerequisite accepted through PR #224, immediate deny-only restriction placement/final owner guard accepted through PR #226, legal-hold/mandatory-retention precedence accepted through PR #230, and durable replay-safe owner execution/outcomes accepted through PR #237.",
)
replace_once(
    "docs/IMPLEMENTATION_ROADMAP.md",
    "Accepted architecture evidence includes PR #197, PR #199, PR #200, PR #203, PR #204, PR #205, PR #218, PR #222, PR #224, PR #226, PR #228, PR #232, PR #230 and PR #235.",
    "Accepted architecture evidence includes PR #197, PR #199, PR #200, PR #203, PR #204, PR #205, PR #218, PR #222, PR #224, PR #226, PR #228, PR #232, PR #230, PR #235 and PR #237.",
)
replace_once(
    "docs/IMPLEMENTATION_ROADMAP.md",
    "The architecture stage table is completion accounting, not a set of independent work queues. The binding repository order is section 2.4 of the architecture plan. Repository steps 1–7 are complete through PR #218, PR #220, PR #222, PR #226, PR #228, PR #230 and PR #235; PR #224 accepted the smallest inserted prerequisite required by step 4, and PR #232 accepted the smallest lockfile-preservation prerequisite required before step 6. Repository step 8 — replay-safe resumable Customer Privacy owner execution and crash-window recovery — is the current next packet.",
    "The architecture stage table is completion accounting, not a set of independent work queues. The binding repository order is section 2.4 of the architecture plan. Repository steps 1–8 are complete through PR #218, PR #220, PR #222, PR #226, PR #228, PR #230, PR #235 and PR #237; PR #224 accepted the smallest inserted prerequisite required by step 4, and PR #232 accepted the smallest lockfile-preservation prerequisite required before step 6. Repository step 9 — affected-scope expansion for contracts, migrations, PostgreSQL/process and product-plane checks — is the current next packet.",
)
replace_once(
    "docs/IMPLEMENTATION_ROADMAP.md",
    "Latest accepted runtime inventory is seven public mutations, four permission-aware public queries and zero Customer Privacy workers through PR #230. `customer_privacy.plan.build@1.0.0` remains accepted trusted-internal runtime without public ingress.",
    "Latest accepted runtime inventory is seven public mutations, four permission-aware public queries and zero Customer Privacy workers through PR #237. `customer_privacy.plan.build@1.0.0` and `customer_privacy.retention.evaluate@1.0.0` remain accepted trusted-internal runtime without public ingress; repository-step-8 owner execution is also trusted-internal and registers no public route or worker.",
)
replace_once(
    "docs/IMPLEMENTATION_ROADMAP.md",
    "Owner outcomes remain an empty deterministic terminal page with bounded request validation, stable page/terminal digests, no synthetic records and no outcome persistence.",
    "Owner outcomes remained an empty deterministic terminal page at the historical PR #211 boundary. PR #237 later accepted durable owner outcomes and real permission-aware bounded pagination without changing the public query coordinate.",
)
replace_once(
    "docs/IMPLEMENTATION_ROADMAP.md",
    "Repository step 7 is accepted through PR #235 / accepted source `7a0cd34dc17085ecd1a8ee233171c0463d91ceba` / squash merge `43d194231fbce1cee28c44e89726929e450f3d18` / 17 of 17 applicable permanent workflows on one unchanged source-authored head. One business-neutral mutation/query conformance support boundary is reused by representative `customer_enrichment.request.create@1.0.0` and permission-aware paginated `customer_privacy.case.list@1.0.0` real-process acceptance. Mutation conformance proves activation, malformed input, tenant mismatch, live authorization denial, exact replay and incompatible replay conflict, safe error classification, rejected-call no-side-effects and atomic record/relationship/event/audit/idempotency/business evidence. Query conformance proves activation, live authorization denial, tenant mismatch and cross-tenant concealment, malformed cursor rejection, bounded keyset pagination, stable safe errors and zero query-side writes. Owner-specific fixture construction, response decoding and domain semantics remain outside the generic suite. Public coordinates and inventory, generic runtime algorithms, crates, dependencies, manifests, `Cargo.lock`, the 113-package workspace, migrations and workers are unchanged.\n\n## 6. Binding active sequence",
    "Repository step 7 is accepted through PR #235 / accepted source `7a0cd34dc17085ecd1a8ee233171c0463d91ceba` / squash merge `43d194231fbce1cee28c44e89726929e450f3d18` / 17 of 17 applicable permanent workflows on one unchanged source-authored head. One business-neutral mutation/query conformance support boundary is reused by representative `customer_enrichment.request.create@1.0.0` and permission-aware paginated `customer_privacy.case.list@1.0.0` real-process acceptance. Mutation conformance proves activation, malformed input, tenant mismatch, live authorization denial, exact replay and incompatible replay conflict, safe error classification, rejected-call no-side-effects and atomic record/relationship/event/audit/idempotency/business evidence. Query conformance proves activation, live authorization denial, tenant mismatch and cross-tenant concealment, malformed cursor rejection, bounded keyset pagination, stable safe errors and zero query-side writes. Owner-specific fixture construction, response decoding and domain semantics remain outside the generic suite. Public coordinates and inventory, generic runtime algorithms, crates, dependencies, manifests, `Cargo.lock`, the 113-package workspace, migrations and workers are unchanged.\n\n### 5.13 Accepted replay-safe resumable owner execution\n\n" + ACCEPTED + "\n\n## 6. Binding active sequence",
)
replace_once(
    "docs/IMPLEMENTATION_ROADMAP.md",
    "8. **Repository step 8 — replay-safe resumable Customer Privacy owner execution and crash-window recovery — Next.**\n9–19. **Repository steps 9–19 — continue exactly as numbered in the architecture plan.**",
    "8. **Repository step 8 — replay-safe resumable Customer Privacy owner execution and crash-window recovery — Complete through PR #237.**\n9. **Repository step 9 — affected-scope expansion for contracts, migrations, PostgreSQL/process and product-plane checks — Next.**\n10–19. **Repository steps 10–19 — continue exactly as numbered in the architecture plan.**",
)
replace_once(
    "docs/IMPLEMENTATION_ROADMAP.md",
    "The inserted prerequisites did not renumber the master sequence. Repository step 8 is now the only next permitted implementation packet.",
    "The inserted prerequisites did not renumber the master sequence. Repository step 9 is now the only next permitted implementation packet.",
)

# Phase 8 delivery plan.
replace_once(
    "docs/PHASE8_DELIVERY_PLAN.md",
    "Latest accepted public runtime inventory is seven mutations, four permission-aware public queries and zero Customer Privacy workers through PR #230. Trusted-internal `customer_privacy.plan.build@1.0.0` still has no public ingress.",
    "Latest accepted public runtime inventory is seven mutations, four permission-aware public queries and zero Customer Privacy workers through PR #237. Trusted-internal `customer_privacy.plan.build@1.0.0`, `customer_privacy.retention.evaluate@1.0.0` and repository-step-8 owner execution have no public ingress.",
)
replace_once(
    "docs/PHASE8_DELIVERY_PLAN.md",
    "Repository step 8 is now the next permitted implementation packet.",
    f"{ACCEPTED}\n\nRepository step 9 is now the next permitted implementation packet.",
)
replace_once(
    "docs/PHASE8_DELIVERY_PLAN.md",
    "2. `customer_privacy.case.owner_outcomes.list@1.0.0` with bounded validation and a deterministic empty terminal page (`items = []`) until owner execution and outcome persistence exist.",
    "2. `customer_privacy.case.owner_outcomes.list@1.0.0` with bounded validation and a deterministic empty terminal page (`items = []`) at the historical PR #211 boundary. PR #237 later accepted durable safe outcomes and real permission-aware pagination without changing this public coordinate.",
)
replace_once(
    "docs/PHASE8_DELIVERY_PLAN.md",
    "Repository step 7 is accepted through PR #235 / accepted source `7a0cd34dc17085ecd1a8ee233171c0463d91ceba` / squash merge `43d194231fbce1cee28c44e89726929e450f3d18` / 17 of 17 applicable permanent workflows on one unchanged source-authored head. One business-neutral mutation/query conformance support boundary is reused by representative `customer_enrichment.request.create@1.0.0` and permission-aware paginated `customer_privacy.case.list@1.0.0` real-process acceptance. Mutation conformance proves activation, malformed input, tenant mismatch, live authorization denial, exact replay and incompatible replay conflict, safe error classification, rejected-call no-side-effects and atomic record/relationship/event/audit/idempotency/business evidence. Query conformance proves activation, live authorization denial, tenant mismatch and cross-tenant concealment, malformed cursor rejection, bounded keyset pagination, stable safe errors and zero query-side writes. Owner-specific fixture construction, response decoding and domain semantics remain outside the generic suite. Public coordinates and inventory, generic runtime algorithms, crates, dependencies, manifests, `Cargo.lock`, the 113-package workspace, migrations and workers are unchanged.\n\n## 9. Binding repository continuation",
    "Repository step 7 is accepted through PR #235 / accepted source `7a0cd34dc17085ecd1a8ee233171c0463d91ceba` / squash merge `43d194231fbce1cee28c44e89726929e450f3d18` / 17 of 17 applicable permanent workflows on one unchanged source-authored head. One business-neutral mutation/query conformance support boundary is reused by representative `customer_enrichment.request.create@1.0.0` and permission-aware paginated `customer_privacy.case.list@1.0.0` real-process acceptance. Mutation conformance proves activation, malformed input, tenant mismatch, live authorization denial, exact replay and incompatible replay conflict, safe error classification, rejected-call no-side-effects and atomic record/relationship/event/audit/idempotency/business evidence. Query conformance proves activation, live authorization denial, tenant mismatch and cross-tenant concealment, malformed cursor rejection, bounded keyset pagination, stable safe errors and zero query-side writes. Owner-specific fixture construction, response decoding and domain semantics remain outside the generic suite. Public coordinates and inventory, generic runtime algorithms, crates, dependencies, manifests, `Cargo.lock`, the 113-package workspace, migrations and workers are unchanged.\n\n### 8.8 Accepted replay-safe resumable owner execution\n\n" + ACCEPTED + "\n\n## 9. Binding repository continuation",
)
replace_once(
    "docs/PHASE8_DELIVERY_PLAN.md",
    "8. repository step 8 — replay-safe resumable owner execution and crash-window recovery — **next**;\n9. repository step 9 — affected-scope expansion for contracts, migrations, PostgreSQL/process and product checks;",
    "8. repository step 8 — replay-safe resumable owner execution and crash-window recovery — **complete through PR #237**;\n9. repository step 9 — affected-scope expansion for contracts, migrations, PostgreSQL/process and product checks — **next**;",
)
replace_once(
    "docs/PHASE8_DELIVERY_PLAN.md",
    "The inserted prerequisites did not renumber the normative master sequence. A later step must not start while repository step 8 is unfinished.",
    "The inserted prerequisites did not renumber the normative master sequence. A later step must not start while repository step 9 is unfinished.",
)
replace_once(
    "docs/PHASE8_DELIVERY_PLAN.md",
    "It closes only after restrictions and legal holds are extended with release/read lifecycle where required, owner execution, access/export, deletion/anonymization/crypto-shred, tombstone/no-orphan behavior, convergence, worker lifecycle, frontend/operations evidence and full process acceptance are merged in the binding repository order.",
    "It closes only after restrictions and legal holds are extended with release/read lifecycle where required, access/export, deletion/anonymization/crypto-shred, tombstone/no-orphan behavior, convergence, worker lifecycle, frontend/operations evidence and full process acceptance are merged in the binding repository order.",
)

# Concise project status.
replace_once("docs/PROJECT_STATUS.md", "Status date: 2026-07-29", "Status date: 2026-07-30")
replace_once(
    "docs/PROJECT_STATUS.md",
    "Latest accepted Customer Privacy runtime baseline is PR #230 / accepted source `131285e07ad7c36c00e399b65d55591db13f0948` / squash merge `18e6218a7e7495219ac9e8c71cafcda1be64a31b` / 32 of 32 permanent workflows.",
    f"Latest accepted Customer Privacy runtime baseline is PR #237 / accepted source `{SOURCE}` / squash merge `{MERGE}` / 33 of 33 applicable permanent workflows on one unchanged source-authored head.",
)
replace_once(
    "docs/PROJECT_STATUS.md",
    "Latest accepted repository architecture/developer-experience packet is PR #232 / accepted source `3f09dcc595f79d633915e4a67117aedc59ed2499` / squash merge `3eab6dcd1d03a15ef0ce148d7f74137d2e1d10ed` / 5 of 5 applicable permanent workflows.",
    f"Latest accepted repository implementation packet is PR #237 / accepted source `{SOURCE}` / squash merge `{MERGE}` / 33 of 33 applicable permanent workflows.",
)
replace_once(
    "docs/PROJECT_STATUS.md",
    "The accepted inventory is seven public mutations (`case.create`, `case.submit`, `case.subject.verify`, `case.cancel`, `case.approve`, `restriction.place`, `legal_hold.place`), four permission-aware public queries (`case.get`, `case.list`, `case.plan.get`, `case.owner_outcomes.list`) and zero Customer Privacy workers. `customer_privacy.plan.build@1.0.0` remains trusted-internal runtime with no public route.",
    "The accepted inventory is seven public mutations (`case.create`, `case.submit`, `case.subject.verify`, `case.cancel`, `case.approve`, `restriction.place`, `legal_hold.place`), four permission-aware public queries (`case.get`, `case.list`, `case.plan.get`, `case.owner_outcomes.list`) and zero Customer Privacy workers. `customer_privacy.plan.build@1.0.0`, `customer_privacy.retention.evaluate@1.0.0` and repository-step-8 owner execution remain trusted-internal with no public route.",
)
replace_once(
    "docs/PROJECT_STATUS.md",
    "`case.owner_outcomes.list` validates bounded page/cursor input and returns a deterministic empty terminal page (`items = []`, empty terminal cursor) because owner execution and outcome persistence remain absent. Stable page/terminal digests and safe allow/deny evidence are append-only in a FORCE-RLS audit table. No outcome table, synthetic outcomes, mutation or worker is added.",
    "At the historical PR #211 boundary, `case.owner_outcomes.list` returned a deterministic empty terminal page. PR #237 later accepted durable FORCE-RLS owner outcomes and real permission-aware bounded pagination with stable safe page evidence, without adding a mutation, worker or new public coordinate.",
)
replace_once(
    "docs/PROJECT_STATUS.md",
    "Repository step 7 is accepted through PR #235 / accepted source `7a0cd34dc17085ecd1a8ee233171c0463d91ceba` / squash merge `43d194231fbce1cee28c44e89726929e450f3d18` / 17 of 17 applicable permanent workflows on one unchanged source-authored head. One business-neutral mutation/query conformance support boundary is reused by representative `customer_enrichment.request.create@1.0.0` and permission-aware paginated `customer_privacy.case.list@1.0.0` real-process acceptance. Mutation conformance proves activation, malformed input, tenant mismatch, live authorization denial, exact replay and incompatible replay conflict, safe error classification, rejected-call no-side-effects and atomic record/relationship/event/audit/idempotency/business evidence. Query conformance proves activation, live authorization denial, tenant mismatch and cross-tenant concealment, malformed cursor rejection, bounded keyset pagination, stable safe errors and zero query-side writes. Owner-specific fixture construction, response decoding and domain semantics remain outside the generic suite. Public coordinates and inventory, generic runtime algorithms, crates, dependencies, manifests, `Cargo.lock`, the 113-package workspace, migrations and workers are unchanged.\n\n## Next permitted repository packet",
    "Repository step 7 is accepted through PR #235 / accepted source `7a0cd34dc17085ecd1a8ee233171c0463d91ceba` / squash merge `43d194231fbce1cee28c44e89726929e450f3d18` / 17 of 17 applicable permanent workflows on one unchanged source-authored head. One business-neutral mutation/query conformance support boundary is reused by representative `customer_enrichment.request.create@1.0.0` and permission-aware paginated `customer_privacy.case.list@1.0.0` real-process acceptance. Mutation conformance proves activation, malformed input, tenant mismatch, live authorization denial, exact replay and incompatible replay conflict, safe error classification, rejected-call no-side-effects and atomic record/relationship/event/audit/idempotency/business evidence. Query conformance proves activation, live authorization denial, tenant mismatch and cross-tenant concealment, malformed cursor rejection, bounded keyset pagination, stable safe errors and zero query-side writes. Owner-specific fixture construction, response decoding and domain semantics remain outside the generic suite. Public coordinates and inventory, generic runtime algorithms, crates, dependencies, manifests, `Cargo.lock`, the 113-package workspace, migrations and workers are unchanged.\n\n## Accepted replay-safe resumable owner execution\n\n" + ACCEPTED + "\n\n## Next permitted repository packet",
)
replace_once(
    "docs/PROJECT_STATUS.md",
    "Repository step 8 is replay-safe resumable Customer Privacy owner execution and crash-window recovery.",
    "Repository step 9 is affected-scope expansion for contracts, migrations, PostgreSQL/process and product-plane checks.",
)
replace_once(
    "docs/PROJECT_STATUS.md",
    "Repository step 9 is affected-scope expansion for contracts, migrations, PostgreSQL/process and product-plane checks.",
    "Repository step 10 is governed Customer Privacy access/export assembly.",
)
# The preceding two replacements target adjacent sections; restore the first section explicitly.
replace_once(
    "docs/PROJECT_STATUS.md",
    "## Next permitted repository packet\n\nRepository step 10 is governed Customer Privacy access/export assembly.",
    "## Next permitted repository packet\n\nRepository step 9 is affected-scope expansion for contracts, migrations, PostgreSQL/process and product-plane checks.",
)
replace_once(
    "docs/PROJECT_STATUS.md",
    "## Following permitted repository packet\n\nRepository step 9 is affected-scope expansion for contracts, migrations, PostgreSQL/process and product-plane checks.",
    "## Following permitted repository packet\n\nRepository step 10 is governed Customer Privacy access/export assembly.",
)
replace_once(
    "docs/PROJECT_STATUS.md",
    "- Stage C is in progress: the Customer Privacy golden package model, final customer-subject policy prerequisite, authoritative restriction decision, public restriction/legal-hold placement, retention adjudication and first protected-owner integration are accepted; broader owner adoption and migration/visibility generalization remain.",
    "- Stage C is in progress: the Customer Privacy golden package model, final customer-subject policy prerequisite, authoritative restriction decision, public restriction/legal-hold placement, retention adjudication, durable replay-safe owner execution/outcomes and first protected-owner integration are accepted; broader owner adoption and migration/visibility generalization remain.",
)
replace_once(
    "docs/PROJECT_STATUS.md",
    "-> 7. reusable generic mutation/query conformance — complete through PR #235\n-> 8. replay-safe resumable Customer Privacy owner execution and crash-window recovery — next",
    "-> 7. reusable generic mutation/query conformance — complete through PR #235\n-> 8. replay-safe resumable Customer Privacy owner execution and crash-window recovery — complete through PR #237\n-> 9. affected-scope expansion for contracts, migrations, PostgreSQL/process and product-plane checks — next",
)

# Permanent documentation guards.
architecture_method = method(f'''
def test_active_packet_is_machine_declared_and_generated(self) -> None:
    self.assertEqual(self.packet["schema_version"], "crm.repository-packet/v1")
    self.assertEqual(self.packet["packet_id"], "repository-step-8-evidence-sync")
    self.assertEqual(self.packet["status"], "active")
    self.assertEqual(self.packet["baseline"]["sha"], "{MERGE}")
    self.assertEqual(self.packet["tracking_issues"], [126, 194])
    for path in (
        "docs/ACTIVE_PACKET.md",
        "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md",
        "docs/IMPLEMENTATION_ROADMAP.md",
        "docs/PHASE8_DELIVERY_PLAN.md",
        "docs/PROJECT_STATUS.md",
        "repository-packet.json",
        "tests/test_architecture_documentation_consistency.py",
        "tests/test_repository_navigation.py",
    ):
        self.assertIn(path, self.packet["allowed_paths"])
    for path in (".github/workflows/**", "Cargo.toml", "Cargo.lock", "crates/**", "database/**", "services/**"):
        self.assertIn(path, self.packet["forbidden_paths"])
    for check in ("Governance CI", "Affected Scope CI", "Rust CI", "Rust Generated Sync"):
        self.assertIn(check, self.packet["required_checks"])

    self.assertIn("Generated by scripts/generate_repository_navigation.py", self.active_packet)
    self.assertIn("repository-step-8-evidence-sync", self.active_packet)
    self.assertIn(self.packet["baseline"]["sha"], self.active_packet)
    self.assertRegex(self.active_packet, r"sha256:[0-9a-f]{{64}}")
    self.assertIn("orientation only", self.active_packet)

    for document in self.authoritative_status_documents:
        self.assertIn("PR #237", document)
        self.assertIn("{SOURCE}", document)
        self.assertIn("{MERGE}", document)
        self.assertIn("33 of 33", document)
        self.assertIn("repository step 9", document.lower())

''')
replace_regex(
    "tests/test_architecture_documentation_consistency.py",
    r"    def test_active_packet_is_machine_declared_and_generated\(self\) -> None:\n.*?(?=    def test_repository_map_matches_authoritative_inventory)",
    architecture_method,
)

navigation_method = method(f'''
def test_active_packet_declaration_is_valid_and_exact(self) -> None:
    packet = load_packet(ROOT)
    self.assertEqual(packet["packet_id"], "repository-step-8-evidence-sync")
    self.assertEqual(packet["status"], "active")
    self.assertEqual(packet["baseline"]["ref"], "main")
    self.assertEqual(packet["baseline"]["sha"], "{MERGE}")
    self.assertEqual(packet["tracking_issues"], [126, 194])
    for path in (
        "docs/ACTIVE_PACKET.md",
        "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md",
        "docs/IMPLEMENTATION_ROADMAP.md",
        "docs/PHASE8_DELIVERY_PLAN.md",
        "docs/PROJECT_STATUS.md",
        "repository-packet.json",
        "tests/test_architecture_documentation_consistency.py",
        "tests/test_repository_navigation.py",
    ):
        self.assertIn(path, packet["allowed_paths"])
    self.assertIn(".github/workflows/**", packet["forbidden_paths"])
    self.assertIn("Cargo.lock", packet["forbidden_paths"])
    self.assertIn("Governance CI", packet["required_checks"])
    self.assertIn("repository step 9 is the only next implementation packet", packet["acceptance"])

''')
replace_regex(
    "tests/test_repository_navigation.py",
    r"    def test_active_packet_declaration_is_valid_and_exact\(self\) -> None:\n.*?(?=    def test_affected_scope_workflow_executes_real_packet_check)",
    navigation_method,
)

fixture_method = method(f'''
def test_packet_check_reports_affected_scope_without_running_git_or_cargo(self) -> None:
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
    affected = {{
        "head_sha": "b" * 40,
        "changed_paths": changed_paths,
        "affected_packages": [],
        "selected_workflows": [
            {{
                "name": "Governance CI",
                "path": ".github/workflows/governance.yml",
                "selected": True,
                "reasons": ["test fixture"],
            }}
        ],
    }}
    with (
        patch("scripts.repository_navigation._git", return_value="{MERGE}"),
        patch("scripts.repository_navigation.build_report", return_value=affected),
        patch("scripts.repository_navigation.stale_generated_documents", return_value=[]),
    ):
        report = packet_check(ROOT, "origin/main")
    self.assertTrue(report["ok"])
    self.assertEqual(report["changed_paths"], changed_paths)
    self.assertEqual(report["blockers"], [])
    self.assertEqual(report["selected_workflows"][0]["name"], "Governance CI")

''')
replace_regex(
    "tests/test_repository_navigation.py",
    r"    def test_packet_check_reports_affected_scope_without_running_git_or_cargo\(self\) -> None:\n.*?(?=    def test_repo_parser_exposes_exact_step_5_commands)",
    fixture_method,
)
