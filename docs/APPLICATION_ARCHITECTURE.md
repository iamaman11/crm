# Ultimate CRM — Application Architecture

Status: **Normative structural guide**

This document explains how the repository becomes a coherent application without collapsing domain ownership into a monolith or prematurely splitting into network microservices.

## 1. Architectural model

The target is a **modular monolith with independently governed modules and explicit extraction boundaries**.

A business module is logically independent but may run in-process. Network distribution is an operational choice, not the mechanism used to create domain boundaries.

The application has five major planes:

```text
┌─────────────────────────────────────────────────────────────┐
│ Product plane                                               │
│ web/mobile shell, Admin Studio, module UI extensions        │
├─────────────────────────────────────────────────────────────┤
│ Delivery plane                                              │
│ crm-api, HTTP/gRPC, auth context, health, process lifecycle  │
├─────────────────────────────────────────────────────────────┤
│ Application plane                                           │
│ capability gateway, query gateway, policy, orchestration    │
├─────────────────────────────────────────────────────────────┤
│ Domain plane                                                │
│ independent owner modules and optional link modules         │
├─────────────────────────────────────────────────────────────┤
│ Infrastructure plane                                        │
│ PostgreSQL, event delivery, files, search, integrations     │
└─────────────────────────────────────────────────────────────┘
```

Dependency direction points inward toward stable contracts and ports. Domain modules never depend outward on infrastructure implementations.

## 2. Repository skeleton

Current structure:

```text
/
├── AGENTS.md
├── proto/                    # authoritative RPC/command/event contracts
├── crates/                   # platform runtimes, ports and adapters
├── modules/                  # independently governed business/link modules
├── services/
│   └── crm-api/              # production application composition root
├── database/                 # authoritative SQL migrations and DB acceptance
├── schemas/                  # authoring schemas compiled to typed runtime IR
├── scripts/                  # governance and architecture enforcement
├── docs/
│   ├── adr/                  # accepted architecture decisions
│   ├── SYSTEM_INVARIANTS.md
│   ├── IMPLEMENTATION_ROADMAP.md
│   ├── PROJECT_STATUS.md
│   ├── MODULE_CATALOG.md
│   └── APPLICATION_ARCHITECTURE.md
└── .github/workflows/        # permanent conformance gates
```

Future product-plane structure may be introduced when Phase 7 starts:

```text
apps/
  web/                        # product shell
packages/
  ui/                         # design system and shared UI primitives
  client/                     # generated/typed API client boundary
```

Do not create frontend directories before there is executable Phase 7 scope and ownership.

## 3. Layer responsibilities

### 3.1 Product plane

Owns presentation and interaction only:

- route/navigation shell;
- tables, forms, timelines and workspaces;
- Admin Studio;
- module UI extension rendering;
- accessibility, localization and responsive behavior.

It does not own business invariants and cannot bypass capability/query authorization.

### 3.2 Delivery plane

`services/crm-api` is the production composition root.

It may own:

- process startup and shutdown;
- configuration parsing and validation;
- dependency construction;
- HTTP/gRPC listeners;
- health, readiness and diagnostics;
- observability bootstrap;
- graceful shutdown and drain.

It must not own:

- Deal/Task/customer/catalog business rules;
- direct cross-module orchestration;
- ad-hoc SQL;
- alternate mutation paths.

### 3.3 Application plane

Owns governed execution flow:

- exact capability/query resolution;
- semantic validation;
- rate/approval/live authorization;
- execution context;
- deterministic planner/executor boundaries;
- safe error mapping;
- composition of owner adapters without domain ownership leakage.

Mutation and query paths remain separate.

### 3.4 Domain plane

Contains owner and link modules.

Owner modules:

- own mutable aggregate invariants;
- expose versioned capabilities/events/queries;
- depend only on stable contracts and SDK ports;
- remain independently buildable and testable.

Link modules:

- own only cross-domain coordination and their own deduplication/configuration state;
- consume versioned events/capabilities;
- never read or mutate another module's storage directly;
- can be disabled or removed without breaking owner modules.

### 3.5 Infrastructure plane

Contains implementations of stable ports:

- PostgreSQL persistence and transactions;
- event delivery/outbox workers;
- files/object storage;
- search/indexing;
- external integration adapters.

Infrastructure may depend on domain-neutral runtime contracts. Business modules do not depend on infrastructure.

## 4. Production request paths

### Mutation

```text
HTTP/gRPC
→ authentication
→ tenant/actor resolution
→ immutable execution context
→ exact capability/version
→ typed + semantic validation
→ rate/approval policy
→ live authorization
→ synchronous deterministic planning
→ one governed PostgreSQL transaction
→ state + idempotency + outbox + audit
→ typed safe response
```

### Query

```text
HTTP/gRPC
→ authentication
→ tenant/actor resolution
→ query-only execution context
→ exact query/version
→ typed + semantic validation
→ live authorization
→ resource/field visibility
→ authoritative tenant-scoped read
→ masking/serialization
→ typed safe response
```

Queries do not require mutation-only idempotency keys or business transaction IDs.

### Event-to-capability cross-domain flow

```text
owner mutation
→ transactional outbox event
→ governed event delivery
→ optional link module
→ deterministic delivery identity
→ target capability through CapabilityClient
→ target owner mutation path
```

Duplicate event delivery must not create duplicate business effects.

## 5. Composition boundaries

The final `crm-api` service should be assembled in explicit stages:

```text
configuration
→ infrastructure resources
→ platform stores/adapters
→ module publication/install state
→ capability/query catalogs
→ owner adapter compositions
→ auth/policy runtime
→ ingress
→ HTTP/gRPC servers
→ health/readiness
→ shutdown/drain
```

Construction failures are startup failures. Invalid production configuration must not degrade silently into partially governed behavior.

## 6. Directory evolution rule

The current flat `crates/` namespace is acceptable while crate names remain explicit and the workspace is manageable.

Do not perform a broad directory reorganization merely for aesthetics. Introduce physical grouping only when one of these becomes true:

- navigation becomes materially difficult;
- workspace size makes ownership unclear;
- separate release/build boundaries require it;
- architecture enforcement can preserve history and dependency rules during the move.

Prefer stable crate names and enforceable dependency rules over frequent moves.

## 7. Module extraction rule

A module may later become a separate process only when there is an operational reason such as:

- independent scaling;
- isolation or residency requirements;
- deployment cadence;
- failure containment.

Extraction must preserve the same versioned capability/event boundaries. Do not introduce a network call merely to simulate modularity.

## 8. Frontend development model

Frontend is a separate product-plane workstream, but it should not be postponed until every backend module is finished.

Sequence:

1. complete the first backend vertical proof and production composition root;
2. establish Phase 7 product shell, typed client boundary and Admin Studio foundations;
3. develop later expert modules as end-to-end vertical slices: domain + contract + backend + projections/search + UI;
4. keep business invariants authoritative on the backend/domain side.

## 9. Architectural definition of done

A new feature is not complete merely because its domain code compiles. Depending on scope, completion includes:

- explicit ownership;
- typed domain invariants;
- versioned contract compatibility;
- governed mutation/query/event path;
- tenant isolation and live authorization;
- idempotency/retry semantics;
- rollback/fault evidence;
- rebuildability for derived state;
- application composition;
- user-facing experience when the product plane exists;
- exact CI evidence and synchronized documentation.