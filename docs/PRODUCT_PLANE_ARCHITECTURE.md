# Ultimate CRM — Product Plane Architecture

Status: **Phase 7C normative implementation guide**  
Tracked by: issue #71  
Parent phase: #10

This document defines the first web product-plane architecture for Ultimate CRM. It complements `SYSTEM_INVARIANTS.md`, `APPLICATION_ARCHITECTURE.md`, `DEVELOPMENT_WORKFLOW.md` and `MULTI_AGENT_DEVELOPMENT.md`.

The product plane owns presentation and interaction. It never becomes an alternate business runtime, authorization authority or source of truth.

## 1. Architecture goals

The Phase 7C product shell must provide a stable foundation for later Admin Studio and expert CRM domain waves:

- a reproducible TypeScript workspace;
- a browser application composition root;
- a contract-derived governed API client boundary;
- centralized authentication/session state;
- permission-aware navigation that never substitutes for backend authorization;
- reusable design-system and application-shell primitives;
- explicit loading, empty, error and recovery conventions;
- localization and time-zone boundaries;
- browser-level acceptance against the real governed backend path.

The product shell is infrastructure for product experience, not a place to duplicate domain invariants.

## 2. Repository structure

```text
apps/
  web/                    # browser application composition root
packages/
  client/                 # generated contracts + governed transport/session boundary
  ui/                     # design-system primitives and application layout
```

Rules:

- `apps/web` may compose product-plane packages and feature surfaces;
- `packages/client` owns all browser communication with the governed application gateway;
- `packages/ui` has no backend, tenant, authorization or business-domain authority;
- future domain UI packages may depend on `packages/client` and `packages/ui`, but must not issue ad-hoc direct network calls to CRM business endpoints;
- generated Protobuf sources are reproducible build output and are not manually edited.

## 3. Selected toolchain baseline

Phase 7C starts with:

- Node.js 24 LTS as the supported local/CI runtime line;
- pnpm workspace management;
- TypeScript in strict mode;
- React 19 product rendering;
- Vite for development and production bundling;
- Oxlint for fast JavaScript/TypeScript linting;
- Protobuf-ES local generation from the repository `proto/` source tree;
- Connect-ES browser client with the gRPC-Web transport against the existing tonic application gateway;
- Vitest and Playwright introduced as the behavior/browser acceptance layers during this packet.

Version pins live in package manifests and the committed pnpm lockfile. Upgrades are ordinary reviewed dependency changes, not floating runtime behavior.

## 4. Authoritative contracts and generated client boundary

Protobuf remains authoritative for RPC, command and event contracts.

The browser contract path is:

```text
proto/**/*.proto
→ local deterministic Protobuf-ES generation
→ packages/client/src/gen/**
→ packages/client governed gateway wrapper
→ apps/web and future product features
```

Generated code must never be manually edited. Contract drift is prevented by regenerating from the repository source and checking the resulting product-plane build in CI.

Application feature code must not construct a second handwritten DTO model that silently changes the meaning of published Protobuf fields.

The generated service descriptor is a transport artifact. The governed client wrapper remains responsible for:

- attaching session-derived authentication metadata;
- attaching tenant/request/correlation/trace metadata where applicable;
- preserving exact owner module, capability/query id and version coordinates;
- mapping transport failures into stable product-plane error categories;
- refusing requests when there is no valid authenticated session;
- exposing typed request/response operations rather than arbitrary fetch access.

## 5. Browser transport decision

The browser uses **gRPC-Web over the existing versioned `crm.gateway.v1.ApplicationGatewayService`**.

The production path is:

```text
React feature
→ packages/client governed wrapper
→ generated ApplicationGatewayService client
→ gRPC-Web transport
→ tonic-web compatibility layer
→ existing ApplicationGatewayService
→ existing GrpcCapabilityMiddleware / GrpcQueryMiddleware
→ CapabilityGateway / QueryGateway
→ governed owner adapter
```

This preserves the current production authority chain. The product plane does not receive a private alternate API.

### Why gRPC-Web

- the existing canonical application gateway is already Protobuf/tonic based;
- browser clients can be generated from the same authoritative service contract;
- the transport can preserve typed unary mutation/query envelopes;
- the existing backend middleware already owns authentication, tenant/actor context, live authorization and safe error mapping;
- later transport evolution can remain behind `packages/client` without changing feature components.

### Same-origin deployment rule

The preferred browser deployment is same-origin:

```text
https://crm.example/
  /                       → static web application
  /crm.gateway.v1.*       → gRPC-Web reverse proxy to crm-api gRPC listener
```

The Vite development server proxies the gRPC-Web service path to the local gRPC listener. This avoids making permissive cross-origin credentials a platform requirement.

Production deployments may use a dedicated API origin only with explicit origin policy, TLS and credential/metadata handling. Wildcard credentialed CORS is not an accepted default.

## 6. Session model

The product client models session state explicitly:

```text
unknown/loading
→ unauthenticated
→ authenticated
→ expired/revoked
```

The browser may hold a session credential needed to call the delivery plane, but it does not decide actor identity or tenant authority. The backend authenticator remains authoritative.

A session snapshot contains only product-plane coordination data required to make a governed request, such as:

- opaque bearer/session credential;
- selected tenant coordinate when the authenticated actor is allowed to operate in more than one tenant;
- optional display-only actor/tenant labels;
- expiry metadata when the authentication system exposes it.

Rules:

- logout/revocation clears protected client caches and in-memory session state;
- session-expiry handling is centralized in `packages/client`/application composition, not duplicated in feature components;
- multiple concurrent unauthenticated failures must converge on one session transition rather than trigger uncontrolled refresh loops;
- production identity-provider integration remains replaceable behind the session provider boundary.

The initial packet may use an explicit development/test session adapter for the current bootstrap bearer token. It must be visibly non-production and must not weaken backend authentication.

## 7. Governed request metadata

The generated transport is wrapped by a metadata interceptor/provider.

At minimum:

- `authorization` comes from the current authenticated session;
- tenant metadata comes from the current permitted tenant selection;
- request/correlation/trace identifiers are generated or propagated by the product client where the gateway contract supports them;
- mutation-only idempotency/business-transaction metadata is never attached to query calls;
- sensitive credentials are never logged by the client diagnostics layer.

Feature components do not receive raw credential strings.

## 8. Permission-aware routing

Client routing has two separate concerns:

1. **navigation eligibility** — whether the UI should present a route/action based on the currently known product capability snapshot;
2. **backend authorization** — the authoritative live server decision for every governed request.

Navigation eligibility is UX only.

A route descriptor may declare:

- stable route id;
- path;
- navigation label;
- authentication requirement;
- optional capability/visibility requirement;
- render element.

The route registry must support:

- public routes;
- authenticated routes;
- capability-aware navigation;
- safe forbidden/not-found states;
- future module UI extension contributions without importing owner-module backend internals.

A stale client permission snapshot may hide too much temporarily, but it must never grant backend access.

## 9. Design-system boundary

`packages/ui` owns reusable visual and interaction primitives:

- application frame and navigation regions;
- buttons, inputs and form field shells;
- feedback/status surfaces;
- loading, empty and error states;
- table/layout primitives required by CRM workflows;
- focus, keyboard and semantic accessibility conventions;
- responsive layout tokens;
- theme/design tokens.

It does not own:

- Deal, Task, Customer, Quote or other business invariants;
- tenant or actor identity;
- permissions;
- API transport;
- persistence.

Business semantics enter UI primitives through typed props from feature/application composition.

## 10. Error model

The product plane distinguishes at least:

- unauthenticated;
- permission denied;
- not found/non-disclosing;
- invalid input;
- conflict;
- rate limited;
- dependency/unavailable;
- unexpected internal failure;
- network/offline failure.

The backend remains authoritative for safe error codes and retryability. UI text must not depend on parsing arbitrary backend error strings.

A route/page must define:

- initial loading state;
- empty state where applicable;
- recoverable error state;
- non-recoverable/forbidden state;
- retry behavior only when the error class permits it.

## 11. Localization and time zones

The initial shell is structured so visible text can move into translation catalogs without changing business contracts.

Rules:

- authoritative timestamps remain machine-readable instants from contracts;
- display formatting uses an explicit user/tenant time-zone choice;
- locale affects presentation, not canonical identifiers or semantic hashes;
- money and other exact decimals are never parsed through locale-dependent floating-point shortcuts;
- server-provided stable codes are mapped to localized product copy at the presentation boundary.

## 12. Product-plane dependency direction

```text
apps/web
  → packages/client
  → generated Protobuf contracts

apps/web
  → packages/ui

future feature UI package
  → packages/client
  → packages/ui
```

Forbidden:

```text
feature UI → arbitrary fetch to CRM business endpoints
feature UI → PostgreSQL or persistence adapter
feature UI → owner-module Rust internals
packages/ui → API/session/tenant authority
client-side route visibility → authorization decision
```

## 13. Development commands

The root package workspace provides stable commands for product-plane work. The intended command surface is:

```text
pnpm web:generate
pnpm web:typecheck
pnpm web:lint
pnpm web:test
pnpm web:build
pnpm web:check
```

Exact commands are implemented in the root/package manifests. `web:check` is the local common product-plane gate; specialized browser/process acceptance remains separate where required.

## 14. Multi-agent checkpoints for #71

### Checkpoint A — architecture and build foundation

Required result:

- workspace installs from declared package-manager/toolchain versions;
- Protobuf-ES generation runs from repository contracts;
- product packages typecheck;
- the web shell production build succeeds;
- lint succeeds;
- architecture document and dependency direction are coherent.

The first local verifier handoff may explicitly allow generation of the initial `pnpm-lock.yaml` as a mechanical artifact if the Architect / Implementer environment cannot access the package registry. Any resulting commit becomes a new exact SHA and must be handed back before implementation continues.

### Checkpoint B — behavior

Required result:

- session state transitions are tested;
- governed client metadata/error mapping is tested;
- permission-aware route eligibility is tested;
- a real existing backend query is exercised through the generated client path;
- negative authentication and authorization/non-disclosure cases are covered.

### Checkpoint C — delivery

Required result:

- clean frozen install from the committed lockfile;
- generation reproducibility;
- typecheck;
- lint;
- unit/integration tests;
- production build;
- Playwright browser acceptance for the first governed workflow;
- applicable Rust/backend integration gates for any changed gateway behavior.

Final merge still requires all applicable GitHub checks green on one exact final review head.

## 15. Definition of done for the foundation

Phase 7C is not complete merely because a React page renders.

The product-shell foundation is complete only when:

- a clean checkout builds reproducibly;
- browser contracts derive from authoritative Protobuf;
- browser transport reaches the existing governed application gateway;
- authentication/session handling is centralized;
- navigation is permission-aware without becoming an authorization control;
- reusable design-system/loading/error/localization foundations exist;
- at least one real CRM read workflow is accepted end-to-end;
- local multi-agent checkpoints and final exact-head CI evidence are recorded;
- documentation describes the exact production path and remaining scope.
