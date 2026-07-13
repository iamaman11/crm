# Ultimate CRM — Project Status

Status date: 2026-07-13

This document is the concise human-readable status page. The normative sequence remains `IMPLEMENTATION_ROADMAP.md`; functional scope completeness is guarded by `CRM_CAPABILITY_COVERAGE.md`; Phase 8 packet sequencing is detailed in `PHASE8_DELIVERY_PLAN.md`; absolute rules remain `SYSTEM_INVARIANTS.md`; implementation grouping follows `DEVELOPMENT_WORKFLOW.md`; multi-agent execution follows `MULTI_AGENT_DEVELOPMENT.md`.

## Current position

**Phases 0.1–7 are complete. Phase 8A is the active expert owner-domain program: canonical customer master, identity, consent and governed customer-data lifecycle (#28).**

Current Phase 8A execution state:

- **8A.1 — complete:** canonical Party, Account and Contact Point references plus owner-domain foundations (#92 / merged PR #93);
- **8A.2a — complete:** production Party create/get through governed mutation/query paths with PostgreSQL, Personal data classification, live authorization, idempotency, outbox and audit evidence (#94 / merged PR #95);
- **8A.2b — active:** optimistic Party update and permission-aware cursor listing (#96 / draft PR #97);
- **8A.2c — planned next:** rebuildable permission-aware Party search/customer discovery projection (#98);
- **8A.3 and later:** Account, Contact Point, Party Relationship, Consent/Preferences, identity resolution, merge/unmerge, provenance, import/export, data quality and privacy lifecycle proof.

The repository now contains a production-composed modular CRM platform foundation plus the first expert customer-master owner-domain expansion:

- executable repository governance and architecture boundaries;
- typed Module Manifest IR and immutable module identity;
- governed Module SDK and deterministic test harness;
- module publication, installation and lifecycle runtime;
- PostgreSQL tenant/RLS, record, relationship, idempotency, outbox and append-only audit foundation;
- authenticated mutation and permission-bound query gateways;
- independent Sales Deal and Activities Task production vertical slices;
- canonical Party identity owner with merged create/get and active update/list expert expansion;
- governed event delivery and the optional `crm.sales-activities-link` module;
- generalized rebuildable projections and tenant/permission-aware search;
- real `crm-application-runtime` composition boundary and deployable `services/crm-api` process host;
- typed web product shell with governed generated browser clients and real browser E2E;
- immutable tenant-authorized metadata publication lifecycle;
- strict typed Admin Studio metadata schemas and canonical validation;
- durable tenant-scoped metadata revision/activation persistence;
- governed public metadata mutation/query contracts with canonical global audit evidence;
- governed Admin Studio authoring → publish → impact → activate → rollback workflow;
- typed trusted-code UI-extension runtime with per-extension load/render failure isolation.

The product is **not yet a complete universal CRM**. `CRM_CAPABILITY_COVERAGE.md` now makes the full target explicit so the program cannot become infrastructure-complete but CRM-incomplete.

## Functional scope completeness baseline

The normative capability baseline now explicitly covers:

- Customer 360, Party/Account/Contact Point master data, relationships, consent, identity resolution, data quality and privacy lifecycle;
- Sales force automation including leads, pipelines, activities, routing, territories, teams, quotas, forecasting, renewals, sequences and mobile/offline workflows;
- Product Catalog, Pricing, CPQ, Quotes, Orders, Contracts, Subscriptions, entitlements and governed billing/ERP/payment/tax boundaries;
- Communications and omnichannel conversation history;
- Service/Support, Knowledge and optional Field Service;
- Marketing automation, segmentation, journeys, attribution and account-based marketing;
- Customer Success, retention/expansion and optional Partner Relationship Management;
- Projects/configurable work, documents, files and e-signature;
- analytics, reporting, performance management and governed warehouse/BI boundaries;
- workflow/automation, approvals and human tasks;
- collaboration, productivity, search and knowledge discovery;
- API/webhooks, bulk import/export, connectors, synchronization and data enrichment;
- administration, customization and governed low-code metadata;
- enterprise identity, authorization, privacy, residency, backup/restore, security and operations;
- AI-native CRM through authenticated audited governed tools;
- signed marketplace and sandboxed untrusted extensions;
- responsive/mobile UX, accessibility, localization, offline/retry states and browser E2E;
- optional vertical packages through the same governed module model.

Capability families are classified as **Production-complete**, **Platform-ready**, **Planned**, **Optional/vertical** or **External integration**. No claim of broad or “ultimate” CRM completeness is valid while a required capability family has neither production implementation nor an explicit owner/boundary classification.

## Completed delivery foundations

### Phases 0.1–5 — platform control plane — Complete

Completed foundations include repository hardening, typed deterministic module manifests, the governed Module SDK, module lifecycle/registry, PostgreSQL tenant/RLS and audit foundations, and the capability execution gateway.

Public state-changing behavior enters through authenticated, tenant- and actor-bound, exact-version capabilities with live authorization, typed validation, atomic PostgreSQL execution, idempotency, outbox and audit evidence.

### Phase 6 — first modular production proof — Complete

PR #63 completed the first full backend vertical proof:

- typed independent Sales `Deal` and Activities `Task` owner aggregates;
- publication-compatible Protobuf contracts;
- authenticated production PostgreSQL mutations and permission-bound queries;
- durable event delivery with retry/recovery/dead-letter behavior;
- lifecycle-aware Sales-to-Activities link execution through the production `CapabilityGateway`;
- rebuildable Deal timeline and Task status projections;
- real application composition root, HTTP/gRPC ingress, health/readiness and graceful shutdown;
- process-level acceptance against real PostgreSQL and `crm-api`.

Final review head `25793548e46bdbd57312a513b4e9ffbceb33a2c1` passed Contract, Governance, Rust, Database, Event Runtime, Application Runtime and Rust Generated Sync before merge.

### Phase 7A — golden module tooling — Complete

#56 / merged PR #64 established repository-supported owner/link module scaffolding, overwrite-safe generation, dependency validation, architecture-safe crate/manifests, acceptance placeholders and permanent repository commands.

### Phase 7B — generalized projection runtime — Complete

#65 / merged PR #67 introduced `crm-projection-runtime`, deterministic projection registration/execution, poison/failure handling and rebuild orchestration without moving owner-domain semantics into infrastructure.

### Phase 7C — permission-aware search — Complete

#66 / merged PR #68 completed the production search foundation:

- search indexes are candidate-only and rebuildable;
- live resource and field visibility are re-checked before disclosure;
- logical search generations support deterministic rebuild/switching;
- PostgreSQL FTS remains a replaceable adapter;
- `search.global.query` is routed through the governed production `QueryGateway`;
- acceptance covers permission revocation, hidden-field non-disclosure, deterministic pagination and tenant isolation.

Final review head `90d8ad4afc15ba31bc27297e4a9c7081e64ac4e7` passed all applicable Contract, Governance, Rust, Database, Projection, Event, Search, Application Runtime and Rust Generated Sync gates.

### Phase 7D — typed web product shell — Complete

#71 / merged PR #73 established the governed product-plane foundation: strict TypeScript, generated Protobuf-ES clients, governed browser boundaries, typed session state, safe error mapping, responsive/accessibility foundations and hermetic Playwright E2E against ephemeral PostgreSQL.

### Phase 7E — immutable, tenant-authorized typed metadata — Complete

#77/#78, #79/#80 and #81/#82 established immutable metadata-bundle snapshots, deterministic revision identity, tenant-scoped publication authority, structural impact analysis, optimistic activation/rollback and strict typed definitions for objects, fields, relationships, layouts, saved views, pipelines, permission templates and workflows.

### Phase 7F — durable tenant-scoped metadata persistence — Complete

#83 / merged PR #84 added immutable tenant-scoped revision persistence, deterministic reconstruction, optimistic activation heads, per-tenant locking, durable rollback history, append-only transition evidence, FORCE RLS and PostgreSQL acceptance.

### Phase 7G — governed metadata API and application composition — Complete

#85 / merged PR #86 closed the public governed boundary over metadata runtime and persistence with exact versioned Protobuf contracts, `CapabilityGateway` mutations, `QueryGateway` reads, PostgreSQL adapters, global audit evidence and typed browser operations.

Final review head `7989ea1256f01bfd4e8ee2d33f5ad8370d6cc645` passed all 11 applicable workflows simultaneously. PR #86 merged to `main` as `970548d14faf26f4b8f6cb47f7d9f168e61d9c28`.

### Phase 7H — first governed Admin Studio workflow — Complete

#87 / merged PR #88 delivered typed object-definition authoring, immutable candidate publication, impact review, breaking-change confirmation, optimistic activation/rollback and real browser E2E against fresh PostgreSQL and `crm-api`.

### Phase 7I — typed UI-extension runtime and host failure isolation — Complete

#89 / merged PR #90 delivered exact typed extension surfaces, immutable validated registration, readonly bounded host context, per-extension lazy-load/render isolation, safe failure evidence and browser proof that the shell, core record content and healthy siblings survive extension failures.

Final review head `874dde11f5d558bd5e53f2def3e8903ff12f361a` passed Governance CI, Rust CI and Product Plane CI including full PostgreSQL/process/browser E2E. PR #90 merged to `main` as `0fb389c72b148311f590c3fdbae2a4f89fffd915`.

Phase 10 remains responsible for signed packages and sandboxed untrusted marketplace execution. Phase 7I deliberately does not claim arbitrary third-party JavaScript isolation.

## Active executable program — Phase 8A customer master

### 8A.1 — identity/reference contracts and owner foundations — Complete

#92 / merged PR #93 established canonical typed references and owner-module foundations for:

- `crm.parties` — Person/Organization identity owner;
- `crm.customer-accounts` — customer/commercial relationship owner;
- `crm.contact-points` — email/phone/postal/messaging endpoint owner.

The shared `crm.customer.v1` package is reference/version metadata only and does not own mutable customer state.

### 8A.2a — authoritative Party create/get — Complete

#94 / merged PR #95 delivered:

- Person and Organization Party aggregate;
- governed create through `CapabilityGateway`;
- permission-aware get through `QueryGateway`;
- Personal data classification;
- tenant isolation;
- idempotency, durable outbox event and canonical audit evidence;
- real PostgreSQL and process-level `crm-api` acceptance.

### 8A.2b — optimistic Party update and permission-aware list — Active

#96 / draft PR #97 is delivering:

- immutable Party identity/kind with optimistic display-name updates;
- exact expected-version conflict handling and deterministic version progression;
- governed `parties.party.update@1.0.0` with idempotency, event and audit evidence;
- deterministic signed cursor listing bound to tenant, actor, capability, filter, sort and page size;
- optional typed Party-kind filter;
- live per-resource and per-field visibility enforcement;
- process-level PostgreSQL acceptance covering create, get, update, replay, conflicting replay, stale version, pagination, kind filtering, unauthenticated access, cross-tenant non-disclosure and exact evidence counts;
- synchronized generated Rust/browser contract descriptors.

This packet is not considered complete until all applicable CI gates are green together on one exact final head and the PR is merged.

### 8A.2c — Party search and customer discovery — Planned next

#98 will integrate Party into the existing rebuildable generation-based search architecture while preserving these rules:

- Party remains the authoritative identity owner;
- search is candidate-only and rebuildable;
- live resource/field authorization is re-checked before disclosure;
- tenant isolation and deterministic generation switch/rebuild behavior are proven;
- create/update event replay remains idempotent.

8A.2 is complete only after 8A.2a, 8A.2b and 8A.2c are production-proven.

### 8A.3 and later customer-master sequence

After Party lifecycle/discovery is stable:

1. Account lifecycle and Party relationship semantics;
2. Contact Point lifecycle, verification and preference state;
3. Party Relationship types, validity intervals and hierarchy projections;
4. Customer 360 composition without a second identity owner;
5. Consent and communication authorization;
6. deterministic/explainable duplicate candidates and identity resolution;
7. governed merge/unmerge, provenance and survivorship;
8. import/export, data quality, enrichment provenance and privacy lifecycle proof.

## Product readiness summary

### Business modules

The repository currently tracks **six business modules**:

- `crm.sales` — production Deal vertical slice; broader expert Sales scope remains planned;
- `crm.activities` — production Task vertical slice; broader calendar/productivity scope remains planned;
- `crm.parties` — canonical identity owner in expert expansion; create/get merged, update/list active, search next;
- `crm.customer-accounts` — owner foundation, production Account lifecycle pending 8A.3;
- `crm.contact-points` — owner foundation, production Contact Point lifecycle pending 8A.3;
- `crm.sales-activities-link` — optional independently governed production integration slice.

Current product-complete expert module count: **0**. A production vertical slice is not the same as complete expert-domain functionality.

### Platform-ready foundations

- module lifecycle and governed capability/query execution;
- tenant/RLS authoritative data foundation and append-only audit;
- durable event delivery;
- rebuildable projections and permission-aware search;
- production application composition and deployable process host;
- typed web product shell and governed browser-client boundary;
- immutable typed metadata publication and persistence;
- governed metadata API and Admin Studio workflow;
- trusted-code UI-extension runtime with failure isolation.

### Major planned product programs

- remaining Customer Master / Identity / Consent work — active Phase 8A;
- Product Catalog, Pricing, CPQ and quote-to-revenue — Phase 8B / #29;
- Sales expert expansion;
- Communications and Omnichannel;
- Service, Support, Knowledge and Field Service;
- Marketing and Growth;
- Customer Success and optional PRM;
- Projects/configurable work, Documents and e-signature;
- Analytics, reporting and performance management;
- workflow, collaboration and end-user product completeness;
- AI-native governed actor/tool layer — Phase 9;
- signed marketplace/WASM sandbox — Phase 10;
- enterprise identity/privacy/restore/failover/security/SLO proof — Phase 11.

## Immediate delivery sequence

1. Finish Phase 8A.2b / #96 / PR #97 on one exact green head.
2. Deliver Phase 8A.2c / #98 Party search and customer discovery projection.
3. Deliver Phase 8A.3 Account, Contact Point, Party Relationship and Customer 360 foundations.
4. Continue 8A with Consent, Identity Resolution, merge/unmerge, provenance, import/export, data quality and privacy proof.
5. Continue Phase 8B / #29 commercial lifecycle without absorbing Catalog/Pricing/Order/Contract ownership into Sales.
6. Advance the remaining capability families from `CRM_CAPABILITY_COVERAGE.md` as explicit owner-domain or governed integration packets.
7. Continue enterprise/security/operational hardening continuously without premature production-completeness claims.

## Development system

The repository uses the exact-SHA multi-agent model from #70 / merged PR #72:

- one Architect / Implementer owns overlapping packet scope;
- a Local Integrator / Verifier may verify an exact immutable SHA or take explicitly delegated non-overlapping work;
- every verification claim names the exact SHA actually tested;
- a new commit invalidates prior evidence for checks not rerun;
- GitHub CI remains the final exact-head merge authority.

#74 / merged PR #75 adds capability-based Codex qualification. #76 remains an open process-hardening follow-up to make exact-SHA review freeze explicitly aware of source-changing automation.

## Development mode

- one branch per coherent delivery packet, not per mechanical edit;
- incremental commits are allowed during implementation;
- one primary writer at a time for overlapping multi-agent scope;
- exact-SHA local handoffs may be used at architecture, behavior and delivery checkpoints;
- qualified agents may own bounded integration fixes, non-overlapping workstreams or full delivery packets according to `CODEX_AGENT_QUALIFICATION.md`;
- full GitHub CI remains mandatory on the exact final review head;
- architecture, contract, tenant, authorization, audit and rollback gates remain strict.

See `DEVELOPMENT_WORKFLOW.md`, `MULTI_AGENT_DEVELOPMENT.md`, `CODEX_AGENT_QUALIFICATION.md` and `MODULE_DEVELOPMENT.md`.

## Documentation hygiene rule

When implementation state changes, update together where applicable:

- `IMPLEMENTATION_ROADMAP.md` — normative phase state and sequence;
- `PROJECT_STATUS.md` — concise current state;
- `MODULE_CATALOG.md` — module lifecycle/readiness state;
- `CRM_CAPABILITY_COVERAGE.md` — functional scope-completeness guardrail;
- parent/phase GitHub issues;
- README only for stable orientation, not detailed phase bookkeeping.

README must not become a second manually maintained roadmap.
