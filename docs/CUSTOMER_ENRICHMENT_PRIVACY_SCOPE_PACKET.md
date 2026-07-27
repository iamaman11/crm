# Customer Enrichment Privacy Scope Owner Packet

Status: **Accepted historical contract**  
Parent program: #126  
Coordinate: `customer_enrichment.privacy.scope.contribute@1.0.0`  
Accepted source: `e90e36027de18a07be68e43327ea732810ff332a`  
Squash merge: `e41cbab0cd30819fcbe2e3c5f2c7415fc6de3e8c`  
Exact-head gate: **28 of 28 permanent workflows succeeded**  
Runtime state: **Contract-only/non-runtime; no route, worker, application registration, production discovery, planning or owner action execution**

## 1. Acceptance conclusion

PR #192 accepted `crm.customer-enrichment` as the ninth and final authoritative Customer Privacy owner contribution. The implementation remains contract-only/non-runtime and adds no HTTP/gRPC ingress, application runtime registration, Customer Privacy worker, provider transport, secret resolution, Party mutation, production discovery, planning or owner action execution.

All nine authoritative owner implementations are now accepted. The next bounded Customer Privacy packet is **Scope discovery and immutable snapshot**. Production discovery remains forbidden until that packet is separately inspected, frozen, implemented and accepted. Planning and action execution remain prohibited until their own later acceptance boundaries are complete.

The permanent `.github/workflows/customer-enrichment-privacy-scope.yml` workflow was added with the implementation and remains after merge.

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

The accepted adapter reuses this relationship as the subject-discovery authority. It creates no second reverse index, infers no scope from provider provenance alone and does not treat direct target fields in descendants as independent discovery roots.

## 3. Shared definitions excluded from subject contribution

The following immutable definition families are shared across many requests and must not be emitted as Party evidence:

- `customer_enrichment.provider_profile_version`;
- `customer_enrichment.mapping_version`.

They remain strict validation dependencies. Relevant evidence is accepted only after the existing owner decoders prove:

- deterministic definition identity matches canonical content;
- mapping references the exact provider-profile version;
- provider profile supports the exact target field and purpose;
- mapping target field and normalization match the relevant request/suggestion target;
- persisted metadata, descriptor hash, data class, encoding, maximum size and retention policy match the owner contract.

Provider keys, adapter coordinates, permitted-use policy, residency, retention configuration, credential aliases, provider field paths, normalization details and definition digests never enter privacy response bytes.

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

The remaining six families are relevant only through strict lineage to a relationship-proven request. There is no payload-only or provenance-only fallback discovery family.

Suggestion and application target Party fields are mandatory consistency evidence, not independent scope expansion. Receipt ids, conflict ids, suggestion ids, review ids, attempt ids, usage ids, provider profiles, mappings, provider correlations and protected evidence references never establish subject relevance by themselves.

## 5. Alias-aware relationship discovery

The adapter reuses the accepted Identity Resolution topology proof under the exact requested generation and builds the bounded active canonical/alias Party set in one tenant-bound immutable snapshot.

Frozen topology limits:

- `MAX_PRIVACY_ALIAS_HOPS = 64`;
- `MAX_PRIVACY_ALIAS_NODES = 4_096`;
- `MAX_PRIVACY_ACTIVE_REDIRECT_EDGES = 4_095`.

Every raw relationship row is charged before deduplication or target validation. Historical alias relationships remain relevant when the alias resolves to the requested canonical Party. A stale generation, active cycle, cross-tenant edge or non-canonical terminal state fails closed.

Direct string equality with the canonical Party is not a terminal topology proof.

## 6. Strict relationship contract and family rehydration

All examined owner records pass the exact existing persisted contract and strict canonical decoder. Selective privacy-only JSON parsing is forbidden.

The authoritative request/Party relationship is validated through typed metadata fields:

- `owner_module_id`;
- `schema_id`;
- `schema_version`;
- `descriptor_hash`;
- `data_class`;
- `payload_encoding`;
- `maximum_payload_size`;
- `retention_policy_id`;
- `payload_bytes`.

The accepted adapter does not read deprecated `attributes` or `attributes_json` columns. Relationship version, typed metadata and canonical empty payload must match exactly. Missing, duplicate-source, cross-Party, wrong-target or cross-tenant links fail closed.

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

Request actor, idempotency key, purpose/legal basis/Consent evidence, requested fields, lifecycle diagnostics, retry generation and timestamps remain private.

### 6.2 Provider response receipt

A receipt has no independent Party relevance. It is accepted only when its strict request is relevant, profile/mapping lineage agrees, the request receipt pointer is consistent and receipt identity/replay/response/timing/metering semantics pass the owner decoder.

Provider correlation, replay key, canonical digest, metered units and protected evidence remain private.

### 6.3 Provider response conflict

A conflict is accepted only through its exact relevant request and first receipt, with compatible retry generation, detection time and optional resolution evidence.

Conflict fingerprint, resolver actor, decision, policy version, reason, approval evidence and causation remain private.

### 6.4 Suggestion

A suggestion is accepted only when its request is relevant, its receipt belongs to that request, profile/mapping lineage agrees, target Party/version/field match the request and deterministic identity/policy/licensing/Consent/residency/retention evidence passes strict validation.

Historical rejected, expired, superseded, accepted and applied suggestions remain relevant immutable provenance. Proposed values, digests, confidence and protected evidence remain private.

### 6.5 Review decision

A review decision is accepted only through its exact relevant suggestion and matching target version/value digest, with valid decision, policy, approval and expiry semantics. Reviewer actor, decision kind, reason, policy and approval evidence remain private.

### 6.6 Application attempt

An application attempt is accepted only through its exact relevant suggestion and review path, matching target Party/version/field and value digest, exact governed Party update coordinate and valid generation/idempotency/outcome semantics.

An attempt never proves current Party state. Target idempotency, owner-call details, outcomes, failures and resulting versions remain private.

### 6.7 Provider usage entry

A usage entry is accepted only through its exact relevant request, matching provider profile and valid optional receipt, usage-kind, metering/quota and timing semantics. Usage has no independent Party relevance and its metering/quota/provider details remain private.

## 7. Frozen bounds

The accepted contract and tests enforce:

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

The shared owner contract retains maximum page size `128` and maximum cursor bytes `2_048`.

Relationship, per-family and owner-wide counters count raw PostgreSQL rows before relevance filtering, strict rehydration, deduplication or association validation. Definition, association and canonical-resolution counters count actual attempts. Exceeding any bound before terminal completeness returns one stable fail-closed owner error and no successful partial contribution.

Runtime promotion requires measured SLO evidence and, if necessary, a separately governed owner-maintained index/backfill/reconciliation/rollback packet.

## 8. PostgreSQL access paths and index decision

Authoritative request discovery uses the existing relationship primary key:

`crm.relationships (tenant_id, relationship_type, source_record_type, source_record_id, target_record_type, target_record_id)`

Owner records use the existing primary key:

`crm.records (tenant_id, record_type, record_id)`

Requests are rehydrated by exact id. The six associated families use bounded same-tenant record-type scans ordered by `record_id ASC`, followed by strict request/parent association.

The accepted contract adds no JSON or byte-payload expression indexes, privacy-only projection table, duplicate request/Party relationships, synthetic descendant-to-Party relationships or ungoverned Party-to-enrichment reverse index.

Permanent CI proves both relationship and record primary-key access paths.

## 9. Deterministic seven-family pagination

Global family order is exactly the order in section 4. Within each family, order is `record_id ASC`.

The cursor binds tenant, privacy case, canonical Party, exact topology generation, registry version/digest, purpose, effective request time, semantic request identity, page size, page number, family discriminator, last emitted record id and owner-specific digest domain.

Every request reconstructs the complete bounded relevant set inside one immutable `REPEATABLE READ, READ ONLY` transaction and uses `page_size + 1` to prove terminal completeness. Cursor tampering, request rebinding, registry drift and stale topology fail closed.

## 10. Transaction and no-write boundary

Each contribution uses exactly one tenant-bound PostgreSQL transaction:

`REPEATABLE READ, READ ONLY`

The transaction binds tenant context before topology, relationship and owner reads, relies on FORCE RLS, uses the accepted read-only Identity Resolution snapshot proof, acquires no advisory or row lock and performs no writes.

The adapter invokes no provider transport, secret resolution, suggestion materialization, review action, Party mutation capability or worker advancement.

Permanent PostgreSQL acceptance compares before/after state for records, relationships, business transactions, idempotency records, outbox events, outbox delivery, audit heads and audit records. The privacy query leaves every surface unchanged.

## 11. Reference-only response

Every emitted resource contains only exact owner resource type/id/version, `Personal` data class, minimized evidence class and exact owner retention-policy id.

Encoded response bytes exclude provider/credential/transport identity, policy/residency details, request actor/idempotency/legal basis/Consent evidence, provider response evidence, conflicts, proposed values, review details, application details, usage/quota details, raw persisted JSON and unrelated or cross-tenant identifiers.

The contribution performs no provider call, access-export assembly, restriction, legal-hold, retention decision, deletion, anonymization or owner mutation.

## 12. Stable error boundary

The accepted adapter defines stable safe errors for request/coordinate/hash mismatch, invalid or rebound cursor, stale/invalid topology, malformed typed relationship metadata, missing/duplicate/inconsistent request links, malformed owner records, missing/inconsistent definitions and lineage, every frozen limit and database unavailability.

Safe messages disclose no Party ids, provider identity, credentials, response evidence, suggestion values, review decisions, application outcomes or quota evidence.

## 13. Accepted PostgreSQL matrix

The permanent workflow proved on clean PostgreSQL and again after full rollback/schema removal/reapply:

- canonical, historical alias, unrelated and same-id cross-tenant Parties;
- canonical and alias request/Party relationships;
- all seven output families;
- shared provider-profile/mapping definition exclusion;
- exact relationship/request target agreement;
- malformed, missing, duplicate, wrong-target and cross-Party relationships fail closed;
- orphan or mismatched receipt/conflict/suggestion/review/application/usage evidence fail closed;
- malformed metadata and canonical payload fail closed for every family;
- historical suggestion/review/application evidence remains discoverable through request lineage;
- deterministic seven-family multi-page traversal;
- cursor rebinding and stale topology rejection;
- relationship and record primary-key plan proof;
- reference-only response exclusions;
- zero query-side writes;
- workspace dependency integrity;
- no regression in Identity Resolution and existing Customer Enrichment runtime workflows.

## 14. Permanent workflow acceptance

`Customer Enrichment Privacy Scope CI` remains permanent and proves:

1. exact candidate checkout;
2. architecture boundary;
3. formatting;
4. focused Clippy including integration targets;
5. focused unit tests;
6. clean migrations and owner fixtures;
7. clean PostgreSQL acceptance;
8. complete rollback and absence of schema `crm`;
9. migration and fixture reapply;
10. repeated PostgreSQL acceptance;
11. workspace dependency graph.

The accepted exact head `e90e36027de18a07be68e43327ea732810ff332a` completed **28 of 28 permanent workflows** with active `0`, failed `0`, branch behind base `0` and unresolved review threads `0` before expected-head squash merge.

## 15. Historical exclusions and next boundary

The accepted owner packet contains no public HTTP or gRPC route, application runtime registration, Customer Privacy worker, generic privacy runtime, production discovery, planning, owner execution, provider transport, secret resolution, Party mutation, unbounded tenant scan, selective JSON parsing, shared-definition inclusion, fallback discovery, synthetic descendant relationship or runtime promotion.

The owner implementation lane is complete at nine of nine. The next packet is **Customer Privacy scope discovery and immutable snapshot**. It must be frozen and accepted separately before any runtime discovery begins; deterministic planning and action execution remain later prohibited packets.
