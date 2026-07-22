# Ultimate CRM — Project Status

Status date: 2026-07-22

This is the concise human-readable status page. Normative delivery order remains in `IMPLEMENTATION_ROADMAP.md` and `PHASE8_DELIVERY_PLAN.md`.

Authoritative references:

1. `SYSTEM_INVARIANTS.md` — absolute architecture rules.
2. `ARCHITECTURE_READINESS.md` — accepted native-composition baseline.
3. `DELIVERY_GOVERNANCE.md` — packet-state and synchronization policy.
4. `IMPLEMENTATION_ROADMAP.md` — normative phase sequence.
5. `PHASE8_DELIVERY_PLAN.md` — detailed Phase 8 packet sequence.
6. `CRM_CAPABILITY_COVERAGE.md` — functional completeness guardrail.
7. `MODULE_CATALOG.md` — merged business-module readiness accounting.

## Current position

**Phases 0.1–7 are complete. Phase 8A is active. Phase 8A.10 is Complete. Phase 8A.11 is In progress and draft PR #146 is in Gate review for its first production mutation.**

Current Phase 8A baseline:

- **8A.1–8A.6 — Complete:** customer references, Party, Account, Contact Point, Party Relationship, Customer 360, Consent and reversible Identity Resolution.
- **8A.7 — Complete:** governed customer import and resumable execution (#120 / PR #121).
- **8A.8 — Complete:** governed customer export, artifacts and reconciliation (#123 / PR #130).
- **8A.9 — Complete:** Customer Data Quality Rules, Completeness and Stewardship (#124 / PR #132).
- **8A.10 — Complete:** Governed Customer Enrichment and Provenance (#125 / PR #137).
- **8A.11 — In progress:** architecture, owner foundation, deterministic domain, canonical persistence, immutable public contracts and FORCE RLS persistence are merged through PR #145. Draft PR #146 promotes only `customer_privacy.case.create@1.0.0`.

The active dependency lane is:

`8A.11 case.create gate review -> separately bounded remaining privacy slices -> Phase 8A closure -> 8B`

## Phase 8A.10 accepted result

Phase 8A.10 was accepted on unchanged user-authored SHA `f92d101206886e3ceaf94d0e56e52580cec21093`, which passed all 17 permanent workflows, and squash-merged through PR #137 as `150e44b95d9dbdc08c1792563de03ec73f34aed1`.

The frozen production inventory is exactly:

- **6 public mutations**;
- **6 permission-aware queries**;
- **5 activation-gated worker-only coordinates** with no public HTTP/gRPC ingress.

All 17 manifest-bound Customer Enrichment coordinates are classified as public runtime or worker runtime. Provider dispatch/response run in phase 240, materialization in phase 245 and owner application/outcome recovery in phase 250.

## Phase 8A.11 merged foundation

The following bounded PRs are merged:

- PR #140 — ownership and guardrail freeze;
- PR #141 — Customer Privacy owner foundation;
- PR #142 — deterministic pure-domain lifecycles;
- PR #143 — canonical private persistence;
- PR #144 — immutable public Protobuf contracts;
- PR #145 — FORCE RLS persistence proof.

PR #145 was accepted on source SHA `f37d9a5e025745abaaf0aeb351ff9bb534455aab` and merged as `721a1cf185ffbdea309bd1199c6c4568cf82d7a1`. Its 17 applicable workflows proved clean migrations, FORCE RLS under `NOSUPERUSER + NOBYPASSRLS`, tenant isolation, missing-context concealment, `row_security=off` resistance, full rollback, schema removal, reapply and repeated FORCE RLS proof. The strict persistence adapter validates record identity/version plus schema, descriptor, data class, encoding, byte ceiling and retention metadata.

## Active packet: first Customer Privacy production mutation

Draft PR #146 is deliberately bounded to:

`customer_privacy.case.create@1.0.0`

The candidate implementation provides:

- a dedicated infrastructure-neutral capability planner;
- deterministic length-framed SHA-256 case identity from tenant and idempotency key;
- canonical confidential Draft/version-1 state;
- one immutable `customer_privacy.case.created` event, one audit intent, one idempotency claim and one atomic batch;
- `MustBeAbsent` successor locking;
- optional predecessor lineage through a transaction-scoped PostgreSQL `FOR SHARE` guard, strict snapshot rehydration, tenant concealment and terminal-only validation;
- production registration only through the generic application mutation ingress, shared live authorizer and activation gate;
- permanent unit, fresh-PostgreSQL, rollback/reapply and real-`crm-api` acceptance;
- exact route parity: one runtime privacy mutation and fifteen non-runtime public privacy coordinates.

No privacy query, submit, subject verification, approval, cancellation, restriction, legal-hold, worker or crypto-shred coordinate is promoted by this slice.

## Remaining Phase 8A.11 boundary

Phase 8A.11 is not complete. Remaining work includes subject verification, the shared canonical-Party subject lock, live restriction and legal-hold precedence, permission-aware query visibility, bounded owner contribution/orchestration, privacy export, deletion/anonymization convergence, immutable-evidence preservation, worker recovery and complete real-process acceptance.

## Merged platform and customer-master baseline

Merged `main` contains executable architecture governance, typed module/runtime foundations, PostgreSQL tenant/RLS/records/idempotency/outbox/audit, authenticated mutation and permission-aware query gateways, native module-owned exact-coordinate composition, durable workers/projections/search, and production slices for Party, Account, Contact Point, Party Relationship, Customer 360, Consent, reversible Identity Resolution, import, export, Data Quality and Customer Enrichment.

## Product completeness reality

The project is **not yet a complete universal CRM**. Major required families still include the remaining Phase 8A.11 privacy lifecycle, Product Catalog/Pricing/CPQ/Quotes/Orders/Contracts/Subscriptions, broader Sales and Activities, omnichannel, Service, Marketing, Customer Success, projects, documents/e-signature, analytics, workflow/collaboration, AI governance, marketplace and enterprise operational proof.

## Immediate next actions

1. Accept draft PR #146 only after all applicable workflows pass on one unchanged exact source SHA and review threads are resolved.
2. Merge `case.create` with expected unchanged head and record source/merge SHAs in PR #146 and issue #126.
3. Select the next bounded Customer Privacy slice separately; do not batch submit, subject verification, restriction or legal hold into `case.create`.
4. Close Phase 8A only after the full privacy/customer-master interaction baseline is merged and reconciled.
5. Begin Phase 8B / #29 only after Phase 8A closure.
