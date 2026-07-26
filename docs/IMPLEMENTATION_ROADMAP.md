# Ultimate CRM — Implementation Roadmap

Status: **Normative delivery plan**

Parent epic: #2  
Governing rules: `SYSTEM_INVARIANTS.md`  
Delivery-control policy: `DELIVERY_GOVERNANCE.md`  
Current concise state: `PROJECT_STATUS.md`  
Detailed Phase 8 sequence: `PHASE8_DELIVERY_PLAN.md`  
Functional completeness guardrail: `CRM_CAPABILITY_COVERAGE.md`  
Business-module accounting: `MODULE_CATALOG.md`

## 1. Purpose

This roadmap defines the dependency order for a universal modular expert CRM platform. It is not a feature wishlist or a second status page.

A phase or packet is complete only when its acceptance boundary is implemented, merged and backed by the required exact-head automated or operational evidence. Universal does not mean one giant Sales module: customer identity, communications, service, catalog, pricing, commercial commitments, subscriptions, billing, consent and other independent domains retain explicit versioned ownership.

## 2. Delivery rules

1. Work is delivered as coherent reviewable packets linked to roadmap issues.
2. Every mutable aggregate has exactly one authoritative owner module.
3. State-changing behavior enters through an exact versioned capability and produces typed audit evidence.
4. Search, analytics, caches and projections remain rebuildable and non-authoritative.
5. Published contracts, policies, metadata and module versions are immutable.
6. Security, privacy, tenant isolation, compatibility and rollback are implementation requirements.
7. Business modules use governed SDK/platform boundaries and never access another module’s storage directly.
8. Exact money, time, identity, lifecycle and authorization semantics use typed contracts rather than conventions.
9. A backend packet is not production-ready while real composition, persistence or process acceptance is missing.
10. Every source or documentation change invalidates earlier exact-SHA gate evidence until applicable checks rerun.
11. Roadmap, status, phase plan, module catalog, issues and PR descriptions are synchronized under `DELIVERY_GOVERNANCE.md`.
12. No milestone may claim the universal CRM product is complete while required capability families remain unimplemented or unclassified.

## 3. Work states

- Planned
- Ready
- In progress
- Gate review
- Complete
- Blocked
- Superseded

Only merged work may be represented as **Complete** in `main` documentation.

## 4. Phase map

| Phase | Issue | Primary result | State | Depends on |
|---|---:|---|---|---|
| 0.1 | #3 | Repository hardening and executable roadmap | **Complete** | Governance v1 |
| 1 | #4 | Typed Module Manifest IR and deterministic identity | **Complete** | #3 |
| 2 | #5 | Governed Module SDK and test harness | **Complete** | #4 |
| 3 | #6 | Module lifecycle and registry runtime | **Complete** | #4, #5 |
| 4 | #7 | PostgreSQL tenant, record, artifact, outbox and audit foundation | **Complete** | #6 |
| 5 | #8 | Capability execution gateway | **Complete** | #5, #7 |
| 6 | #9 | Sales + Activities + link/projection/application vertical proof | **Complete** | #8 |
| 7 | #10 | Search, generalized projections, Admin Studio, product shell and UI-extension isolation | **Complete** | #9 |
| 8 | #11 | Expert modules and product-quality CRM experience | **In progress** | #5, #9, #10 |
| 8A | #28 | Canonical customer master, identity, consent and governed customer-data lifecycle | **In progress** | #9, #10 |
| 8B | #29 | Product catalog, CPQ and quote-to-revenue lifecycle | **Planned** | completed 8A baseline |
| 9 | #12 | AI-native governed actor/tool layer | **Planned** | mature domain capabilities |
| 10 | #13 | Signed marketplace and sandboxed untrusted extensions | **Planned** | #6, #8, #10 |
| 11 | #14 | Enterprise security, resilience and production proof | **Planned / continuous hardening** | all critical phases |

## 5. Completed platform foundation

### Phases 0.1–5 — Complete

Delivered repository governance, immutable module identity, governed Module SDK, module lifecycle, PostgreSQL tenant/RLS/record/artifact/idempotency/outbox/audit foundations and exact-version authenticated capability execution.

### Phase 6 — Complete

Delivered independent Sales `Deal` and Activities `Task` owners, versioned contracts, PostgreSQL-backed mutation/query paths, authenticated HTTP/gRPC ingress, governed event delivery, the optional Sales–Activities link, rebuildable projections and a deployable `crm-api` process.

### Phase 7 — Complete

Delivered golden module tooling, generalized projection runtime, permission-aware global search, typed web shell, immutable tenant-authorized metadata, Admin Studio publication/rollback and trusted-code UI-extension failure isolation.

### Native application-composition integrity — Complete

Issue #134 / PR #135 / merge `023fa5ef1d510d5bcc32222c739e6d58e5696fb8` established module-owned exact-coordinate routing, durable tenant activation, pre-authorization cross-owner semantics, deterministic worker contributions, exact production-route parity, immutable compatibility gates and production contribution scaffolding.

## 6. Phase 8A — canonical customer master and governed customer-data lifecycle

State: **In progress**  
Parent issue: #28

Completed packets:

- **8A.1–8A.6** — customer references, Party, Account, Contact Point, Party Relationship, Customer 360, Consent and reversible Identity Resolution.
- **8A.7** — governed immutable import sources, server-side parsing/validation, resumable Party import and crash/retry recovery (#120 / PR #121).
- **8A.8** — governed Party export jobs, immutable selection/manifests, deterministic artifacts, reconciliation and both crash-window recoveries (#123 / PR #130).
- **8A.9** — Customer Data Quality Rules, Completeness and Stewardship (#124 / PR #132; merge `8a1664309be9dc0c5e3bf9014cf248b1c3680035`).
- **8A.10** — Governed Customer Enrichment and Provenance (#125 / PR #137; accepted source `f92d101206886e3ceaf94d0e56e52580cec21093`; merge `150e44b95d9dbdc08c1792563de03ec73f34aed1`).

Active sequence:

1. **8A.11 / #126 — In progress:** architecture/domain/contracts/FORCE-RLS foundation is merged through PR #145; `case.create`, `case.submit`, `case.subject.verify`, `case.get`, `case.cancel` and `case.list` are merged through PR #152; owner-scope contracts and authoritative Parties, Consents, Customer Accounts, Contact Points and Party Relationships implementations are merged through PR #181.
2. **Architecture scalability runway — Complete:** measured complexity/cache decisions, module-owned contribution proofs, first-party aggregation, explainable affected-scope iteration and the bounded PostgreSQL isolation pilot are accepted through PR #173.
3. **Shared owner-scope support — Complete through PR #176:** accepted source `eb8e6b6f2edf038485e5c64014d7d28dba302ce8`, merge `80411d54a3ca45a783d982152c5cd8317f1fd9bd`; only proven common mechanics were extracted and owner behavior remained unchanged. PRs #179 and #181 added Customer Accounts and Contact Points as the third and fourth mechanically permitted consumers without changing shared behavior.
4. **Customer Accounts owner slice — Complete through PR #179:** accepted source `7d3e44e6dede36f76dfe92145dea6129a2b4639e`, merge `5b5252a437c6bebbd7afdead0162063af4c0b7e4`, 23/23 applicable permanent workflows; authoritative embedded `Primary`/`Member` Account associations, strict rehydration, bounded owner pagination and reference-only no-write evidence remain contract-only/non-runtime.
5. **Contact Points owner slice — Complete through PR #181:** accepted source `00c5b940326b14f5e4aab7d8c8b467ee688f6c9c`, merge `96cd0cf548310592a0718c97242a724a29717a72`, 24/24 applicable permanent workflows; exact direct Party binding, strict Contact Point rehydration, bounded owner pagination and reference-only endpoint-value-free evidence remain contract-only/non-runtime.
6. **Next bounded owner slice — Identity Resolution:** implement `party_relationships.privacy.scope.contribute@1.0.0` through authoritative two-endpoint relationship state, strict temporal/directional rehydration, deterministic reference-only evidence and tenant/RLS/no-write proof. Add no runtime route, worker, production schema migration, Customer Privacy orchestration or speculative shared abstraction.
7. **Remaining owner slices:** implement the other privacy owner contributions one bounded owner at a time on the validated support boundary before runtime promotion.
8. **Scope discovery and lifecycle:** assemble a sufficient owner set, then separately prove discovery/planning, approval, restrictions, legal-hold/retention precedence, plan/outcome reads, resumable orchestration, export/deletion/convergence and workers.
9. **Phase 8A closure:** only after the complete privacy/customer-master interaction baseline is merged and reconciled.
10. **8B / #29:** starts only from the completed Phase 8A baseline.

### Phase 8A.10 accepted boundary

The frozen inventory is exactly **6 public mutations + 6 permission-aware queries + 5 activation-gated worker-only coordinates**. All 17 manifest-bound coordinates are public runtime or worker runtime; no completed Customer Enrichment coordinate remains non-runtime.

The merged packet includes immutable provider/mapping/request/response/conflict/suggestion/review/usage/application evidence, exact provider transport and secret boundaries, independent worker authorization, deterministic replay/recovery, governed Party application, permission-aware reads, transaction-scoped reference guards, FORCE RLS, rollback/reapply and permanent real-process acceptance.

### Phase 8A.11 merged foundation

Issue #126 freezes `crm.customer-privacy` as the privacy case and orchestration owner while existing modules retain authoritative Party, Account, Contact Point, Relationship, Consent, Identity Resolution, import/export, Data Quality and Enrichment values.

Merged bounded layers:

- PR #140 — ownership/enforcement architecture freeze;
- PR #141 — owner foundation;
- PR #142 — deterministic pure-domain lifecycles;
- PR #143 — canonical private persistence;
- PR #144 — immutable public contracts;
- PR #145 — FORCE RLS persistence proof.

PR #145 was accepted on `f37d9a5e025745abaaf0aeb351ff9bb534455aab` and merged as `721a1cf185ffbdea309bd1199c6c4568cf82d7a1`. It proves clean migrations, FORCE RLS under a non-privileged runtime role, tenant isolation and concealment, full rollback/schema removal/reapply, repeated FORCE RLS and strict persistence-envelope metadata validation.

The architecture inventory remains:

- **9 public mutations**;
- **7 permission-aware public queries**;
- **9 trusted worker/internal coordinates** in deterministic phases 260 → 270 → 280 → 290;
- **1 reasoned non-runtime crypto-shredding coordinate**.

### Phase 8A.11.1 — `case.create` — Complete

PR #146 accepted unchanged source `9b53c3ebd81b58518dc445b02b33b35403ffa7c3`, passed all 18 applicable workflows and merged as `2d28937a123e4ba31ab0d835c4c30e3dfed0f187`.

Accepted: deterministic tenant/idempotency identity, confidential Draft/version-1 state, optional terminal predecessor lineage, exact replay/conflict, one atomic record/event/audit/idempotency batch, generic ingress, live authorization/activation, FORCE RLS and permanent real-process acceptance.

### Phase 8A.11.2 — `case.submit` — Complete

PR #147 accepted unchanged source `8b41e8420b1a897777596c68cb615e2b8bf80c34`, passed all 18 permanent workflows and merged as `0eba56084405301eb667f2173b3aef6565b95f87`.

Accepted: strict `MustExist` rehydration, optimistic `Draft -> Submitted`, atomic status evidence, exact replay, incompatible replay, stale/wrong-state/cross-tenant/malformed rollback, generic ingress and clean/reapplied FORCE-RLS process proof.

### Phase 8A.11.3 — `case.subject.verify` — Complete

PR #148 accepted unchanged source `118327e09a6e31ba87b02bdab99289035b572ed9`, passed all 18 permanent workflows and merged as `8ee5538bf97031dd48ab3726a605b9f3ad4bfd1e`.

Accepted: authoritative Party visibility, canonical redirect/active merge lineage, monotonic topology generation, shared topology/canonical-subject locks, atomic `Submitted N -> SubjectVerified N + 1`, replay/conflict/concealment/malformed/lock-contention proof and generic real HTTP/gRPC ingress.

### Phase 8A.11.4 — `case.get` — Complete

PR #149 accepted unchanged post-sync source `5a47318b24007cd534434ff6bac33fbd59215d38`, passed all 18 permanent workflows and merged as `5d580a7c253bcfa6c2dd981100612b222fd26825`.

Accepted: exact confidential contracts, FORCE-RLS lookup, strict aggregate rehydration, live case/canonical-Party visibility, uniform concealment, field redaction, generic ingress and no query-side writes.

### Phase 8A.11.5 — `case.cancel` — Complete

PR #150 accepted unchanged post-sync source `be05e874b21ab33cb8b6a84fbcefc3c025aa88cb`, passed all 18 permanent workflows and was squash-merged as `2a4c34727e9d7bf8ed51b6411b7ab9c76c109671`.

Accepted: strict optimistic terminal cancellation; immutable subject/rescope/scope/plan/approval lineage; sorted/deduplicated subject locks before a retained final case-row `FOR UPDATE`; direct row serialization for unbound cases; retryable TOCTOU denial; one atomic record/event/audit/idempotency transaction; exact replay/conflict; generic ingress; permanent clean/reapplied process proof.

### Phase 8A.11.6 — `case.list` — Complete

PR #152 accepted unchanged source `9de6048f951c0797a94871457d2bdd73357aee59`, passed all 18 permanent workflows and was squash-merged as `26f5b4644c935001806343b2feaf802a78c90eae`.

Accepted boundary:

- required canonical Party scope with optional kind/status filters;
- page size default 50 and maximum 100;
- HMAC cursor bound to tenant, actor, capability/version, Party, filters, updated-at sort and page size;
- tenant-bound FORCE-RLS keyset scan with a hard 4096-candidate ceiling;
- strict persistence-envelope and aggregate rehydration;
- matching only authoritative verified canonical subject bindings;
- no disclosure from unbound cases or pending-rescope targets;
- live Party visibility before scanning and live case visibility before output;
- per-case field redaction preserving stable case identity;
- generic HTTP/gRPC query composition with activation and live authorization;
- no audit, idempotency, business-transaction, event, outbox or record writes;
- permanent isolated real-process proof for two-page pagination, no duplicates, filters, cursor tamper/rebinding, cross-tenant empty concealment, redaction, suspension and missing grants;
- merged production partition of four runtime mutations, two runtime queries, ten public non-runtime coordinates and zero Customer Privacy workers.

The accepted packet excludes approval, plan/outcome reads, restrictions, legal holds, workers, owner execution and crypto-shred.

### Phase 8A.11 remaining acceptance boundary

The remaining program must prove access/export through existing governed artifacts; immediate restriction through the accepted shared tenant + canonical Party lock; deterministic owner/data-class action plans; legal-hold/retention precedence; resumable orchestration; search/projection/cache convergence; non-reusable erased Party tombstones; immutable evidence preservation; cross-tenant denial; migration safety; and complete real-process acceptance.

Phase 8A is complete only when privacy access/export/restriction/deletion/legal-hold interactions are merged and reconciled with Consent, Identity Resolution, Import/Export, Data Quality and Customer Enrichment.

## 7. Phase 8B — product catalog, pricing, CPQ and quote-to-revenue

State: **Planned**  
Issue: #29

Required owner domains include Product Catalog, Price Books/Pricing, CPQ, Quotes, Orders, Contracts, Subscriptions/Entitlements and governed billing/ERP/payment/tax/fulfillment integration boundaries. Catalog, pricing and commercial commitment ownership must not be absorbed into Sales.

## 8. Additional expert-product waves

After stable prerequisite domains, Phase 8 continues with Sales/Activities expert expansion, communications and omnichannel, Service/Support/Knowledge/Field Service, Marketing, Customer Success/PRM, projects/configurable work, documents/e-signature, analytics/performance management, workflow/approvals/collaboration and complete responsive accessible product UX.

Each authoritative domain receives an explicit owner and cannot be hidden inside generic metadata or a giant Sales module.

## 9. Later platform phases

### Phase 9 — AI-native CRM

AI is an authenticated audited Actor using permission-scoped governed tools. Required outcomes include tenant/data-class/purpose/residency/cost-aware routing, permission-filtered retrieval, live authorization, approvals, budgets/failure controls and security/correctness evaluations.

### Phase 10 — signed marketplace and sandbox

Required outcomes include signed packages, publisher identity, dependency/compatibility resolution, SBOM/provenance policy, explicit grants, sandboxed untrusted execution, quotas, timeouts, kill switch and safe lifecycle operations.

### Phase 11 — enterprise security and production proof

Required outcomes include OIDC/SAML, SCIM, enterprise authorization, key hierarchy/encryption, WORM audit export, privacy/legal-hold integration, backup/PITR/restore, residency, supply-chain/security testing, load/chaos proof, SLOs, alerting, incident response and runbooks.

## 10. Immediate authoritative delivery sequence

1. Complete this post-merge documentation synchronization for PR #152 without changing runtime inventory.
2. Compare approval, restriction placement, legal-hold precedence, plan/outcome reads and worker dependencies.
3. Select exactly one next Customer Privacy production boundary; do not combine unrelated trust surfaces.
4. Keep all remaining privacy coordinates non-runtime until their own production proofs are complete.
5. Close Phase 8A only after the full merged customer-master acceptance baseline is proven.
6. Begin Phase 8B / #29 from the completed customer-master baseline.


PR #183 accepted unchanged user-authored source `a431185e01e95dfeffcf7d9c9a440afc8f0c9a57`, passed all 25 applicable permanent workflows and was squash-merged as `9ad2aa91321e9edb54cab98218f93143923ef33f`. Party Relationships proves the fifth authoritative owner shape through strict two-endpoint matching, directional/reciprocal and temporal domain rehydration, bounded owner-specific pagination/cursors, reference-only evidence, response-byte non-disclosure, clean rollback/schema-removal/reapply acceptance and zero query-side writes. It remains contract-only/non-runtime and changes no shared-support behavior.
