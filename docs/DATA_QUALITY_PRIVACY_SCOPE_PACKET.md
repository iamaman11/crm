# Data Quality Privacy Scope Owner Packet

Status: **Accepted historical contract**  
Parent program: #126  
Coordinate: `data_quality.privacy.scope.contribute@1.0.0`  
Accepted source: `dcfe8faebc7462b888f8fc1721cb379a40fea88a`  
Merge: `deac197c97cddc15bb9916092ca87f6e767ce1de`  
Exact-head gate: **27 of 27 permanent workflows succeeded**  
Runtime state: **Contract-only/non-runtime; no route, worker or application registration was added**

## 1. Objective

Implement `crm.data-quality` as the eighth authoritative Customer Privacy owner contribution without turning shared quality definitions into Party-owned resources, without constructing a second quality lineage model and without promoting the published owner coordinate into runtime reachability.

The accepted implementation preserves the frozen persisted record families, Party relevance, association validation, strict rehydration, scan and rehydration bounds, pagination order, response minimization, PostgreSQL access path and acceptance boundary.

## 2. Existing authoritative owner baseline

Phase 8A.9 was accepted through PR #132 from source `c066c278edd75b5f78bbfcead792d34164c76ff5`, merge `8a1664309be9dc0c5e3bf9014cf248b1c3680035`.

`crm.data-quality` owns quality-governance definitions, exact-version Party evaluation evidence, findings and observations, completeness evidence, stewardship state and deterministic remediation-attempt evidence. It does not own mutable Party values.

The owner publishes exactly nine persisted record types:

1. `data_quality.party_rule_set_version`;
2. `data_quality.party_completeness_profile_version`;
3. `data_quality.party_evaluation_job`;
4. `data_quality.party_evaluation_input`;
5. `data_quality.rule_outcome`;
6. `data_quality.finding`;
7. `data_quality.finding_observation`;
8. `data_quality.party_completeness_result`;
9. `data_quality.remediation_attempt`.

The production planners create no Data Quality rows in `crm.relationships`. Existing lineage is encoded in strict authoritative record state and deterministic identities. The privacy adapter must not add a parallel relationship model merely to simplify discovery.

## 3. Shared definitions excluded from subject contribution

The following immutable definition families are shared across many Party evaluations and are not Party-owned subject resources:

- `data_quality.party_rule_set_version`;
- `data_quality.party_completeness_profile_version`.

They must not be emitted in a Party contribution merely because one relevant evaluation references them.

They remain strict validation dependencies. Relevant subject evidence may be accepted only after referenced definitions are rehydrated through the existing owner decoders and the following bindings are proven:

- deterministic definition identity matches canonical content;
- a completeness profile references the exact rehydrated rule-set version;
- every referenced rule or component exists in the bound definition;
- persisted metadata, descriptor hash, data class, encoding, size and retention policy match the owner contract.

Definition titles, remediation guidance, evaluator parameters, placeholder tokens and component weights must never enter privacy response bytes.

## 4. Frozen subject resource families

The contribution emits only these seven direct-subject families, in this exact global order:

1. `data_quality.party_evaluation_job`;
2. `data_quality.party_evaluation_input`;
3. `data_quality.rule_outcome`;
4. `data_quality.finding`;
5. `data_quality.finding_observation`;
6. `data_quality.party_completeness_result`;
7. `data_quality.remediation_attempt`.

Every successful family carries an authoritative persisted `party_id`. There is no provenance-only fallback discovery family in the current owner model. A job id, finding id, observation id, outcome id, definition id, actor assignment or remediation id alone is not subject relevance.

A record is relevant only when its strict authoritative `party_id` resolves through the accepted Identity Resolution topology to the request's canonical Party under the exact requested topology generation.

Direct string equality is not a terminal topology proof. Alias-aware canonical resolution is mandatory for historical Party identifiers.

## 5. Strict family rehydration and association semantics

All examined records must pass the exact existing owner persisted contract and strict canonical decoder. Selective privacy-only JSON parsing is forbidden.

### 5.1 Party evaluation job

A job is relevant when its strict `party_id` resolves to the requested canonical Party.

The adapter must validate:

- record identity equals `job_id`;
- positive record version;
- valid `CREATED`, `STAGED` or `COMPLETED` lifecycle state;
- exact rule-set and completeness-profile references;
- profile-to-rule-set binding;
- positive Party resource version whenever the job has crossed staging;
- bounded evaluated/failed counters consistent with the frozen definition limits.

### 5.2 Party evaluation input

An input is relevant only when:

1. its strict `party_id` resolves to the requested canonical Party;
2. an exact rehydrated evaluation job exists with the same `job_id`;
3. the input record id equals that `job_id`;
4. input and job agree on Party identity and staged Party resource version;
5. the staged timestamp and job state are compatible with authoritative staging semantics.

An orphan input or an input attached to a different Party must fail closed and must not be emitted.

The private staged `display_name` and Party kind are evidence values and must never enter response bytes.

### 5.3 Rule outcome

An outcome is relevant only when:

1. its strict `party_id` resolves to the requested canonical Party;
2. an exact rehydrated evaluation job exists with the same `job_id` and Party;
3. the outcome's rule-set version equals the job's rule-set version;
4. the rule key exists in the exact rehydrated rule set;
5. the deterministic outcome identity matches `(job_id, rule_key, rule_set_version_id)`;
6. Party resource version and evaluation timestamp are compatible with the staged job/input lineage.

An outcome must not be included by job id or rule-set id alone.

### 5.4 Finding

A finding is relevant when its strict `party_id` resolves to the requested canonical Party.

The adapter must validate:

- deterministic finding identity from tenant, authoritative target type, Party id, rule-set version and rule key;
- exact existing rule-set and rule key;
- positive evaluated Party resource version;
- valid current lifecycle state;
- current observation id is non-empty and resolves to an exact strict observation for the same finding, Party, rule-set and rule key;
- optional assignment, waiver and remediated-outcome fields are valid for the lifecycle state;
- referenced remediating outcome, when present, is strict and agrees on Party/rule lineage.

Finding assignment, waiver reason and lifecycle detail remain private owner payload and are not response evidence.

### 5.5 Finding observation

An observation is relevant only when:

1. its strict `party_id` resolves to the requested canonical Party;
2. an exact parent finding exists with the same `finding_id` and Party;
3. observation and finding agree on rule-set version and rule key;
4. the observation id matches the deterministic `(finding_id, Party resource version)` identity;
5. the Party resource version is positive.

Historical observations remain valid even when they are no longer the finding's current observation. The adapter must not require every historical observation to equal `current_observation_id`; it must require the current finding pointer itself to resolve correctly.

An orphan or cross-Party observation fails closed.

### 5.6 Party completeness result

A completeness result is relevant only when:

1. its strict `party_id` resolves to the requested canonical Party;
2. an exact evaluation job exists with the same `job_id` and Party;
3. the result references the job's exact completeness profile;
4. the profile and rule set are strictly rehydrated and mutually bound;
5. every component references an exact strict rule outcome from the same job, Party and rule set;
6. component keys, rule keys, outcome ids and awarded basis points satisfy the owner domain;
7. awarded component sum equals the stored score exactly;
8. deterministic result identity and Party resource version match the job lineage.

A result must not be included from profile id, job id or component outcome ids alone.

Scores, component details and outcome reasons must never enter response bytes.

### 5.7 Remediation attempt

A remediation attempt is relevant only when:

1. its strict `party_id` resolves to the requested canonical Party;
2. the exact parent finding exists for the same Party;
3. the exact referenced observation belongs to that finding and Party;
4. expected finding and Party versions are positive and internally consistent;
5. deterministic attempt and target idempotency identities satisfy the owner domain;
6. updated Party version is positive and follows the expected Party version;
7. the persisted attempt passes the existing strict owner decoder.

The attempt does not prove that the finding is currently remediated; a later deterministic evaluation remains authoritative for pass/fail state.

Requested display name, caller/target idempotency keys, actor-related evidence and version details remain private payload and must never enter response bytes.

## 6. Frozen bounds

The contract layer must publish and tests must enforce these maximums:

- `MAX_PRIVACY_EVALUATION_JOBS_SCANNED = 8_192`;
- `MAX_PRIVACY_EVALUATION_INPUTS_SCANNED = 8_192`;
- `MAX_PRIVACY_RULE_OUTCOMES_SCANNED = 32_768`;
- `MAX_PRIVACY_FINDINGS_SCANNED = 16_384`;
- `MAX_PRIVACY_FINDING_OBSERVATIONS_SCANNED = 32_768`;
- `MAX_PRIVACY_COMPLETENESS_RESULTS_SCANNED = 8_192`;
- `MAX_PRIVACY_REMEDIATION_ATTEMPTS_SCANNED = 8_192`;
- `MAX_PRIVACY_DEFINITION_RECORDS_REHYDRATED = 8_192`;
- `MAX_PRIVACY_ASSOCIATION_RECORDS_REHYDRATED = 65_536`;
- `MAX_PRIVACY_CANONICAL_PARTY_RESOLUTIONS = 65_536`;
- `MAX_PRIVACY_OWNER_RECORDS_SCANNED = 65_536`;
- `PRIVACY_OWNER_SCAN_BATCH_SIZE = 512`.

The shared owner contract continues to freeze:

- maximum page size `128`;
- maximum cursor bytes `2_048`.

Per-family and owner-wide counters count raw rows returned by PostgreSQL before relevance filtering, alias resolution, deduplication or association validation. Malformed, unrelated, duplicate and cross-tenant candidates do not refund counters.

Definition, association and canonical-resolution counters count actual rehydration/resolution attempts. Deduplicated in-memory cache hits may avoid another database read but may not erase already charged work.

Exceeding any bound before terminal completeness returns one stable fail-closed owner error and no successful partial contribution.

The values are intentionally suitable only for the contract-only proof. Runtime promotion requires measured SLO evidence or an independently governed owner-maintained canonical-subject index with migration, backfill, reconciliation, rollback and alias-convergence proof.

## 7. PostgreSQL access path and index decision

The contract-only implementation uses bounded same-tenant scans of `crm.records` for one exact owner record type at a time, ordered by `record_id ASC` with `record_id > last_record_id` keyset continuation.

The existing primary key:

`crm.records (tenant_id, record_type, record_id)`

is sufficient for this proof. The query must also validate `owner_module_id = 'crm.data-quality'`, deleted state and strict persistence metadata.

No new PostgreSQL index is required in the entry packet. In particular, implementation must not add:

- JSON or byte-payload expression indexes;
- a privacy-only projection table;
- synthetic `crm.relationships` rows duplicating payload lineage;
- an ungoverned Party-to-quality reverse index.

The implementation workflow must capture `EXPLAIN (COSTS OFF)` or equivalent structural proof that every family scan uses the bounded tenant/type/record-id path. A sequential full-tenant scan is a gate failure.

If existing planner behavior cannot prove the intended access path on clean PostgreSQL, the implementation must stop and propose a separately governed owner index migration rather than weakening scan bounds.

## 8. Deterministic seven-family pagination

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

Cursor tampering, tenant/Party/case/purpose/time/page-size rebinding, registry drift and stale topology generation fail closed.

## 9. Transaction and topology boundary

Each contribution request uses exactly one tenant-bound PostgreSQL transaction:

`REPEATABLE READ, READ ONLY`

The transaction must:

- bind tenant context before every owner read;
- rely on FORCE RLS;
- use the accepted read-only Identity Resolution snapshot proof;
- perform no advisory lock and no row lock;
- perform no writes;
- reuse the authoritative topology generation and validation semantics rather than reconstructing aliases locally.

The existing read-write merge/unmerge topology path retains its advisory and row locks. This packet must not alter or weaken that path.

## 10. Reference-only response

Every emitted resource contains only:

- exact resource type;
- exact resource id;
- positive resource version;
- `Personal` data class;
- minimized evidence class required by the shared privacy contract;
- exact owner retention-policy id.

The common contribution lineage may contain the canonical Party id. Owner resource evidence must not expose historical alias ids.

Encoded response bytes must exclude:

- staged Party kind and display name;
- definition titles, guidance, evaluator parameters and placeholder values;
- rule keys and reason codes unless they are part of the opaque resource identity itself;
- finding status, assignment, waiver reason and current-observation details;
- completeness score and component details;
- remediation requested display name and idempotency keys;
- raw timestamps, source versions, hashes, descriptor details or persisted JSON;
- unrelated or cross-tenant Party/resource identifiers;
- human-readable diagnostic or stewardship conclusions.

The contribution performs no access-export assembly, deletion, anonymization, restriction, legal-hold or retention decision.

## 11. Stable error boundary

The owner adapter must define stable safe errors for:

- request/coordinate/semantic-hash mismatch;
- cursor invalid or rebound;
- topology stale, invalid or unavailable;
- each malformed persisted family;
- missing or inconsistent definition lineage;
- missing or inconsistent job/input/outcome lineage;
- missing or inconsistent finding/observation lineage;
- missing or inconsistent completeness component lineage;
- missing or inconsistent remediation lineage;
- every frozen scan, definition, association and canonical-resolution bound;
- database unavailable.

Safe messages must not disclose Party ids, display names, rule content, finding status, actor assignments, waiver reasons, remediation values or idempotency material.

## 12. No-write proof

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

## 13. Required PostgreSQL acceptance matrix

The implementation PR must prove on clean PostgreSQL:

- canonical Party, historical alias Party, unrelated Party and same-id cross-tenant Party;
- at least one relevant record from each of the seven subject families;
- shared rule-set/profile definitions excluded from the response;
- unrelated and cross-tenant records excluded;
- alias-aware inclusion for every direct Party-bearing family;
- orphan input, outcome, observation, completeness and remediation evidence fail closed;
- mismatched job/Party/rule/profile/finding/observation/component association fail closed;
- malformed metadata and malformed canonical payload fail closed for every family;
- deterministic seven-family multi-page traversal;
- stable cursor, cursor rebinding rejection and stale topology rejection;
- reference-only response-byte exclusions;
- clean migrations and fixtures;
- complete rollback and absence of schema `crm`;
- reapply and repeated acceptance;
- no-write proof;
- workspace dependency graph;
- no regression in Identity Resolution privacy CI.

## 14. Permanent workflow requirement

The permanent `Data Quality Privacy Scope CI` was added with implementation code and proves:

1. architecture boundary;
2. formatting;
3. focused Clippy;
4. focused unit tests;
5. clean PostgreSQL migrations and fixtures;
6. access-path/index proof;
7. clean PostgreSQL acceptance;
8. complete rollback;
9. complete absence of schema `crm`;
10. migration and fixture reapply;
11. repeated PostgreSQL acceptance;
12. workspace dependency graph.

The workflow remains permanent after merge.

## 15. Explicit implementation exclusions

The implementation packet must add no:

- public HTTP or gRPC route;
- application runtime registration;
- Customer Privacy worker;
- generic privacy runtime;
- production discovery or planning;
- cross-owner storage access;
- direct Party mutation;
- new mutable definition or queue model;
- unbounded tenant scan;
- selective JSON parsing;
- shared-definition inclusion as Party evidence;
- runtime promotion of `data_quality.privacy.scope.contribute@1.0.0`.

## 16. Entry conclusion

The persisted record-family and subject-relevance semantics were implemented without weakening the frozen contract. PR #190 accepted the dedicated Data Quality privacy adapter and permanent workflow on source `dcfe8faebc7462b888f8fc1721cb379a40fea88a`, with 27 of 27 permanent workflows successful before squash merge `deac197c97cddc15bb9916092ca87f6e767ce1de`.

This packet is now an accepted historical contract. Customer Enrichment is the ninth and final owner. Production discovery remains forbidden until that final owner contribution is accepted.
