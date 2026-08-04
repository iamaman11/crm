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
| `crm.parties` | Canonical person/organization identity | **Expert expansion** | Party create/update/get/list/search plus accepted privacy tombstone convergence | Structured profile depth and source identifiers |
| `crm.customer-accounts` | Customer/commercial relationship | **Vertical slice** | Account create/update/get/list with Party associations | Advanced hierarchy and product UX |
| `crm.contact-points` | Canonical communication endpoints | **Vertical slice** | Create/update/verify/get/list; create is protected by the final Customer Privacy restriction guard | Broader channel UX and omnichannel use |
| `crm.party-relationships` | Typed temporal Party relationships | **Vertical slice** | Create/update/get/list and hierarchy foundation | Additional relationship semantics |
| `crm.consents` | Purpose/channel authorization | **Vertical slice** | Immutable assertions, withdrawal and exact decisions | Wider privacy enforcement |
| `crm.identity-resolution` | Duplicate cases, merge lineage and canonical resolution | **Expert expansion** | Candidate/review plus reversible merge/unmerge | Broader survivorship and privacy orchestration |
| `crm.customer-data-operations` | Governed import/export coordination | **Expert expansion** | Resumable import, deterministic export and recovery | More profiles and privacy execution integration |
| `crm.data-quality` | Customer-data quality coordinator | **Vertical slice** | Evaluation, findings/completeness, stewardship and remediation | Additional owner-resource profiles |
| `crm.customer-enrichment` | Provider-neutral enrichment coordinator | **Production integration slice** | Provider boundary, provenance, review and deterministic owner application | Additional providers, fields, UX and privacy interaction |
| `crm.customer-privacy` | Privacy case and owner-orchestration coordinator | **Expert expansion** | Case lifecycle, approval, permission-aware get/list/plan/outcome reads, exact-nine discovery and immutable snapshots, deterministic planning, restriction/legal-hold placement, retention precedence, durable owner execution/outcomes, governed access/export assembly, authoritative owner-specific anonymization/deletion and Party tombstone/no-orphan convergence | Restriction/legal-hold release and reads where required, frontend, accessibility, browser and operations proof |

Current merged authoritative/coordination module count: **12**.

## 3. Link and read-composition modules

`crm.sales-activities-link` is the accepted optional production integration link. `crm.customer360` is a lifecycle-managed read-composition module and owns no mutable customer-master values.

Current merged business-module total: **13** — twelve authoritative/coordination modules plus one optional link module.

## 4. Customer Privacy boundary

Phase 8A.11 / issue #126 remains **In progress**.

Latest accepted public inventory remains **seven mutations and four permission-aware public queries**. The accepted first-party background-worker inventory is now **one Customer Privacy owner worker**: `crm.customer-privacy` / `owner-execution` at phase `260`. Public placement coordinates include `customer_privacy.restriction.place@1.0.0` and `customer_privacy.legal_hold.place@1.0.0`. Trusted-internal `customer_privacy.plan.build@1.0.0`, `customer_privacy.retention.evaluate@1.0.0`, replay-safe owner execution, `customer_privacy.access_export.request@1.0.0` and authoritative exact-nine owner-action execution have no public ingress.

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
- PR #269 / source `74b1d7b0f8764fcd90839b7aab25f8f82fe5e552` / merge `6f82de0a7b2dcd1ab5dd0ae6473d46d7d9d34bdd` / 20 of 20 — reusable worker conformance and representative Customer Enrichment / CRM API import adoption;
- PR #270 / source `8e2baac0822eefbb6d3c474ffce0cee69e3e4e98` / merge `ce0ca881461d1ee8964a11b28c1fcff46cf145cb` / 17 of 17 — two live production import executors serialize and converge to exactly one durable Party effect;
- PR #237 — durable replay-safe owner execution, checkpoints and real owner outcomes;
- PR #239 — multi-plane affected-scope enforcement;
- PR #241 / accepted source `2bb3a671deb18a6ae3bcea228ed01ed287b9de6a` / merge `19232f6f3e2ae87aabeb080257c1aac5477a6616` / 34 of 34 permanent workflows — governed access/export assembly with Customer Data Operations artifact ownership and crash recovery;
- PR #244 / accepted source `405d2dbb97bb371b51cfb1d4ffb5549a57262878` / merge `4b08202fe9dd0c0df83567e24e6b9d86fb79c9db` / 34 of 34 permanent workflows — authoritative owner-specific anonymization and supported deletion execution with immutable lineage and fail-closed unsupported actions;
- PR #246 / accepted source `3b4fe7cdf458daac9c12f816d0d6a87039e613f3` / merge `f090fa8785ac2bbb7e5cf186b4c5011cb9aeb978` / 37 of 37 permanent workflows;
- PR #248 / accepted source `b15482361ab2b322591d488843ab9b46ff676dba` / merge `b4222364c21cb74127834f5ff4f0739343d26379` / 37 of 37 permanent workflows;
- PR #249 / accepted source `7876945586e5a6cc94f8d3b0f6ba2b57316484d2` / merge `f36592211bed3e0df7cf3771164b4bc24026eff3` / 37 of 37 permanent workflows — complete first-party production contribution aggregation through `crm-first-party-modules` without behavior changes;
- PR #281 / accepted source `76a0d93d594b6ffbb890f90d3cb9037febf4c3f8` / merge `87bc0d33befc4525b62aa7e0e1884abc07e12abf` / 7 of 7 permanent workflows — deterministic repository-pinned doctor and locked isolated bootstrap, with no module readiness or product behavior change.
- PR #283 / accepted source `5f4480fafd37cc8c89df60f3688e756d7f881af8` / merge `21e2f73b57d2c35c16eccc15ee3e075e818f488a` / 7 of 7 permanent workflows — checkout-owned PostgreSQL dev-up/dev-reset, immutable image and schema digest, fail-closed ownership/reset semantics and real-Docker create/reuse/reset acceptance, with no module readiness or product behavior change.
- PR #285 / accepted source `a522b8b11a0c6f143694f516e7a7f9d522c18ce3` / merge `a906f2c514285974113749b8b8ad9446202a5fa1` / 19 of 19 permanent workflows — versioned idempotent local-demo-acme seeding through the governed Party gateway and real crm-api smoke proof for readiness, permission grants, authentication denial and tenant non-disclosure, with no module-readiness advancement.

Repository Step 13 architecture governance is accepted through PRs #253, #255 and #257. Repository Step 14 and architecture Stage G are accepted through PR #259 / source `8aa0b33c6609e74f98363071c6e7c44ec59fc098` / merge `2b0b558077c444d4469137c8a2bcca2c14ae426` / 36 of 36.

Repository Step 15 is accepted through:

- PR #263 / source `6c2a54f6780988a12fec3cd77ca2cd39ad349140` / merge `bd205e0af77b676654dff8ddf26d3b5b195880b2` / 32 of 32 — Search convergence;
- PR #264 / source `e6c9d2901109c8d5b9e0f3cf783214407e26451a` / merge `e9fe1f352386d80a29d122db5d1ed6c47266bfaf` / 6 of 6 — Customer 360 tombstone convergence;
- PR #265 / source `ef572bdf31c584c397c215cd1b62ee47cad54e64` / merge `2a0ee9c33fbe23cdf9c6dccb1acc7e3bd8bcba3a` / 19 of 19 — canonical execution and rebuild/Search replay;
- PR #266 / source `ded5d80ae11bbf044b5bfe5b572e8dab521f884a` / merge `1f889a810c82da3d0fee12427eacccbe43613bac` / 19 of 19 — automatic Customer 360 v2 generation rollover;
- PR #267 / source `f1b72dbee09f152005cb3584b9bcc1573bf2c4fe` / merge `4a14a5a0bda6d25d27e7c2da7c5f4809fa1efbdf` / 19 of 19 — real `crm-api` process-host no-orphan repair on clean and rollback/reapplied schemas.

Repository Step 17 is accepted through:

- PR #275 / source `1c6366b557e255a14a677758fd87f7fe63184a89` / merge `60d5974349a04c462475aadd4af0a37bada9713b` / 23 of 23 — published wire-compatible `activities.task.create@1.1.0`, migrated the sole repository-owned consumer and made live authorization overlap exact-version safe;
- PR #278 / source `228f459963e571a830aee6c43cddd15e3c9b0d5f` / merge `996d634a33945e618be5ff81c297f0f617ce19d5` / 8 of 8 — made production zero-usage retirement evidence SHA-256-bound, complete, append-only and fail closed;
- PR #279 / source `0dce0895edec56508df1b4fc880d09ab27fc00df` / merge `ea3d3894d05e3a8d814aff69824b593843763d03` / 22 of 22 — proved the coordinate was never externally released through live empty GitHub Releases, tags and deployments, non-publishable packages and no external consumer record, then retired `activities.task.create@1.0.0` while preserving its historical tombstone and keeping `telemetry.zero_since` null.

The accepted Step 17 result keeps `activities.task.create@1.1.0` as the sole live create coordinate. `activities.task.create@1.0.0` is absent from the current provider manifest and production capability catalog, rejected before payload decoding, retained only as immutable historical registry/lifecycle evidence and classified `never_externally_released`. The ordinary 30-day/production-zero-usage path remains binding for released contracts; no production history was fabricated. Repository Step 18 is complete through accepted PRs #281, #283 and #285: PR #281 / source `76a0d93d594b6ffbb890f90d3cb9037febf4c3f8` / squash merge `87bc0d33befc4525b62aa7e0e1884abc07e12abf` / 7 of 7; PR #283 / source `5f4480fafd37cc8c89df60f3688e756d7f881af8` / squash merge `21e2f73b57d2c35c16eccc15ee3e075e818f488a` / 7 of 7; PR #285 / source `a522b8b11a0c6f143694f516e7a7f9d522c18ce3` / squash merge `a906f2c514285974113749b8b8ad9446202a5fa1` / 19 of 19 applicable permanent workflows, each on one unchanged exact head. The accepted lifecycle delivers deterministic repository-pinned `doctor`, locked isolated `bootstrap`, checkout-owned PostgreSQL `dev-up` / `dev-reset`, versioned idempotent `seed-demo` through the governed Party mutation gateway and real-process `smoke` proving readiness, permission, authentication and tenant boundaries. Repository Step 20A is accepted through PR #292. Repository Step 20 is complete; Repository Step 21 Phase 8A closure is the only next permitted implementation packet.

The accepted Step 15 result preserves the current measured repository at **112 workspace packages**, **835 internal dependency edges**, maximum dependency depth **18**, **5,377** conservative public Rust items, **270** dependency declarations and **91** suppression occurrences. Repository Step 16 is complete through PRs #269 and #270; reusable worker conformance, retry/restart recovery, tenant isolation and exactly-once contention convergence are accepted. Repository Steps 17 and 18 are complete through PRs #279 and #285 respectively; Repository Step 19 is complete through PRs #287–#290. Repository Step 20A is accepted through PR #292; Repository Step 20 is complete and Repository Step 21 Phase 8A closure is the only next permitted implementation packet. The accepted first-party inventory now includes one Customer Privacy owner worker; this evidence-sync changes no runtime behavior and does not advance Customer Privacy to Product complete.

## 5. Phase 8A packet accounting

Completed:

- 8A.1–8A.6 — customer references, Party, Account, Contact Point, Party Relationship, Customer 360, Consent and reversible Identity Resolution;
- 8A.7 — Customer Import;
- 8A.8 — Customer Export;
- 8A.9 — Customer Data Quality;
- 8A.10 — Governed Customer Enrichment and Provenance.

In progress:

- 8A.11 / #126 — Customer Privacy. Discovery, immutable snapshots, deterministic planning, permission-aware reads, approval, restriction placement/final enforcement, legal-hold placement, mandatory-retention adjudication, durable owner execution/outcomes, governed access/export assembly, authoritative owner-specific anonymization/deletion and Party tombstone/no-orphan convergence are accepted. Remaining product work includes restriction and legal-hold release/read lifecycle where required, frontend/accessibility/browser proof and production operations evidence.

Current workspace packages: **112**. Current product-complete expert modules: **0**.

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

## Repository Step 19 accepted closure

Repository Step 19 is complete only through the combined accepted evidence below, each on one unchanged exact source head with no unresolved comments, reviews or review threads:

- PR #287 / source `23b2f4ea660bcd46884fe054cd0c37e89b1495c4` / squash merge `c0fec3ae08c836ab483737442ed4377c99c85e9a` / **11 of 11** applicable permanent workflows — added the bounded Customer Privacy owner-worker boundary without public ingress or new schema/dependency surface;
- PR #288 / source `b99a4fc4ff7ef5cfd47813b487900b4c2a9f3b77` / squash merge `bc653de5f1a853791d3ab4a03f59f3daad54bf54` / **24 of 24** — added PostgreSQL ready-work discovery for planned Customer Privacy owner actions;
- PR #289 / source `3e21e79e1600727ebcda222af389d568d857cff8` / squash merge `d1c4dd278853a1e6a426fab284c70b3529d42833` / **24 of 24** — registered `crm.customer-privacy` / `owner-execution` at phase `260` in the production `ApplicationRuntime`, with activation gating and replay-safe canonical execution;
- PR #290 / source `9bbb339f39133955a7f42ea67f3334e597066e2e` / squash merge `49c5e35814adceb2be9d4cc2302bf10032b807a0` / **19 of 19** — proved the assembled real `crm-api` lifecycle on clean and rollback/reapplied PostgreSQL schemas: ready-work discovery, a real Parties privacy action, one durable attempt, successful outcome, completed checkpoint, audit evidence, owner event/outbox and final case transition, plus restart no-duplicate proof and uninstall no-discovery/no-effect proof.

The accepted first-party background-worker inventory is now **one Customer Privacy owner worker**: module `crm.customer-privacy`, worker `owner-execution`, phase `260`. This is a production background worker, not a new public capability route; the latest public Customer Privacy inventory remains **seven mutations and four permission-aware public queries**.

Repository Steps 1–20 are complete. The bounded Repository Step 20A product-plane slice is accepted. Repository Step 20 is complete; Repository Step 21 Phase 8A closure is the only next permitted implementation packet. Phase 8A.11 / issue #126 remains in progress; Customer Privacy is not product-complete; current product-complete expert modules remain zero; architecture 10/10 and the Universal CRM product are not declared complete.

The Step 19 packets add no crate, dependency, route, public API, module manifest, migration or schema. The conservative public Rust surface remains **5,377**, suppression occurrences remain **91**, and `crm-application-runtime` non-comment/source LOC remains within the frozen **7,269** ceiling.

## Accepted Repository Step 20A evidence

PR #292 / source `938cebed1e78bf7debf40dc544431bfe819970f4` / squash merge `fffd6baf35544eea736d183af0a5ba38518cce9a` / 17 of 17 applicable permanent workflows on one unchanged exact head accepts the bounded Customer Privacy product-plane slice.

The accepted evidence proves:

- exact typed `customer_privacy.case.list@1.0.0` and `customer_privacy.case.get@1.0.0` governed clients with envelope, contract, descriptor-hash, data-class, payload-size and retention checks before rendering;
- an authenticated capability-gated `/customer/privacy` route while backend authentication, tenant isolation, authorization and visibility remain authoritative;
- a bounded accessible case list/detail experience with explicit loading, empty, error and retry states, live announcements, deterministic focus behavior and permission/not-found concealment;
- a governed Party and verified PrivacyCase fixture created through assembled production composition and mutations, with no direct Customer Privacy record writes and no mock backend;
- real PostgreSQL, assembled `crm-api`, Vite and Chromium acceptance for keyboard-only list/detail review, session expiry and cross-tenant concealment;
- no backend route, capability, contract, manifest, schema, migration, dependency, lockfile or Rust production-source change.

Step 20A is accepted. Repository Step 20 is complete; Step 20B restore, SLO, observability, performance, security and supply-chain operations evidence is accepted through PR #294; Repository Step 21 Phase 8A closure is the only next permitted packet. Phase 8A.11, Phase 8A, product-complete expert modules, architecture 10/10 and the Universal CRM product remain incomplete. The accepted one-worker Customer Privacy inventory, seven public mutations, four permission-aware public queries, 5,377 public Rust items, 91 suppressions and the `crm-application-runtime` 7,269 LOC ceiling remain unchanged.

## Repository Step 20 accepted closure

Repository Step 20 is complete through PR #292 / source `938cebed1e78bf7debf40dc544431bfe819970f4` / squash merge `fffd6baf35544eea736d183af0a5ba38518cce9a` / 17 of 17 applicable permanent workflows and PR #294 / source `f9c5faa667f4d5483335ec2cb5bac31596d818c8` / squash merge `ef3457c11646b1069e5e65683d3618b3d470136e` / 8 of 8 applicable permanent workflows, each accepted on one unchanged exact head with zero unresolved comments, reviews or review threads.

Step 20A proves the typed governed Customer Privacy browser product plane against real PostgreSQL, assembled `crm-api`, Vite and Chromium. Step 20B proves independent PostgreSQL logical backup and restore, restored-process startup and readiness, active `customer_privacy.case.list` and `customer_privacy.case.get` metrics, cross-tenant and expired-session concealment, startup `0.101` seconds, nearest-rank readiness p95 `2.977` milliseconds, backup SHA-256 `700b8ae13a71af30010b11877f70b6a4b3efe1b0ec3beddaf0f3e3bc19533d3c`, backup size `1,118,941` bytes and Chromium 3 of 3.

Repository Steps 1–20 are complete. Repository Step 21 Phase 8A closure is the only next permitted implementation packet. Phase 8A.11, Phase 8A, Customer Privacy as a complete product capability, product-complete expert modules, architecture 10/10 and the Universal CRM product remain incomplete.

