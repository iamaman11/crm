# Ultimate CRM — Module Catalog

Status: **Normative business-module ownership and readiness catalog**

Delivery governance: `DELIVERY_GOVERNANCE.md`  
Roadmap: `IMPLEMENTATION_ROADMAP.md`  
Functional completeness guardrail: `CRM_CAPABILITY_COVERAGE.md`

This catalog tracks business-domain ownership and readiness without counting technical crates, services, projections or contracts as product modules.

## 1. Counting and readiness

A business module is an independently governed runtime unit under `modules/` with stable identity, explicit ownership or coordination role, versioned contracts and no direct cross-module storage bypass.

Readiness states are Planned, Foundation, Vertical slice, Production integration slice, Expert expansion, Gate review and Product complete. Only merged `main` affects totals.

Architecture and governance steps 12–25 may reduce extension cost or prove platform quality, but they do not by themselves advance a module readiness state. Readiness changes only when separate product evidence proves additional domain breadth, governed runtime behavior, UX and operations acceptance.

## 2. Implemented authoritative owner and coordination modules

| Module | Ownership | Current merged readiness | Implemented production slice | Still required |
|---|---|---|---|---|
| `crm.sales` | Sales owner domain | **Vertical slice** | Deal create/update/stage/get/list | Leads, richer pipelines, territories, quotas and forecasting |
| `crm.activities` | Activities/productivity owner | **Vertical slice** | Task create/update/complete/reminder/get/list | Appointments, recurring work and calendar synchronization |
| `crm.parties` | Canonical person/organization identity | **Expert expansion** | Party create/update/get/list/search | Structured profile depth and source identifiers |
| `crm.customer-accounts` | Customer/commercial relationship | **Vertical slice** | Account create/update/get/list with Party associations | Advanced hierarchy and product UX |
| `crm.contact-points` | Canonical communication endpoints | **Vertical slice** | Create/update/verify/get/list; create is protected by the final Customer Privacy restriction guard | Broader channel UX and omnichannel use |
| `crm.party-relationships` | Typed temporal Party relationships | **Vertical slice** | Create/update/get/list and hierarchy foundation | Additional relationship semantics |
| `crm.consents` | Purpose/channel authorization | **Vertical slice** | Immutable assertions, withdrawal and exact decisions | Wider privacy enforcement |
| `crm.identity-resolution` | Duplicate cases, merge lineage and canonical resolution | **Expert expansion** | Candidate/review plus reversible merge/unmerge | Broader survivorship and privacy orchestration |
| `crm.customer-data-operations` | Governed import/export coordination | **Expert expansion** | Resumable import, deterministic export and recovery | More profiles and privacy execution integration |
| `crm.data-quality` | Customer-data quality coordinator | **Vertical slice** | Evaluation, findings/completeness, stewardship and remediation | Additional owner-resource profiles |
| `crm.customer-enrichment` | Provider-neutral enrichment coordinator | **Production integration slice** | Provider boundary, provenance, review and deterministic owner application | Additional providers, fields, UX and privacy interaction |
| `crm.customer-privacy` | Privacy case and owner-orchestration coordinator | **Expert expansion** | Case lifecycle, approval, permission-aware get/list/plan/outcome reads, exact-nine discovery and immutable snapshots, deterministic planning, restriction/legal-hold placement, retention precedence, durable owner execution/outcomes, governed access/export assembly and authoritative owner-specific anonymization/deletion execution | Restriction/legal-hold release and reads where required, Party tombstone/convergence, workers, frontend and operations proof |

Current merged authoritative/coordination module count: **12**.

## 3. Link and read-composition modules

`crm.sales-activities-link` is the accepted optional production integration link. `crm.customer360` is a lifecycle-managed read-composition module and owns no mutable customer-master values.

Current merged business-module total: **13** — twelve authoritative/coordination modules plus one optional link module.

## 4. Customer Privacy boundary

Phase 8A.11 / issue #126 remains **In progress**.

Latest accepted public inventory is **seven mutations, four permission-aware public queries and zero Customer Privacy workers through PR #244**. Public placement coordinates include `customer_privacy.restriction.place@1.0.0` and `customer_privacy.legal_hold.place@1.0.0`. Trusted-internal `customer_privacy.plan.build@1.0.0`, `customer_privacy.retention.evaluate@1.0.0`, replay-safe owner execution, `customer_privacy.access_export.request@1.0.0` and authoritative exact-nine owner-action execution have no public ingress.

Scope discovery and immutable snapshot execution remain accepted through PR #206 and are the authoritative foundation for planning, owner execution and access/export evidence. At the historical PR #206 boundary, planning and action execution remain not started; PR #209 and later accepted packets subsequently implemented those capabilities without rewriting the historical claim. This catalog records the latest merged aggregate state below.

Nine owner-scope contribution coordinates are published as non-public owner-owned reads. **All nine authoritative implementations are accepted:**

- PR #156 — Parties;
- PR #175 — Consents;
- PR #179 — Customer Accounts;
- PR #181 — Contact Points;
- PR #183 — Party Relationships;
- PR #186 — Identity Resolution / accepted source `24456b86379a1ef23ed5a60804cdcae5075d407c` / merge `509eb304a76055c9f49b0beed3b007963a91cb22` / 25 of 25 permanent workflows;
- PR #188 — Customer Data Operations / accepted source `07f34786e82fdfa78d263790e9f50541529006f8` / merge `089be72fa3010b4aa15aff7f9ea55fd86290f8fc` / 26 of 26 permanent workflows;
- PR #190 — Data Quality / accepted source `dcfe8faebc7462b888f8fc1721cb379a40fea88a` / merge `deac197c97cddc15bb9916092ca87f6e767ce1de` / 27 of 27 permanent workflows;
- PR #192 — Customer Enrichment / accepted source `e90e36027de18a07be68e43327ea732810ff332a` / merge `e41cbab0cd30819fcbe2e3c5f2c7415fc6de3e8c` / 28 of 28 permanent workflows.

Accepted Customer Privacy architecture/runtime evidence now includes:

- PR #206 — exact-nine discovery and immutable snapshots;
- PR #209 — deterministic planning runtime;
- PR #211 — permission-aware plan/outcome reads;
- PR #220 — public approval;
- PR #222 / accepted source `b5651e784a156758b39eaa04abc1124c7c0832f9` / merge `fd86ab1408e435ccc9f47b7a86ab3dd66df64ec1` / 16 of 16 permanent workflows — first bounded contribution aggregation;
- PR #224 / accepted source `e57307fcb1b5192d5e6340247cb6633f32b7ba34` / merge `67804d9478b2bbaf342a398b649e23bd5ead6c08` / 28 of 28 permanent workflows — final customer-subject policy prerequisite;
- PR #226 / accepted source `ad08a691ec759b8b3b523fa66a034cecf4138ff0` / merge `a46460623e90c5649d36bedba055fb55023d9349` / 34 of 34 permanent workflows — public `customer_privacy.restriction.place@1.0.0` and first protected-owner enforcement;
- PR #230 — public legal hold and mandatory-retention precedence;
- PR #235 — reusable mutation/query conformance;
- PR #237 — durable replay-safe owner execution, checkpoints and real owner outcomes;
- PR #239 — multi-plane affected-scope enforcement;
- PR #241 / accepted source `2bb3a671deb18a6ae3bcea228ed01ed287b9de6a` / merge `19232f6f3e2ae87aabeb080257c1aac5477a6616` / 34 of 34 permanent workflows — governed access/export assembly with Customer Data Operations artifact ownership and crash recovery;
- PR #244 / accepted source `405d2dbb97bb371b51cfb1d4ffb5549a57262878` / merge `4b08202fe9dd0c0df83567e24e6b9d86fb79c9db` / 34 of 34 permanent workflows — authoritative owner-specific anonymization and supported deletion execution with immutable lineage and fail-closed unsupported actions;
- PR #246 / accepted source `3b4fe7cdf458daac9c12f816d0d6a87039e613f3` / merge `f090fa8785ac2bbb7e5cf186b4c5011cb9aeb978` / 37 of 37 permanent workflows;
- PR #248 / accepted source `b15482361ab2b322591d488843ab9b46ff676dba` / merge `b4222364c21cb74127834f5ff4f0739343d26379` / 37 of 37 permanent workflows;
- PR #249 / accepted source `7876945586e5a6cc94f8d3b0f6ba2b57316484d2` / merge `f36592211bed3e0df7cf3771164b4bc24026eff3` / 37 of 37 permanent workflows — complete first-party production contribution aggregation through `crm-first-party-modules` without behavior changes.

Repository steps 1–12 and Stage D are complete. PR #253 / accepted source `475533b185b871418273c1c1e3f63a1d62542677` / squash merge `7dcda204be07209d9e4996fdc9c5fd364cea179e` / 7 of 7 applicable permanent workflows on one unchanged exact head. The accepted exact current-main baseline contains 113 workspace packages, 841 internal dependency edges, maximum dependency depth 18, maximum direct dependents 105, maximum transitive reverse impact 106, a conservative public Rust surface of 5,377 items, 40 permanent workflows, 41 jobs, 1,712 path-filter entries, 31 PostgreSQL workflows and 94 equivalent suppression entries (3 direct lint tables, 87 source-level `allow` attributes, 0 `expect` attributes and 4 ignored foundation tests). Repository step 13 remains in progress. The next permitted implementation packet registers the accepted suppression baseline, mechanically blocks every new unregistered equivalent bypass while allowing reductions, removes the three direct lint-table exceptions without hidden replacements and calibrates role-aware dependency, public-surface, central-LOC, reverse-impact and change-cost budgets. Repository step 14 remains blocked. This architecture work does not advance Customer Privacy product readiness.

## 5. Phase 8A packet accounting

Completed:

- 8A.1–8A.6 — customer references, Party, Account, Contact Point, Party Relationship, Customer 360, Consent and reversible Identity Resolution;
- 8A.7 — Customer Import;
- 8A.8 — Customer Export;
- 8A.9 — Customer Data Quality;
- 8A.10 — Governed Customer Enrichment and Provenance.

In progress:

- 8A.11 / #126 — Customer Privacy. Discovery, immutable snapshots, deterministic planning, permission-aware reads, approval, restriction placement/final enforcement, legal-hold placement, mandatory-retention adjudication, durable owner execution/outcomes, governed access/export assembly and authoritative owner-specific anonymization/deletion are accepted. Remaining product work includes restriction and legal-hold release/read lifecycle where required, Party tombstone/no-orphan and projection/search/cache convergence, Customer Privacy worker lifecycle, disable/uninstall fail-closed semantics, frontend/accessibility/browser proof and production operations evidence.

Workspace packages remain 113. Current product-complete expert modules remain **0**.

## 6. Customer-master ownership baseline

- `crm.parties` owns canonical Party identity and lifecycle.
- `crm.customer-accounts` owns commercial Account identity/lifecycle and Party associations.
- `crm.contact-points` owns endpoint identity/value/lifecycle/verification.
- `crm.party-relationships` owns stable temporal Party relationships.
- `crm.consents` owns authorization assertions, withdrawal and decisions.
- `crm.identity-resolution` owns merge lineage, redirects and survivorship provenance.
- Customer Data Operations, Data Quality, Customer Enrichment and Customer Privacy own coordination/evidence, not customer-master values.
- `crm.customer360` owns only rebuildable read composition.

## 7. Planned commercial and expert domains

Phase 8B / issue #29 remains planned and blocked on completed Phase 8A plus the repository-step-22 measurement checkpoint. Repository step 23 is the first later expert-domain wave. Product Catalog, Pricing, CPQ, Quotes, Orders, Contracts, Subscriptions/Entitlements/Usage and governed billing/ERP/payment/tax/fulfillment remain independent owner domains.

Broader Sales/Activities, omnichannel, Service/Knowledge/Field Service, Marketing, Customer Success, projects/configurable work, documents/e-signature, analytics, workflow/collaboration, AI governance, marketplace and enterprise operational proof remain incomplete or planned.

## 8. Completion accounting

Current product-complete expert modules: **0**.

A module is not product-complete merely because a crate, schema, manifest or backend path exists. Product complete requires domain breadth, governed APIs, persistence, authorization, audit, product UX and production/operational evidence.
