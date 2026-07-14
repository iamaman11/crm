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

A **link module** is also a business module, but it owns optional cross-domain coordination rather than source or target aggregates.

The following do **not** count as business modules:

- platform/runtime/composition crates under `crates/`;
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
| `crm.parties` | Canonical person and organization identity | **Expert expansion** | Production Party create/update/get/list plus permission-aware rebuildable global search discovery | Structured person/organization profiles, provenance, identity resolution, merge/unmerge, import/export and privacy lifecycle work across later Phase 8A packets |
| `crm.customer-accounts` | Canonical customer/commercial relationship | **Vertical slice** | Governed Account create/update/get/list with typed Party associations, live reference integrity, signed cursor listing and real PostgreSQL/process acceptance | Account hierarchy/advanced commercial semantics where justified, plus Customer 360 composition in later 8A.3 packets |
| `crm.contact-points` | Canonical email/phone/postal/web/messaging endpoints | **Vertical slice** | Merged governed create/update/verify/get/list with deterministic endpoint normalization, verification/preference lifecycle, Party-reference integrity and real PostgreSQL/process acceptance | Broader communication-channel product UX and consent-aware downstream usage remain separate later packets |
| `crm.party-relationships` | Authoritative typed Party-to-Party relationships and temporal hierarchy source state | **Vertical slice** | Merged create/update/get/list lifecycle with directional/reciprocal semantics, same-tenant Party-reference integrity, signed cursor queries, durable evidence and rebuildable hierarchy acceptance from #108 / PR #109 | Later explicitly governed overlap-policy expansion where justified |
| `crm.consents` | Authoritative purpose/channel-scoped Consent and Communication Authorization assertions | **Vertical slice** | Merged immutable grant/deny assertions, governed grant withdrawal, exact authoritative allow/deny decisions, cross-owner Party/Contact Point integrity, authoritative Party access path, signed get/list queries and real PostgreSQL/process acceptance from #112 / PR #113 | Later privacy orchestration and downstream campaign/provider enforcement remain separate governed packets |
| `crm.identity-resolution` | Authoritative duplicate-candidate cases, explainable evidence provenance and reviewer decisions | **Vertical slice — gate review** | Canonical unordered Party-pair cases, deterministic IDs, immutable evidence/version provenance, exact Party-version integrity, authoritative Party access paths, governed register/refresh/dismiss/confirm-duplicate, signed permission-aware get/list-by-Party queries and real PostgreSQL/process acceptance in #114 / PR #115 | Party merge/unmerge, reference redirection, immutable merge lineage and survivorship remain Phase 8A.6 |

Current owner-module count: **8**.

Production owner vertical slices are **Sales, Activities, Parties, Customer Accounts, Contact Points, Party Relationships and Consents**. `crm.identity-resolution` has a production-complete duplicate-candidate vertical slice in final gate review; its later Party merge/unmerge capability is intentionally not part of Phase 8A.5.

Current count of product-complete expert modules: **0**.

## 4. Implemented link module

| Module | Type | State | Implemented production slice |
|---|---|---|---|
| `crm.sales-activities-link` | Link module | **Production integration slice — Complete** | Consumes versioned Sales stage-change events through restart-safe governed delivery, checks tenant installation lifecycle, invokes Activities only through the governed capability gateway, uses durable retry/dead-letter delivery state and target idempotency, and remains independently suspendable/uninstallable |

The published `module_id` is fixed as **`crm.sales-activities-link`** and is treated as immutable module identity.

Current business-module count: **9** — eight owner modules plus one optional link module. The independently governed `crm.customer360` read-composition module remains tracked separately from this owner/link count.

## 5. Mandatory customer-master owner domains

Tracked by Phase 8A / issue #28.

- `crm.parties` — Party owner for person and organization identity — **Expert expansion**; create/update/get/list/search production lifecycle complete.
- `crm.customer-accounts` — Account owner for customer/commercial relationships — **Merged production vertical slice** from #101 / PR #102.
- `crm.contact-points` — Contact Point owner for email, phone, postal, web and messaging endpoints — **Merged production vertical slice** from #103 / PR #104.
- `crm.party-relationships` — Party Relationship owner for employment, household, parent/subsidiary and bounded configurable typed relationships — **Merged production vertical slice** from #108 / PR #109.
- `crm.consents` — authoritative purpose/channel/legal-basis/jurisdiction/source/evidence assertions and exact communication authorization decisions — **Merged production vertical slice from #112 / PR #113**.
- `crm.identity-resolution` — authoritative duplicate-candidate cases, immutable explainable evidence/version provenance and reviewer decisions — **Implemented production vertical slice in #114 / PR #115; final exact-head merge gate pending**.
- Party merge/unmerge, reference redirection, immutable merge lineage and survivorship — **Planned for Phase 8A.6**.

The shared `crm.customer.v1` Protobuf package contains only cross-owner references and shared public version metadata. It is **not** a business module and owns no mutable behavior or storage.

`crm-global-search-composition` is also **not** a business module. It owns only cross-domain rebuildable projection composition; Party remains the identity owner and search remains non-authoritative.

`crm.customer360` is an independently lifecycle-managed **read-composition module**, not an authoritative owner module. It publishes `customer360.customer.get@1.0.0`, consumes immutable owner events and materializes rebuildable contribution documents, but declares no owned objects, record types, private authoritative state or mutation capability. It is tracked separately from the seven authoritative owner modules and the optional link-module count.

### Account ownership boundary

`crm.customer-accounts` owns:

- Account identity and lifecycle;
- Account name/status;
- typed Party association roles;
- Account optimistic version progression.

It does **not** own:

- mutable Party identity attributes;
- Contact Point lifecycle;
- Party Relationship hierarchy;
- consent or communication authorization;
- Customer 360 projections.

Party-reference existence and tenant integrity are validated in application/platform composition before Account mutation execution, preserving a pure owner-domain aggregate with no direct Party storage access.

### Contact Point ownership boundary

`crm.contact-points` owns:

- Contact Point identity and Party attachment by stable reference;
- typed endpoint kind and canonical endpoint value;
- active/inactive lifecycle;
- validity interval state;
- preferred-contact-point state;
- verification state and bounded verification evidence reference;
- Contact Point optimistic version progression.

It must **not** own:

- mutable Party identity attributes;
- consent or communication authorization;
- provider delivery state or omnichannel conversation state;
- Account hierarchy or Party Relationship state.

Merged PR #104 delivers the authoritative 8A.3b Contact Point vertical slice: strict deterministic persistence, additive v1 contracts/events, governed mutation and permission-aware query adapters, application-level Party-reference integrity, runtime composition, synchronized descriptors and fresh-PostgreSQL real `crm-api` process acceptance. The owner module remains pure: it has no SQL, transport types or direct cross-owner storage access. PR #104 merged to `main` as `00f41b4bf2bf11dc4a5bb62d9cc1b46c6ad88fd8`.

### Party Relationship ownership boundary

`crm.party-relationships` owns stable Party Relationship identity, immutable canonical Party endpoints, bounded typed directional/reciprocal semantics, lifecycle, validity intervals and optimistic version progression. It does **not** own Party identity attributes, Account membership, Contact Point state, consent/communication authorization, Sales roles, provider state or Customer 360. Application composition validates both Party references without giving the owner SQL or direct Party storage access. The hierarchy read model is rebuildable and non-authoritative. #108 / PR #109 merged to `main` as `36c238d51a156e3864e2dad0f53762e95e47680d`.

### Customer 360 read-composition boundary

`crm.customer360` owns only the versioned read-composition contract, deterministic mapping from validated owner events into rebuildable contributions and the permission-aware query assembly policy. It does **not** own Party identity, Account membership, Contact Point lifecycle/verification, Party Relationship state, consent, identity resolution, provider delivery state or any mutable customer master. Source resources are live-authorized and field-redacted before disclosure, and projection rebuild cannot mutate authoritative records, outbox events or audit evidence. #110 / PR #111 completed the exact-head gate and merged to `main` as `30ce84c57064134202c03c07a943bcd0859e1ea9`.

### Consent and Communication Authorization ownership boundary

`crm.consents` owns immutable Consent Authorization identity and scope, Party/optional Contact Point references, exact purpose/channel, grant-or-deny effect, legal-basis/jurisdiction/source/evidence codes, effective/expiry windows, irreversible grant withdrawal and the deterministic current communication authorization decision. It does **not** own Party attributes, Contact Point endpoint/verification/preference state, provider delivery state, campaigns, journeys, Customer 360 mutation or identity resolution. The pure owner crate has no SQL or cross-owner storage access. Application composition validates same-tenant Party and Contact Point ownership before mutation, while each create transaction atomically writes one authoritative `consents.authorization.party` relationship so `consents.communication.authorize` can read only that Party's authoritative Consent records without relying on a rebuildable projection or tenant-wide scan. #112 / PR #113 completed the unchanged exact-head gate and merged to `main` as `381a9fd5e6eb54918fc43801062957ca4a854486`.

### Identity Resolution ownership boundary

`crm.identity-resolution` owns canonical unordered duplicate-candidate Party pairs, deterministic candidate-case identity, immutable bounded evidence snapshots with exact authoritative Party source versions, matcher/signal provenance and terminal reviewer decisions. It does **not** own mutable Party attributes and Phase 8A.5 never merges, deletes, aliases or redirects Party records. Application composition validates same-tenant Party existence and exact current versions before registration or evidence refresh, and refuses terminal reviewer decisions when the current evidence snapshot is stale. Registration atomically creates authoritative Party-to-case relationships used by permission-aware get/list-by-Party queries without tenant-wide scans or rebuildable projections. #114 / PR #115 is in final exact-head gate review; merge/unmerge remains Phase 8A.6.

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

Tracked primarily by Phase 8 / issue #11. Each area must either become an explicit owner module, a set of owner modules, or an explicitly tracked cross-domain product capability. The normative completeness guardrail is `CRM_CAPABILITY_COVERAGE.md`.

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

State: **In progress through Phase 8A customer-master delivery; other areas remain planned except the existing Sales and Activities vertical slices and Sales–Activities production link slice**.

## 8. Platform capabilities that are not business modules

These are major product/platform workstreams but should not inflate the business-module count:

- Module SDK and registry.
- Capability execution gateway.
- Query gateway.
- PostgreSQL authoritative data runtime.
- Event delivery runtime and rebuildable projection runtime.
- Production application composition root and process host.
- Permission-aware search runtime and neutral cross-domain search composition.
- Admin Studio metadata publication.
- Product shell/design system and typed UI-extension runtime.
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

The final universal CRM will contain substantially more than the current nine business modules. The roadmap and `CRM_CAPABILITY_COVERAGE.md` already imply **more than twenty owner/link bounded contexts or major independently governed domain areas**, but the final count is intentionally driven by authoritative ownership rather than an arbitrary module target.

The exact count becomes authoritative only as domains receive stable published module identities.
