# Ultimate CRM — Project Status

Status date: 2026-07-13

This document is the concise human-readable status page. The normative sequence remains `IMPLEMENTATION_ROADMAP.md`; functional scope completeness is guarded by `CRM_CAPABILITY_COVERAGE.md`; Phase 8 packet sequencing is detailed in `PHASE8_DELIVERY_PLAN.md`; absolute rules remain `SYSTEM_INVARIANTS.md`; implementation grouping follows `DEVELOPMENT_WORKFLOW.md`; multi-agent execution follows `MULTI_AGENT_DEVELOPMENT.md`.

## Current position

**Phases 0.1–7 are complete. Phase 8A is the active expert owner-domain program: canonical customer master, identity, consent and governed customer-data lifecycle (#28).**

Current Phase 8A execution state:

- **8A.1 — complete:** canonical Party, Account and Contact Point references plus owner-domain foundations (#92 / merged PR #93);
- **8A.2a — complete:** authoritative Party create/get (#94 / merged PR #95);
- **8A.2b — complete:** optimistic Party update and permission-aware cursor listing (#96 / merged PR #97);
- **8A.2c — complete:** rebuildable permission-aware Party search/customer discovery (#98 / merged PR #99);
- **8A.3a — complete:** authoritative Account lifecycle and Party associations (#101 / merged PR #102);
- **8A.3b — active:** Contact Point lifecycle, verification and preference (#103; implementation branch `develop/phase8a3b-contact-point-lifecycle`);
- **8A.3c–8A.3d:** Party Relationship lifecycle/hierarchy and permission-aware Customer 360 composition;
- **8A.4 and later:** consent/preferences, identity resolution, merge/unmerge, provenance, import/export, data quality and privacy lifecycle proof.

The repository now contains a production-composed modular CRM platform foundation plus the first expert customer-master owner domains:

- executable repository governance and architecture boundaries;
- typed Module Manifest IR and immutable module identity;
- governed Module SDK and deterministic test harness;
- module publication, installation and lifecycle runtime;
- PostgreSQL tenant/RLS, record, relationship, idempotency, outbox and append-only audit foundation;
- authenticated mutation and permission-bound query gateways;
- independent Sales Deal and Activities Task production vertical slices;
- canonical Party identity owner with production create/update/get/list and permission-aware rebuildable search discovery;
- canonical Customer Account owner with production create/update/get/list, typed Party associations and cross-owner reference-integrity composition;
- governed event delivery and the optional `crm.sales-activities-link` module;
- generalized rebuildable projections and tenant/permission-aware search;
- neutral cross-domain global-search composition that owns projection mapping but no authoritative business state;
- real `crm-application-runtime` composition boundary and deployable `services/crm-api` process host;
- typed web product shell with governed generated browser clients and real browser E2E;
- immutable tenant-authorized metadata publication lifecycle;
- strict typed Admin Studio metadata schemas and canonical validation;
- durable tenant-scoped metadata revision/activation persistence;
- governed public metadata mutation/query contracts with canonical global audit evidence;
- governed Admin Studio authoring → publish → impact → activate → rollback workflow;
- typed trusted-code UI-extension runtime with per-extension load/render failure isolation.

The product is **not yet a complete universal CRM**. `CRM_CAPABILITY_COVERAGE.md` makes the full target explicit so the program cannot become infrastructure-complete but CRM-incomplete.

## Functional scope completeness baseline

The normative capability baseline explicitly covers:

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

PR #63 established independent typed Sales `Deal` and Activities `Task` owner aggregates, publication-compatible Protobuf contracts, production PostgreSQL mutations and permission-bound queries, authenticated HTTP/gRPC ingress, durable event delivery, the optional Sales–Activities link module, rebuildable projections, a real application composition root and deployable `crm-api` process with process-level acceptance.

### Phase 7 — search, metadata, Admin Studio and product plane — Complete

Phase 7 delivered:

- golden module tooling;
- generalized projection runtime;
- permission-aware candidate-only search with logical generations;
- typed web product shell and governed browser-client boundary;
- immutable tenant-authorized typed metadata;
- durable metadata persistence and optimistic activation/rollback;
- governed metadata API and application composition;
- the first governed Admin Studio publish/impact/activate/rollback workflow;
- typed trusted-code UI-extension runtime with per-extension load/render failure isolation.

Untrusted marketplace execution remains Phase 10 and is not confused with same-realm trusted UI extensions.

## Active executable program — Phase 8A customer master

### 8A.1 — identity/reference contracts and owner foundations — Complete

#92 / merged PR #93 established canonical typed references and owner-module foundations for:

- `crm.parties` — Person/Organization identity owner;
- `crm.customer-accounts` — customer/commercial relationship owner;
- `crm.contact-points` — email/phone/postal/messaging endpoint owner.

The shared `crm.customer.v1` package is reference/version metadata only and owns no mutable customer state.

### 8A.2 — complete Party lifecycle and discovery — Complete

The complete first Party lifecycle was deliberately split into three production packets:

- **8A.2a / #94 / PR #95:** governed create/get, Personal data classification, tenant isolation, idempotency, outbox and audit evidence;
- **8A.2b / #96 / PR #97:** optimistic update, signed cursor listing, live per-resource/per-field visibility and replay/conflict/non-disclosure proof;
- **8A.2c / #98 / PR #99:** neutral cross-domain search composition, Party create/update indexing from immutable owner events, generation migration to `g2`, live search authorization and process-level search convergence/non-disclosure proof.

Party remains the authoritative identity owner. Search remains rebuildable and non-authoritative.

### 8A.3a — authoritative Account lifecycle and Party associations — Complete

#101 / merged PR #102 delivered:

- immutable typed Account identity owned solely by `crm.customer-accounts`;
- normalized bounded Account name and explicit Active/Inactive lifecycle;
- stable Party references and typed Primary/Member roles without copying mutable Party identity data;
- exactly one primary Party association and deterministic duplicate-free association normalization;
- optimistic exact-version updates, monotonic governed mutation time and semantic no-op rejection;
- strict deterministic persisted Account state contract;
- additive v1 create/update/get/list and created/updated event contracts;
- immutable module manifest and exact contract bindings;
- governed transactional create/update with idempotency, optimistic persistence, outbox and audit evidence;
- permission-aware get/list with signed tenant/actor/filter/sort/page-bound cursors and field redaction;
- platform-level cross-owner Party-reference integrity validation before Account mutation execution;
- identical safe rejection for missing and cross-tenant Party references;
- synchronized Rust/browser contract descriptors;
- independent fresh-database process acceptance for the existing full application, Party lifecycle and Account lifecycle;
- Account process proof covering prerequisite governed Party creation, reference-integrity rejection without Account side effects, create/replay/get/update/update replay/stale conflict, pagination, status filter, unauthenticated rejection, cross-tenant non-disclosure and durable evidence deltas.

The Account owner module itself has no SQL dependency and does not read Party storage directly.

Final verified review head `0d6d79dce31aaea4d2a0998fadb1ac842fdcfde4` passed all 11 applicable workflows together. PR #102 merged to `main` as `7ee48530d880ef8aeb6abf2140b524ac724d4fc9`.

### 8A.3b — Contact Point lifecycle, verification and preference — Active

Issue #103 is active. The implementation branch `develop/phase8a3b-contact-point-lifecycle` is currently two commits ahead of `main` and contains the first owner-domain foundation:

- typed Contact Point identity and stable Party reference;
- Email, Phone, Postal, Web and Messaging kinds;
- Active/Inactive lifecycle;
- preferred flag and validity interval state;
- explicit Unverified/Verified state with bounded evidence reference and verification time;
- create/update/verify commands with exact expected-version progression;
- changing the endpoint value resets verification to Unverified;
- canonical value normalization and validation foundations;
- owner-module dependency additions required by that domain code.

This is **not yet a production-complete Contact Point slice**. Public Protobuf contracts, mutation/query adapters, persistence mapping, application composition, Party-reference integrity, generated browser contracts and real PostgreSQL/`crm-api` process acceptance still need to be delivered before 8A.3b can merge.

Consent and communication authorization remain separate owner concerns and will not be hidden inside Contact Point flags.

### Remaining 8A sequence

After Contact Point:

1. Party Relationship lifecycle and temporal hierarchy foundations;
2. permission-aware rebuildable Customer 360 composition without a second customer master;
3. Consent and communication authorization;
4. deterministic/explainable duplicate candidates and identity resolution;
5. governed merge/unmerge, provenance and survivorship;
6. import/export, data quality, enrichment provenance and privacy lifecycle proof.

## Product readiness summary

### Business modules

The repository currently tracks **six business modules**:

- `crm.sales` — production Deal vertical slice; broader expert Sales scope remains planned;
- `crm.activities` — production Task vertical slice; broader calendar/productivity scope remains planned;
- `crm.parties` — canonical identity owner in expert expansion with production create/update/get/list/search;
- `crm.customer-accounts` — merged authoritative Account production vertical slice;
- `crm.contact-points` — owner foundation with active 8A.3b domain implementation, not yet production-complete;
- `crm.sales-activities-link` — optional independently governed production integration slice.

Current product-complete expert module count: **0**. A production vertical slice is not the same as complete expert-domain functionality.

### Platform-ready foundations

- module lifecycle and governed capability/query execution;
- tenant/RLS authoritative data foundation and append-only audit;
- durable event delivery;
- rebuildable projections and permission-aware search;
- neutral cross-domain search composition over generic generation infrastructure;
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

1. Continue Phase 8A.3b / #103 from `develop/phase8a3b-contact-point-lifecycle` and complete contracts, persistence, governed mutation/query adapters, application composition and process-level production proof.
2. Deliver Party Relationship and Customer 360 composition packets.
3. Continue 8A with Consent, Identity Resolution, merge/unmerge, provenance, import/export, data quality and privacy proof.
4. Continue Phase 8B / #29 commercial lifecycle without absorbing Catalog/Pricing/Order/Contract ownership into Sales.
5. Advance the remaining capability families from `CRM_CAPABILITY_COVERAGE.md` as explicit owner-domain or governed integration packets.
6. Continue enterprise/security/operational hardening continuously without premature production-completeness claims.

## Development system

The repository uses the exact-SHA multi-agent model from #70 / merged PR #72:

- one Architect / Implementer owns overlapping packet scope;
- a Local Integrator / Verifier may verify an exact immutable SHA or take explicitly delegated non-overlapping work;
- every verification claim names the exact SHA actually tested;
- a new commit invalidates prior evidence for checks not rerun;
- GitHub CI remains the final exact-head merge authority.

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
