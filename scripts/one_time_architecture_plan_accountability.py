from __future__ import annotations

import json
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


def replace_exact(path: str, old: str, new: str, *, expected: int = 1) -> None:
    target = ROOT / path
    text = target.read_text(encoding="utf-8")
    count = text.count(old)
    if count != expected:
        raise SystemExit(f"{path}: expected {expected} exact matches, found {count}")
    target.write_text(text.replace(old, new), encoding="utf-8")


def replace_between(path: str, start: str, end: str, replacement: str) -> None:
    target = ROOT / path
    text = target.read_text(encoding="utf-8")
    if text.count(start) != 1 or text.count(end) != 1:
        raise SystemExit(f"{path}: section markers are not exact")
    before, remainder = text.split(start, 1)
    _, after = remainder.split(end, 1)
    target.write_text(before + replacement + end + after, encoding="utf-8")


# Architecture plan: current checkpoint and stage accountability.
replace_exact(
    "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md",
    "Current execution checkpoint: **2026-07-30**",
    "Current execution checkpoint: **2026-07-31**",
)
replace_exact(
    "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md",
    "| B — dependency, crate and exception governance | **In progress** | reproducible baseline, crate justification, calibrated inheritance cohorts, root-family no-growth, exact Rust `1.97.1` toolchain/workspace `rust-version`, measured zero-warning Rust/Clippy baseline, lockfile-preserving Rust workflows and three exact expiring legacy lint exceptions | additional homogeneous dependency cohorts, removal of the three direct-lint exceptions, public-surface/fan-out calibration |",
    "| B — dependency, crate and exception governance | **In progress** | reproducible baseline, crate justification, calibrated inheritance cohorts, root-family no-growth, exact Rust `1.97.1` toolchain/workspace `rust-version`, measured zero-warning Rust/Clippy baseline, lockfile-preserving Rust workflows and three exact expiring legacy lint exceptions | additional homogeneous dependency cohorts, removal of the three direct-lint exceptions, public-surface/fan-out calibration, measured consolidation at step 13 and remeasurement at steps 20 and 23 |",
)
replace_exact(
    "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md",
    "| C — golden owner package and persistence model | **In progress** | Customer Privacy domain/application/postgres/production pilot, transaction-scoped final policy port, authoritative deny-only restriction decision, public restriction/legal-hold placement, legal-hold/mandatory-retention adjudication, durable replay-safe owner execution/outcomes and first protected-owner integration are accepted; ordinary Customer Privacy capabilities add zero crates | generalize scaffolding, migration ownership and visibility policy; adopt the model for later owners without forced rewrites |",
    "| C — golden owner package and persistence model | **In progress** | Customer Privacy domain/application/postgres/production pilot, transaction-scoped final policy port, authoritative deny-only restriction decision, public restriction/legal-hold placement, legal-hold/mandatory-retention adjudication, durable replay-safe owner execution/outcomes, governed access/export assembly and first protected-owner integration are accepted; ordinary Customer Privacy capabilities add zero crates | destructive owner execution at step 11, Party tombstone/convergence at step 14, complete worker lifecycle at step 17, Phase 8A closure at step 19, and later-owner adoption without forced rewrites |",
)
replace_exact(
    "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md",
    "| D — contribution aggregation | **In progress** | owner-owned contribution pattern proven; first bounded Customer Accounts registration-inventory aggregation accepted through the first-party bundle | migrate remaining owners, expand the first-party bundle and remove additional concrete domain imports from generic runtime |",
    "| D — contribution aggregation | **In progress** | owner-owned contribution pattern proven; first bounded Customer Accounts registration-inventory aggregation accepted through the first-party bundle | repository step 12 completes current first-party owner contribution entry points, expands the mechanically checked bundle and removes remaining ordinary-registration concrete domain imports from generic runtime |",
)
replace_exact(
    "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md",
    "| F — generic conformance and contract lifecycle | **In progress** | native conformance, manifest/route parity and exact-head gates exist; reusable cross-owner mutation/query conformance is accepted through PR #235 | reusable worker suites plus compatibility, deprecation and retirement enforcement |",
    "| F — generic conformance and contract lifecycle | **In progress** | native conformance, manifest/route parity and exact-head gates exist; reusable cross-owner mutation/query conformance is accepted through PR #235 | reusable worker conformance at step 15, real worker adoption at step 17, Phase 8A process proof at step 19, plus compatibility, deprecation and retirement enforcement |",
)
replace_exact(
    "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md",
    "| G — transitional consolidation | **Not started** | candidates and stop rules are defined | complete at least one behavior-neutral domain-cluster consolidation with measured improvement |",
    "| G — transitional consolidation | **Not started** | candidates and stop rules are defined | repository step 13 completes at least one behavior-neutral domain-cluster consolidation with measured improvement |",
)
replace_exact(
    "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md",
    "| H — reproducible environment and navigation | **In progress** | stable docs index, `affected`, `check-affected`, deterministic `explain`, fail-closed `packet-check`, generated active packet and repository map are accepted through PR #228 | `doctor`, `bootstrap`, `dev-up`, `dev-reset`, `seed-demo` and `smoke` |",
    "| H — reproducible environment and navigation | **In progress** | stable docs index, `affected`, `check-affected`, deterministic `explain`, fail-closed `packet-check`, generated active packet and repository map are accepted through PR #228 | repository step 16 implements and proves `doctor`, `bootstrap`, `dev-up`, `dev-reset`, `seed-demo` and `smoke` |",
)
replace_exact(
    "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md",
    "| I — frontend and operations parity | **Not started as a complete stage** | existing product/process checks remain preserved | domain-oriented frontend proof, accessibility/browser evidence, restore/SLO/performance/security/supply-chain gates |",
    "| I — frontend and operations parity | **Not started as a complete stage** | existing product/process checks remain preserved | repository step 18 adds domain-oriented frontend proof, accessibility/browser evidence and restore/SLO/performance/security/supply-chain gates; step 19 proves Phase 8A closure |",
)

stage_accountability = """
### 2.3.1 Stage-to-step accountability

Stages A–I are completion ledgers, not parallel queues. Repository steps are the only executable order. A step may advance more than one stage, while completed stages A and E remain mandatory invariants for every later packet.

| Stage | Remaining numbered accountability | Exit evidence |
|---|---|---|
| A — documentation and policy baseline | preserved by every packet and its evidence synchronization | one live source hierarchy; stale phase, owner, next-packet and completion claims fail permanent guards |
| B — dependency, crate and exception governance | steps 12, 13, 20 and 23, plus every structural preflight | no unjustified package growth; calibrated dependency/public-surface/fan-out budgets; expired exceptions zero; measured before/after reports |
| C — golden owner package and persistence model | steps 11, 14, 17 and 19 | destructive execution, tombstone/convergence, worker lifecycle and Phase 8A closure preserve owner, tenant, RLS, audit and rollback boundaries |
| D — contribution aggregation | **step 12** | every currently active first-party owner exposes a stable contribution entry point; ordinary capability/worker registration changes no generic runtime algorithm or source |
| E — affected-scope CI | preserved by every packet; rechecked at steps 20 and 23 | every changed path has explainable ownership and executable checks; unknown impact broadens fail closed |
| F — generic conformance and contract lifecycle | steps 15, 17 and 19 | reusable worker conformance is adopted by a real worker; contract compatibility/deprecation/retirement evidence remains enforceable |
| G — transitional consolidation | **step 13** | at least one domain cluster is consolidated behavior-neutrally with measured package, fan-out, public-surface or build/test improvement |
| H — reproducible environment and navigation | **step 16** | clean-machine `doctor`, `bootstrap`, `dev-up`, `dev-reset`, `seed-demo` and `smoke` are deterministic, repeatable and production-aligned |
| I — frontend and operations parity | steps 18 and 19 | critical journeys have component/browser/accessibility proof and executable restore/SLO/performance/security/supply-chain evidence |

### 2.3.2 Score-recovery accountability

The program does not raise scores by declaration. Each weak dimension has explicit evidence:

| Dimension | Primary correcting steps | Evidence required before 10/10 |
|---|---|---|
| Extensibility cost | 12, 13, 15, 17, 21 and 22 | ordinary capabilities touch one owner closure, zero generic-runtime files, zero new crates and no unrelated workflows; two contrasting later expert-domain waves keep that cost bounded as module count grows |
| Developer comprehension | 16 plus permanent Stage A guards | one supported command path from clean machine to running/demo/smoke state; `explain` and packet navigation resolve owner, contract, persistence, composition and required checks without stale live trackers |
| Build and CI scalability | 12, 13, 15, 20, 21 and 22 | representative leaf changes avoid unrelated full-workspace closure; every broadening has a machine-readable reason; duration and fan-out budgets are measured and enforced |
| Local development reproducibility | 16 | pinned-tool diagnostics, deterministic bootstrap/database/demo state, safe reset, production-aligned ingress and repeatable smoke proof on a clean environment |

Repository step 20 is a Phase 8A architecture measurement checkpoint. It cannot by itself declare architecture 10/10 or close issue #194. The earliest final claim is repository step 23, after two contrasting later expert-domain waves at steps 21 and 22 and only if every completion criterion in section 12 is mechanically proven.

"""
replace_exact(
    "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md",
    "\nRepository step 1 is accepted through PR #218",
    "\n" + stage_accountability + "Repository step 1 is accepted through PR #218",
)

old_sequence = """11. owner-specific deletion, anonymization and supported crypto-shred execution — **Next**;
12. first measured behavior-neutral transitional domain-cluster consolidation;
13. Party tombstone, no-orphan proof and projection/search/cache convergence;
14. reusable generic worker conformance suite;
15. deterministic local lifecycle commands: `doctor`, `bootstrap`, `dev-up`, `dev-reset`, `seed-demo` and `smoke`;
16. Customer Privacy worker, disable/uninstall fail-closed semantics and complete process/end-to-end acceptance;
17. Phase 8A frontend, accessibility, browser, restore, SLO, performance, security and supply-chain evidence;
18. Phase 8A closure;
19. architecture remeasurement, remaining-gate review and publication of the next numbered sequence;
20. first Phase 8B packet only after step 19 is accepted."""
new_sequence = """11. owner-specific deletion, anonymization and supported crypto-shred execution — **Next**;
12. complete first-party contribution aggregation for all currently active owners without behavior changes;
13. first measured behavior-neutral transitional domain-cluster consolidation;
14. Party tombstone, no-orphan proof and projection/search/cache convergence;
15. reusable generic worker conformance suite;
16. deterministic local lifecycle commands: `doctor`, `bootstrap`, `dev-up`, `dev-reset`, `seed-demo` and `smoke`;
17. Customer Privacy worker, disable/uninstall fail-closed semantics and complete process/end-to-end acceptance;
18. Phase 8A frontend, accessibility, browser, restore, SLO, performance, security and supply-chain evidence;
19. Phase 8A closure;
20. Phase 8A architecture remeasurement, remaining-gate review and publication of the measured Phase 8B extension baseline — **not a final 10/10 declaration**;
21. first Phase 8B expert-domain wave proving bounded extension cost;
22. second contrasting expert-domain wave proving bounded extension cost as module count grows;
23. final architecture 10/10 closure review only when every section 12 criterion is mechanically proven."""
replace_exact(
    "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md",
    old_sequence,
    new_sequence,
)
replace_exact(
    "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md",
    "No item may be described as “next” when an earlier unfinished item exists.",
    "Repository step 12 may be delivered as sequential bounded behavior-neutral owner batches, but it remains one unfinished master step until every currently active first-party owner satisfies section 6 completion evidence. Repository step 20 publishes measurements and remaining blockers; it cannot waive the two later-wave requirement or close issue #194.\n\nNo item may be described as “next” when an earlier unfinished item exists.",
)
replace_exact(
    "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md",
    "- Transitional capability-specific packages are consolidated gradually through behavior-neutral packets with measured benefit.\n",
    "- Transitional capability-specific packages are consolidated gradually through behavior-neutral packets with measured benefit.\n\nZero new crates is the default for an ordinary capability inside an existing owner, not an absolute ban. A genuinely new authoritative owner normally introduces three to five owner packages. A provider, secret, trust, process, extraction or compiler-enforced visibility boundary may justify an additional package when an internal module cannot protect that boundary. The accepted Customer Privacy capability packets through repository step 10 correctly reused existing owner packages because they introduced no new independent dependency, trust, process or ownership boundary. Repository step 11 must repeat that preflight; a real crypto/KMS boundary may justify a dedicated adapter, but one crate per action remains forbidden.\n",
)
replace_exact(
    "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md",
    "A crate is rejected when it contains only one handler, planner, thin re-export, copied validation or capability-specific composition function.\n",
    "Decision matrix:\n\n- ordinary command/query/worker in an existing owner → internal module in the existing application/adapter/production packages; zero new crates;\n- new authoritative mutable domain → a new owner module with the target three-to-five package layout;\n- provider SDK, arbitrary HTTP, secrets, broker, object storage, KMS/HSM or independent process → optional dedicated boundary crate when isolation is compiler-enforced and review-visible;\n- reusable abstraction → shared crate only after at least two contrasting real consumers prove common semantics;\n- handler, planner, thin re-export, copied validation or capability-specific composition fragment → reject the crate and keep it internal.\n\nA crate is rejected when it contains only one handler, planner, thin re-export, copied validation or capability-specific composition function.\n",
)
replace_exact(
    "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md",
    "Completion evidence:\n\n- adding a capability changes no generic runtime source;",
    "Repository step 12 is the bounded completion step for this program. It may use sequential owner batches, but feature behavior and consolidation remain excluded.\n\nCompletion evidence:\n\n- every currently active first-party owner exposes one stable contribution entry point;\n- adding a capability changes no generic runtime source;",
)
replace_exact(
    "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md",
    "Issue #194 closes only when:\n",
    "Repository step 20 performs the first full remeasurement after Phase 8A, but it cannot close issue #194 because the program also requires two later contrasting expert-domain waves. Repository step 23 is the earliest final closure review and succeeds only when every item below is executable, measured and difficult to bypass.\n\nIssue #194 closes only when:\n",
)

# Implementation roadmap.
replace_exact(
    "docs/IMPLEMENTATION_ROADMAP.md",
    "| 8B | #29 | Product Catalog, CPQ and quote-to-revenue lifecycle | **Planned; blocked on repository steps 1–19 and completed 8A** |",
    "| 8B | #29 | Product Catalog, CPQ and quote-to-revenue lifecycle | **Planned; blocked on repository steps 1–20 and completed 8A; first extension wave is step 21** |",
)
replace_exact(
    "docs/IMPLEMENTATION_ROADMAP.md",
    "- Stage D contribution aggregation — **In progress; first bounded Customer Accounts registration-inventory aggregation accepted through PR #222**.",
    "- Stage D contribution aggregation — **In progress; first bounded Customer Accounts registration-inventory aggregation accepted through PR #222; repository step 12 is the explicit completion step for all currently active first-party owners**.",
)
replace_exact(
    "docs/IMPLEMENTATION_ROADMAP.md",
    "- Stage H — **In progress**: deterministic explain, packet-check and generated navigation are accepted through PR #228; local lifecycle commands remain open.\n- Stage F — **In progress**: reusable generic mutation/query conformance is accepted through PR #235; worker conformance and contract lifecycle enforcement remain open.\n- Stages G and I — measured consolidation, frontend and operations parity remain open.",
    "- Stage H — **In progress**: deterministic explain, packet-check and generated navigation are accepted through PR #228; repository step 16 owns local lifecycle completion.\n- Stage F — **In progress**: reusable generic mutation/query conformance is accepted through PR #235; repository step 15 owns worker conformance and step 17 proves real adoption.\n- Stage G — **Not started**: repository step 13 owns the first measured behavior-neutral consolidation.\n- Stage I — **Not started as a complete stage**: repository step 18 owns frontend/operations evidence and step 19 proves Phase 8A closure.",
)
roadmap_mapping = """
### 3.1 Remaining stage-to-step ownership

The stage labels are accounting only; the architecture master sequence remains authoritative.

| Remaining step | Primary stage | Supporting stages |
|---|---|---|
| 11 — deletion/anonymization/crypto-shred | C | B, F |
| 12 — complete contribution aggregation | D | B, E |
| 13 — measured consolidation | G | B, D |
| 14 — tombstone/no-orphan/convergence | C | F |
| 15 — generic worker conformance | F | E |
| 16 — local lifecycle commands | H | A |
| 17 — Customer Privacy worker and full E2E | C, F | D, H |
| 18 — frontend and operations evidence | I | E |
| 19 — Phase 8A closure | C, F, I | A, E |
| 20 — Phase 8A architecture remeasurement | all stages | measurement only; no automatic 10/10 claim |
| 21–22 — contrasting later expert-domain waves | C, D | B, E, F, H, I |
| 23 — final 10/10 closure review | all stages | succeeds only if every normative criterion is proven |

"""
replace_exact(
    "docs/IMPLEMENTATION_ROADMAP.md",
    "\n## 4. Phase 8A completed foundation",
    "\n" + roadmap_mapping + "## 4. Phase 8A completed foundation",
)
old_roadmap_sequence = """11. **Repository step 11 — owner-specific deletion, anonymization and supported crypto-shred execution — Next.**
12–19. **Repository steps 12–19 — continue exactly as numbered in the architecture plan.**
20. **Repository step 20 — first Phase 8B packet.**"""
new_roadmap_sequence = """11. **Repository step 11 — owner-specific deletion, anonymization and supported crypto-shred execution — Next.**
12. **Repository step 12 — complete first-party contribution aggregation for all currently active owners without behavior changes.**
13. **Repository step 13 — first measured behavior-neutral transitional domain-cluster consolidation.**
14. **Repository step 14 — Party tombstone, no-orphan proof and projection/search/cache convergence.**
15. **Repository step 15 — reusable generic worker conformance.**
16. **Repository step 16 — deterministic local lifecycle commands.**
17. **Repository step 17 — Customer Privacy worker, disable/uninstall fail-closed semantics and full process acceptance.**
18. **Repository step 18 — Phase 8A frontend and operations evidence.**
19. **Repository step 19 — Phase 8A closure.**
20. **Repository step 20 — Phase 8A architecture remeasurement; not a final 10/10 declaration.**
21. **Repository step 21 — first Phase 8B expert-domain wave.**
22. **Repository step 22 — second contrasting expert-domain wave.**
23. **Repository step 23 — final architecture 10/10 closure review only after every criterion is mechanically proven.**"""
replace_exact(
    "docs/IMPLEMENTATION_ROADMAP.md",
    old_roadmap_sequence,
    new_roadmap_sequence,
)
replace_exact(
    "docs/IMPLEMENTATION_ROADMAP.md",
    "Phase 8B remains planned and blocked on repository steps 1–19 and completed Phase 8A.",
    "Phase 8B remains planned and blocked on repository steps 1–20 and completed Phase 8A. Repository step 21 begins the first measured extension wave; step 22 must provide a contrasting second wave before the final step 23 architecture closure review.",
)
replace_exact(
    "docs/IMPLEMENTATION_ROADMAP.md",
    "A module, phase or product is not complete merely because a crate, contract, migration or one backend path exists.",
    "Repository step 20 remeasures the Phase 8A architecture but cannot itself claim architecture 10/10. Final closure is no earlier than step 23 and requires the two later expert-domain waves plus every architecture completion criterion.\n\nA module, phase or product is not complete merely because a crate, contract, migration or one backend path exists.",
)

# Phase 8 plan.
old_phase_sequence = """11. repository step 11 — owner-specific deletion, anonymization and supported crypto-shred execution — **next**;
12. repository step 12 — first measured behavior-neutral consolidation;
13. repository step 13 — Party tombstone, no-orphan proof and projection/search/cache convergence;
14. repository step 14 — reusable generic worker conformance;
15. repository step 15 — deterministic local lifecycle commands;
16. repository step 16 — Customer Privacy worker, disable/uninstall fail-closed semantics and complete process/end-to-end acceptance;
17. repository step 17 — Phase 8A frontend and operations evidence;
18. repository step 18 — Phase 8A closure;
19. repository step 19 — architecture remeasurement and publication of the next numbered order;
20. repository step 20 — first Phase 8B packet."""
new_phase_sequence = """11. repository step 11 — owner-specific deletion, anonymization and supported crypto-shred execution — **next**;
12. repository step 12 — complete first-party contribution aggregation for all currently active owners without behavior changes;
13. repository step 13 — first measured behavior-neutral consolidation;
14. repository step 14 — Party tombstone, no-orphan proof and projection/search/cache convergence;
15. repository step 15 — reusable generic worker conformance;
16. repository step 16 — deterministic local lifecycle commands;
17. repository step 17 — Customer Privacy worker, disable/uninstall fail-closed semantics and complete process/end-to-end acceptance;
18. repository step 18 — Phase 8A frontend and operations evidence;
19. repository step 19 — Phase 8A closure;
20. repository step 20 — Phase 8A architecture remeasurement and remaining-gate review, not a final 10/10 declaration;
21. repository step 21 — first Phase 8B expert-domain wave;
22. repository step 22 — second contrasting expert-domain wave;
23. repository step 23 — final architecture 10/10 closure review only after every criterion is mechanically proven."""
replace_exact("docs/PHASE8_DELIVERY_PLAN.md", old_phase_sequence, new_phase_sequence)
phase_mapping = """
### 9.1 Stage accountability for the remaining Phase 8 sequence

| Step | Primary stage responsibility |
|---|---|
| 11 | C — destructive owner execution inside the golden owner model |
| 12 | D — complete contribution aggregation for current first-party owners |
| 13 | G — measured behavior-neutral consolidation |
| 14 | C — tombstone, no-orphan and convergence persistence model |
| 15 | F — reusable worker conformance |
| 16 | H — reproducible local lifecycle |
| 17 | C + F — real Customer Privacy worker and lifecycle proof |
| 18 | I — frontend and operations parity |
| 19 | C + F + I — Phase 8A closure |
| 20 | all stages — measurement checkpoint only |
| 21–22 | later-domain proof that extension cost remains bounded |
| 23 | final architecture closure review |

Step 12 is architecture refactoring only and must not change Customer Privacy product behavior. Step 13 is physical consolidation only and remains separate from feature behavior. Step 20 cannot close issue #194 or declare 10/10 before steps 21 and 22 provide contrasting later-domain evidence.

"""
replace_exact(
    "docs/PHASE8_DELIVERY_PLAN.md",
    "\n## 10. Frozen ownership",
    "\n" + phase_mapping + "## 10. Frozen ownership",
)
replace_exact(
    "docs/PHASE8_DELIVERY_PLAN.md",
    "Product Catalog, Pricing, CPQ, Orders, Contracts, Subscriptions and Billing remain blocked until the repository order and Phase 8A closure permit them.",
    "Product Catalog, Pricing, CPQ, Orders, Contracts, Subscriptions and Billing remain blocked until repository step 20 and Phase 8A closure permit step 21. Two contrasting later expert-domain waves at steps 21 and 22 are required before the step 23 final architecture 10/10 review.",
)

# Project status.
replace_exact("docs/PROJECT_STATUS.md", "Status date: 2026-07-30", "Status date: 2026-07-31")
replace_exact(
    "docs/PROJECT_STATUS.md",
    "- Stage D is in progress: the first bounded Customer Accounts registration-inventory aggregation is accepted through PR #222, but the generic runtime still imports many other concrete owner adapters and remaining owners have not migrated.",
    "- Stage D is in progress: the first bounded Customer Accounts registration-inventory aggregation is accepted through PR #222; repository step 12 now explicitly completes contribution entry points for all currently active first-party owners and removes remaining ordinary-registration concrete imports from generic runtime.",
)
replace_exact(
    "docs/PROJECT_STATUS.md",
    "- Stage H is in progress: deterministic explain, packet-check and generated navigation are accepted through PR #228; local lifecycle commands remain repository step 15.",
    "- Stage H is in progress: deterministic explain, packet-check and generated navigation are accepted through PR #228; local lifecycle commands are repository step 16.",
)
replace_exact(
    "docs/PROJECT_STATUS.md",
    "- Stage F is in progress: reusable generic mutation/query conformance is accepted through PR #235; generic worker conformance and contract lifecycle enforcement remain open.",
    "- Stage F is in progress: reusable generic mutation/query conformance is accepted through PR #235; generic worker conformance is repository step 15 and real Customer Privacy worker adoption is step 17; contract lifecycle enforcement remains open.",
)
replace_exact(
    "docs/PROJECT_STATUS.md",
    "- Stages G and I remain unstarted; measured consolidation, frontend and operations parity are not complete.",
    "- Stage G remains unstarted and is owned by repository step 13. Stage I remains incomplete and is owned by steps 18–19.",
)
replace_exact(
    "docs/PROJECT_STATUS.md",
    "Repository step 12 is the first measured behavior-neutral transitional domain-cluster consolidation.",
    "Repository step 12 completes first-party contribution aggregation for all currently active owners without behavior changes. Repository step 13 is the first measured behavior-neutral transitional domain-cluster consolidation.",
)
old_status_sequence = """-> 11. owner-specific deletion, anonymization and supported crypto-shred execution — next
```"""
new_status_sequence = """-> 11. owner-specific deletion, anonymization and supported crypto-shred execution — next
-> 12. complete first-party contribution aggregation for all currently active owners
-> 13. first measured behavior-neutral transitional domain-cluster consolidation
-> 14. Party tombstone, no-orphan proof and projection/search/cache convergence
-> 15. reusable generic worker conformance
-> 16. deterministic local lifecycle commands
-> 17. Customer Privacy worker and complete process/end-to-end acceptance
-> 18. Phase 8A frontend and operations evidence
-> 19. Phase 8A closure
-> 20. Phase 8A architecture remeasurement — checkpoint, not final 10/10
-> 21–22. two contrasting later expert-domain waves
-> 23. final architecture 10/10 closure review only if every criterion is mechanically proven
```"""
replace_exact("docs/PROJECT_STATUS.md", old_status_sequence, new_status_sequence)
status_boundary = """
## Architecture 10/10 declaration boundary

Repository step 20 is a measured Phase 8A checkpoint, not an automatic success declaration. Architecture 10/10 requires the completed Stage D packet at step 12, measured consolidation at step 13, worker/local/frontend/operations closure through step 19, two contrasting later expert-domain waves at steps 21 and 22, and a separate final review at step 23. Issue #194 remains open until every executable completion criterion is proven.

An ordinary capability in an existing owner still creates zero new crates. That is not a blanket ban: a new authoritative owner normally creates three to five owner packages, and a real provider, secrets, KMS/HSM, trust, process, extraction or compiler-enforced visibility boundary may justify a dedicated crate after architecture preflight.

"""
replace_exact(
    "docs/PROJECT_STATUS.md",
    "\n## Architecture and developer-experience 10/10 checkpoint",
    "\n" + status_boundary + "## Architecture and developer-experience 10/10 checkpoint",
)

# Module catalog: replace stale Customer Privacy and accounting sections.
module_section = """## 4. Customer Privacy boundary

Phase 8A.11 / issue #126 remains **In progress**.

Latest accepted public inventory is **seven mutations, four permission-aware public queries and zero Customer Privacy workers through PR #241**. Trusted-internal `customer_privacy.plan.build@1.0.0`, `customer_privacy.retention.evaluate@1.0.0`, repository-step-8 owner execution and `customer_privacy.access_export.request@1.0.0` have no public ingress.

Nine owner-scope contribution coordinates are published as non-public owner-owned reads. **All nine authoritative implementations are accepted:**

- PR #156 — Parties;
- PR #175 — Consents;
- PR #179 — Customer Accounts;
- PR #181 — Contact Points;
- PR #183 — Party Relationships;
- PR #186 — Identity Resolution / accepted source `24456b86379a1ef23ed5a60804cdcae5075d407c` / merge `509eb304a76055c9f49b0beed3b007963a91cb22` / 25 of 25 permanent workflows;
- PR #188 — Customer Data Operations / accepted source `07f34786e82fdfa78d263790e9f50541529006f8` / merge `089be72fa3010b4aa15aff7f9ea55fd86290f8fc` / 26 of 26 permanent workflows;
- PR #190 — Data Quality / accepted source `dcfe8faebc7462b888f8fc1721cb379a40fea88a` / merge `deac197c97cddc15bb9916092ca87f6e767ce1de` / 27 of 27 permanent workflows;
- PR #192 — Customer Enrichment / accepted source `e90e36027de18a07be68e43327ea732810ff332a` / merge `e41cbab0cd30819fcbe2e3c5f2c7415fc6de3e8c` / 28 of 28 permanent workflows.

Accepted Customer Privacy architecture/runtime evidence now includes:

- PR #206 — exact-nine discovery and immutable snapshots;
- PR #209 — deterministic planning runtime;
- PR #211 — permission-aware plan/outcome reads;
- PR #220 — public approval;
- PR #222 / source `b5651e784a156758b39eaa04abc1124c7c0832f9` / merge `fd86ab1408e435ccc9f47b7a86ab3dd66df64ec1` / 16 of 16 — first bounded contribution aggregation;
- PR #224 / source `e57307fcb1b5192d5e6340247cb6633f32b7ba34` / merge `67804d9478b2bbaf342a398b649e23bd5ead6c08` / 28 of 28 — final customer-subject policy prerequisite;
- PR #226 / source `ad08a691ec759b8b3b523fa66a034cecf4138ff0` / merge `a46460623e90c5649d36bedba055fb55023d9349` / 34 of 34 — public `customer_privacy.restriction.place@1.0.0` and first protected-owner enforcement;
- PR #230 — public legal hold and mandatory-retention precedence;
- PR #235 — reusable mutation/query conformance;
- PR #237 — durable replay-safe owner execution, checkpoints and real owner outcomes;
- PR #239 — multi-plane affected-scope enforcement;
- PR #241 / accepted source `2bb3a671deb18a6ae3bcea228ed01ed287b9de6a` / merge `19232f6f3e2ae87aabeb080257c1aac5477a6616` / 34 of 34 — governed access/export assembly with Customer Data Operations artifact ownership and crash recovery.

Repository step 11 — owner-specific deletion, anonymization and supported crypto-shred execution — is the only next implementation packet. Access/export assembly is accepted and must not be described as incomplete.

## 5. Phase 8A packet accounting

Completed:

- 8A.1–8A.6 — customer references, Party, Account, Contact Point, Party Relationship, Customer 360, Consent and reversible Identity Resolution;
- 8A.7 — Customer Import;
- 8A.8 — Customer Export;
- 8A.9 — Customer Data Quality;
- 8A.10 — Governed Customer Enrichment and Provenance.

In progress:

- 8A.11 / #126 — Customer Privacy. Discovery, immutable snapshots, deterministic planning, permission-aware reads, approval, restriction placement/final enforcement, legal-hold placement, mandatory-retention adjudication, durable owner execution/outcomes and governed access/export assembly are accepted. Remaining product work includes restriction and legal-hold release/read lifecycle where required, deletion/anonymization/supported crypto-shred, Party tombstone/no-orphan and convergence, Customer Privacy worker lifecycle, frontend/accessibility/browser proof and production operations evidence.

Workspace packages remain 113. Current product-complete expert modules remain **0**.

"""
replace_between(
    "docs/MODULE_CATALOG.md",
    "## 4. Customer Privacy boundary\n",
    "## 6. Customer-master ownership baseline",
    module_section,
)
replace_exact(
    "docs/MODULE_CATALOG.md",
    "| `crm.customer-privacy` | Privacy case and owner-orchestration coordinator | **Expert expansion** | Case lifecycle, approval, permission-aware get/list/plan/outcome reads, trusted-internal exact-nine discovery/immutable snapshots and deterministic planning, public deny-only restriction placement and first protected-owner enforcement | Restriction release/reads, holds/retention, owner execution, export/deletion/convergence and workers |",
    "| `crm.customer-privacy` | Privacy case and owner-orchestration coordinator | **Expert expansion** | Case lifecycle, approval, permission-aware get/list/plan/outcome reads, trusted-internal exact-nine discovery/immutable snapshots and deterministic planning, public deny-only restriction/legal-hold placement, retention adjudication, durable owner execution/outcomes and governed access/export assembly | Restriction/legal-hold release and reads where required, owner-specific deletion/anonymization/crypto-shred, tombstone/convergence, workers, frontend and operations proof |",
)

# Permanent tests.
replace_exact(
    "tests/test_repository_navigation.py",
    'self.assertEqual(packet["packet_id"], "repository-step-10-evidence-sync")',
    'self.assertEqual(packet["packet_id"], "architecture-plan-stage-accountability")',
)
replace_exact(
    "tests/test_repository_navigation.py",
    '"19232f6f3e2ae87aabeb080257c1aac5477a6616",',
    '"dad639c7d269bc802d053f1d99cf0fbf466ce4fb",',
    expected=2,
)
replace_exact(
    "tests/test_repository_navigation.py",
    '                "docs/IMPLEMENTATION_ROADMAP.md",\n                "docs/PHASE8_DELIVERY_PLAN.md",',
    '                "docs/IMPLEMENTATION_ROADMAP.md",\n                "docs/MODULE_CATALOG.md",\n                "docs/PHASE8_DELIVERY_PLAN.md",',
)
replace_exact(
    "tests/test_repository_navigation.py",
    '            "repository step 11 is the only next implementation packet",\n            packet["acceptance"],\n        )',
    '            "repository step 11 remains the only next implementation packet",\n            packet["acceptance"],\n        )\n        self.assertIn(\n            "Stage D has an explicit bounded completion packet before transitional consolidation",\n            packet["acceptance"],\n        )\n        self.assertIn(\n            "repository step 20 is a measurement checkpoint rather than an automatic 10/10 declaration",\n            packet["acceptance"],\n        )',
)

replace_exact(
    "tests/test_architecture_documentation_consistency.py",
    '            "11. owner-specific deletion, anonymization and supported crypto-shred execution — **Next**;",\n        )',
    '            "11. owner-specific deletion, anonymization and supported crypto-shred execution — **Next**;",\n            "12. complete first-party contribution aggregation for all currently active owners without behavior changes;",\n            "13. first measured behavior-neutral transitional domain-cluster consolidation;",\n            "16. deterministic local lifecycle commands: `doctor`, `bootstrap`, `dev-up`, `dev-reset`, `seed-demo` and `smoke`;",\n            "20. Phase 8A architecture remeasurement, remaining-gate review and publication of the measured Phase 8B extension baseline — **not a final 10/10 declaration**;",\n            "21. first Phase 8B expert-domain wave proving bounded extension cost;",\n            "22. second contrasting expert-domain wave proving bounded extension cost as module count grows;",\n            "23. final architecture 10/10 closure review only when every section 12 criterion is mechanically proven.",\n        )',
)
replace_exact(
    "tests/test_architecture_documentation_consistency.py",
    '            "11. repository step 11 — owner-specific deletion, anonymization and supported crypto-shred execution — **next**;",\n            self.phase8,\n        )',
    '            "11. repository step 11 — owner-specific deletion, anonymization and supported crypto-shred execution — **next**;",\n            self.phase8,\n        )\n        self.assertIn(\n            "12. repository step 12 — complete first-party contribution aggregation for all currently active owners without behavior changes;",\n            self.phase8,\n        )\n        self.assertIn(\n            "20. repository step 20 — Phase 8A architecture remeasurement and remaining-gate review, not a final 10/10 declaration;",\n            self.phase8,\n        )',
)
replace_exact(
    "tests/test_architecture_documentation_consistency.py",
    'self.assertEqual(self.packet["packet_id"], "repository-step-10-evidence-sync")',
    'self.assertEqual(self.packet["packet_id"], "architecture-plan-stage-accountability")',
)
replace_exact(
    "tests/test_architecture_documentation_consistency.py",
    'self.assertEqual(self.packet["baseline"]["sha"], "19232f6f3e2ae87aabeb080257c1aac5477a6616")',
    'self.assertEqual(self.packet["baseline"]["sha"], "dad639c7d269bc802d053f1d99cf0fbf466ce4fb")',
)
replace_exact(
    "tests/test_architecture_documentation_consistency.py",
    '            "docs/IMPLEMENTATION_ROADMAP.md",\n            "docs/PHASE8_DELIVERY_PLAN.md",',
    '            "docs/IMPLEMENTATION_ROADMAP.md",\n            "docs/MODULE_CATALOG.md",\n            "docs/PHASE8_DELIVERY_PLAN.md",',
)
replace_exact(
    "tests/test_architecture_documentation_consistency.py",
    '            "repository step 11 is the only next implementation packet",\n            self.packet["acceptance"],\n        )',
    '            "repository step 11 remains the only next implementation packet",\n            self.packet["acceptance"],\n        )\n        self.assertIn(\n            "Stage D has an explicit bounded completion packet before transitional consolidation",\n            self.packet["acceptance"],\n        )\n        self.assertIn(\n            "repository step 20 is a measurement checkpoint rather than an automatic 10/10 declaration",\n            self.packet["acceptance"],\n        )',
)
replace_exact(
    "tests/test_architecture_documentation_consistency.py",
    '        self.assertIn("repository-step-10-evidence-sync", self.active_packet)',
    '        self.assertIn("architecture-plan-stage-accountability", self.active_packet)',
)
replace_exact(
    "tests/test_architecture_documentation_consistency.py",
    '            "Single repository execution order",\n        )',
    '            "Single repository execution order",\n            "Stage-to-step accountability",\n            "Score-recovery accountability",\n            "repository step 12 is the bounded completion step",\n            "not an absolute ban",\n            "two contrasting later expert-domain waves",\n            "step 23",\n        )',
)
new_test = '''\n    def test_stage_accountability_and_live_catalog_are_current(self) -> None:\n        for stage in (\n            "A — documentation and policy baseline",\n            "B — dependency, crate and exception governance",\n            "C — golden owner package and persistence model",\n            "D — contribution aggregation",\n            "E — affected-scope CI",\n            "F — generic conformance and contract lifecycle",\n            "G — transitional consolidation",\n            "H — reproducible environment and navigation",\n            "I — frontend and operations parity",\n        ):\n            self.assertIn(stage, self.plan)\n\n        self.assertIn("repository step 12", self.plan.lower())\n        self.assertIn("repository step 20 is a phase 8a architecture measurement checkpoint", self.plan.lower())\n        self.assertIn("repository step 23", self.plan.lower())\n        self.assertIn("seven mutations, four permission-aware public queries and zero Customer Privacy workers through PR #241", self.catalog)\n        self.assertIn("customer_privacy.access_export.request@1.0.0", self.catalog)\n        self.assertIn("Repository step 11", self.catalog)\n        self.assertNotIn("Repository step 5 — `repo.py explain`, `repo.py packet-check`, generated active packet and repository map — is the next repository packet", self.catalog)\n\n'''
replace_exact(
    "tests/test_architecture_documentation_consistency.py",
    "    def test_repository_map_matches_authoritative_inventory(self) -> None:\n",
    new_test + "    def test_repository_map_matches_authoritative_inventory(self) -> None:\n",
)

# Validate the declared packet paths and emit generated navigation.
packet = json.loads((ROOT / "repository-packet.json").read_text(encoding="utf-8"))
expected_paths = {
    "docs/ACTIVE_PACKET.md",
    "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md",
    "docs/IMPLEMENTATION_ROADMAP.md",
    "docs/MODULE_CATALOG.md",
    "docs/PHASE8_DELIVERY_PLAN.md",
    "docs/PROJECT_STATUS.md",
    "repository-packet.json",
    "tests/test_architecture_documentation_consistency.py",
    "tests/test_repository_navigation.py",
}
if set(packet["allowed_paths"]) != expected_paths:
    raise SystemExit("repository packet allowed_paths do not match the permanent diff")
