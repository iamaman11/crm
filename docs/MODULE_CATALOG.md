# Ultimate CRM — Module Catalog

Status: **Normative business-module ownership and readiness catalog**

Delivery governance: `DELIVERY_GOVERNANCE.md`  
Roadmap: `IMPLEMENTATION_ROADMAP.md`  
Functional completeness guardrail: `CRM_CAPABILITY_COVERAGE.md`

This document tracks business-domain ownership and readiness without confusing technical crates, services, projections or contracts with product modules. Current delivery sequence belongs in the roadmap and project status, not in this catalog.

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
| `crm.identity-resolution` | Duplicate cases, merge lineage and canonical resolution | **Expert expansion** | Candidate/review plus reversible merge/unmerge | Broader survivorship and production privacy orchestration |
| `crm.customer-data-operations` | Governed import/export jobs and evidence | **Expert expansion** | Resumable import, deterministic export/artifacts/reconciliation and crash recovery | More resource profiles and privacy access/deletion integration |
| `crm.data-quality` | Customer-data quality governance coordinator | **Vertical slice** | Exact-version Party evaluation, findings/completeness, stewardship and governed remediation | Additional owner-resource profiles and privacy coordination |
| `crm.customer-enrichment` | Provider-neutral enrichment coordinator | **Production integration slice** | Exact provider transport/secret boundary, immutable provenance, review, deterministic Party owner-capability application and recovery | Additional providers, target fields, product UX and privacy interaction |
| `crm.customer-privacy` | Privacy case, restriction/legal-hold and owner-orchestration coordinator | **Vertical slice** | Case create/submit/subject verification/cancel plus permission-aware get/list, Party/topology guards, subject locks, FORCE RLS and real-process acceptance | Approval, plans/outcomes, restriction/legal-hold precedence, owner orchestration, export/deletion/convergence and workers |

Current merged authoritative/coordination module count: **12**.

## 4. Implemented link module

| Module | Type | State | Implemented production slice |
|---|---|---|---|
| `crm.sales-activities-link` | Optional link module | **Production integration slice — Complete** | Restart-safe stage-event delivery, lifecycle gating and governed Activities invocation with retry/dead-letter/idempotency evidence |

Current merged business-module total: **13** — twelve authoritative/coordination modules plus one optional link module.

## 5. Independently governed read composition

`crm.customer360` is a lifecycle-managed read-composition module. It owns versioned Customer 360 contracts, rebuildable contributions and permission-aware assembly/freshness metadata. It owns no mutable customer-master values and is tracked separately from the owner/link count.

## 6. Customer Enrichment boundary

Phase 8A.10 / issue #125 / PR #137 is complete. Accepted source `f92d101206886e3ceaf94d0e56e52580cec21093` passed all 17 permanent workflows unchanged and was squash-merged as `150e44b95d9dbdc08c1792563de03ec73f34aed1`.

Frozen production inventory:

- six public mutations;
- six permission-aware queries;
- five activation-gated worker-only coordinates;
- zero completed non-runtime coordinates.

Mutable customer values remain with authoritative modules. Customer Enrichment owns coordination and immutable provenance, not Party values.

## 7. Customer Privacy boundary

Phase 8A.11 / issue #126 remains **In progress**.

Merged production inventory:

- four public mutations;
- two permission-aware public queries;
- ten public non-runtime Customer Privacy coordinates;
- zero Customer Privacy workers.

Nine owner-scope contribution coordinates are published and remain contract-only/non-runtime. Six authoritative implementations are accepted:

- PR #156 — Parties, one authoritative Party record;
- PR #175 — Consents, multiple authoritative Consent records through owner relationships and bounded pagination;
- PR #179 — Customer Accounts, strict Account rehydration and embedded `Primary`/`Member` Party associations;
- PR #181 — Contact Points, strict endpoint rehydration and direct Party binding;
- PR #183 — Party Relationships, strict two-endpoint temporal relationship rehydration;
- PR #186 — Identity Resolution, bounded active alias graph, candidate/merge persistence, provenance-only discovery and heterogeneous pagination.

All six remain non-runtime and add no Customer Privacy worker or public ingress.

Shared support was accepted in PR #176 / merge `80411d54a3ca45a783d982152c5cd8317f1fd9bd`. Later owner PRs extend only mechanical consumer and bound-read allowlists where independently proven; owner-specific SQL, rehydration, pagination, evidence, response and errors remain outside shared support.

Customer Data Operations is the next bounded contract-only owner implementation through `customer_data.privacy.scope.contribute@1.0.0`. Data Quality and Customer Enrichment owner contributions follow one at a time before production scope discovery/planning.

The Customer Data Operations packet distinguishes subject-level import-row/export-selection/execution evidence from multi-subject job, progress and artifact containers. It requires bounded alias-safe scans, strict owner rehydration and reference-only output without introducing container-level deletion semantics.

The merged Customer Privacy boundary proves deterministic case identity, optimistic lifecycle transitions, authoritative Party/topology proof, permission-aware reads, race-free cancellation, signed pagination, FORCE RLS, rollback/reapply and real HTTP/gRPC acceptance. It does not yet prove approval, immediate restrictions, legal-hold/retention precedence, owner execution, access/export, deletion/anonymization, convergence or workers.

## 8. Phase 8A packet accounting

Completed:

- 8A.1–8A.6 — customer references, Party, Account, Contact Point, Party Relationship, Customer 360, Consent and reversible Identity Resolution;
- 8A.7 — Customer Import;
- 8A.8 — Customer Export;
- 8A.9 — Customer Data Quality;
- 8A.10 — Governed Customer Enrichment and Provenance.

In progress:

- 8A.11 / #126 — Customer Privacy. Six of nine owner contributions are accepted through PR #186. Customer Data Operations is next; Data Quality, Customer Enrichment, discovery/planning, approval, restrictions, legal holds, execution, access/export, deletion/anonymization and convergence remain incomplete.

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

The remaining privacy lifecycle, Sales/Activities expansion, omnichannel, Service/Knowledge/Field Service, Marketing, Customer Success, optional PRM, projects/configurable work, documents/e-signature, analytics/performance management, workflow/approvals/collaboration, AI governance, marketplace and enterprise operational proof remain incomplete or planned.

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
