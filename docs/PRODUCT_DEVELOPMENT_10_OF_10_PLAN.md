# Ultimate CRM — Product Development 10/10 Plan

Status: **Normative product-portfolio and functional-completeness plan**  
Parent roadmap: [`IMPLEMENTATION_ROADMAP.md`](IMPLEMENTATION_ROADMAP.md)  
Functional coverage guardrail: [`CRM_CAPABILITY_COVERAGE.md`](CRM_CAPABILITY_COVERAGE.md)  
Current state: [`PROJECT_STATUS.md`](PROJECT_STATUS.md)  
Business ownership/readiness: [`MODULE_CATALOG.md`](MODULE_CATALOG.md)  
Architecture execution order: [`ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md`](ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md) section 2.4

## 1. Purpose

This plan turns the broad CRM capability coverage baseline into a sequenced product-development program for a universal, modern, configurable and automation-first CRM.

The target is not merely a database of customers and deals. The completed product must support daily operational work across sales, service, marketing, customer success, revenue operations, projects and partner channels, with safe low-code customization, programmable automation, AI assistance and enterprise-grade governance.

A feature is not complete because a type, schema, endpoint or backend path exists. Product completion requires authoritative ownership, governed runtime behavior, understandable UX, accessibility, browser acceptance, migration/import paths, observability, recoverability, security and measurable user value.

This plan does not change the single repository execution order. Repository Steps 1–19 are complete. Repository Step 20 is the only next permitted implementation packet. Product waves below begin only when their stated architecture and prior-product dependencies are accepted.

## 2. Product 10/10 definition

The universal CRM may claim product-level 10/10 only when all applicable capability families are either production-complete or intentionally classified as optional/vertical/external integration with a proven extension boundary.

The target product must provide:

1. canonical customer and organization data;
2. sales force automation and configurable pipelines;
3. visual Kanban, list, calendar, timeline and workload views;
4. programmable workflows, triggers, robots and approvals;
5. sales cadences, playbooks and guided next actions;
6. omnichannel communication and agent inboxes;
7. customer service, knowledge and SLA management;
8. marketing segmentation, campaigns and journeys;
9. customer success, onboarding, renewal and expansion management;
10. catalog, pricing, CPQ, quotes and commercial lifecycle;
11. orders, contracts, subscriptions, entitlements and usage references;
12. projects, configurable work and operational cases;
13. documents, templates and e-signature integration;
14. analytics, reporting, forecasting and performance management;
15. import/export, APIs, webhooks, connectors and synchronization;
16. admin studio, metadata, custom objects/fields and controlled low-code;
17. mobile/responsive and offline-capable workflows where required;
18. AI-native assistance and governed tool execution;
19. signed marketplace and vertical extension support;
20. enterprise identity, privacy, security, resilience and operations proof.

No single module may absorb all of these responsibilities. Each mutable business concept retains one authoritative owner and communicates through governed versioned boundaries.

## 3. Product principles

### 3.1 Automation-first, not automation-bypass

Automation is a first-class product surface, but it has no alternate mutation or authorization path.

Every automated action must invoke the same exact versioned domain capability used by an authorized interactive/API caller. A workflow may decide **when** and **why** to invoke a capability, but it may not directly write another owner’s storage, execute arbitrary SQL, obtain unrestricted secrets, or call arbitrary network endpoints outside governed connector policy.

### 3.2 Configurable without becoming ungoverned

Tenants must be able to configure fields, layouts, pipelines, stages, rules, workflows, approvals, queues, reports and dashboards without rebuilding the product. Configuration publication must remain versioned, impact-analyzed, testable, reversible and auditable.

Critical invariants remain compiled or owner-enforced. A low-code rule cannot disable tenant isolation, authorization, privacy, audit, idempotency, contract compatibility or retention rules.

### 3.3 Multiple work views over one authoritative state

Kanban boards, funnels, lists, calendars, timelines, workload queues and dashboards are user experiences over authoritative domain state and rebuildable projections. Drag-and-drop and bulk actions must still use governed capabilities with conflict handling, authorization and auditable partial-failure behavior.

### 3.4 Universal core, optional expert modules

The core supplies identity, security, metadata, automation, search, communications boundaries, analytics contracts and extension governance. Expert domains and industry packages remain installable modules so tenants do not inherit irrelevant complexity.

### 3.5 Product evidence, not checklist completion

Each wave must prove at least one real cross-domain journey through backend, worker/process, frontend, accessibility, browser and operations acceptance. A coverage bullet without an accepted journey remains planned.

## 4. Binding product wave map

| Wave | Primary result | Entry dependency | Completion state |
|---|---|---|---|
| Phase 8A | Customer master, identity, data quality, enrichment, consent and privacy lifecycle | Current active program | **In progress** |
| Repository Steps 19–21 | Complete remaining Phase 8A product/runtime/UX/operations evidence | Sequential architecture order | **Next through planned** |
| Repository Step 22 | Architecture remeasurement, runtime fan-in decision and permanent-gate value review | Phase 8A complete | **Planned checkpoint** |
| Repository Step 23 / Phase 8B.1 | Catalog and effective-dated pricing foundation | Step 22 accepted | **Planned first extension wave** |
| Repository Step 24 / Phase 8B.2 | Quote/CPQ approvals and process-heavy orchestration foundation | Step 23 accepted | **Planned contrasting extension wave** |
| Repository Step 25 | Final architecture 10/10 review | Steps 23–24 accepted | **Planned architecture closure** |
| Phase 8B.3–8B.8 | Complete quote-to-revenue lifecycle | Step 25 and prior Phase 8B packets | **Planned** |
| Phase 8C | Universal workflow automation, pipelines, Kanban, robots and configurable work | Stable worker/contract/runtime foundations; bounded Phase 8B orchestration proof | **Planned** |
| Phase 8D | Expanded sales execution and revenue operations | Phase 8C core automation and Phase 8B commercial references | **Planned** |
| Phase 8E | Omnichannel service, knowledge and field service | Communications and automation foundations | **Planned** |
| Phase 8F | Marketing, segmentation, journeys and growth | Consent, communications, automation and analytics foundations | **Planned** |
| Phase 8G | Customer success, PRM, projects, documents and e-signature | Customer/commercial/service foundations | **Planned** |
| Phase 8H | Analytics, data platform, admin studio, low-code and mobile maturity | Representative expert modules in production form | **Planned** |
| Phase 9 | AI-native CRM actor, retrieval and governed tools | Stable capability/query registries and product journeys | **Planned** |
| Phase 10 | Signed marketplace and sandboxed extensions | Mature module lifecycle and permissions | **Planned** |
| Phase 11 | Enterprise/vertical production proof | Prior product families at target readiness | **Planned / continuous** |

The wave map is dependency order, not a promise that all work inside one wave belongs in one pull request. Every implementation remains bounded and exact-head accepted.

## 5. Phase 8A — customer foundation and privacy

Phase 8A remains the prerequisite for all customer-facing automation and engagement.

Accepted prerequisites now include Party tombstone/no-orphan and projection convergence, generic worker conformance, contract lifecycle enforcement, and the complete deterministic local lifecycle through PR #285 / source `a522b8b11a0c6f143694f516e7a7f9d522c18ce3` / merge `a906f2c514285974113749b8b8ad9446202a5fa1` / 19 of 19 applicable permanent workflows.

Phase 8A must still finish:

- a real Customer Privacy worker lifecycle and complete process/end-to-end acceptance;
- customer privacy frontend/browser/accessibility acceptance;
- restore, SLO, observability, performance, security and supply-chain evidence.

No automation, marketing, communication or AI feature may use customer data in a way that bypasses consent, restriction, legal hold, deletion or live authorization.

## 6. Phase 8B — commercial lifecycle and quote-to-revenue

### 6.1 Phase 8B.1 / Repository Step 23 — Catalog and Pricing foundation

The first extension wave is deliberately reference-heavy and effective-dated.

Required product scope:

- products and services;
- families, categories, variants and units of measure;
- bundles, options, dependencies and compatibility constraints;
- catalog versions and effective dates;
- price books, currencies and price-list versions;
- customer, segment and channel price eligibility;
- base prices, tiered prices and governed promotion references;
- active/inactive/retired lifecycle;
- import/export and migration profile;
- catalog search, list and record UX;
- authorization-aware product selection for downstream domains.

Required proof:

- ordinary capability growth does not edit owner-specific application-runtime composition;
- no new unjustified permanent gate;
- stable references survive catalog and price version changes;
- deterministic effective-date evaluation and replay;
- frontend and browser journey for product/price administration and lookup.

### 6.2 Phase 8B.2 / Repository Step 24 — CPQ and approval/orchestration foundation

The second extension wave is deliberately process-heavy and must validate workers, timers, approvals and human tasks.

Required product scope:

- quote creation and revision lineage;
- quote lines referencing exact catalog/price versions;
- configurable product validation;
- discount and exception rules;
- serial and parallel approval chains;
- human approval tasks, delegation, expiry and escalation;
- wait/timer semantics and business calendars;
- immutable orchestration execution evidence;
- retry, idempotency, cancellation and recovery;
- quote comparison, expiry and customer-ready document preview;
- Kanban/list views for quote work queues and approval state.

This step may establish the minimum reusable automation primitives needed for CPQ, but must not prematurely absorb the full Phase 8C product surface.

### 6.3 Remaining Phase 8B packets

After Step 25 architecture closure, Phase 8B continues through bounded packets:

- **8B.3 — CPQ configuration depth:** option groups, dependency rules, guided selling, validation explanations and simulation;
- **8B.4 — Quotes:** customer-facing revisions, acceptance, expiry, approval evidence and document generation;
- **8B.5 — Orders:** order capture, change/cancel lifecycle, fulfillment coordination and reconciliation boundaries;
- **8B.6 — Contracts:** terms, amendments, renewals, termination, obligations and e-signature references;
- **8B.7 — Subscriptions/Entitlements/Usage:** plans, lifecycle changes, entitlement checks and governed usage ingestion;
- **8B.8 — Billing integration:** invoicing/payment/tax/ERP boundaries, reconciliation and commercial lineage without moving accounting ownership into CRM.

A complete commercial lineage must connect opportunity → quote → order → contract → subscription/entitlement while preserving each owner’s authority.

## 7. Phase 8C — universal workflow automation, robots and configurable work

Phase 8C is the full automation product program requested for a modern universal CRM.

### 7.1 Trigger model

The platform must support typed, versioned and observable triggers for:

- record created, updated, deleted or restored;
- selected field/value changes;
- lifecycle or pipeline stage entry/exit;
- governed domain events;
- inbound verified webhook events;
- scheduled date/time and recurring schedules;
- relative timers, inactivity and SLA thresholds;
- message delivery/open/reply/bounce events where providers expose them;
- assignment, ownership or queue changes;
- analytics/score/health threshold crossings through governed derived events;
- manual user start;
- API/integration start;
- bulk/import completion and reconciliation outcomes.

Trigger evaluation must define ordering, deduplication, re-entry, loop prevention, tenant quotas and replay behavior.

### 7.2 Conditions and decision logic

Supported decision primitives must include:

- typed comparisons and null/empty semantics;
- related-resource predicates through authorized queries;
- AND/OR/NOT groups;
- deterministic expression functions;
- business-calendar calculations;
- changed-from/changed-to predicates;
- actor/team/territory/permission conditions;
- consent, privacy and data-class conditions;
- feature/module activation conditions;
- bounded decision tables;
- explainable rule outcomes.

Arbitrary tenant code is not a substitute for a typed condition model.

### 7.3 Robot/action catalog

Automation actions must be selected from allowlisted governed actions, including:

- create or update a domain resource through its owner capability;
- move a record through an allowed lifecycle or pipeline transition;
- assign/reassign to user, team, queue, territory or routing policy;
- create task, appointment, reminder, checklist or human work item;
- send notification;
- send consent-authorized email/SMS/message through Communications;
- start, pause, resume or cancel a sales cadence/journey where allowed;
- request serial/parallel approval;
- generate a document or e-signature envelope;
- invoke an allowlisted connector action;
- publish a governed integration event;
- wait until time, event, condition or human completion;
- branch, parallelize, join or call a reusable subflow;
- invoke a governed AI tool with policy and approval controls;
- compensate or reverse a prior action only when the owner exposes a supported reversal.

A “robot” is therefore a configured orchestration using governed capabilities, not an unrestricted script with database access.

### 7.4 Execution semantics

The runtime must provide:

- immutable definition versions;
- explicit draft, test, published, suspended and retired states;
- exact binding to action/capability versions;
- durable execution instances and step history;
- retries with bounded backoff;
- idempotency and duplicate-trigger suppression;
- leases, crash recovery and replay;
- waits and timers that survive deployment/restart;
- parallel branches and deterministic joins;
- bounded loops/iteration with quota controls;
- cancellation and timeout;
- compensation strategy where supported;
- dead-letter and operator recovery queues;
- version-aware migration policy for running instances;
- complete actor, reason, input digest, result and cost evidence;
- tenant/workflow/action concurrency and rate limits.

### 7.5 Visual workflow studio

Admins must receive a governed low-code authoring experience with:

- trigger/action node palette;
- drag-and-drop graph editor;
- typed configuration forms;
- branch and decision-table editors;
- reusable subflows/templates;
- validation before save and publish;
- dependency and permission impact analysis;
- simulation using representative/synthetic data;
- dry-run mode with no side effects;
- step-through testing and captured traces;
- version diff, clone and rollback;
- sandbox-to-production promotion;
- run explorer, failure diagnostics and replay controls;
- usage, latency, error and cost dashboards;
- localized descriptions and accessibility.

### 7.6 Programmable extension boundary

Some tenants and partners will need custom logic. The supported extension model must be explicit:

- typed custom functions compiled or sandboxed under resource limits;
- no raw database connectivity;
- no unrestricted network or secret access;
- declared input/output schemas;
- declared data classes and permissions;
- deterministic timeout, memory and CPU quotas;
- signed package/version identity;
- test fixtures and contract compatibility;
- complete execution audit;
- kill switch and revocation.

The preferred long-term untrusted execution boundary remains the signed marketplace sandbox planned for Phase 10. Critical owner invariants must never depend on arbitrary customer script behavior.

## 8. Pipelines, funnels, Kanban and configurable processes

Pipelines are first-class product configuration, not hard-coded deal columns.

Required coverage:

- multiple pipelines per supported owner/resource type;
- versioned stage definitions and ordering;
- stage entry/exit criteria;
- required fields and checklists;
- allowed transitions and role restrictions;
- stage probability, target duration and SLA;
- automatic transitions through workflow actions where policy allows;
- manual transition with validation and conflict handling;
- reasons for skip, regression, loss, cancellation or reopening;
- pipeline templates and controlled cloning;
- migration of active records when a pipeline version changes;
- cross-pipeline transfer rules;
- stage history and duration analytics;
- funnel conversion and leakage analytics.

Kanban/board experience must include:

- drag-and-drop with server-authoritative transition validation;
- optimistic UX only with explicit rollback on conflict;
- customizable cards and fields;
- grouping and swimlanes;
- filters, saved views and sharing;
- sorting, search and quick actions;
- aggregate totals and weighted values;
- WIP/staleness/SLA indicators;
- bulk move with preview and partial-failure results;
- permissions and field masking on every card;
- responsive and keyboard-accessible operation;
- large-pipeline virtualization and stable pagination;
- real-time or bounded-freshness updates.

The same process state should also be usable through list, table, calendar, timeline, workload and reporting views.

## 9. Sales cadences, sequences, playbooks and guided work

The product must support repeatable seller/customer-success/service outreach without conflating it with general-purpose workflow.

Required coverage:

- enroll/unenroll individuals or segments;
- manual, rule-based and workflow-based enrollment;
- email, call, task, meeting, social/manual and wait steps;
- conditional branches based on activity, reply, field, score or stage;
- automatic stop on reply, opt-out, conversion, disqualification or policy condition;
- quiet hours, locale/time zone and frequency caps;
- consent and suppression enforcement;
- templates, snippets, scripts and personalization;
- owner/team work queues and next-best task ordering;
- overdue handling, reassignment and pause/resume;
- A/B variants and performance analytics;
- cadence versioning and in-flight target policy;
- reusable sales/service/success playbooks;
- guided stage checklists and recommended next actions;
- AI-assisted drafting/recommendation only through governed policy.

## 10. Phase 8D — expanded sales and revenue operations

Required waves include:

- leads/prospects, qualification and conversion lineage;
- richer opportunities/deals and configurable sales processes;
- account/contact/opportunity roles and buying groups;
- routing, assignment, queues, workload balancing and territories;
- teams, overlays and collaboration;
- quotas, targets and period management;
- forecasting, categories, rollups and scenarios;
- pipeline inspection, risk signals and coaching;
- win/loss taxonomies and sales-cycle analytics;
- renewals, expansion and cross-sell coordination;
- partner-sourced/influenced attribution;
- product/quote context through Phase 8B references;
- mobile and offline field-selling journeys where required.

## 11. Phase 8E — omnichannel service, knowledge and field service

Required waves include:

- cases/tickets with configurable lifecycle, priority and major-incident relationships;
- skills, queues, assignment and capacity-aware routing;
- SLAs, milestones, business calendars and escalations;
- customer/order/contract/product/asset context;
- email, messaging, SMS, telephony, web chat and bot handoff;
- unified conversation and participant resolution;
- internal notes versus customer-visible replies;
- templates, macros and guided resolution;
- knowledge authoring, review, localization, publication and feedback;
- self-service portal/API and deflection analytics;
- CSAT/NPS/CES or configurable feedback boundaries;
- field-service work orders, dispatch, territories, appointments and technician mobile/offline workflows;
- parts/inventory integration rather than hidden inventory ownership.

## 12. Phase 8F — marketing automation and lifecycle journeys

Required waves include:

- campaigns and hierarchy;
- static and dynamic segments;
- lists and suppression lists;
- forms, landing-page and event ingestion boundaries;
- scoring and qualification models;
- multi-step journeys with trigger, branch, wait, goal and exit semantics;
- email/SMS/messaging activation through consent-aware Communications;
- experiments and holdout groups;
- frequency caps, quiet hours and deliverability policy;
- campaign-member and touchpoint lineage;
- attribution, funnel, cohort and conversion analytics;
- webinar/event integration;
- account-based marketing and buying groups;
- loyalty/referral as optional governed modules.

Marketing journeys may reuse the general automation runtime but retain separate audience, consent, experimentation and communication semantics.

## 13. Phase 8G — customer success, partners, projects and documents

### Customer Success

- onboarding plans and lifecycle stages;
- success plans, objectives and milestones;
- explainable health scores;
- product adoption/usage signals;
- risks, alerts and playbooks;
- business reviews and stakeholder maps;
- renewal, expansion and churn coordination.

### Partner relationship management

- partner organizations/contacts;
- tiers, programs, certification and eligibility;
- deal registration and conflict resolution;
- lead/opportunity distribution;
- incentives/rebate integration boundaries;
- delegated partner portal access;
- partner performance analytics.

### Projects and configurable work

- projects, workstreams, milestones and tasks;
- dependencies, checklists, templates and recurring work;
- risks, issues and decisions;
- Kanban/list/calendar/timeline views;
- customer-facing visibility policy;
- portfolio and delivery analytics;
- configurable operational case types built on governed metadata/process primitives.

### Documents and e-signature

- secure files and malware scanning;
- versioned documents and relationships;
- template-based generation and authorized merge fields;
- e-signature envelope/signer/status integration;
- immutable signed evidence;
- privacy, retention and legal-hold interaction;
- rebuildable preview, OCR and search.

## 14. Phase 8H — analytics, data platform, administration and product maturity

### Analytics and reporting

- operational dashboards and KPIs;
- permission-aware report builder;
- semantic fields and reusable metrics;
- drill-down to live-authorized records;
- funnel, cohort, retention, forecast, SLA and performance analytics;
- scheduled delivery with authorization re-check;
- lineage, freshness and reproducibility;
- governed export and warehouse/BI integration;
- no analytical projection becoming a hidden system of record.

### Data and integration platform

- versioned bulk import/export;
- public API and webhook platform;
- verified inbound webhooks and replay protection;
- reliable outbound delivery, retry, reconciliation and dead-letter handling;
- connector credentials through secret handles;
- mapping/transformation with versioned schemas;
- sync cursors, conflict strategy and source-of-truth declarations;
- ERP, finance, payment, tax, identity, telephony, messaging, ad and data-provider adapters;
- tenant quotas and rate limits.

### Admin Studio and low-code

- custom objects and typed custom fields;
- layouts, sections, related lists and conditional visibility;
- pipelines, stages, forms and saved views;
- validation and calculation rules;
- permissions, roles, teams and templates;
- workflow/process authoring;
- module activation and lifecycle;
- tenant settings, branding and localization;
- audit/diagnostics;
- sandbox, package and controlled promotion;
- impact analysis, publication and rollback.

### Product maturity

- responsive desktop/mobile web;
- installable/offline-capable journeys where required;
- accessibility across critical flows;
- localization and locale-aware values;
- fast records, lists, boards and search;
- empty/loading/error/offline/retry states;
- onboarding, contextual help and templates;
- notifications and deep links;
- command palette, keyboard shortcuts, recent items and favorites.

## 15. Phase 9 — AI-native CRM

AI is an authenticated, authorized and audited actor, not an alternate application runtime.

Required waves include:

- tenant/purpose/data-class/residency/cost-aware model routing;
- permission-filtered retrieval and grounding;
- summarization, drafting, classification and extraction;
- next-best-action, risk and opportunity recommendations with explanations;
- conversational analytics and knowledge assistance;
- governed tool schemas generated from capability/query registries;
- human approval for configured high-risk actions;
- prompt-injection/data-leakage/hallucination/tool-correctness evaluations;
- budget, latency and provider-failure controls;
- complete actor/model/prompt-policy/tool/cost audit evidence;
- reversible action handling where the domain supports reversal.

## 16. Phase 10 — marketplace and programmable ecosystem

Required waves include:

- signed packages and publisher identity;
- dependency/compatibility resolution;
- explicit data, capability, UI, network and secret grants;
- sandboxed untrusted execution;
- resource quotas and timeouts;
- install, upgrade, rollback, suspend and uninstall evidence;
- emergency kill switch and revocation;
- bounded UI extensions using host-owned context;
- connector and workflow action packages;
- vertical packages using the same owner/security/audit rules;
- extension failure isolation and no infrastructure bypass.

## 17. Phase 11 — enterprise and vertical proof

Product 10/10 requires executable enterprise evidence:

- OIDC/SAML and SCIM where required;
- resource/field-level authorization, masking and separation of duties;
- tenant key hierarchy, encryption and residency controls;
- immutable audit/WORM export;
- backup, PITR, tenant restore and disaster recovery;
- defined RPO/RTO and tested runbooks;
- SLOs, alerting, capacity and incident response;
- security scans, SBOM, provenance, penetration and abuse testing;
- upgrade, rollback and data migration safety;
- high-volume and large-tenant performance profiles;
- deployment topology and operational support policy;
- representative vertical packages proving governed extensibility.

## 18. Cross-wave product acceptance contract

Every product wave must define and prove:

1. authoritative owner and mutable aggregates;
2. public/internal capabilities, queries, events and workers;
3. exact tenant activation and authorization behavior;
4. persistence, migrations, rollback and reapply;
5. idempotency, audit, outbox and replay where applicable;
6. privacy, consent, retention and deletion interaction;
7. product UX for list/record/workspace/configuration surfaces;
8. keyboard/accessibility and browser acceptance;
9. import/migration and demo/seed path;
10. observability, SLO, recovery and failure-mode evidence;
11. documentation, onboarding and administrator diagnostics;
12. bounded extension cost without unrelated runtime edits or CI fan-out.

## 19. Automation-specific quality targets

Before automation may be called production-complete, accepted evidence must include explicit targets for:

- event-to-start latency and timer accuracy;
- durable wait/restart behavior;
- at-least-once delivery with idempotent effects or a stronger documented guarantee;
- duplicate-trigger suppression;
- retry/backoff and terminal failure handling;
- dead-letter recovery time;
- definition publication and rollback safety;
- in-flight instance version policy;
- maximum graph size, branch/loop limits and tenant quotas;
- simulation fidelity and side-effect isolation;
- run trace completeness and searchable diagnostics;
- connector timeout/rate-limit/failure behavior;
- authorization and consent re-check immediately before side effects;
- no workflow path capable of bypassing owner invariants;
- load tests for scheduled bursts and event storms.

Exact numeric thresholds are set in the delivery packet using the target deployment profile; absence of measurable thresholds blocks completion.

## 20. Kanban/pipeline quality targets

Before pipeline/Kanban may be called production-complete, accepted evidence must include:

- transition validation and conflict behavior;
- stage-history correctness;
- large-board performance and stable pagination;
- keyboard and screen-reader operation;
- drag/drop rollback on rejected transition;
- permission and masking correctness on cards;
- bulk-move preview and partial-failure reporting;
- bounded-freshness updates across concurrent users;
- pipeline-definition version migration;
- funnel/stage-duration metric reproducibility.

## 21. Product portfolio accounting

Every capability family and wave must be classified as:

- **Production-complete** — full domain/runtime/UX/operations evidence;
- **Platform-ready** — reusable infrastructure exists but product breadth is incomplete;
- **In progress** — active accepted sequence with remaining exit work;
- **Planned** — explicit wave, owner/boundary and dependency exist;
- **Optional/vertical** — intentionally installable rather than universal;
- **External integration** — CRM owns orchestration/reference/reconciliation, not the external system of record.

The current count of product-complete expert modules remains **0**. This plan broadens and sequences future delivery; it does not claim implementation.

## 22. Immediate continuation

This planning document does not start later product work.

The only next permitted implementation packet is:

> Repository Step 20 — Phase 8A frontend, accessibility, browser and operations evidence.

After Steps 20–21 complete Phase 8A and Step 22 resolves architecture/runtime/gate decisions, Step 23 begins the first measured product-extension wave. No Phase 8B–11 implementation may bypass that order.

## Repository Step 19 accepted closure

Repository Step 19 is complete only through the combined accepted evidence below, each on one unchanged exact source head with no unresolved comments, reviews or review threads:

- PR #287 / source `23b2f4ea660bcd46884fe054cd0c37e89b1495c4` / squash merge `c0fec3ae08c836ab483737442ed4377c99c85e9a` / **11 of 11** applicable permanent workflows — added the bounded Customer Privacy owner-worker boundary without public ingress or new schema/dependency surface;
- PR #288 / source `b99a4fc4ff7ef5cfd47813b487900b4c2a9f3b77` / squash merge `bc653de5f1a853791d3ab4a03f59f3daad54bf54` / **24 of 24** — added PostgreSQL ready-work discovery for planned Customer Privacy owner actions;
- PR #289 / source `3e21e79e1600727ebcda222af389d568d857cff8` / squash merge `d1c4dd278853a1e6a426fab284c70b3529d42833` / **24 of 24** — registered `crm.customer-privacy` / `owner-execution` at phase `260` in the production `ApplicationRuntime`, with activation gating and replay-safe canonical execution;
- PR #290 / source `9bbb339f39133955a7f42ea67f3334e597066e2e` / squash merge `49c5e35814adceb2be9d4cc2302bf10032b807a0` / **19 of 19** — proved the assembled real `crm-api` lifecycle on clean and rollback/reapplied PostgreSQL schemas: ready-work discovery, a real Parties privacy action, one durable attempt, successful outcome, completed checkpoint, audit evidence, owner event/outbox and final case transition, plus restart no-duplicate proof and uninstall no-discovery/no-effect proof.

The accepted first-party background-worker inventory is now **one Customer Privacy owner worker**: module `crm.customer-privacy`, worker `owner-execution`, phase `260`. This is a production background worker, not a new public capability route; the latest public Customer Privacy inventory remains **seven mutations and four permission-aware public queries**.

Repository Steps 1–19 are complete. Repository Step 20 — Phase 8A frontend, accessibility, browser and operations evidence — is the only next permitted implementation packet. Phase 8A.11 / issue #126 remains in progress; Customer Privacy is not product-complete; current product-complete expert modules remain zero; architecture 10/10 and the Universal CRM product are not declared complete.

The Step 19 packets add no crate, dependency, route, public API, module manifest, migration or schema. The conservative public Rust surface remains **5,377**, suppression occurrences remain **91**, and `crm-application-runtime` non-comment/source LOC remains within the frozen **7,269** ceiling.
