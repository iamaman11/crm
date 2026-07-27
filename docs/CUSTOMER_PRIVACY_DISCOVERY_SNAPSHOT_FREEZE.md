# Customer Privacy Scope Discovery and Immutable Snapshot Freeze

Status: **Normative contract and acceptance boundary; production runtime not started**  
Phase: 8A.11  
Tracking issue: #126  
Architecture program: #194  
Parent architecture freeze: `contracts/customer-privacy-architecture-freeze.json`  
Machine-readable packet: `contracts/customer-privacy-discovery-snapshot-freeze.json`

## Objective

Freeze the complete semantic boundary for `customer_privacy.scope.discover@1.0.0` before Stage C packaging or production discovery begins.

The packet makes one previously implicit requirement executable: an immutable snapshot is not identified only by owner resources and topology. It is also bound to the exact privacy case, registry, purpose and effective request time that authorized discovery. A change in any of those values creates a different snapshot lineage and cannot silently reuse or rebase prior evidence.

This packet adds no public route, worker registration, PostgreSQL migration, provider call, owner mutation, restriction, legal-hold decision, retention decision, action plan or destructive action.

## Exact owner registry

The canonical registry remains `crm.customer-privacy.scope-registry/1.0.0` and contains exactly nine independently accepted owner coordinates:

1. `crm.consents` → `consents.privacy.scope.contribute@1.0.0`;
2. `crm.contact-points` → `contact_points.privacy.scope.contribute@1.0.0`;
3. `crm.customer-accounts` → `customer_accounts.privacy.scope.contribute@1.0.0`;
4. `crm.customer-data-operations` → `customer_data.privacy.scope.contribute@1.0.0`;
5. `crm.customer-enrichment` → `customer_enrichment.privacy.scope.contribute@1.0.0`;
6. `crm.data-quality` → `data_quality.privacy.scope.contribute@1.0.0`;
7. `crm.identity-resolution` → `identity_resolution.privacy.scope.contribute@1.0.0`;
8. `crm.parties` → `parties.privacy.scope.contribute@1.0.0`;
9. `crm.party-relationships` → `party_relationships.privacy.scope.contribute@1.0.0`.

Ordering is canonical by owner module ID, capability ID and capability version. Registry version and digest are immutable inputs to every owner page and the final snapshot. An unavailable, disabled, stale, incompatible or descriptor-mismatched owner fails discovery closed.

## Discovery lineage

Every invocation, owner page, durable receipt and final snapshot binds the same fields:

```text
privacy_case_id
tenant_id
canonical_party_id
identity_resolution_generation
registry_version
registry_digest_sha256
purpose_code
effective_request_at_unix_ms
```

`purpose_code` is an explicit bounded policy value, not free text. `effective_request_at_unix_ms` is the legal/product time at which the requested scope is evaluated; it is not replaced by worker execution time.

Identity Resolution topology drift produces an explicit rescope-required result. Registry drift starts a new explicit lineage. Neither drift can silently mutate an in-flight or finalized snapshot.

## Bounded pagination and receipts

Each owner is called deterministically and paginated independently:

- default page size: 64;
- maximum page size: 128;
- maximum cursor size: 2,048 bytes;
- page numbering starts at 1;
- a terminal page has an empty next cursor and `terminal_complete = true`;
- a snapshot cannot finalize until every registered owner proves terminal completeness.

A durable owner-page receipt binds owner coordinate, full lineage digest, page number, request and response cursor digests, page digest, scanned/emitted counts and terminal state. Replay must return the same logical page or fail closed on conflict. A checkpoint advances only across a contiguous durable page prefix.

## Safe resource projection

Discovery records references and classifications only:

```text
owner_module_id
resource_type
resource_id
resource_version
data_class
evidence_class
retention_policy_id
```

Resource payloads and owner-private metadata are prohibited. Exact duplicate references are deduplicated. A duplicate identity with a different version or classification is a terminal conflict.

Resources are ordered by owner, resource type, resource ID, version, data class, evidence class and retention policy. Owner result arrival order never changes the snapshot.

## Digest and snapshot identity

The existing deterministic aggregation remains authoritative for normalized owner resources and terminal completeness. This packet adds full-lineage binding around it.

Versioned digest profiles are:

```text
crm.customer-privacy.scope-registry/v1
crm.customer-privacy.scope-contribution/v1
crm.customer-privacy.discovery-owner-contribution/v1
crm.customer-privacy.scope-completeness/v1
crm.customer-privacy.scope-snapshot/v1
crm.customer-privacy.discovery-lineage/v1
crm.customer-privacy.discovery-snapshot/v1
```

The authoritative discovery snapshot identity includes:

- every discovery-lineage field;
- capture time;
- deterministic aggregation snapshot ID;
- completeness digest;
- ordered bound-owner contribution digests.

Changing purpose or effective request time changes the authoritative snapshot ID even when the discovered resource set is identical.

## Persistence boundary

The pure owner module exposes strict canonical state for the bound snapshot:

```text
record type: customer-privacy.scope-snapshot
schema: crm.customer-privacy.discovery_scope_snapshot.state@1.0.0
encoding: crm.cjson/v1
maximum: 524,288 bytes
retention: crm.customer_privacy.discovery_scope_snapshot
```

Rehydration rejects unknown fields, noncanonical numbers/JSON, identity or digest drift, mismatched registry content, lineage/aggregation mismatch and oversized state. Finalized snapshots are append-only evidence.

The Stage C pilot must place this contract in the accepted Customer Privacy PostgreSQL ownership boundary. This freeze does not add a migration.

## Failure, retry and crash semantics

Discovery fails closed for missing/nonterminal owners, page or cursor digest mismatch, page replay conflict, registry/topology drift, resource conflict and corrupt persisted evidence.

Transient owner or storage unavailability is retryable with the same discovery-attempt and owner-page keys. Mandatory crash windows are:

1. owner page persisted before checkpoint;
2. all owner pages persisted before snapshot finalization;
3. snapshot finalized before privacy-case state transition.

Recovery reuses the same logical lineage and must produce the same page/snapshot identity or an explicit conflict. It never skips an owner or silently accepts partial completeness.

## Authorization and disclosure

Discovery invocation is trusted, activation-gated and internal. Snapshot reads use a separate permission-aware internal query boundary with live authorization, tenant binding, resource visibility and audit.

Possession of a case ID or snapshot ID is never authority. Audit evidence contains identifiers, coordinates, digests, counts and policy versions only—never owner resource payloads.

No new public HTTP/gRPC route is introduced. Existing public Customer Privacy route inventory and zero-worker runtime state remain unchanged.

## Acceptance boundary for later runtime implementation

Production discovery cannot be accepted until one unchanged exact head proves:

- exact nine-owner registry and contract compatibility;
- deterministic page/cursor/terminal completeness behavior;
- purpose/effective-time-bound snapshot identity;
- fail-closed unavailable, disabled, stale and incompatible owners;
- registry and topology drift recovery;
- permission-aware snapshot reads and audit;
- clean PostgreSQL with FORCE RLS and cross-tenant negatives;
- complete schema removal, reapply and repeated acceptance;
- crash-window/idempotent replay;
- real-process discovery without planning, retention or owner actions;
- exact public/worker/non-runtime route parity.

## Explicit exclusions

This packet does not implement planning, approval, restrictions, legal holds, retention evaluation, owner action dispatch, export assembly, deletion/anonymization, convergence or case finalization.

It does not add `crm-customer-privacy-application`, `crm-customer-privacy-postgres` or `crm-customer-privacy-production`. The next packet is the separate behavior-neutral Stage C Customer Privacy golden-package pilot. Runtime discovery remains blocked until that package boundary is accepted.
