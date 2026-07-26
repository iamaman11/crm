# Ultimate CRM — Project Status

Status date: 2026-07-26

This is the concise human-readable status page. Normative delivery order remains in `IMPLEMENTATION_ROADMAP.md` and `PHASE8_DELIVERY_PLAN.md`.

Authoritative references:

1. `SYSTEM_INVARIANTS.md` — absolute architecture rules.
2. `ARCHITECTURE_READINESS.md` — accepted native-composition baseline.
3. `ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md` — code/package/dependency scalability plan.
4. `ARCHITECTURE_CI_SCALABILITY_AND_DEVICE_LAB_PLAN.md` — CI, PostgreSQL isolation and real-device execution plan.
5. `WORKSPACE_COMPLEXITY_BASELINE.md` — reviewed static workspace and workflow measurement snapshot.
6. `CI_TELEMETRY_BASELINE.md` — reviewed GitHub Actions run/job/step runtime telemetry snapshots.
7. `RUST_CI_CACHE_PILOT.md` — concluded cache experiment and evidence for retaining no Rust cache.
8. `GOLDEN_MODULE_CONTRIBUTION.md` — first accepted owner-built production contribution pilot.
9. `FIRST_PARTY_MODULE_AGGREGATION.md` — mechanically narrow aggregation of proven owner production contributions.
10. `AFFECTED_SCOPE_CI.md` — explainable affected-package/check iteration layer.
11. `POSTGRES_PROCESS_ISOLATION_PILOT.md` — bounded independent PostgreSQL real-process shard evidence.
12. `DELIVERY_GOVERNANCE.md` — packet-state and synchronization policy.
13. `IMPLEMENTATION_ROADMAP.md` — normative phase sequence.
14. `PHASE8_DELIVERY_PLAN.md` — detailed Phase 8 packet sequence.
15. `CRM_CAPABILITY_COVERAGE.md` — functional completeness guardrail.
16. `MODULE_CATALOG.md` — merged business-module readiness accounting.
17. `PRIVACY_OWNER_SCOPE_SHARED_SUPPORT_COMPARISON.md` — accepted shared-support boundary, compatibility baseline and current five-consumer proof.

## Current position

**Phases 0.1–7 are complete. Phase 8A is active. Phase 8A.10 is Complete. Phase 8A.11 is In progress; six Customer Privacy runtime coordinates and nine contract-only owner contribution coordinates are published. Parties, Consents, Customer Accounts, Contact Points and Party Relationships now have accepted authoritative contract-only owner implementations, while shared support remains behavior-neutral and mechanically restricted to those five consumers. PR #181 accepted unchanged source `00c5b940326b14f5e4aab7d8c8b467ee688f6c9c` with 24/24 applicable permanent workflows and merged as `96cd0cf548310592a0718c97242a724a29717a72`. The next bounded owner is Identity Resolution through `identity_resolution.privacy.scope.contribute@1.0.0`; it must remain contract-only/non-runtime.**

Current Phase 8A baseline:

- **8A.1–8A.6 — Complete:** customer references, Party, Account, Contact Point, Party Relationship, Customer 360, Consent and reversible Identity Resolution.
- **8A.7 — Complete:** governed customer import and resumable execution (#120 / PR #121).
- **8A.8 — Complete:** governed customer export, artifacts and reconciliation (#123 / PR #130).
- **8A.9 — Complete:** Customer Data Quality Rules, Completeness and Stewardship (#124 / PR #132).
- **8A.10 — Complete:** Governed Customer Enrichment and Provenance (#125 / PR #137).
- **8A.11 — In progress:** architecture, owner foundation, deterministic domain, canonical persistence, immutable public contracts, FORCE RLS persistence, four public mutations, two permission-aware queries, immutable owner-scope envelopes and nine owner-specific contract-only contribution coordinates are merged; authoritative Parties, Consents, Customer Accounts, Contact Points and Party Relationships owner implementations are accepted through PR #181, and Party Relationships is selected as the next bounded owner implementation.

The active dependency lane is:

`Party Relationships privacy owner contribution -> remaining owner privacy contributions -> sufficient owner set and scope discovery/planning -> approval/restriction/legal-hold/plan/outcome/worker lifecycle -> export/deletion/convergence -> Phase 8A closure -> 8B`

## Phase 8A.10 accepted result

Phase 8A.10 was accepted on unchanged user-authored SHA `f92d101206886e3ceaf94d0e56e52580cec21093`, which passed all 17 permanent workflows, and squash-merged through PR #137 as `150e44b95d9dbdc08c1792563de03ec73f34aed1`.

The frozen production inventory is exactly:

- **6 public mutations**;
- **6 permission-aware queries**;
- **5 activation-gated worker-only coordinates** with no public HTTP/gRPC ingress.

All 17 manifest-bound Customer Enrichment coordinates are classified as public runtime or worker runtime. Provider dispatch/response run in phase 240, materialization in phase 245 and owner application/outcome recovery in phase 250.

## Phase 8A.11 merged foundation and accepted production coordinates

The following bounded PRs are merged:

- PR #140 — ownership and guardrail freeze;
- PR #141 — Customer Privacy owner foundation;
- PR #142 — deterministic pure-domain lifecycles;
- PR #143 — canonical private persistence;
- PR #144 — immutable public Protobuf contracts;
- PR #145 — FORCE RLS persistence proof;
- PR #146 — `customer_privacy.case.create@1.0.0`;
- PR #147 — `customer_privacy.case.submit@1.0.0`;
- PR #148 — `customer_privacy.case.subject.verify@1.0.0`;
- PR #149 — `customer_privacy.case.get@1.0.0`;
- PR #150 — `customer_privacy.case.cancel@1.0.0`;
- PR #152 — `customer_privacy.case.list@1.0.0`;
- PR #153 — post-merge `case.list` governance synchronization;
- PR #154 — immutable privacy owner-scope protocol foundation;
- PR #155 — nine owner-specific privacy scope contribution contracts, all contract-only non-runtime;
- PR #156 — first authoritative non-runtime Parties privacy scope owner implementation;
- PR #175 — contrasting authoritative non-runtime Consents privacy scope owner implementation with relationship traversal and keyset pagination;
- PR #176 — behavior-neutral shared owner-scope support extraction with mechanical consumer restriction and compatibility proof.
- PR #179 — authoritative non-runtime Customer Accounts privacy scope owner implementation through embedded Account-owned Party associations and bounded keyset pagination.
- PR #181 — authoritative non-runtime Contact Points privacy scope owner implementation through direct persisted Party binding, strict endpoint-state rehydration and bounded keyset pagination.

PR #145 was accepted on source SHA `f37d9a5e025745abaaf0aeb351ff9bb534455aab` and merged as `721a1cf185ffbdea309bd1199c6c4568cf82d7a1`. Its applicable workflows proved clean migrations, FORCE RLS under `NOSUPERUSER + NOBYPASSRLS`, tenant isolation, missing-context concealment, `row_security=off` resistance, full rollback, schema removal, reapply and repeated FORCE RLS proof.

PR #146 was accepted on unchanged source SHA `9b53c3ebd81b58518dc445b02b33b35403ffa7c3`, passed all 18 applicable workflows and merged as `2d28937a123e4ba31ab0d835c4c30e3dfed0f187`. It provides deterministic tenant/idempotency case identity, confidential Draft/version-1 state, optional terminal predecessor lineage, exact replay/conflict behavior, generic HTTP/gRPC ingress, live authorization, activation gating and permanent fresh-PostgreSQL plus real-process evidence.

PR #147 was accepted on unchanged source SHA `8b41e8420b1a897777596c68cb615e2b8bf80c34`, passed all 18 permanent workflows and merged as `0eba56084405301eb667f2173b3aef6565b95f87`. It provides exact optimistic `Draft -> Submitted`, strict confidential rehydration, replay/conflict and malformed-state rollback, generic ingress, activation/live authorization, FORCE RLS and clean/reapplied real-process acceptance.

PR #148 was accepted on unchanged source SHA `118327e09a6e31ba87b02bdab99289035b572ed9`, passed all 18 permanent workflows and merged as `8ee5538bf97031dd48ab3726a605b9f3ad4bfd1e`. It provides authoritative Party existence/visibility, canonical redirect and active merge lineage, monotonic Identity Resolution topology generation, shared topology and canonical-subject locks, atomic `Submitted v2 -> SubjectVerified v3`, exact replay/conflict/concealment and permanent HTTP/gRPC process acceptance.

PR #149 was accepted on unchanged post-sync source SHA `5a47318b24007cd534434ff6bac33fbd59215d38`, passed all 18 permanent workflows and merged as `5d580a7c253bcfa6c2dd981100612b222fd26825`. It provides strict FORCE-RLS case lookup, canonical aggregate rehydration, live case and canonical Party visibility, field redaction, uniform concealment and side-effect-free real HTTP/gRPC query acceptance.

PR #150 was accepted on unchanged post-sync source SHA `be05e874b21ab33cb8b6a84fbcefc3c025aa88cb`, passed all 18 permanent workflows and was squash-merged as `2a4c34727e9d7bf8ed51b6411b7ab9c76c109671`. It provides race-free terminal cancellation with shared sorted/deduplicated subject locks before a retained final case-row `FOR UPDATE`, direct row serialization for unbound cases, immutable lineage preservation, exact replay/conflict, lock-contention rollback/retry, generic ingress, live authorization/activation and permanent clean/reapplied FORCE-RLS process acceptance.

PR #152 was accepted on unchanged source SHA `9de6048f951c0797a94871457d2bdd73357aee59`, passed all 18 permanent workflows and was squash-merged as `26f5b4644c935001806343b2feaf802a78c90eae`. It provides canonical-Party-scoped listing, signed filter-bound pagination, bounded FORCE-RLS keyset scanning, strict verified-subject matching, live Party/case visibility, field redaction, uniform empty concealment and side-effect-free real HTTP/gRPC acceptance on an isolated fresh database.

PR #155 was accepted on unchanged source SHA `7574297ed9c7a28b0e5612052898d43a6e156dcc`, passed 15 applicable workflows and was squash-merged as `624f7f3dd5099384cd7304db6bab78bc4cfc9c51`. It publishes one immutable reference-only owner contribution coordinate for each of Parties, Customer Accounts, Contact Points, Party Relationships, Consents, Identity Resolution, Customer Data Operations, Data Quality and Customer Enrichment. No owner implementation, worker registration, public ingress or runtime promotion was introduced.

PR #156 was accepted on unchanged source SHA `753acdb2ad2c25b343d0aae3413bb8b5c38581e2`, passed all 18 applicable workflows and was squash-merged as `4368b8c3710e05137b71ba999bf7f3497c0801c8`. It implements the Parties owner contribution in one tenant-bound `REPEATABLE READ, READ ONLY` transaction with transaction-scoped RLS, the shared topology advisory lock, exact generation and canonical-claim proof, strict Party rehydration, reference-only deterministic evidence and clean/reapplied malformed/cross-tenant/stale-lineage/no-write PostgreSQL acceptance. It remains contract-only/non-runtime.

PR #175 was accepted on unchanged source SHA `b492d5302b421942903be4eb0662522323b05106`, passed all 22 applicable permanent workflows and was squash-merged as `039d6461803208f6cb70ce0fbcfcaffaf59d7125`. It implements Consents as the contrasting multi-record owner through authoritative Party-to-Consent relationships, strict Consent rehydration, bounded keyset pagination, lineage-bound cursor evidence, immutable-required-evidence classification, clean rollback/reapply and repeated no-write PostgreSQL acceptance. It remains contract-only/non-runtime.

PR #176 was accepted on unchanged source SHA `eb8e6b6f2edf038485e5c64014d7d28dba302ce8`, passed all 21 applicable permanent workflows and was squash-merged as `80411d54a3ca45a783d982152c5cd8317f1fd9bd`. It extracts only proven common request integrity, lineage/registry/time/page-size validation, canonical Party proof and digest framing; originally limited consumers to Parties and Consents, while PRs #179 and #181 later extended only the mechanical allowlists to independently proven Customer Accounts and Contact Points adapters; owner errors, digests and runtime inventory remain unchanged.

PR #179 was accepted on unchanged user-authored source SHA `7d3e44e6dede36f76dfe92145dea6129a2b4639e`, passed all 23 applicable permanent workflows and was squash-merged as `5b5252a437c6bebbd7afdead0162063af4c0b7e4`. It implements Customer Accounts through strict Account rehydration, embedded `Primary` and `Member` Party association matching, bounded owner-specific keyset pagination and cursors, deterministic reference-only evidence, clean rollback/schema-removal/reapply acceptance and zero query-side writes. It remains contract-only/non-runtime and leaves shared support behavior unchanged.

PR #181 was accepted on unchanged user-authored source SHA `00c5b940326b14f5e4aab7d8c8b467ee688f6c9c`, passed all 24 applicable permanent workflows and was squash-merged as `96cd0cf548310592a0718c97242a724a29717a72`. It implements Contact Points through strict persistence-envelope and full-domain rehydration, exact direct Party binding, bounded owner-specific keyset pagination and cursor/digest domains, deterministic reference-only evidence, endpoint-value byte exclusion, clean rollback/schema-removal/reapply acceptance and zero query-side writes. It remains contract-only/non-runtime and leaves shared support behavior unchanged.

Identity Resolution is selected as the next bounded contract-only owner implementation through `identity_resolution.privacy.scope.contribute@1.0.0`. Its packet must preserve authoritative two-endpoint temporal relationship semantics, strict rehydration, reference-only evidence, owner-specific matching/pagination/retention/errors and the accepted shared-support boundary without runtime promotion or Customer Privacy orchestration.


PR #183 accepted unchanged user-authored source `a431185e01e95dfeffcf7d9c9a440afc8f0c9a57`, passed all 25 applicable permanent workflows and was squash-merged as `9ad2aa91321e9edb54cab98218f93143923ef33f`. Party Relationships proves the fifth authoritative owner shape through strict two-endpoint matching, directional/reciprocal and temporal domain rehydration, bounded owner-specific pagination/cursors, reference-only evidence, response-byte non-disclosure, clean rollback/schema-removal/reapply acceptance and zero query-side writes. It remains contract-only/non-runtime and changes no shared-support behavior.

## Architecture scalability governance

PR #157 was accepted on unchanged source SHA `aca66bf56b206e5f01ed4fa2fa76486a0860290d`, passed all 16 applicable workflows and was squash-merged as `6195fb631b9f01e2de4a4975c1f44de24610938a`.

The accepted architecture direction now requires:

- business modules to remain authoritative while normal capabilities add zero crates;
- measured dependency, fan-out, build and test complexity;
- module-owned contribution aggregation;
- affected-scope iteration with unchanged exact-head final acceptance;
- two contrasting privacy owner implementations before shared protocol extraction;
- gradual behavior-neutral consolidation rather than a workspace rewrite.

PR #158 adds the normative CI scalability, PostgreSQL E2E isolation, immutable Action pinning, cache trust and secure device-lab companion plan.

## Accepted CI controls and Phase B evidence

- PR #159 was accepted on unchanged source SHA `75b5194f1d79f68e8b1c17dc7bc5373bb87c688d`, passed all 17 applicable workflows and merged as `8d45e9f37c0f21dc4a7d530c6dacdecbe2fa1a4f`. It restricts branch push runs to `main`, cancels only superseded pull-request runs and enforces the policy in Governance CI.
- PR #160 was accepted on unchanged source SHA `7bcc0b37cd323010f03e4f5abfb61660bad4dd52`, passed all 18 applicable workflows and merged as `f89b3238a05e8825e48f418bc86457769604ba9c`. It pins every external GitHub Action reference to a reviewed full commit SHA and enforces immutable pinning through Governance CI.
- PR #161 was accepted on unchanged source SHA `755f91b25c36fe14b4dd418599b5e4c992fae037`, passed all 3 applicable workflows and merged as `466ded7185fadbfc0d5ceeb2574e34d40ef457b6`. It adds exact-head static workspace and workflow complexity reporting plus the reviewed initial snapshot.
- PR #162 was accepted on unchanged source SHA `fa510b87066723295d0f64781c4c87b7e55bf44e`, passed all 4 applicable workflows and merged as `62d3d2622331a11b6b27c88c3501a385b1aa2d80`. It adds read-only CI runtime telemetry, daily artifacts and the reviewed initial burst snapshot.
- PR #164 was accepted on unchanged source SHA `3d5c35cca46e092b0f161932599daee093c1efeb`, passed all 3 applicable workflows and merged as `7d919fa86c1069737fea075ddda21e97bb8f5082`. It extends runtime telemetry to non-internal workflow steps and records Rust workspace-test, Clippy and Governance conformance timing distributions.
- PR #165–#167 executed and concluded the controlled Rust cache experiment. Full `target/` caching restored approximately 20.15 GiB and was slower than the relevant baseline. The dependency-only variant restored approximately 71.41 MiB but two exact-hit samples averaged only about `+0.07%` versus the exact cold run. PR #167 restored the permanent cache-free workflow and merged as `a7b4281d7a6822486ed8113edbf8b4e667bd64eb` after all applicable exact-head checks passed.

The reviewed static snapshot records 100 workspace packages, 638 declared internal dependency edges, 28 one-consumer packages, 11 duplicate external dependency families and 847 manually maintained workflow path-filter entries. The initial runtime burst snapshot records execution p50/p95 of 85/232 seconds across 100 recent completed runs and confirms active cancellation of superseded pull-request work. Step telemetry identifies Rust Clippy/workspace tests and Governance conformance as major contributors. Current evidence does not justify a permanent Rust cache or an absolute performance budget.

## Module-owned production contributions and first-party aggregation

PR #168 was accepted on unchanged user-authored SHA `20d08c15071a251441954db0f6c7421765fa9f88`, passed all 19 applicable permanent workflows and was squash-merged as `c82a690acd161be6ecea2728ee53177308782132`. Customer Accounts now builds its complete mutation/query `ModuleContributionSet` inside its existing owner composition package.

PR #170 accepted Consents as the required contrasting stable production owner and squash-merged as `7a92047e5381ffe5c51a1d15ca71047a56559612`. Consents now owns its production route construction while the generic process host retains only the explicit Consent query dependency required by Customer Enrichment.

PR #171 accepted the mechanically narrow `crm-first-party-modules` aggregation seam on unchanged source SHA `f0ccf5066c607701a2d54fa6c8d600c795499688`, with all 19 applicable permanent workflows green, and squash-merged as `f433bb8d507c08bc1fc34723a656effb545e9b68`. The aggregate repeats no module IDs or route coordinates; owner packages remain authoritative and generic runtime no longer imports the proven Account/Consents production composition packages directly.

## Affected-scope iteration and PostgreSQL isolation evidence

PR #172 accepted explainable affected-scope iteration on unchanged source SHA `ff5a7c0ad0c5031984305ae7379820c3bbb1f1a4` and merged as `ac500f7bcea102d40d6e8d1ef200c3aa396c3000`. `repo.py affected` and phased `check-affected` compute changed package ownership, reverse dependency closure and workflow reasons; unknown/shared impact broadens to the full matrix, and final exact-head acceptance remains unchanged.

PR #173 accepted a bounded two-shard Party/Account PostgreSQL isolation pilot on unchanged source SHA `abd612788be6c054c76ceeda1b6bba3c7305b40a`, passed all five applicable permanent workflows and squash-merged as `70808722036aa48be5e6212290e757e01b671c0f`. Two repeated samples proved independent runners, PostgreSQL services, database/artifact namespaces and real process lifecycles. Database setup remained about 1.5 seconds and warm process execution about 2.4–3.4 seconds, while cold compilation consumed roughly 94–96% of measured shard time. The sequential Application Runtime CI remains the authoritative control lane and the pilot is not expanded yet.

## Current merged Customer Privacy production inventory

Merged `main` is exactly:

- **4 public mutations:** create, submit, subject verification and cancel;
- **2 permission-aware queries:** case get and subject-scoped case list;
- **10 public Customer Privacy non-runtime coordinates**;
- **9 owner privacy scope contribution coordinates**, all contract-only non-runtime;
- **2 implemented owner contributions:** Parties and Consents, both still contract-only/non-runtime;
- **0 Customer Privacy worker runtime routes**;
- crypto-shred remains reasoned non-runtime and is not introduced as a public, worker or platform route.

No approval, plan/outcome read, restriction, legal-hold, worker, owner-execution or crypto-shred coordinate has been promoted. The Parties owner privacy scope implementation is merged but is not composed, activated, authorized or exposed as runtime.

## Remaining Phase 8A.11 boundary

Phase 8A.11 is not complete. Remaining work includes a contrasting second owner proof and only then shared protocol extraction, the remaining fully proven owner privacy scope contributions, approval, live restriction and legal-hold precedence, plan and owner-outcome reads, bounded owner contribution/orchestration, privacy export, deletion/anonymization convergence, immutable-evidence preservation, worker recovery and complete lifecycle acceptance.

## Merged platform and customer-master baseline

Merged `main` contains executable architecture governance, typed module/runtime foundations, PostgreSQL tenant/RLS/records/idempotency/outbox/audit, authenticated mutation and permission-aware query gateways, native module-owned exact-coordinate composition, durable workers/projections/search, and production slices for Party, Account, Contact Point, Party Relationship, Customer 360, Consent, reversible Identity Resolution, import, export, Data Quality, Customer Enrichment and six accepted Customer Privacy runtime coordinates. It also contains the first proven non-runtime Parties privacy scope owner implementation, governed CI event/cancellation and immutable Action policies, permanent static/runtime/step complexity telemetry, the concluded cache-free Rust decision, two contrasting module-owned production contributions, the first-party aggregate, explainable affected-scope iteration and the bounded independent PostgreSQL process-isolation pilot.

## Product completeness reality

The project is **not yet a complete universal CRM**. Major required families still include the remaining Phase 8A.11 privacy lifecycle, Product Catalog/Pricing/CPQ/Quotes/Orders/Contracts/Subscriptions, broader Sales and Activities, omnichannel, Service, Marketing, Customer Success, projects, documents/e-signature, analytics, workflow/collaboration, AI governance, marketplace and enterprise operational proof.

## Immediate next actions

1. Implement one contrasting second privacy scope owner, preferably Consents, as a bounded contract-only/non-runtime packet with owner-authoritative reads, tenant/RLS proof, malformed/cross-tenant/stale-lineage/no-write evidence and no premature worker/runtime promotion.
2. Compare the second privacy owner with Parties and extract only protocol support proven common by both; do not copy one large adapter or create a separately edited owner catalog.
3. Migrate the remaining privacy owner contributions one bounded owner at a time through the accepted shared support and first-party architecture seams.
4. Continue approval, immediate restrictions, legal-hold/retention precedence, plan/outcome reads, bounded orchestration, export/deletion/anonymization convergence and worker recovery without promoting contract-only coordinates before production proof.
5. Retain full unchanged exact-head final acceptance, the sequential process control lane and measured affected-scope/isolation evidence; close Phase 8A only after the complete privacy/customer-master interaction baseline is merged and reconciled, then begin Phase 8B / #29.
