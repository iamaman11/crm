# Phase 8A.5 — Identity Resolution and Duplicate Candidates Architecture

## Status

Normative implementation packet for issue #114 and draft PR #115.

**Delivery state: implementation complete / exact-head gate review.** The production owner, governed mutation/query contracts and adapters, application composition, authoritative Party-to-case access path, canonical contract publication, browser descriptor synchronization, PostgreSQL registry fixture and fresh-PostgreSQL real `crm-api` process acceptance are implemented. The final merge condition remains all applicable workflows green together on one unchanged source-authored SHA.

Phase 8A.5 introduces an independently governed identity-resolution case owner. It does **not** merge, delete, alias or mutate Party identity. `crm.parties` remains the only authoritative Party owner; Party merge/unmerge, reference redirection, provenance and survivorship remain Phase 8A.6.

## Ownership boundary

`crm.identity-resolution` owns:

- deterministic tenant-scoped duplicate-candidate case identity for one canonical unordered Party pair;
- immutable evidence snapshots and matcher/signal provenance used to explain why a pair became a candidate;
- exact source Party versions captured by each evidence snapshot;
- reviewer lifecycle state: `open`, `dismissed`, or `confirmed_duplicate`;
- optimistic aggregate version and governed mutation timestamps;
- durable decision reason evidence.

It does not own:

- Party attributes or Party lifecycle;
- Contact Point, Account, Consent, Party Relationship or Customer 360 state;
- search indexes or probabilistic model state as authoritative identity truth;
- Party merge/unmerge, aliasing, reference redirection, survivorship or golden-record field selection.

## Core invariants

1. A candidate pair is unordered. `(A, B)` and `(B, A)` resolve to the same canonical pair and deterministic case identifier.
2. A Party cannot be paired with itself.
3. Candidate evidence is explainable and version-bound. Every snapshot records exact Party source versions, matcher profile, bounded score, generated time and non-empty canonical signal provenance.
4. Evidence snapshots are immutable. Refresh appends a new snapshot; it never rewrites prior evidence.
5. Evidence refresh is allowed only while the case is open and cannot regress either Party source version. At least one Party source version must advance.
6. Reviewer decisions are terminal in Phase 8A.5. `dismissed` and `confirmed_duplicate` cannot be reopened or converted into each other.
7. `confirmed_duplicate` is evidence for a later governed merge workflow. It is not a merge command and must produce no Party mutation or reference redirection.
8. All mutations require exact optimistic version and strictly increasing governed mutation time.
9. Persisted state is versioned, deterministic, bounded and strictly revalidated on rehydration; malformed or semantically non-canonical state is rejected as corruption.

## Application composition boundary

The pure owner module has no SQL, transport types or direct Party storage access. Production composition validates both Party references before candidate registration:

- both Parties exist in the request tenant;
- missing and cross-tenant references have one safe non-disclosing public result;
- the exact authoritative Party versions match the versions claimed by the evidence snapshot;
- real datastore failures remain distinguishable internally and are not converted into missing-reference results.

Refresh repeats exact Party-version validation for the new evidence snapshot. Terminal reviewer decisions also fail closed when the current evidence snapshot no longer matches the authoritative Party versions, preventing a stale duplicate decision from being recorded silently.

## Authoritative access model

A single canonical candidate case exists per tenant and canonical Party pair. Registration persists authoritative Party-to-case relationships for both endpoints atomically with the case mutation so permission-aware list-by-Party queries do not require tenant-wide scans or depend on rebuildable search/Customer 360 projections.

Derived candidate generation may later be rebuilt or recomputed from authorized Party/search signals, but reviewer decisions and accepted evidence history are durable owner state.

## Governed capabilities

Mutation surface:

- `identity_resolution.candidate.register@1.0.0`
- `identity_resolution.candidate.evidence.refresh@1.0.0`
- `identity_resolution.candidate.dismiss@1.0.0`
- `identity_resolution.candidate.confirm_duplicate@1.0.0`

Query surface:

- `identity_resolution.candidate.get@1.0.0`
- `identity_resolution.candidate.list_by_party@1.0.0`

Public contracts are additive `crm.identity_resolution.v1` Protobuf contracts. Private aggregate persistence remains native owner implementation state.

## Acceptance evidence

Fresh PostgreSQL plus a real `crm-api` process acceptance test proves:

- canonical pair identity is independent of input order;
- self-pairs and duplicate cases fail without side effects;
- missing/cross-tenant Party references are safely rejected;
- stale claimed Party source versions are rejected;
- register/replay/conflicting replay behavior is exact and idempotent;
- evidence refresh appends immutable evidence and rejects source-version regression;
- dismiss and confirm-duplicate are exact-versioned terminal transitions;
- confirmation does not mutate either Party or create merge/reference-redirection evidence;
- get/list-by-Party are permission-aware, tenant-isolated and use signed bound cursors where pagination applies;
- durable record, relationship, outbox, audit, idempotency and business-transaction evidence is exact;
- deterministic persistence round-trip and corruption rejection are tested.

The process-acceptance source head `9075f408925ed5a74260e5ec129807033b5e3f2a` passed 10 of 11 applicable workflows; the only failing Rust CI step was `cargo fmt --check`. `Rust Generated Sync` then applied the required formatting successfully. This status commit intentionally invalidates earlier exact-SHA evidence; PR #115 must still pass every applicable workflow together on the new unchanged head before merge.
