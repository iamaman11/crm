# Ultimate CRM — Phase 8 Delivery Plan

Status: **Normative Phase 8 delivery sequence**  
Parent roadmap: `IMPLEMENTATION_ROADMAP.md`  
Architecture order: `ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md` section 2.4  
Current state: `PROJECT_STATUS.md`

## 1. Phase 8 objective

Phase 8 delivers expert CRM domains and product-quality cross-domain journeys on top of the governed platform completed in Phases 0.1–7.

Phase 8 is split into:

- **Phase 8A** — canonical customer master, identity, consent, governed customer-data operations, enrichment and privacy;
- **Phase 8B** — Product Catalog, Pricing, CPQ and quote-to-revenue, followed by later expert domains.

Phase 8A remains **In progress**. It must close before Phase 8B implementation begins. Repository Step 22 then remeasures architecture; it does not automatically declare 10/10.

## 2. Delivery rules

1. One authoritative owner exists for every mutable aggregate.
2. Cross-owner interaction uses versioned contracts and governed ports, never direct storage access.
3. Runtime claims require real composition, persistence, authorization, audit, rollback and process evidence.
4. Tenant activation, live authorization, FORCE RLS and cross-tenant concealment are mandatory.
5. Public coordinates, internal coordinates, routes, workers and lifecycle state are counted separately.
6. Only merged `main` work changes readiness.
7. Repository packets follow the single order in the architecture plan; later steps cannot start early.
8. Product behavior, physical package consolidation and evidence synchronization remain separate when packet discipline requires it.
9. Architecture reductions do not advance module product readiness without separate product/UX/operations evidence.

## 3. Phase 8A delivery map

| Packet | Result | State |
|---|---|---|
| 8A.1 | Customer references and canonical Party foundation | **Complete** |
| 8A.2 | Customer Accounts and Party associations | **Complete** |
| 8A.3 | Contact Points and verification | **Complete** |
| 8A.4 | Party Relationships and hierarchy foundation | **Complete** |
| 8A.5 | Customer 360 read composition | **Complete** |
| 8A.6 | Consent and reversible Identity Resolution | **Complete** |
| 8A.7 | Governed Customer Import | **Complete** |
| 8A.8 | Governed Customer Export | **Complete** |
| 8A.9 | Customer Data Quality | **Complete** |
| 8A.10 | Governed Customer Enrichment and Provenance | **Complete** |
| 8A.11 / #126 | Customer Privacy and Phase 8A closure | **In progress** |

## 4. Accepted Customer Privacy foundation

Nine-owner set complete. All nine authoritative privacy-owner scope implementations are accepted:

1. Parties;
2. Consents;
3. Customer Accounts;
4. Contact Points;
5. Party Relationships;
6. Identity Resolution;
7. Customer Data Operations;
8. Data Quality;
9. Customer Enrichment.

Customer Data Operations owner-scope evidence is accepted through PR #188 / accepted source `07f34786e82fdfa78d263790e9f50541529006f8` / squash merge `089be72fa3010b4aa15aff7f9ea55fd86290f8fc` / 26 of 26 permanent workflows.

Data Quality owner-scope evidence is accepted through PR #190 / accepted source `dcfe8faebc7462b888f8fc1721cb379a40fea88a` / squash merge `deac197c97cddc15bb9916092ca87f6e767ce1de` / 27 of 27 permanent workflows.

Customer Enrichment owner-scope evidence is accepted through PR #192 / accepted source `e90e36027de18a07be68e43327ea732810ff332a` / squash merge `e41cbab0cd30819fcbe2e3c5f2c7415fc6de3e8c` / 28 of 28 permanent workflows.

Scope discovery and immutable snapshot execution is Accepted through PR #206 / accepted source `086b17a95058eee285fcb67a903bd21d9263d357` / squash merge `95818fd3aeb54a9593a45642583f0b7224d5ecfe`.

Accepted Customer Privacy evidence includes:

- scope discovery and immutable snapshot execution;
- deterministic action planning and immutable lineage;
- permission-aware plan and owner-outcome reads;
- public approval;
- final transaction-scoped customer-subject policy;
- public restriction placement and immediate protected-owner denial;
- public legal-hold placement and mandatory-retention precedence;
- durable replay-safe owner execution, checkpoints and outcomes;
- governed access/export assembly with Customer Data Operations ownership;
- authoritative exact-nine owner-specific anonymization and supported deletion;
- complete first-party production contribution aggregation;
- blocking architecture, dependency, suppression and affected-scope governance.

Latest accepted public Customer Privacy inventory remains:

- **7 public mutations**;
- **4 permission-aware public queries**;
- **0 Customer Privacy workers**.

Trusted-internal planning, retention evaluation, replay-safe owner execution, access/export assembly and exact-nine action execution remain non-public.

## 5. Accepted Repository Step 14 architecture result

Repository Step 14 and architecture Stage G are complete through PR #259 / accepted source `8aa0b33c6609e74f98363071c6e7c44ec59fc098` / squash merge `2b0b558077c444d44691371c8a2bcca2c14ae426` / 36 of 36 applicable permanent workflows on one unchanged meaningful user-authored exact head.

The packet is behavior-neutral. It removes `crm-customer-accounts-capability-composition`, moves the owner production contribution into `crm-customer-accounts-query-adapter`, preserves mutation planning and exact first-party inventory, and changes no public contract, route, schema, migration, persistence or worker behavior.

Exact measured result:

- workspace packages: **113 → 112**;
- internal dependency edges: **841 → 835**;
- conservative public Rust items: **5,379 → 5,377**;
- maximum dependency depth: **18 → 18**;
- dependency declarations: **270 → 270**;
- suppression occurrences: **91 → 91**.

Approval, Discovery and Planning permanent workflows were synchronized to the accepted 112-package workspace while retaining their existing clean-database, E2E, frozen-non-effect, rollback, reapply and repeated-acceptance checks.

This result completes an architecture stage only. Phase 8A.11 remains in progress, Customer Privacy remains incomplete and Current product-complete expert modules: **0**.

## 6. Remaining Phase 8A.11 product work

The remaining work is sequenced, not parallel:

### Repository Step 15 — next, not started

- authoritative Party tombstone semantics;
- no-orphan proof across owned and referencing records;
- deterministic projection, search and cache convergence;
- tenant/RLS/authorization/audit/idempotency preservation;
- rollback, replay and crash-window acceptance.

### Repository Step 16

- reusable generic worker conformance;
- representative real-worker adoption without owner-specific logic in generic algorithms.

### Repository Step 17

- contract compatibility and published-version gates;
- deprecation telemetry;
- consumer migration evidence;
- governed retirement enforcement.

### Repository Step 18

- deterministic `doctor`, `bootstrap`, `dev-up`, `dev-reset`, `seed-demo` and `smoke` commands on a clean environment.

### Repository Step 19

- a real Customer Privacy worker lifecycle;
- complete activation, authorization, lease/replay/crash and process/end-to-end evidence;
- disable/uninstall fail-closed semantics.

### Repository Step 20

- frontend ownership and bounded dependencies;
- critical component/browser/accessibility journeys;
- production restore, SLO, observability, performance, security and supply-chain evidence.

### Repository Step 21

- final Phase 8A product and operations closure against every accepted invariant.

No Step 16 or later implementation may begin before Step 15 is accepted and synchronized.

## 7. Phase 8A closure criteria

Phase 8A is complete only when:

- Customer Privacy required lifecycle behavior is complete;
- Party tombstone/no-orphan and all projection/search/cache convergence are proven;
- public and internal route inventories are exact and intentional;
- a real worker lifecycle is accepted;
- disable/uninstall and recovery fail closed;
- critical frontend journeys pass browser and accessibility acceptance;
- production restore, SLO, observability, performance, security and supply-chain gates are executable;
- clean apply, rollback, reapply and repeated acceptance succeed;
- roadmap, status, catalog, issues and generated packet agree;
- product-complete readiness is justified by product evidence, not by architecture structure alone.

## 8. Phase 8B entry

Phase 8B / issue #29 remains planned. Entry requires:

1. completed Phase 8A through Repository Step 21;
2. Repository Step 22 architecture remeasurement with no hidden regression;
3. a bounded Step 23 first expert-domain wave.

Planned independent owner domains include Product Catalog, Pricing, Promotions, CPQ, Quotes, Orders, Contracts, Subscriptions, Entitlements, Usage and governed Billing/ERP/payment/tax/fulfillment integration.

Step 24 must add a contrasting expert-domain wave and prove that extension cost remains bounded as module count grows.

## 9. Binding repository continuation

Repository Steps 1–14 are complete.

15. Repository Step 15 — Party tombstone, no-orphan proof and projection/search/cache convergence — **next, not started**;
16. Repository Step 16 — reusable worker conformance;
17. Repository Step 17 — contract lifecycle enforcement;
18. Repository Step 18 — deterministic local lifecycle;
19. Repository Step 19 — Customer Privacy worker and full E2E;
20. Repository Step 20 — frontend and operations evidence;
21. Repository Step 21 — Phase 8A closure;
22. Repository Step 22 — architecture remeasurement, not final 10/10;
23. Repository Step 23 — first contrasting Phase 8B expert-domain wave;
24. Repository Step 24 — second contrasting Phase 8B expert-domain wave;
25. Repository Step 25 — final architecture 10/10 review only if every normative criterion is mechanically proven.

Architecture 10/10 remains unclaimed. Issue #194 and issue #126 remain open. Current product-complete expert modules: **0**.
