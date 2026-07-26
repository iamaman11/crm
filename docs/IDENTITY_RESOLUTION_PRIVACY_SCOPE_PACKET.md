# Identity Resolution Privacy Scope Owner Packet

Status: **Ready after Party Relationships PR #183 and post-merge synchronization**  
Parent program: #126  
Prerequisites: Parties PR #156, Consents PR #175, shared support PR #176, Customer Accounts PR #179, Contact Points PR #181, Party Relationships PR #183  
Coordinate: `identity_resolution.privacy.scope.contribute@1.0.0`  
Implementation state: **Not started; this packet freezes the entry boundary only.**

## 1. Objective

Implement `crm.identity-resolution` as the sixth authoritative privacy-scope owner contribution while preserving strict module ownership and the accepted contract-only owner protocol.

The packet remains contract-only and non-runtime. It proves that one owner can contribute two authoritative resource families — duplicate-candidate cases and reversible merge operations — while keeping graph construction, alias-aware matching, persistence semantics, heterogeneous pagination, retention and errors owner-specific.

## 2. Why Identity Resolution is next

Identity Resolution is selected because:

- the exact owner coordinate is published and remains non-runtime;
- the owner has two strict persisted-state contracts: `identity_resolution.candidate_case` and `identity_resolution.merge_operation`;
- candidate cases retain Party pairs, evidence history, matcher profiles, scores, signals, evidence references and terminal decisions;
- merge operations retain source/survivor Party versions, decision provenance, survivorship evidence and reversible lineage;
- an accepted canonical Party may own relevant evidence through a current alias chain even when its identifier is absent from an older record;
- a merge operation may be relevant only through a survivorship provenance Party that has no relationship index row;
- the response must remain reference-only because the underlying values reveal identity similarity, counterpart Parties, evidence sources, actors and field-level provenance.

## 3. Frozen authoritative owner boundary

`crm.identity-resolution` owns:

1. `identity_resolution.candidate_case`
   - deterministic identifier from a canonical unordered Party pair;
   - canonical left/right Party references and exact Party versions;
   - bounded evidence history;
   - matcher profile, score, sorted unique signals and evidence references;
   - `Open`, `Dismissed` or `ConfirmedDuplicate` state;
   - optional terminal decision reason;
   - created/updated timestamps and aggregate version.

2. `identity_resolution.merge_operation`
   - operation identifier;
   - source and survivor Party references with exact versions;
   - merge decision reference, actor and reason;
   - bounded deterministic field survivorship selections;
   - provenance Party/version, source-value SHA-256 and evidence reference per field;
   - `Active` or `Unmerged` state;
   - optional unmerge decision evidence;
   - created/updated timestamps and aggregate version.

The owner does not own Party values, Account membership, Contact Points, Party Relationships, Consents, Customer Privacy cases or downstream projections. The privacy adapter must not read or reinterpret those owners' business values.

## 4. Canonical subject and lineage semantics

The request first passes accepted shared request, lineage, registry, tenant, time, page-size and canonical-Party validation inside the caller-opened read transaction.

Identity Resolution then applies owner-specific lineage rules:

- every merge operation used for matching is fully rehydrated under its exact persistence envelope;
- only `Active` operations create current canonical edges;
- the active graph permits one outgoing edge per source, unique operation identifiers, no cycles and at most 64 hops per path;
- an `Unmerged` operation contributes no active edge, but its retained historical record can remain relevant through exact source, survivor or provenance references;
- relationship rows may narrow reads but never replace authoritative record rehydration;
- every depth, breadth, candidate-scan and provenance-scan limit fails closed with a stable owner error.

### 4.1 Reverse alias closure

The privacy subject is the accepted canonical Party. The adapter must derive the complete bounded set of current aliases that resolve to that Party.

Discovery is deterministic reverse breadth-first traversal:

1. seed depth 0 with the accepted canonical Party;
2. read same-tenant active canonical-redirect relationship candidates whose target is in the current frontier;
3. sort candidates by `(depth, source_party_id, target_party_id)`;
4. for each candidate, find and fully rehydrate the exact matching Active merge operation;
5. accept the edge only when the authoritative operation has the same source and survivor and satisfies all persisted-state invariants;
6. add a previously unseen source Party to the next frontier;
7. continue until the frontier is empty or a bound is exceeded;
8. build the owner `CanonicalPartyGraph` from the accepted Active operations and resolve every discovered alias back to the requested canonical Party.

Relationship corruption, a missing or duplicate matching Active operation, an edge whose authoritative operation disagrees, a cycle, duplicate outgoing source or excessive depth fails closed. The adapter must not silently omit an alias.

### 4.2 Frozen graph and scan bounds

The implementation must publish named constants with these exact initial limits:

- `MAX_PRIVACY_ALIAS_HOPS = 64`;
- `MAX_PRIVACY_ALIAS_NODES = 4_096`, including the accepted canonical Party;
- `MAX_PRIVACY_ACTIVE_REDIRECT_EDGES = 4_095`;
- `MAX_PRIVACY_RELATIONSHIP_CANDIDATES = 16_384` across alias, candidate and direct merge relationship reads;
- `MAX_PRIVACY_CANDIDATE_RECORDS_REHYDRATED = 8_192` per request;
- `MAX_PRIVACY_MERGE_RECORDS_REHYDRATED = 8_192` per request;
- `MAX_PRIVACY_OWNER_RECORDS_SCANNED = 16_384` per resource family per request.

Counters apply to actual database rows examined, including malformed, duplicate, unrelated and filtered candidates. Limits are not pagination shortcuts: exceeding one returns a retryable unavailable/fail-closed owner error and never a terminal incomplete contribution.

A later change to a bound requires a separately reviewed packet with complexity, database and acceptance evidence.

### 4.3 Candidate-case inclusion

Candidate discovery uses owner-maintained `identity_resolution.candidate.party` relationships for every Party in the bounded alias set, deduplicates candidate record IDs and rehydrates each authoritative candidate case.

A case is emitted only when either fully rehydrated endpoint:

- exactly belongs to the alias set; and
- resolves through the validated Active graph to the accepted canonical Party.

This preserves pre-merge evidence across chained merges without treating inactive or unmerged edges as current aliasing. A relationship row pointing to a case whose rehydrated endpoints do not satisfy the rule is corruption and fails closed rather than being silently trusted.

### 4.4 Merge-operation inclusion

Direct merge discovery uses `identity_resolution.merge.party` relationships for every Party in the alias set and strictly rehydrates the referenced operations.

An operation is relevant when any fully rehydrated Party reference is relevant to the accepted subject:

- source Party;
- survivor Party;
- survivorship provenance Party.

For an Active operation, a reference is relevant when it belongs to the alias set or resolves through the validated Active graph to the accepted canonical Party. For an Unmerged operation, exact source, survivor or provenance membership in the current alias set is sufficient historical evidence; the inactive edge must not expand the current alias set.

### 4.5 Provenance-only fallback discovery

Existing merge relationship rows index source and survivor Parties but not survivorship provenance Parties. Until a separately governed owner index/backfill packet exists, the contract-only adapter must perform a bounded same-tenant keyset scan of authoritative `identity_resolution.merge_operation` records in `record_id` order to discover provenance-only relevance.

The fallback scan must:

- validate exact owner, record type and persistence envelope before use;
- fully rehydrate every scanned operation;
- count every examined record against the merge and owner-scan limits;
- deduplicate operations already discovered through direct relationships;
- emit only operations satisfying section 4.4;
- prove terminal completeness or fail closed when a bound is reached.

This fallback is acceptable only for the non-runtime owner proof. Production promotion requires either measured proof that the bounded scan meets the runtime scale/SLO envelope or a separately accepted owner-maintained provenance index with migration, backfill, reconciliation and rollback evidence.

## 5. Response boundary

The owner emits deterministic reference-only resources with two stable resource types:

- `identity_resolution.candidate_case`;
- `identity_resolution.merge_operation`.

Each resource contains only owner module, resource type, resource identifier, positive resource version, customer data class and evidence classification required by the shared contract.

Both resource families are classified as `Personal` and `RetainMinimizedEvidence` for scope planning. The contribution performs no retention, anonymization, deletion, export or legal-hold decision.

Encoded response bytes must not contain:

- candidate pair Party identifiers or Party versions;
- matcher profile, score, signal kind/source, contribution or evidence reference;
- candidate state or decision reason;
- merge source, survivor or provenance Party identifiers or versions;
- decision references, actors or reasons;
- survivorship field paths, source-value digests or evidence references;
- Active/Unmerged state, unmerge evidence or timestamps;
- persisted JSON or derived human-readable identity conclusions.

## 6. Heterogeneous bounded pagination

The frozen global ordering is:

1. `identity_resolution.candidate_case` by `record_id` ascending;
2. `identity_resolution.merge_operation` by `record_id` ascending.

The cursor binds:

- coordinate and contract version;
- privacy case, tenant, canonical Party and Identity Resolution generation;
- registry version/digest, purpose and effective request time;
- page size;
- resource-family discriminator;
- last emitted record identifier;
- page number and owner cursor digest domain.

Every request independently reconstructs and validates the bounded alias set. It then discovers matching resources in global order until it has `page_size + 1` matches or proves both families exhausted. The extra match establishes whether a next cursor is required. Sparse progress and a page crossing from candidate cases to merge operations must not rescan an emitted record, skip a record or produce a terminal page before completeness is proven.

Cursor state may describe only the global ordering position. It must not serialize alias Party IDs, graph edges, private owner values or an unverified partial scan result.

## 7. Transaction and persistence proof

One request uses exactly one tenant-bound `REPEATABLE READ, READ ONLY` PostgreSQL transaction.

Inside it the adapter must:

1. validate the exact `QueryRequest`, Protobuf wrapper and semantic input hash;
2. invoke accepted shared validation and canonical-Party proof using the already-open transaction;
3. derive and validate the bounded reverse alias closure;
4. read only same-tenant Identity Resolution records and owner-maintained relationships;
5. validate record type, owner module, schema identifier/version, descriptor hash, data class, encoding, byte ceiling and retention policy;
6. fully decode and rehydrate every selected candidate case and merge operation;
7. apply exact alias, unmerged and provenance relevance rules;
8. produce deterministic compound pagination and reference-only output;
9. commit the read-only transaction;
10. produce zero writes to records, relationships, business transactions, idempotency, outbox or audit surfaces.

Malformed metadata, noncanonical values, impossible status/version/time combinations, invalid evidence progression, noncanonical survivorship ordering, graph corruption or any exceeded bound fails closed.

## 8. Required PostgreSQL acceptance matrix

The permanent owner gate must pass on a clean database and again after complete rollback, schema removal and reapply.

### 8.1 Reverse alias and graph proof

- direct canonical subject with no aliases;
- one incoming alias;
- multi-hop chain;
- multiple sources converging on one final survivor;
- deterministic traversal independent of row insertion order;
- missing/duplicate Active operation for a redirect fails closed;
- relationship/operation source-survivor disagreement fails closed;
- cycle, duplicate outgoing source, 65-hop path, node breadth and row-scan exhaustion fail closed;
- no alias is silently omitted from a successful terminal result.

### 8.2 Candidate cases

- direct endpoint match;
- match through one alias and through a multi-hop chain;
- unrelated same-tenant exclusion;
- `Open`, `Dismissed` and `ConfirmedDuplicate` rehydration;
- multi-snapshot evidence history and positive version preservation;
- malformed envelope/evidence progression fails closed;
- stale or corrupt candidate relationship fails closed;
- Party IDs, profiles, scores, signals, evidence and decisions absent from response bytes.

### 8.3 Merge operations

- Active version-1 source-to-survivor record;
- Unmerged version-2 record with exact unmerge evidence;
- final-survivor inclusion of every operation in a chained Active lineage;
- direct relevance through source and survivor;
- relevance only through survivorship provenance;
- provenance-only record found by the bounded fallback scan;
- inactive edge does not create current cross-subject aliasing;
- malformed lifecycle, decision or survivorship state fails closed;
- source/survivor/provenance IDs, fields, digests, actors and reasons absent from response bytes.

### 8.4 Pagination, isolation and no-write proof

- bounded first, sparse intermediate, cross-family and terminal pages;
- empty owner scope;
- page-size rebinding and cursor corruption rejection;
- stale Identity Resolution generation and noncanonical input rejection;
- cross-tenant concealment under FORCE RLS;
- deterministic repeated output;
- exact `page_size + 1` terminal-completeness behavior;
- every limit error leaves zero partial response and zero writes;
- zero records, relationships, transactions, idempotency, outbox or audit writes before and after success and rejection;
- full workspace dependency and architecture-policy integrity.

## 9. Error contract

The adapter owns stable errors for at least:

- request/contract/hash mismatch;
- cursor invalid or rebound;
- persisted candidate state invalid;
- persisted merge state invalid;
- canonical redirect missing operation, duplicate operation or disagreement;
- graph cycle, duplicate source or depth exceeded;
- alias node/edge breadth exceeded;
- relationship candidate limit exceeded;
- candidate record scan/rehydration limit exceeded;
- merge record scan/rehydration limit exceeded;
- terminal completeness not provable within bounds.

Bound exhaustion and incomplete discovery are retryable unavailable failures. Corruption and contract mismatch remain non-retryable unless the existing shared contract explicitly defines otherwise.

## 10. Shared-support boundary

The adapter may consume only behavior-neutral support accepted through PR #176:

- request integrity;
- lineage/registry/time/page-size validation;
- canonical Party claim proof using an already-open transaction;
- genuinely identical safe error mapping and digest framing.

It must not move into shared support:

- Identity schema metadata or decoding;
- reverse alias discovery or graph validation;
- direct, unmerged or provenance relevance rules;
- two-family ordering/cursors;
- evidence classification or retention;
- response resource types;
- Identity Resolution errors or numeric bounds.

Mechanical consumer and bound-read allowlists may be extended only in the implementation PR.

## 11. Explicit non-goals

This packet adds no:

- public HTTP/gRPC ingress;
- application registration or production route promotion;
- Customer Privacy discovery/planning worker;
- owner action, merge, unmerge, decision or Party mutation;
- public proto change;
- production database migration;
- cross-owner value read;
- shared-support behavior expansion;
- export payload, deletion, anonymization, restriction or legal-hold execution.

## 12. Deferred runtime blockers

The following remain explicit:

- owner cursor digests are not HMAC-authenticated;
- separate page requests do not retain one database snapshot or owner dataset generation;
- `effective_request_at_unix_ms` is lineage-bound but not an SQL as-of predicate;
- provenance-only discovery currently requires a bounded owner-record fallback scan;
- exact Rust toolchain and PostgreSQL image pinning remain separate maintenance work;
- Customer Privacy discovery/orchestration, approval, actions, recovery and convergence are not production-proven.

## 13. Entry checklist

Implementation may start only when:

- `main` contains Party Relationships merge `9ad2aa91321e9edb54cab98218f93143923ef33f`;
- the post-merge synchronization and owner-status integrity correction are accepted;
- the two resource families, reverse-alias algorithm, provenance fallback and numeric bounds above remain frozen;
- no runtime promotion is bundled with the adapter;
- clean/rollback/reapply PostgreSQL acceptance is mandatory;
- the final candidate contains no temporary workflow, script or diagnostics file.
