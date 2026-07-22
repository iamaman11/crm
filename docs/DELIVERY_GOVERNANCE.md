# Ultimate CRM — Delivery Governance

Status: **Normative delivery-control policy**

This document defines how repository status, roadmap sequencing, issues, pull requests and exact-SHA acceptance evidence stay synchronized. It prevents stale parallel plans, ambiguous active work and completion claims not backed by merged code and reproducible evidence.

## 1. Source-of-truth hierarchy

Use the following order when determining project state:

1. `SYSTEM_INVARIANTS.md` — absolute architecture and conformance rules.
2. `ARCHITECTURE_READINESS.md` — accepted native-composition non-regression baseline.
3. `IMPLEMENTATION_ROADMAP.md` — normative phase map and dependency order.
4. `PHASE8_DELIVERY_PLAN.md` — detailed packet sequence for the active expert-domain program.
5. `CRM_CAPABILITY_COVERAGE.md` — completeness guardrail for the target CRM product.
6. `MODULE_CATALOG.md` — business-module ownership and readiness accounting.
7. `PROJECT_STATUS.md` — concise current snapshot for humans.
8. GitHub parent/packet issues — executable scope and acceptance for a delivery packet.
9. Pull requests — implementation state for one reviewable packet.

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

For a strict dependency chain, only one production packet is the active implementation target at a time. A later packet may have architecture notes or a stacked draft branch, but it must not be represented as the active merge candidate until its prerequisite packet is merged and it is rebased or retargeted onto the accepted baseline.

Current customer-master lane:

1. **8A.7 / #120 / PR #121** — Customer Import — **Complete**.
2. **8A.8 / #123 / PR #130** — Customer Export — **Complete**.
3. **8A.9 / #124 / PR #132** — Customer Data Quality — **Complete**.
4. **Architecture integrity / #134 / PR #135** — native composition/lifecycle gating — **Complete**.
5. **8A.10 / #125 / PR #137** — Governed Customer Enrichment and Provenance — **Complete**.
6. **8A.11 / #126** — Customer Privacy Lifecycle, Restriction, Deletion and Legal Hold — **In progress**; foundation plus `case.create`, `case.submit`, `case.subject.verify`, `case.get` and `case.cancel` are merged through PR #150.
7. **Next bounded 8A.11 slice** — select exactly one of `case.list`, `case.approve` or restriction placement after this post-merge synchronization.
8. **Phase 8A closure** — after merged privacy interaction proof.
9. **8B / #29** — Product Catalog, Pricing, CPQ and Quote-to-Revenue.

Parallel work is allowed only when ownership boundaries and dependencies are explicit and the work cannot invalidate another packet's exact candidate.

## 4. Packet entry criteria

A packet may move from **Planned** to **Ready** only when:

- all named prerequisite packets are merged;
- owner-domain and cross-owner boundaries are explicit;
- module-owned route/validator/worker contributions and durable activation behavior are explicit;
- public contract/versioning and production-route classification implications are identified;
- persistence, migration and rollback implications are identified;
- authorization, data-class, audit, idempotency and approval requirements are identified;
- required process/browser/operational acceptance is defined;
- the issue body is sufficient to reject out-of-scope implementation shortcuts.

A packet moves to **In progress** when its implementation branch or draft PR exists.

## 5. Exact-SHA gate discipline

A packet may move to **Gate review** only when:

- the implementation boundary is complete;
- packet documentation and machine-readable inventory/classification contracts are synchronized on the candidate branch;
- all applicable checks pass on one unchanged candidate SHA;
- every source or documentation commit after that evidence invalidates the evidence and requires a new exact-head gate;
- source-changing automation has reached a stable head;
- native composition readiness and route parity pass when module/runtime scope is affected;
- no unresolved blocking review thread or known gate defect remains.

A packet becomes **Complete** only after merge to `main` and synchronization of the merged state.

A post-merge integrity defect in inventory, classification or status documentation must be corrected before the dependent packet starts. Such correction does not retroactively invalidate the accepted implementation when the source checkpoint and merge remain unchanged, but the corrective branch itself requires applicable exact-head checks before merge.

## 6. Documentation synchronization contract

Whenever implementation state changes, update the affected sources in the same delivery packet where practical:

- `IMPLEMENTATION_ROADMAP.md` — phase and packet sequence;
- `PROJECT_STATUS.md` — current human-readable state;
- `PHASE8_DELIVERY_PLAN.md` — detailed Phase 8 packet state and next dependency;
- `MODULE_CATALOG.md` — module readiness/count only when merged implementation justifies it;
- packet architecture/guardrail/acceptance documents;
- machine-readable production promotion and route classifications;
- parent and packet GitHub issues;
- pull request body with actual delivered scope and exact validation state.

`README.md` remains stable orientation and must not become a second roadmap.

## 7. Production inventory integrity

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

## 8. PR hygiene

- One PR represents one natural architecture/delivery packet.
- A superseded PR is closed promptly and linked to its replacement.
- Draft PR bodies describe actual current implementation state, not only the initial skeleton.
- A PR must not claim production completeness while required contracts, runtime composition, persistence or process acceptance remain absent.
- Stacked PRs are retargeted/reverified after prerequisite merges.
- Merge remains an explicit action after gate success.

## 9. Issue hygiene

- Parent issues define program outcomes; child issues define reviewable packets.
- Every active packet has explicit dependencies and acceptance gates.
- Ambiguous “later work” is replaced by named issues when the sequence is known.
- Closed/superseded paths remain historical evidence but are not shown as active execution.
- Closed issue bodies and final comments record accepted source SHA and merge commit.

## 10. Completion claims

The following claims are distinct:

- platform foundation complete;
- a module has a production vertical/integration slice;
- a module is product complete;
- a capability family is production complete;
- the universal CRM product is complete.

`CRM_CAPABILITY_COVERAGE.md` guards product completeness. A crate, schema, manifest or isolated backend path is insufficient.

## 11. Current control baseline

As of 2026-07-22:

- Phases 0.1–7 are complete and Phase 8A is active.
- Phase 8A.7 is complete through PR #121 / merge `5f60f24d6d3a3bb46720658f4e98d4a7ebb15637`.
- Phase 8A.8 is complete through PR #130 / merge `0e7f9889362533446cc65d95dcf7969a60086a57`.
- Phase 8A.9 is complete through PR #132 / merge `8a1664309be9dc0c5e3bf9014cf248b1c3680035`.
- Native module composition/lifecycle integrity is complete through PR #135 / merge `023fa5ef1d510d5bcc32222c739e6d58e5696fb8`.
- Phase 8A.10 is complete through PR #137; accepted source `f92d101206886e3ceaf94d0e56e52580cec21093`; merge `150e44b95d9dbdc08c1792563de03ec73f34aed1`.
- Customer Enrichment runtime inventory is exactly 6 public mutations, 6 permission-aware queries and 5 activation-gated worker-only coordinates.
- Phase 8A.11 / #126 is **In progress**. PR #150 accepted source `be05e874b21ab33cb8b6a84fbcefc3c025aa88cb` and merge `2a4c34727e9d7bf8ed51b6411b7ab9c76c109671` established four runtime Customer Privacy mutations, one runtime query, eleven public non-runtime coordinates and zero Customer Privacy workers.
- The next Customer Privacy implementation coordinate is not selected until this corrective documentation branch merges and `case.list`, `case.approve` and restriction placement are compared.
- Phase 8B / #29 remains planned after Phase 8A closure.

This baseline must be updated whenever the active packet or merged completion state changes.
