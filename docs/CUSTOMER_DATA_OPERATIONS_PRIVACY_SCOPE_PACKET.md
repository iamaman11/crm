# Customer Data Operations Privacy Scope Owner Packet

Status: **Accepted historical contract**  
Parent program: #126  
Implementation PR: #188  
Accepted source: `07f34786e82fdfa78d263790e9f50541529006f8`  
Merge: `089be72fa3010b4aa15aff7f9ea55fd86290f8fc`  
Exact-head gate: **26 of 26 permanent workflows succeeded**  
Coordinate: `customer_data.privacy.scope.contribute@1.0.0`  
Runtime state: **Contract-only/non-runtime; no route, worker or application registration**

## 1. Accepted objective

PR #188 implemented `crm.customer-data-operations` as the seventh authoritative privacy-scope owner contribution without treating multi-subject import/export containers as subject-owned records.

The accepted packet proves subject discovery across import-row evidence and export-selection/execution evidence while preserving strict owner persistence, bounded canonical resolution, deterministic heterogeneous pagination, reference-only output and zero writes.

## 2. Accepted authoritative owner boundary

`crm.customer-data-operations` owns these persisted record types:

1. `customer_data.import_job`;
2. `customer_data.import_row`;
3. `customer_data.export_job`;
4. `customer_data.export_selection_boundary`;
5. `customer_data.export_selection_progress`;
6. `customer_data.export_selection_item`;
7. `customer_data.export_execution_stage`;
8. `customer_data.export_execution_outcome`.

The privacy contribution emits only the four subject-level resource families:

1. `customer_data.import_row`;
2. `customer_data.export_selection_item`;
3. `customer_data.export_execution_stage`;
4. `customer_data.export_execution_outcome`.

The following remain excluded merely because one child record is relevant:

- import jobs and source artifacts;
- export jobs, selection boundaries and selection progress;
- complete export artifacts or shared artifact chunks;
- every other multi-subject container.

Those records remain owner-controlled shared evidence. Later destructive-action packets must define container-level minimization, rewriting or retention separately and must never delete another subject's data as a side effect.

## 3. Accepted subject relevance semantics

The request passes shared request, lineage, registry, tenant, time, page-size and canonical-Party validation inside one tenant-bound read transaction.

For each Party identifier carried by Customer Data Operations state, the adapter uses the accepted authoritative Identity Resolution topology proof. It does not reconstruct, reinterpret or mutate a second topology model.

### 3.1 Import-row relevance

An authoritative `customer_data.import_row` is relevant when either fully rehydrated identifier resolves to the accepted canonical Party:

- `prepared_party.party_id` for a validated/prepared row;
- `target_party_id` for a row whose execution targeted an authoritative Party.

Source external-identifier digests, row positions, source names, diagnostics or import-job membership alone are not subject proof. Impossible or malformed lifecycle state fails closed.

### 3.2 Export-selection relevance

An authoritative `customer_data.export_selection_item` is relevant when its fully rehydrated `party_id` resolves to the accepted canonical Party. Historical relevance survives later Party version drift.

### 3.3 Export stage and outcome relevance

An execution stage or outcome is relevant only when:

1. a fully rehydrated relevant selection item exists with the same `(job_id, manifest_position)`;
2. the stage/outcome deterministic identity matches that pair;
3. owner, record type, persistence envelope and lifecycle state are valid.

A shared job ID, artifact, chunk index or manifest digest alone is insufficient. Orphan stage/outcome records are excluded.

### 3.4 Canonical alias safety

Historical Party identifiers may now be aliases. Every examined Party reference is resolved under the accepted topology generation. Failed, stale, cyclic or unavailable resolution fails closed; exact-string comparison is not a successful terminal proof.

The shared topology helper preserves two paths:

- read-write merge/unmerge transactions retain advisory topology and row locks;
- `REPEATABLE READ, READ ONLY` privacy transactions use one immutable snapshot without forbidden advisory or row locks.

Both paths preserve the same topology generation, Party existence, redirect and strict Active merge-lineage validation semantics.

## 4. Frozen scan and rehydration bounds

The accepted implementation publishes:

- `MAX_PRIVACY_IMPORT_ROWS_SCANNED = 16_384`;
- `MAX_PRIVACY_EXPORT_SELECTION_ITEMS_SCANNED = 16_384`;
- `MAX_PRIVACY_ASSOCIATED_EXPORT_RECORDS_REHYDRATED = 32_768`;
- `MAX_PRIVACY_CANONICAL_PARTY_RESOLUTIONS = 32_768`;
- `MAX_PRIVACY_OWNER_RECORDS_SCANNED = 65_536`.

It also freezes maximum cursor bytes and page size in the contract layer.

Counters apply to actual database rows and canonical resolutions examined, including malformed, duplicate, unrelated and filtered candidates. Deduplication never refunds a counter.

Traversal is deterministic same-tenant keyset pagination in `record_id` order. Exceeding a bound before terminal completeness returns a stable fail-closed owner error and no partial successful contribution.

These scans are accepted only for the contract-only owner proof. Runtime promotion requires measured SLO evidence or a separately governed owner-maintained canonical-subject index with migration, backfill, reconciliation and rollback proof.

## 5. Strict persistence proof

Every examined record is validated against its exact authoritative persistence envelope:

- owner module and record type;
- schema identifier and version;
- descriptor hash;
- data class and encoding;
- maximum payload size;
- retention policy;
- deterministic identity and positive aggregate version;
- canonical owner lifecycle invariants.

The implementation uses existing strict owner decoders for import rows, export selection items, execution stages and execution outcomes. Selective privacy-only JSON parsing is forbidden. Malformed metadata or payload state fails closed.

## 6. Reference-only response boundary

Each emitted resource contains only:

- resource type;
- resource identifier;
- positive resource version;
- `Personal` data class;
- `RetainMinimizedEvidence` evidence class;
- exact owner retention-policy identifier.

The common contribution lineage may legally contain the accepted canonical Party ID. The response does not expose alias IDs, unrelated Party IDs or owner payload values.

Encoded response bytes exclude:

- source names, external identifiers and import display values;
- mappings, diagnostics and error details;
- export CSV rows, hashes, profile fields and manifest details;
- artifact identifiers, chunks, hashes and sizes;
- execution payload details and reconciliation values;
- persisted JSON or human-readable conclusions.

The contribution performs no deletion, anonymization, export assembly, artifact disclosure, legal-hold or retention decision.

## 7. Deterministic four-family pagination

Frozen global ordering:

1. `customer_data.import_row` by `record_id` ascending;
2. `customer_data.export_selection_item` by `record_id` ascending;
3. `customer_data.export_execution_stage` by `record_id` ascending;
4. `customer_data.export_execution_outcome` by `record_id` ascending.

The cursor binds:

- tenant and privacy case;
- canonical Party and topology generation;
- registry digest;
- purpose and effective request time;
- semantic request identity through validated lineage;
- page size and page number;
- resource-family discriminator and last emitted record ID;
- owner-specific cursor digest domain.

Every request reconstructs bounded relevant evidence and uses `page_size + 1` to prove terminal completeness. Family transitions cannot skip or duplicate records. Cursor tampering, rebinding and stale topology fail closed.

## 8. Transaction and no-write proof

Each request uses exactly one tenant-bound `REPEATABLE READ, READ ONLY` PostgreSQL transaction with FORCE RLS.

The accepted PostgreSQL matrix proves no query-side changes to:

- records;
- relationships;
- business transactions;
- idempotency records;
- outbox events;
- outbox delivery;
- audit heads;
- audit records.

## 9. Permanent acceptance evidence

`Customer Data Operations Privacy Scope CI` passed on exact source `07f34786e82fdfa78d263790e9f50541529006f8` and proved:

1. architecture boundary;
2. formatting;
3. focused Clippy;
4. focused unit tests;
5. clean PostgreSQL migrations and owner fixtures;
6. clean PostgreSQL acceptance;
7. complete rollback;
8. complete absence of schema `crm` after rollback;
9. migration and fixture reapply;
10. repeated PostgreSQL acceptance;
11. workspace dependency graph.

The full exact-head permanent matrix passed 26 of 26 with active 0 and failed 0. `Identity Resolution Privacy Scope CI` also passed after the shared snapshot-proof change.

The PostgreSQL fixture matrix includes canonical, alias, unrelated and same-ID cross-tenant Parties; relevant and unrelated import rows; relevant selection/stage/outcome chains; unrelated and orphan evidence; multi-page four-family traversal; cursor rebinding rejection; stale generation rejection; strict persisted-contract corruption failure; response-byte exclusions; and no-write counts across all listed surfaces.

## 10. Stable error boundary

The adapter owns stable errors for:

- request, contract and semantic-hash mismatch;
- cursor invalid or rebound;
- canonical resolution stale, invalid or unavailable;
- persisted import-row, export-selection, export-stage and export-outcome state invalid;
- selection/stage/outcome identity disagreement;
- scan, rehydration and canonical-resolution bounds;
- database unavailable.

Safe messages do not disclose Party identifiers, source values, exported data or artifact details.

## 11. Explicit exclusions preserved after acceptance

PR #188 added no:

- HTTP or gRPC ingress;
- application registration or worker reachability;
- new production table, relationship or index;
- file-artifact disclosure;
- Customer Privacy discovery/orchestration;
- deletion, anonymization, retention or legal-hold behavior;
- generic privacy runtime;
- runtime promotion of any owner-scope coordinate.

## 12. Historical conclusion

All entry and exit criteria are accepted and merged. This packet is immutable historical evidence for the seventh owner contribution. The active owner sequence has advanced to Data Quality, followed by Customer Enrichment. Production discovery remains prohibited until the complete nine-owner set is accepted.
