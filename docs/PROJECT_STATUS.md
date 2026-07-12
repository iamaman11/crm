# Ultimate CRM — Project Status

Status date: 2026-07-12

This document is the concise human-readable status page. The normative sequence remains `IMPLEMENTATION_ROADMAP.md`; absolute rules remain `SYSTEM_INVARIANTS.md`; implementation grouping follows `DEVELOPMENT_WORKFLOW.md`; multi-agent execution follows `MULTI_AGENT_DEVELOPMENT.md`.

## Current position

**Phase 6 is complete. Phase 7 is in progress.**

The repository contains a complete first production-composed modular CRM proof and the first production-quality product-plane foundation:

- executable repository governance and architecture boundaries;
- typed Module Manifest IR and immutable module identity;
- governed Module SDK and deterministic test harness;
- module publication, installation and lifecycle runtime;
- PostgreSQL tenant/RLS, record, relationship, idempotency, outbox and append-only audit foundation;
- authenticated mutation and permission-bound query gateways;
- independent Sales Deal and Activities Task owner-domain vertical slices;
- governed event delivery and the optional `crm.sales-activities-link` module;
- rebuildable projections and tenant/permission-aware search;
- real `crm-application-runtime` composition boundary and deployable `services/crm-api` process host;
- typed web product shell with a governed generated client boundary and real browser E2E through `search.global.query`.

The breadth of end-user CRM functionality is still intentionally smaller than the target universal expert CRM. Customer master, commercial lifecycle, service, marketing, communications, analytics, AI, marketplace and enterprise operational proof remain explicit future owner-domain/platform programs.

## Completed delivery foundations

### Phase 6 — first modular production proof

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

### Phase 7A — golden module tooling and generalized projections

- #56 / merged PR #64 established repository-supported owner/link module scaffolding and permanent repository commands.
- #65 / merged PR #67 introduced `crm-projection-runtime`, deterministic projection registration/execution, poison/failure handling and rebuild orchestration without moving owner-domain semantics into infrastructure.

### Phase 7B — permission-aware search

#66 / merged PR #68 completed the production search foundation:

- search indexes are candidate-only and rebuildable;
- live resource and field visibility are re-checked before disclosure;
- logical search generations support deterministic rebuild/switching;
- PostgreSQL FTS remains a replaceable adapter;
- `search.global.query` is routed through the governed production `QueryGateway`;
- acceptance covers permission revocation, hidden-field non-disclosure, deterministic pagination and tenant isolation.

Final review head `90d8ad4afc15ba31bc27297e4a9c7081e64ac4e7` passed all applicable Contract, Governance, Rust, Database, Projection, Event, Search, Application Runtime and Rust Generated Sync gates.

### Phase 7C — typed web product shell — Complete

#71 / merged PR #73 established the first governed product-plane foundation:

- reproducible Node 24 / pnpm 11 / strict TypeScript workspace;
- `apps/web`, `packages/client` and `packages/ui` product-plane boundaries;
- generated Protobuf-ES browser contracts;
- browser access through the existing governed `ApplicationGatewayService` over gRPC-Web;
- typed `GovernedClient.searchGlobal` with exact contract identity validation;
- centralized typed session state and product-owned safe error mapping;
- permission-aware routing as UX only, with backend authorization remaining authoritative;
- no public arbitrary raw gateway/query escape hatch;
- design-system/application-shell primitives and responsive/accessibility foundations;
- hermetic Playwright E2E against ephemeral PostgreSQL covering the real governed search workflow and negative authentication/authorization paths.

Final review head `b62dd50225fde6e58aac9a6b4cec307bd2245616` passed all applicable checks before PR #73 was merged and #71 closed.

## Current executable packet — Phase 7D

**#77 / draft PR #78 — immutable Admin Studio metadata publication runtime.**

The current packet establishes the backend-neutral lifecycle foundation that later Admin Studio builders and governed publication APIs must consume:

- `crm-metadata-runtime` as a pure platform-domain crate with no PostgreSQL, transport, browser or business owner-module dependency;
- typed metadata kinds for object, field, relationship, layout, view, pipeline, permission and workflow definitions;
- validated namespaced metadata identifiers;
- complete metadata bundle snapshots with explicit intra-bundle dependencies;
- deterministic content-addressed SHA-256 revision identity under a versioned canonical profile;
- immutable and idempotent publication;
- deterministic structural impact analysis for added, modified and removed definitions;
- explicit confirmation for structurally breaking activation;
- tenant-scoped optimistic activation generations;
- rollback by moving the active pointer to a prior immutable revision;
- focused tests for deterministic identity, invalid dependencies, impact analysis, concurrency conflicts, rollback and tenant isolation.

The packet deliberately does not yet claim kind-specific object/field/layout/workflow schema semantics, PostgreSQL persistence, governed publication APIs or Admin Studio UI. Those are follow-on layers and must not duplicate or weaken the runtime lifecycle invariants.

Current exact-head evidence is recorded in PR #78 and must be refreshed after every source or documentation commit before merge.

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
- typed web product shell and governed browser client boundary;
- metadata publication lifecycle foundation — in progress in #77 / PR #78.

### Not yet complete

- kind-specific Admin Studio metadata builders, durable metadata persistence, governed publication APIs and Admin Studio workflows;
- typed UI-extension runtime with host-shell failure isolation;
- broad product-quality Sales/Activities UX and mobile experience;
- canonical customer master, identity resolution and consent — #28;
- product catalog, pricing, CPQ and quote-to-revenue lifecycle — #29;
- communications, marketing, support/service, projects, documents/e-signature and analytics domains;
- AI-native governed actor/tool layer;
- signed marketplace/WASM sandbox;
- enterprise restore/failover/security/SLO and operational proof.

## Immediate delivery sequence

1. Complete #77 / PR #78: immutable metadata publication, deterministic identity, impact analysis, optimistic activation and rollback.
2. Add metadata-kind-specific schemas/validators/builders and durable PostgreSQL publication/activation persistence with typed audit evidence.
3. Expose governed metadata publication/query contracts and compose the first Admin Studio workflows through the product plane.
4. Complete the typed UI-extension runtime/failure-isolation foundation required to close Phase 7.
5. Begin the domain-wave program #57, with customer master/identity/consent (#28) and commercial lifecycle (#29) remaining explicit owner-domain programs rather than being absorbed into Sales.
6. Continue frontend and expert backend modules as end-to-end vertical slices.

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
