# Ultimate CRM — Implementation Roadmap

Status: **Normative delivery plan**

Parent epic: #2  
Governing rules: `SYSTEM_INVARIANTS.md`  
Delivery-control policy: `DELIVERY_GOVERNANCE.md`  
Current concise state: `PROJECT_STATUS.md`  
Detailed Phase 8 sequence: `PHASE8_DELIVERY_PLAN.md`  
Product portfolio and functional 10/10 sequence: `PRODUCT_DEVELOPMENT_10_OF_10_PLAN.md`  
Architecture/developer-experience program and repository order: `ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md` / issue #194  
Step 22 decision: `adr/ADR-032-step-22-runtime-fanin-and-permanent-gate-value.md`  
Accepted Rust boundary: `RUST_TOOLCHAIN_AND_LINT_BASELINE.md` / `rust-governance-policy.json`  
Functional completeness guardrail: `CRM_CAPABILITY_COVERAGE.md`  
Business-module accounting: `MODULE_CATALOG.md`

## 1. Purpose and delivery rules

This roadmap defines dependency order for a universal modular expert CRM platform. A phase or packet is complete only when implemented, merged and backed by unchanged exact-head evidence.

1. Preserve one authoritative owner for every mutable aggregate.
2. Enter state-changing behavior through exact versioned capabilities with typed audit evidence.
3. Never access another module's storage or internals directly.
4. Treat security, privacy, tenant isolation, rollback and operations as implementation requirements.
5. Require real composition, persistence and process evidence before runtime claims.
6. Invalidate old exact-SHA evidence after every source or documentation change.
7. Synchronize roadmap, phase plan, status, catalog, issues and pull-request evidence.
8. An ordinary capability added to an existing owner creates zero new crates by default.
9. Generic router and worker algorithms do not change merely to register one owner capability.
10. Feature behavior and physical crate consolidation remain separate packets.
11. Repository implementation is strictly sequential: only the first unfinished item in the architecture plan section 2.4 may start.
12. Only one implementation packet may be active; evidence synchronization closes the accepted packet before the next implementation begins.
13. Only merged `main` work may be represented as complete.
14. A new permanent gate requires a concrete failure mode, non-duplication rationale, expected cost, named owner and retirement/review condition before acceptance.
15. Repository Step 22 cannot close on non-growth alone; ADR-032 runtime fan-in and permanent-gate decisions are mandatory.
16. Product scope follows `PRODUCT_DEVELOPMENT_10_OF_10_PLAN.md`; a coverage bullet is not complete without owner/runtime/UX/accessibility/browser/operations evidence.
17. Workflow, trigger, robot, pipeline, Kanban, cadence, AI and low-code behavior must use governed capabilities and may not create an alternate mutation, authorization, privacy or audit path.

## 2. Product phase map

| Phase | Issue | Primary result | State |
|---|---:|---|---|
| 0.1–7 | #3–#10 | Governed platform, Sales/Activities proof, search, product shell and native composition | **Complete** |
| 8 | #11 | Expert modules and product-quality CRM experience | **In progress** |
| 8A | #28 | Canonical customer master, identity, consent and governed customer-data lifecycle | **In progress** |
| 8B | #29 | Product Catalog, Pricing, CPQ, Quotes and quote-to-revenue lifecycle | **Planned; blocked on completed Phase 8A and Repository Step 22; first extension wave is Step 23** |
| 8C | future bounded issues | Universal workflow automation, triggers, robots, approvals, programmable actions, pipelines, funnels, Kanban and configurable work | **Planned; minimum process primitives are first proven in Step 24** |
| 8D | future bounded issues | Expanded sales execution, cadences, territories, quotas, forecasting and revenue operations | **Planned** |
| 8E | future bounded issues | Omnichannel service, conversations, knowledge, SLA and field service | **Planned** |
| 8F | future bounded issues | Marketing automation, segmentation, campaigns and lifecycle journeys | **Planned** |
| 8G | future bounded issues | Customer Success, partner management, projects, documents and e-signature | **Planned** |
| 8H | future bounded issues | Analytics, reporting, data/integration platform, Admin Studio, low-code and mobile maturity | **Planned** |
| 9 | #12 | AI-native governed actor/tool layer | **Planned** |
| 10 | #13 | Signed marketplace and sandboxed untrusted extensions | **Planned** |
| 11 | #14 | Enterprise security, resilience, vertical and production proof | **Planned / continuous** |

The complete functional scope, wave boundaries, automation semantics and product-completion contract are normative in `PRODUCT_DEVELOPMENT_10_OF_10_PLAN.md`. `CRM_CAPABILITY_COVERAGE.md` remains the no-omission coverage baseline.

## 3. Architecture and repository program

Issue #194 remains open. The architecture stage ledger is:

- Stage A documentation/navigation baseline — **Complete**;
- Stage B dependency, crate and exception governance — **Complete through PRs #253, #255 and #257; ADR-032 Step 22 decision remains future closure evidence**;
- Stage C Customer Privacy golden owner and persistence model — **In progress; Party tombstone/no-orphan convergence accepted through Step 15**;
- Stage D contribution aggregation — **Complete through PR #249**;
- Stage E affected-scope CI — **Complete through PR #239; permanent-gate value review remains mandatory at Step 22**;
- Stage F generic conformance and contract lifecycle — **In progress**;
- Stage G transitional consolidation — **Complete through PR #259**;
- Stage H reproducible environment and navigation — **In progress**;
- Stage I frontend and operations parity — **Incomplete**.

Repository Steps 1–15 are complete. Repository Step 16 is the next permitted implementation step and is **not started**.

### 3.1 Accepted Repository Step 14 closure

PR #259 / accepted source `8aa0b33c6609e74f98363071c6e7c44ec59fc098` / squash merge `2b0b558077c444d44691371c8a2bcca2c14ae426` / 36 of 36 applicable permanent workflows on one unchanged meaningful user-authored exact head completes Repository Step 14 and Stage G.

The behavior-neutral Customer Accounts consolidation:

- removes `crm-customer-accounts-capability-composition`;
- moves its production contribution into `crm-customer-accounts-query-adapter`;
- preserves mutation planning in `crm-customer-accounts-capability-adapter`;
- preserves exact mutation/query inventory and contribution ordering through `crm-first-party-modules`;
- changes no public route, coordinate, contract, Protobuf schema, database schema, migration, persistence behavior or worker lifecycle;
- preserves tenant activation, FORCE RLS, live authorization, Party-reference validation, idempotency, audit and transaction semantics;
- synchronizes Approval, Discovery and Planning workflow package assertions from the historical 113-package baseline to the accepted 112-package workspace while retaining full behavioral, rollback and reapply checks.

Exact current metrics:

| Metric | Step 13 historical baseline | Current after Step 15 |
|---|---:|---:|
| Workspace packages | 113 | **112** |
| Internal dependency edges | 841 | **835** |
| Maximum dependency depth | 18 | **18** |
| Conservative public Rust items | 5,379 | **5,377** |
| Dependency declarations | 270 | **270** |
| Suppression occurrences | 91 | **91** |

This architecture reduction does not by itself complete Customer Privacy product readiness and does not declare architecture 10/10.

### 3.2 Accepted Repository Step 15 closure

Step 15 is accepted through five bounded slices:

- PR #263 / source `6c2a54f6780988a12fec3cd77ca2cd39ad349140` / merge `bd205e0af77b676654dff8ddf26d3b5b195880b2` / 32 of 32 — global-search tombstone convergence;
- PR #264 / source `e6c9d2901109c8d5b9e0f3cf783214407e26451a` / merge `e9fe1f352386d80a29d122db5d1ed6c47266bfaf` / 6 of 6 — Customer 360 tombstone convergence and root-membership removal;
- PR #265 / source `ef572bdf31c584c397c215cd1b62ee47cad54e64` / merge `2a0ee9c33fbe23cdf9c6dccb1acc7e3bd8bcba3a` / 19 of 19 — canonical owner execution, immutable-history rebuild and Search replay;
- PR #266 / source `ded5d80ae11bbf044b5bfe5b572e8dab521f884a` / merge `1f889a810c82da3d0fee12427eacccbe43613bac` / 19 of 19 — automatic Customer 360 generation rollover to `customer.customer-360.v2`;
- PR #267 / source `f1b72dbee09f152005cb3584b9bcc1573bf2c4fe` / merge `4a14a5a0bda6d25d27e7c2da7c5f4809fa1efbdf` / 19 of 19 — real `crm-api` restart and process-host no-orphan convergence on clean and rollback/reapplied PostgreSQL schemas.

The combined evidence proves stable non-reusable Party privacy tombstones, strict event/version lineage, non-disclosure in Search and Customer 360, deterministic rebuild/replay, automatic fresh-generation rollout and process-owned repair without package, dependency-edge, public-contract, route or migration growth.

### 3.3 Binding next repository sequence

The complete order is normative in the architecture plan. The remaining sequence begins:

16. reusable generic worker conformance — **next, not started**;
17. contract compatibility, deprecation, consumer-migration and retirement enforcement;
18. deterministic local lifecycle commands;
19. Customer Privacy worker and full process/end-to-end acceptance;
20. Phase 8A frontend, accessibility, browser and operations evidence;
21. Phase 8A closure;
22. Phase 8A architecture remeasurement, `crm-application-runtime` direct-dependency decision and permanent-gate value/cost review — checkpoint, not final 10/10;
23. Step 23 — Phase 8B.1 Catalog and effective-dated Pricing foundation, the first contrasting expert-domain wave;
24. Step 24 — Phase 8B.2 Quote/CPQ approval and process-heavy orchestration foundation, the second contrasting expert-domain wave;
25. final architecture 10/10 closure review only when every criterion is mechanically proven.

No later packet may start while an earlier item remains unfinished. Step 24 proves only the minimum reusable process primitives needed for the bounded CPQ slice; the complete universal workflow/robot/pipeline product remains Phase 8C.

## 4. Phase 8A completed foundation

Completed product slices:

- **8A.1–8A.6** — customer references, Party, Account, Contact Point, Party Relationship, Customer 360, Consent and reversible Identity Resolution;
- **8A.7** — governed immutable import and recovery;
- **8A.8** — governed deterministic export and recovery;
- **8A.9** — Customer Data Quality Rules, Completeness and Stewardship;
- **8A.10** — Governed Customer Enrichment and Provenance.

All nine Customer Privacy owner-scope implementations are accepted: Parties, Consents, Customer Accounts, Contact Points, Party Relationships, Identity Resolution, Customer Data Operations, Data Quality and Customer Enrichment.

Customer Data Operations owner-scope evidence is accepted through PR #188 / accepted source `07f34786e82fdfa78d263790e9f50541529006f8` / squash merge `089be72fa3010b4aa15aff7f9ea55fd86290f8fc` / 26 of 26 permanent workflows.

Data Quality owner-scope evidence is accepted through PR #190 / accepted source `dcfe8faebc7462b888f8fc1721cb379a40fea88a` / squash merge `deac197c97cddc15bb9916092ca87f6e767ce1de` / 27 of 27 permanent workflows.

Customer Enrichment owner-scope evidence is accepted through PR #192 / accepted source `e90e36027de18a07be68e43327ea732810ff332a` / squash merge `e41cbab0cd30819fcbe2e3c5f2c7415fc6de3e8c` / 28 of 28 permanent workflows.

Scope discovery and immutable snapshot execution is accepted through PR #206 / accepted source `086b17a95058eee285fcb67a903bd21d9263d357` / squash merge `95818fd3aeb54a9593a45642583f0b7224d5ecfe`.

## 5. Phase 8A.11 Customer Privacy — current boundary

Issue #126 remains **In progress**.

Accepted runtime and architecture evidence includes:

- scope discovery and immutable snapshot execution accepted through PR #206;
- deterministic planning through PR #209;
- permission-aware plan and outcome reads through PR #211;
- public approval through PR #220;
- final customer-subject policy prerequisite through PR #224;
- public restriction placement and first protected-owner enforcement through PR #226;
- public legal-hold placement and mandatory-retention precedence through PR #230;
- reusable mutation/query conformance through PR #235;
- durable replay-safe owner execution, checkpoints and real outcomes through PR #237;
- multi-plane affected-scope enforcement through PR #239;
- governed access/export assembly through PR #241;
- authoritative exact-nine owner-specific anonymization/deletion through PR #244;
- complete first-party contribution aggregation through PR #249;
- architecture governance closure through PR #257;
- first measured transitional consolidation through PR #259;
- Party tombstone, Search/Customer 360 convergence, immutable-history replay, automatic v2 rollover and real process-host no-orphan closure through PRs #263–#267.

Current public Customer Privacy inventory remains:

- seven public mutations;
- four permission-aware public queries;
- zero Customer Privacy workers.

Trusted-internal planning, retention evaluation, replay-safe owner execution, access/export assembly and exact-nine owner-action execution remain non-public.

### 5.1 Remaining Phase 8A.11 product work

The remaining product work after Step 15 is:

- restriction and legal-hold release/read lifecycle where required;
- reusable worker conformance and a real Customer Privacy worker lifecycle;
- disable/uninstall fail-closed semantics;
- frontend, accessibility and browser acceptance;
- production restore, SLO, observability, performance, security and supply-chain evidence.

Repository Step 16 owns only reusable generic worker conformance. It must not absorb Steps 17–21 or claim a Customer Privacy worker before Step 19.

## 6. Module-readiness accounting

Current product-complete expert modules: **0**.

A module is not product-complete merely because a crate, schema, manifest or backend path exists. Product complete requires:

- sufficient domain breadth;
- governed APIs and contracts;
- authoritative persistence and migration ownership;
- tenant activation and live authorization;
- audit, idempotency and operational evidence;
- product UX, accessibility and browser acceptance;
- production restore, SLO, performance and security proof.

Phase 8A remains incomplete until these criteria are met for the current customer-master/privacy scope.

## 7. Product development program after Phase 8A

The complete product-portfolio program is normative in `PRODUCT_DEVELOPMENT_10_OF_10_PLAN.md`.

### 7.1 Phase 8B — commercial lifecycle

Planned independent owners and coordination boundaries include:

- Product Catalog and variants;
- effective-dated Pricing, price books and promotions;
- CPQ configuration and validation;
- Quotes, revisions, expiry and approvals;
- Orders and fulfillment coordination;
- Contracts, amendments, renewals and termination;
- Subscriptions, Entitlements and Usage;
- governed Billing/ERP/payment/tax/fulfillment integration.

Step 23 pre-registers Catalog/Pricing as the reference-heavy extension test. Step 24 pre-registers Quote/CPQ approval and orchestration as the process-heavy contrast. These choices may not be replaced by easier demonstration modules merely to improve architecture scores.

### 7.2 Phase 8C — automation, workflows and configurable work

The universal automation product must include:

- record/event/webhook/time/inactivity/SLA/manual/API triggers;
- typed conditions, branches, decision tables and business calendars;
- governed robot actions invoking exact domain capabilities;
- waits, timers, parallel branches, joins, reusable subflows and bounded loops;
- human tasks, serial/parallel approvals, delegation and escalation;
- durable execution, retry, idempotency, replay, crash recovery and dead-letter handling;
- versioned definitions, simulation, dry run, publication, rollback and run diagnostics;
- allowlisted connectors and sandboxed typed custom functions;
- no arbitrary SQL, unrestricted HTTP, raw secret access or hidden mutation bypass.

### 7.3 Pipelines, funnels and Kanban

Pipelines must be configurable for supported business objects with versioned stages, entry/exit rules, allowed transitions, required fields/checklists, stage history, duration and conversion analytics.

Kanban/board UX must support drag-and-drop with server-authoritative transition validation, customizable cards, swimlanes, filters, saved views, aggregates, WIP/staleness/SLA indicators, bulk movement with preview/partial-failure reporting, authorization-aware cards, accessibility and large-board performance.

The same authoritative state must also support list, table, calendar, timeline, workload and reporting views.

### 7.4 Sales cadences and guided execution

The product must support enrollment, email/call/task/meeting/manual/wait steps, conditional branching, automatic stop conditions, consent and quiet-hours enforcement, work queues, playbooks, reassignment, A/B variants, analytics and governed AI-assisted drafting.

### 7.5 Remaining product trains

Later expert waves cover:

- expanded Sales and Revenue Operations;
- omnichannel Communications, Service, Knowledge and Field Service;
- Marketing segmentation, campaigns and journeys;
- Customer Success, partner/channel management and retention;
- projects/configurable work, documents and e-signature;
- analytics, reporting, data platform and integrations;
- Admin Studio, custom objects/fields, layouts, rules and controlled low-code;
- responsive/mobile/offline product maturity;
- Phase 9 AI governance and tools;
- Phase 10 marketplace and sandboxed extensions;
- Phase 11 enterprise/vertical production proof.

No product wave is complete without the cross-wave acceptance contract in the product plan.

## 8. Repository Step 22 decision boundary

Repository Step 22 is a measurement and remediation checkpoint, not an automatic completion declaration.

Before Step 22 can close:

- every internal direct dependency of `crm-application-runtime` must be classified as removed, platform-generic, owner-specific-unavoidable or test-only;
- every safely removable owner-specific dependency must be removed;
- each retained owner-specific dependency must prove an unavoidable stable process-composition boundary and prove that ordinary owner changes do not modify the runtime manifest or owner-specific runtime source;
- mere non-growth against the current direct-dependency budget is insufficient;
- every permanent workflow, job and gate must record its concrete failure mode, observed defect evidence or preventive rationale, duplication/overlap, execution cost, owner, retain/simplify/merge/remove decision and retirement condition;
- duplicate or low-value gates must be simplified, merged or removed unless independent value is proven;
- every new permanent gate must satisfy the same entry contract.

The complete binding rules are in ADR-032.

## 9. Architecture and product 10/10 declaration boundaries

Architecture 10/10 remains reserved for Step 25 after:

- Steps 16–21 close their mechanical criteria;
- Phase 8A is complete;
- Step 22 leaves zero unresolved runtime-fan-in or permanent-gate value decisions;
- the pre-registered Step 23 Catalog/Pricing wave and Step 24 Quote/CPQ orchestration wave prove bounded extension cost and validate the Step 22 decisions;
- dependency, package, public-surface, change-locality, CI, local-development, contract-lifecycle, frontend and operations measurements show no regression;
- every final criterion in the architecture plan is mechanically reproduced.

Product-level 10/10 is a later portfolio claim. It additionally requires every applicable family in `PRODUCT_DEVELOPMENT_10_OF_10_PLAN.md` and `CRM_CAPABILITY_COVERAGE.md` to be production-complete or explicitly classified as optional/vertical/external integration with a proven boundary.

Until then issue #194 remains open, Phase 8A and Customer Privacy remain incomplete, and current product-complete expert modules remain **0**.
