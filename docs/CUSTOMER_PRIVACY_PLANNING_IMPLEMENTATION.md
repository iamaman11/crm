# Customer Privacy deterministic planning implementation

Status: **Accepted and merged through PR #209.** Accepted source `b97fd9bb4537c14df4497ad7b737d0f0a64c4f3b` passed **29 of 29 permanent workflows** unchanged and was squash-merged as `30621ffff5c1e07e1275cc80fee3f1297a91f49e`.

Historical source of truth remains `contracts/customer-privacy-planning-freeze.json` and `CUSTOMER_PRIVACY_PLANNING_FREEZE.md`. This document records the later runtime implementation and acceptance without rewriting the historical `runtime_not_started` evidence accepted in PR #208.

## Implemented coordinate

`customer_privacy.plan.build@1.0.0` is implemented as a trusted-internal, activation-gated phase-270 service inside the accepted Customer Privacy packages. It has no public HTTP/gRPC ingress and is not registered as a generic-runtime worker.

The service accepts only an explicit trusted invocation bound to tenant, privacy case, actor, request/correlation/trace identity, request time, proposed planning time and immutable planning policy.

## Exact source validation

Planning loads and verifies:

- one tenant-bound canonical privacy case;
- status `Scoped` for a new plan, or `Planned`/`AwaitingApproval` only for exact replay;
- exact optimistic case version;
- verified canonical Party and Identity Resolution generation;
- the exact case-referenced immutable discovery snapshot;
- snapshot identity, binding digest, completeness digest, registry digest, purpose and effective request time;
- exact case kind, policy version, jurisdiction, approval requirement and explicit crypto-shred support.

Any case, snapshot, topology, registry, policy or replay mismatch fails closed. The service never silently rediscovers or rebases an in-flight case.

## Deterministic planning

The implementation consumes the immutable PR #208 domain contract. It preserves:

- canonical owner/resource/data-class ordering;
- contiguous item sequence beginning at one;
- exact lineage, item and plan digests;
- one deterministic `privacy-action-plan-<sha256>` identity;
- an empty immutable plan for an empty complete scope;
- no inferred `NoOpAlreadyCompliant` without later exact owner evidence;
- unsupported crypto-shred failure without a destructive fallback.

The exact mapping remains:

- Access and Portability Export → `Retain`;
- Restrict Processing → `RestrictOnly`;
- Erasure destroyable data → `Delete`;
- Erasure retain-minimized evidence → `Anonymize`;
- Erasure immutable required evidence → `Retain`;
- Erasure derived rebuildable state → `Delete`;
- Erasure crypto-shreddable data → `CryptoShred` only with explicit accepted support.

Planning classifies intended owner actions only. It performs no owner mutation, restriction placement, hold or retention adjudication, access/export request, deletion, anonymization or crypto-shred execution.

## Atomic persistence

One PostgreSQL transaction:

1. binds tenant, actor, request and exact planning coordinate;
2. locks and strictly rehydrates the privacy case;
3. reloads the immutable snapshot and rebuilds the expected plan;
4. inserts the immutable plan as `customer-privacy.action-plan` in `crm.records`;
5. strictly rehydrates and compares the inserted plan;
6. transitions the case `Scoped → Planned` or `Scoped → AwaitingApproval` with optimistic version increment;
7. inserts exact case/snapshot/plan replay evidence;
8. appends a safe planning audit event;
9. commits all evidence together.

A concurrent or repeated invocation returns the same accepted plan only when case state, policy, proposed planning time and durable evidence match exactly. Conflicting replay fails closed.

## PostgreSQL controls

Migration `0102_customer_privacy_planning` adds:

- `crm.customer_privacy_action_plans`;
- `crm.customer_privacy_planning_audit`.

Both tables use ENABLE + FORCE RLS and the canonical `tenant_isolation` policy with `crm.current_tenant_id()` in both `USING` and `WITH CHECK`.

The case/plan link, audit rows and `customer-privacy.action-plan` records are append-only. Planning times must be microsecond aligned; the application validates that constraint and PostgreSQL `timestamptz` round-trips the canonical nanosecond integer exactly.

Rollback removes both tables, their record trigger and the planning immutability function. Reapply and repeated acceptance are mandatory.

## Production composition and non-effects

The existing `crm-customer-privacy-production` owner entry point exposes internal discovery, snapshot reading and planning services together. Generic application runtime code does not import a concrete planner or add a Customer Privacy branch.

Accepted public inventory remains unchanged:

- four public mutations;
- two permission-aware public queries;
- zero Customer Privacy workers;
- no public `plan.build` route;
- 113 workspace packages;
- no new dependency family.

`customer_privacy.case.plan.get@1.0.0` and `customer_privacy.case.owner_outcomes.list@1.0.0` remain published but non-runtime. Their permission-aware runtime promotion is the next bounded packet.

## Accepted evidence

The immutable implementation evidence is recorded in `contracts/customer-privacy-planning-implementation.json`:

- pull request: `209`;
- accepted source: `b97fd9bb4537c14df4497ad7b737d0f0a64c4f3b`;
- permanent workflow matrix: `29 of 29` successful on the unchanged source;
- squash merge: `30621ffff5c1e07e1275cc80fee3f1297a91f49e`.

Acceptance included clean PostgreSQL, strict package tests, FORCE RLS and cross-tenant negatives, immutable evidence, full rollback/schema removal, reapply, repeated acceptance, unchanged 4/2/0 public inventory, unchanged package count and all applicable workflows successful on one unchanged exact source SHA.

## Next bounded packet

Promote only the existing published reads:

- `customer_privacy.case.plan.get@1.0.0` as a permission-aware, tenant-bound, payload-safe plan summary with strict case↔plan↔snapshot lineage validation and audited concealment;
- `customer_privacy.case.owner_outcomes.list@1.0.0` as a bounded deterministic empty terminal page until owner execution and outcome persistence exist.

Approval, restrictions, legal-hold/retention adjudication, owner execution, destructive actions and Customer Privacy workers remain outside that packet.
