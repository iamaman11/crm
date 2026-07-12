# crm-metadata-runtime

Pure lifecycle runtime for immutable Admin Studio metadata publications.

## Ownership

This crate owns generic metadata publication mechanics only:

- validated metadata coordinates;
- complete bundle snapshots;
- deterministic content-addressed revision identity;
- immutable/idempotent publication;
- structural impact analysis;
- tenant-scoped optimistic activation;
- rollback by active-pointer movement across immutable revisions.

It does **not** own:

- object/field/layout/workflow business semantics;
- PostgreSQL or any other storage implementation;
- HTTP/gRPC transport;
- authentication or authorization policy;
- Admin Studio presentation state;
- business owner-module state.

Kind-specific metadata schemas and validators must feed canonical bytes into this runtime. Durable adapters must preserve the runtime's immutable revision and optimistic activation semantics rather than reimplementing weaker lifecycle rules.

## Core invariants

1. A published revision is immutable.
2. Revision identity is deterministic and independent of authoring insertion order.
3. A revision is a complete metadata snapshot; declared dependencies must resolve inside the same snapshot.
4. Tenant activation state references only published revisions.
5. Activation is guarded by an expected generation.
6. Removing metadata is structurally breaking and requires explicit confirmation before activation.
7. Rollback changes the active revision pointer/history only; historical revisions remain unchanged.
8. Tenant activation histories are isolated.
