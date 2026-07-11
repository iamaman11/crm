# ADR-034: Transaction-aware aggregate planning

- Status: Accepted
- Date: 2026-07-11
- Phase: 6C

## Context

A mutation planner that receives only the client request can safely create a new aggregate, but it cannot safely update an existing aggregate. Update, stage-transition, completion and reminder decisions depend on the current authoritative version and state. Reading that state before live authorization or outside the mutation transaction creates a time-of-check/time-of-use race. Treating a client-supplied snapshot as authoritative would let stale or forged state influence business decisions.

The existing batch idempotency response also persisted only database mutation evidence. A transaction-aware executor must check replay before loading or planning the aggregate, and a later replay must return the original capability output even if the aggregate has subsequently changed.

An absent aggregate has no row that PostgreSQL can protect with `FOR UPDATE`. A create planner that only checks absence can therefore race with another create for the same tenant, owner module and record identity unless the target identity itself is serialized transactionally.

## Decision

Introduce `TransactionalAggregatePlanner` with two synchronous, no-I/O operations:

1. resolve the typed aggregate target and whether it must exist or be absent;
2. build a deterministic capability batch from the authoritative `RecordSnapshot` supplied by the platform.

`PostgresTransactionalAggregateExecutor` is the first awaited operation after live authorization. It:

1. binds the complete execution context;
2. checks capability-scoped idempotency replay;
3. claims the idempotency key;
4. acquires a tenant-, owner-module- and record-scoped transaction advisory lock;
5. loads any existing owner-module record with `SELECT ... FOR UPDATE`;
6. validates target presence;
7. invokes the pure domain planner;
8. verifies that the planned create/update matches the locked target and version;
9. atomically writes state, relationships, outbox, audit, the full capability response and the transaction completion marker.

The advisory identity uses length-prefixed tenant, owner-module, record-type and record-id components with a dedicated lock namespace. A hash collision can only cause additional serialization; it cannot permit two conflicting target mutations to proceed concurrently. Existing rows retain their row lock as an independent database-level guard.

Capability replay responses use the internal immutable schema `crm.core.data.capability_execution_result@1.0.0`. The stored payload includes output and affected-resource versions. Replay changes only the transient `replayed` flag.

PostgreSQL acceptance scenarios that intentionally share the repository fixture tenant acquire a dedicated test-only advisory transaction lock. This serializes only those test cases so one scenario cannot advance the fixture audit head between another scenario's assertions. Production aggregate concurrency relies on the per-target advisory lock, the existing-row lock, capability idempotency and the tenant audit-chain serialization defined by ADR-033.

## Consequences

- Business modules remain free of SQL and infrastructure clients.
- Stale client snapshots cannot become authoritative inputs to domain transitions.
- Competing updates serialize on the aggregate target and optimistic domain checks remain deterministic.
- Competing creates for one absent target produce one commit and one stable typed conflict rather than a storage error.
- Replay returns the original response rather than reconstructing output from later state.
- The older deterministic batch planner remains supported for create-only or fully self-contained mutations.
- Sales and Activities persisted codecs and planners can now be added without weakening the live-authorization or atomic-evidence invariants.
