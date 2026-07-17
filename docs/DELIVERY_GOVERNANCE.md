# Ultimate CRM — Delivery Governance

Status: **Normative delivery-control policy**

This document defines how repository status, roadmap sequencing, issues, pull requests and exact-SHA acceptance evidence stay synchronized. It exists to prevent stale parallel plans, ambiguous active work and completion claims that are not backed by merged code and reproducible evidence.

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

A lower-level source must not contradict a higher-level source. A pull request may be ahead of merged documentation while work is in progress, but it must not cause merged `main` documentation to claim unmerged functionality as complete.

## 2. State model

Every delivery packet uses exactly one of these states:

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

For the current customer-master lane the authoritative sequence is:

1. **8A.7 / #120 / PR #121** — Customer Import Jobs and Resumable Execution — **Complete**.
2. **8A.8 / #123 / PR #130** — Customer Export Jobs, Artifacts and Reconciliation Evidence — **Complete**.
3. **8A.9 / #124 / PR #132** — Customer Data Quality Rules, Completeness and Stewardship — **Complete**.
4. **Architecture integrity / #134 / PR #135** — native module composition and lifecycle gating — **Complete**.
5. **8A.10 / #125** — Governed Customer Enrichment and Provenance — **Ready** and next in the lane.
6. **8A.11 / #126** — Customer Privacy Lifecycle, Restriction, Deletion and Legal Hold — **Planned**.
7. **8B / #29** — Product Catalog, Pricing, CPQ and Quote-to-Revenue.

Parallel work is allowed only when ownership boundaries and dependencies are explicit and the work cannot invalidate the exact candidate of another packet.

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

A packet moves to **In progress** when the implementation branch or draft PR exists.

## 5. Exact-SHA gate discipline

A packet may move to **Gate review** only when:

- the implementation boundary is complete;
- documentation for that packet is synchronized on the candidate branch;
- all applicable checks have passed on one unchanged candidate SHA;
- every source-changing or documentation-changing commit after that evidence invalidates the evidence and requires a new exact-head gate;
- source-changing automation has reached a stable head before review handoff;
- native composition readiness and production-route parity pass when module/runtime scope is affected;
- no unresolved blocking review thread or known gate defect remains.

A packet becomes **Complete** only after merge to `main` and synchronization of the merged state.

## 6. Documentation synchronization contract

Whenever implementation state changes, update the affected sources in the same delivery packet where practical:

- `IMPLEMENTATION_ROADMAP.md` — phase and packet sequence;
- `PROJECT_STATUS.md` — current human-readable state;
- `PHASE8_DELIVERY_PLAN.md` — detailed Phase 8 packet state and next dependency;
- `MODULE_CATALOG.md` — module readiness/count only when the merged implementation justifies it;
- parent and packet GitHub issues;
- pull request body with actual delivered scope and exact validation state.

`README.md` remains stable orientation and must not become a second roadmap.

## 7. PR hygiene

- One PR must represent one natural architecture/delivery packet.
- A superseded PR is closed promptly and linked to its replacement.
- Draft PR bodies must describe actual current implementation state, not only the initial skeleton.
- A PR must not claim production completeness while required contracts, runtime composition, persistence or process acceptance remain absent.
- Stacked PRs must be retargeted/reverified after prerequisite merges.
- Merge remains an explicit action after gate success.

## 8. Issue hygiene

- Parent issues define program outcomes; child issues define reviewable packets.
- Every active packet has explicit dependencies and acceptance gates.
- Ambiguous phrases such as “later work” are replaced by named issues when the sequence is known.
- Closed/superseded paths remain historical evidence but are not shown as active execution.

## 9. Completion claims

The following claims are distinct and must not be conflated:

- **platform foundation complete**;
- **a module has a production vertical slice**;
- **a module is product complete**;
- **a capability family is production complete**;
- **the universal CRM product is complete**.

`CRM_CAPABILITY_COVERAGE.md` is the guardrail for product completeness. A crate, schema, manifest or isolated backend path is not sufficient by itself.

## 10. Current control baseline

As of 2026-07-17:

- Phases 0.1–7 are complete and Phase 8A is active.
- Phase 8A.6 is complete through PR #119 / merge `d5cb4502ad0c49158e0789d8749dc09160da7895`.
- Phase 8A.7 is complete through PR #121 / merge `5f60f24d6d3a3bb46720658f4e98d4a7ebb15637`.
- Phase 8A.8 is complete through PR #130 / merge `0e7f9889362533446cc65d95dcf7969a60086a57`.
- Phase 8A.9 is complete through PR #132 / merge `8a1664309be9dc0c5e3bf9014cf248b1c3680035`.
- Native module composition and lifecycle gating are complete through issue #134 / PR #135 / merge `023fa5ef1d510d5bcc32222c739e6d58e5696fb8`.
- Phase 8A.10 / #125 is **Ready** and is the next customer-master production packet from the accepted architecture baseline.
- Phase 8A.11 / #126 remains planned after #125, followed by Phase 8A closure and Phase 8B / #29.
- PR #118 remains superseded by merged PR #119 and is not an active delivery path.

This baseline must be updated whenever the active packet or merged completion state changes.
