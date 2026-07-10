# ADR-025 — PostgreSQL Tenant and Transaction Foundation

Status: **Accepted**  
Issue: [#7](https://github.com/iamaman11/crm/issues/7)

## Context

The platform requires tenant isolation, atomic mutation evidence, an append-only audit chain, immutable published module versions and controlled dynamic data. Application conventions alone are insufficient: the database must reject important invariant violations even when a platform adapter is defective.

## Decision

### 1. Transaction-local execution context

Trusted platform adapters bind the following PostgreSQL custom settings with `SET LOCAL` inside every transaction:

- `app.tenant_id`
- `app.actor_id`
- `app.request_id`
- `app.capability_id`
- `app.capability_version`
- `app.business_transaction_id`

Mutable tenant tables use a common trigger to reject writes when the context is missing, when the row tenant differs from the bound tenant, or when a row transaction identifier differs from the bound business transaction.

Business modules never receive a database connection and therefore cannot change this context or issue SQL.

### 2. Forced row-level security

Every tenant-scoped table contains a non-null `tenant_id`, enables RLS and uses `FORCE ROW LEVEL SECURITY`. Policies compare the row tenant with the transaction-local tenant context. Absence of tenant context yields no tenant rows.

Platform-global catalogs are explicitly identified and are not made accidentally tenant-global by nullable tenant columns.

### 3. Atomic transaction evidence

A state-changing capability transaction writes, in one PostgreSQL transaction:

- business state;
- an idempotency record;
- one or more outbox events;
- one or more audit records;
- an immutable `business_transactions` completion marker.

Foreign keys to the completion marker are deferred until commit. A deferred constraint trigger verifies that the marker's declared event, audit and idempotency counts equal the durable rows. Missing evidence aborts the database transaction.

This does not replace the capability execution gateway; it gives the gateway a database-enforced commit protocol.

### 4. Canonical payload discipline

Dynamic payload columns are permitted only together with owner, schema identifier, schema version, descriptor hash, data class, maximum size and retention policy. Payload length and hash length are constrained by PostgreSQL.

JSONB is allowed only for bounded typed projections or attributes and is not the authoritative business payload.

### 5. Audit chain

`audit_records` is insert-only. Each tenant has a locked `audit_heads` row containing the next expected sequence and previous hash. A security-definer trigger atomically advances that head only when sequence and previous hash match. Updates and deletes are rejected.

Canonical audit envelopes and their hashes are computed by the governed application layer; PostgreSQL verifies chain continuity and immutability.

### 6. Published module immutability

Published module versions and their dependency declarations are platform-global immutable catalog rows. Re-publication of the same coordinate is handled by the registry domain before persistence; SQL rejects mutation after insertion.

### 7. Migration ownership

SQL migrations are the sole authority for PostgreSQL schema evolution. Production runtime roles do not own schemas or tables. CI applies migrations to a clean PostgreSQL instance and verifies RLS, context enforcement, immutability, audit continuity and deferred transaction evidence.

## Consequences

- Tenant isolation and transaction completeness are testable database properties.
- Operational and migration code must distinguish platform-global catalogs from tenant data.
- Every write adapter must bind a complete context and completion marker.
- Audit insertion is serialized per tenant audit head; later benchmarks determine whether partitioning is required.
- Tenant-level restore must restore tenant rows and audit head consistently.

## Rejected alternatives

- **Application predicates only:** too easy to omit and impossible to prove centrally.
- **Nullable tenant identifiers for global rows:** creates ambiguous policy and indexing behavior.
- **Direct module-owned tables and pools:** violates module independence and governance.
- **Search or analytics as authoritative state:** breaks rebuild and restore guarantees.
- **Mutable audit rows:** destroys evidentiary value.
