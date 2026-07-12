# Metadata Publication Persistence

Status: Phase 7F persistence contract.

This document defines the durable boundary between the pure Admin Studio metadata lifecycle runtime and PostgreSQL.

## Source of truth

`crm-metadata-runtime` remains the semantic authority for:

- deterministic revision identity;
- complete bundle validation;
- structural impact analysis;
- breaking-change classification;
- optimistic activation semantics;
- rollback behavior.

`crm-core-data::PostgresMetadataStore` is an adapter. It persists and reconstructs those semantics; it must not invent a second SQL-only interpretation of metadata lifecycle rules.

## Durable model

Migration `0010_metadata_publication_runtime` stores:

- immutable tenant-scoped revision headers;
- immutable canonical documents;
- immutable explicit dependency edges;
- one mutable tenant activation head guarded by a monotonically increasing generation;
- a mutable operational rollback stack with push/pop semantics;
- append-only publish, activate and rollback transition evidence.

The `metadata_revisions_v2` name intentionally distinguishes the immutable revision model from the earlier Phase 4 metadata package/object/field foundation. A future compatibility/cutover packet may retire the legacy authoring tables only after governed APIs and Admin Studio consumers have migrated; Phase 7F does not silently repurpose them.

## Tenant authority

A revision hash is a content identity, not an authorization secret.

Every persisted revision is keyed by `(tenant_id, revision_id)`. Revision lookup, impact analysis, activation and rollback run under transaction-local tenant context and FORCE RLS. Tenant B cannot use a revision published only by Tenant A even when both parties know the same SHA-256 identifier.

Identical content may have the same deterministic revision identity in multiple tenants only after independent publication into each tenant authority.

## Publication transaction

A new publication transaction:

1. validates the execution context;
2. computes the deterministic runtime revision identity before SQL;
3. inserts the tenant revision header idempotently;
4. inserts canonical documents and dependency edges in the same transaction;
5. records one append-only publish transition;
6. commits atomically.

Re-publication of identical content returns the existing revision and does not duplicate documents or transition evidence. Reconstruction from PostgreSQL recomputes the runtime revision identity and fails closed on mismatch.

## Activation transaction

Activation serializes per tenant with a PostgreSQL transaction advisory lock and then checks the expected generation under the same transaction.

The adapter reconstructs the active and candidate bundles and delegates impact analysis to `crm-metadata-runtime`. Breaking changes require explicit confirmation before the active pointer can move.

A successful non-initial activation:

1. pushes the previous active revision onto the rollback stack;
2. increments the activation generation;
3. updates the active pointer and rollback depth;
4. appends activation transition evidence;
5. commits atomically.

A stale writer receives an explicit generation conflict.

## Rollback transaction

Rollback uses the same tenant serialization and expected-generation check.

A successful rollback:

1. reads the exact top rollback-stack entry;
2. removes that entry;
3. moves the active pointer to the popped immutable revision;
4. increments the generation and decreases rollback depth;
5. appends rollback transition evidence;
6. commits atomically.

The replaced revision is not pushed during rollback. Therefore repeated rollback cannot toggle the tenant forward to the revision that was just rolled back.

## Evidence boundary

Phase 7F writes append-only metadata transition evidence containing:

- tenant;
- action;
- generation and rollback depth;
- from/to revision identities;
- actor;
- request;
- capability id/version;
- business transaction id;
- occurrence time.

This persistence adapter deliberately does not fabricate unrelated outbox or idempotency rows merely to satisfy the existing global business-transaction audit chain. The follow-on governed metadata capability/API layer must produce canonical `crm.audit_records` evidence through the normal capability execution contract.

## Database enforcement

All six Phase 7F metadata tables have tenant RLS enabled and forced. All writes require the transaction-local execution context. Published revision headers, documents, dependency edges and transition evidence reject UPDATE and DELETE through application writes.

The rollback stack is intentionally mutable operational state: activation pushes entries and rollback pops them. The activation head is intentionally mutable but guarded by tenant serialization and optimistic generation.

## Verification

`Metadata Runtime CI` proves against real PostgreSQL:

- clean migration through `0010`;
- deterministic publish/read round-trip;
- dependency preservation;
- idempotent re-publication;
- cross-tenant non-disclosure;
- stale generation rejection;
- breaking-change confirmation;
- concurrent activation serialization;
- rollback without forward toggling;
- FORCE RLS;
- immutable revision rejection;
- migration rollback and reapply.

General Rust, Governance, Database and specialized runtime gates remain required on the exact final review head before merge.
