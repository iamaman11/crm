# ADR-027: PostgreSQL Capability Executor Boundary

- Status: Accepted
- Date: 2026-07-11

## Context

ADR-026 requires live authorization to be the final awaited decision before transactional side effects. The existing PostgreSQL batch runtime already provides tenant-bound execution context, idempotent replay, optimistic versions, record and relationship mutation, outbox evidence, audit continuity and atomic rollback. The capability layer needs to reuse that runtime without inserting an ungoverned asynchronous planning step or allowing capability metadata to drift from persisted idempotency evidence.

## Decision

`crm-core-data` provides `PostgresTransactionalCapabilityExecutor` as the concrete implementation of the capability runtime executor port.

The adapter uses a synchronous `CapabilityBatchPlanner`. Planner implementations must be pure and deterministic: they may transform an already validated capability request into one `BatchMutationPlan`, but they may not perform I/O, read clocks or obtain non-deterministic randomness. Consequently, the first awaited operation after live authorization remains the existing PostgreSQL `execute_batch` call.

Before the database runtime is called, the adapter verifies all of the following:

1. the capability is state-changing and requires idempotency;
2. the planned execution context is exactly equal to the gateway request context;
3. the idempotency scope is derived from the exact capability ID and version;
4. the idempotency key equals the request execution context key;
5. the persisted request hash equals the gateway semantic input hash;
6. the complete batch plan satisfies the existing PostgreSQL runtime invariants;
7. the planned output is valid and matches the declared output contract.

The database runtime then executes exactly one atomic batch. The adapter derives affected resource references from the persisted batch result and propagates its replay flag. Batch failures are mapped to stable typed SDK codes; public messages do not contain SQL details or require parsing human text.

## Idempotency identity

The canonical scope is:

`capability:<capability-id>:<capability-version>`

Tenant identity remains part of the database key, and the request idempotency key supplies the caller-selected identity within that capability version. Reusing the same key with a different semantic input hash is rejected by the existing batch runtime.

## Consequences

- capability execution reuses the proven PostgreSQL transaction, replay, outbox and audit machinery;
- a planner cannot substitute another tenant, actor, capability, version, idempotency key or semantic request hash;
- output-contract failures are detected before side effects commit;
- replay returns persisted mutation results without repeating side effects;
- authorization ordering remains mechanically meaningful because no awaited planning work occurs before the database batch;
- concrete business capability planners and end-to-end gateway tests are still required before public mutation transports are enabled.
