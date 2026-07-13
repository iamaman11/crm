# Ultimate CRM — Project Status

Status date: 2026-07-13

This document is the concise human-readable status page. The normative sequence remains `IMPLEMENTATION_ROADMAP.md`; absolute rules remain `SYSTEM_INVARIANTS.md`; implementation grouping follows `DEVELOPMENT_WORKFLOW.md`; multi-agent execution follows `MULTI_AGENT_DEVELOPMENT.md`.

## Current position

**Phase 6 and Phase 7 are complete. Phase 8A is now the active expert owner-domain program: canonical customer master, identity resolution and consent (#28).**

The repository now contains a complete first production-composed modular CRM proof plus production-quality search, product-plane, governed Admin Studio and trusted-code UI-extension foundations:

- executable repository governance and architecture boundaries;
- typed Module Manifest IR and immutable module identity;
- governed Module SDK and deterministic test harness;
- module publication, installation and lifecycle runtime;
- PostgreSQL tenant/RLS, record, relationship, idempotency, outbox and append-only audit foundation;
- authenticated mutation and permission-bound query gateways;
- independent Sales Deal and Activities Task owner-domain vertical slices;
- governed event delivery and the optional `crm.sales-activities-link` module;
- generalized rebuildable projections and tenant/permission-aware search;
- real `crm-application-runtime` composition boundary and deployable `services/crm-api` process host;
- typed web product shell with governed generated browser clients and real browser E2E;
- immutable tenant-authorized metadata publication lifecycle;
- strict typed Admin Studio metadata schemas and canonical validation;
- durable tenant-scoped metadata revision/activation persistence;
- governed public metadata mutation/query contracts with canonical global audit evidence;
- first governed Admin Studio authoring → publish → impact → activate → rollback workflow;
- typed trusted-code UI-extension runtime with per-extension load/render failure isolation and browser proof that shell, core record content and healthy siblings survive extension failures.

The breadth of end-user CRM functionality is still intentionally smaller than the target universal expert CRM. Customer master, commercial lifecycle, service, marketing, communications, analytics, AI, marketplace and enterprise operational proof remain explicit owner-domain/platform programs.

## Completed delivery foundations

### Phases 0.1–5 — platform control plane

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

#71 / merged PR #73 established the first governed product-plane foundation:

- reproducible Node 24 / pnpm 11 / strict TypeScript workspace;
- `apps/web`, `packages/client` and `packages/ui` product-plane boundaries;
- generated Protobuf-ES browser contracts;
- browser access through governed typed application boundaries;
- centralized typed session state and product-owned safe error mapping;
- permission-aware routing as UX only, with backend authorization authoritative;
- no public arbitrary raw gateway/query escape hatch;
- responsive/accessibility foundations;
- hermetic Playwright E2E against ephemeral PostgreSQL.

Final review head `b62dd50225fde6e58aac9a6b4cec307bd2245616` passed all applicable checks before merge.

### Phase 7E — immutable, tenant-authorized typed metadata — Complete

#77/#78, #79/#80 and #81/#82 established:

- complete immutable metadata-bundle snapshots;
- deterministic SHA-256 revision identity;
- tenant-scoped publication authority by construction;
- structural impact analysis and explicit breaking-change confirmation;
- optimistic activation generations and rollback;
- strict typed object, field, relationship, layout, saved-view, pipeline, permission-template and workflow definitions;
- canonical JSON and deterministic dependency extraction;
- exact governed-capability workflow actions with no raw script, SQL or arbitrary HTTP primitive.

### Phase 7F — durable tenant-scoped metadata persistence — Complete

#83 / merged PR #84 added the PostgreSQL persistence boundary for the immutable metadata lifecycle:

- immutable tenant-scoped revision headers, canonical documents and dependency edges;
- deterministic reconstruction with revision identity verification;
- optimistic activation heads and expected-generation conflicts;
- per-tenant transaction advisory locking;
- durable pop-only rollback history;
- append-only transition evidence bound to actor, request, capability and business transaction;
- FORCE RLS and immutable published-state enforcement;
- real PostgreSQL acceptance and migration rollback/reapply coverage.

PR #84 merged as `adbb639da69f5d87873b3c603a1388021c8359da` after all applicable gates were green.

### Phase 7G — governed metadata API and application composition — Complete

#85 / merged PR #86 closed the public governed boundary over metadata runtime and persistence:

- exact versioned Protobuf mutations and queries for publish, impact, activate, revision read, activation read and rollback;
- typed schema-to-bundle conversion through `crm-metadata-schema`;
- metadata mutations through `CapabilityGateway` and reads through `QueryGateway`;
- PostgreSQL-backed production adapters and application composition;
- canonical global `crm.audit_records` plus normal idempotency/business-transaction evidence;
- typed browser metadata operations with a shared governed gRPC-Web transport;
- no generic raw metadata gateway or frontend capability/query coordinate escape hatch.

Final review head `7989ea1256f01bfd4e8ee2d33f5ad8370d6cc645` passed all 11 applicable workflows simultaneously. PR #86 merged to `main` as `970548d14faf26f4b8f6cb47f7d9f168e61d9c28`.

### Phase 7H — first governed Admin Studio workflow — Complete

#87 / merged PR #88 delivered the first real product-plane metadata workflow:

- permission-aware `/admin/metadata` route;
- typed object-definition authoring with no raw JSON editor;
- immutable candidate publication;
- structural impact review;
- explicit breaking-change confirmation;
- optimistic activation and rollback generation handling;
- mutation idempotency scoped to user intent;
- safe authentication, authorization, validation, conflict and transport states;
- real browser E2E against fresh PostgreSQL and `crm-api` proving first activation, a breaking second candidate, confirmation-gated activation and rollback.

Final review head `f78f1c75bf97733ff88eafcd2d2ed2ab6c7615d9` passed Product Plane CI, including real process/browser E2E, and Rust CI. PR #88 merged to `main` as `0f01f22e6c77cd4f138a6b678d75d259f3ac71ff`.

### Phase 7I — typed UI-extension runtime and host failure isolation — Complete

#89 / merged PR #90 closed the final Phase 7 product-plane foundation:

- exact typed record-page extension surfaces;
- immutable validated registry with owner-bound deterministic coordinates and locale-independent ordering;
- invalid and duplicate registration rejection;
- readonly host-owned typed context without session/client/raw gateway/infrastructure injection;
- per-extension lazy loading, `Suspense`, render/load error isolation and bounded retry;
- safe failure evidence containing extension identity, surface, phase and attempt only;
- failure-observer isolation so telemetry hooks cannot break the host;
- development-only lazy-loaded record-page proof fixture rather than a fake production record page;
- real browser acceptance proving host shell, core record content and healthy sibling extensions survive render and lazy-load failures and targeted retry.

The new duplicate-coordinate unit test exposed and prevented a real defect where `Set.add()` had been incorrectly treated as a boolean. Final review head `874dde11f5d558bd5e53f2def3e8903ff12f361a` passed Governance CI, Rust CI and Product Plane CI including full PostgreSQL/process/browser E2E. PR #90 merged to `main` as `0fb389c72b148311f590c3fdbae2a4f89fffd915`.

Phase 10 remains responsible for signed packages and sandboxed untrusted marketplace execution. Phase 7I deliberately does not claim arbitrary third-party JavaScript isolation.

## Active executable program — Phase 8A

**#28 — canonical customer master, identity resolution and consent.**

This is now the first active expert owner-domain program. It must establish stable customer identity ownership before Sales, Service, Marketing, Billing, projects or AI expand around incompatible local person/account models.

Required owner-domain work includes:

- Party identities for people and organizations;
- Account/customer relationships that reference parties rather than own identity;
- typed contact points and preferences;
- time-bounded party relationships and hierarchies;
- purpose/channel/jurisdiction-specific consent and withdrawal evidence;
- source identifiers, match evidence, survivorship decisions and immutable merge/unmerge lineage;
- duplicate detection with explainable candidates and approval-aware high-risk decisions;
- privacy/export/deletion/legal-hold interaction evidence;
- governed import/export and enrichment with provenance.

The first implementation packet must define exact ownership and contracts before broad CRUD surface is added.

## Development system

The repository uses the exact-SHA multi-agent model from #70 / merged PR #72:

- one Architect / Implementer owns overlapping packet scope;
- a Local Integrator / Verifier may verify an exact immutable SHA or take explicitly delegated non-overlapping work;
- every verification claim names the exact SHA actually tested;
- a new commit invalidates prior evidence for checks not rerun;
- GitHub CI remains the final exact-head merge authority.

#74 / merged PR #75 adds capability-based Codex qualification. #76 remains an open process-hardening follow-up to make exact-SHA review freeze explicitly aware of source-changing automation.

## Product readiness summary

### Implemented business owner modules

- `crm.sales` — production Deal vertical slice; broader Sales expert functionality remains planned.
- `crm.activities` — production Task vertical slice; broader activity/calendar/productivity functionality remains planned.

### Implemented link module

- `crm.sales-activities-link` — independently governed optional link module with pure core, published contract adapter, durable event delivery, lifecycle gating and production end-to-end acceptance.

### Implemented platform/product foundations

- module lifecycle, governed capability/query execution, tenant/RLS data foundation and append-only audit;
- rebuildable projections and permission-aware search;
- production application composition;
- typed web product shell and governed browser-client boundary;
- immutable tenant-scoped metadata publication lifecycle;
- strict typed metadata schemas and durable tenant-scoped persistence;
- governed metadata public API/application composition;
- governed Admin Studio publication/impact/activation/rollback workflow;
- typed trusted-code UI-extension runtime with failure isolation and real browser acceptance.

### Not yet complete

- broad canonical customer master, identity resolution and consent — active #28;
- broad product-quality Sales/Activities UX and mobile experience;
- product catalog, pricing, CPQ and quote-to-revenue lifecycle — #29;
- communications, marketing, support/service, projects, documents/e-signature and analytics domains;
- AI-native governed actor/tool layer;
- signed marketplace/WASM sandbox;
- enterprise restore/failover/security/SLO and operational proof.

## Immediate delivery sequence

1. Begin Phase 8A / #28 with explicit Party, Account, Contact Point, Relationship, Consent and Identity Resolution ownership/contracts.
2. Deliver the customer-master program as end-to-end owner-domain packets with cross-tenant, idempotency, audit, merge/unmerge, consent-withdrawal and privacy evidence.
3. Follow with Phase 8B / #29 commercial lifecycle without absorbing catalog/pricing/order/contract ownership into Sales.
4. Continue frontend and expert backend work as end-to-end vertical slices.
5. Continue continuous enterprise/security/operational hardening without making premature production-completeness claims.

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
- parent/phase GitHub issues;
- README only for stable orientation, not detailed phase bookkeeping.

README must not become a second manually maintained roadmap.
