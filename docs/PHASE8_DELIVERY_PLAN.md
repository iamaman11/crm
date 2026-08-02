# Ultimate CRM — Phase 8 Delivery Plan

Status: **Normative Phase 8 delivery sequence**  
Parent roadmap: `IMPLEMENTATION_ROADMAP.md`  
Product portfolio sequence: `PRODUCT_DEVELOPMENT_10_OF_10_PLAN.md`  
Architecture order: `ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md` section 2.4  
Step 22 decision: `adr/ADR-032-step-22-runtime-fanin-and-permanent-gate-value.md`  
Current state: `PROJECT_STATUS.md`

## 1. Phase 8 objective

Phase 8 delivers expert CRM domains and product-quality cross-domain journeys on top of the governed platform completed in Phases 0.1–7.

Phase 8 is split into a sequenced portfolio:

- **Phase 8A** — canonical customer master, identity, consent, governed customer-data operations, enrichment and privacy;
- **Phase 8B** — Product Catalog, Pricing, CPQ and quote-to-revenue;
- **Phase 8C** — universal workflow automation, triggers, robots, approvals, programmable actions, pipelines, funnels, Kanban and configurable work;
- **Phase 8D** — expanded sales execution, cadences, forecasting and revenue operations;
- **Phase 8E** — omnichannel service, knowledge and field service;
- **Phase 8F** — marketing automation and lifecycle journeys;
- **Phase 8G** — Customer Success, partner management, projects, documents and e-signature;
- **Phase 8H** — analytics, integrations, Admin Studio, low-code and product-surface maturity.

The complete functional and completion contract for these waves is normative in `PRODUCT_DEVELOPMENT_10_OF_10_PLAN.md`. Phase 8A remains **In progress**. It must close before Phase 8B implementation begins. Repository Step 22 then remeasures architecture, resolves `crm-application-runtime` fan-in and reviews permanent-gate value/cost; it does not automatically declare 10/10.

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
10. Step 22 cannot close through non-growth alone; ADR-032 requires dependency-by-dependency and gate-by-gate decisions.
11. Workflow, trigger, robot, pipeline, Kanban, cadence and low-code behavior must use exact governed domain capabilities and may not bypass owner, authorization, privacy, audit or idempotency boundaries.
12. A product wave closes only with backend, worker/process, frontend, accessibility, browser, import/migration and operations evidence for representative real journeys.

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

Scope discovery and immutable snapshot execution is accepted through PR #206 / accepted source `086b17a95058eee285fcb67a903bd21d9263d357` / squash merge `95818fd3aeb54a9593a45642583f0b7224d5ecfe`.

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
- blocking architecture, dependency, suppression and affected-scope governance;
- Party tombstone, Search/Customer 360 convergence, immutable-history rebuild, automatic Customer 360 v2 rollover and real process-host no-orphan repair through Repository Step 15.

Latest accepted public Customer Privacy inventory remains:

- **7 public mutations**;
- **4 permission-aware public queries**;
- **0 Customer Privacy workers**.

Trusted-internal planning, retention evaluation, replay-safe owner execution, access/export assembly and exact-nine action execution remain non-public.

## 5. Accepted Repository Step 14 and Step 15 architecture results

Repository Step 14 and architecture Stage G are complete through PR #259 / accepted source `8aa0b33c6609e74f98363071c6e7c44ec59fc098` / squash merge `2b0b558077c444d44691371c8a2bcca2c14ae426` / 36 of 36 applicable permanent workflows on one unchanged meaningful user-authored exact head.

The packet is behavior-neutral. It removes `crm-customer-accounts-capability-composition`, moves the owner production contribution into `crm-customer-accounts-query-adapter`, preserves mutation planning and exact first-party inventory, and changes no public contract, route, schema, migration, persistence or worker behavior.

Repository Step 15 is complete through:

- PR #263 / source `6c2a54f6780988a12fec3cd77ca2cd39ad349140` / merge `bd205e0af77b676654dff8ddf26d3b5b195880b2` / 32 of 32;
- PR #264 / source `e6c9d2901109c8d5b9e0f3cf783214407e26451a` / merge `e9fe1f352386d80a29d122db5d1ed6c47266bfaf` / 6 of 6;
- PR #265 / source `ef572bdf31c584c397c215cd1b62ee47cad54e64` / merge `2a0ee9c33fbe23cdf9c6dccb1acc7e3bd8bcba3a` / 19 of 19;
- PR #266 / source `ded5d80ae11bbf044b5bfe5b572e8dab521f884a` / merge `1f889a810c82da3d0fee12427eacccbe43613bac` / 19 of 19;
- PR #267 / source `f1b72dbee09f152005cb3584b9bcc1573bf2c4fe` / merge `4a14a5a0bda6d25d27e7c2da7c5f4809fa1efbdf` / 19 of 19.

The combined Step 15 evidence proves stable Party privacy tombstones, strict lineage, Search and Customer 360 non-disclosure, deterministic rebuild/replay, automatic `customer.customer-360.v2` rollout and real `crm-api` process-owned no-orphan repair on clean and rollback/reapplied schemas.

Exact measured result remains:

- workspace packages: **113 → 112**;
- internal dependency edges: **841 → 835**;
- conservative public Rust items: **5,379 → 5,377**;
- maximum dependency depth: **18 → 18**;
- dependency declarations: **270 → 270**;
- suppression occurrences: **91 → 91**.

These results do not complete Phase 8A.11. Customer Privacy remains incomplete and Current product-complete expert modules: **0**.

## 6. Remaining Phase 8A.11 product work

The remaining work is sequenced, not parallel:

### Repository Step 16 — next, not started

- reusable generic worker conformance;
- activation, authorization, tenant, replay, lease/crash and no-side-effect guarantees;
- representative contrasting real-worker adoption without owner-specific logic in generic algorithms.

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

No Step 17 or later implementation may begin before Step 16 is accepted and synchronized.

## 7. Phase 8A closure criteria

Phase 8A is complete only when:

- Customer Privacy required lifecycle behavior is complete;
- accepted Party tombstone/no-orphan and projection/search/cache convergence remain protected by permanent evidence;
- public and internal route inventories are exact and intentional;
- a real Customer Privacy worker lifecycle is accepted;
- disable/uninstall and recovery fail closed;
- critical frontend journeys pass browser and accessibility acceptance;
- production restore, SLO, observability, performance, security and supply-chain gates are executable;
- clean apply, rollback, reapply and repeated acceptance succeed;
- roadmap, status, catalog, issues and generated packet agree;
- product-complete readiness is justified by product evidence, not by architecture structure alone.

Phase 8A closure remains a product ledger result. It does not by itself resolve architecture Step 22.

## 8. Repository Step 22 mandatory review

After Step 21, Step 22 must complete all ADR-032 obligations before Phase 8B entry.

### 8.1 `crm-application-runtime` fan-in

Step 22 must inventory every internal direct dependency and classify it as:

- removed;
- platform-generic;
- owner-specific-unavoidable;
- test-only.

Every safely removable owner-specific dependency must be removed. Every retained owner-specific dependency must prove an unavoidable stable process-composition boundary, a named owner and a removal/review condition.

Mere non-growth against the current direct-dependency budget is insufficient. Ordinary existing-owner capability changes must not modify `crm-application-runtime/Cargo.toml` or owner-specific runtime composition. Steps 23 and 24 must validate that conclusion under contrasting expert-domain waves.

### 8.2 Permanent-gate value and cost

Every permanent workflow, job and repository gate must record:

- concrete prevented failure mode;
- real defects previously detected or a specific preventive rationale;
- overlap and duplication;
- execution duration, runner/fan-out and expensive environment cost;
- owner;
- retain, simplify, merge or remove decision;
- retirement/re-review condition.

Duplicate or low-value gates must be simplified, merged or removed unless independent value is proven. Every new permanent gate must declare the same information before acceptance.

Step 22 closes only with zero unresolved runtime-fan-in classifications and zero unresolved permanent-gate value decisions.

## 9. Phase 8B entry and pre-registered architecture proof waves

Phase 8B / issue #29 remains planned. Entry requires:

1. completed Phase 8A through Repository Step 21;
2. Repository Step 22 architecture remeasurement, runtime fan-in decision and permanent-gate value/cost review with no hidden regression or unresolved decision;
3. a bounded Step 23 first expert-domain wave.

### Step 23 / Phase 8B.1 — Catalog and Pricing

The first later-domain proof is pre-registered as Product Catalog plus effective-dated Pricing. It must prove stable references, version/effective-date behavior, bounded extension cost, owner-owned production contribution, frontend administration/lookup and no owner-specific `crm-application-runtime` edit.

### Step 24 / Phase 8B.2 — Quote/CPQ approvals and orchestration

The second contrasting proof is pre-registered as Quote/CPQ with discount/exception rules, serial/parallel approvals, human tasks, waits/timers, retry/recovery and Kanban/list work queues. It must prove the process-heavy worker/orchestration case without central owner-specific runtime growth or an unjustified new permanent gate.

Step 24 establishes only the minimum reusable process primitives required by its bounded product slice. Full automation authoring, universal trigger/action catalog, programmable robots, general pipelines/Kanban, cadences and configurable work remain Phase 8C.

After Step 25, remaining Phase 8B packets complete CPQ depth, Quotes, Orders, Contracts, Subscriptions, Entitlements, Usage and governed Billing/ERP/payment/tax/fulfillment integration.

## 10. Phase 8C — automation, pipelines, Kanban and programmable work

The full automation product must cover:

- record/field/stage/event/webhook/time/inactivity/SLA/manual/API/import triggers;
- typed conditions, branches, decision tables and business calendars;
- governed actions that invoke exact owner capabilities;
- assignment, task, notification, communication, document, connector and AI-tool actions;
- waits, timers, parallel branches, joins, reusable subflows and bounded loops;
- human tasks, serial/parallel approvals, delegation, expiry and escalation;
- durable instances, retries, idempotency, replay, crash recovery and dead letters;
- definition versioning, simulation, dry run, impact analysis, publication and rollback;
- run explorer, diagnostics, metrics, quotas and operator recovery;
- sandboxed typed custom functions without raw SQL, unrestricted network or secret access.

Pipelines must support versioned stages, required fields/checklists, allowed transitions, automatic/manual stage changes, history and conversion/duration analytics.

Kanban must support validated drag/drop, customizable cards, swimlanes, filters, saved views, aggregates, WIP/staleness/SLA indicators, bulk movement with preview and partial-failure results, authorization-aware disclosure, accessibility and large-board performance.

Sales/service/success cadences must support enrollment, activity/wait steps, branches, automatic stop conditions, consent/quiet-hour/frequency controls, work queues, playbooks, versioning and analytics.

The complete semantics and acceptance targets are in `PRODUCT_DEVELOPMENT_10_OF_10_PLAN.md`.

## 11. Later Phase 8 product trains

Following the product plan:

- **8D:** expanded Sales, Revenue Operations, territories, quotas, forecasting, pipeline inspection and guided execution;
- **8E:** omnichannel Communications, Service, Knowledge, SLA and Field Service;
- **8F:** Marketing campaigns, segmentation, journeys, experiments and attribution;
- **8G:** Customer Success, PRM, projects/configurable work, documents and e-signature;
- **8H:** analytics/reporting, data/integration platform, Admin Studio, custom objects/fields, low-code, responsive/mobile/offline maturity.

Phases 9–11 then add governed AI, signed marketplace/sandboxed extensions and enterprise/vertical production proof.

No later product train may be represented as complete from backend/platform work alone.

## 12. Binding repository continuation

Repository Steps 1–15 are complete.

16. Repository Step 16 — reusable worker conformance — **next, not started**;
17. Repository Step 17 — contract lifecycle enforcement;
18. Repository Step 18 — deterministic local lifecycle;
19. Repository Step 19 — Customer Privacy worker and full E2E;
20. Repository Step 20 — frontend and operations evidence;
21. Repository Step 21 — Phase 8A closure;
22. Repository Step 22 — architecture remeasurement, `crm-application-runtime` fan-in decision and permanent-gate value/cost review, not final 10/10;
23. Repository Step 23 — Phase 8B.1 Catalog/Pricing wave validating Step 22;
24. Repository Step 24 — Phase 8B.2 Quote/CPQ orchestration wave validating Step 22;
25. Repository Step 25 — final architecture 10/10 review only if every normative criterion is mechanically proven.

Architecture 10/10 remains unclaimed. Product-level 10/10 remains a later portfolio result governed by `PRODUCT_DEVELOPMENT_10_OF_10_PLAN.md`. Issue #194 and issue #126 remain open. Current product-complete expert modules: **0**.
