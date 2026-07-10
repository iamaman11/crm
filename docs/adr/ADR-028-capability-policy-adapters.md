# ADR-028: Capability Catalog and Live Policy Adapters

- Status: Accepted
- Date: 2026-07-11

## Context

The deterministic gateway defines stable ports for exact capability resolution, rate limiting, approval verification and live authorization. Test doubles prove ordering, but public mutation transports require concrete concurrent implementations with safe typed failures. The current Module Manifest declares provided capability coordinates only; it does not contain the complete runtime contracts, risk level, authorization policy, approval requirement or rate-limit policy required by `CapabilityDefinition`.

## Decision

The platform introduces a separate `crm-capability-adapters` crate containing production-shaped, concurrency-safe adapters that implement the Phase 5A ports without depending on business modules or transports.

### Capability catalog

The runtime catalog is immutable after construction and resolves only an exact capability ID and version. Catalog construction rejects duplicate coordinates and incomplete definitions. Input and output contracts must be owned by the capability owner module. Tenant installation state is not inferred by this global catalog; it is evaluated by live authorization.

### Live authorization

Authorization grants are tenant-, actor- and policy-scoped and are additionally bound to the exact capability ID, capability version and owner module. Grant updates and revocations advance a store revision. Every authorization call reads current state and emits a revisioned decision ID, so a revocation is visible to the next decision immediately before execution.

### Approval verification

Approval records are immutable after issue and may be revoked. Records bind actor, exact capability/version, semantic input hash, policy version and expiry. Only a SHA-256 digest of the opaque high-entropy proof is stored. Verification compares the supplied proof digest without early exit and returns stable typed errors without exposing proof material.

### Rate limiting

The initial rate limiter uses concurrent tenant/actor/policy/capability/version-scoped fixed windows. Policy updates are versioned by store revision. Denials include a retry interval while internal counters and policy details remain outside the public message.

## Consequences

- test doubles are no longer required for composition of registry, authorization, approval and rate-limit stages;
- exact capability coordinates and policy bindings are enforced independently of transport code;
- authorization revocation is evaluated live on every request;
- approval secrets are not persisted in plaintext;
- rate-limit counters cannot leak across tenants, actors or capability versions;
- these in-process stores expose stable adapter contracts and can later be replaced with PostgreSQL or distributed implementations;
- authentication, complete `ExecutionContext` resolution, transport middleware and end-to-end PostgreSQL gateway tests remain required before issue #8 can close.
