# Ultimate CRM — Delivery Governance

Status: **Normative delivery-control policy**

This document defines how repository status, roadmap sequencing, issues, pull requests, permanent gates and exact-SHA acceptance evidence stay synchronized. It prevents stale parallel plans, ambiguous active work, governance growth without a failure model and completion claims not backed by merged code and reproducible evidence.

## 1. Source-of-truth hierarchy

Use the following order when determining project state:

1. `SYSTEM_INVARIANTS.md` — absolute architecture and conformance rules.
2. Accepted ADRs — binding architecture decisions, including ADR-031 and ADR-032.
3. `ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md` — the single repository-step order and architecture closure criteria.
4. `IMPLEMENTATION_ROADMAP.md` — normative product phase map and dependency order.
5. `PHASE8_DELIVERY_PLAN.md` — detailed packet sequence for the active expert-domain program.
6. `CRM_CAPABILITY_COVERAGE.md` — completeness guardrail for the target CRM product.
7. `MODULE_CATALOG.md` — business-module ownership and readiness accounting.
8. `PROJECT_STATUS.md` — concise current snapshot for humans.
9. `repository-packet.json` and generated navigation — the active bounded repository packet.
10. GitHub parent/packet issues and pull requests — executable scope and accepted evidence.

A lower-level source must not contradict a higher-level source. A pull request may be ahead of merged documentation while work is in progress, but merged `main` documentation must never claim unmerged functionality as complete.

Machine-readable production inventory and route-classification contracts are authoritative for exact public, worker and non-runtime coordinates. Human documentation must match them.

## 2. State model

Every delivery packet uses exactly one state:

- **Planned** — scope exists, but prerequisites are incomplete or implementation has not started.
- **Ready** — prerequisites are complete and the packet may begin.
- **In progress** — an implementation branch or draft PR exists.
- **Gate review** — implementation is complete and the exact candidate SHA is under final validation/review.
- **Complete** — merged to `main`; required gates passed on the accepted candidate and completion documentation is synchronized.
- **Blocked** — a named dependency, defect or decision prevents progress.
- **Superseded** — replaced by another issue/PR and no longer an active delivery path.

Only merged work may be described as **Complete** in `main` documentation.

## 3. One active packet per dependency lane

For the strict repository sequence, only the first unfinished step in the architecture plan may be the active implementation target.

A numbered repository step may use multiple small atomic PRs when needed, but:

- every PR has one coherent failure model and bounded outcome;
- measurement, remediation and evidence synchronization remain separate when combining them would hide before/after evidence;
- later repository steps do not begin until all implementation and required evidence packets for the earlier step are accepted;
- a packet must not expand merely to absorb adjacent planned work.

Parallel work is allowed only when ownership boundaries and dependencies are explicit and the work cannot invalidate another packet's exact candidate.

Current strict sequence status:

- Repository Steps 1–14 — **Complete**;
- Repository Step 15 — **next and not started**;
- Repository Steps 16–25 — blocked by the first earlier unfinished step.

## 4. Packet entry criteria

A packet may move from **Planned** to **Ready** only when:

- all named prerequisites are merged;
- owner-domain and cross-owner boundaries are explicit;
- module-owned route/validator/worker contributions and durable activation behavior are explicit;
- public contract/versioning and production-route classification implications are identified;
- persistence, migration and rollback implications are identified;
- authorization, data-class, audit, idempotency and approval requirements are identified;
- required process/browser/operational acceptance is defined;
- exact allowed and forbidden paths are declared;
- required permanent workflows are selected by the affected-scope policy;
- the issue or packet declaration is sufficient to reject out-of-scope shortcuts.

A packet moves to **In progress** when its implementation branch or draft PR exists.

## 5. Exact-SHA gate discipline

A packet may move to **Gate review** only when:

- the implementation boundary is complete;
- packet documentation and machine-readable inventory/classification contracts are synchronized on the candidate branch;
- all applicable checks pass on one unchanged meaningful candidate SHA;
- every source or documentation commit after that evidence invalidates the evidence and requires a new exact-head gate;
- source-changing automation has reached a stable head;
- native composition readiness and route parity pass when module/runtime scope is affected;
- no unresolved blocking review thread or known gate defect remains;
- the final changed-file set exactly matches the active packet.

A packet becomes **Complete** only after merge to `main` and synchronization of the merged state.

A post-merge integrity defect in inventory, classification or status documentation must be corrected before the dependent packet starts. Such correction does not retroactively invalidate the accepted implementation when the source checkpoint and merge remain unchanged, but the corrective branch itself requires applicable exact-head checks before merge.

## 6. Permanent-gate entry contract

A permanent workflow, job or repository gate is an engineering control with an ongoing execution and maintenance cost. It must not be created merely because another governance mechanism exists.

Every proposal for a new permanent gate must declare before acceptance:

- stable gate identifier;
- named owning team;
- concrete failure mode it prevents;
- why current gates do not already prevent that failure;
- authoritative inputs and affected scope;
- expected duration, runner cost, fan-out and expensive database/process/browser setup;
- false-positive controls;
- deterministic evidence emitted on success and failure;
- compensating checks and escalation path;
- review and retirement condition.

A gate without a concrete failure mode is not eligible to become permanent.

A gate that duplicates another gate must prove an independent failure mode or intentionally independent implementation path whose value exceeds its cost. Otherwise it must be simplified, merged or removed.

Temporary inspection, migration or diagnostic workflows remain temporary and must be removed before exact-head acceptance unless they independently satisfy this permanent-gate entry contract.

## 7. Permanent-gate change rules

A change to an existing permanent gate must identify:

- the failure mode affected by the change;
- whether coverage becomes broader, narrower or merely reorganized;
- execution-cost delta;
- overlap introduced or removed;
- whether the gate's retirement condition changed;
- any required update to affected-scope ownership or workflow filters.

A new governance abstraction must not be added solely to validate an existing governance abstraction. The packet must name the real product, architecture, security, persistence, compatibility or operational failure it prevents.

## 8. Step 22 permanent-gate value review

ADR-032 requires a complete value/cost review before Repository Step 22 architecture remeasurement can be accepted.

Every permanent workflow, job and repository gate must appear in one machine-readable ledger and human-readable report containing:

- concrete prevented failure mode;
- defects actually detected, or a specific preventive rationale when no historical defect exists;
- scope and authoritative inputs;
- overlap and duplication analysis;
- duration, runner/fan-out and expensive environment cost;
- false-positive and maintenance history where measurable;
- named owner;
- `retain`, `simplify`, `merge` or `remove` decision;
- independent value for any apparently duplicate retained gate;
- retirement or re-review condition;
- compensating checks for simplification, merge or removal.

All immediately safe simplifications, merges and removals must be completed before Step 22 closes. A deferred action requires a named owner, exact rationale and deadline before Step 25. Known safe simplification cannot be deferred merely to preserve the current workflow or gate count.

Step 22 cannot close with an unresolved permanent-gate value decision.

## 9. Runtime fan-in decision governance

The current `crm-application-runtime` non-growth budget prevents regression but does not prove that its broad direct dependency surface is irreducible.

ADR-032 requires Step 22 to classify every internal direct dependency as `removed`, `platform-generic`, `owner-specific-unavoidable` or `test-only`.

Every safely removable owner-specific dependency must be removed. Every retained owner-specific dependency must provide unavoidable-boundary evidence, a named owner and a removal/review condition. Mere non-growth is insufficient.

Ordinary existing-owner capability changes and the Step 23–24 expert-domain waves must not modify `crm-application-runtime/Cargo.toml` or owner-specific process-composition source. Contrary evidence reopens the Step 22 decision and blocks Step 25.

## 10. Documentation synchronization contract

Whenever implementation or binding plan state changes, update the affected sources in the same delivery packet where practical:

- `ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md` — repository order, architecture metrics and closure criteria;
- `IMPLEMENTATION_ROADMAP.md` — product phase and packet sequence;
- `PROJECT_STATUS.md` — current human-readable state;
- `PHASE8_DELIVERY_PLAN.md` — detailed Phase 8 packet state and next dependency;
- `MODULE_CATALOG.md` — module readiness/count only when merged product evidence justifies it;
- accepted ADRs and packet architecture/guardrail documents;
- machine-readable production promotion and route classifications;
- parent and packet GitHub issues;
- pull request body with actual delivered scope and exact validation state.

`README.md` remains stable orientation and must not become a second roadmap.

## 11. Production inventory integrity

For every manifest-bound capability coordinate exactly one classification applies:

- public runtime mutation/query;
- activation-gated worker runtime;
- individually reasoned non-runtime contract route.

A coordinate implemented and registered in a production worker must not remain classified non-runtime. A worker-only coordinate must not be counted as public ingress. Promotion contracts must distinguish completed promotion history from future work and match route classifications exactly.

Parity tests must fail when:

- classifications overlap;
- a completed module coordinate remains non-runtime;
- a worker coordinate is omitted from worker runtime inventory;
- public runtime inventory differs from compiled route definitions;
- promotion history contradicts current runtime state.

## 12. PR and issue hygiene

- One PR represents one natural architecture/delivery packet.
- A superseded PR is closed promptly and linked to its replacement.
- Draft PR bodies describe actual current implementation state, not only the initial skeleton.
- A PR must not claim production completeness while required contracts, runtime composition, persistence or process acceptance remain absent.
- Stacked PRs are retargeted and reverified after prerequisite merges.
- Merge remains an explicit action after gate success and must use the accepted expected head when supported.
- Parent issues define program outcomes; child issues define reviewable packets.
- Closed issue bodies or final comments record accepted source SHA, applicable workflow count and merge commit.

## 13. Separate architecture and product ledgers

The following claims are distinct:

- architecture/change-economics criteria are satisfied;
- platform foundation is complete;
- a module has a production vertical/integration slice;
- a module is product complete;
- a capability family is production complete;
- the universal CRM product is complete.

Architecture metrics and governance gates cannot raise product readiness. `CRM_CAPABILITY_COVERAGE.md` and `MODULE_CATALOG.md` guard product completeness. A crate, schema, manifest, architecture score or isolated backend path is insufficient.

## 14. Current control baseline

As of 2026-08-02:

- Phases 0.1–7 are complete and Phase 8A is active.
- Phase 8A.1–8A.10 are complete.
- Phase 8A.11 / issue #126 is **In progress**.
- all nine Customer Privacy owner-scope implementations are accepted;
- Customer Privacy public inventory remains seven mutations, four permission-aware queries and zero workers;
- Repository Steps 1–14 are complete;
- Repository Step 15 is next and not started;
- current workspace packages are 112, internal dependency edges 835, maximum dependency depth 18, conservative public Rust items 5,377, dependency declarations 270 and suppression occurrences 91;
- architecture 10/10 is unclaimed;
- current product-complete expert modules are zero;
- ADR-032 binds Step 22 to runtime fan-in and permanent-gate value decisions in addition to remeasurement.

This baseline must be updated whenever the active packet or merged completion state changes.
