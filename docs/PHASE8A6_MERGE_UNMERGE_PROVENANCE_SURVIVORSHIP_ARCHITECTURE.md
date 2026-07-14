# Phase 8A.6 — Merge, Unmerge, Provenance and Survivorship Architecture

Status: **In progress — stacked on the exact-head-complete Phase 8A.5 packet**

Issue: #116  
Parent: #28  
Dependency: Phase 8A.5 / PR #115

## 1. Goal

Phase 8A.6 adds governed, reversible Party merge semantics without creating a second Party master and without destroying source evidence.

`crm.parties` remains the authoritative owner of Party records and Party field values. `crm.identity-resolution` owns the immutable merge lineage, active canonical redirection topology, merge/unmerge decision evidence and field-level survivorship provenance.

## 2. Non-destructive reference model

A merge does **not** bulk-rewrite every downstream Party reference.

Existing stored references remain historically stable. An active merge operation contributes one authoritative logical redirection edge:

```text
source Party -> survivor Party
```

Consumers that need the current canonical Party resolve the original reference through the authoritative merge-lineage resolver. Historical records can still retain and disclose the original Party reference when policy permits.

This makes unmerge reversible: unmerge deactivates one active redirection edge while preserving the immutable merge operation and every original downstream reference.

## 3. Owner boundaries

### `crm.parties`

Owns:

- Party identity and Party record versions;
- current Party field values;
- Party mutations.

Does not own:

- duplicate-candidate review state;
- merge lineage;
- canonical redirection topology;
- survivorship provenance.

### `crm.identity-resolution`

Owns:

- duplicate-candidate cases from Phase 8A.5;
- immutable merge-operation identity and lineage;
- active/unmerged merge-operation lifecycle;
- exact source and survivor Party versions captured at merge time;
- approval/reason evidence;
- field-level survivorship provenance;
- canonical Party resolution over active merge edges.

Does not directly write Party tables or downstream module state.

### Application composition

Later production layers validate:

- both Parties exist in the execution tenant;
- exact Party versions match the governed command;
- the source and survivor are current canonical roots before a new merge;
- survivorship provenance references belong to the source/survivor lineage and exact authoritative versions;
- any selected survivor Party field mutation is executed through an explicit governed Party-owner boundary;
- datastore and authorization failures are not disguised as missing references.

## 4. Merge operation model

A merge operation is an immutable lineage identity with a small reversible lifecycle:

```text
active -> unmerged
```

The initial operation records:

- stable merge operation ID;
- source Party reference and exact source version;
- survivor Party reference and exact survivor version;
- governed approval reference and approving actor;
- normalized decision reason;
- deterministic field-level survivorship selections;
- governed creation time.

Unmerge appends terminal reversal evidence:

- exact expected merge-operation version;
- governed approval reference and actor;
- normalized unmerge reason;
- governed unmerge time.

An unmerged operation cannot be reactivated. A later re-merge is a new immutable merge operation.

## 5. Canonical topology invariants

The active merge graph is a directed forest toward canonical roots.

Required invariants:

1. a Party has at most one active outgoing merge edge;
2. a merge operation ID appears at most once in the active graph;
3. self-merge is impossible;
4. cycles are impossible;
5. canonical resolution is bounded and deterministic;
6. a new merge source must currently be a canonical root;
7. a new merge survivor must currently be a canonical root;
8. roots with inbound merged Parties may themselves later merge into another root;
9. unmerge removes exactly one active edge and deterministically restores the topology implied by the remaining active edges.

Example:

```text
A -> B -> C
```

resolves `A`, `B` and `C` to `C`.

If the `B -> C` operation is unmerged, the remaining topology is:

```text
A -> B    C
```

and `A` resolves to `B` again without reconstructing or rewriting any original downstream reference.

## 6. Survivorship and provenance

A survivorship selection identifies:

- canonical field path;
- provenance Party reference;
- exact provenance Party version;
- SHA-256 digest of the source value used for the decision;
- immutable evidence reference.

The merge-lineage owner records provenance, not a competing copy of Party fields. Raw source values remain in authoritative Party/source records and governed evidence systems.

The same field may appear at most once in one merge operation. Selections are stored in deterministic field-path order.

Later composition may apply a selected value to the survivor only through a governed Party-owner mutation. The merge record must preserve enough provenance to explain which source/version/evidence produced the selected value.

## 7. Canonical resolver contract

The canonical resolver consumes only active authoritative merge edges and returns:

- requested Party;
- canonical Party root;
- ordered Party path;
- ordered merge-operation path.

Search, Customer 360, analytics and projections may cache or project this result but are never authoritative. Permission-sensitive reads repeat live authorization at disclosure time.

## 8. Concurrency and stale state

Merge and unmerge commands use exact optimistic concurrency.

Before merge, application composition must compare the command's Party source versions with current authoritative `crm.parties` versions. Stale versions fail explicitly.

Before unmerge, the target merge operation must still be active at the exact expected version. Replaying the same governed request is handled by platform idempotency; conflicting replay cannot create a second lineage edge.

## 9. Approval and AI boundary

Merge and unmerge are high-impact governed mutations. Production capability definitions must be approval-aware.

AI, fuzzy matching and deterministic candidate generation may recommend a merge or provide evidence. They cannot create an active merge edge, choose survivorship or unmerge without the governed capability and approval boundary.

## 10. Persistence direction

Phase 8A.6 uses a dedicated retained record type:

```text
identity_resolution.merge_operation
```

Candidate cases remain independent records:

```text
identity_resolution.candidate_case
```

The merge-operation persisted state must be strict, deterministic, versioned and canonically rehydratable. Active canonical topology is derived from authoritative active merge-operation records; any acceleration index is rebuildable.

## 11. Production acceptance target

The complete packet must prove on fresh PostgreSQL through real `crm-api`:

- same-tenant Party and exact-version integrity;
- self-merge and cycle rejection;
- one active outgoing edge per source;
- deterministic chain resolution;
- merge replay and conflicting replay;
- stale source-version rejection;
- field-level provenance preservation;
- no source Party deletion;
- no destructive downstream reference rewrite;
- unmerge restoration of prior canonical topology;
- unmerge replay/conflict behavior;
- tenant non-disclosure and live authorization;
- exact outbox, audit, idempotency and transaction evidence;
- projection/cache rebuild equivalence where acceleration is introduced.

## 12. Explicit Phase 8A.7 boundary

Bulk import/export, mapping versions, data-quality workflows, enrichment provenance and privacy deletion/restriction/legal-hold interaction proof remain Phase 8A.7.
