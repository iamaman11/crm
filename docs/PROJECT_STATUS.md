# Ultimate CRM — Project Status

Status date: 2026-08-01

This is the concise current-state snapshot. Normative product dependencies remain in `IMPLEMENTATION_ROADMAP.md` and `PHASE8_DELIVERY_PLAN.md`; the single repository execution order is in `ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md` section 2.4; module readiness remains in `MODULE_CATALOG.md`.

## Authoritative references

- `SYSTEM_INVARIANTS.md`, `APPLICATION_ARCHITECTURE.md`, `DELIVERY_GOVERNANCE.md`;
- `ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md` section 2.4 for the only permitted repository packet order;
- `adr/ADR-031-step-13-complexity-remeasurement-and-anti-circumvention.md` for the binding step-13 entry and exit evidence;
- `RUST_TOOLCHAIN_AND_LINT_BASELINE.md` and `rust-governance-policy.json` for the accepted Rust boundary and measured lint cohorts;
- `IMPLEMENTATION_ROADMAP.md`, `PHASE8_DELIVERY_PLAN.md`, `MODULE_CATALOG.md`;
- `CUSTOMER_PRIVACY_DISCOVERY_SNAPSHOT_FREEZE.md` and `CUSTOMER_PRIVACY_DISCOVERY_SNAPSHOT_IMPLEMENTATION.md`;
- `CUSTOMER_PRIVACY_PLANNING_FREEZE.md` and `CUSTOMER_PRIVACY_PLANNING_IMPLEMENTATION.md`;
- `CUSTOMER_PRIVACY_PLAN_READS_IMPLEMENTATION.md` and its machine-readable evidence;
- `CUSTOMER_PRIVACY_APPROVAL_IMPLEMENTATION.md` and `contracts/customer-privacy-approval-implementation.json`;
- `CRM_CAPABILITY_COVERAGE.md` and the accepted historical owner packet documents.

## Current position

**Phases 0.1–7 are complete. Phase 8A is active. Phase 8A.10 is complete. Phase 8A.11 / issue #126 is in progress.**

Latest accepted Customer Privacy runtime baseline is PR #244 / accepted source `405d2dbb97bb371b51cfb1d4ffb5549a57262878` / squash merge `4b08202fe9dd0c0df83567e24e6b9d86fb79c9db` / 34 of 34 applicable permanent workflows on one unchanged exact head.

Latest accepted repository implementation packet is PR #257 / accepted source `6cde72d7fc9a442018c51fd6e6772e626b26e307` / squash merge `10516e84ea3c2d0fa8ee0c61c9eeec7e96a6273c` / 7 of 7 applicable permanent workflows on one unchanged exact head. It completes the remaining ADR-031 blocking exit evidence without product or runtime changes. Repository step 13 is complete after this exact-head synchronization; repository step 14 is next and not started.

PR #257 / accepted source `6cde72d7fc9a442018c51fd6e6772e626b26e307` / squash merge `10516e84ea3c2d0fa8ee0c61c9eeec7e96a6273c` / 7 of 7 applicable permanent workflows on one unchanged exact head completes the remaining ADR-031 blocking exit evidence. It freezes reduction-only workspace and role-aware central-system budgets; separately proves `crm-api` remains production-thin at one runtime internal dependency plus eighteen acceptance-only dev dependencies and `crm-application-runtime` remains at sixty-two runtime plus one dev internal dependency; and blocks unmeasured process-host, representative change-cost, dependency-version/feature, heavy-feature, declaration and workspace-centralization growth while permitting reductions. The nine-file packet changes no product/runtime source, Cargo manifest, dependency declaration, package, route, contract, schema, migration, persistence or worker behavior.

ADR-031 remains the binding architecture decision through PR #251. PR #253 established the exact 113-package measurement baseline, PR #255 activated suppression/direct-lint enforcement, and PR #257 mechanically proves every remaining process-host, change-cost and dependency-governance exit criterion. No architecture score is raised by declaration; this closes repository step 13 only, not the overall 10/10 program.

The accepted inventory is seven public mutations (`case.create`, `case.submit`, `case.subject.verify`, `case.cancel`, `case.approve`, `restriction.place`, `legal_hold.place`), four permission-aware public queries (`case.get`, `case.list`, `case.plan.get`, `case.owner_outcomes.list`) and zero Customer Privacy workers. `customer_privacy.plan.build@1.0.0`, `customer_privacy.retention.evaluate@1.0.0`, replay-safe owner execution, `customer_privacy.access_export.request@1.0.0` and authoritative exact-nine owner-action execution remain trusted-internal with no public route.

**All nine authoritative owner implementations are accepted:**

1. Parties — PR #156 / merge `4368b8c3710e05137b71ba999bf7f3497c0801c8`;
2. Consents — PR #175 / merge `039d6461803208f6cb70ce0fbcfcaffaf59d7125`;
3. Customer Accounts — PR #179 / merge `5b5252a437c6bebbd7afdead0162063af4c0b7e4`;
4. Contact Points — PR #181 / merge `96cd0cf548310592a0718c97242a724a29717a72`;
5. Party Relationships — PR #183 / merge `9ad2aa91321e9edb54cab98218f93143923ef33f`;
6. Identity Resolution — PR #186 / accepted source `24456b86379a1ef23ed5a60804cdcae5075d407c` / merge `509eb304a76055c9f49b0beed3b007963a91cb22` / 25 of 25 permanent workflows;
7. Customer Data Operations — PR #188 / accepted source `07f34786e82fdfa78d263790e9f50541529006f8` / merge `089be72fa3010b4aa15aff7f9ea55fd86290f8fc` / 26 of 26 permanent workflows;
8. Data Quality — PR #190 / accepted source `dcfe8faebc7462b888f8fc1721cb379a40fea88a` / merge `deac197c97cddc15bb9916092ca87f6e767ce1de` / 27 of 27 permanent workflows;
9. Customer Enrichment — PR #192 / accepted source `e90e36027de18a07be68e43327ea732810ff332a` / merge `e41cbab0cd30819fcbe2e3c5f2c7415fc6de3e8c` / 28 of 28 permanent workflows.

## Accepted scope discovery and immutable snapshot

Accepted through PR #206 / source `086b17a95058eee285fcb67a903bd21d9263d357` / merge `95818fd3aeb54a9593a45642583f0b7224d5ecfe` / 31 of 31 permanent workflows. The packet accepted trusted-internal exact-nine discovery, immutable snapshot lineage, bounded pages/checkpoints, strict rehydration, replay/crash recovery, permission-aware internal reads, safe audit, FORCE RLS, cross-tenant concealment, rollback, reapply and repeated acceptance.

The public inventory and workspace package count remained unchanged. PR #207 synchronized its post-merge evidence.

## Accepted deterministic planning runtime

PR #208 froze the deterministic action-plan and permission-aware read semantics. Its historical status text says **runtime implementation not started** and remains immutable historical evidence.

PR #209 later accepted the trusted-internal activation-gated planning runtime. It verifies the exact case, immutable scope snapshot, Party/Identity Resolution binding, policy and jurisdiction lineage; builds deterministic `Retain`, `RestrictOnly`, `Anonymize`, `Delete` or supported `CryptoShred` items; reserves `NoOpAlreadyCompliant`; strictly rehydrates the plan; atomically persists case/snapshot/plan replay evidence and transitions `Scoped → Planned` or `Scoped → AwaitingApproval`.

Accepted controls include canonical owner/resource order, contiguous sequence, lineage/item/plan digests, unsupported crypto-shred fail closed, idempotent replay/conflict detection, append-only evidence, FORCE RLS, canonical `tenant_isolation`, cross-tenant concealment, clean PostgreSQL, rollback/reapply and repeated acceptance. PR #209 historical inventory remains 4 mutations / 2 queries / 0 workers.

## Accepted permission-aware read packet

PR #211 / accepted source `933fa4b502d60a23b83de9ccee279cc6517b5cba` / merge `a1f3a60a6d8e8bba7bda50f936c57a61bc3521f7` / 32 of 32 permanent workflows promotes only `customer_privacy.case.plan.get@1.0.0` and `customer_privacy.case.owner_outcomes.list@1.0.0` through the existing Customer Privacy application/PostgreSQL/production packages.

`case.plan.get` is activation-gated, tenant-bound and live permission-aware. It strictly validates the case, immutable snapshot, immutable plan and durable replay link before returning a payload-safe summary without owner resource payloads.

At the historical PR #211 boundary, `case.owner_outcomes.list` returned a deterministic empty terminal page. PR #237 later accepted durable FORCE-RLS owner outcomes and real permission-aware bounded pagination with stable safe page evidence, without adding a mutation, worker or new public coordinate.

## Accepted approval runtime

PR #220 / accepted source `98000b0c1c2c15e14c7ee0cd2a366020040567e6` / squash merge `01118df3b6349b6d854c4182c17f7eb9a6316b9c` / 21 of 21 permanent workflows completes repository step 2. `customer_privacy.case.approve@1.0.0` is activation-gated, live-authorized, tenant-bound and optimistic-concurrency protected. It permits only `AwaitingApproval → Planned`, locks the case and immutable snapshot/plan evidence, preserves exact case↔subject↔snapshot↔plan lineage, records immutable approval actor/time, and atomically persists status, event, audit, idempotency and business evidence. Exact replay succeeds; conflicting replay, corrupt evidence, unauthorized access and cross-tenant existence fail closed.

The packet adds no crate, dependency family, worker, restriction placement/release, legal-hold or mandatory-retention adjudication, owner execution/outcome persistence, access/export assembly or destructive action.

## Accepted Rust toolchain and lint governance

Repository step 1 is accepted through PR #218 / accepted source `71c88f3e894f1fd943f373d8509e7569cf9aa291` / squash merge `e8fea1645fe108aa8334c40a445299dde8b444f0` / 30 of 30 applicable permanent workflows.

The repository supports exact Rust `1.97.1`; root workspace `rust-version` is `1.97.1`; Rust and Clippy warning/error budgets are zero and measured from JSON compiler output. PR #255 removed all three historical package-local `clippy.too_many_arguments = "allow"` tables and their matching architecture exceptions, moved the affected packages to workspace lint inheritance, and proved canonical Clippy and workspace tests without source-level replacement suppressions. `Cargo.lock`, all 113 workspace packages, external dependencies and product behavior remain unchanged.

## Accepted bounded contribution aggregation

Repository step 3 is accepted through PR #222 / accepted source `b5651e784a156758b39eaa04abc1124c7c0832f9` / squash merge `fd86ab1408e435ccc9f47b7a86ab3dd66df64ec1` / 16 of 16 permanent workflows on one unchanged exact head.

The Customer Accounts owner composition now exposes its exact data-only mutation/query definition factories, and `crm-first-party-modules` re-exports them beside the already accepted owner contribution builder. Generic application runtime consumes those selected inventory factories through the first-party facade while preserving the exact mutation/query order, deterministic Account-before-Consents contribution order and existing activation gates.

The five-file packet changed no route, coordinate, public inventory, persistence, migration, tenant isolation, authorization, audit, idempotency, product behavior, dependency family, manifest, `Cargo.lock`, workspace member, worker or Customer Privacy behavior. Workspace package count remains 113.

## Accepted final customer-subject policy prerequisite

PR #224 / accepted source `e57307fcb1b5192d5e6340247cb6633f32b7ba34` / squash merge `67804d9478b2bbaf342a398b649e23bd5ead6c08` / 28 of 28 permanent workflows accepts the smallest architecture prerequisite required by repository step 4.

`crm-core-data` now exposes a transaction-scoped `TransactionalCustomerSubjectPolicyPort` for authoritative live processing/communication decisions in the caller's PostgreSQL transaction and a deterministic `TransactionalAggregateGuardChain` for ordered final guards. The port contract requires the shared tenant + canonical Party lock and bounded denial on unavailable, stale, corrupt or cross-tenant decisions. No no-op or allow-all production implementation exists.

The three-file prerequisite added no route, coordinate, public inventory, restriction runtime, owner integration, persistence, migration, dependency family, manifest, `Cargo.lock`, workspace package, worker or product behavior. Those historical prerequisite non-effects remain accepted.

## Accepted immediate deny-only restriction runtime

Repository step 4 is accepted through PR #226 / accepted source `ad08a691ec759b8b3b523fa66a034cecf4138ff0` / squash merge `a46460623e90c5649d36bedba055fb55023d9349` / 34 of 34 permanent workflows on one unchanged source-authored head.

`customer_privacy.restriction.place@1.0.0` is public, activation-gated, live-authorized, tenant-bound and idempotent. Placement proves the Party is currently canonical, acquires the shared tenant + Party lock, validates a strict high-risk Personal contract, persists record/event/audit/idempotency/business evidence and uses deterministic tenant/idempotency-bound identity.

The authoritative PostgreSQL final decision reads bounded FORCE-RLS state under the same lock and fails closed on unavailable, malformed, over-bound or cross-tenant evidence. `contact-points.contact-point.create@1.0.0` is the first complete protected-owner boundary and checks the final policy immediately before persistence. Permanent real-process acceptance proves pre-restriction success, public placement, active denial without side effects, unrelated-Party isolation, full rollback/reapply and repeated acceptance.

Restriction release/reads, owner execution/outcomes, access/export assembly, destructive actions and Customer Privacy workers remain non-runtime.

## Accepted repository explanation and generated navigation

Repository step 5 is accepted through PR #228 / accepted source `a9aa0bef028d906b61e83803436167bf6f91e634` / squash merge `727a244fcf174dc517dec6fdbb6b8997eb205f14` / 5 of 5 applicable permanent workflows on one unchanged source-authored head.

The repository now has deterministic `repo.py explain` for exact module/capability ownership and route classification, fail-closed `repo.py packet-check` for exact baseline/path/affected/workflow/freshness validation, generated `docs/ACTIVE_PACKET.md`, and generated `docs/generated/REPOSITORY_MAP.md` with source digests. Affected Scope CI executes packet-check against the real pull-request diff before structural, Clippy and test closures.

The accepted generated inventory is 113 workspace packages, 14 business manifests, 119 published capability coordinates, 70 published event coordinates, 7 platform runtime routes, 5 worker runtime routes, 17 non-runtime contract routes and one route-less module. Product runtime, contracts, manifests, migrations, dependencies, `Cargo.lock`, package count and Customer Privacy behavior are unchanged.

## Accepted lockfile-preserving Rust workflow prerequisite

PR #232 / accepted source `3f09dcc595f79d633915e4a67117aedc59ed2499` / squash merge `3eab6dcd1d03a15ef0ce148d7f74137d2e1d10ed` / 5 of 5 applicable permanent workflows accepts the smallest repository-step-6 architecture prerequisite. Rust Generated Sync and Rust CI now verify the committed dependency graph with locked Cargo commands, preserve `Cargo.lock` byte-for-byte on ordinary packets and cannot auto-commit registry drift. Intentional lockfile refresh remains explicit through `python scripts/repo.py lock` inside a bounded packet. The six-file change adds no product behavior, contract, manifest, dependency, package, persistence or migration change.

## Accepted legal-hold and mandatory-retention precedence

Repository step 6 is accepted through PR #230 / accepted source `131285e07ad7c36c00e399b65d55591db13f0948` / squash merge `18e6218a7e7495219ac9e8c71cafcda1be64a31b` / 32 of 32 permanent workflows on one unchanged source-authored head. It promotes `customer_privacy.legal_hold.place@1.0.0`, preserves the shared tenant + canonical Party lock, strictly rehydrates bounded FORCE-RLS legal-hold state and evaluates immutable plan items with precedence active legal hold → mandatory retention → approved privacy action. Public placement is activation-gated, live-authorized, tenant-bound, idempotent and atomic; malformed, unavailable, stale, over-bound and cross-tenant evidence fails closed. Clean PostgreSQL, real `crm-api`, rollback/reapply, replay and repeated acceptance are proven. The accepted inventory is 7 mutations / 4 permission-aware queries / 0 workers, with no owner execution, outcome persistence, export assembly, destructive action, dependency, `Cargo.lock`, manifest or workspace-package change.

## Accepted reusable generic mutation/query conformance

Repository step 7 is accepted through PR #235 / accepted source `7a0cd34dc17085ecd1a8ee233171c0463d91ceba` / squash merge `43d194231fbce1cee28c44e89726929e450f3d18` / 17 of 17 applicable permanent workflows on one unchanged source-authored head. One business-neutral mutation/query conformance support boundary is reused by representative `customer_enrichment.request.create@1.0.0` and permission-aware paginated `customer_privacy.case.list@1.0.0` real-process acceptance. Mutation conformance proves activation, malformed input, tenant mismatch, live authorization denial, exact replay and incompatible replay conflict, safe error classification, rejected-call no-side-effects and atomic record/relationship/event/audit/idempotency/business evidence. Query conformance proves activation, live authorization denial, tenant mismatch and cross-tenant concealment, malformed cursor rejection, bounded keyset pagination, stable safe errors and zero query-side writes. Owner-specific fixture construction, response decoding and domain semantics remain outside the generic suite. Public coordinates and inventory, generic runtime algorithms, crates, dependencies, manifests, `Cargo.lock`, the 113-package workspace, migrations and workers are unchanged.

## Accepted replay-safe resumable owner execution

Repository step 8 is accepted through PR #237 / accepted source `f926ece93dc2b24683f982828e72bf9170dc123a` / squash merge `9f21a2b40f6af5ce57045fc4c1fbfc1bd6cb5b90` / 33 of 33 applicable permanent workflows on one unchanged source-authored head. It persists deterministic tenant-bound owner execution attempts, checkpoints and safe outcomes with immutable action-plan and retention-decision lineage; composes exactly nine canonical owner endpoints through trusted-internal production wiring; recovers from pre-invocation, post-owner-result and post-outcome/pre-checkpoint crash windows without duplicate owner invocation; and makes `customer_privacy.case.owner_outcomes.list@1.0.0` paginate real persisted payload-safe outcomes. Activation, registered initiating-capability attribution, FORCE RLS, strict rehydration, idempotency, audit, clean apply, rollback, reapply and repeated PostgreSQL acceptance are proven. Public inventory remains 7 mutations / 4 permission-aware queries / 0 workers; no public route, worker, access/export assembly, destructive owner execution, crate, dependency, `Cargo.lock`, workspace-package or generic-runtime algorithm change was introduced.

## Accepted multi-plane affected-scope enforcement

Repository step 9 is accepted through PR #239 / accepted source `e7ed45a7da5f14fa79e1ca4d23fc808004b6a642` / squash merge `e40832ae21118dd7f033e2811ca466d1242a19f0` / 8 of 8 applicable permanent workflows on one unchanged exact head. It establishes one declarative affected-scope policy for contracts, Protobuf/API compatibility, database migrations, PostgreSQL acceptance, process/runtime acceptance, product-plane checks, frontend checks and operations checks; preserves deterministic Rust ownership and reverse closure; requires the real permanent workflow filters for every selected scope; blocks unknown non-Rust paths until classified; and records exact pull-request-head evidence. Shared workflow or policy changes widen validation to all 113 Rust workspace packages. The final 12-file packet changes no product behavior, Customer Privacy public inventory, runtime route, worker, contract, Protobuf message, schema, migration, crate, dependency, `Cargo.lock`, workspace package or generic-runtime business algorithm.

## Accepted governed access/export assembly

Repository step 10 is accepted through PR #241 / accepted source `2bb3a671deb18a6ae3bcea228ed01ed287b9de6a` / squash merge `19232f6f3e2ae87aabeb080257c1aac5477a6616` / 34 of 34 applicable permanent workflows on one unchanged exact head. It implements trusted-internal, replay-safe Customer Privacy access/export assembly through `customer_privacy.access_export.request@1.0.0` and the exact `customer_data.export.privacy.request@1.0.0` Customer Data Operations boundary. Customer Privacy persists an immutable strictly rehydrated manifest and stable job/artifact references before I/O; Customer Data Operations remains the durable job and immutable artifact owner. Deterministic identities recover pre-target and finalized-artifact/pre-link crash windows without a second logical job or artifact. Activation, exact case/snapshot/plan/checkpoint lineage, tenant and canonical-Party locking, registered initiating-capability provenance, FORCE RLS, transaction/outbox/audit/idempotency evidence, clean PostgreSQL, rollback/reapply and repeated acceptance are proven. Public inventory remains 7 mutations / 4 permission-aware queries / 0 workers; no public route or alternate download endpoint, destructive action, crate, dependency, `Cargo.lock`, Protobuf contract, migration, workspace package or generic-runtime business switch was introduced.

Repository step 11 is accepted through PR #244 / accepted source `405d2dbb97bb371b51cfb1d4ffb5549a57262878` / squash merge `4b08202fe9dd0c0df83567e24e6b9d86fb79c9db` / 34 of 34 applicable permanent workflows on one unchanged exact head. It executes approved owner-specific anonymization and supported deletion through the exact nine authoritative owner boundaries, binds every call to canonical immutable case/snapshot/plan/retention/attempt lineage, and persists replay-safe tenant-bound mutation, idempotency, business transaction, audit and outbox evidence atomically under FORCE RLS. Real Parties acceptance proves mutation, exact replay, stale and cross-tenant rejection, clean PostgreSQL, rollback/reapply and repeated execution. Unsupported owner/action combinations and unavailable crypto-shred fail closed before mutation. Public inventory remains 7 mutations / 4 permission-aware queries / 0 workers; no crate, dependency, contract, migration, Cargo.lock, workspace-package or generic-runtime business-switch change was introduced.

## Accepted repository step 13 closure evidence

PR #253 / accepted source `475533b185b871418273c1c1e3f63a1d62542677` / squash merge `7dcda204be07209d9e4996fdc9c5fd364cea179e` / 7 of 7 applicable permanent workflows established the exact current-main baseline: 113 workspace packages, 841 internal dependency edges, maximum dependency depth 18, maximum direct dependents 105, maximum transitive reverse impact 106, conservative public Rust surface 5,377, 40 permanent workflows, 41 jobs, 1,712 path-filter entries, 31 PostgreSQL workflows and 94 equivalent suppression occurrences across 66 stable keys.

PR #255 / accepted source `4c80546283af9c869a28c2da9c8697b203d0c327` / squash merge `393b60bdcfad6e92fc37eacabe0920645d530f6b` / 21 of 21 applicable permanent workflows registers that historical multiset with explicit policy metadata, blocks new stable keys and occurrence growth while allowing reductions and line movement, enforces canonical formatting, removes all three direct lint tables and matching architecture exceptions, moves the affected packages to workspace lint inheritance, and activates calibrated blocking dependency/public-surface/central-LOC/reverse-impact/change-cost governance. Exact-head Rust compile, Clippy, workspace tests, generated sync, affected scope, database and applicable process/privacy workflows all passed.

The current suppression inventory is reduced from the historical 94 occurrences because the three direct lint entries are retired; the accepted baseline remains immutable evidence. No source-level `allow` or `expect` replaced them. Workspace package count remains 113 and product, tenant, RLS, authorization, persistence, route, audit and worker semantics are unchanged.

Repository step 13 is complete through PRs #253, #255 and #257. This exact-head documentation packet synchronizes the accepted closure evidence across all live normative sources. Repository step 14 is the next permitted implementation step and remains not started until this packet is accepted and merged.

## Accepted repository step 12 batch 1

Repository step 12 batch 1 is accepted through PR #246 / accepted source `3b4fe7cdf458daac9c12f816d0d6a87039e613f3` / squash merge `f090fa8785ac2bbb7e5cf186b4c5011cb9aeb978` / 37 of 37 applicable permanent workflows on one unchanged exact head. It moves Parties, Consents, Contact Points and Party Relationships exact mutation/query inventories and activation-gated contribution builders behind `crm-first-party-modules`, preserves the already aggregated Customer Accounts contribution, exact public coordinates and ordering, activation, authorization, Party-reference validation, persistence, workers, package count and external dependency versions, and removes their ordinary registration/inventory bypasses from generic native composition. The exact native-composition guard path is classified under the existing operations scope while unknown sibling scripts remain fail closed. Repository step 12 is complete; repository step 13 is complete through PRs #253, #255 and #257.

## Next permitted repository packet

Repository step 14 is the next permitted implementation packet after this exact-head repository-step-13 evidence synchronization is accepted and merged. It remains **not started** and is limited to the first measured behavior-neutral transitional domain-cluster consolidation.

## Following permitted repository packet

Repository step 15 follows only after repository step 14 is accepted and synchronized. It owns Party tombstone, no-orphan and projection/search/cache convergence evidence.

## Architecture 10/10 declaration boundary

Repository step 22 is a measured Phase 8A checkpoint, not an automatic success declaration. Architecture 10/10 requires the completed Stage D packet at step 12, explicit Stage B governance closure at step 13, measured consolidation at step 14, worker/contract/local/frontend/operations closure through step 21, two contrasting later expert-domain waves at steps 23 and 24, and a separate final review at step 25. Issue #194 remains open until every executable completion criterion is proven.

An ordinary capability in an existing owner still creates zero new crates. That is not a blanket ban: a new authoritative owner normally creates three to five owner packages, and a real provider, secrets, KMS/HSM, trust, process, extraction or compiler-enforced visibility boundary may justify a dedicated crate after architecture preflight.

## Architecture and developer-experience 10/10 checkpoint

Issue #194 remains open.

- Stage A documentation/source hierarchy and stable navigation are complete.
- Stage B dependency/crate/exception governance is complete through PRs #253, #255 and #257: reproducible metrics, exact Rust `1.97.1`, zero-warning Rust/Clippy governance, lockfile preservation, blocking suppression governance, zero direct lint tables, section-aware process-host non-growth and representative change-cost/dependency-version-feature budgets are mechanically enforced.
- Stage C is in progress: the Customer Privacy golden package model, final customer-subject policy prerequisite, authoritative restriction decision, public restriction/legal-hold placement, retention adjudication, durable replay-safe owner execution/outcomes, governed access/export assembly, authoritative exact-nine owner-specific anonymization/deletion execution and first protected-owner integration are accepted; broader owner adoption and migration/visibility generalization remain.
- Stage D is complete: all currently active first-party owners expose owner-owned production contribution boundaries aggregated through `crm-first-party-modules`; generic native composition retains platform-level composition only, accepted through PRs #246, #248 and #249.
- Stage E is complete through PR #239: deterministic Rust closure, exact repository-scope ownership, executable contract/Protobuf/API/migration/PostgreSQL/process/product/frontend/operations workflow coverage, exact-head evidence and unknown-path fail-closed enforcement are accepted.
- Stage H is in progress: deterministic explain, packet-check and generated navigation are accepted through PR #228; local lifecycle commands are repository step 18.
- Stage F is in progress: reusable generic mutation/query conformance is accepted through PR #235; generic worker conformance is repository step 16, contract lifecycle enforcement is step 17 and real Customer Privacy worker adoption is step 19.
- Stage G remains not started and is the next permitted implementation stage at repository step 14 after this synchronization is accepted. Stage I remains incomplete and is owned by steps 20–21.

## Repository continuation order

Only one implementation packet may be active. The current order begins:

```text
1. supported Rust toolchain / rust-version / measured lint baseline — complete through PR #218
-> 2. Customer Privacy approval runtime — complete through PR #220
-> 3. bounded contribution aggregation — complete through PR #222
-> 3a. final customer-subject policy port prerequisite — complete through PR #224
-> 4. immediate deny-only restrictions — complete through PR #226
-> 5. explain / packet-check / generated active packet and repository map — complete through PR #228
-> 5a. lockfile-preserving Rust workflow prerequisite — complete through PR #232
-> 6. legal-hold and mandatory-retention precedence — complete through PR #230
-> 7. reusable generic mutation/query conformance — complete through PR #235
-> 8. replay-safe resumable Customer Privacy owner execution and crash-window recovery — complete through PR #237
-> 9. affected-scope expansion for contracts, migrations, PostgreSQL/process and product-plane checks — complete through PR #239
-> 10. governed Customer Privacy access/export assembly — complete through PR #241
-> 11. owner-specific deletion, anonymization and supported crypto-shred execution — complete through PR #244
-> 12. complete first-party contribution aggregation for all currently active owners — complete through PR #249
-> 13. ADR-031 measurement, suppression/direct-lint enforcement and remaining exit-evidence enforcement — complete through PR #257
-> 14. first measured behavior-neutral transitional domain-cluster consolidation — next, not started
-> 15. Party tombstone, no-orphan proof and projection/search/cache convergence
-> 16. reusable generic worker conformance
-> 17. contract compatibility, deprecation, consumer-migration and retirement enforcement
-> 18. deterministic local lifecycle commands
-> 19. Customer Privacy worker and complete process/end-to-end acceptance
-> 20. Phase 8A frontend and operations evidence
-> 21. Phase 8A closure
-> 22. Phase 8A architecture remeasurement — checkpoint, not final 10/10
-> 23–24. two contrasting later expert-domain waves
-> 25. final architecture 10/10 closure review only if every criterion is mechanically proven
```

The complete binding order through Phase 8B entry is `ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md` section 2.4. No later item may start while an earlier item is unfinished.

Phase 8A closure does not make the universal CRM complete. Product Catalog/Pricing/CPQ/Orders/Contracts/Subscriptions/Billing and the wider expert CRM domains remain planned or incomplete.

Current product-complete expert modules: **0**.

## Repository step 13 plan-hardening evidence

ADR-031 is accepted through PR #251 / accepted source `22e515453e3ed66d0f059bd3c0fe926cee524620` / squash merge `be1411136fd36397b22e26737b441351894fdb66` / 5 of 5 applicable permanent workflows on one unchanged exact head.

At the historical PR #251 boundary, the next permitted packet was repository-step-13 measurement and governance calibration only. That packet is now accepted through PR #253 / accepted source `475533b185b871418273c1c1e3f63a1d62542677` / squash merge `7dcda204be07209d9e4996fdc9c5fd364cea179e` / 7 of 7 applicable permanent workflows on one unchanged exact head.

Repository step 13 is **complete** through PRs #253, #255 and #257 after this exact-head synchronization. Repository step 14 is the next permitted implementation packet and remains **not started**. Customer Privacy and Phase 8A remain incomplete.

## Repository step 12 completion evidence

Repository step 12 and Stage D — contribution aggregation are **complete**. All currently active first-party owners now expose owner-owned production contribution boundaries aggregated through `crm-first-party-modules`; generic native composition retains platform-level composition only.

Accepted implementation evidence:

- PR #246 / accepted source `3b4fe7cdf458daac9c12f816d0d6a87039e613f3` / squash merge `f090fa8785ac2bbb7e5cf186b4c5011cb9aeb978` / 37 of 37 applicable permanent workflows — Parties, Consents, Contact Points and Party Relationships, preserving the already aggregated Customer Accounts owner;
- PR #248 / accepted source `b15482361ab2b322591d488843ab9b46ff676dba` / squash merge `b4222364c21cb74127834f5ff4f0739343d26379` / 37 of 37 applicable permanent workflows — Identity Resolution, Customer Data Operations and Data Quality;
- PR #249 / accepted source `7876945586e5a6cc94f8d3b0f6ba2b57316484d2` / squash merge `f36592211bed3e0df7cf3771164b4bc24026eff3` / 37 of 37 applicable permanent workflows — Sales/Activities, Customer 360 and Customer Enrichment.

The accepted batches are behavior-neutral: public coordinates and ordering, tenant activation, authorization, governed Party/Consent reads, persistence, projections and workers remain unchanged; workspace package count and external dependency versions remain unchanged.

Repository step 13 is **complete** through PRs #253, #255 and #257 after this exact-head synchronization. Repository step 14 is **next, not started**. Customer Privacy and Phase 8A product readiness remain unchanged; current product-complete expert modules remain **0**.
