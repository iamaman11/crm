# Ultimate CRM — Project Status

Status date: 2026-07-13

This document is the concise human-readable status page. The normative sequence remains `IMPLEMENTATION_ROADMAP.md`; absolute rules remain `SYSTEM_INVARIANTS.md`; implementation grouping follows `DEVELOPMENT_WORKFLOW.md`; multi-agent execution follows `MULTI_AGENT_DEVELOPMENT.md`.

## Current position

**Phase 6 is complete. Phase 7 is in progress. The active delivery packet is Phase 7G: governed metadata API and application composition.**

The repository now contains a complete first production-composed modular CRM proof plus production-quality search, product-plane and Admin Studio metadata foundations:

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
- typed web product shell with a governed generated browser-client boundary and real browser E2E;
- immutable tenant-authorized metadata publication lifecycle;
- strict typed Admin Studio metadata schemas and canonical validation;
- durable tenant-scoped metadata revision/activation persistence;
- governed public metadata mutation/query contracts, canonical global audit evidence, production composition and typed browser metadata operations in the active Phase 7G packet.

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
- browser access through the existing governed `ApplicationGatewayService` over gRPC-Web;
- typed `GovernedClient.searchGlobal` with exact contract identity validation;
- centralized typed session state and product-owned safe error mapping;
- permission-aware routing as UX only, with backend authorization remaining authoritative;
- no public arbitrary raw gateway/query escape hatch;
- design-system/application-shell primitives and responsive/accessibility foundations;
- hermetic Playwright E2E against ephemeral PostgreSQL.

Final review head `b62dd50225fde6e58aac9a6b4cec307bd2245616` passed all applicable checks before merge.

### Phase 7E-1 — immutable metadata publication lifecycle — Complete

#77 / merged PR #78 established `crm-metadata-runtime` with complete immutable metadata-bundle snapshots, deterministic SHA-256 revision identity, idempotent publication, structural impact analysis, explicit breaking-change confirmation, optimistic activation generations and rollback.

Final review head `9595ce934f0ceaf23025676474f340e62bdd960d` passed all applicable gates before PR #78 was squash-merged as `de1ea407790d8c6c74f363b21622d332df85f727`.

### Phase 7E-2 — tenant-scoped metadata publication authority — Complete

#79 / merged PR #80 made metadata publication authority tenant-scoped by construction. Publication, revision lookup, impact analysis, activation and rollback require explicit tenant identity; a revision hash is identity, never authorization; activation generations and rollback histories remain tenant-isolated.

Final review head `675d389695e4881e62732bcec17b4eadcaf62917` passed architecture, lockfile, `rustfmt`, Clippy, full workspace tests and Rust Generated Sync before merge.

### Phase 7E-3 — typed Admin Studio metadata schemas — Complete

#81 / merged PR #82 replaced opaque metadata authoring payloads with strict typed v1 contracts for object, field, relationship, layout, saved view, pipeline, permission template and workflow definitions. Canonical JSON, dependency extraction, strict validation and exact governed-capability workflow actions are enforced before persistence or public API use.

Final review head `889a5161233283a1b1460a221df2b406522b588b` passed Governance, Rust, Rust Generated Sync, Database, Event, Projection, Search and Application Runtime CI before PR #82 was squash-merged as `885f479bcfa85ccd52817900359ea397e7a20544`.

### Phase 7F — durable tenant-scoped metadata persistence — Complete

#83 / merged PR #84 added the PostgreSQL persistence boundary for the immutable metadata lifecycle:

- migration `0010_metadata_publication_runtime`;
- immutable tenant-scoped revision headers, canonical documents and dependency edges;
- deterministic reconstruction with revision identity verification;
- optimistic activation heads and expected-generation conflicts;
- per-tenant transaction advisory locking for concurrent activation;
- durable push/pop rollback history that cannot toggle a rolled-back revision forward;
- append-only transition evidence bound to actor, request, capability and business transaction;
- FORCE RLS, transaction-local write-context enforcement and immutable UPDATE/DELETE rejection;
- real PostgreSQL acceptance for identity, idempotence, tenant isolation, concurrency, breaking confirmation, rollback, RLS and migration rollback/reapply.

Final review head `8c8ac7855f8a2e4f0148203c022aa60dadcc1843` passed Governance, Rust, Database, Metadata Runtime, Product Plane, Event, Projection, Search, Application Runtime and Rust Generated Sync before PR #84 merged as `adbb639da69f5d87873b3c603a1388021c8359da`.

## Current executable packet — Phase 7G

**#85 / draft PR #86 — governed metadata capabilities, queries, global audit, production composition and typed browser operations.**

The active packet adds the public governed boundary over the completed metadata runtime and persistence layers:

- exact versioned metadata mutation/query Protobuf contracts;
- typed schema-to-bundle conversion rather than opaque document publication;
- `CapabilityGateway` execution for publish/activate/rollback;
- `QueryGateway` execution for impact/revision/activation reads;
- canonical global `crm.audit_records` evidence for public metadata mutations;
- normal idempotency and business-transaction completion evidence;
- PostgreSQL-backed metadata mutation/query adapters;
- production `crm-application-runtime` composition and process-level PostgreSQL acceptance;
- typed browser operations for publish, impact, activate, revision read, activation read and rollback;
- fail-closed browser-client tests for missing/expired sessions and blank mutation idempotency keys;
- no generic raw metadata gateway and no frontend raw capability/query coordinate escape hatch.

During final review, three integration defects were fixed without weakening architecture gates: generated descriptor-hash drift, a stale acceptance-test transition column name and application bootstrap support for the metadata query owner. Exact-head CI must converge again after the final source/documentation commits before the packet can be declared merge-ready.

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
- strict typed metadata schemas and canonical validators;
- durable tenant-scoped metadata persistence;
- governed metadata public API/application composition in active Phase 7G review.

### Not yet complete

- merge/freeze of the active Phase 7G packet on one green exact head;
- first Admin Studio authoring/review/impact/activate/rollback workflows in the product plane;
- typed UI-extension runtime with host-shell failure isolation;
- broad product-quality Sales/Activities UX and mobile experience;
- canonical customer master, identity resolution and consent — #28;
- product catalog, pricing, CPQ and quote-to-revenue lifecycle — #29;
- communications, marketing, support/service, projects, documents/e-signature and analytics domains;
- AI-native governed actor/tool layer;
- signed marketplace/WASM sandbox;
- enterprise restore/failover/security/SLO and operational proof.

## Immediate delivery sequence

1. Freeze #85 / PR #86 on one exact head with all applicable Contract, Governance, Rust, Database, Metadata Runtime, Application Runtime, Product Plane and generated-sync gates green.
2. Build the first Admin Studio workflows through typed governed metadata operations only.
3. Complete the typed UI-extension runtime/failure-isolation foundation required to close Phase 7.
4. Begin the Phase 8 domain-wave program, with customer master/identity/consent (#28) and commercial lifecycle (#29) remaining explicit owner-domain programs rather than being absorbed into Sales.
5. Continue frontend and expert backend modules as end-to-end vertical slices.

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
