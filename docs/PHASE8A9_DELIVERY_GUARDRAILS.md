# Phase 8A.9 — Delivery Guardrails for Sustained 10/10 Engineering

## Status and authority

This document is the packet-level execution control for issue #124 and draft PR #132.

It is subordinate to, and must never weaken:

1. `SYSTEM_INVARIANTS.md`;
2. `DELIVERY_GOVERNANCE.md`;
3. `IMPLEMENTATION_ROADMAP.md`;
4. `PHASE8_DELIVERY_PLAN.md`;
5. `CRM_CAPABILITY_COVERAGE.md`;
6. `PHASE8A9_DATA_QUALITY_ARCHITECTURE.md`.

Delivery state: **In progress**. Phase 8A.8 is merged. PR #132 is the single active customer-master production packet. Phase 8A.10 and 8A.11 remain non-active follow-on packets.

This document exists to keep later implementation work from satisfying local tests while weakening the overall product, architecture or evidence standard.

## 1. Completion claim discipline

PR #132 must remain draft until every required production layer is present and one unchanged final SHA passes all applicable gates.

The following claims remain distinct:

- domain semantics implemented;
- public contract implemented;
- adapter implemented;
- application runtime composed;
- fresh-process acceptance implemented;
- packet ready for Gate review;
- packet merged and Complete;
- module product complete;
- universal CRM complete.

No lower claim may be presented as a higher claim. Green CI for an incremental head is evidence for that head and implemented slice only; it is not evidence that Phase 8A.9 is complete.

## 2. Frozen ownership and execution boundaries

All later work in this packet must preserve these boundaries:

- `crm.data-quality` owns quality-governance definitions, evaluation evidence, findings, completeness results, stewardship state and remediation-attempt evidence only;
- authoritative Party values remain owned by `crm.parties`;
- no Data Quality crate, adapter or worker may read or write another module's tables directly;
- authoritative source reads use governed owner/query composition with live top-level authorization and resource/field visibility;
- remediation re-enters an exact versioned Party owner capability with its normal authorization, optimistic concurrency, approval, idempotency and audit behavior;
- no arbitrary SQL, tenant-supplied regex, script, user code, filesystem, shell, unrestricted network or unbounded expression execution is introduced;
- derived search, Customer 360 and projection data cannot become authoritative evaluation evidence without exact owner/resource-version lineage.

A change that violates one of these rules is not an implementation shortcut; it is a packet-blocking architecture defect.

## 3. Identity and canonicalization gate

Rule-set, completeness-profile, job, outcome, finding, observation and remediation identities are durable semantic contracts.

Before Gate review:

1. every semantic digest must use an explicitly versioned canonicalization profile;
2. the implementation must be reconciled with `SYSTEM_INVARIANTS.md` requirements for `crm.cjson/v1`, or an approved ADR must define the exact compatible domain-specific profile, migration path and risk acceptance;
3. the canonicalization profile identifier must be stored or otherwise inseparably bound beside every durable semantic digest;
4. canonical ordering, normalization, integer encoding and domain separation must be mechanically tested;
5. exact known identity fixtures must prevent an implementation or dependency upgrade from silently reinterpreting already published versions;
6. changing identity semantics requires a new semantic/profile version and must not reinterpret persisted v1 records.

Until the profile-storage decision is fully implemented, the exact identity regression fixtures are mandatory and the packet cannot enter Gate review.

## 4. Tenant, authorization and disclosure proof

Acceptance must distinguish different rejection layers instead of treating every denial as tenant-isolation proof.

Required independent evidence:

- an unauthenticated request is rejected at ingress;
- a token not authorized for a tenant is rejected at tenant/context resolution;
- an authenticated and tenant-authorized actor cannot read another tenant's record through the query/storage boundary;
- FORCE RLS prevents cross-tenant direct storage access under the application role;
- a visible resource with a hidden field returns the resource with that field redacted;
- a non-visible, missing or cross-tenant resource uses the same safe non-disclosing public behavior where required;
- possession of a record id is never treated as authority;
- every protected query repeats live capability authorization and resource/field visibility.

A cross-tenant test that fails before entering the authorized query/storage path does not satisfy the storage-isolation gate by itself.

## 5. Ordered implementation sequence

Work continues in this order unless a documented dependency requires a smaller preparatory commit:

1. rule-set publish/get foundation and exact identity/persistence regression protection;
2. completeness-profile publish/get with strict same-owner rule-set binding;
3. governed Party quality source composition;
4. durable evaluation job creation and exact immutable source staging;
5. bounded restart-safe worker execution;
6. deterministic outcomes, findings, observations and completeness results;
7. permission-aware finding/completeness/stewardship queries;
8. optimistic-concurrency assignment, acknowledgement and waiver;
9. governed display-name remediation through `parties.party.update@1.0.0`;
10. target-success/data-quality-outcome-missing recovery;
11. FORCE RLS, migration clean apply, rollback and reapply evidence;
12. full two-tenant, visibility, restart/retry and reconciliation process acceptance;
13. documentation synchronization and one unchanged exact-head final gate.

A later production layer must not become the active merge target before its prerequisite layer is integrated and verified on the current branch.

## 6. Test and CI quality rules

Every new public capability or query requires, as applicable:

- contract compile, descriptor and round-trip checks;
- exact public capability metadata tests;
- strict unknown-field and malformed persisted-state rejection;
- positive behavior tests;
- invalid input with zero durable side effects;
- same-key replay and changed-request conflict proof;
- cross-tenant and hidden-resource negative proof;
- resource-visible/field-redacted proof;
- durable record, idempotency, outbox, audit and business-transaction reconciliation;
- fresh-PostgreSQL real `crm-api` process acceptance;
- restart/crash-window proof for asynchronous or cross-owner operations;
- explicit bounds for payloads, collections, scans, batches and per-tenant work.

Tests must prove the intended layer. A passing ingress rejection cannot substitute for RLS proof; a unit test cannot substitute for real-process composition; a process smoke test cannot substitute for exact durable evidence assertions.

## 7. Operational and product-quality continuity

Phase 8A.9 does not by itself make the universal CRM complete. Nevertheless, it must leave production-hardening hooks intact:

- bounded per-tenant scheduling and fairness;
- stable typed error codes and safe external messages;
- trace, correlation and causation continuity;
- retention-aware private evaluation evidence;
- no sensitive raw Party value in events, generic queries, logs or audit envelopes;
- deterministic reconciliation counters and diagnostics;
- readiness failure when required workers cannot make safe progress;
- migration and compatibility discipline suitable for later backup/restore, residency and legal-hold integration.

Performance, security, restore and SLO claims require measured evidence under Phase 11. Documentation or green functional tests alone must not be used for those claims.

## 8. Documentation synchronization checkpoint

During active development, merged `main` documents may correctly continue to show 8A.9 as Ready because only merged work may be Complete. Before Gate review, the candidate branch must synchronize:

- `IMPLEMENTATION_ROADMAP.md`;
- `PROJECT_STATUS.md`;
- `PHASE8_DELIVERY_PLAN.md`;
- `MODULE_CATALOG.md` only to the readiness justified by the completed candidate;
- issue #124;
- PR #132 body and validation state.

After merge, `main` must record 8A.9 as Complete and move the single active customer-master packet to 8A.10. The module catalog must not count `crm.data-quality` as an implemented production module before the qualifying vertical slice is merged.

## 9. Gate-review entry checklist

PR #132 may leave draft only when all answers are yes:

- Is every required public surface implemented and composed in the real application?
- Are all authoritative reads and mutations routed through governed owner boundaries?
- Are deterministic identities explicitly profile-versioned and regression-locked?
- Are findings and completeness evidence bound to exact Party versions?
- Are restart, replay and both cross-owner crash windows proven without duplicate effects?
- Are authorization, field redaction, safe non-disclosure and true two-tenant storage isolation independently proven?
- Are all scans, jobs, payloads and queues bounded per tenant?
- Do migrations prove clean apply, rollback and reapply with FORCE RLS?
- Is historical evidence immutable across lifecycle changes?
- Are roadmap, status, phase plan, issue and PR descriptions synchronized?
- Has every applicable workflow passed on one unchanged source-authored SHA?
- Are there no unresolved blocking review threads or known gate defects?

Any no keeps the PR in **In progress**.