# Customer Privacy approval runtime

Status: **Accepted and merged through PR #220.**

Acceptance evidence:

- accepted source: `98000b0c1c2c15e14c7ee0cd2a366020040567e6`;
- permanent workflows: **21 of 21** successful on the unchanged source-authored SHA;
- squash merge: `01118df3b6349b6d854c4182c17f7eb9a6316b9c`.

Historical sources remain immutable:

- `contracts/customer-privacy-planning-freeze.json` / PR #208 freeze planning and approval semantics before runtime work;
- `contracts/customer-privacy-planning-implementation.json` / PR #209 record accepted trusted-internal planning runtime;
- `contracts/customer-privacy-plan-reads-implementation.json` / PR #211 record accepted permission-aware plan/outcome reads;
- this packet records only the accepted approval mutation and its evidence.

## Runtime coordinate

`customer_privacy.case.approve@1.0.0` is a public high-risk mutation. It uses the existing Customer Privacy domain, application, PostgreSQL and production packages and adds no workspace package or dependency family.

The accepted public inventory is:

- five mutations;
- four permission-aware queries;
- zero Customer Privacy workers;
- 113 workspace packages.

## Request and authorization boundary

Every request requires:

- active `crm.customer-privacy` module for the tenant;
- live permission authorization;
- tenant-bound case identity;
- expected case version;
- stable actor, request, correlation and trace identity;
- an idempotency key.

Unauthorized and cross-tenant existence is concealed. The mutation accepts only a case in `AwaitingApproval` and transitions it to `Planned`.

## Locked evidence and strict rehydration

The transaction locks the privacy case with `FOR UPDATE` and locks the immutable scope snapshot and action plan with `FOR SHARE`. It strictly rehydrates and verifies:

1. the exact tenant-bound privacy case;
2. the canonical subject and Identity Resolution generation;
3. the immutable discovery snapshot referenced by the case;
4. the immutable deterministic plan referenced by the case;
5. case kind, policy, registry, purpose and effective-time lineage;
6. snapshot binding/completeness and plan digests;
7. the approval-required flag and eligible source status.

Missing, corrupt, cross-linked or conflicting evidence fails closed.

## Atomic approval evidence

A successful approval atomically persists:

- the `AwaitingApproval → Planned` case transition and new version;
- immutable approval actor and approval time;
- append-only status/event evidence;
- typed audit evidence;
- idempotency request/result evidence;
- the business transaction outcome.

No partial approval evidence may remain after a failed transaction. Exact replay returns the original result; a replay with conflicting request identity or payload is rejected.

## Explicit non-effects

This packet does not implement or imply:

- processing-restriction placement or release;
- legal-hold or mandatory-retention adjudication;
- owner execution or owner-outcome persistence;
- access/export assembly;
- deletion, anonymization or crypto-shred execution;
- a Customer Privacy worker;
- dependency upgrades, generic-runtime business switches or crate consolidation.

Approval records authorization to proceed with the immutable plan. It does not execute any plan item.

## PostgreSQL and process acceptance

Permanent `.github/workflows/customer-privacy-approval.yml` proved on clean PostgreSQL:

- migration application and FORCE RLS behavior;
- canonical fixture generation;
- real `crm-api` process routing;
- activation, authorization, tenant and expected-version negatives;
- eligible-state-only transition;
- exact replay and conflicting replay rejection;
- strict corruption and cross-tenant fail-closed behavior;
- no restriction, hold/retention, owner execution, destructive action or worker effects;
- complete rollback/schema removal;
- reapply and repeated SQL/Rust/process acceptance.

The complete exact-head matrix was 21 of 21 permanent workflows on accepted source `98000b0c1c2c15e14c7ee0cd2a366020040567e6`. Machine-readable accepted evidence is in `contracts/customer-privacy-approval-implementation.json`.

## Next bounded repository packet

Repository step 3 is bounded contribution aggregation without behavior change. Immediate deny-only processing restrictions using final subject locks remain repository step 4. Legal-hold/mandatory-retention precedence, owner execution, access/export, destructive actions, convergence and Customer Privacy workers remain separate later packets.
