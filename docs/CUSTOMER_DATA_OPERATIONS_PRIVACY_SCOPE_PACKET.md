# Customer Data Operations Privacy Scope Owner Packet

Status: **Ready after Identity Resolution PR #186 and post-merge synchronization**  
Parent program: #126  
Prerequisites: accepted Parties, Consents, Customer Accounts, Contact Points, Party Relationships and Identity Resolution owner contributions  
Coordinate: `customer_data.privacy.scope.contribute@1.0.0`  
Implementation state: **Not started; this packet freezes the entry boundary only.**

## 1. Objective

Implement `crm.customer-data-operations` as the seventh authoritative privacy-scope owner contribution without promoting the coordinate to runtime and without treating multi-subject import/export containers as subject-owned records.

The packet proves subject discovery across import-row evidence and export-selection/execution evidence while preserving strict owner persistence, bounded canonical resolution, deterministic heterogeneous pagination and reference-only output.

## 2. Why Customer Data Operations is next

Customer Data Operations is selected because:

- the exact coordinate is already published as contract-only/non-runtime;
- the module owns eight authoritative record types, but only a subset carries subject-level Party relevance;
- import rows can retain prepared or successfully targeted Party identifiers plus source-derived diagnostics;
- export selection items retain exact Party identifiers and resource versions;
- export execution stages and outcomes retain subject-derived row/chunk evidence by the same job and manifest position;
- import/export jobs, boundaries, progress records and artifacts can be shared by many subjects and must not be reclassified as wholly owned by one subject;
- no current owner-maintained canonical-subject relationship index covers historical Party aliases, so the non-runtime proof requires bounded same-tenant scans and accepted canonical Party resolution;
- response bytes must expose only resource references, never imported values, exported rows, artifact digests, diagnostics or other subjects.

## 3. Frozen authoritative owner boundary

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

The following are deliberately not emitted merely because one child record is relevant:

- import jobs and source artifacts;
- export jobs, selection boundaries and selection progress;
- complete export artifacts or shared artifact chunks;
- any other multi-subject container.

Those records remain owner-controlled shared evidence. A later destructive-action packet must define container-level minimization, rewriting or retention semantics separately and must never delete another subject's data as a side effect.

## 4. Subject relevance semantics

The request first passes the accepted shared request, lineage, registry, tenant, time, page-size and canonical-Party validation inside the caller-opened read transaction.

For each Party identifier carried by Customer Data Operations state, the adapter uses the accepted canonical Party resolution primitive. It must not reconstruct, reinterpret or directly mutate Identity Resolution state.

### 4.1 Import-row relevance

An authoritative `customer_data.import_row` is relevant when either of these fully rehydrated identifiers resolves to the accepted canonical Party:

- `prepared_party.party_id` for a validated/prepared row;
- `target_party_id` for a row whose execution targeted an authoritative Party.

A source external identifier digest, row position, source name, diagnostic or import-job membership alone is not subject proof.

If both Party fields are present they must satisfy the persisted lifecycle invariants. Conflicting impossible state fails closed rather than choosing one field.

### 4.2 Export-selection relevance

An authoritative `customer_data.export_selection_item` is relevant when its fully rehydrated `party_id` resolves to the accepted canonical Party.

The item preserves the exact Party resource version selected for the export manifest. Version drift does not remove historical relevance.

### 4.3 Export stage and outcome relevance

An authoritative execution stage or outcome is relevant only when:

1. a fully rehydrated relevant selection item exists with the same `export_job_id` and `manifest_position`;
2. the stage/outcome deterministic identity matches that pair;
3. its owner, record type, persistence envelope and lifecycle state are valid.

The adapter must not infer relevance from an export job, artifact, chunk index or manifest digest without the matching authoritative selection item.

A stage and outcome may remain relevant when the Party later becomes invisible, changes version or is excluded, because they are historical evidence of the governed export attempt.

### 4.4 Canonical alias safety

Historical import rows or export selections may carry a Party identifier that is now an alias. The adapter therefore resolves every examined Party reference to its current canonical Party under the accepted topology generation bound.

A failed, stale, cyclic or unavailable canonical resolution fails closed. Exact-string matching alone is insufficient for a successful terminal result.

## 5. Frozen scan and rehydration bounds

The implementation must publish named constants with these exact initial limits:

- `MAX_PRIVACY_IMPORT_ROWS_SCANNED = 16_384`;
- `MAX_PRIVACY_EXPORT_SELECTION_ITEMS_SCANNED = 16_384`;
- `MAX_PRIVACY_ASSOCIATED_EXPORT_RECORDS_REHYDRATED = 32_768`;
- `MAX_PRIVACY_CANONICAL_PARTY_RESOLUTIONS = 32_768`;
- `MAX_PRIVACY_OWNER_RECORDS_SCANNED = 65_536`.

Counters apply to actual database rows and canonical resolutions examined, including malformed, duplicate, unrelated and filtered candidates. Deduplication never refunds the counter.

The scan is deterministic same-tenant keyset traversal in `record_id` order. Reaching a bound before terminal completeness returns a stable retryable fail-closed owner error and no partial successful contribution.

These bounded scans are acceptable only for the contract-only owner proof. Runtime promotion requires measured SLO evidence or a separately governed owner-maintained canonical-subject index with migration, backfill, reconciliation and rollback proof.

## 6. Persistence proof

Every selected record is validated against its exact authoritative persistence envelope:

- owner module;
- record type;
- schema identifier and version;
- descriptor hash;
- data class;
- encoding;
- maximum payload size;
- retention policy;
- canonical JSON encoding;
- deterministic identity;
- positive aggregate version;
- lifecycle and timestamp invariants.

The implementation must use the existing strict owner decoders for import rows, export selection items, execution stages and execution outcomes. Re-parsing fields into a weaker privacy-only shape is forbidden.

## 7. Response boundary

The owner emits deterministic reference-only resources with the four stable resource types listed in section 3.

Each reference contains only:

- owner module;
- resource type;
- resource identifier;
- positive resource version;
- `Personal` data class;
- `RetainMinimizedEvidence` evidence class;
- the exact owner retention-policy identifier.

The contribution performs no deletion, anonymization, export assembly, artifact disclosure, legal-hold or retention decision.

Encoded response bytes must not contain:

- prepared or target Party identifiers;
- source names, source-system identifiers or external-identifier digests;
- imported display names, kinds, mappings, diagnostics or error codes;
- export profile fields, specification versions or manifest digests;
- exported CSV rows or row hashes;
- artifact file identifiers, chunk indices, chunk hashes or byte sizes;
- exclusion reasons, redaction counts or reconciliation values;
- job identifiers or manifest positions except where they are already part of the opaque owner resource identifier;
- persisted JSON or human-readable conclusions.

## 8. Heterogeneous bounded pagination

The frozen global ordering is:

1. `customer_data.import_row` by `record_id` ascending;
2. `customer_data.export_selection_item` by `record_id` ascending;
3. `customer_data.export_execution_stage` by `record_id` ascending;
4. `customer_data.export_execution_outcome` by `record_id` ascending.

The owner cursor binds:

- coordinate and contract version;
- privacy case, tenant, canonical Party and topology generation;
- registry version/digest, purpose and effective request time;
- page size;
- resource-family discriminator;
- last emitted record identifier;
- page number and owner cursor digest domain.

Every request independently reconstructs the bounded relevant selection set until it has `page_size + 1` matches or proves all four families exhausted. Sparse pages and family transitions must not skip or duplicate records.

Cursor state must not serialize Party IDs, imported/exported values, manifest positions, artifact evidence or an unverified partial scan result.

## 9. Transaction and no-write proof

One request uses exactly one tenant-bound `REPEATABLE READ, READ ONLY` PostgreSQL transaction.

Inside it the adapter must:

1. validate the exact query contract and semantic input hash;
2. invoke accepted shared lineage and canonical-Party validation;
3. scan only same-tenant Customer Data Operations records;
4. resolve Party references through the accepted canonical-resolution primitive;
5. strictly rehydrate every examined relevant record;
6. join stage/outcome evidence only through authoritative selection identity;
7. produce deterministic pagination and reference-only output;
8. commit the read-only transaction;
9. produce zero writes to records, relationships, transactions, idempotency, outbox, audit or file artifacts.

## 10. Required PostgreSQL acceptance matrix

The permanent owner gate must pass on a clean database and again after complete rollback, schema removal and reapply.

### 10.1 Import evidence

- prepared Party match;
- successful target Party match;
- historical alias resolving to the canonical subject;
- unrelated and cross-tenant exclusion;
- pending/invalid rows without Party proof excluded;
- conflicting or malformed lifecycle state fails closed;
- source values and diagnostics absent from response bytes.

### 10.2 Export evidence

- direct selection-item match;
- historical alias selection match;
- emitted and excluded execution stages;
- emitted and excluded execution outcomes;
- stage/outcome included only through the exact matching selection item;
- unrelated job/position evidence excluded;
- malformed deterministic identity or persistence envelope fails closed;
- row bytes, hashes, artifact and reconciliation evidence absent from response bytes.

### 10.3 Shared-container exclusion

- relevant child records do not emit import/export job references;
- source artifacts and complete export artifacts are not emitted;
- one subject cannot cause another subject's shared container to enter its scope;
- no container-level destructive semantics are introduced.

### 10.4 Pagination, limits and no-write proof

- first, sparse, cross-family and terminal pages;
- exact `page_size + 1` terminal-completeness behavior;
- cursor tamper, page-size rebinding and stale topology rejection;
- raw scanned-row counters before deduplication;
- every configured bound fails closed with zero partial response;
- deterministic repeated output;
- zero records, relationships, transactions, idempotency, outbox, audit or file-artifact writes;
- full workspace and architecture-policy integrity.

## 11. Error contract

The adapter owns stable errors for at least:

- request, contract or semantic-hash mismatch;
- cursor invalid or rebound;
- canonical resolution stale, invalid or unavailable;
- persisted import-row state invalid;
- persisted export-selection state invalid;
- persisted export-stage state invalid;
- persisted export-outcome state invalid;
- selection/stage/outcome identity disagreement;
- scan, rehydration or canonical-resolution bound exceeded;
- database unavailable.

Internal Party identifiers, source values, exported data and artifact details must not appear in safe error messages.

## 12. Explicit exclusions

This packet does not add:

- HTTP or gRPC ingress;
- application registration or worker reachability;
- new production tables, relationships or indexes;
- file-artifact disclosure;
- Customer Privacy discovery/orchestration;
- deletion, anonymization, retention or legal-hold behavior;
- shared-support behavior expansion;
- runtime promotion of any owner-scope coordinate.

## 13. Exit criteria

The packet may enter gate review only when:

1. the exact contract is implemented without speculative shared abstraction;
2. all four subject-level resource families are proven;
3. multi-subject containers are explicitly excluded;
4. bounded alias-safe scans prove terminal completeness or fail closed;
5. response-byte and no-write tests pass;
6. clean, rollback, schema-removal, reapply and repeated PostgreSQL acceptance pass;
7. all applicable permanent workflows pass on one unchanged source SHA;
8. no runtime route, worker or application registration is introduced.
