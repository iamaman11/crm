# Metadata publication tenancy boundary

`TenantMetadataCatalog` is the application-facing in-memory composition boundary for metadata publication lifecycle rules.

## Threat model

A `MetadataRevisionId` is a deterministic SHA-256 content identity. It may be logged, correlated, cached or otherwise become known outside the tenant that first published the underlying bundle. Knowledge of that identifier must never grant revision read or activation authority.

## Required composition rule

All publication, revision lookup, impact analysis, activation and rollback operations are bound to an explicit `TenantId` through `TenantMetadataCatalog`.

The crate's deterministic single-scope engine is private and exists only so each tenant authority can reuse the same lifecycle rules. Future PostgreSQL and service-layer adapters must preserve the same boundary:

- durable revision ownership is tenant-scoped;
- tenant B cannot activate a revision solely because the same content exists for tenant A;
- identical content may retain the same deterministic revision identity after independent publication in multiple tenant authorities;
- activation generations and rollback histories are tenant-isolated;
- authorization must not rely on revision-hash secrecy.

This boundary is required before metadata persistence, public publication APIs or Admin Studio workflows are composed.
