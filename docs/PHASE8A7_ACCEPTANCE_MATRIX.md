# Phase 8A.7 Production Acceptance Matrix

## Status

Normative acceptance matrix for the customer Party import production packet.

A capability is not considered complete because its domain types or adapters compile. Phase 8A.7 completes only when the governed source, validation, execution and restart paths are demonstrated against fresh PostgreSQL and the real application runtime.

## A. Immutable source artifact

### A1. Artifact creation

- create through `customer_data.import.party.source.create@1.0.0`;
- immutable expected size and SHA-256 are frozen;
- artifact is tenant scoped and owned by `crm.customer-data-operations`;
- capability idempotency, audit and business-transaction completion evidence are durable.

### A2. Chunk append

- chunks are non-empty and bounded;
- exact sequential chunk index is required;
- chunk SHA-256 must match the exact bytes;
- identical replay of an already accepted chunk is safe;
- conflicting replay is rejected;
- out-of-order append is rejected;
- a real append commits artifact state, outbox event, audit and idempotency completion atomically.

### A3. Finalization

- finalization is rejected before declared byte length is complete;
- chunk indexes must be contiguous;
- every chunk digest is re-verified;
- ordered full bytes must match declared SHA-256;
- finalized artifact is immutable;
- finalized reads re-verify exact bytes;
- cross-tenant reads disclose no artifact.

### A4. Transaction rollback

A synthetic failure after artifact mutation but before evidence completion must leave:

- zero committed artifact mutation;
- zero idempotency claim;
- zero outbox event;
- zero audit record;
- zero business-transaction completion marker.

## B. Artifact-backed import job creation

- only a finalized artifact may create a production import job;
- exact bytes are parsed server-side;
- parser profile is immutable job input, not generic file metadata;
- row count is derived server-side;
- `SourceDescriptor` stores exact artifact ID and SHA-256;
- source-system external identity remains distinct from canonical Party identity;
- old client-preparsed `customer_data.import.party.create` is absent from the public production catalog.

## C. Server-side validation

- exact finalized bytes bound to the job are re-read;
- SHA-256 is compared to the job binding;
- exact immutable parser profile is used;
- parsed row count must match the job binding;
- requested source positions are bounded and one-based;
- client-preparsed maps are not accepted as authoritative production input;
- job progress, row records, job-to-row relationships, row events, audit and idempotency commit atomically;
- live authorization is repeated after file read and parsing and immediately before the atomic write;
- dry-run/validation creates zero target Party mutation side effects.

## D. Validation finalization

- `valid_rows + invalid_rows` is server-derived;
- finalization succeeds only when the authoritative counters cover immutable `total_rows`;
- stale optimistic version is rejected;
- restart after partial validation preserves counters and continues safely.

## E. Execution

- execution order is source `row_position`, never relationship pagination order;
- invalid rows are skipped only under the allowed partial-execution policy;
- valid/retryable rows invoke exact `parties.party.create@1.0.0` through `GatewayCapabilityClient`;
- import code never writes Party storage directly;
- target idempotency key is deterministic per import row;
- target result must identify the prepared Party;
- success atomically persists row success and exact next checkpoint;
- retryable failure persists row failure evidence without advancing checkpoint;
- completion is durable and terminal.

## F. Crash and restart

### F1. Target success before import checkpoint

1. governed Party create commits;
2. process stops before import-owned success/checkpoint commit;
3. application restarts;
4. worker loads the same job and same next row;
5. exact Party create input and deterministic idempotency key are repeated;
6. Party capability replays without creating a duplicate Party;
7. import-owned row success and checkpoint are rebuilt durably.

Expected evidence:

- exactly one Party record;
- exactly one authoritative Party create side-effect set;
- import row eventually `Succeeded`;
- checkpoint eventually advances exactly once.

### F2. Retryable dependency failure

- retryable target failure persists bounded failure evidence;
- checkpoint does not advance;
- restart or next worker cycle retries the same row;
- later success uses the same deterministic target identity/idempotency semantics.

## G. Tenant and query security

- import jobs and rows are tenant isolated;
- source artifacts are tenant isolated;
- worker grants are bound to dedicated worker actor identity;
- private outcome capabilities are absent from public HTTP/gRPC mutation catalogs;
- signed cursor tampering is rejected;
- cursor cannot be replayed across tenant, actor, capability, filter or page-size binding.

## H. Migration lifecycle

On a fresh database:

1. apply all migrations;
2. run file/import acceptance;
3. remove acceptance data;
4. roll back migrations in supported order;
5. verify authoritative schema removal;
6. reapply migrations;
7. rerun the production path.

Rollback of immutable file storage must refuse while retained artifacts exist.

## I. Final exact-head gate

Before PR #121 may leave draft state:

- source and documentation are frozen at one candidate SHA;
- all applicable workflows pass on that unchanged SHA;
- no source-changing automation commits after the candidate gate begins;
- PR body and issue #120 reflect the exact merged-ready boundary;
- no later Phase 8A packet is started in the same dependency lane.
