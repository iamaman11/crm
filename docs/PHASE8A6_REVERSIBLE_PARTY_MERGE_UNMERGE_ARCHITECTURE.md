# Phase 8A.6 — Reversible Party Merge/Unmerge, Provenance and Survivorship Architecture

Status: **Normative foundation for implementation**

Issue: #117  
Parent program: #28  
Depends on: completed Phase 8A.5 / #114 / merged PR #115

## 1. Purpose

Phase 8A.6 adds a separate governed Party merge/unmerge lifecycle after duplicate-candidate review. It does not reinterpret a `confirmed_duplicate` decision as an automatic merge and it does not create a second customer master.

The design must preserve all of the following at the same time:

- `crm.parties` remains the authoritative owner of Party identity state;
- `crm.identity-resolution` owns reviewed duplicate evidence, immutable merge lineage, survivorship decisions and provenance;
- absorbed Party identity is never hard-deleted by merge;
- existing downstream Party references remain stable and do not require a cross-module rewrite transaction;
- canonical active identity can be resolved explicitly through a governed query boundary;
- unmerge is a real inverse lifecycle with optimistic conflict detection, not a best-effort recreation;
- no owner module gains raw cross-owner SQL mutation authority.

## 2. Owner boundaries

### 2.1 `crm.parties`

`crm.parties` continues to own the authoritative Party aggregate. Phase 8A.6 may extend that aggregate with explicit lifecycle state sufficient to represent an active Party or a merged/redirected Party.

Only Party-owned domain and Party-owned planner code may construct authoritative Party persisted state. Other modules and application composition may validate, coordinate and combine owner-produced plans, but they may not serialize private Party state or issue raw SQL updates against Party records.

Party ID and Party kind remain immutable.

### 2.2 `crm.identity-resolution`

`crm.identity-resolution` owns:

- duplicate-candidate review evidence from Phase 8A.5;
- immutable merge operation identity;
- the exact reviewed candidate-case reference and version used to authorize a merge attempt;
- explicit survivor and absorbed Party roles;
- field-level survivorship decisions;
- source Party/value/version provenance for each chosen field value;
- before/after Party version coordinates;
- merge lifecycle status and unmerge history;
- reviewer/actor evidence and governed mutation time.

The merge-lineage record is authoritative historical evidence. It is not a copy of the whole Party master and it does not become the Party query source of truth.

### 2.3 Application composition

Application composition is responsible for cross-owner integrity and orchestration:

1. read the exact candidate case and Party records from authoritative owner stores;
2. validate tenant, pair, case status, case version, Party kind, Party lifecycle and exact Party versions;
3. invoke owner-specific planning code so each owner constructs only its own state changes;
4. combine the resulting owner-produced mutations/evidence into one platform atomic business transaction where supported by the transactional batch executor;
5. reject the operation before persistence when any owner plan or cross-owner invariant fails.

Composition must not duplicate private persistence encoders from either owner.

## 3. Why merge does not rewrite downstream references

A universal CRM cannot safely update every current and future module that stores a Party reference inside one merge transaction. Such a design would couple merge correctness to all installed modules and would make unmerge effectively impossible.

Therefore Phase 8A.6 uses stable identity plus explicit canonical resolution:

- the absorbed Party record remains durably addressable for history, audit, provenance and direct authorized inspection;
- the absorbed Party is marked as merged/redirected to a survivor through Party-owned state;
- downstream records may continue to store the original stable Party reference;
- consumers that require the current canonical active identity call a governed resolver or consume an explicitly versioned canonical-resolution projection;
- unmerge removes the redirect through the Party owner instead of attempting to recreate deleted identities or reverse foreign-key rewrites.

A projection or cache may accelerate canonical resolution, but Party owner state remains authoritative.

## 4. Merge preconditions

The initial v1 merge command must fail closed unless all conditions hold:

1. survivor and absorbed Party references are distinct;
2. both Parties exist in the execution tenant;
3. both Parties have the same immutable Party kind;
4. both Parties are currently eligible for merge;
5. the absorbed Party is not already merged/redirected;
6. exact expected Party versions match authoritative versions;
7. the referenced duplicate-candidate case exists in the same tenant;
8. the case canonical pair exactly matches the survivor/absorbed pair independent of direction;
9. the case status is `confirmed_duplicate`;
10. the exact case version supplied by the command matches authoritative state;
11. the evidence snapshot used for the confirmation is not stale relative to the Party versions required by the merge policy;
12. every survivorship decision references a supported mutable field and valid source provenance;
13. governed time is monotonic for every owner state transition;
14. required authorization and approval evidence is present before execution.

A fuzzy score, AI suggestion or `confirmed_duplicate` status alone never determines merge direction.

## 5. Party lifecycle model

The initial Party lifecycle extension should represent at least:

- `active`;
- `merged`, including the canonical survivor Party reference and authoritative merge-lineage reference.

The absorbed Party retains:

- immutable Party ID;
- immutable Party kind;
- its own historical field values;
- created time;
- version history through events/audit;
- explicit redirect metadata while merged.

A merged Party cannot be updated through normal mutable Party commands until it is successfully unmerged.

A survivor remains active and may be referenced directly. Later support for multiple absorbed Parties must preserve unambiguous lineage; an already absorbed Party cannot be merged again while redirected.

## 6. Survivorship and field provenance

Survivorship is an explicit decision, not a side effect of Party ordering.

For every mutable field changed by merge, the lineage record stores:

- field identifier;
- chosen canonical value;
- source Party reference;
- source Party version;
- survivor pre-merge value;
- survivor pre-merge version;
- resulting survivor version.

For the current Party model, `display_name` is the first governed survivorship field.

A v1 merge may choose either:

- retain the survivor's current display name; or
- take the absorbed Party's current display name.

Arbitrary free-form replacement values are out of scope for the first merge packet because they would blur merge provenance with ordinary Party editing.

No source value is silently discarded: both original Party records remain durable and the lineage record captures the explicit choice.

## 7. Merge lineage lifecycle

The merge-lineage aggregate is append-preserving and has an immutable operation identity.

Initial states:

- `active` — merge has been applied and the absorbed Party is redirected;
- `unmerged` — the exact merge operation has been reversed through the governed unmerge lifecycle.

A lineage record is never deleted when unmerge occurs.

The record stores at minimum:

- merge ID;
- candidate-case ID and exact case version;
- survivor Party reference;
- absorbed Party reference;
- Party kinds validated at merge time;
- exact pre-merge Party versions;
- exact post-merge Party versions;
- survivorship decisions and provenance;
- merge actor/reviewer reference;
- merge time;
- current lineage version;
- optional unmerge actor/reason/time and exact restoration versions.

## 8. Merge identity

Merge IDs are deterministic for idempotent semantic identity only when the command's immutable identity coordinates are the same. The identifier profile must be explicitly versioned.

The first implementation should include at least:

- candidate-case ID;
- candidate-case version;
- survivor Party ID;
- absorbed Party ID.

The exact hash namespace and canonical byte encoding are owner-domain details and must be covered by deterministic tests.

A later merge of the same pair after a completed unmerge must not silently reuse an old active lineage record unless the versioned identity profile intentionally includes a new reviewed case/version coordinate that makes it a distinct operation.

## 9. Atomic execution model

The target production execution is one governed business transaction containing owner-produced changes:

- Party survivor mutation when survivorship changes an owned Party field;
- absorbed Party lifecycle transition to merged/redirected;
- identity-resolution merge-lineage record creation/update;
- owner events in the outbox;
- audit evidence;
- idempotency evidence;
- business transaction evidence.

The application composition layer may combine plans, but each state payload and owner event must be produced by the owning module's adapter/domain code.

If the current transactional planner cannot safely combine multiple owner-produced record mutations, the platform capability must be extended explicitly rather than bypassed with ad hoc SQL.

## 10. Unmerge semantics

Unmerge is an explicit governed command over a specific active merge-lineage record.

The initial v1 unmerge must require:

- the lineage record is currently active;
- the absorbed Party is still merged into the expected survivor through that lineage;
- exact current Party versions match the versions expected by the lineage/unmerge command;
- the survivor state that would be restored has not been overwritten by unrelated later changes;
- governed time is monotonic;
- authorization and approval requirements pass.

On success:

- absorbed Party returns to active state;
- the survivor field changed by this merge is restored only when the current authoritative value/version still matches the merge result owned by this lineage;
- the lineage record becomes `unmerged` and records immutable reversal evidence;
- no historical merge event, audit record or lineage record is deleted.

If later Party mutations make exact restoration unsafe, unmerge fails with a typed conflict. It must never overwrite newer unrelated customer-master edits.

## 11. Query semantics

Phase 8A.6 requires permission-aware governed queries for at least:

- get merge lineage by merge ID;
- list merge lineage by Party;
- resolve a Party reference to its current canonical active Party.

Canonical resolution must distinguish:

- requested Party reference;
- canonical Party reference;
- whether redirection occurred;
- authoritative versions used for the decision.

Tenant non-disclosure and live resource/field visibility remain mandatory.

## 12. Events

The initial event vocabulary should include separate owner events for:

- Party marked merged/redirected;
- Party reactivated by unmerge;
- Party field updated by merge survivorship when applicable;
- merge lineage created/applied;
- merge lineage unmerged.

Event naming and payloads must preserve owner boundaries. Identity-resolution events must not masquerade as Party state mutation evidence, and Party events must not erase the merge-lineage reference that explains why the state changed.

## 13. Approval and risk

Merge and unmerge are high-impact customer-master mutations. The capability definitions should be approval-aware and may require explicit approval policy before execution.

Tests must prove that approval/authorization cannot be bypassed through direct adapter invocation in the deployable application path.

## 14. Initial non-goals

- automatic merge from matching score or AI recommendation;
- hard deletion of absorbed Party;
- bulk graph merge;
- cross-kind person/organization merge;
- mandatory rewrite of all downstream Party references;
- privacy deletion, legal hold or data-subject export orchestration;
- arbitrary free-form field editing disguised as survivorship;
- treating Search or Customer 360 as merge authority.

## 15. Required production acceptance

Fresh-PostgreSQL real `crm-api` process acceptance must prove at minimum:

1. missing, cross-tenant, self and cross-kind merges fail safely;
2. an open/dismissed/stale candidate case cannot authorize merge;
3. exact confirmed-case and Party versions are enforced;
4. merge direction is explicit;
5. absorbed Party is preserved and becomes redirected, not deleted;
6. survivor and absorbed Party direct historical reads remain authorization-bound;
7. canonical resolution returns the survivor for the absorbed Party;
8. downstream records are not rewritten as a merge prerequisite;
9. survivorship changes and provenance are exact and reconstructable;
10. idempotent replay produces no duplicate effects;
11. semantic replay conflict is typed;
12. merge writes owner events, audit, idempotency and transaction evidence atomically;
13. unmerge restores exact eligible state and preserves immutable lineage;
14. unmerge fails closed after conflicting later Party mutation;
15. tenant B cannot discover tenant A lineage or redirects;
16. all applicable CI workflows are green together on one unchanged final SHA.

## 16. Delivery sequence

1. normative architecture and owner boundaries;
2. pure merge-lineage/survivorship owner domain;
3. Party merge lifecycle domain and strict Party persistence upgrade;
4. strict merge-lineage persistence;
5. additive versioned Protobuf contracts and canonical registry bindings;
6. Party-owned merge/unmerge planning surface;
7. atomic cross-owner composition and exact integrity checks;
8. canonical-resolution and lineage query adapters;
9. application runtime and capability registry wiring;
10. fresh-PostgreSQL real `crm-api` acceptance;
11. roadmap/project/module status synchronization;
12. one unchanged exact final SHA with all applicable workflows green before merge.
