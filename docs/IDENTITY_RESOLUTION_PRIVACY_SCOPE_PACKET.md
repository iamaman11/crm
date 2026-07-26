# Identity Resolution Privacy Scope Owner Packet

Status: **Ready after Party Relationships PR #183 and post-merge synchronization**  
Parent program: #126  
Prerequisites: Parties PR #156, Consents PR #175, shared support PR #176, Customer Accounts PR #179, Contact Points PR #181, Party Relationships PR #183  
Coordinate: `identity_resolution.privacy.scope.contribute@1.0.0`

## 1. Objective

Implement `crm.identity-resolution` as the sixth authoritative privacy-scope owner contribution while preserving strict module ownership and the accepted contract-only owner protocol.

The packet is contract-only and non-runtime. It must prove that one owner can contribute two different authoritative resource families — duplicate-candidate cases and reversible merge operations — while keeping canonical graph construction, alias-aware matching, evidence/lineage semantics, heterogeneous pagination, retention and errors entirely owner-specific.

## 2. Why Identity Resolution is next

Identity Resolution is selected because:

- the exact owner coordinate is already published and remains non-runtime;
- the owner has two separate strict persisted-state contracts: `identity_resolution.candidate_case` and `identity_resolution.merge_operation`;
- candidate cases retain canonical Party pairs, versioned match-evidence history, matcher profiles, scores, signals, evidence references and terminal decisions;
- merge operations retain source/survivor Party versions, decision provenance, field-level survivorship evidence, active or unmerged state and reversible canonical lineage;
- an accepted canonical Party may own relevant evidence through a current alias chain even when its identifier is absent from an older candidate case or earlier merge operation;
- this requires bounded owner-defined lineage closure rather than another direct-reference scan;
- the response must remain reference-only because candidate and merge state can reveal another person or organization, inferred identity similarity, evidence sources, decision actors and field-level provenance.

## 3. Frozen authoritative owner boundary

`crm.identity-resolution` owns:

1. `identity_resolution.candidate_case`
   - deterministic case identifier derived from a canonical unordered Party pair;
   - canonical left/right Party references and exact Party versions;
   - bounded evidence history;
   - matcher profile, score and sorted unique match signals;
   - evidence references and per-signal contribution basis points;
   - `Open`, `Dismissed` or `ConfirmedDuplicate` status;
   - optional terminal decision reason;
   - created/updated timestamps and aggregate version.

2. `identity_resolution.merge_operation`
   - operation identifier;
   - source and survivor Party references with exact versions;
   - merge decision reference, actor and reason;
   - bounded deterministic field survivorship selections;
   - provenance Party/version, source-value SHA-256 and evidence reference per selected field;
   - `Active` or `Unmerged` status;
   - optional unmerge decision evidence;
   - created/updated timestamps and aggregate version.

The owner does not own Party values, Account membership, Contact Points, Party Relationships, Consents, Customer Privacy cases or downstream projections. The privacy adapter must not read those owners' values or reinterpret their business state.

## 4. Canonical subject and lineage semantics

The request must first pass the accepted shared lineage, registry, tenant, time, page-size and canonical-Party proof inside the caller-opened read transaction.

Identity Resolution then applies its own bounded lineage rules:

- every merge operation used for matching is selected from authoritative owner records and fully rehydrated under its exact persistence envelope;
- only `Active` merge operations create current canonical edges;
- the active graph must preserve one outgoing edge per source, unique operation identifiers, no cycles and the owner's existing maximum-hop bound;
- owner matching resolves referenced Parties through the active graph to the accepted canonical Party;
- an `Unmerged` operation contributes no active edge, but its retained historical record remains relevant to either exact current source or survivor Party;
- relationship/index rows may narrow reads, but they cannot replace strict authoritative record rehydration or become a competing source of truth;
- graph, scan or lineage bounds are fail-closed and use stable Identity Resolution privacy error codes.

### 4.1 Candidate-case inclusion

A candidate case is included when either fully rehydrated pair endpoint:

- exactly equals the accepted canonical Party; or
- resolves through the valid active merge graph to the accepted canonical Party.

This preserves pre-merge candidate evidence for the surviving canonical subject across chained merges without treating an inactive/unmerged edge as current aliasing.

### 4.2 Merge-operation inclusion

A merge operation is included when any fully rehydrated Party reference carried by the operation is relevant to the accepted subject:

- source Party;
- survivor Party;
- survivorship provenance Party.

For active lineage, each reference is resolved through the active graph before comparison so a final survivor receives the complete retained chain. For an unmerged operation, exact source/survivor/provenance matching remains sufficient evidence for each now-independent Party; the inactive edge must not merge their current scopes.

## 5. Response boundary

The owner emits deterministic reference-only resources with separate stable resource types:

- `identity_resolution.candidate_case`;
- `identity_resolution.merge_operation`.

Each resource contains only the owner module, resource type, resource identifier, positive resource version, customer data class and owner evidence classification required by the shared contract.

Both resource families are classified as `Personal` and `RetainMinimizedEvidence` for scope planning. The contribution does not execute retention, anonymization, deletion, export or legal-hold decisions; those remain later Customer Privacy planning/action responsibilities.

Encoded response bytes must not contain:

- candidate pair Party identifiers or Party versions;
- matcher profile, score, signal kind/source, contribution or evidence reference;
- candidate status or terminal decision reason;
- merge source, survivor or provenance Party identifiers or versions;
- decision references, actor identifiers or reason codes;
- survivorship field paths, source-value digests or evidence references;
- active/unmerged state, unmerge decision details or owner timestamps;
- persisted JSON or any derived human-readable identity conclusion.

## 6. Heterogeneous bounded pagination

Identity Resolution owns a deterministic compound keyset across both resource families.

The frozen ordering is:

1. `identity_resolution.candidate_case` by `record_id` ascending;
2. `identity_resolution.merge_operation` by `record_id` ascending.

The owner cursor must bind at least:

- coordinate and contract version;
- privacy-case and tenant lineage;
- canonical Party and Identity Resolution generation;
- registry version/digest and purpose;
- effective request time;
- page size;
- resource-family discriminator;
- last scanned record identifier;
- page number and owner cursor digest domain.

The adapter must support sparse progress and a page boundary crossing from candidate cases to merge operations without rescanning or skipping records. Scan limits, cursor format/digest, retention and errors remain Identity Resolution-specific.

## 7. Transaction and persistence proof

One request uses exactly one tenant-bound `REPEATABLE READ, READ ONLY` PostgreSQL transaction.

Inside that transaction the adapter must:

1. validate the exact `QueryRequest`, Protobuf wrapper and semantic input hash;
2. invoke accepted shared validation and canonical-Party proof using the already-open transaction;
3. read only same-tenant Identity Resolution owner records;
4. validate record type, owner module, schema identifier/version, descriptor hash, data class, JSON encoding, maximum size and retention policy for each family;
5. decode and fully rehydrate every selected candidate case and merge operation;
6. build and validate the bounded active merge graph;
7. apply owner-specific alias-aware matching and compound pagination;
8. encode deterministic reference-only output;
9. commit the read-only transaction;
10. produce zero writes to records, relationships, business transactions, idempotency, outbox or audit surfaces.

Malformed metadata, noncanonical persisted values, impossible status/version/time combinations, invalid candidate evidence progression, noncanonical survivorship ordering, graph cycles, duplicate active sources or depth exhaustion must fail closed.

## 8. Required PostgreSQL acceptance matrix

The permanent owner gate must prove on a clean database and again after complete rollback/schema removal/reapply:

### Candidate cases

- direct canonical endpoint match;
- match through one active alias edge;
- match through a chained merge path;
- unrelated same-tenant case exclusion;
- `Open`, `Dismissed` and `ConfirmedDuplicate` rehydration;
- multi-snapshot evidence history and positive version preservation;
- malformed envelope and malformed evidence progression fail closed;
- pair IDs, profiles, scores, signals, evidence references and decision state absent from response bytes.

### Merge operations

- active version-1 source-to-survivor record;
- unmerged version-2 record with exact unmerge evidence;
- final-survivor inclusion of every operation in a chained active lineage;
- exact relevance through source, survivor and survivorship provenance references;
- inactive edge does not create current cross-subject aliasing;
- malformed status/version/time, decision evidence or survivorship state fails closed;
- source/survivor/provenance IDs, field paths, digests, actors and reasons absent from response bytes.

### Shared protocol and isolation

- bounded first, sparse intermediate, cross-family and terminal pages;
- empty owner scope;
- page-size rebinding and cursor corruption rejection;
- stale Identity Resolution generation and noncanonical input rejection;
- cross-tenant concealment under FORCE RLS;
- graph cycle, duplicate-source and maximum-hop failures are stable and fail closed;
- deterministic repeat output;
- zero query-side writes before and after rejected requests;
- full workspace dependency integrity.

## 9. Shared-support boundary

The adapter may consume only the behavior-neutral support already accepted through PR #176:

- strict request integrity;
- lineage/registry/time/page-size validation;
- canonical Party claim proof using an already-open transaction;
- common safe error mapping where behavior is genuinely identical.

It must not move into shared support:

- candidate or merge schema metadata;
- decoding/rehydration;
- active graph construction or canonical resolution;
- exact/unmerged relevance rules;
- two-family ordering and cursors;
- evidence classification or retention;
- response resource types;
- Identity Resolution error codes.

Any mechanical consumer or bound-read allowlist extension occurs only in the implementation PR after independent acceptance.

## 10. Explicit non-goals

This packet does not add:

- public HTTP/gRPC ingress;
- application-composition registration or production route promotion;
- a Customer Privacy discovery/planning worker;
- `customer_privacy.action.apply` behavior;
- merge, unmerge, candidate decision or Party mutation behavior;
- public proto changes;
- production database migrations;
- cross-owner value reads;
- shared-support behavior expansion;
- privacy export payloads;
- deletion, anonymization, restriction or legal-hold execution.

## 11. Deferred runtime blockers

The same known contract-only blockers remain explicit:

- cursor digests are not HMAC-authenticated;
- separate page requests do not retain one database snapshot or dataset generation;
- `effective_request_at_unix_ms` is lineage-bound but not an SQL as-of predicate;
- exact Rust toolchain and PostgreSQL image pinning remain a separate maintenance packet;
- Customer Privacy discovery/orchestration, approval, owner actions, recovery and convergence are not yet production-proven.

## 12. Entry checklist

Implementation may start only when:

- `main` contains Party Relationships merge `9ad2aa91321e9edb54cab98218f93143923ef33f`;
- the post-merge documentation PR is accepted;
- the two authoritative record families and alias-aware matching rules above remain frozen;
- no runtime promotion is bundled with the owner adapter;
- clean/rollback/reapply PostgreSQL acceptance is mandatory;
- the final candidate contains no temporary workflow or diagnostics file.
