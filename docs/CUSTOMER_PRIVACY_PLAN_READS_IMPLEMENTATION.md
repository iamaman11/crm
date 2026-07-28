# Customer Privacy permission-aware plan reads

Status: **Implemented in PR #211 and pending unchanged exact-head acceptance.**

Historical sources remain immutable:

- `contracts/customer-privacy-planning-freeze.json` / PR #208 freeze the planning and read semantics before runtime work;
- `contracts/customer-privacy-planning-implementation.json` / PR #209 record accepted trusted-internal planning runtime;
- this packet promotes only the two already published read coordinates.

## Runtime coordinates

- `customer_privacy.case.plan.get@1.0.0`;
- `customer_privacy.case.owner_outcomes.list@1.0.0`.

Both are public permission-aware queries. The packet adds no mutation, worker, crate, generic-runtime Customer Privacy switch, approval, restriction, hold/retention decision, owner mutation or destructive execution.

## Application boundary

The reads are implemented in the existing `crm-customer-privacy-application` package and composed through the existing PostgreSQL and production packages.

Each request requires:

- exact capability/version and Protobuf contract;
- active `crm.customer-privacy` module for the tenant;
- live case, canonical Party and action-plan visibility decisions;
- tenant-bound source reads;
- safe allow/deny audit evidence.

Unauthorized and cross-tenant existence is concealed as the same not-found response.

## `case.plan.get`

The implementation loads and strictly rehydrates:

1. the tenant-bound privacy case;
2. the exact immutable discovery scope snapshot referenced by the case;
3. the exact immutable action plan referenced by the case;
4. the durable case/snapshot/plan replay link.

It verifies case status and version lineage, case↔snapshot↔plan identity, canonical Party and Identity Resolution generation, case kind, policy version, registry/purpose/effective-time lineage, snapshot binding/completeness digests, plan digest, approval flag and planning time.

Missing, corrupt or conflicting evidence fails closed. The response is a payload-safe plan summary containing only plan/case references, finalized status, policy version, version and finalization time. It never exposes owner resource payloads or immutable plan items.

## `case.owner_outcomes.list`

Owner execution and outcome persistence do not exist yet. The coordinate therefore implements the frozen future-safe boundary only:

- page size defaults to 64 and is bounded at 128;
- cursor input is bounded at 2048 bytes;
- the only accepted cursor is the empty initial cursor;
- `items = []`;
- `next_cursor = ""` as terminal semantics;
- deterministic page and terminal digests are written to safe read-audit evidence;
- no synthetic outcome is returned;
- no outcome table or record is created.

The owner-module filter participates in authorization/audit identity and deterministic page digest but does not create fictional owner results.

## PostgreSQL evidence

Migration `0103_customer_privacy_plan_reads` adds only `crm.customer_privacy_plan_read_audit`.

The table stores tenant/case/plan identifiers, digests, bounded page metadata, safe authorization result codes, actor/request/correlation/trace identity and request time. It stores no owner resource payload.

Controls:

- ENABLE + FORCE RLS;
- canonical `tenant_isolation` policy using `crm.current_tenant_id()` in `USING` and `WITH CHECK`;
- append-only update/delete rejection;
- cross-tenant read and insert denial;
- complete rollback and reapply.

No `customer_privacy_owner_outcomes` persistence exists in this packet.

## Production inventory

After this packet the intended Customer Privacy inventory is:

- four public mutations;
- four permission-aware public queries;
- zero Customer Privacy workers;
- 113 workspace packages;
- zero new dependency families.

Trusted-internal `customer_privacy.plan.build@1.0.0` remains non-public. Scope discovery remains trusted-internal. The generic query router and worker algorithms remain unchanged.

## Acceptance

Permanent `.github/workflows/customer-privacy-planning.yml` proves:

- clean PostgreSQL application;
- `database/tests/0045_customer_privacy_plan_reads.sql`;
- strict application/PostgreSQL/production package tests;
- no public `plan.build` route;
- no Customer Privacy worker;
- no owner-outcome persistence;
- unchanged 113-package workspace;
- complete schema rollback/removal;
- reapply and repeated SQL/Rust acceptance.

Machine-readable pending evidence is in `contracts/customer-privacy-plan-reads-implementation.json`. Accepted source SHA, permanent workflow count and merge SHA remain null until every applicable permanent workflow succeeds on one unchanged source-authored or connector-authored exact head; bot-authored synchronization commits are not accepted as final evidence.

## Next bounded packet

Approval runtime is next. Processing restrictions, legal-hold/mandatory-retention precedence, owner execution, access/export assembly, deletion/anonymization/crypto-shred, Party tombstone, convergence and Customer Privacy workers remain separate later packets.
