# Customer Privacy deterministic planning and read-boundary freeze

Status: **Contract and acceptance semantics frozen; production planning runtime not started.**

Issue: #126  
Parent phase: 8A.11  
Accepted discovery baseline: PR #206 / source `086b17a95058eee285fcb67a903bd21d9263d357` / merge `95818fd3aeb54a9593a45642583f0b7224d5ecfe`  
Accepted evidence synchronization: PR #207 / merge `090e8991da091ea894a1cb684bcaa19984b14f1c`

Machine-readable source of truth: `contracts/customer-privacy-planning-freeze.json`.

## 1. Purpose

This packet freezes the exact contract between the accepted immutable discovery snapshot and later approval, restriction, retention and owner-execution packets.

It defines a pure deterministic action plan and permission-aware read boundaries. It does **not** register `plan.build` in production, add public routes, place restrictions, decide legal holds or retention, approve a case, dispatch owner actions or perform deletion, anonymization or crypto-shredding.

## 2. Exact coordinates

- trusted internal phase-270 builder: `customer_privacy.plan.build@1.0.0`;
- permission-aware public read: `customer_privacy.case.plan.get@1.0.0`;
- permission-aware public read: `customer_privacy.case.owner_outcomes.list@1.0.0`.

`plan.build` has no public HTTP/gRPC ingress. The two read coordinates remain published in the module manifest but are not promoted into the current production query inventory by this freeze.

The current merged production inventory remains four public mutations, two permission-aware public queries and zero Customer Privacy workers.

## 3. Immutable planning lineage

Every action plan binds all of the following:

1. tenant and privacy case identity;
2. canonical Party identity and exact Identity Resolution generation;
3. source privacy-case version in `Scoped` state;
4. exact discovery snapshot ID;
5. exact discovery snapshot binding digest;
6. exact scope completeness and registry digests;
7. discovery purpose and effective request time;
8. snapshot capture time;
9. privacy-case kind;
10. policy version and jurisdiction code;
11. immutable `approval_required` input;
12. explicit crypto-shred support evidence.

A stale case version, changed snapshot reference, topology/registry drift, policy or jurisdiction drift, or any digest mismatch fails closed. The planner may not silently rediscover, rebase or reinterpret the snapshot.

## 4. Exact action set

The immutable action vocabulary is:

- `Retain`;
- `RestrictOnly`;
- `Anonymize`;
- `Delete`;
- `CryptoShred`;
- `NoOpAlreadyCompliant`.

`NoOpAlreadyCompliant` is reserved for a later owner-evidence-aware replay path. The initial deterministic planner cannot infer owner compliance and therefore never emits it.

## 5. Frozen case-kind mapping

### Access

Every scoped resource receives:

- action `Retain`;
- reason `AccessDisclosureOnly`.

This plan is disclosure-only and performs no owner mutation.

### Portability export

Every scoped resource receives:

- action `Retain`;
- reason `PortabilityDisclosureOnly`.

Export assembly remains a later packet through Customer Data Operations.

### Restrict processing

Every scoped resource receives:

- action `RestrictOnly`;
- reason `RestrictionRequested`.

The plan records intended treatment only. It does not place or release a processing restriction.

### Erasure

| Evidence class | Planned action | Reason |
|---|---|---|
| `DestroyableSubjectData` | `Delete` | `ErasureDestroyableSubjectData` |
| `RetainMinimizedEvidence` | `Anonymize` | `ErasureRetainMinimizedEvidence` |
| `ImmutableRequiredEvidence` | `Retain` | `ErasureImmutableRequiredEvidence` |
| `DerivedRebuildableState` | `Delete` | `ErasureDerivedRebuildableState` |
| `CryptoShreddableData` | `CryptoShred` only with explicit support | `ErasureCryptoShreddableData` |

Unsupported crypto-shred fails closed. The planner may not silently convert it to delete, anonymize or retain.

Unknown/future case kinds, data classes, evidence classes, actions or reasons fail closed through strict enum/state rehydration.

## 6. Retention and legal-hold boundary

The snapshot's `retention_policy_id` is only immutable owner classification input. This packet does not adjudicate mandatory retention, ordinary retention or legal holds.

The precedence rule remains reserved for later packets:

```text
active legal hold > mandatory retention > approved privacy action > ordinary retention
```

The plan may record `Retain` for `ImmutableRequiredEvidence`, but that does not constitute a legal-hold or retention decision.

## 7. Deterministic item ordering and identity

Each item contains only safe reference/classification evidence:

- contiguous sequence beginning at one;
- owner module ID;
- resource type, ID and version;
- data class and evidence class;
- retention-policy ID;
- planned action and reason;
- item digest.

Canonical ordering is owner module, resource type, resource ID, resource version, data class, evidence class, retention-policy ID, planned action and reason.

Resource payloads and owner-private metadata are forbidden. Duplicate resource identity, non-contiguous sequence or non-canonical ordering fails closed.

Digest profiles are frozen:

- lineage: `crm.customer-privacy.action-plan-lineage/v1`;
- item: `crm.customer-privacy.action-plan-item/v1`;
- plan: `crm.customer-privacy.action-plan/v1`.

Plan IDs use `privacy-action-plan-<sha256>`. The exact same lineage, planned time and ordered items replay to the same plan ID and canonical bytes. Conflicting replay fails closed.

A complete snapshot with zero resources produces one valid empty immutable plan.

## 8. Persistence contract

- record type: `customer-privacy.action-plan`;
- schema: `crm.customer-privacy.action_plan.state@1.0.0`;
- encoding: strict `crm.cjson/v1` canonical JSON;
- maximum bytes: `524288`;
- maximum items: `16384`;
- retention policy: `crm.customer_privacy.action_plan`;
- append-only after finalization;
- deny unknown fields;
- recompute lineage, item and plan digests plus plan ID during rehydration.

Whitespace, unknown fields, alternate decimal encoding, reordered items, action/reason tampering or digest/ID mismatch is rejected.

## 9. Permission-aware reads

Possession of a plan or outcome ID is never authority.

`case.plan.get` requires tenant binding, live authorization, field visibility and allow/deny audit evidence.

`case.owner_outcomes.list` is frozen as a future-safe read boundary. Before owner execution exists, the only valid result is an empty terminal page. Synthetic outcomes are forbidden.

Outcome pagination bounds are:

- default page size `64`;
- maximum page size `128`;
- maximum cursor size `2048` bytes.

## 10. Failure and replay semantics

Fail closed on:

- case not `Scoped` or case version drift;
- case/snapshot reference mismatch;
- snapshot identity, binding, completeness, registry or topology mismatch;
- policy-version or jurisdiction mismatch;
- unknown/future classification values;
- unsupported crypto-shred;
- duplicate identity, item order or sequence conflict;
- lineage, item, plan digest or plan ID mismatch;
- non-canonical persisted state;
- conflicting replay.

A transient storage failure before finalization may retry from the same exact lineage. Once finalized, the plan is immutable.

## 11. Explicit non-effects

This freeze adds:

- no public `plan.build` route;
- no Customer Privacy worker;
- no owner mutation or provider call;
- no approval;
- no restriction state change;
- no legal-hold or retention adjudication;
- no owner dispatch or outcome recording;
- no access/export request;
- no deletion, anonymization or crypto-shred execution;
- no case-state transition runtime;
- no new crate, dependency family or generic-runtime business switch.

The workspace remains at 113 packages.

## 12. Runtime implementation boundary

After this contract is accepted, runtime implementation must remain inside:

```text
modules/crm-customer-privacy/
crates/crm-customer-privacy-application/
crates/crm-customer-privacy-postgres/
crates/crm-customer-privacy-production/
```

It must add no capability-specific crate and must not modify generic router/worker algorithms merely to register Customer Privacy behavior.

## 13. Required acceptance before runtime promotion

The later runtime packet must prove:

1. exact case-kind/evidence mapping;
2. deterministic lineage/item/plan digests and IDs;
3. case version, snapshot, policy and jurisdiction sensitivity;
4. unsupported crypto-shred fail-closed behavior without fallback;
5. valid empty-plan behavior;
6. no inferred `NoOpAlreadyCompliant`;
7. strict canonical persistence and tamper rejection;
8. permission-aware plan and outcome reads with safe audit;
9. FORCE RLS, cross-tenant negatives, clean PostgreSQL, rollback/removal, reapply and repeated acceptance;
10. unchanged route/worker/non-runtime classification;
11. unchanged package/dependency boundary;
12. all applicable workflows on one unchanged exact source SHA.

Approval, restrictions, legal holds, retention decisions and owner execution remain separate later packets even after planning runtime acceptance.
