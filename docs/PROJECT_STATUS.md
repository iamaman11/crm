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

**Phases 0.1–7 are complete. Phase 8A is active. Phase 8A.10 is Complete. Phase 8A.11 is In progress; `case.create` is merged and draft PR #147 is in Gate review for `case.submit`.**

Current Phase 8A baseline:

- **8A.1–8A.6 — Complete:** customer references, Party, Account, Contact Point, Party Relationship, Customer 360, Consent and reversible Identity Resolution.
- **8A.7 — Complete:** governed customer import and resumable execution (#120 / PR #121).
- **8A.8 — Complete:** governed customer export, artifacts and reconciliation (#123 / PR #130).
- **8A.9 — Complete:** Customer Data Quality Rules, Completeness and Stewardship (#124 / PR #132).
- **8A.10 — Complete:** Governed Customer Enrichment and Provenance (#125 / PR #137).
- **8A.11 — In progress:** architecture, owner foundation, deterministic domain, canonical persistence, immutable public contracts, FORCE RLS persistence and `customer_privacy.case.create@1.0.0` are merged through PR #146. Draft PR #147 promotes only `customer_privacy.case.submit@1.0.0`.

The active dependency lane is:

`8A.11 case.submit gate review -> subject verification and shared subject lock -> separately bounded remaining privacy slices -> Phase 8A closure -> 8B`

## Phase 8A.10 accepted result

Phase 8A.10 was accepted on unchanged user-authored SHA `f92d101206886e3ceaf94d0e56e52580cec21093`, which passed all 17 permanent workflows, and squash-merged through PR #137 as `150e44b95d9dbdc08c1792563de03ec73f34aed1`.

The frozen production inventory is exactly:

- **6 public mutations**;
- **6 permission-aware queries**;
- **5 activation-gated worker-only coordinates** with no public HTTP/gRPC ingress.

All 17 manifest-bound Customer Enrichment coordinates are classified as public runtime or worker runtime. Provider dispatch/response run in phase 240, materialization in phase 245 and owner application/outcome recovery in phase 250.

## Phase 8A.11 merged foundation and first mutation

The following bounded PRs are merged:

- PR #140 — ownership and guardrail freeze;
- PR #141 — Customer Privacy owner foundation;
- PR #142 — deterministic pure-domain lifecycles;
- PR #143 — canonical private persistence;
- PR #144 — immutable public Protobuf contracts;
- PR #145 — FORCE RLS persistence proof;
- PR #146 — first production mutation, `customer_privacy.case.create@1.0.0`.

PR #145 was accepted on source SHA `f37d9a5e025745abaaf0aeb351ff9bb534455aab` and merged as `721a1cf185ffbdea309bd1199c6c4568cf82d7a1`. Its 17 applicable workflows proved clean migrations, FORCE RLS under `NOSUPERUSER + NOBYPASSRLS`, tenant isolation, missing-context concealment, `row_security=off` resistance, full rollback, schema removal, reapply and repeated FORCE RLS proof.

PR #146 was accepted on unchanged source SHA `9b53c3ebd81b58518dc445b02b33b35403ffa7c3`, passed all 18 applicable workflows and merged as `2d28937a123e4ba31ab0d835c4c30e3dfed0f187`. It provides deterministic tenant/idempotency case identity, confidential Draft/version-1 state, optional terminal predecessor lineage, exact replay/conflict behavior, generic HTTP/gRPC ingress, live authorization, activation gating and permanent fresh-PostgreSQL plus real-process evidence.

## Active packet: Customer Privacy case submission

Draft PR #147 is deliberately bounded to:

`customer_privacy.case.submit@1.0.0`

The candidate implementation provides:

- a dedicated infrastructure-neutral submit planner;
- exact request/response owner, capability, version and Protobuf-contract validation;
- one tenant-bound `customer-privacy.case` target with `AggregatePresence::MustExist`;
- strict canonical confidential state rehydration through the accepted persistence adapter;
- exact optimistic `Draft -> Submitted` transition using the public expected version;
- one record update, one immutable `customer_privacy.case.status_changed` event, one audit intent and one idempotency claim in one transaction;
- production registration only through generic application mutation ingress, shared live authorization and activation gating;
- permanent candidate unit, fresh-PostgreSQL, rollback/reapply and real-`crm-api` process evidence;
- route parity of exactly two runtime privacy mutations and fourteen non-runtime public privacy coordinates.

No subject verification, approval, cancellation, privacy query, restriction, legal-hold, worker or crypto-shred coordinate is promoted by this slice.

## Remaining Phase 8A.11 boundary

Phase 8A.11 is not complete. Remaining work includes verified subject binding, canonical Party lineage and identity-resolution generation, the shared canonical-Party subject lock, live restriction and legal-hold precedence, permission-aware query visibility, bounded owner contribution/orchestration, privacy export, deletion/anonymization convergence, immutable-evidence preservation, worker recovery and complete real-process acceptance.

## Merged platform and customer-master baseline

Merged `main` contains executable architecture governance, typed module/runtime foundations, PostgreSQL tenant/RLS/records/idempotency/outbox/audit, authenticated mutation and permission-aware query gateways, native module-owned exact-coordinate composition, durable workers/projections/search, and production slices for Party, Account, Contact Point, Party Relationship, Customer 360, Consent, reversible Identity Resolution, import, export, Data Quality, Customer Enrichment and Customer Privacy case creation.

## Product completeness reality

The project is **not yet a complete universal CRM**. Major required families still include the remaining Phase 8A.11 privacy lifecycle, Product Catalog/Pricing/CPQ/Quotes/Orders/Contracts/Subscriptions, broader Sales and Activities, omnichannel, Service, Marketing, Customer Success, projects, documents/e-signature, analytics, workflow/collaboration, AI governance, marketplace and enterprise operational proof.

## Immediate next actions

1. Accept draft PR #147 only after all applicable workflows pass on one unchanged exact source SHA and review threads are resolved.
2. Merge `case.submit` with expected unchanged head and record source/merge SHAs in PR #147 and issue #126.
3. Implement `customer_privacy.case.subject.verify@1.0.0` separately with canonical Party lineage, identity-resolution generation validation and shared subject locking.
4. Keep all remaining privacy coordinates non-runtime until their own production proofs are complete.
5. Close Phase 8A only after the full privacy/customer-master interaction baseline is merged and reconciled; begin Phase 8B / #29 only afterward.
