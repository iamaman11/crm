# Ultimate CRM — Module Catalog

Status: **Normative business-module ownership and readiness catalog**

Delivery governance: `DELIVERY_GOVERNANCE.md`  
Roadmap: `IMPLEMENTATION_ROADMAP.md`  
Functional completeness guardrail: `CRM_CAPABILITY_COVERAGE.md`

This document tracks business-domain ownership and readiness without confusing technical crates, services, projections or contracts with product modules.

## 1. Counting rules

A business module is an independently governed runtime unit under `modules/` with stable identity, explicit ownership or coordination role, versioned lifecycle/contracts, independent build/test behavior and no direct infrastructure or cross-module storage bypass.

Technical crates, process shells, Protobuf packages, SQL migrations and generic projection/search infrastructure do not count as business modules. Only merged `main` state affects totals.

## 2. Readiness states

- **Planned** — roadmap scope not started.
- **Foundation** — merged manifest/contracts/domain foundation without a complete production path.
- **Vertical slice** — at least one governed production mutation/query/process lifecycle.
- **Production integration slice** — governed integration/coordinator lifecycle with real external/owner boundaries and production acceptance.
- **Expert expansion** — broader domain surface on top of a production slice.
- **Gate review** — unmerged packet awaits synchronized exact-head evidence/review.
- **Product complete** — full required domain/product/operational acceptance is satisfied.

## 3. Implemented authoritative owner and coordination modules

| Module | Ownership | Current merged readiness | Implemented production slice | Still required |
|---|---|---|---|---|
| `crm.sales` | Sales owner domain | **Vertical slice** | Deal create/update/stage/get/list | Leads, richer pipelines, territories, quotas, forecasting and expert Sales scope |
| `crm.activities` | Activities/productivity owner | **Vertical slice** | Task create/update/complete/reminder/get/list | Appointments, recurring work, calendars, synchronization and broader productivity |
| `crm.parties` | Canonical person/organization identity | **Expert expansion** | Party create/update/get/list/search | Structured profile depth and broader source identifiers |
| `crm.customer-accounts` | Customer/commercial relationship | **Vertical slice** | Account create/update/get/list with Party associations | Advanced hierarchy/commercial semantics and product UX |
| `crm.contact-points` | Canonical communication endpoints | **Vertical slice** | Create/update/verify/get/list | Broader channel UX and downstream omnichannel use |
| `crm.party-relationships` | Typed temporal Party relationships | **Vertical slice** | Create/update/get/list and hierarchy foundation | Additional governed relationship semantics |
| `crm.consents` | Purpose/channel Consent and Communication Authorization | **Vertical slice** | Immutable assertions, withdrawal and exact authorization decisions | Privacy orchestration and wider downstream enforcement |
| `crm.identity-resolution` | Duplicate cases, merge lineage and canonical resolution | **Expert expansion** | Candidate/review plus reversible merge/unmerge | Privacy lifecycle integration and broader survivorship |
| `crm.customer-data-operations` | Governed import/export jobs and evidence | **Expert expansion** | Resumable import, deterministic export/artifacts/reconciliation and crash recovery | More resource profiles and privacy access/deletion integration |
| `crm.data-quality` | Customer-data quality governance coordinator | **Vertical slice** | Exact-version Party evaluation, findings/completeness, stewardship and governed remediation | Additional owner-resource profiles and privacy coordination |
| `crm.customer-enrichment` | Provider-neutral enrichment coordinator | **Production integration slice** | Exact provider transport/secret boundary, immutable provenance, review, deterministic Party owner-capability application and recovery | Additional providers, target fields, product UX and privacy interaction |
| `crm.customer-privacy` | Privacy case, restriction/legal-hold and owner-orchestration coordinator | **Vertical slice** | Deterministic, live-authorized and activation-gated case create/submit/subject verification with authoritative Party/topology guards, shared subject locks, FORCE RLS and permanent real-process acceptance | Permission-aware reads, approval/cancel, restriction/legal-hold precedence, owner orchestration, export/deletion/convergence and workers |

Current merged authoritative/coordination module count: **12**.

## 4. Implemented link module

| Module | Type | State | Implemented production slice |
|---|---|---|---|
| `crm.sales-activities-link` | Optional link module | **Production integration slice — Complete** | Restart-safe stage-event delivery, lifecycle gating and governed Activities invocation with retry/dead-letter/idempotency evidence |

Current merged business-module total: **13** — twelve authoritative/coordination modules plus one optional link module.

## 5. Independently governed read composition

`crm.customer360` is a lifecycle-managed read-composition module. It owns versioned Customer 360 contracts, rebuildable contributions and permission-aware assembly/freshness metadata. It owns no mutable customer-master values and is tracked separately from the owner/link count.

## 6. Customer Enrichment boundary

Phase 8A.10 / issue #125 / PR #137 is Complete. Accepted source `f92d101206886e3ceaf94d0e56e52580cec21093` passed all 17 permanent workflows unchanged and was squash-merged as `150e44b95d9dbdc08c1792563de03ec73f34aed1`.

Ownership includes provider-neutral requests and immutable provider-profile, mapping, response/conflict, suggestion/provenance, review, usage and owner-application evidence. Mutable customer values remain with authoritative modules; provider HTTP, secrets, quotas/circuits and PostgreSQL reference guards remain host-owned infrastructure.

Frozen production inventory:

- **6 public mutations**;
- **6 permission-aware queries**;
- **5 activation-gated worker-only coordinates**;
- **0 completed non-runtime coordinates**.

## 7. Customer Privacy boundary

Phase 8A.11 / issue #126 remains **In progress**. PRs #140–#145 merged the architecture freeze, owner foundation, deterministic domain, canonical persistence, immutable public contracts and FORCE RLS proof. PR #145 accepted source `f37d9a5e025745abaaf0aeb351ff9bb534455aab` was merged as `721a1cf185ffbdea309bd1199c6c4568cf82d7a1`.

The accepted production vertical slices are deliberately bounded:

- PR #146 / `customer_privacy.case.create@1.0.0` — accepted source `9b53c3ebd81b58518dc445b02b33b35403ffa7c3`, merge `2d28937a123e4ba31ab0d835c4c30e3dfed0f187`;
- PR #147 / `customer_privacy.case.submit@1.0.0` — accepted source `8b41e8420b1a897777596c68cb615e2b8bf80c34`, merge `0eba56084405301eb667f2173b3aef6565b95f87`;
- PR #148 / `customer_privacy.case.subject.verify@1.0.0` — accepted source `118327e09a6e31ba87b02bdab99289035b572ed9`, merge `8ee5538bf97031dd48ab3726a605b9f3ad4bfd1e`.

The merged production boundary proves:

- deterministic tenant/idempotency case identity and confidential Draft/version-1 state;
- exact optimistic `Draft -> Submitted -> SubjectVerified` transitions with replay-safe atomic evidence;
- authoritative Party existence/tenant visibility, canonical redirect and active merge lineage;
- monotonic Identity Resolution topology generation and shared fail-fast topology/canonical-subject locks;
- common live authorization and activation through generic HTTP/gRPC application ingress;
- fresh PostgreSQL, non-privileged FORCE RLS, rollback/schema removal/reapply and permanent real-process acceptance;
- exactly three runtime Customer Privacy mutations and thirteen remaining public privacy coordinates still non-runtime on merged `main`.

Draft PR #149 is the separately bounded Gate-review candidate for permission-aware `customer_privacy.case.get@1.0.0`. It does not alter the merged-readiness count until accepted and merged.

Ownership:

- privacy cases and verified-subject orchestration;
- immutable scope snapshots;
- processing/communication restrictions;
- customer-data legal holds and retention decisions;
- deterministic owner plans, attempts/outcomes and checkpoints;
- governed export references and convergence evidence.

Non-ownership:

- Party, Account, Contact Point, Relationship, Consent, Identity Resolution, import/export, Data Quality and Enrichment authoritative state remains with those modules;
- PostgreSQL composition guards remain infrastructure adapters outside the pure business core;
- derived projections, search and caches remain non-authoritative.

These vertical slices do not make the Customer Privacy module product-complete or complete Phase 8A.11.

## 8. Phase 8A packet accounting

Completed:

- 8A.1 — customer references and owner foundations.
- 8A.2 — Party lifecycle and discovery.
- 8A.3 — Account, Contact Point, Party Relationship and Customer 360.
- 8A.4 — Consent and Communication Authorization.
- 8A.5 — Identity Resolution duplicate candidates.
- 8A.6 — reversible merge/unmerge and survivorship.
- 8A.7 — Customer Import.
- 8A.8 — Customer Export.
- 8A.9 — Customer Data Quality.
- 8A.10 — Governed Customer Enrichment and Provenance.

In progress:

- 8A.11 / #126 — Customer Privacy. Three production mutations are merged independently; permission-aware `case.get` is in separate Gate review and the remaining lifecycle stays incomplete.

## 9. Customer-master ownership baseline

- `crm.parties` owns canonical Party identity and lifecycle.
- `crm.customer-accounts` owns commercial Account identity/lifecycle and Party associations.
- `crm.contact-points` owns typed endpoint identity/value/lifecycle/verification.
- `crm.party-relationships` owns stable temporal Party relationships.
- `crm.consents` owns immutable authorization assertions, withdrawal and current decisions.
- `crm.identity-resolution` owns candidate/reviewer/merge lineage, canonical redirect and survivorship provenance.
- `crm.customer-data-operations` owns import/export job/evidence lifecycles, not customer values.
- `crm.data-quality` owns quality definitions/evidence/stewardship, not customer values.
- `crm.customer-enrichment` owns enrichment coordination/evidence, not customer values.
- `crm.customer-privacy` owns privacy coordination/evidence, not customer values.
- `crm.customer360` owns only rebuildable read composition.

## 10. Mandatory commercial lifecycle domains

Tracked by Phase 8B / #29 and currently **Planned**:

- Product Catalog;
- Price Books and Pricing;
- CPQ;
- Quotes and immutable revisions;
- Orders;
- Contracts and amendments;
- Subscriptions, entitlements and usage;
- governed billing/ERP/payment/tax/fulfillment boundaries.

These domains must not be absorbed into Sales.

## 11. Other expert CRM domains still required

The remaining Phase 8A.11 privacy runtime, Sales/Activities expansion, omnichannel, Service/Knowledge/Field Service, Marketing, Customer Success, optional PRM, projects/configurable work, documents/e-signature, analytics/performance management, workflow/approvals/collaboration, AI governance, marketplace and enterprise operational proof remain incomplete or planned.

## 12. Module creation checklist

Before introducing a module:

1. Prove a distinct ownership or coordination boundary.
2. Explain why the behavior cannot remain in an existing owner/platform runtime.
3. Define immutable identity/lifecycle and provided/consumed contracts.
4. Define storage, retention, permissions and data classes.
5. Define failure, retry, idempotency and disabled/uninstalled behavior.
6. Add architecture and real production acceptance gates.
7. Update roadmap/status/catalog/issue/PR state under `DELIVERY_GOVERNANCE.md`.

## 13. Completion accounting

Current product-complete expert modules: **0**.

A module is not product-complete merely because a crate, schema, manifest or one backend path exists. Product-complete status requires required domain breadth, governed APIs, persistence, authorization, audit, product UX and production/operational evidence.
