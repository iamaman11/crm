# ADR-032: Versioned Sales and Activities Protobuf contracts

Status: Accepted

## Context

Phase 6A established invariant-safe Rust domain types for Sales deals and Activities tasks. Those types are intentionally not a public wire format: Rust layout, debug output and implementation details are not compatibility contracts. Public capability requests, responses and domain event payloads need a language-neutral, mechanically checked schema with explicit versioning and safe evolution rules.

The repository already used a Buf v2 workspace and contained an early `crm.sales.v1.Deal` schema plus a legacy `ChangeStage` RPC. Removing or renumbering those fields and methods would break generated clients and persisted payloads.

## Decision

### Authoritative wire format

Protobuf packages under `proto/crm/**/v1` are the authoritative public wire contracts for RPC requests, RPC responses and event payloads. Package major versions are explicit. Semantic breaking changes require a new package or contract version; existing field numbers and RPC methods are retained or reserved.

### Compatibility-preserving Sales evolution

The original fields 1 through 8 of `crm.sales.v1.Deal` and the legacy `ChangeStage` RPC remain wire-compatible. Legacy scalar stage, amount and owner fields are deprecated but not removed. Structured stage identity, exact money, actor/team ownership, resource references, lifecycle status, close outcome, probability and timestamps are added on new field numbers.

### Activities publication

`crm.activities.v1` publishes task commands, responses, query contracts and event payloads for create, update, complete, reminder scheduling, get and list behavior. The schema mirrors the Phase 6A aggregate without exposing storage layout.

### Shared exact primitives

`crm.core.v1` adds:

- `ExactMoney`, using a canonical base-10 string for arbitrary-precision integer minor units plus currency code;
- `CalendarDate` without implicit time zone;
- `UnixTime` in nanoseconds;
- actor/team ownership;
- version-aware resource references;
- explicit patch messages where an absent patch means Keep and a selected oneof means Set or Clear.

Authoritative money is never represented by binary floating point.

### Contract-to-module binding registry

`contracts/module-contract-bindings.json` binds every Sales and Activities capability/version to an exact service method, request message and response message, and binds every event/version to an exact payload message.

`validate_contract_bindings.py` reads the compiled FileDescriptorSet and module manifests and fails when:

- a bound message or RPC does not exist;
- the RPC input/output differs from the registry;
- a manifest capability/event has no binding;
- a binding is not declared by its owner module;
- duplicate module, capability or event bindings exist.

### Continuous compatibility evidence

Contract CI uses a pinned stable Buf GitHub Action and CLI to:

- build a complete descriptor graph;
- enforce canonical formatting and STANDARD lint rules;
- run FILE-level breaking detection against the pull request base branch;
- validate manifest-to-descriptor bindings;
- retain diagnostics and an immutable descriptor artifact.

The descriptor artifact is evidence and an input for future SDK generation; it is not a substitute for source-controlled schemas.

## Consequences

- Sales and Activities contracts are consumable across languages without importing Rust modules.
- Existing early Sales clients remain wire-compatible while new clients can use structured fields.
- Manifest and Protobuf drift becomes a CI failure rather than a runtime surprise.
- Contract review has explicit CODEOWNERS coverage.
- This ADR does not yet bind the Protobuf messages to PostgreSQL planners or expose production endpoints; that is the next Phase 6 slice under issue #9.
