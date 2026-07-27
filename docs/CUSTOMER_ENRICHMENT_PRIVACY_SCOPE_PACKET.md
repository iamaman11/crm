# Customer Enrichment Privacy Scope Owner Packet

Status: **Bounded entry contract frozen; implementation not started**  
Parent program: #126  
Coordinate: `customer_enrichment.privacy.scope.contribute@1.0.0`  
Baseline: `a2373255d7625c6807cd0bef89dc0fa22453a29b`  
Runtime state: **Contract-only/non-runtime; no route, worker or application registration is permitted in this packet**

## 1. Objective

Implement `crm.customer-enrichment` as the ninth and final authoritative Customer Privacy owner contribution without treating shared provider/mapping definitions as Party resources, without exposing protected provider or review evidence and without promoting the published privacy coordinate into runtime reachability.

This entry packet freezes authoritative discovery, the seven subject-evidence families, strict parent/provenance associations, alias handling, bounds, pagination, response minimization, PostgreSQL access paths and permanent acceptance before adapter code is written.

## 2. Existing authoritative owner baseline

Phase 8A.10 was accepted through PR #137 from source `f92d101206886e3ceaf94d0e56e52580cec21093`, merge `150e44b95d9dbdc08c1792563de03ec73f34aed1`.

`crm.customer-enrichment` is a provider-neutral owner/coordinator. It owns immutable provider and mapping definitions, enrichment requests, sanitized response/conflict evidence, suggestions, reviews, application attempts and provider usage. It never owns mutable Party values.

The owner publishes exactly nine persisted record types:

1. `customer_enrichment.provider_profile_version`;
2. `customer_enrichment.mapping_version`;
3. `customer_enrichment.request`;
4. `customer_enrichment.provider_response_receipt`;
5. `customer_enrichment.provider_response_conflict`;
6. `customer_enrichment.suggestion`;
7. `customer_enrichment.review_decision`;
8. `customer_enrichment.application_attempt`;
9. `customer_enrichment.provider_usage_entry`.

Request creation atomically creates the authoritative relationship:

- relationship type: `customer_enrichment.request.party`;
- source record type: `parties.party`;
- target record type: `customer_enrichment.request`.

The privacy adapter must reuse this relationship as the subject-discovery authority. It must not create a second reverse index, infer scope from provider provenance alone or treat direct target fields in descendants as independent discovery roots.

## 3. Shared definitions excluded from subject contribution

The following immutable definition families are shared across many requests and are not Party-owned subject resources:

- `customer_enrichment.provider_profile_version`;
- `customer_enrichment.mapping_version`.

They must not be emitted merely because a relevant request references them.

They remain strict validation dependencies. Relevant evidence may be accepted only after the existing owner decoders prove:

- deterministic definition identity matches canonical content;
- mapping references the exact provider-profile version;
- provider profile supports the exact target field and purpose;
- mapping target field and normalization match the relevant request/suggestion target;
- persisted metadata, descriptor hash, data class, encoding, maximum size and retention policy match the owner contract.

Provider keys, adapter coordinates, license/permitted-use policy, residency, retention configuration, credential-handle aliases, provider field paths, normalization details and definition digests must never enter privacy response bytes.

## 4. Frozen subject resource families

The contribution emits only these seven owner-record families, in this exact global order:

1. `customer_enrichment.request`;
2. `customer_enrichment.provider_response_receipt`;
3. `customer_enrichment.provider_response_conflict`;
4. `customer_enrichment.suggestion`;
5. `customer_enrichment.review_decision`;
6. `customer_enrichment.application_attempt`;
7. `customer_enrichment.provider_usage_entry`.

The request is the only subject-discovery root. A request is relevant only when one exact authoritative `customer_enrichment.request.party` relationship links a Party in the accepted canonical/alias set to that request and the strict request target agrees with the relationship.

The remaining six families are relevant only through strict lineage to a relevant request. There is no payload-only or provenance-only fallback discovery family.

Suggestion/application target Party fields are mandatory consistency evidence, not independent scope expansion. Receipt ids, conflict ids, suggestion ids, review ids, attempt ids, usage ids, provider profiles, mappings, provider correlations and protected evidence references are never sufficient subject relevance by themselves.

## 5. Alias-aware relationship discovery

The adapter must reuse the accepted Identity Resolution topology proof under the exact requested generation.

It must build the bounded active canonical/alias Party set using the existing topology semantics and then discover request relationships for every accepted Party node in one immutable tenant snapshot.

Frozen topology limits remain:

- `MAX_PRIVACY_ALIAS_HOPS = 64`;
- `MAX_PRIVACY_ALIAS_NODES = 4_096`;
- `MAX_PRIVACY_ACTIVE_REDIRECT_EDGES = 4_095`.

Every raw relationship row is charged before deduplication or target validation. Historical alias relationships remain relevant when the alias resolves to the requested canonical Party. A stale generation, active cycle, cross-tenant edge or non-canonical terminal state fails closed.

Direct string equality with the canonical Party is not a terminal topology proof.

## 6. Strict family rehydration and association semantics

All examined owner records must pass the exact existing persisted contract and strict canonical decoder. Selective privacy-only JSON parsing is forbidden.

### 6.1 Enrichment request

A request is relevant only when:

1. an authoritative request/Party relationship targets its exact record id;
2. the relationship source Party resolves to the requested canonical Party under the exact generation;
3. the strict request tenant matches the bounded transaction;
4. request record identity equals its deterministic `request_id`;
5. request target resource type is `parties.party` and its target field is the accepted Party display-name field;
6. request target Party id equals the relationship source Party id;
7. request target resource version is positive;
8. the exact provider profile and mapping exist, strictly rehydrate and are mutually bound;
9. mapping/profile target support agrees with the request target and requested fields;
10. lifecycle, retry, response-receipt pointer and timestamps satisfy the owner domain.

Every request must have exactly one authoritative request/Party relationship. Missing, duplicate-source, cross-Party or cross-tenant links fail closed. Relationship payload and attributes are validation evidence and are not emitted.

Request actor, idempotency key, purpose/legal basis/Consent evidence, requested fields, lifecycle diagnostics, retry generation and timestamps remain private.

### 6.2 Provider response receipt

A receipt is relevant only when:

1. its strict `request_id` resolves to a relevant request;
2. receipt and request agree on exact provider-profile and mapping versions;
3. the request's response-receipt pointer, when required by lifecycle state, equals the receipt id;
4. receipt identity, replay key, response class, canonical response digest, timing and metering satisfy the owner domain;
5. no second contradictory receipt is treated as ordinary subject evidence.

The receipt has no independent Party relevance. Provider correlation, replay key, canonical digest, metered units and protected evidence reference remain private.

### 6.3 Provider response conflict

A conflict is relevant only when:

1. its strict tenant matches the bounded transaction;
2. its exact `request_id` resolves to a relevant request;
3. the exact first receipt exists, belongs to that request and matches request/profile/mapping lineage;
4. retry generation and detection time are compatible with the request/receipt history;
5. optional resolution evidence passes the strict owner decoder.

A conflict does not make a provider profile, mapping or conflicting external payload a Party resource. Conflict fingerprint, resolver actor, decision, policy version, reason, approval evidence and causation remain private.

### 6.4 Suggestion

A suggestion is relevant only when:

1. its strict request is relevant;
2. its exact receipt exists and belongs to that request;
3. suggestion/request/receipt agree on profile and mapping versions;
4. suggestion target Party id, resource version and field exactly equal the request target;
5. profile/mapping target support and normalization match;
6. deterministic suggestion identity, proposed-value digest, policy/licensing/Consent/residency/retention evidence and timestamps satisfy the owner domain.

Historical rejected, expired, superseded, accepted and applied suggestions remain relevant provenance. Current lifecycle status does not erase immutable subject evidence.

Proposed values, value digests, confidence, policy/licensing/Consent evidence, provider evidence references and timestamps must never enter response bytes.

### 6.5 Review decision

A review decision is relevant only when:

1. its exact suggestion exists and is relevant;
2. decision suggestion id, target resource version and proposed-value digest agree with the suggestion;
3. decision identity, kind, policy, approval and expiry semantics pass the strict owner decoder;
4. the decision is not used to expand scope to an unrelated request or Party.

Historical review evidence remains relevant even after a later lifecycle transition. Reviewer actor, decision kind, reason, policy version, approval evidence and timestamps remain private.

### 6.6 Application attempt

An application attempt is relevant only when:

1. its exact suggestion exists and is relevant;
2. its exact review decision exists, belongs to that suggestion and represents the required accepted/approved path;
3. attempt target Party id, resource version and field agree with the suggestion and request;
4. proposed-value digest agrees across suggestion, review and attempt;
5. owner capability is the exact governed Party update coordinate allowed by the owner domain;
6. application generation, target idempotency identity and optional recorded outcome pass strict validation.

An attempt does not prove the current Party value. Authoritative Party state remains owned by `crm.parties`.

Target idempotency, owner capability details, planned timestamps, recorded outcome, resulting versions, safe failure details and proposed-value digest remain private.

### 6.7 Provider usage entry

A usage entry is relevant only when:

1. its exact request exists and is relevant;
2. provider-profile version equals the request profile;
3. optional response receipt, when present or required by usage kind, exists and belongs to the same request;
4. request-dispatch, response-received, billable-unit and quota-snapshot semantics satisfy the strict owner domain;
5. provider observation/recording times are internally valid.

Usage has no independent Party relevance. Metered units, quota bucket/remaining values, provider codes, receipt linkage and timestamps remain private.

## 7. Frozen bounds

The contract layer must publish and tests must enforce:

- `MAX_PRIVACY_REQUEST_RELATIONSHIPS_SCANNED = 16_384`;
- `MAX_PRIVACY_REQUEST_RECORDS_REHYDRATED = 16_384`;
- `MAX_PRIVACY_RESPONSE_RECEIPTS_SCANNED = 32_768`;
- `MAX_PRIVACY_RESPONSE_CONFLICTS_SCANNED = 16_384`;
- `MAX_PRIVACY_SUGGESTIONS_SCANNED = 65_536`;
- `MAX_PRIVACY_REVIEW_DECISIONS_SCANNED = 65_536`;
- `MAX_PRIVACY_APPLICATION_ATTEMPTS_SCANNED = 65_536`;
- `MAX_PRIVACY_PROVIDER_USAGE_ENTRIES_SCANNED = 65_536`;
- `MAX_PRIVACY_DEFINITION_RECORDS_REHYDRATED = 8_192`;
- `MAX_PRIVACY_ASSOCIATION_RECORDS_REHYDRATED = 131_072`;
- `MAX_PRIVACY_CANONICAL_PARTY_RESOLUTIONS = 16_384`;
- `MAX_PRIVACY_OWNER_RECORDS_SCANNED = 131_072`;
- `PRIVACY_RELATIONSHIP_SCAN_BATCH_SIZE = 512`;
- `PRIVACY_OWNER_SCAN_BATCH_SIZE = 512`.

The shared owner contract continues to freeze maximum page size `128` and maximum cursor bytes `2_048`.

Relationship, per-family and owner-wide counters count raw PostgreSQL rows before relevance filtering, strict rehydration, deduplication or association validation. Malformed, unrelated, duplicate and cross-tenant candidates do not refund counters.

Definition, association and canonical-resolution counters count actual attempts. Cache hits may avoid another database read but may not erase already charged work.

Exceeding any bound before terminal completeness returns one stable fail-closed owner error and no successful partial contribution.

These limits are for contract-only proof. Runtime promotion requires measured SLO evidence and, if needed, a separately governed owner-maintained subject/parent index with migration, backfill, reconciliation, rollback and alias-convergence proof.

## 8. PostgreSQL access paths and index decision

Authoritative request discovery uses the existing relationship primary-key order:

`crm.relationships (tenant_id, relationship_type, source_record_type, source_record_id, target_record_type, target_record_id)`

For each accepted canonical/alias Party node, the adapter scans only:

- relationship type `customer_enrichment.request.party`;
- source type `parties.party`;
- exact source Party id;
- target type `customer_enrichment.request`;
- target record id keyset continuation.

Owner records use the existing primary key:

`crm.records (tenant_id, record_type, record_id)`

Requests are rehydrated by exact id. The six associated families use bounded same-tenant record-type scans ordered by `record_id ASC`, followed by strict request/parent association.

No new PostgreSQL index is required for the contract-only packet. Implementation must not add:

- JSON or byte-payload expression indexes;
- a privacy-only projection table;
- duplicate request/Party relationships;
- synthetic descendant-to-Party relationships;
- an ungoverned Party-to-enrichment reverse index.

Permanent CI must capture `EXPLAIN (COSTS OFF)` or equivalent structural proof for both the relationship discovery path and owner record path. A full-tenant sequential scan where an existing primary-key path should apply is a gate failure.

If clean PostgreSQL cannot prove those paths, implementation must stop for a separately governed owner index migration rather than weaken bounds.

## 9. Deterministic seven-family pagination

Global family order is exactly the order in section 4. Within each family, order is `record_id ASC`.

The cursor must bind:

- tenant id;
- privacy case id;
- canonical Party id;
- exact Identity Resolution topology generation;
- registry version and digest;
- purpose;
- effective request time;
- semantic request identity;
- page size and page number;
- family discriminator;
- last emitted record id;
- owner-specific cursor digest domain.

Every request reconstructs the complete bounded relevant set inside one immutable snapshot and uses `page_size + 1` to prove whether another item exists. Family transitions must not skip or duplicate records.

Cursor tampering, tenant/Party/case/purpose/time/page-size rebinding, registry drift and stale topology fail closed.

## 10. Transaction and topology boundary

Each contribution request uses exactly one tenant-bound PostgreSQL transaction:

`REPEATABLE READ, READ ONLY`

The transaction must:

- bind tenant context before topology, relationship and owner reads;
- rely on FORCE RLS;
- use the accepted read-only Identity Resolution snapshot proof;
- perform no advisory lock and no row lock;
- perform no writes;
- preserve the read-write merge/unmerge lock path unchanged.

The privacy adapter must not invoke provider transport, resolve secrets, materialize suggestions, review decisions, call Party mutation capabilities or advance workers.

## 11. Reference-only response

Every emitted resource contains only:

- exact owner resource type;
- exact resource id;
- positive resource version;
- `Personal` data class;
- minimized evidence class required by the shared privacy contract;
- exact owner retention-policy id.

The common lineage may contain the canonical Party id. Resource evidence must not expose historical alias ids or relationship payloads.

Encoded response bytes must exclude:

- provider keys, adapter/transport coordinates and credential-handle aliases;
- license, permitted-use, residency and definition policy details;
- request actor, idempotency, purpose/legal basis/Consent evidence, requested fields and lifecycle diagnostics;
- provider correlation, replay key, response class, canonical digest, protected evidence and metering;
- conflict fingerprint, resolution actor/decision/reason/policy/approval/causation;
- proposed values, value digests, confidence and suggestion evidence references;
- review actor, kind, reason, policy and approval evidence;
- application idempotency, owner-call/output/failure details and resulting versions;
- usage units, quota data, provider codes and timestamps;
- raw persisted JSON or unrelated/cross-tenant identifiers.

The contribution performs no provider call, access-export assembly, restriction, legal-hold, retention decision, deletion, anonymization or owner mutation.

## 12. Stable error boundary

The owner adapter must define stable safe errors for:

- request/coordinate/semantic-hash mismatch;
- cursor invalid or rebound;
- topology stale, invalid or unavailable;
- malformed request/Party relationship;
- missing, duplicate or inconsistent request relationship;
- malformed persisted metadata or canonical state for every owner family;
- missing/inconsistent provider-profile or mapping definition;
- missing/inconsistent request/receipt/conflict lineage;
- missing/inconsistent suggestion/review/application lineage;
- missing/inconsistent usage lineage;
- each frozen topology, relationship, family, owner, definition, association and canonical-resolution bound;
- database unavailable.

Safe messages must not disclose Party ids, provider identity, credential aliases, response evidence, suggestion values, review decisions, application outcomes or usage/quota evidence.

## 13. No-write proof

Permanent PostgreSQL acceptance must compare before/after state for at least:

- records;
- relationships;
- business transactions;
- idempotency records;
- outbox events;
- outbox delivery;
- audit heads;
- audit records.

The privacy query must leave every surface unchanged.

## 14. Required PostgreSQL acceptance matrix

The implementation PR must prove on clean PostgreSQL:

- canonical Party, historical alias Party, unrelated Party and same-id cross-tenant Party;
- authoritative request/Party relationships for canonical and alias subjects;
- at least one relevant resource from each of the seven output families;
- provider-profile and mapping definitions excluded from response;
- unrelated and cross-tenant records excluded;
- exact relationship/request target agreement;
- missing, duplicate-source, wrong-target and cross-Party relationships fail closed;
- orphan or mismatched receipt, conflict, suggestion, review, application and usage evidence fail closed;
- malformed metadata and malformed canonical payload fail closed for every family;
- historical suggestion/review/application evidence remains discoverable through request lineage;
- deterministic seven-family multi-page traversal;
- stable cursor, cursor rebinding rejection and stale topology rejection;
- relationship and records primary-key plan proof;
- reference-only response-byte exclusions;
- clean migrations and fixtures;
- complete rollback and absence of schema `crm`;
- reapply and repeated acceptance;
- no-write proof;
- workspace dependency graph;
- no regression in Identity Resolution and existing Customer Enrichment runtime workflows.

## 15. Permanent workflow requirement

Add a permanent `Customer Enrichment Privacy Scope CI` only with implementation code. It must prove:

1. architecture boundary;
2. formatting;
3. focused Clippy;
4. focused unit tests;
5. clean PostgreSQL migrations and owner fixtures;
6. relationship and records access-path proof;
7. clean PostgreSQL acceptance;
8. complete rollback;
9. complete absence of schema `crm`;
10. migration and fixture reapply;
11. repeated PostgreSQL acceptance;
12. workspace dependency graph.

The workflow must remain permanent after merge.

## 16. Explicit implementation exclusions

The implementation packet must add no:

- public HTTP or gRPC route;
- application runtime registration;
- Customer Privacy worker;
- generic privacy runtime;
- production discovery or planning;
- provider transport or secret resolution;
- direct Party mutation;
- new mutable definition or queue model;
- full unbounded tenant scan;
- selective JSON parsing;
- shared-definition inclusion as Party evidence;
- payload-only or provenance-only fallback discovery;
- synthetic descendant relationships;
- runtime promotion of `customer_enrichment.privacy.scope.contribute@1.0.0`.

## 17. Entry conclusion

The existing owner model provides an authoritative request/Party relationship, strict persisted codecs and deterministic parent lineage sufficient for a bounded contract-only implementation without a new index or second provenance model.

Implementation remains not started in this commit. The next source change may add the dedicated Customer Enrichment privacy adapter and permanent workflow only while preserving every frozen boundary above.

Customer Enrichment is the ninth and final owner. Production discovery remains forbidden until this owner contribution is accepted and synchronized.
