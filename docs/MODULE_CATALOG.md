# Ultimate CRM — Module Catalog

Status: **Normative planning catalog**

This document defines what counts as a CRM module and tracks business-domain readiness without confusing technical crates with product modules.

## 1. Counting rules

A **business module** is an independently governed runtime unit under `modules/` with:

- a stable `module_id`;
- explicit aggregate/object ownership;
- a versioned manifest and lifecycle;
- published/consumed capabilities and events;
- independent build/test behavior;
- no direct infrastructure or cross-module storage access.

A **link module** is also a business module, but it owns optional cross-domain coordination rather than the source or target aggregates.

The following do **not** count as business modules:

- platform/runtime crates under `crates/`;
- deployable process shells under `services/`;
- Protobuf packages;
- SQL migrations;
- projections/search indexes unless they become independently lifecycle-managed modules.

## 2. Readiness states

- **Planned** — owner/link domain exists in the roadmap but implementation has not started.
- **Foundation** — manifest/contracts/domain skeleton exists but no complete production path.
- **Vertical slice** — an owner module has at least one real aggregate with production mutation/query acceptance.
- **Production integration slice** — a link module has governed source-event delivery, lifecycle gating, target-capability invocation and production acceptance.
- **Expert expansion** — broader domain surface is actively being delivered.
- **Product complete** — required domain capabilities, operations and product experience satisfy the module acceptance gates.

A manifest may declare future owned object names. That declaration does not mean those objects are implemented.

## 3. Implemented owner modules

| Module | Ownership | Current readiness | Implemented production slice | Still required |
|---|---|---|---|---|
| `crm.sales` | Sales owner domain | **Vertical slice** | Deal create/update/stage advance/get/list | Leads, qualification, pipelines/admin depth, territories, quotas, forecasts, account plans, revenue intelligence, coaching and other expert Sales scope |
| `crm.activities` | Activities/productivity owner domain | **Vertical slice** | Task create/update/complete/reminder/get/list | Appointments, recurring work, queues, calendars, synchronization and other expert activity scope |

Current count: **2 implemented owner modules with production vertical slices**.

Current count of product-complete expert modules: **0**.

## 4. Implemented link module

| Module | Type | State | Implemented production slice |
|---|---|---|---|
| `crm.sales-activities-link` | Link module | **Production integration slice — Complete** | Consumes versioned Sales stage-change events through restart-safe governed delivery, checks tenant installation lifecycle, invokes Activities only through the governed capability gateway, uses durable retry/dead-letter delivery state and target idempotency, and remains independently suspendable/uninstallable |

The published `module_id` is fixed as **`crm.sales-activities-link`** and is treated as immutable module identity.

Current implemented business-module count: **3** — two owner modules plus one optional link module.

## 5. Mandatory customer-master owner domains

Tracked by Phase 8A / issue #28.

These are independent authoritative domains; final packaging may use one or more installable modules only if ownership remains explicit and non-overlapping.

- Party — person and organization identity.
- Account — customer/commercial relationship.
- Contact Point — email, phone, postal and messaging endpoints.
- Party Relationship — employment, household, hierarchy and typed relationships.
- Consent and Preferences — purpose/channel/legal-basis consent and suppression.
- Identity Resolution — source identities, matching, survivorship, merge/unmerge lineage.

State: **Planned**.

## 6. Mandatory commercial lifecycle owner domains

Tracked by Phase 8B / issue #29.

- Product Catalog.
- Price Books and Pricing.
- CPQ/configuration and pricing explanation.
- Quotes and immutable revisions.
- Orders.
- Contracts and amendments.
- Subscriptions and Usage.
- Governed billing/ERP/payment/tax/fulfillment integration boundaries.

State: **Planned**.

## 7. Expert CRM product areas

Tracked primarily by Phase 8 / issue #11. Each area must either become an explicit owner module, a set of owner modules, or an explicitly tracked cross-domain product capability.

- Sales expert expansion.
- Activities/productivity expert expansion.
- Communications and omnichannel interaction history.
- Marketing segmentation, campaigns, journeys and attribution.
- Support and service management.
- Field service.
- Customer success.
- Partner relationship management.
- Projects and configurable work management.
- Documents and e-signature.
- Analytics and performance management.
- Data operations/import/export/deduplication stewardship.
- Automation runtime and administration.
- Governed integration adapters.

State: **Planned except the existing Sales and Activities vertical slices and the Sales–Activities production link slice**.

## 8. Platform capabilities that are not business modules

These are major product/platform workstreams but should not inflate the business-module count:

- Module SDK and registry.
- Capability execution gateway.
- Query gateway.
- PostgreSQL authoritative data runtime.
- Event delivery runtime and rebuildable projection runtime.
- Production application composition root and process host.
- Search and generalized indexes.
- Admin Studio metadata publication.
- Product shell/design system.
- AI actor/tool layer.
- Signed marketplace/WASM sandbox.
- Enterprise security and operational proof.

## 9. Module creation checklist

Before introducing a new module:

1. Prove it has a distinct authoritative ownership boundary or optional cross-domain coordination role.
2. Define why the behavior cannot remain inside an existing owner domain or platform runtime.
3. Define immutable module identity and lifecycle.
4. Define provided/consumed versioned contracts.
5. Define storage ownership and uninstall/retention semantics.
6. Define permissions and data classes.
7. Define failure, retry and idempotency behavior.
8. Define independent disabled/uninstalled behavior where applicable.
9. Add architecture and acceptance gates.
10. Update this catalog and the roadmap.

Do not create a module solely because a directory, screen, table or team exists.

## 10. Target scale

The final universal CRM will contain substantially more than the current three implemented business modules. The roadmap already implies **more than twenty owner/link bounded contexts or major independently governed domain areas**, but the final count is intentionally driven by authoritative ownership rather than an arbitrary module target.

The exact count becomes authoritative only as domains receive stable published module identities.
