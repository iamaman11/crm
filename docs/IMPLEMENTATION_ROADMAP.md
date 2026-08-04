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
- Stage H reproducible environment and navigation — **Complete through PR #285**;
- Stage I frontend and operations parity — **Incomplete**.

Repository Steps 1–19 are complete. Repository Step 19 is complete through PRs #287–#290. Repository Step 20 is the only next permitted implementation packet.

### 3.1 Accepted Repository Step 14 closure

PR #259 / accepted source `8aa0b33c6609e74f98363071c6e7c44ec59fc098` / squash merge `2b0b558077c444d4469137c8a2bcca2c14ae426` / 36 of 36 applicable permanent workflows on one unchanged meaningful user-authored exact head completes Repository Step 14 and Stage G.

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

### 3.3 Accepted Repository Step 16 closure

Step 16 is accepted through two bounded slices:

- PR #269 / source `74b1d7b0f8764fcd90839b7aab25f8f82fe5e552` / merge `6f82de0a7b2dcd1ab5dd0ae6473d46d7d9d34bdd` / 20 of 20 — business-neutral worker conformance plus representative Customer Enrichment and CRM API import adoption, including no-side-effect denial, retryable preservation, exact restart recovery, completed replay and tenant isolation;
- PR #270 / source `8e2baac0822eefbb6d3c474ffce0cee69e3e4e98` / merge `ce0ca881461d1ee8964a11b28c1fcff46cf145cb` / 17 of 17 — two live `crm-api` executors mechanically proven to serialize the same durable Party-import work and converge to exactly one committed Party/idempotency/event/audit effect with no duplicate replay.

The accepted worker suite keeps owner-specific semantics outside the generic algorithms, adds no generic lease API, and changes no production algorithm, public contract, route, schema, migration, package, dependency or permanent workflow. Customer Privacy remains at zero workers until Step 19.

### 3.4 Accepted Repository Step 17 closure

Step 17 is accepted through three bounded slices:

- PR #275 / source `1c6366b557e255a14a677758fd87f7fe63184a89` / merge `60d5974349a04c462475aadd4af0a37bada9713b` / 23 of 23 — published wire-compatible `activities.task.create@1.1.0`, migrated the sole repository-owned consumer and made live authorization overlap exact-version safe;
- PR #278 / source `228f459963e571a830aee6c43cddd15e3c9b0d5f` / merge `996d634a33945e618be5ff81c297f0f617ce19d5` / 8 of 8 — made production zero-usage retirement evidence SHA-256-bound, complete, append-only and fail closed;
- PR #279 / source `0dce0895edec56508df1b4fc880d09ab27fc00df` / merge `ea3d3894d05e3a8d814aff69824b593843763d03` / 22 of 22 — proved the coordinate was never externally released through live empty GitHub Releases, tags and deployments, non-publishable packages and no external consumer record, then retired `activities.task.create@1.0.0` while preserving its historical tombstone and keeping `telemetry.zero_since` null.

The accepted Step 17 result keeps `activities.task.create@1.1.0` as the sole live create coordinate. `activities.task.create@1.0.0` is absent from the current provider manifest and production capability catalog, rejected before payload decoding, retained only as immutable historical registry/lifecycle evidence and classified `never_externally_released`. The ordinary 30-day/production-zero-usage path remains binding for released contracts; no production history was fabricated. Repository Step 18 is complete through accepted PRs #281, #283 and #285: PR #281 / source `76a0d93d594b6ffbb890f90d3cb9037febf4c3f8` / squash merge `87bc0d33befc4525b62aa7e0e1884abc07e12abf` / 7 of 7; PR #283 / source `5f4480fafd37cc8c89df60f3688e756d7f881af8` / squash merge `21e2f73b57d2c35c16eccc15ee3e075e818f488a` / 7 of 7; PR #285 / source `a522b8b11a0c6f143694f516e7a7f9d522c18ce3` / squash merge `a906f2c514285974113749b8b8ad9446202a5fa1` / 19 of 19 applicable permanent workflows, each on one unchanged exact head. The accepted lifecycle delivers deterministic repository-pinned `doctor`, locked isolated `bootstrap`, checkout-owned PostgreSQL `dev-up` / `dev-reset`, versioned idempotent `seed-demo` through the governed Party mutation gateway and real-process `smoke` proving readiness, permission, authentication and tenant boundaries. The next permitted bounded implementation packet is Repository Step 20: Phase 8A frontend, accessibility, browser and operations evidence.


### 3.5 Accepted Repository Step 18 closure

Repository Step 18 is complete through accepted PRs #281, #283 and #285: PR #281 / source `76a0d93d594b6ffbb890f90d3cb9037febf4c3f8` / squash merge `87bc0d33befc4525b62aa7e0e1884abc07e12abf` / 7 of 7; PR #283 / source `5f4480fafd37cc8c89df60f3688e756d7f881af8` / squash merge `21e2f73b57d2c35c16eccc15ee3e075e818f488a` / 7 of 7; PR #285 / source `a522b8b11a0c6f143694f516e7a7f9d522c18ce3` / squash merge `a906f2c514285974113749b8b8ad9446202a5fa1` / 19 of 19 applicable permanent workflows, each on one unchanged exact head. The accepted lifecycle delivers deterministic repository-pinned `doctor`, locked isolated `bootstrap`, checkout-owned PostgreSQL `dev-up` / `dev-reset`, versioned idempotent `seed-demo` through the governed Party mutation gateway and real-process `smoke` proving readiness, permission, authentication and tenant boundaries. The next permitted bounded implementation packet is Repository Step 20: Phase 8A frontend, accessibility, browser and operations evidence.

The accepted Step 18 lifecycle changes no product ownership, public contract, schema, migration, dependency or lockfile. Step 18 is complete; Step 19 is complete; Step 20 is next and Customer Privacy still publishes zero workers until that packet is accepted.

### 3.6 Binding next repository sequence

The complete order is normative in the architecture plan. The remaining sequence begins:

16. reusable generic worker conformance — **complete through PR #270**;
17. contract compatibility, deprecation, consumer-migration and retirement enforcement — **complete through PR #279**;
18. deterministic local lifecycle commands — **complete through PR #285**;
19. Customer Privacy worker and full process/end-to-end acceptance — **complete through PR #290**;
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
- reusable worker conformance and representative real-worker adoption through PRs #269 and #270;
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
- one Customer Privacy owner worker.

Trusted-internal planning, retention evaluation, replay-safe owner execution, access/export assembly and exact-nine owner-action execution remain non-public.

### 5.1 Remaining Phase 8A.11 product work

The remaining Phase 8A.11 product work after accepted Repository Step 19 is:

- restriction and legal-hold release/read lifecycle where required;
-;
-;
- frontend, accessibility and browser acceptance;
- production restore, SLO, observability, performance, security and supply-chain evidence.

Repository Step 17 contract lifecycle enforcement is complete. Repository Step 18 now owns only deterministic local lifecycle commands and must not absorb Steps 19–21 or claim a Customer Privacy worker before Step 19.

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

## Repository Step 19 accepted closure

Repository Step 19 is complete only through the combined accepted evidence below, each on one unchanged exact source head with no unresolved comments, reviews or review threads:

- PR #287 / source `23b2f4ea660bcd46884fe054cd0c37e89b1495c4` / squash merge `c0fec3ae08c836ab483737442ed4377c99c85e9a` / **11 of 11** applicable permanent workflows — added the bounded Customer Privacy owner-worker boundary without public ingress or new schema/dependency surface;
- PR #288 / source `b99a4fc4ff7ef5cfd47813b487900b4c2a9f3b77` / squash merge `bc653de5f1a853791d3ab4a03f59f3daad54bf54` / **24 of 24** — added PostgreSQL ready-work discovery for planned Customer Privacy owner actions;
- PR #289 / source `3e21e79e1600727ebcda222af389d568d857cff8` / squash merge `d1c4dd278853a1e6a426fab284c70b3529d42833` / **24 of 24** — registered `crm.customer-privacy` / `owner-execution` at phase `260` in the production `ApplicationRuntime`, with activation gating and replay-safe canonical execution;
- PR #290 / source `9bbb339f39133955a7f42ea67f3334e597066e2e` / squash merge `49c5e35814adceb2be9d4cc2302bf10032b807a0` / **19 of 19** — proved the assembled real `crm-api` lifecycle on clean and rollback/reapplied PostgreSQL schemas: ready-work discovery, a real Parties privacy action, one durable attempt, successful outcome, completed checkpoint, audit evidence, owner event/outbox and final case transition, plus restart no-duplicate proof and uninstall no-discovery/no-effect proof.

The accepted first-party background-worker inventory is now **one Customer Privacy owner worker**: module `crm.customer-privacy`, worker `owner-execution`, phase `260`. This is a production background worker, not a new public capability route; the latest public Customer Privacy inventory remains **seven mutations and four permission-aware public queries**.

Repository Steps 1–19 are complete. Repository Step 20 — Phase 8A frontend, accessibility, browser and operations evidence — is the only next permitted implementation packet. Phase 8A.11 / issue #126 remains in progress; Customer Privacy is not product-complete; current product-complete expert modules remain zero; architecture 10/10 and the Universal CRM product are not declared complete.

The Step 19 packets add no crate, dependency, route, public API, module manifest, migration or schema. The conservative public Rust surface remains **5,377**, suppression occurrences remain **91**, and `crm-application-runtime` non-comment/source LOC remains within the frozen **7,269** ceiling.
