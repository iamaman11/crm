# First Usable CRM — Pilot Execution and Developer Checkpoints

**Document class:** Non-normative navigation; release/deployment is **not** accepted. G0 is accepted only after ADR-033 and the exact-head governance packet are merged.  
**Date:** 2026-11-06  
**Inventory baseline:** `main@fab78b95afb66344af2dccbff87121cd24ba3cd5` (recheck immediately before code work).  
**Executable coordination board:** [#310](https://github.com/iamaman11/crm/issues/310)

## 1. Purpose, scope and authority

The repository has a modular Rust CRM backend, authoritative PostgreSQL state and a limited React product shell, but **no complete ordinary-user, release-equivalent production journey**. Deliver a **closed-pilot** CRM that actually serves operators: customers/contacts, deals and tasks with sign-in, durable saves, restart and recovery.

This page is a **short navigation/checkpoint proposal**, not a second roadmap. The canonical scope, status, owner/evidence and GO/NO-GO are on issue #310 and its linked children. Normative truth remains in `docs/SYSTEM_INVARIANTS.md`, accepted contracts/ADRs, `docs/APPLICATION_ARCHITECTURE.md`, `docs/IMPLEMENTATION_ROADMAP.md`, `docs/PROJECT_STATUS.md`, `docs/DEVELOPMENT_WORKFLOW.md` and existing issue #194. When conflicting, follow existing precedence and mark this proposal BLOCKED until G0 alignment is accepted.

**Active packet discipline:** merged Step22H is historical; the G0 planning PR declares a new narrowly scoped **Step 22P / G0** packet based on the actual Step22H merged `main` SHA. Each subsequent G1–G5 implementation must declare its **own** bounded packet after previous acceptance. Do not expand G0 to include runtime/product implementation and do not treat a draft ADR or a failed CI as accepted.

## 2. Smallest complete user result

A non-developer opens a deployed, HTTPS-protected CRM, signs in, creates an organization/person, adds a contact point, creates a deal and advances its stage, creates/completes a related task, searches/reopens the records, logs out, and sees the correct authorized data after browser and service restart.

Relevant existing owners: `crm.parties`, `crm.customer-accounts`, `crm.contact-points`, `crm.sales`, `crm.activities`; search/customer views remain read models over governed authoritative state. This is not a claim that the full Universal CRM, Phase 8B/C or Phase 11 are complete.

Out of this first pilot: AI, marketplace, general automation designer, full CPQ/catalog, omnichannel, marketing, complex forecasting and enterprise certification. Keep them in their established roadmap/issues, **not as prerequisites**.

## 3. Non-negotiable module/DX budget

A change to an **existing** owner normally touches only its current contract/application/adapter/production contribution and focused tests:

- **0 new Rust crates** for an ordinary feature;
- **0 edits to generic router/worker algorithms** and **0 owner-specific switches in `crm-application-runtime`**;
- **0 direct cross-owner storage or internal-type dependencies**, and **0 unrelated owner changes**;
- business rules/authorization/idempotency/audit/RLS live on the governed backend, never duplicated in UI or services;
- typed versioned public capability/query contracts; UI uses generated typed clients, not raw storage or alternate CRUD paths;
- one authoritative owner per mutable domain, optional link module only for real cross-owner coordination, rebuildable search/projections;
- one coherent PR per real outcome with exact SHA/affected scope; new crate/service/adapter only for an explicitly justified independent boundary;
- one short developer path and one deploy path; avoid multiple competing checklists, config stacks or duplicated docs.

An exception requires evidence in the issue/PR: why the existing owner boundary cannot implement the capability, alternatives, extra dependencies, testing impact and migration/rollback plan.

## 4. Developer starting path

1. Read `AGENTS.md` → `docs/README.md` → `docs/SYSTEM_INVARIANTS.md` → current `docs/PROJECT_STATUS.md` → applicable issue and `docs/ACTIVE_PACKET.md`.
2. Resolve owner/coordinate with `python scripts/repo.py explain <module-or-capability@version>` and inspect `modules/<owner>/module.yaml`, relevant `proto/`, application and production contribution code.
3. Implement in the existing owner; add only necessary schema/contract changes and preserve live authorization, audit, outbox, RLS, idempotency and transaction correctness. Cross-owner linking uses capabilities/events/links.
4. Add/adjust generated typed frontend bindings. Browser pages assemble authorized information; they do not invent mutable aggregates or trust client-side permission hints.
5. Check local readiness using the existing `repo.py doctor/bootstrap/dev-up/seed-demo/smoke` chain **only for development**. These do not equal deployment/acceptance.
6. Run focused owner tests, `repo.py affected/check-affected/packet-check` against the **approved** packet baseline, applicable specialized workflows, and exact-SHA CI. Do not weaken a gate or patch source inside CI.
7. Attach measured changed-file/crate/runtime counts, selected CI, time-to-verify, positive/negative cases and evidence to the issue. Update normative state only for accepted merged behavior.

**Developer-experience gate:** a new contributor should trace a feature end-to-end within 15 minutes (target to measure); ordinary owner feature should add zero crates and avoid generic runtime changes. Track actual time, changed paths, dependency edges and CI cost in [#315](https://github.com/iamaman11/crm/issues/315); don't equate more crates or workflows with progress.

## 5. Checkpoints: evidence, not prose

| Gate | Responsible issue | Evidence to attach to #310 | Initial planning baseline (2026-11-06) |
|---|---|---|---|
| **G0 — decision & governance** | #310, #315, #194 | reviewed normative sequence / packet alignment, exact baseline and honest CI snapshot | **OPEN** |
| **G1 — production identity** | #311 | production-built login/logout/expiry/revocation, tenant/actor negative tests, no web secrets | **NO-GO** |
| **G2 — everyday CRM UX** | #312 | full create/edit/search/link/reload in immutable browser build + data correctness | **NO-GO** |
| **G3 — deployment & recovery** | #313 | fresh install over HTTPS, persistence, safe upgrade/rollback, restored independent backup | **NO-GO** |
| **G4 — release verification** | #314, #315 | unchanged build artifacts, all applicable checks, security/tenant/failure tests, extension-cost evidence | **NO-GO** |
| **G5 — installed pilot** | #316 | real operator trial, restart, backup restore, UX/performance notes, explicit signed GO/NO-GO | **NO-GO** |

The states above are **historical planning-baseline snapshots**, not a duplicate live dashboard. The only current gate state is in #310. States: `PLANNED → IMPLEMENTING → CI_GREEN → INSTALLED → ACCEPTED`, or `BLOCKED`. **CI_GREEN is never proof of installation or final acceptance.** Close each issue only when its own evidence is attached. Close #310 only on G5 **closed-pilot** GO.

## 6. Proposed work sequence

**First: G0**. Accept ADR-033's explicit **scheduling** amendment, with all ADR-032 Step 22 **closure** obligations intact. The G0 packet does not authorize product implementation; it just establishes the next governed order after Step22H. No rival roadmap.

**Next: sequential bounded packets, one active implementation at a time:** #311 (identity), #312 (product UX), #313 (deployment), #314–#315 (immutable release verification and measured extension cost), #316 (installed acceptance). Architecture Step22R then resumes outstanding ADR-032 obligations before Step23. Specification/review can be prepared early, but do not run overlapping implementation heads.

**Throughout:** #314 checks actual production-built artifacts and security/tenant/restore; implement fixes in committed source, not runner-time transformations. Keep architecture-wide fan-in/gate-value refactors tracked separately in #194 unless an actual pilot blocker is proven.

**Finally: #316** tests fresh install, multiuser/multitenant, workflow, restart, rollback/restore, error cases and non-developer usability before GO/NO-GO.

## 7. Known issues from source inspection — verify; don't overclaim

| Finding | Source | Impact |
|---|---|---|
| Production web session defaults unauthenticated; capability navigation DEV-only | `apps/web/src/developmentSession.ts`, `apps/web/src/App.tsx` | G1 blocker |
| Startup actor/bearer is not a production user login product | `crates/crm-application-runtime/src/config.rs`, `runtime.rs` | G1 blocker |
| Product routes mostly search/privacy/admin/demo; no complete everyday CRM screens | `apps/web/src/routes.ts` | G2 blocker |
| Local `smoke` validates API, not a released web application; Vite dev proxy only | `scripts/repo.py`, `apps/web/vite.config.ts` | G3/G4 blockers |
| Acceptance runner temporarily edits UI/spec and separate CI instruments runtime source | `scripts/run_customer_privacy_operations.sh`, `.github/workflows/customer-privacy-operations.yml` | G4 blocker |
| Architecture Step 22 incomplete; existing source-of-truth requires it before the next product wave | #194, `docs/IMPLEMENTATION_ROADMAP.md`, active packet | G0 governance blocker |

## 8. Evidence attached to each delivery PR

Always record: issue and gate; baseline and final **commit SHA**; precise ownership and public contracts; paths touched, crate/dependency delta and generic-runtime impact; exact built artifacts and SHA-256; migrations; privilege/tenant/idempotency/rollback evidence; focused tests and selected applicable CI conclusions; browser/user workflow; failure mode, recovery, operational acceptance; remaining explicit gaps. Do not claim a pass for an unexecuted check.

## 9. Existing work, not copied

- [#2](https://github.com/iamaman11/crm/issues/2): repository product epic.
- [#11](https://github.com/iamaman11/crm/issues/11): expert modules and product-quality UX.
- [#14](https://github.com/iamaman11/crm/issues/14): broader enterprise security/resilience/production proof.
- [#194](https://github.com/iamaman11/crm/issues/194): architecture/developer-experience Step 22–25 program.
- [#310](https://github.com/iamaman11/crm/issues/310): **single first-pilot execution plan**, with child #311–#316.

**Acceptance rule:** This document never overrides the governing ADR-033, architecture plan, roadmap or packet. Merge the G0 documentation only when strict packet-check, generated navigation and every required exact-head CI are green; after merge, issue #310 remains the live checkpoint board. No G1–G5 work is treated as accepted by this planning document.
