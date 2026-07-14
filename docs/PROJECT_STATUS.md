# Ultimate CRM — Project Status

Status date: 2026-07-14

This document is the concise human-readable status page. The normative sequence remains `IMPLEMENTATION_ROADMAP.md`; functional scope completeness is guarded by `CRM_CAPABILITY_COVERAGE.md`; Phase 8 packet sequencing is detailed in `PHASE8_DELIVERY_PLAN.md`; absolute rules remain `SYSTEM_INVARIANTS.md`; implementation grouping follows `DEVELOPMENT_WORKFLOW.md`; multi-agent execution follows `MULTI_AGENT_DEVELOPMENT.md`.

## Current position

**Phases 0.1–7 are complete. Phase 8A is the active expert owner-domain program: canonical customer master, identity, consent and governed customer-data lifecycle (#28).**

Current Phase 8A execution state:

- **8A.1 — complete:** canonical Party, Account and Contact Point references plus owner-domain foundations (#92 / merged PR #93);
- **8A.2a — complete:** authoritative Party create/get (#94 / merged PR #95);
- **8A.2b — complete:** optimistic Party update and permission-aware cursor listing (#96 / merged PR #97);
- **8A.2c — complete:** rebuildable permission-aware Party search/customer discovery (#98 / merged PR #99);
- **8A.3a — complete:** authoritative Account lifecycle and Party associations (#101 / merged PR #102);
- **8A.3b — complete:** authoritative Contact Point lifecycle, verification and preference (#103 / merged PR #104; merge commit `00f41b4bf2bf11dc4a5bb62d9cc1b46c6ad88fd8`);
- **8A.3c — complete:** authoritative Party Relationship lifecycle and rebuildable hierarchy foundations (#108 / merged PR #109; merge commit `36c238d51a156e3864e2dad0f53762e95e47680d`);
- **8A.3d — complete:** permission-aware rebuildable Customer 360 composition (#110 / merged PR #111; final verified head `b3bca41c393577e2da5a84bcbe0309996fbdef90`; merge commit `30ce84c57064134202c03c07a943bcd0859e1ea9`);
- **8A.4 — complete:** authoritative Consent and Communication Authorization (#112 / merged PR #113; final verified head `9e9f86bea82581f3e3d0ff8b6027d3b39e84cfcc`; merge commit `381a9fd5e6eb54918fc43801062957ca4a854486`);
- **8A.5 — complete:** authoritative Identity Resolution duplicate-candidate cases and reviewer decisions without Party merge (#114 / merged PR #115; final verified head `74ab48427b9dd5a30f0a0637cc52e74bd395b3c7`; merge commit `bdefcbd85496d9a0481b57d04ef1d6c731a12683`);
- **8A.6 — gate review:** approval-required non-destructive merge/unmerge, immutable lineage, survivorship provenance and canonical Party resolution (#116 / PR #119);
- **later 8A packets:** import/export, data quality/enrichment provenance and privacy lifecycle proof.

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
- canonical Contact Point owner with typed endpoint normalization, verification/preference lifecycle and production Party-reference integrity;
- authoritative Party Relationship owner with typed directional/reciprocal semantics, temporal lifecycle and a rebuildable cycle-safe hierarchy projection;
- independently governed read-only `crm.customer360` composition with deterministic multi-owner contributions, live source authorization/redaction, indexed Party-root lookup and rebuildable freshness-aware views;
- authoritative `crm.identity-resolution` duplicate-candidate cases plus immutable merge/unmerge lineage, approval-required reversible canonical Party redirection, exact Party-version integrity, field-level survivorship provenance, authoritative Party access paths and signed permission-aware duplicate/merge/canonical-resolution queries without destructive Party deletion or historical-reference rewriting;
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

### 8A.3b — Contact Point lifecycle, verification and preference — Complete

Issue #103 and draft PR #104 now contain the implemented authoritative Contact Point vertical slice:

- typed Contact Point identity and stable Party reference;
- deterministic Email, Phone, Postal, Web and Messaging normalization/validation;
- Active/Inactive lifecycle, preferred state, validity intervals and explicit verification evidence/time;
- verification preservation for non-value changes and reset only when the canonical endpoint value changes;
- exact optimistic concurrency, monotonic mutation time and deterministic persisted-state validation;
- additive v1 create/update/verify/get/list contracts plus created/updated/verified events;
- governed transactional mutations with idempotency, outbox and audit evidence;
- permission-aware get/list with typed filters, signed cursor binding and live field/resource visibility;
- application-level Party-reference integrity with the same safe result for missing and cross-tenant Parties and no direct cross-owner storage access from the owner module;
- immutable manifest/contract bindings and synchronized generated descriptors;
- application-runtime composition and field-bounded visibility bootstrap;
- fresh-PostgreSQL real `crm-api` process acceptance for lifecycle, verification/reset, replay/conflict, filters, signed cursor pagination/tamper rejection, unauthenticated rejection, tenant non-disclosure and durable evidence counts.

The implementation packet is **Complete**: all 11 applicable CI workflows were green together before merge, and PR #104 merged to `main` as `00f41b4bf2bf11dc4a5bb62d9cc1b46c6ad88fd8`.

Consent and communication authorization, provider delivery state, Party Relationship and Customer 360 remain separate owner concerns.

### 8A.3c — Party Relationship lifecycle and hierarchy foundations — Complete

#108 / merged PR #109 delivered:

- a dedicated pure `crm.party-relationships` owner with immutable Party Relationship identity and immutable canonical Party endpoints;
- explicit directional and reciprocal semantics, deterministic reciprocal endpoint ordering and reserved built-in semantic definitions;
- Active/Inactive lifecycle, half-open validity intervals, exact optimistic versioning, monotonic mutation time, semantic no-op rejection and atomic overflow safety;
- strict deterministic versioned persisted state and canonical rehydration validation;
- additive v1 create/update/get/list contracts plus created/updated events and shared canonical Party references;
- governed transactional create/update with idempotency, outbox and audit evidence;
- application-level same-tenant validation for both Party references with identical safe missing/cross-tenant rejection while real datastore failures remain distinguishable;
- permission-aware get/list with typed filters, signed cursor binding, bounded visibility scans and live field/resource authorization;
- application-runtime catalog/router composition and field-bounded visibility bootstrap;
- a rebuildable non-authoritative adjacency projection with effective-time filtering and deterministic cycle-safe bounded traversal;
- immutable PostgreSQL registry fixtures and synchronized Rust/browser descriptor identities;
- fresh-PostgreSQL real `crm-api` process acceptance covering prerequisite Parties, safe reference rejection with zero relationship side effects, create/replay/conflicting replay, duplicate aggregate-id rejection, directional and reciprocal semantics, reciprocal canonicalization, lifecycle/validity update and replay, stale/no-op rejection, typed filters, signed cursor pagination/tamper rejection, unauthenticated and cross-tenant non-disclosure, exact durable evidence deltas and projection run/rebuild equivalence.

The final merge gate completed and PR #109 merged to `main` as `36c238d51a156e3864e2dad0f53762e95e47680d`. The owner module remains free of SQL, transport types and direct cross-owner storage access.

### 8A.3d — permission-aware rebuildable Customer 360 composition — Complete / merge gate

#110 / draft PR #111 now delivers:

- an independently governed read-only `crm.customer360` composition module with no owned mutable customer records or private authoritative state;
- additive `crm.customer_360.v1` typed query contracts and canonical `customer360.customer.get@1.0.0` capability binding;
- strict validation of immutable Party, Account, Contact Point and Party Relationship owner events before projection use;
- deterministic one-current-contribution-per-source projection documents with canonical Party-root membership, preventing stale Account association fan-out;
- indexed tenant-scoped Party-root lookup, repeatable-read projection/checkpoint snapshots and bounded failure instead of silent partial Customer 360 results;
- live per-source resource authorization and least-privilege field redaction across Party, Account, Contact Point and Party Relationship sections;
- explicit source owner/resource/version lineage and projection checkpoint freshness metadata;
- production application-runtime query routing, background projection convergence and a public read-only module manifest with no mutation capability;
- fresh-PostgreSQL real `crm-api` process acceptance proving owner-event convergence, Contact Point verify → canonical value change → verification reset, Account root-membership removal, Party Relationship lifecycle/validity updates, authentication and tenant non-disclosure, field redaction, source-version/freshness progression, rebuild equality and unchanged authoritative record/outbox/audit evidence across rebuild;
- synchronized Rust/browser descriptors and indexed migration/rollback coverage.

All 11 applicable CI workflows were green together on pre-documentation exact head `1c3008b3dfc801867d8c62fcbb7b0370d87642ca`. This documentation commit intentionally invalidates that evidence until the post-documentation exact-head rerun is green. Customer 360 remains rebuildable and non-authoritative; Party, Account, Contact Point and Party Relationship remain the sole owners of their mutable state.

### Remaining 8A sequence

After merged Consent and Phase 8A.5 duplicate-candidate delivery:

1. complete the post-documentation exact-head gate for Phase 8A.6 and move PR #119 to ready-for-review;
2. merge Phase 8A.6 only as a separate explicit action after review;
3. deliver import/export, data quality, enrichment provenance and privacy lifecycle proof in later explicit packets.

## Product readiness summary

### Business modules

The repository currently tracks **nine business modules**:

- `crm.sales` — production Deal vertical slice; broader expert Sales scope remains planned;
- `crm.activities` — production Task vertical slice; broader calendar/productivity scope remains planned;
- `crm.parties` — canonical identity owner in expert expansion with production create/update/get/list/search;
- `crm.customer-accounts` — merged authoritative Account production vertical slice;
- `crm.contact-points` — merged authoritative Contact Point production vertical slice;
- `crm.party-relationships` — merged authoritative Party Relationship production vertical slice;
- `crm.consents` — merged authoritative Consent and Communication Authorization vertical slice;
- `crm.identity-resolution` — merged duplicate-candidate/reviewer-decision owner plus Phase 8A.6 merge/unmerge lineage, provenance and canonical-resolution implementation in gate review;
- `crm.sales-activities-link` — optional independently governed production integration slice.

The repository also contains the independently governed read-only composition module `crm.customer360`. It is not counted as an authoritative owner module because it owns no customer aggregate or mutable source state; it provides a rebuildable permission-aware Customer 360 query surface over owner contracts.

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

1. Complete the unchanged exact-head all-workflow gate for Phase 8A.5 / #114 / PR #115 and merge only from that verified SHA.
2. Deliver Phase 8A.6 governed Party merge/unmerge with immutable lineage, provenance and survivorship.
3. Continue later 8A packets for import/export, data quality, enrichment provenance and privacy lifecycle proof.
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
