# ADR-033: closed-pilot validation within Repository Step 22 before broad expert expansion

Status: **Proposed until merged**  
Date: 2026-11-06  
Decision: #310 (G0); architecture #194  
Supersedes: **only the scheduling assumption** of ADR-032 / repository Step 22 that all fan-in and permanent-gate decisions must finish before any additional product-user validation; **not** any mandatory architecture, security, source-of-truth or closure requirement.

## Problem

At `main@fab78b95afb66344af2dccbff87121cd24ba3cd5`, the first-party backend has extensive governed modules, while the production browser product has no usable identity session or everyday customer/deal/task workflow and no reproducible closed-pilot deployment. Step 22H reduced the runtime direct internal dependency surface to **53 total, 52 production, 1 test-only**, but **36 accepted inventory dependencies and permanent-gate value dispositions remain unresolved**. Further isolated architectural reductions without a measured working product risk optimizing a platform that operators cannot yet use.

## Decision and exact sequence

Preserve **one active bounded packet** and the precedence of the current architecture plan. After merged Step 22H, the next bounded repository work is **Step 22P / G0**: this governance/packet transition proposal. Once accepted, continue **sequentially**, never in parallel on changing repository heads, through bounded Step 22P work packets:

1. **G1**: production identity and server-authoritative tenant/actor/permission integration (#311);
2. **G2**: complete useful customer/contact/deal/task browser workflow through established owners (#312);
3. **G3**: minimal installed HTTPS app, durable database, safe migration/backup/restore (#313);
4. **G4/DX**: immutable release-equivalent CI/security checks and measured extension cost (#314, #315). Apply relevant tests in every preceding packet; G4 closure collects final evidence;
5. **G5**: independent installed closed-pilot acceptance and GO/NO-GO (#316).

Then resume **Step 22R**, finish every remaining ADR-032 fan-in and permanent-gate value/cost decision and architecture remeasurement **before Step 23 or any Phase 8B expansion**. Steps 23, 24 and 25 retain their existing order and full requirements. A closed-pilot GO does not mean Step 22 completion, architecture 10/10, Phase 8B or public production certification.

**This explicitly amends scheduling but does not falsely report Step 22 completed.** No product packet may be worked under a preceding packet's allowlist; each must declare its own baseline, allowed/forbidden paths and required CI and pass fail-closed `packet-check`. Only one active implementation packet at a time.

## Preservation of binding requirements

All ADR-032 Step 22 exit requirements survive unchanged: classify **every** internal direct dependency; remove safely removable owner-specific runtime fan-in; justify unavoidable boundaries; inventory and decide value/cost for **every** permanent workflow/job/gate; implement immediately safe simplifications; mechanically remeasure; close with **zero** unresolved classifications/dispositions. Preserve exact-SHA acceptance and no-gate-weakening requirements. No additional permanent gate for pilot work without a distinct failure model, owner, measured cost and retirement condition. Step 23–24 still measure extension cost against the Step 22R accepted baseline.

One authoritative owner per mutable domain, governed versioned APIs, tenant activation, live authorization, FORCE RLS, transactional idempotency/audit/outbox, immutable contracts and rebuildable projections are absolute. Pilot UI must never directly mutate PostgreSQL or bypass domain permissions.

## Why this order / cost

A genuine product-user workflow exposes the **actual** extension and operational bottlenecks (production auth, frontend ergonomics, release build, deployment, restore) and feeds actionable evidence to Step 22R rather than continuing architecture-only optimization without a real customer-facing result. It also protects the broader universal roadmap from being mistaken for the first useful deployable CRM.

Alternatives rejected:
- declare Step 22 complete based on 10 removed dependencies: wrong, 36 remain unresolved and gate ledger incomplete;
- skip Step 22 requirements permanently: prohibited;
- deploy dev-mode/browser bearer or bypass RLS: prohibited;
- parallel competing implementation packets: violates exact-SHA/packet governance;
- rebuild as microservices, generic CRUD or alternate business data source: unnecessary and increases cost.

## Checkpoints and consequences

G0 = merged ADR/order/roadmap/status/packet transition + exact-SHA CI. G1–G5 are not done by this ADR. Product pilot success is recorded in #310 and measured DX in #315; architecture debt/resolution remains on #194. Failed, missing or misleading verification is **NO-GO**. No release or real-customer production data is authorized merely by merging this ADR.
