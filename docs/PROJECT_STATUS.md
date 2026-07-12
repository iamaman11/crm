# Ultimate CRM — Project Status

Status date: 2026-07-12

This document is the concise human-readable status page. The normative sequence remains `IMPLEMENTATION_ROADMAP.md`; absolute rules remain `SYSTEM_INVARIANTS.md`; implementation grouping follows `DEVELOPMENT_WORKFLOW.md`; multi-agent execution follows `MULTI_AGENT_DEVELOPMENT.md`.

## Current position

**Phase 6 is complete. Phase 7 is in progress.**

The repository contains a complete first production-composed modular CRM proof and the first production-quality product-plane and Admin Studio metadata foundations:

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
- typed web product shell with a governed generated client boundary and real browser E2E through `search.global.query`;
- immutable tenant-authorized metadata publication lifecycle;
- strict typed Admin Studio metadata schemas and canonical validation;
- durable tenant-scoped metadata revision/activation persistence — current Phase 7F packet.

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

### Phase 7D — immutable metadata publication lifecycle — Complete

#77 / merged PR #78 established the backend-neutral lifecycle foundation for Admin Studio metadata:

- `crm-metadata-runtime` as a pure platform-domain crate with no PostgreSQL, transport, browser or business owner-module dependency;
- typed metadata kinds for object, field, relationship, layout, view, pipeline, permission and workflow definitions;
- validated namespaced metadata identifiers;
- complete metadata-bundle snapshots with explicit intra-bundle dependencies;
- deterministic content-addressed SHA-256 revision identity under `crm.metadata.bundle.sha256/v1`;
- immutable and idempotent publication;
- deterministic structural impact analysis for additions, modifications and removals;
- explicit confirmation for structurally breaking activation;
- optimistic activation generations and rollback across immutable revisions.

Final review head `9595ce934f0ceaf23025676474f340e62bdd960d` passed Governance, Rust, Rust Generated Sync, Database, Event, Projection, Search and Application Runtime CI before PR #78 was squash-merged as `de1ea407790d8c6c74f363b21622d332df85f727`.

#79 / merged PR #80 then hardened the public composition boundary so metadata publication authority is tenant-scoped by construction:

- the deterministic single-scope engine is private;
- application-facing callers use `TenantMetadataCatalog`;
- publication, revision lookup, impact analysis, activation and rollback require explicit `TenantId`;
- knowing a deterministic revision hash does not grant cross-tenant read or activation authority;
- identical content may retain identical content identity only after independent publication into each tenant authority;
- activation generations and rollback histories remain isolated.

Final review head `675d389695e4881e62732bcec17b4eadcaf62917` passed architecture, lockfile, `rustfmt`, Clippy, full workspace tests and Rust Generated Sync before PR #80 was squash-merged as `fcf2d8d7ab0d1c94999b8a6feea7b3be9f97db7f`.

### Phase 7E — typed Admin Studio metadata schemas — Complete

#81 / merged PR #82 replaced opaque metadata authoring payloads with strict typed v1 contracts before persistence, public APIs or UI composition:

- pure `crm-metadata-schema` crate;
- typed object, field, relationship, layout, saved-view, pipeline, permission-template and workflow definitions;
- bounded field/configuration semantics, including text length, decimal precision/scale and enum uniqueness;
- strict duplicate and intra-definition reference validation;
- deterministic dependency extraction into `MetadataKey` references consumed by bundle-level validation;
- deterministic canonical UTF-8 JSON under `crm.metadata.definition/v1`;
- set-like members canonicalized independently of insertion order while meaningful authoring order remains identity-significant;
- workflow actions restricted to exact SemVer governed capability references, with no raw script, SQL or arbitrary HTTP execution primitive;
- focused acceptance for all eight metadata kinds, canonicalization, ordered identity, typed validation failures, bundle dependency enforcement and strict unknown-field rejection.

Final review head `889a5161233283a1b1460a221df2b406522b588b` passed Governance, Rust, Rust Generated Sync, Database, Event, Projection, Search and Application Runtime CI before PR #82 was squash-merged as `885f479bcfa85ccd52817900359ea397e7a20544`.

## Current executable packet — Phase 7F

**#83 / draft PR #84 — durable tenant-scoped metadata persistence and transition evidence.**

The current packet persists the immutable runtime lifecycle without moving metadata semantics into SQL:

- migration `0010_metadata_publication_runtime`;
- immutable tenant-scoped revision headers, canonical documents and explicit dependency edges;
- deterministic PostgreSQL reconstruction with revision identity verification;
- tenant-scoped optimistic activation heads;
- per-tenant transaction advisory locking plus expected-generation conflicts for concurrent activation;
- a durable push/pop rollback stack that cannot toggle a rolled-back revision forward;
- structural impact and breaking-change analysis delegated back to `crm-metadata-runtime`;
- append-only publish/activate/rollback transition evidence bound to actor, request, capability and business-transaction context;
- FORCE RLS and transaction-local write-context enforcement on all six metadata tables;
- immutable UPDATE/DELETE rejection for published revision state and transition evidence;
- real PostgreSQL acceptance for round-trip identity, idempotence, cross-tenant non-disclosure, concurrency, breaking confirmation, rollback, RLS and immutability;
- dedicated migration clean-install, rollback and reapply verification.

The persistence packet intentionally does not fabricate outbox/idempotency state merely to enter the existing global business-transaction audit chain. The follow-on governed metadata capability/API packet must add canonical `crm.audit_records` evidence through the normal public capability execution contract.

Exact-head evidence is recorded in PR #84 and must be refreshed after every source or documentation commit before merge.

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
- immutable tenant-scoped metadata publication lifecycle;
- strict typed metadata schemas and canonical validators;
- durable tenant-scoped metadata persistence — in progress in #83 / PR #84.

### Not yet complete

- governed metadata publication/query APIs and canonical global audit evidence through the public capability transaction contract;
- first Admin Studio workflows in the product plane;
- typed UI-extension runtime with host-shell failure isolation;
- broad product-quality Sales/Activities UX and mobile experience;
- canonical customer master, identity resolution and consent — #28;
- product catalog, pricing, CPQ and quote-to-revenue lifecycle — #29;
- communications, marketing, support/service, projects, documents/e-signature and analytics domains;
- AI-native governed actor/tool layer;
- signed marketplace/WASM sandbox;
- enterprise restore/failover/security/SLO and operational proof.

## Immediate delivery sequence

1. Complete #83 / PR #84: durable tenant-scoped PostgreSQL publication/activation persistence, optimistic concurrency, rollback stack and append-only transition evidence.
2. Expose governed metadata publish/activate/rollback/query contracts and produce canonical global audit evidence through the normal capability execution boundary.
3. Compose the first Admin Studio workflows through the product plane against those governed contracts.
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
