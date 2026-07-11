# ADR-036: Audited aggregate no-op transactions

- Status: Accepted
- Date: 2026-07-11
- Phase: 6F foundation

## Context

Some mutation capabilities are semantically idempotent beyond transport replay. Completing a task that is already completed, or scheduling the reminder value that is already stored at the requested optimistic version, returns `changed = false`. Emitting another completion or reminder event would create false domain evidence. Mutating the row only to satisfy a generic batch requirement would also create an incorrect new aggregate version.

The authorized request still needs an immutable idempotency result, canonical audit evidence and an atomic completion marker.

## Decision

The transaction-aware aggregate executor accepts an explicit audited no-op only when:

- the target presence is `MustExist`;
- the authoritative target is locked in the PostgreSQL transaction;
- there are no record, relationship or outbox mutations;
- the capability/version-bound idempotency evidence is valid;
- at least one canonical audit intent is present;
- the typed output matches the declared output contract.

Generic batch execution remains mutation-only and still requires state or relationship mutation plus outbox evidence. Migration `0005_audited_noop_transactions` permits `expected_outbox_events = 0`; audit and idempotency counts remain strictly positive and are still verified by the deferred transaction-evidence constraint.

Rollback of migration 0005 is refused while immutable zero-outbox transaction history exists.

## Consequences

- A semantic no-op never publishes a misleading domain event or advances the aggregate version.
- Replay returns the original `changed = false` response without new audit, state or outbox effects.
- The no-op decision is still based on the authoritative locked aggregate and live authorization.
- Transactions that actually change state continue to require outbox and audit evidence atomically.
