# Identity Resolution Privacy Scope Owner Packet

Status: **Accepted through PR #186**  
Parent program: #126  
Coordinate: `identity_resolution.privacy.scope.contribute@1.0.0`  
Accepted source: `24456b86379a1ef23ed5a60804cdcae5075d407c`  
Merge: `509eb304a76055c9f49b0beed3b007963a91cb22`  
Permanent workflows: **25/25 successful on the unchanged accepted source**  
Runtime state: **Contract-only/non-runtime; no ingress, application registration or worker promotion.**

## 1. Accepted objective

`crm.identity-resolution` is the sixth authoritative privacy-scope owner implementation. It contributes duplicate-candidate cases and reversible merge operations while retaining owner-specific graph construction, persistence validation, pagination, evidence classification and errors.

## 2. Accepted authoritative boundary

The adapter strictly rehydrates:

1. `identity_resolution.candidate_case`;
2. `identity_resolution.merge_operation`.

Candidate cases retain Party pairs, exact Party versions, bounded evidence history, matcher profile, score, signals, evidence references and terminal decision state.

Merge operations retain source/survivor Party versions, decision evidence, bounded field survivorship, provenance Party/version, source-value digests, Active/Unmerged state and unmerge evidence.

The adapter does not own or reinterpret Party values, Account membership, Contact Points, Party Relationships, Consents or Customer Privacy state.

## 3. Accepted canonical lineage semantics

The request passes the shared request, lineage, registry, tenant, effective-time, page-size and canonical-Party checks inside one caller-opened read transaction.

Identity Resolution then:

- fully rehydrates every merge operation used for matching;
- accepts only authoritative `Active` operations as current canonical edges;
- enforces one outgoing edge per source, unique operation identifiers and no cycles;
- derives deterministic reverse breadth-first alias closure;
- validates every redirect relationship against the exact matching Active operation;
- preserves retained `Unmerged` history without treating it as current aliasing;
- fails closed on missing, duplicate, disagreeing or malformed topology evidence.

## 4. Accepted bounds

The implementation publishes and enforces:

- `MAX_PRIVACY_ALIAS_HOPS = 64`;
- `MAX_PRIVACY_ALIAS_NODES = 4_096`;
- `MAX_PRIVACY_ACTIVE_REDIRECT_EDGES = 4_095`;
- `MAX_PRIVACY_RELATIONSHIP_CANDIDATES = 16_384`;
- `MAX_PRIVACY_CANDIDATE_RECORDS_REHYDRATED = 8_192`;
- `MAX_PRIVACY_MERGE_RECORDS_REHYDRATED = 8_192`;
- `MAX_PRIVACY_OWNER_RECORDS_SCANNED = 16_384`.

Counters apply to actual rows examined before deduplication. Exceeding a bound fails closed and never produces a terminal incomplete contribution.

## 5. Accepted discovery rules

### Candidate cases

Candidate discovery uses owner-maintained Party relationships for every validated alias, deduplicates record identifiers and strictly rehydrates each case. A case is relevant only when an authoritative endpoint belongs to the current alias set and resolves to the accepted canonical Party.

### Merge operations

Direct merge discovery strictly rehydrates operations referenced through source/survivor relationships. Relevance may arise through source, survivor or survivorship provenance Party.

For Active operations, references are resolved through the validated graph. For Unmerged operations, exact current alias membership is sufficient historical evidence without expanding the alias set.

### Provenance-only fallback discovery

Because existing relationships do not index survivorship provenance Parties, the accepted adapter uses bounded same-tenant keyset traversal of authoritative merge operations.

The fallback:

- validates exact owner and persistence envelope;
- fully rehydrates every scanned operation;
- counts raw examined rows;
- deduplicates direct-discovery results;
- emits only authoritative relevant operations;
- proves terminal completeness or fails closed.

Production promotion remains blocked until this fallback has measured runtime/SLO evidence or is replaced by a separately governed owner-maintained index and backfill.

## 6. Accepted response boundary

The response emits only references for:

- `identity_resolution.candidate_case`;
- `identity_resolution.merge_operation`.

Each reference carries owner, type, identifier, positive version, `Personal` data class, `RetainMinimizedEvidence` evidence class and exact owner retention policy.

Encoded response bytes exclude:

- Party identifiers and versions;
- matcher profiles, scores and signals;
- evidence references and decision reasons;
- merge source/survivor/provenance details;
- survivorship fields and source-value digests;
- actors, Active/Unmerged state and unmerge details;
- persisted JSON and human-readable identity conclusions.

## 7. Accepted pagination

The global ordering is:

1. candidate cases by `record_id`;
2. merge operations by `record_id`.

The cursor binds coordinate/version, privacy lineage, tenant, canonical Party, topology generation, registry evidence, purpose, effective time, page size, resource family, last emitted identifier and page number.

Every page independently validates the alias graph and discovers `page_size + 1` matches or proves both families exhausted. This establishes terminal completeness without skips or duplicates.

Cursor state contains no aliases, graph edges, private owner values or unverified partial result.

## 8. Accepted transaction and PostgreSQL proof

One request uses one tenant-bound `REPEATABLE READ, READ ONLY` PostgreSQL transaction and produces zero writes to records, relationships, business transactions, idempotency, outbox or audit surfaces.

The permanent gate proves:

- direct, chained and converging aliases;
- deterministic traversal;
- candidate cases across direct and aliased endpoints;
- Active and Unmerged merge operations;
- source, survivor and provenance relevance;
- provenance-only fallback;
- unrelated and cross-tenant exclusion;
- sparse and cross-family pagination;
- stale generation, cursor tamper and rebinding rejection;
- malformed persistence and topology fail-closed behavior;
- response-byte exclusions;
- clean database acceptance;
- complete rollback and `crm` schema removal;
- migration reapply and repeated acceptance;
- workspace dependency and architecture integrity.

The test-only corruption fixture uses an isolated privileged transaction solely to emulate already-corrupted persisted metadata. Production write guards remain unchanged.

## 9. Accepted exclusions

PR #186 introduced no:

- HTTP/gRPC ingress;
- application composition or registration;
- Customer Privacy discovery/orchestration;
- worker reachability;
- new production migration;
- deletion, anonymization, retention or legal-hold behavior;
- speculative shared behavior.

## 10. Follow-on

The next owner packet is `CUSTOMER_DATA_OPERATIONS_PRIVACY_SCOPE_PACKET.md`. Identity Resolution remains an accepted, immutable contract-only owner until a separate production-promotion packet satisfies runtime scale, indexing and orchestration requirements.
