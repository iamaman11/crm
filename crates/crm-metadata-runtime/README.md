# crm-metadata-runtime

Pure lifecycle runtime for immutable Admin Studio metadata publications.

## Public composition boundary

Application-facing callers use `TenantMetadataCatalog`.

The deterministic single-scope catalog engine is intentionally private to the crate. `TenantMetadataCatalog` maintains an isolated engine per `TenantId` and requires the tenant identity for publication, revision lookup, impact analysis, activation and rollback. A content hash is an identity, never an authorization secret.

Identical canonical content may produce the same deterministic revision identity in multiple tenants, but each tenant must publish that revision independently before it can be read, impact-analyzed or activated in that tenant's authority.

## Ownership

This crate owns generic metadata publication mechanics only:

- validated metadata coordinates;
- complete bundle snapshots;
- deterministic content-addressed revision identity;
- immutable/idempotent publication;
- structural impact analysis;
- tenant-scoped publication authority and optimistic activation;
- rollback by active-pointer movement across immutable revisions.

It does **not** own:

- object/field/layout/workflow business semantics;
- PostgreSQL or any other storage implementation;
- HTTP/gRPC transport;
- authentication or authorization policy;
- Admin Studio presentation state;
- business owner-module state.

Kind-specific metadata schemas and validators must feed canonical bytes into this runtime. Durable adapters must preserve the runtime's immutable revision, tenant publication authority and optimistic activation semantics rather than reimplementing weaker lifecycle rules.

## Core invariants

1. A published revision is immutable.
2. Revision identity is deterministic and independent of authoring insertion order.
3. A revision is a complete metadata snapshot; declared dependencies must resolve inside the same snapshot.
4. Publication authority is tenant-scoped; one tenant cannot use another tenant's publication merely by knowing its revision hash.
5. Tenant activation state references only revisions published in that tenant's authority.
6. Activation is guarded by an expected generation.
7. Removing metadata is structurally breaking and requires explicit confirmation before activation.
8. Rollback changes the active revision pointer/history only; historical revisions remain unchanged.
9. Tenant publication, activation generations and rollback histories are isolated.
