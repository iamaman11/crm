# Ultimate CRM — Project Status

Status date: 2026-08-04

This document is the concise authoritative current-state snapshot. Product dependencies remain normative in `IMPLEMENTATION_ROADMAP.md` and `PHASE8_DELIVERY_PLAN.md`; the single repository execution order remains normative in `ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md` section 2.4; module readiness remains normative in `MODULE_CATALOG.md`.

## Authoritative references

- `SYSTEM_INVARIANTS.md`, `APPLICATION_ARCHITECTURE.md`, `DELIVERY_GOVERNANCE.md`;
- `ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md` for architecture stages, metrics and the only permitted repository-step order;
- ADR-031 for complexity remeasurement/anti-circumvention and ADR-032 for the mandatory Step 22 runtime-fan-in and permanent-gate value decisions;
- `IMPLEMENTATION_ROADMAP.md` and `PHASE8_DELIVERY_PLAN.md` for product sequencing;
- `MODULE_CATALOG.md` and `CRM_CAPABILITY_COVERAGE.md` for module ownership and product readiness;
- `repository-packet.json`, generated `ACTIVE_PACKET.md` and generated `generated/REPOSITORY_MAP.md` for the active bounded packet;
- accepted pull requests and permanent-workflow evidence for historical implementation facts.

## Current position

**Phases 0.1–7 are complete. Phase 8A is active. Phase 8A.10 is complete. Phase 8A.11 / issue #126 remains in progress.**

Phase 8A.11 / issue #126 is in progress.

Repository Steps 1–19 are complete. The bounded Repository Step 20A product-plane slice is accepted. Repository Step 20A is accepted through PR #292. Repository Step 20 remains in progress; Repository Step 20B restore, SLO, observability, performance, security and supply-chain operations evidence is the only next permitted implementation packet. Architecture Stages A, B, D, E, F, G and H are complete. Stages C and I remain incomplete or in progress according to the architecture plan.

Repository Step 14 and Stage G are accepted through PR #259 / accepted source `8aa0b33c6609e74f98363071c6e7c44ec59fc098` / squash merge `2b0b558077c444d4469137c8a2bcca2c14ae426` / 36 of 36 applicable permanent workflows on one unchanged meaningful user-authored exact head.

The accepted Step 14 consolidation is behavior-neutral:

- `crm-customer-accounts-capability-composition` is removed;
- its production contribution is owned by `crm-customer-accounts-query-adapter`;
- mutation planning remains in `crm-customer-accounts-capability-adapter`;
- `crm-first-party-modules` remains the mechanically narrow aggregate;
- public mutations, queries, workers, route classifications, contracts, schemas, migrations, tenant isolation, FORCE RLS, authorization, idempotency, audit, persistence and worker behavior remain unchanged;
- the permanent Approval, Discovery and Planning workflows now assert the accepted 112-package workspace while retaining their full behavioral, rollback and reapply checks.

Repository Step 15 is accepted through five bounded implementation slices:

- PR #263 / source `6c2a54f6780988a12fec3cd77ca2cd39ad349140` / merge `bd205e0af77b676654dff8ddf26d3b5b195880b2` / 32 of 32: Party privacy tombstones converge into global search generation `g3` and are excluded before candidate disclosure;
- PR #264 / source `e6c9d2901109c8d5b9e0f3cf783214407e26451a` / merge `e9fe1f352386d80a29d122db5d1ed6c47266bfaf` / 6 of 6: Customer 360 overwrites the stable Party contribution with a non-personal tombstone and removes root membership;
- PR #265 / source `ef572bdf31c584c397c215cd1b62ee47cad54e64` / merge `2a0ee9c33fbe23cdf9c6dccb1acc7e3bd8bcba3a` / 19 of 19: canonical owner execution, immutable-history rebuild and Search replay eliminate stale derived personal data without mutating authoritative evidence;
- PR #266 / source `ded5d80ae11bbf044b5bfe5b572e8dab521f884a` / merge `1f889a810c82da3d0fee12427eacccbe43613bac` / 19 of 19: Customer 360 advances to fresh production generation `customer.customer-360.v2`, while legacy `v1` remains historical and non-authoritative;
- PR #267 / source `f1b72dbee09f152005cb3584b9bcc1573bf2c4fe` / merge `4a14a5a0bda6d25d27e7c2da7c5f4809fa1efbdf` / 19 of 19: a real `crm-api` restart repairs missing Customer 360 v2 derived state through the production background-worker cycle on clean and rollback/reapplied PostgreSQL schemas.

Step 15 proves stable non-reusable Party identity tombstones, strict privacy-event lineage, Search and Customer 360 non-disclosure, deterministic rebuild/replay, automatic fresh-generation rollover and real process-host no-orphan convergence. It adds no workspace package or dependency edge and does not change public contracts, routes or migrations.

Repository Step 16 is accepted through two bounded implementation slices:

- PR #269 / source `74b1d7b0f8764fcd90839b7aab25f8f82fe5e552` / merge `6f82de0a7b2dcd1ab5dd0ae6473d46d7d9d34bdd` / 20 of 20: a business-neutral standard-library worker conformance helper proves no-side-effect denial, retryable progress/effect preservation and exact recovery; representative Customer Enrichment and CRM API import processes adopt the helper with activation, live authorization, tenant, replay and crash/restart evidence;
- PR #270 / source `8e2baac0822eefbb6d3c474ffce0cee69e3e4e98` / merge `ce0ca881461d1ee8964a11b28c1fcff46cf145cb` / 17 of 17: two simultaneously live production `crm-api` import executors contend for the same durable Party-import work, PostgreSQL blocking evidence proves serialization before a second target effect, and release converges to one completed checkpoint and exactly one Party record, idempotency record, event and audit record with no duplicate replay.

Step 16 proves reusable worker conformance across contrasting real workers without adding a generic lease API or changing production algorithms, contracts, schemas, migrations, crates, dependencies or permanent workflows. Customer Privacy now publishes one first-party production background worker through the registered `crm.customer-privacy` / `owner-execution` phase-260 runtime boundary accepted in Repository Step 19.

## Repository Step 17 accepted closure

Repository Step 17 is complete through three bounded accepted slices:

- PR #275 / source `1c6366b557e255a14a677758fd87f7fe63184a89` / merge `60d5974349a04c462475aadd4af0a37bada9713b` / 23 of 23 — published wire-compatible `activities.task.create@1.1.0`, migrated the sole repository-owned consumer and made live authorization overlap exact-version safe;
- PR #278 / source `228f459963e571a830aee6c43cddd15e3c9b0d5f` / merge `996d634a33945e618be5ff81c297f0f617ce19d5` / 8 of 8 — made production zero-usage retirement evidence SHA-256-bound, complete, append-only and fail closed;
- PR #279 / source `0dce0895edec56508df1b4fc880d09ab27fc00df` / merge `ea3d3894d05e3a8d814aff69824b593843763d03` / 22 of 22 — proved the coordinate was never externally released through live empty GitHub Releases, tags and deployments, non-publishable packages and no external consumer record, then retired `activities.task.create@1.0.0` while preserving its historical tombstone and keeping `telemetry.zero_since` null.

The accepted Step 17 result keeps `activities.task.create@1.1.0` as the sole live create coordinate. `activities.task.create@1.0.0` is absent from the current provider manifest and production capability catalog, rejected before payload decoding, retained only as immutable historical registry/lifecycle evidence and classified `never_externally_released`. The ordinary 30-day/production-zero-usage path remains binding for released contracts; no production history was fabricated. Repository Step 18 is complete through accepted PRs #281, #283 and #285: PR #281 / source `76a0d93d594b6ffbb890f90d3cb9037febf4c3f8` / squash merge `87bc0d33befc4525b62aa7e0e1884abc07e12abf` / 7 of 7; PR #283 / source `5f4480fafd37cc8c89df60f3688e756d7f881af8` / squash merge `21e2f73b57d2c35c16eccc15ee3e075e818f488a` / 7 of 7; PR #285 / source `a522b8b11a0c6f143694f516e7a7f9d522c18ce3` / squash merge `a906f2c514285974113749b8b8ad9446202a5fa1` / 19 of 19 applicable permanent workflows, each on one unchanged exact head. The accepted lifecycle delivers deterministic repository-pinned `doctor`, locked isolated `bootstrap`, checkout-owned PostgreSQL `dev-up` / `dev-reset`, versioned idempotent `seed-demo` through the governed Party mutation gateway and real-process `smoke` proving readiness, permission, authentication and tenant boundaries. Repository Step 20A is accepted through PR #292. Repository Step 20 remains in progress; Repository Step 20B restore, SLO, observability, performance, security and supply-chain operations evidence is the only next permitted implementation packet.


## Repository Step 18 accepted closure

Repository Step 18 is complete through accepted PRs #281, #283 and #285: PR #281 / source `76a0d93d594b6ffbb890f90d3cb9037febf4c3f8` / squash merge `87bc0d33befc4525b62aa7e0e1884abc07e12abf` / 7 of 7; PR #283 / source `5f4480fafd37cc8c89df60f3688e756d7f881af8` / squash merge `21e2f73b57d2c35c16eccc15ee3e075e818f488a` / 7 of 7; PR #285 / source `a522b8b11a0c6f143694f516e7a7f9d522c18ce3` / squash merge `a906f2c514285974113749b8b8ad9446202a5fa1` / 19 of 19 applicable permanent workflows, each on one unchanged exact head. The accepted lifecycle delivers deterministic repository-pinned `doctor`, locked isolated `bootstrap`, checkout-owned PostgreSQL `dev-up` / `dev-reset`, versioned idempotent `seed-demo` through the governed Party mutation gateway and real-process `smoke` proving readiness, permission, authentication and tenant boundaries. Repository Step 20A is accepted through PR #292. Repository Step 20 remains in progress; Repository Step 20B restore, SLO, observability, performance, security and supply-chain operations evidence is the only next permitted implementation packet.

Accepted behavior:

- `repo.py doctor` provides deterministic human and JSON output, repository-pinned Rust/Node/pnpm validation, Python venv validation and actionable fail-closed remediation;
- the bootstrap profile excludes Docker, while the full profile additionally validates Docker CLI, Compose v2 and daemon reachability;
- `repo.py bootstrap` creates an isolated `.venv`, installs committed Python constraints, uses Cargo `--locked` and pnpm `--frozen-lockfile`, and verifies locked metadata plus generated navigation;
- `repo.py dev-up` creates or reuses an immutable PostgreSQL 17 dependency plane with checkout-scoped ownership labels, loopback-only publishing, ordered migrations/fixtures and a deterministic schema digest;
- `repo.py dev-reset` verifies ownership before removing only the owned container and volume, then recreates clean state;
- permanent real-Docker acceptance proves create, marker persistence, unchanged reuse, destructive reset, pre-reset probe removal and CRM schema restoration;
- `repo.py seed-demo` creates or idempotently replays the versioned `local-demo-acme` organization only through the governed Party mutation gateway;
- `repo.py smoke` starts the real `crm-api`, proves denial without a live query grant, verifies the explicit grant, rejects missing authentication and conceals tenant-A data from tenant B;
- dry-run executes no Docker mutation or process command and reports the exact ordered argument-array plan without exposing admin credentials;
- no runtime, owner, route, worker, contract, schema, migration, dependency, lockfile or product behavior changed.

## Current measured repository baseline

| Metric | Step 13 historical baseline | Current after Step 14 |
|---|---:|---:|
| Workspace packages | 113 | **112** |
| Internal dependency edges | 841 | **835** |
| Maximum dependency depth | 18 | **18** |
| Conservative public Rust items | 5,379 | **5,377** |
| Dependency declarations | 270 | **270** |
| Suppression occurrences | 91 after direct-lint retirement | **91** |

The earlier 113-package and 841-edge measurements remain immutable historical Step 13 evidence. They are not the current repository state.

## Accepted repository-step evidence

| Step | Accepted evidence | Result |
|---|---|---|
| 1 | PR #218 / source `71c88f3e894f1fd943f373d8509e7569cf9aa291` / merge `e8fea1645fe108aa8334c40a445299dde8b444f0` / 30 of 30 | Exact Rust 1.97.1, measured warning/lint governance |
| 2 | PR #220 / source `98000b0c1c2c15e14c7ee0cd2a366020040567e6` / merge `01118df3b6349b6d854c4182c17f7eb9a6316b9c` / 21 of 21 | Customer Privacy approval runtime |
| 3 | PR #222 / source `b5651e784a156758b39eaa04abc1124c7c0832f9` / merge `fd86ab1408e435ccc9f47b7a86ab3dd66df64ec1` / 16 of 16 | First bounded contribution aggregation |
| 3a | PR #224 / source `e57307fcb1b5192d5e6340247cb6633f32b7ba34` / merge `67804d9478b2bbaf342a398b649e23bd5ead6c08` / 28 of 28 | Final customer-subject policy prerequisite |
| 4 | PR #226 / source `ad08a691ec759b8b3b523fa66a034cecf4138ff0` / merge `a46460623e90c5649d36bedba055fb55023d9349` / 34 of 34 | Immediate deny-only restriction placement and first protected owner |
| 5 | PR #228 / source `a9aa0bef028d906b61e83803436167bf6f91e634` / merge `727a244fcf174dc517dec6fdbb6b8997eb205f14` / 5 of 5 | `explain`, `packet-check`, generated navigation |
| 5a | PR #232 / source `3f09dcc595f79d633915e4a67117aedc59ed2499` / merge `3eab6dcd1d03a15ef0ce148d7f74137d2e1d10ed` / 5 of 5 | Lockfile-preserving Rust workflows |
| 6 | PR #230 / source `131285e07ad7c36c00e399b65d55591db13f0948` / merge `18e6218a7e7495219ac9e8c71cafcda1be64a31b` / 32 of 32 | Legal hold and mandatory-retention precedence |
| 7 | PR #235 / source `7a0cd34dc17085ecd1a8ee233171c0463d91ceba` / merge `43d194231fbce1cee28c44e89726929e450f3d18` / 17 of 17 | Reusable mutation/query conformance |
| 8 | PR #237 / source `f926ece93dc2b24683f982828e72bf9170dc123a` / merge `9f21a2b40f6af5ce57045fc4c1fbfc1bd6cb5b90` / 33 of 33 | Replay-safe resumable owner execution and outcomes |
| 9 | PR #239 / source `e7ed45a7da5f14fa79e1ca4d23fc808004b6a642` / merge `e40832ae21118dd7f033e2811ca466d1242a19f0` / 8 of 8 | Multi-plane affected-scope enforcement |
| 10 | PR #241 / source `2bb3a671deb18a6ae3bcea228ed01ed287b9de6a` / merge `19232f6f3e2ae87aabeb080257c1aac5477a6616` / 34 of 34 | Governed access/export assembly |
| 11 | PR #244 / source `405d2dbb97bb371b51cfb1d4ffb5549a57262878` / merge `4b08202fe9dd0c0df83567e24e6b9d86fb79c9db` / 34 of 34 | Authoritative owner-specific anonymization/deletion |
| 12 | PRs #246, #248, #249 / 37 of 37 on each accepted head | Complete first-party contribution aggregation; Stage D complete |
| 13 | PR #253 / source `475533b185b871418273c1c1e3f63a1d62542677` / merge `7dcda204be07209d9e4996fdc9c5fd364cea179e`; PR #255 / source `4c80546283af9c869a28c2da9c8697b203d0c327` / merge `393b60bdcfad6e92fc37eacabe0920645d530f6b`; PR #257 / source `6cde72d7fc9a442018c51fd6e6772e626b26e307` / merge `10516e84ea3c2d0fa8ee0c61c9eeec7e96a6273c` | Measurement, suppression/direct-lint governance and remaining ADR-031 exit evidence; Stage B complete |
| 14 | PR #259 / source `8aa0b33c6609e74f98363071c6e7c44ec59fc098` / merge `2b0b558077c444d4469137c8a2bcca2c14ae426` / 36 of 36 | First measured behavior-neutral transitional consolidation; Stage G complete |
| 15 | PRs #263–#267 / exact sources and merges listed above / final closure 19 of 19 | Party tombstone, Search/Customer 360 convergence, immutable-history rebuild, automatic v2 rollover and real `crm-api` no-orphan proof |
| 16 | PR #269 / source `74b1d7b0f8764fcd90839b7aab25f8f82fe5e552` / merge `6f82de0a7b2dcd1ab5dd0ae6473d46d7d9d34bdd` / 20 of 20; PR #270 / source `8e2baac0822eefbb6d3c474ffce0cee69e3e4e98` / merge `ce0ca881461d1ee8964a11b28c1fcff46cf145cb` / 17 of 17 | Reusable worker conformance, representative real-worker adoption, retry/restart recovery and exactly-once contention convergence |
| 17 | PR #275 / source `1c6366b557e255a14a677758fd87f7fe63184a89` / merge `60d5974349a04c462475aadd4af0a37bada9713b` / 23 of 23; PR #278 / source `228f459963e571a830aee6c43cddd15e3c9b0d5f` / merge `996d634a33945e618be5ff81c297f0f617ce19d5` / 8 of 8; PR #279 / source `0dce0895edec56508df1b4fc880d09ab27fc00df` / merge `ea3d3894d05e3a8d814aff69824b593843763d03` / 22 of 22 | Wire-compatible migration, exact-version authorization overlap, immutable retirement evidence and proven never-externally-released retirement |
| 18 | PR #281 / source `76a0d93d594b6ffbb890f90d3cb9037febf4c3f8` / merge `87bc0d33befc4525b62aa7e0e1884abc07e12abf` / 7 of 7; PR #283 / source `5f4480fafd37cc8c89df60f3688e756d7f881af8` / merge `21e2f73b57d2c35c16eccc15ee3e075e818f488a` / 7 of 7; PR #285 / source `a522b8b11a0c6f143694f516e7a7f9d522c18ce3` / merge `a906f2c514285974113749b8b8ad9446202a5fa1` / 19 of 19 | Complete deterministic doctor/bootstrap/dev-up/dev-reset/seed-demo/smoke lifecycle; Step 19 is complete; Step 20A is accepted through PR #292; Step 20 remains in progress and Step 20B is next |

## Customer Privacy product boundary

All nine authoritative owner implementations are accepted. The owner set includes Parties, Consents, Customer Accounts, Contact Points, Party Relationships, Identity Resolution, Customer Data Operations, Data Quality and Customer Enrichment.

Customer Data Operations owner-scope evidence is accepted through PR #188 / accepted source `07f34786e82fdfa78d263790e9f50541529006f8` / squash merge `089be72fa3010b4aa15aff7f9ea55fd86290f8fc` / 26 of 26 permanent workflows.

Data Quality owner-scope evidence is accepted through PR #190 / accepted source `dcfe8faebc7462b888f8fc1721cb379a40fea88a` / squash merge `deac197c97cddc15bb9916092ca87f6e767ce1de` / 27 of 27 permanent workflows.

Customer Enrichment owner-scope evidence is accepted through PR #192 / accepted source `e90e36027de18a07be68e43327ea732810ff332a` / squash merge `e41cbab0cd30819fcbe2e3c5f2c7415fc6de3e8c` / 28 of 28 permanent workflows.

Scope discovery and immutable snapshot execution is accepted through PR #206 / accepted source `086b17a95058eee285fcb67a903bd21d9263d357` / squash merge `95818fd3aeb54a9593a45642583f0b7224d5ecfe`.

Latest accepted Customer Privacy public inventory remains:

- seven public mutations: `case.create`, `case.submit`, `case.subject.verify`, `case.cancel`, `case.approve`, `restriction.place`, `legal_hold.place`;
- four permission-aware public queries: `case.get`, `case.list`, `case.plan.get`, `case.owner_outcomes.list`;
- one Customer Privacy owner worker.

Trusted-internal planning, retention evaluation, replay-safe exact-nine owner execution, access/export assembly and owner-specific action execution remain non-public.

Accepted product/runtime evidence includes scope discovery and immutable snapshot execution, deterministic planning, permission-aware reads, approval, restriction placement and final enforcement, legal-hold placement and retention precedence, durable owner execution/outcomes, governed access/export assembly, authoritative owner-specific anonymization/deletion and Party tombstone convergence across Search, Customer 360, rebuild/replay and the real process host.

Still required before Phase 8A.11 can close:

- restriction and legal-hold release/read lifecycle where required;
- Repository Step 20B production operations, restore, SLO, observability, security, performance and supply-chain evidence;
- a full Phase 8A.11 closure review beyond the accepted bounded Step 20A case-read slice.

Customer Privacy and Phase 8A remain incomplete. Current product-complete expert modules: **0**.

## Architecture-stage position

- **Stage A — complete:** source hierarchy, status authority and stable navigation are enforced.
- **Stage B — complete:** dependency, crate, exception, Rust, suppression, process-host and change-cost governance are blocking and reduction-aware; Step 22 must still resolve remaining runtime fan-in rather than merely freeze it.
- **Stage C — in progress:** the Customer Privacy golden owner package now includes accepted Party tombstone/no-orphan convergence, but wider adoption and visibility/migration generalization remain.
- **Stage D — complete:** all active first-party owner contributions are aggregated through `crm-first-party-modules`.
- **Stage E — complete:** affected-scope selection covers Rust, contracts, Protobuf/API, migrations, PostgreSQL, process, product, frontend and operations planes; Step 22 must review value, duplication and cost of every permanent gate.
- **Stage F — complete through PR #290:** mutation/query and worker conformance, complete contract lifecycle enforcement and real Customer Privacy worker adoption/lifecycle proof are accepted.
- **Stage G — complete:** PR #259 proves the first measured behavior-neutral transitional consolidation.
- **Stage H — complete through PR #285:** explanation, packet checking, generated navigation and the full doctor/bootstrap/dev-up/dev-reset/seed-demo/smoke lifecycle are accepted.
- **Stage I — in progress:** the bounded Step 20A frontend, accessibility and browser slice is accepted through PR #292; Step 20B operations parity remains.

Architecture 10/10 is **not declared**. Repository Step 22 is a measurement and decision checkpoint, and final closure remains reserved for Step 25 after every criterion and two contrasting expert-domain waves are mechanically proven.

## Hardened Repository Step 22 boundary

ADR-032 makes two formerly implicit risks explicit and blocking.

First, `crm-application-runtime` remains a broad process-composition surface. Step 22 must classify every internal direct dependency as removed, platform-generic, owner-specific-unavoidable or test-only. Every safely removable owner-specific dependency must be removed. Every retained owner-specific dependency must prove an unavoidable stable process-composition boundary and prove that ordinary owner changes do not modify the runtime manifest or owner-specific runtime source. Mere non-growth is insufficient.

Second, Step 22 must review every permanent workflow, job and gate. The review must record the concrete prevented failure mode, observed defects or preventive rationale, overlap/duplication, execution cost, owner, retain/simplify/merge/remove decision and retirement condition. Duplicate or low-value gates must be simplified, merged or removed unless independent value is proven. Every new permanent gate must satisfy the same entry contract.

Step 22 cannot close with an unresolved runtime dependency classification or unresolved gate-value decision. Steps 23 and 24 must validate these conclusions under contrasting expert-domain waves.

## Next permitted repository packet

Repository Step 20A is accepted through PR #292. Repository Step 20 remains in progress; Repository Step 20B restore, SLO, observability, performance, security and supply-chain operations evidence is the only next permitted implementation packet. Repository Step 19 is complete through PRs #287–#290.

Repository Step 20B may begin only after the accepted Step 20A evidence is synchronized through this packet.

## Repository continuation order

```text
1–19. accepted and complete
-> 20. Phase 8A frontend, accessibility, browser and operations evidence — in progress; Step 20A accepted, Step 20B next
-> 21. Phase 8A closure
-> 22. architecture remeasurement + crm-application-runtime fan-in decision + permanent-gate value/cost review — checkpoint, not final 10/10
-> 23. first contrasting later expert-domain wave validating Step 22
-> 24. second contrasting later expert-domain wave validating Step 22
-> 25. final architecture 10/10 closure review only if every criterion is mechanically proven
```

Phase 8A closure will not make the universal CRM product complete. Product Catalog, Pricing, CPQ, Quotes, Orders, Contracts, Subscriptions, Entitlements, Usage, Billing and wider expert CRM domains remain planned or incomplete.

Issue #194 remains open until the architecture program is fully proven. Issue #126 remains open until Customer Privacy and Phase 8A.11 product evidence is complete.

## Repository Step 19 accepted closure

Repository Step 19 is complete only through the combined accepted evidence below, each on one unchanged exact source head with no unresolved comments, reviews or review threads:

- PR #287 / source `23b2f4ea660bcd46884fe054cd0c37e89b1495c4` / squash merge `c0fec3ae08c836ab483737442ed4377c99c85e9a` / **11 of 11** applicable permanent workflows — added the bounded Customer Privacy owner-worker boundary without public ingress or new schema/dependency surface;
- PR #288 / source `b99a4fc4ff7ef5cfd47813b487900b4c2a9f3b77` / squash merge `bc653de5f1a853791d3ab4a03f59f3daad54bf54` / **24 of 24** — added PostgreSQL ready-work discovery for planned Customer Privacy owner actions;
- PR #289 / source `3e21e79e1600727ebcda222af389d568d857cff8` / squash merge `d1c4dd278853a1e6a426fab284c70b3529d42833` / **24 of 24** — registered `crm.customer-privacy` / `owner-execution` at phase `260` in the production `ApplicationRuntime`, with activation gating and replay-safe canonical execution;
- PR #290 / source `9bbb339f39133955a7f42ea67f3334e597066e2e` / squash merge `49c5e35814adceb2be9d4cc2302bf10032b807a0` / **19 of 19** — proved the assembled real `crm-api` lifecycle on clean and rollback/reapplied PostgreSQL schemas: ready-work discovery, a real Parties privacy action, one durable attempt, successful outcome, completed checkpoint, audit evidence, owner event/outbox and final case transition, plus restart no-duplicate proof and uninstall no-discovery/no-effect proof.

The accepted first-party background-worker inventory is now **one Customer Privacy owner worker**: module `crm.customer-privacy`, worker `owner-execution`, phase `260`. This is a production background worker, not a new public capability route; the latest public Customer Privacy inventory remains **seven mutations and four permission-aware public queries**.

Repository Steps 1–19 are complete. The bounded Repository Step 20A product-plane slice is accepted. Repository Step 20 remains in progress; Repository Step 20B restore, SLO, observability, performance, security and supply-chain operations evidence is the only next permitted implementation packet. Phase 8A.11 / issue #126 remains in progress; Customer Privacy is not product-complete; current product-complete expert modules remain zero; architecture 10/10 and the Universal CRM product are not declared complete.

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

Step 20A is accepted. Repository Step 20 remains in progress; Step 20B restore, SLO, observability, performance, security and supply-chain operations evidence is the only next permitted packet. Phase 8A.11, Phase 8A, product-complete expert modules, architecture 10/10 and the Universal CRM product remain incomplete. The accepted one-worker Customer Privacy inventory, seven public mutations, four permission-aware public queries, 5,377 public Rust items, 91 suppressions and the `crm-application-runtime` 7,269 LOC ceiling remain unchanged.
