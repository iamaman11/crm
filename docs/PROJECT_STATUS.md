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

**Phases 0.1–7 are complete. Phase 8A is active. Phase 8A.10 is Complete. Phase 8A.11 is In progress; `case.create`, `case.submit`, `case.subject.verify`, `case.cancel` and permission-aware `case.get` are merged through PR #150.**

Current Phase 8A baseline:

- **8A.1–8A.6 — Complete:** customer references, Party, Account, Contact Point, Party Relationship, Customer 360, Consent and reversible Identity Resolution.
- **8A.7 — Complete:** governed customer import and resumable execution (#120 / PR #121).
- **8A.8 — Complete:** governed customer export, artifacts and reconciliation (#123 / PR #130).
- **8A.9 — Complete:** Customer Data Quality Rules, Completeness and Stewardship (#124 / PR #132).
- **8A.10 — Complete:** Governed Customer Enrichment and Provenance (#125 / PR #137).
- **8A.11 — In progress:** architecture, owner foundation, deterministic domain, canonical persistence, immutable public contracts, FORCE RLS persistence, four public mutations and one permission-aware query are merged through PR #150.

The active dependency lane is:

`post-merge PR #150 governance sync -> one bounded case.list / case.approve / restriction slice -> remaining privacy lifecycle -> Phase 8A closure -> 8B`

## Phase 8A.10 accepted result

Phase 8A.10 was accepted on unchanged user-authored SHA `f92d101206886e3ceaf94d0e56e52580cec21093`, which passed all 17 permanent workflows, and squash-merged through PR #137 as `150e44b95d9dbdc08c1792563de03ec73f34aed1`.

The frozen production inventory is exactly:

- **6 public mutations**;
- **6 permission-aware queries**;
- **5 activation-gated worker-only coordinates** with no public HTTP/gRPC ingress.

All 17 manifest-bound Customer Enrichment coordinates are classified as public runtime or worker runtime. Provider dispatch/response run in phase 240, materialization in phase 245 and owner application/outcome recovery in phase 250.

## Phase 8A.11 merged foundation and accepted production coordinates

The following bounded PRs are merged:

- PR #140 — ownership and guardrail freeze;
- PR #141 — Customer Privacy owner foundation;
- PR #142 — deterministic pure-domain lifecycles;
- PR #143 — canonical private persistence;
- PR #144 — immutable public Protobuf contracts;
- PR #145 — FORCE RLS persistence proof;
- PR #146 — `customer_privacy.case.create@1.0.0`;
- PR #147 — `customer_privacy.case.submit@1.0.0`;
- PR #148 — `customer_privacy.case.subject.verify@1.0.0`;
- PR #149 — `customer_privacy.case.get@1.0.0`;
- PR #150 — `customer_privacy.case.cancel@1.0.0`.

PR #145 was accepted on source SHA `f37d9a5e025745abaaf0aeb351ff9bb534455aab` and merged as `721a1cf185ffbdea309bd1199c6c4568cf82d7a1`. Its applicable workflows proved clean migrations, FORCE RLS under `NOSUPERUSER + NOBYPASSRLS`, tenant isolation, missing-context concealment, `row_security=off` resistance, full rollback, schema removal, reapply and repeated FORCE RLS proof.

PR #146 was accepted on unchanged source SHA `9b53c3ebd81b58518dc445b02b33b35403ffa7c3`, passed all 18 applicable workflows and merged as `2d28937a123e4ba31ab0d835c4c30e3dfed0f187`. It provides deterministic tenant/idempotency case identity, confidential Draft/version-1 state, optional terminal predecessor lineage, exact replay/conflict behavior, generic HTTP/gRPC ingress, live authorization, activation gating and permanent fresh-PostgreSQL plus real-process evidence.

PR #147 was accepted on unchanged source SHA `8b41e8420b1a897777596c68cb615e2b8bf80c34`, passed all 18 permanent workflows and merged as `0eba56084405301eb667f2173b3aef6565b95f87`. It provides exact optimistic `Draft -> Submitted`, strict confidential rehydration, replay/conflict and malformed-state rollback, generic ingress, activation/live authorization, FORCE RLS and clean/reapplied real-process acceptance.

PR #148 was accepted on unchanged source SHA `118327e09a6e31ba87b02bdab99289035b572ed9`, passed all 18 permanent workflows and merged as `8ee5538bf97031dd48ab3726a605b9f3ad4bfd1e`. It provides authoritative Party existence/visibility, canonical redirect and active merge lineage, monotonic Identity Resolution topology generation, shared topology and canonical-subject locks, atomic `Submitted v2 -> SubjectVerified v3`, exact replay/conflict/concealment and permanent HTTP/gRPC process acceptance.

PR #149 was accepted on unchanged post-sync source SHA `5a47318b24007cd534434ff6bac33fbd59215d38`, passed all 18 permanent workflows and merged as `5d580a7c253bcfa6c2dd981100612b222fd26825`. It provides strict FORCE-RLS case lookup, canonical aggregate rehydration, live case and canonical Party visibility, field redaction, uniform concealment and side-effect-free real HTTP/gRPC query acceptance.

PR #150 was accepted on unchanged post-sync source SHA `be05e874b21ab33cb8b6a84fbcefc3c025aa88cb`, passed all 18 permanent workflows and was squash-merged as `2a4c34727e9d7bf8ed51b6411b7ab9c76c109671`. It provides race-free terminal cancellation with shared sorted/deduplicated subject locks before a retained final case-row `FOR UPDATE`, direct row serialization for unbound cases, immutable lineage preservation, exact replay/conflict, lock-contention rollback/retry, generic ingress, live authorization/activation and permanent clean/reapplied FORCE-RLS process acceptance.

## Current Customer Privacy production inventory

The merged inventory is exactly:

- **4 public mutations:** create, submit, subject verification and cancel;
- **1 permission-aware query:** case get;
- **11 public non-runtime coordinates**;
- **0 Customer Privacy worker runtime routes**;
- crypto-shred remains reasoned non-runtime and is not introduced as a public, worker or platform route.

No approval, remaining query, restriction, legal-hold, worker or owner-execution coordinate was promoted by PR #150.

## Remaining Phase 8A.11 boundary

Phase 8A.11 is not complete. Remaining work includes approval, live restriction and legal-hold precedence, the remaining permission-aware query surfaces, bounded owner contribution/orchestration, privacy export, deletion/anonymization convergence, immutable-evidence preservation, worker recovery and complete lifecycle acceptance.

## Merged platform and customer-master baseline

Merged `main` contains executable architecture governance, typed module/runtime foundations, PostgreSQL tenant/RLS/records/idempotency/outbox/audit, authenticated mutation and permission-aware query gateways, native module-owned exact-coordinate composition, durable workers/projections/search, and production slices for Party, Account, Contact Point, Party Relationship, Customer 360, Consent, reversible Identity Resolution, import, export, Data Quality, Customer Enrichment and five accepted Customer Privacy coordinates.

## Product completeness reality

The project is **not yet a complete universal CRM**. Major required families still include the remaining Phase 8A.11 privacy lifecycle, Product Catalog/Pricing/CPQ/Quotes/Orders/Contracts/Subscriptions, broader Sales and Activities, omnichannel, Service, Marketing, Customer Success, projects, documents/e-signature, analytics, workflow/collaboration, AI governance, marketplace and enterprise operational proof.

## Immediate next actions

1. Merge this post-PR #150 documentation synchronization after its own applicable exact-head gates.
2. Compare only `customer_privacy.case.list@1.0.0`, `customer_privacy.case.approve@1.0.0` and restriction placement.
3. Select one next bounded Customer Privacy coordinate without combining approval, queries, restrictions, legal holds or workers.
4. Keep all remaining privacy coordinates non-runtime until their own production proofs are complete.
5. Close Phase 8A only after the full privacy/customer-master interaction baseline is merged and reconciled; begin Phase 8B / #29 only afterward.
