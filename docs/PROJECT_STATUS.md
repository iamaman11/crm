# Ultimate CRM — Project Status

Status date: 2026-07-25

This is the concise human-readable status page. Normative delivery order remains in `IMPLEMENTATION_ROADMAP.md` and `PHASE8_DELIVERY_PLAN.md`.

Authoritative references:

1. `SYSTEM_INVARIANTS.md` — absolute architecture rules.
2. `ARCHITECTURE_READINESS.md` — accepted native-composition baseline.
3. `ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md` — code/package/dependency scalability plan.
4. `ARCHITECTURE_CI_SCALABILITY_AND_DEVICE_LAB_PLAN.md` — CI, PostgreSQL isolation and real-device execution plan.
5. `DELIVERY_GOVERNANCE.md` — packet-state and synchronization policy.
6. `IMPLEMENTATION_ROADMAP.md` — normative phase sequence.
7. `PHASE8_DELIVERY_PLAN.md` — detailed Phase 8 packet sequence.
8. `CRM_CAPABILITY_COVERAGE.md` — functional completeness guardrail.
9. `MODULE_CATALOG.md` — merged business-module readiness accounting.

## Current position

**Phases 0.1–7 are complete. Phase 8A is active. Phase 8A.10 is Complete. Phase 8A.11 is In progress; six Customer Privacy runtime coordinates, the immutable owner-scope protocol foundation, nine contract-only owner contribution coordinates and the first authoritative non-runtime Parties owner implementation are merged through PR #156.**

Current Phase 8A baseline:

- **8A.1–8A.6 — Complete:** customer references, Party, Account, Contact Point, Party Relationship, Customer 360, Consent and reversible Identity Resolution.
- **8A.7 — Complete:** governed customer import and resumable execution (#120 / PR #121).
- **8A.8 — Complete:** governed customer export, artifacts and reconciliation (#123 / PR #130).
- **8A.9 — Complete:** Customer Data Quality Rules, Completeness and Stewardship (#124 / PR #132).
- **8A.10 — Complete:** Governed Customer Enrichment and Provenance (#125 / PR #137).
- **8A.11 — In progress:** architecture, owner foundation, deterministic domain, canonical persistence, immutable public contracts, FORCE RLS persistence, four public mutations, two permission-aware queries, immutable owner-scope envelopes, nine owner-specific contract-only contribution coordinates and the first proven owner implementation are merged.

The active dependency lane is:

`Immediate CI event/concurrency/action-pinning and status controls -> complexity and CI critical-path baseline -> golden package/contribution aggregation/affected CI -> second contrasting owner -> shared privacy contribution protocol -> remaining privacy lifecycle -> Phase 8A closure -> 8B`

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
- PR #150 — `customer_privacy.case.cancel@1.0.0`;
- PR #152 — `customer_privacy.case.list@1.0.0`;
- PR #153 — post-merge `case.list` governance synchronization;
- PR #154 — immutable privacy owner-scope protocol foundation;
- PR #155 — nine owner-specific privacy scope contribution contracts, all contract-only non-runtime;
- PR #156 — first authoritative non-runtime Parties privacy scope owner implementation.

PR #145 was accepted on source SHA `f37d9a5e025745abaaf0aeb351ff9bb534455aab` and merged as `721a1cf185ffbdea309bd1199c6c4568cf82d7a1`. Its applicable workflows proved clean migrations, FORCE RLS under `NOSUPERUSER + NOBYPASSRLS`, tenant isolation, missing-context concealment, `row_security=off` resistance, full rollback, schema removal, reapply and repeated FORCE RLS proof.

PR #146 was accepted on unchanged source SHA `9b53c3ebd81b58518dc445b02b33b35403ffa7c3`, passed all 18 applicable workflows and merged as `2d28937a123e4ba31ab0d835c4c30e3dfed0f187`. It provides deterministic tenant/idempotency case identity, confidential Draft/version-1 state, optional terminal predecessor lineage, exact replay/conflict behavior, generic HTTP/gRPC ingress, live authorization, activation gating and permanent fresh-PostgreSQL plus real-process evidence.

PR #147 was accepted on unchanged source SHA `8b41e8420b1a897777596c68cb615e2b8bf80c34`, passed all 18 permanent workflows and merged as `0eba56084405301eb667f2173b3aef6565b95f87`. It provides exact optimistic `Draft -> Submitted`, strict confidential rehydration, replay/conflict and malformed-state rollback, generic ingress, activation/live authorization, FORCE RLS and clean/reapplied real-process acceptance.

PR #148 was accepted on unchanged source SHA `118327e09a6e31ba87b02bdab99289035b572ed9`, passed all 18 permanent workflows and merged as `8ee5538bf97031dd48ab3726a605b9f3ad4bfd1e`. It provides authoritative Party existence/visibility, canonical redirect and active merge lineage, monotonic Identity Resolution topology generation, shared topology and canonical-subject locks, atomic `Submitted v2 -> SubjectVerified v3`, exact replay/conflict/concealment and permanent HTTP/gRPC process acceptance.

PR #149 was accepted on unchanged post-sync source SHA `5a47318b24007cd534434ff6bac33fbd59215d38`, passed all 18 permanent workflows and merged as `5d580a7c253bcfa6c2dd981100612b222fd26825`. It provides strict FORCE-RLS case lookup, canonical aggregate rehydration, live case and canonical Party visibility, field redaction, uniform concealment and side-effect-free real HTTP/gRPC query acceptance.

PR #150 was accepted on unchanged post-sync source SHA `be05e874b21ab33cb8b6a84fbcefc3c025aa88cb`, passed all 18 permanent workflows and was squash-merged as `2a4c34727e9d7bf8ed51b6411b7ab9c76c109671`. It provides race-free terminal cancellation with shared sorted/deduplicated subject locks before a retained final case-row `FOR UPDATE`, direct row serialization for unbound cases, immutable lineage preservation, exact replay/conflict, lock-contention rollback/retry, generic ingress, live authorization/activation and permanent clean/reapplied FORCE-RLS process acceptance.

PR #152 was accepted on unchanged source SHA `9de6048f951c0797a94871457d2bdd73357aee59`, passed all 18 permanent workflows and was squash-merged as `26f5b4644c935001806343b2feaf802a78c90eae`. It provides canonical-Party-scoped listing, signed filter-bound pagination, bounded FORCE-RLS keyset scanning, strict verified-subject matching, live Party/case visibility, field redaction, uniform empty concealment and side-effect-free real HTTP/gRPC acceptance on an isolated fresh database.

PR #155 was accepted on unchanged source SHA `7574297ed9c7a28b0e5612052898d43a6e156dcc`, passed 15 applicable workflows and was squash-merged as `624f7f3dd5099384cd7304db6bab78bc4cfc9c51`. It publishes one immutable reference-only owner contribution coordinate for each of Parties, Customer Accounts, Contact Points, Party Relationships, Consents, Identity Resolution, Customer Data Operations, Data Quality and Customer Enrichment. No owner implementation, worker registration, public ingress or runtime promotion was introduced.

PR #156 was accepted on unchanged source SHA `753acdb2ad2c25b343d0aae3413bb8b5c38581e2`, passed all 18 applicable workflows and was squash-merged as `4368b8c3710e05137b71ba999bf7f3497c0801c8`. It implements the Parties owner contribution in one tenant-bound `REPEATABLE READ, READ ONLY` transaction with transaction-scoped RLS, the shared topology advisory lock, exact generation and canonical-claim proof, strict Party rehydration, reference-only deterministic evidence and clean/reapplied malformed/cross-tenant/stale-lineage/no-write PostgreSQL acceptance. It remains contract-only/non-runtime.

## Architecture scalability governance

PR #157 was accepted on unchanged source SHA `aca66bf56b206e5f01ed4fa2fa76486a0860290d`, passed all 16 applicable workflows and was squash-merged as `6195fb631b9f01e2de4a4975c1f44de24610938a`.

The accepted architecture direction now requires:

- business modules to remain authoritative while normal capabilities add zero crates;
- measured dependency, fan-out, build and test complexity;
- module-owned contribution aggregation;
- affected-scope iteration with unchanged exact-head final acceptance;
- two contrasting privacy owner implementations before shared protocol extraction;
- gradual behavior-neutral consolidation rather than a workspace rewrite.

PR #158 adds the normative CI scalability, PostgreSQL E2E isolation, immutable Action pinning, cache trust and secure device-lab companion plan.

## Current merged Customer Privacy production inventory

Merged `main` is exactly:

- **4 public mutations:** create, submit, subject verification and cancel;
- **2 permission-aware queries:** case get and subject-scoped case list;
- **10 public Customer Privacy non-runtime coordinates**;
- **9 owner privacy scope contribution coordinates**, all contract-only non-runtime;
- **1 implemented owner contribution:** Parties, still contract-only/non-runtime;
- **0 Customer Privacy worker runtime routes**;
- crypto-shred remains reasoned non-runtime and is not introduced as a public, worker or platform route.

No approval, plan/outcome read, restriction, legal-hold, worker, owner-execution or crypto-shred coordinate has been promoted. The Parties owner privacy scope implementation is merged but is not composed, activated, authorized or exposed as runtime.

## Remaining Phase 8A.11 boundary

Phase 8A.11 is not complete. Remaining work includes a contrasting second owner proof and only then shared protocol extraction, the remaining fully proven owner privacy scope contributions, approval, live restriction and legal-hold precedence, plan and owner-outcome reads, bounded owner contribution/orchestration, privacy export, deletion/anonymization convergence, immutable-evidence preservation, worker recovery and complete lifecycle acceptance.

## Merged platform and customer-master baseline

Merged `main` contains executable architecture governance, typed module/runtime foundations, PostgreSQL tenant/RLS/records/idempotency/outbox/audit, authenticated mutation and permission-aware query gateways, native module-owned exact-coordinate composition, durable workers/projections/search, and production slices for Party, Account, Contact Point, Party Relationship, Customer 360, Consent, reversible Identity Resolution, import, export, Data Quality, Customer Enrichment and six accepted Customer Privacy runtime coordinates. It also contains the first proven non-runtime Parties privacy scope owner implementation.

## Product completeness reality

The project is **not yet a complete universal CRM**. Major required families still include the remaining Phase 8A.11 privacy lifecycle, Product Catalog/Pricing/CPQ/Quotes/Orders/Contracts/Subscriptions, broader Sales and Activities, omnichannel, Service, Marketing, Customer Success, projects, documents/e-signature, analytics, workflow/collaboration, AI governance, marketplace and enterprise operational proof.

## Immediate next actions

1. Land immediate CI controls in separately gated PRs: normalize push/PR events, cancel superseded PR runs, pin third-party Actions to full SHAs and begin CI critical-path measurement.
2. Execute complexity Phase B, then golden package, module-owned contribution aggregation and affected-scope CI with a PostgreSQL isolation pilot.
3. Implement a contrasting second privacy owner, compare it with Parties and extract only proven common protocol behavior.
4. Continue the remaining privacy lifecycle without promoting contract-only coordinates before their own production proof.
5. Close Phase 8A only after the full privacy/customer-master interaction baseline is merged and reconciled; begin Phase 8B / #29 only afterward.
