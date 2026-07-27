# Customer Privacy production discovery and immutable snapshot implementation

Status: **Implemented; exact-head acceptance pending until all applicable permanent workflows pass on one unchanged source SHA.**

Historical source of truth remains `contracts/customer-privacy-discovery-snapshot-freeze.json` and `CUSTOMER_PRIVACY_DISCOVERY_SNAPSHOT_FREEZE.md`. This document records the later implementation without rewriting the historical `runtime_not_started` evidence from PR #204.

## Implemented boundary

The internal coordinate `customer_privacy.scope.discover@1.0.0` is implemented at phase `260` inside the accepted Customer Privacy golden packages:

```text
modules/crm-customer-privacy/
crates/crm-customer-privacy-application/
crates/crm-customer-privacy-postgres/
crates/crm-customer-privacy-production/
```

The package count remains `113`. No new crate, workspace package, external dependency family, direct dependency version, feature/source override or architecture exception is introduced.

The implementation is trusted-internal and activation-gated. `worker_internal` remains a frozen execution classification, not a generic-runtime worker registration. There is no public HTTP/gRPC route and no Customer Privacy worker.

## Runtime inventory preserved

- Customer Privacy public mutations: `4`;
- Customer Privacy permission-aware public queries: `2`;
- Customer Privacy workers: `0`;
- new public routes: `0`.

The existing generic-runtime metadata-only query adapter dependency is unchanged and Stage D consolidation is outside this packet.

## Exact-nine production orchestration

Production composition assembles the accepted `QueryExecutor` and exact `CapabilityDefinition` for all nine owner coordinates. Registry traversal remains canonical by:

1. `owner_module_id`;
2. `capability_id`;
3. `capability_version`.

The orchestration fails closed when an owner is missing, disabled, unavailable, stale, incompatible or descriptor-mismatched. It does not read owner storage directly and does not create a second registry or digest framework.

Every owner request receives the same immutable lineage:

- privacy case;
- tenant;
- canonical Party;
- Identity Resolution generation;
- registry version and SHA-256 digest;
- purpose code;
- effective request time.

Registry drift and Identity Resolution topology drift require a new explicit attempt. Silent rebasing is prohibited.

## Pagination and durable evidence

The frozen bounds are enforced:

- default page size `64`;
- maximum page size `128`;
- maximum cursor size `2048` bytes;
- first page number `1`;
- owner-specific cursors;
- empty next cursor on a terminal page;
- terminal completeness for every owner.

Every accepted page stores identifiers, lineage/page/cursor digests, scanned and emitted counts and terminal completeness. Exact response bytes are retained only as the governed reference-only Protobuf contribution envelope; resource payload and owner-private metadata are not part of that envelope.

Owner pages are append-only. Checkpoints may advance only across a contiguous durable prefix. Replay either resolves to the same logical page or fails closed as a conflict.

## Safe resource projection

Only the frozen projection is aggregated:

- owner module;
- resource type and id;
- resource version;
- data class;
- evidence class;
- retention policy id.

Exact duplicates are deduplicated. Conflicting classification for one resource identity fails closed. Resource payload and owner-private metadata are prohibited from the snapshot and audit evidence.

## Immutable snapshot identity and strict rehydration

Snapshot identity binds:

- the complete immutable lineage;
- captured nanosecond timestamp;
- deterministic aggregation snapshot id;
- completeness digest;
- ordered bound-owner contribution digests.

The id uses the prefix `privacy-discovery-scope-`. The finalized state uses:

- record type `customer-privacy.scope-snapshot`;
- schema `crm.customer-privacy.discovery_scope_snapshot.state@1.0.0`;
- maximum payload `524288` bytes;
- retention policy `crm.customer_privacy.discovery_scope_snapshot`;
- canonical JSON encoding `canonical_json_crm.cjson/v1`.

Rehydration reconstructs the lineage, contributions and deterministic aggregation, then recomputes and verifies the aggregation id, completeness digest, snapshot id and binding digest. Stored identity values are never trusted blindly.

## PostgreSQL model

Customer Privacy owns five FORCE-RLS evidence tables:

1. discovery attempts;
2. immutable owner pages;
3. mutable contiguous checkpoints;
4. immutable attempt-to-snapshot mappings;
5. immutable safe audit receipts.

The finalized snapshot itself is stored in the existing governed `crm.records` envelope. Final record insertion and attempt-to-snapshot mapping occur in one tenant-bound transaction. Triggers reject update/delete of immutable discovery evidence and finalized discovery snapshot records.

Tenant-bound transactions and FORCE RLS deny cross-tenant reads and writes. The dedicated PostgreSQL acceptance proves clean migration, same-tenant flow, cross-tenant negatives, immutable evidence, rollback, complete `crm` schema removal, reapply and repeated acceptance.

## Idempotency and crash recovery

Attempt identity binds tenant, case, canonical Party, topology generation, registry digest, purpose and effective request time.

Owner-page identity binds attempt digest, owner module, page number and request cursor digest.

Recovery semantics cover:

1. page persisted before checkpoint — replay reloads the durable page and advances the checkpoint;
2. all pages persisted before finalization — replay reconstructs the same contributions and finalizes the same snapshot;
3. snapshot finalized before later case transition — replay returns the same immutable snapshot. This packet intentionally does not perform that later case transition.

Conflicting page replay, corrupt evidence, invalid lineage, descriptor incompatibility and resource-classification conflicts are terminal. Temporary owner/storage unavailability and page-before-checkpoint recovery remain retryable.

## Authorization and audit

Snapshot ids are references, not authority. Internal reads call live visibility authorization and deny by default, including cross-tenant access.

Safe audit event types are:

- `discovery_started`;
- `owner_page_accepted`;
- `owner_terminal_complete`;
- `discovery_failed`;
- `snapshot_finalized`;
- `snapshot_read_allowed`;
- `snapshot_read_denied`.

Audit material contains only identifiers, digests, counts and policy references.

## Explicit non-effects

This packet does not implement:

- action planning or plan/outcome reads;
- approval;
- processing restrictions;
- legal-hold or mandatory-retention decisions;
- owner mutations or action execution;
- provider calls;
- access/export assembly;
- deletion or anonymization;
- Party tombstone;
- projection/search/cache convergence;
- destructive actions;
- Phase 8B.

Phase 8A therefore remains open. Product-complete expert modules remain `0` until the complete lifecycle and product wave are accepted.

## Acceptance evidence

Permanent acceptance is owned by `.github/workflows/customer-privacy-discovery.yml` plus the repository-wide applicable workflows. The machine-readable implementation record is `contracts/customer-privacy-discovery-snapshot-implementation.json`.

The next bounded packet after merge is deterministic planning and permission-aware plan/outcome reads. Restrictions, holds, retention and execution remain blocked until their own packets.
