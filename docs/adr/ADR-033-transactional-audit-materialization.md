# ADR-033: Transactional Audit-Chain Materialization

- Status: Accepted
- Date: 2026-07-11

## Context

The Phase 5 capability executor intentionally requires a synchronous, deterministic, no-I/O planner so that the real PostgreSQL batch remains the first awaited operation after live authorization. The existing batch contract nevertheless required planners to provide `audit_sequence`, `previous_hash` and `record_hash` before the transaction began.

That contract is unsafe for production concurrency. The next audit sequence and previous hash are tenant-global chain state. Two authorized mutations can observe the same audit head if it is read before execution, and a planner cannot correctly reserve a chain position without performing I/O or holding a database lock. Allowing a pre-execution read would weaken the established authorization boundary; guessing the values would create avoidable serialization failures and make domain planners responsible for infrastructure state.

## Decision

Business and capability planners provide only a validated `AuditIntent`:

- stable audit record identity;
- canonicalization profile identity;
- canonical audit envelope bytes;
- occurrence time.

The PostgreSQL runtime materializes the chain inside the same transaction that persists business state, idempotency, outbox and completion evidence.

For every non-replayed mutation the runtime:

1. binds the tenant/actor/capability execution context;
2. claims idempotency and applies the planned business mutations;
3. acquires a transaction-scoped advisory lock derived from the tenant identity and a fixed audit namespace;
4. reads `crm.audit_heads` inside that transaction;
5. assigns contiguous audit sequence numbers and previous hashes;
6. normalizes occurrence time to PostgreSQL's persisted microsecond precision and computes each record hash with SHA-256 over a versioned, length-prefixed envelope containing the persisted audit identity, tenant, sequence, business transaction, actor, capability, canonicalization profile, previous hash, canonical envelope and normalized occurrence time;
7. inserts the materialized audit records in order;
8. commits only after the existing deferred transaction-evidence constraint confirms the required event, audit and idempotency counts.

The hash domain separator is `crm.audit.record.sha256/v1`. The timestamp component is the Unix nanosecond value truncated to a whole microsecond, matching PostgreSQL `timestamptz` persistence, so an independent verifier can reproduce the hash from stored rows. Changing field order, field set, timestamp normalization, length-prefix rules or digest algorithm requires a new hash profile and must not reinterpret existing records.

The same materialization path is used by both the multi-record batch runtime and the legacy single-record adapter so no supported PostgreSQL mutation API requires callers to supply chain position or chain hash.

## Concurrency and failure semantics

The advisory lock serializes audit-chain allocation per tenant, not globally. Hash collisions can only cause unnecessary cross-tenant serialization; they cannot merge chains because every query and row remains tenant-scoped and protected by RLS and the existing audit trigger.

If state mutation, outbox insertion, audit insertion, idempotency completion or the deferred evidence check fails, PostgreSQL rolls back the business changes and the audit-head transition together. Replay returns the stored result before acquiring the audit lock and therefore creates no additional audit record.

## Consequences

- live authorization remains the last awaited dependency before one PostgreSQL execution;
- deterministic planners no longer read or predict infrastructure-owned audit state;
- concurrent same-tenant mutations produce one contiguous append-only chain;
- audit hashes cover the persisted identity and execution metadata rather than only opaque planner-provided bytes;
- stored audit rows contain all information required to reproduce their hash profile;
- existing table shape and triggers remain compatible, so no migration is required;
- all planner constructors and tests must migrate from `AuditEvidence` to `AuditIntent`;
- Database CI must prove concurrent allocation, rollback preservation and replay without chain growth before merge.
