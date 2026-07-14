# Ultimate CRM — Module Catalog

Status: **Normative business-module ownership and readiness catalog**

Delivery governance: `DELIVERY_GOVERNANCE.md`  
Roadmap: `IMPLEMENTATION_ROADMAP.md`  
Functional completeness guardrail: `CRM_CAPABILITY_COVERAGE.md`

This document tracks business-domain ownership and readiness without confusing technical crates, services, projections or contracts with product modules.

## 1. Counting rules

A **business module** is an independently governed runtime unit under `modules/` with:

- stable immutable `module_id`;
- explicit aggregate/object ownership or an explicit optional link/composition role;
- versioned manifest and lifecycle;
- published/consumed versioned contracts;
- independent build/test behavior;
- no direct infrastructure or cross-module storage access.

The following do not count as business modules:

- platform/runtime/composition crates under `crates/`;
- deployable process shells under `services/`;
- Protobuf packages;
- SQL migrations;
- generic search/projection infrastructure;
- an unmerged module skeleton that has not yet established a production or accepted foundation state on `main`.

## 2. Readiness states

- **Planned** — owner/link domain is in the roadmap but implementation has not started.
- **Foundation** — merged manifest/contracts/domain foundation exists but no complete production path.
- **Vertical slice** — at least one real aggregate has production mutation/query acceptance.
- **Production integration slice** — a link module has governed source-event delivery, lifecycle gating, target-capability invocation and production acceptance.
- **Expert expansion** — broader domain surface is actively being delivered on top of a production slice.
- **Product complete** — required domain capabilities, product experience and operational evidence satisfy the module acceptance gates.

Only merged `main` state is counted here. Active draft-PR work is tracked separately and must not inflate implemented module counts.

## 3. Implemented authoritative owner modules

| Module | Ownership | Current readiness | Implemented production slice | Still required |
|---|---|---|---|---|
| `crm.sales` | Sales owner domain | **Vertical slice** | Deal create/update/stage advance/get/list | Leads, qualification, richer pipelines, territories, quotas, forecasting, account plans, revenue intelligence, sequences, coaching and broader expert Sales scope |
| `crm.activities` | Activities/productivity owner domain | **Vertical slice** | Task create/update/complete/reminder/get/list | Appointments, recurring work, queues, calendars, synchronization and broader productivity scope |
| `crm.parties` | Canonical person and organization identity | **Expert expansion** | Party create/update/get/list plus permission-aware rebuildable search | Structured profile depth, broader source-identifier support and later privacy/data-operations interactions |
| `crm.customer-accounts` | Customer/commercial relationship | **Vertical slice** | Account create/update/get/list with typed Party associations | Advanced hierarchy/commercial semantics where justified and later expert product UX |
| `crm.contact-points` | Canonical email/phone/postal/web/messaging endpoints | **Vertical slice** | Create/update/verify/get/list with normalization, verification and preference lifecycle | Broader communication-channel UX and downstream omnichannel use |
| `crm.party-relationships` | Typed temporal Party-to-Party relationships | **Vertical slice** | Create/update/get/list plus rebuildable hierarchy foundation | Additional governed relationship semantics where justified |
| `crm.consents` | Purpose/channel-scoped Consent and Communication Authorization | **Vertical slice** | Immutable assertions, grant withdrawal and exact communication-authorization decisions | Broader privacy orchestration and downstream campaign/provider enforcement |
| `crm.identity-resolution` | Duplicate candidates, merge lineage, survivorship provenance and canonical Party resolution | **Expert expansion** | Duplicate-candidate/reviewer-decision slice plus approval-required reversible merge/unmerge and canonical resolution | Further integration with data quality, enrichment and privacy lifecycle packets |

Current implemented authoritative owner-module count: **8**.

## 4. Implemented link module

| Module | Type | State | Implemented production slice |
|---|---|---|---|
| `crm.sales-activities-link` | Optional link module | **Production integration slice — Complete** | Consumes versioned Sales stage-change events through restart-safe governed delivery, checks installation lifecycle and invokes Activities only through the governed capability gateway with durable retry/dead-letter state and target idempotency |

Current counted business-module total: **9** — eight authoritative owner modules plus one optional link module.

## 5. Independently governed read composition

`crm.customer360` is an independently lifecycle-managed **read-composition module**.

It owns:

- the versioned Customer 360 read contract;
- deterministic mapping from validated owner events into rebuildable contributions;
- permission-aware query assembly and freshness/lineage metadata.

It does not own mutable customer-master records and exposes no mutation capability. It is tracked separately from authoritative owner and optional link counts.

## 6. Current active module foundation not yet counted

### `crm.customer-data-operations`

State: **In progress on #120 / draft PR #121; not yet counted as implemented on `main`**.

Intended ownership boundary:

- import/export job lifecycle;
- immutable source/specification/mapping identity;
- deterministic row/work identity;
- validation/execution outcomes;
- resumable checkpoints, counters and reconciliation evidence;
- bounded safe diagnostics and artifact references where applicable.

It must not own Party, Account, Contact Point, Party Relationship, Consent or Identity Resolution records. Target changes must invoke exact governed owner capabilities.

The active 8A.7 branch already contains import domain and strict persistence foundations, but the module must not be promoted in this catalog until the production packet is merged with contracts, runtime/persistence composition and process acceptance.

## 7. Customer-master ownership baseline

Tracked by Phase 8A / #28.

### `crm.parties`

Owns person/organization identity, stable Party identity and authoritative Party lifecycle.

### `crm.customer-accounts`

Owns customer/commercial Account identity, lifecycle, name/status and typed Party associations. It does not own mutable Party identity attributes.

### `crm.contact-points`

Owns typed contact endpoint identity/value, lifecycle, validity, preference and verification. It does not own consent or provider delivery state.

### `crm.party-relationships`

Owns stable typed Party-to-Party relationships and temporal semantics. It does not own Account membership or Party identity fields.

### `crm.consents`

Owns immutable purpose/channel/legal-basis/jurisdiction/source/evidence assertions, withdrawal and exact current communication-authorization decisions.

### `crm.identity-resolution`

Owns canonical duplicate-candidate cases, immutable evidence, reviewer decisions, immutable merge-operation lineage, active canonical redirect topology and survivorship provenance. It does not own mutable Party attributes.

### `crm.customer360`

Owns only rebuildable read composition and permission-aware assembly.

### `crm.customer-data-operations` — active development

Will coordinate governed import/export and related data-operation evidence without becoming a second customer master.

## 8. Phase 8A packet accounting

Completed:

- 8A.1 — customer reference/owner foundations.
- 8A.2a–8A.2c — Party lifecycle and discovery.
- 8A.3a–8A.3d — Account, Contact Point, Party Relationship and Customer 360.
- 8A.4 — Consent and Communication Authorization.
- 8A.5 — Identity Resolution duplicate candidates.
- 8A.6 — reversible merge/unmerge, provenance and survivorship through merged PR #119.

Active:

- 8A.7 / #120 / PR #121 — Customer Import Jobs and Resumable Execution.

Planned:

- 8A.8 / #123 — Customer Export Jobs, Artifacts and Reconciliation Evidence.
- 8A.9 / #124 — Customer Data Quality Rules, Completeness and Stewardship.
- 8A.10 / #125 — Governed Customer Enrichment and Provenance.
- 8A.11 / #126 — Customer Privacy Lifecycle, Restriction, Deletion and Legal Hold.

## 9. Mandatory commercial lifecycle owner domains

Tracked by Phase 8B / #29.

Planned explicit ownership:

- Product Catalog;
- Price Books and Pricing;
- CPQ/configuration and pricing explanation;
- Quotes and immutable revisions;
- Orders;
- Contracts and amendments;
- Subscriptions, entitlements and usage;
- governed billing/ERP/payment/tax/fulfillment integration boundaries.

State: **Planned**.

## 10. Expert CRM product areas still requiring owner modules or explicit product capabilities

Tracked by Phase 8 / #11 and `CRM_CAPABILITY_COVERAGE.md`:

- Sales expert expansion;
- Activities/productivity expert expansion;
- communications and omnichannel interaction history;
- Service/Support, Knowledge and optional Field Service;
- Marketing automation, segmentation, journeys and attribution;
- Customer Success;
- optional Partner Relationship Management;
- projects and configurable work management;
- documents and e-signature;
- analytics, reporting and performance management;
- data operations and integrations;
- workflow, approvals and human tasks;
- collaboration and personal productivity.

These remain incomplete except for the existing merged production slices explicitly listed above.

## 11. Platform capabilities that are not business modules

Major platform workstreams include:

- Module SDK and registry;
- Capability and Query Gateways;
- PostgreSQL authoritative runtime;
- event delivery and rebuildable projection runtime;
- production application composition and process host;
- permission-aware search and neutral cross-domain search composition;
- Admin Studio metadata publication;
- product shell/design system and trusted UI-extension runtime;
- AI actor/tool layer;
- signed marketplace/WASM sandbox;
- enterprise security and operational proof.

These must not inflate business-module counts.

## 12. Module creation checklist

Before introducing a new module:

1. Prove a distinct authoritative ownership boundary or optional coordination role.
2. Explain why the behavior cannot remain inside an existing owner or platform runtime.
3. Define immutable module identity and lifecycle.
4. Define provided/consumed versioned contracts.
5. Define storage ownership, retention and uninstall semantics.
6. Define permissions and data classes.
7. Define failure, retry and idempotency behavior.
8. Define disabled/uninstalled behavior where applicable.
9. Add architecture and production acceptance gates.
10. Update roadmap/status/catalog under `DELIVERY_GOVERNANCE.md`.

Do not create a module solely because a directory, screen, table or team exists.

## 13. Completion accounting

Current count of **product-complete expert modules: 0**.

A module is not product-complete merely because a crate, schema, manifest or one backend path exists. Product-complete status requires the applicable domain breadth, governed APIs, persistence, authorization, audit, product UX and production/operational evidence.

The final universal CRM will contain substantially more than the current nine counted business modules. The exact final count is driven by authoritative ownership boundaries rather than an arbitrary target.