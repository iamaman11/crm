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

**Phases 0.1–7 are complete. Phase 8A is active. Phase 8A.10 is Complete. Phase 8A.11 is In progress; `case.create`, `case.submit` and `case.subject.verify` are merged, and draft PR #149 is in Gate review for `case.get`.**

Current Phase 8A baseline:

- **8A.1–8A.6 — Complete:** customer references, Party, Account, Contact Point, Party Relationship, Customer 360, Consent and reversible Identity Resolution.
- **8A.7 — Complete:** governed customer import and resumable execution (#120 / PR #121).
- **8A.8 — Complete:** governed customer export, artifacts and reconciliation (#123 / PR #130).
- **8A.9 — Complete:** Customer Data Quality Rules, Completeness and Stewardship (#124 / PR #132).
- **8A.10 — Complete:** Governed Customer Enrichment and Provenance (#125 / PR #137).
- **8A.11 — In progress:** architecture, owner foundation, deterministic domain, canonical persistence, immutable public contracts, FORCE RLS persistence and three public mutations are merged through PR #148. Draft PR #149 promotes only permission-aware `customer_privacy.case.get@1.0.0`.

The active dependency lane is:

`8A.11 case.get gate review -> separately bounded approval/cancellation/remaining queries/restriction/legal-hold/worker slices -> Phase 8A closure -> 8B`

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
- PR #148 — `customer_privacy.case.subject.verify@1.0.0`.

PR #145 was accepted on source SHA `f37d9a5e025745abaaf0aeb351ff9bb534455aab` and merged as `721a1cf185ffbdea309bd1199c6c4568cf82d7a1`. Its applicable workflows proved clean migrations, FORCE RLS under `NOSUPERUSER + NOBYPASSRLS`, tenant isolation, missing-context concealment, `row_security=off` resistance, full rollback, schema removal, reapply and repeated FORCE RLS proof.

PR #146 was accepted on unchanged source SHA `9b53c3ebd81b58518dc445b02b33b35403ffa7c3`, passed all 18 applicable workflows and merged as `2d28937a123e4ba31ab0d835c4c30e3dfed0f187`. It provides deterministic tenant/idempotency case identity, confidential Draft/version-1 state, optional terminal predecessor lineage, exact replay/conflict behavior, generic HTTP/gRPC ingress, live authorization, activation gating and permanent fresh-PostgreSQL plus real-process evidence.

PR #147 was accepted on unchanged source SHA `8b41e8420b1a897777596c68cb615e2b8bf80c34`, passed all 18 permanent workflows and merged as `0eba56084405301eb667f2173b3aef6565b95f87`. It provides exact optimistic `Draft -> Submitted`, strict confidential rehydration, replay/conflict and malformed-state rollback, generic ingress, activation/live authorization, FORCE RLS and clean/reapplied real-process acceptance.

PR #148 was accepted on unchanged source SHA `118327e09a6e31ba87b02bdab99289035b572ed9`, passed all 18 permanent workflows and merged as `8ee5538bf97031dd48ab3726a605b9f3ad4bfd1e`. It provides authoritative Party existence/visibility, canonical redirect and active merge lineage, monotonic Identity Resolution topology generation, shared topology and canonical-subject locks, atomic `Submitted v2 -> SubjectVerified v3`, exact replay/conflict/concealment and permanent HTTP/gRPC process acceptance.

## Active packet: permission-aware Customer Privacy case read

Draft PR #149 is deliberately bounded to:

`customer_privacy.case.get@1.0.0`

The candidate implementation provides:

- exact query owner, capability/version and confidential Protobuf request/response validation;
- tenant-scoped authoritative read through the accepted non-privileged FORCE-RLS record adapter;
- strict private-state envelope validation and aggregate rehydration;
- live privacy-case resource visibility and canonical Party visibility after subject verification;
- uniform not-found concealment for missing, cross-tenant and hidden resources;
- field-level redaction through the shared query visibility policy and deployment ceiling;
- generic production HTTP/gRPC query ingress with live query authorization and activation gating;
- no audit, idempotency, outbox, event or business-transaction writes and no aggregate version change;
- permanent unit and real-process acceptance for success, redaction, token scope, concealment, suspension, absent grant and safe bounded errors;
- route parity of exactly three runtime Customer Privacy mutations, one runtime query and twelve non-runtime public coordinates.

No approval, cancellation, remaining privacy query, restriction, legal-hold, worker, owner-execution or crypto-shred coordinate is promoted by this slice.

## Remaining Phase 8A.11 boundary

Phase 8A.11 is not complete. Remaining work includes approval/cancellation, live restriction and legal-hold precedence, the remaining permission-aware query surfaces, bounded owner contribution/orchestration, privacy export, deletion/anonymization convergence, immutable-evidence preservation, worker recovery and complete lifecycle acceptance.

## Merged platform and customer-master baseline

Merged `main` contains executable architecture governance, typed module/runtime foundations, PostgreSQL tenant/RLS/records/idempotency/outbox/audit, authenticated mutation and permission-aware query gateways, native module-owned exact-coordinate composition, durable workers/projections/search, and production slices for Party, Account, Contact Point, Party Relationship, Customer 360, Consent, reversible Identity Resolution, import, export, Data Quality, Customer Enrichment and the first three Customer Privacy mutations.

## Product completeness reality

The project is **not yet a complete universal CRM**. Major required families still include the remaining Phase 8A.11 privacy lifecycle, Product Catalog/Pricing/CPQ/Quotes/Orders/Contracts/Subscriptions, broader Sales and Activities, omnichannel, Service, Marketing, Customer Success, projects, documents/e-signature, analytics, workflow/collaboration, AI governance, marketplace and enterprise operational proof.

## Immediate next actions

1. Accept draft PR #149 only after all 18 applicable workflows pass on one unchanged post-Generated-Sync source SHA and review threads are resolved.
2. Merge `case.get` with the expected unchanged head and record source/merge SHAs in PR #149 and issue #126.
3. Select the next bounded Customer Privacy coordinate without combining approval, cancellation, remaining queries, restrictions, legal holds or workers in one slice.
4. Keep all remaining privacy coordinates non-runtime until their own production proofs are complete.
5. Close Phase 8A only after the full privacy/customer-master interaction baseline is merged and reconciled; begin Phase 8B / #29 only afterward.
