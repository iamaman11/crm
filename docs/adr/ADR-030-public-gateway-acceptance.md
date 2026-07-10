# ADR-030: Public Gateway Acceptance and No-Bypass Enforcement

- Status: Accepted
- Date: 2026-07-11

## Context

The capability gateway, PostgreSQL executor, live policy adapters and authenticated ingress are individually tested. Issue #8 additionally requires system evidence that public transports cannot bypass governance and that replay, authorization and rollback invariants survive the complete authentication-to-database composition.

## Decision

The platform treats `crm-capability-ingress` as the only runtime dependency available to `crm-api`. The architecture policy now defines explicit runtime dependency allowlists for both crates and scans production source for direct data-store, batch-runtime, transactional-executor and business-module mutation markers. Development dependencies and code below `#[cfg(test)]` remain available for acceptance composition but cannot enter production transport source.

Database CI runs one sequential public-boundary scenario after the established PostgreSQL foundation and advanced adapter suites. The scenario composes:

1. hashed bearer authentication and tenant membership;
2. complete execution-context resolution;
3. the immutable capability catalog;
4. tenant-scoped fixed-window rate limiting;
5. stored approval verification;
6. live authorization grants;
7. the deterministic gateway;
8. the PostgreSQL transactional capability executor;
9. the existing batch runtime, RLS, idempotency, outbox, audit and completion-marker constraints.

## Required evidence

The acceptance scenario must prove all of the following through `HttpCapabilityMiddleware`:

- invalid credentials and cross-tenant requests produce no authorization or database call;
- a successful mutation reaches the real PostgreSQL batch only after a successful live authorization decision;
- replay with the same tenant, capability version, idempotency key and semantic input returns the persisted result with `replayed = true`;
- replay creates no additional record, outbox event, audit record, idempotency row or business transaction;
- reuse of the idempotency key with a different semantic input returns a stable conflict code without new side effects;
- revoking the authorization grant is visible to the next request, which records authorization but never enters the batch runtime;
- a fault that omits required outbox evidence reaches the real transaction, returns only a safe storage error and rolls back every mutation and evidence row;
- rollback leaves the tenant audit head unchanged;
- transport-safe errors contain stable codes and safe messages rather than SQL, policy internals or dependency details.

## Consequences

- public mutation code cannot acquire a direct PostgreSQL, batch-executor or business-module mutation dependency without failing architecture CI;
- gateway order and live revocation are verified against the real transaction boundary rather than only test doubles;
- replay and rollback are measured by actual persisted evidence deltas;
- issue #8 may close only when Governance, Rust and Database CI all pass this acceptance scenario and no review threads remain;
- this acceptance does not imply that the complete CRM product or later vertical slices are finished.
