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
| `crm.customer-privacy` | Privacy case and owner-orchestration coordinator | **Expert expansion** | Case lifecycle, approval, permission-aware get/list/plan/outcome reads, trusted-internal exact-nine discovery/immutable snapshots and deterministic planning, public deny-only restriction/legal-hold placement, retention adjudication, durable owner execution/outcomes and governed access/export assembly | Restriction/legal-hold release and reads where required, owner-specific deletion/anonymization/crypto-shred, tombstone/convergence, workers, frontend and operations proof |

Current merged authoritative/coordination module count: **12**.

## 3. Link and read-composition modules

`crm.sales-activities-link` is the accepted optional production integration link. `crm.customer360` is a lifecycle-managed read-composition module and owns no mutable customer-master values.

Current merged business-module total: **13** — twelve authoritative/coordination modules plus one optional link module.

## 4. Customer Privacy boundary

Phase 8A.11 / issue #126 remains **In progress**.

Latest accepted public inventory is **seven mutations, four permission-aware public queries and zero Customer Privacy workers through PR #241**. Trusted-internal `customer_privacy.plan.build@1.0.0`, `customer_privacy.retention.evaluate@1.0.0`, repository-step-8 owner execution and `customer_privacy.access_export.request@1.0.0` have no public ingress.

Scope discovery and immutable snapshot execution remain accepted through PR #206 and are the authoritative foundation for the later planning, owner-execution and access/export evidence.

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
- PR #222 / source `b5651e784a156758b39eaa04abc1124c7c0832f9` / merge `fd86ab1408e435ccc9f47b7a86ab3dd66df64ec1` / 16 of 16 — first bounded contribution aggregation;
- PR #224 / source `e57307fcb1b5192d5e6340247cb6633f32b7ba34` / merge `67804d9478b2bbaf342a398b649e23bd5ead6c08` / 28 of 28 — final customer-subject policy prerequisite;
- PR #226 / source `ad08a691ec759b8b3b523fa66a034cecf4138ff0` / merge `a46460623e90c5649d36bedba055fb55023d9349` / 34 of 34 — public `customer_privacy.restriction.place@1.0.0` and first protected-owner enforcement;
- PR #230 — public legal hold and mandatory-retention precedence;
- PR #235 — reusable mutation/query conformance;
- PR #237 — durable replay-safe owner execution, checkpoints and real owner outcomes;
- PR #239 — multi-plane affected-scope enforcement;
- PR #241 / accepted source `2bb3a671deb18a6ae3bcea228ed01ed287b9de6a` / merge `19232f6f3e2ae87aabeb080257c1aac5477a6616` / 34 of 34 — governed access/export assembly with Customer Data Operations artifact ownership and crash recovery.

Repository step 11 — owner-specific deletion, anonymization and supported crypto-shred execution — is the only next implementation packet. Access/export assembly is accepted and must not be described as incomplete.

## 5. Phase 8A packet accounting

Completed:

- 8A.1–8A.6 — customer references, Party, Account, Contact Point, Party Relationship, Customer 360, Consent and reversible Identity Resolution;
- 8A.7 — Customer Import;
- 8A.8 — Customer Export;
- 8A.9 — Customer Data Quality;
- 8A.10 — Governed Customer Enrichment and Provenance.

In progress:

- 8A.11 / #126 — Customer Privacy. Discovery, immutable snapshots, deterministic planning, permission-aware reads, approval, restriction placement/final enforcement, legal-hold placement, mandatory-retention adjudication, durable owner execution/outcomes and governed access/export assembly are accepted. Remaining product work includes restriction and legal-hold release/read lifecycle where required, deletion/anonymization/supported crypto-shred, Party tombstone/no-orphan and convergence, Customer Privacy worker lifecycle, frontend/accessibility/browser proof and production operations evidence.

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
