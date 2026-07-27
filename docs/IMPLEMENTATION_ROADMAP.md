# Ultimate CRM — Implementation Roadmap

Status: **Normative delivery plan**

Parent epic: #2  
Governing rules: `SYSTEM_INVARIANTS.md`  
Delivery-control policy: `DELIVERY_GOVERNANCE.md`  
Current concise state: `PROJECT_STATUS.md`  
Detailed Phase 8 sequence: `PHASE8_DELIVERY_PLAN.md`  
Architecture/developer-experience program: `ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md` / issue #194  
Measured architecture baseline: `WORKSPACE_COMPLEXITY_BASELINE.md`  
Functional completeness guardrail: `CRM_CAPABILITY_COVERAGE.md`  
Business-module accounting: `MODULE_CATALOG.md`

## 1. Purpose

This roadmap defines dependency order for a universal modular expert CRM platform. It is not a feature wishlist, architecture essay or historical status log.

A phase or packet is complete only when its acceptance boundary is implemented, merged and backed by unchanged exact-head evidence. Cross-cutting architecture work must preserve product delivery and cannot use a big-bang rewrite as a substitute for bounded implementation.

## 2. Delivery rules

1. Deliver coherent reviewable packets linked to roadmap or cross-cutting issues.
2. Preserve one authoritative owner for every mutable aggregate.
3. Enter state-changing behavior through exact versioned capabilities with typed audit evidence.
4. Never access another module's storage or internals directly.
5. Treat security, privacy, tenant isolation, compatibility, rollback and operations as implementation requirements.
6. Require real composition, persistence and process evidence before runtime claims.
7. Invalidate exact-SHA evidence after every source or documentation change until applicable checks rerun.
8. Synchronize roadmap, phase plan, status, catalog, issues and PR descriptions.
9. Do not mark the universal CRM product complete while required capability families remain incomplete.
10. A normal capability added to an existing owner creates zero new crates by default.
11. Generic router and worker algorithms must not change merely to register one owner capability.
12. Feature behavior and physical crate consolidation are separate delivery packets.
13. Shared abstractions are extracted only after contrasting real implementations prove common behavior.
14. Iterative affected-scope CI may reduce feedback cost but never weakens final exact-head acceptance.
15. Cross-cutting stages advance in dependency order, but product work may proceed at a natural boundary when prerequisites are accepted.

## 3. Work states

- Planned
- Ready
- In progress
- Gate review
- Complete
- Blocked
- Superseded

Only merged `main` work may be represented as **Complete**.

## 4. Product phase map

| Phase | Issue | Primary result | State | Depends on |
|---|---:|---|---|---|
| 0.1 | #3 | Repository hardening and executable roadmap | **Complete** | Governance v1 |
| 1 | #4 | Typed Module Manifest IR and deterministic identity | **Complete** | #3 |
| 2 | #5 | Governed Module SDK and test harness | **Complete** | #4 |
| 3 | #6 | Module lifecycle and registry runtime | **Complete** | #4, #5 |
| 4 | #7 | PostgreSQL tenant, record, artifact, outbox and audit foundation | **Complete** | #6 |
| 5 | #8 | Capability execution gateway | **Complete** | #5, #7 |
| 6 | #9 | Sales + Activities + link/projection/application vertical proof | **Complete** | #8 |
| 7 | #10 | Search, generalized projections, product shell, Admin Studio and UI-extension isolation | **Complete** | #9 |
| 8 | #11 | Expert modules and product-quality CRM experience | **In progress** | #5, #9, #10 |
| 8A | #28 | Canonical customer master, identity, consent and governed customer-data lifecycle | **In progress** | #9, #10 |
| 8B | #29 | Product catalog, CPQ and quote-to-revenue lifecycle | **Planned** | completed 8A baseline |
| 9 | #12 | AI-native governed actor/tool layer | **Planned** | mature domain capabilities |
| 10 | #13 | Signed marketplace and sandboxed untrusted extensions | **Planned** | #6, #8, #10 |
| 11 | #14 | Enterprise security, resilience and production proof | **Planned / continuous** | all critical phases |

## 5. Cross-cutting architecture 10/10 program

Issue #194 is **Open** and runs alongside product delivery. It is complete only when the measurable criteria in `ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md` are implemented.

### 5.1 Stage state

| Stage | State | Current result / next dependency |
|---|---|---|
| A — documentation and policy baseline | **Complete** | Stable hierarchy, navigation and permanent consistency checks are accepted. |
| B — dependency, crate and exception governance | **Complete for the no-growth prerequisite** | PR #203 freezes root-family direct dependency debt; later debt reduction remains bounded work. |
| C — golden owner package and persistence model | **Complete** | PR #205 accepted the Customer Privacy domain/application/PostgreSQL/production package boundary. |
| D — contribution aggregation | **Planned** | Begin only at a later natural boundary; PR #206 must not become generic-runtime consolidation. |
| E — affected-scope CI | **Partial foundation accepted / planned expansion** | Existing affected-scope proof remains active. |
| F — generic conformance and contract lifecycle | **Planned** | Depends on stable owner package/contribution boundaries. |
| G — transitional consolidation | **Planned** | Begin with one measured cluster only after explicit packet approval. |
| H — reproducible environment and generated navigation | **Planned** | `doctor`, `bootstrap`, `dev-up`, `dev-reset`, `seed-demo`, `smoke`, `explain` and packet tooling remain open. |
| I — frontend and operations parity | **Planned** | Follows stable backend/product delivery paths. |

### 5.2 Accepted architecture evidence

1. PR #197 / merge `dbd7f6646f255b5f654060a045e26f99fc12c1f9` — reproducible workspace/dependency/public-surface/CI baseline and exception governance.
2. PR #199 / accepted source `2335ea00bb73d875c291b4a7668921beaec87adc` / merge `cbcce5f18f3b08851ad781d13bc3fe01c2eeb62c` — business-module dependency inheritance.
3. PR #200 / accepted source `31b3ab09caa4eccaba76a34c7d2211622830115f` / merge `aec7130bd48302d20bf821a617c339b2a9d755cf` — owner privacy-scope adapter dependency inheritance.
4. PR #203 / accepted source `37cec8e2e68c42e85468cea83b31dcf3ba4138d4` / merge `6a445cd4cb9f423561f834fd7f291635f82eb464` — repository-wide root-family no-growth.
5. PR #204 / accepted source `e9a0c1f67d81a3d1f6f6b4504487ac11216edf56` / merge `33186bab67932d5e878019fc7e59181e123bbf67` — Customer Privacy discovery/snapshot freeze.
6. PR #205 / accepted source `18c3e991454241f7ee3b02884345eac462bb6c04` / merge `f0f46238cf103f6e36487f599181e83849342021` — behavior-neutral Customer Privacy golden packages.

Current workspace package count is `113`. PR #206 must keep it unchanged and may add no dependency family, direct version, feature/source drift or unjustified lockfile growth.

### 5.3 Required continuation order

1. Preserve the accepted Stage B no-growth, PR #204 freeze and PR #205 package boundary.
2. Accept PR #206 production discovery and immutable snapshot persistence on one unchanged exact head.
3. Continue deterministic planning and permission-aware plan/outcome reads over the immutable snapshot.
4. Continue restrictions, legal hold/retention, execution, access/export, deletion/anonymization, tombstone and convergence only through later bounded packets.
5. Continue residual architecture calibration only at natural product boundaries unless it blocks correctness.

## 6. Completed foundation

### Phases 0.1–5 — Complete

Repository governance, immutable module identity, governed Module SDK, lifecycle, PostgreSQL tenant/RLS/record/artifact/idempotency/outbox/audit foundations and exact-version capability execution are merged.

### Phase 6 — Complete

Independent Sales and Activities owners, optional governed link, projections and deployable application composition are merged.

### Phase 7 — Complete

Generalized projections, permission-aware search, typed product shell, metadata/Admin Studio and trusted UI-extension isolation are merged.

### Native application-composition integrity — Complete

Issue #134 / PR #135 established module-owned exact-coordinate routing, tenant activation, pre-authorization cross-owner semantics, deterministic worker contributions and production-route parity.

## 7. Phase 8A — canonical customer master and governed customer-data lifecycle

State: **In progress**  
Parent issue: #28

Completed packets:

- **8A.1–8A.6** — customer references, Party, Account, Contact Point, Party Relationship, Customer 360, Consent and reversible Identity Resolution;
- **8A.7** — governed immutable import sources, parsing/validation, resumable Party import and recovery;
- **8A.8** — governed Party export, immutable selection/manifests, deterministic artifacts and recovery;
- **8A.9** — Customer Data Quality Rules, Completeness and Stewardship;
- **8A.10** — Governed Customer Enrichment and Provenance.

### 7.1 Phase 8A.11 Customer Privacy — current boundary

Issue #126 is **In progress**.

Merged public runtime inventory remains:

- four public mutations: `case.create`, `case.submit`, `case.subject.verify`, `case.cancel`;
- two permission-aware queries: `case.get`, `case.list`;
- ten public non-runtime Customer Privacy coordinates;
- zero Customer Privacy workers.

All nine privacy owner-scope implementations are accepted. Their coordinates remain non-public owner-owned reads. PR #206 composes them into trusted-internal production discovery while adding no planning or owner execution runtime.

Accepted owner evidence:

1. Parties — PR #156;
2. Consents — PR #175;
3. Customer Accounts — PR #179;
4. Contact Points — PR #181;
5. Party Relationships — PR #183;
6. Identity Resolution — PR #186 / accepted source `24456b86379a1ef23ed5a60804cdcae5075d407c` / merge `509eb304a76055c9f49b0beed3b007963a91cb22` / 25 of 25 permanent workflows;
7. Customer Data Operations — PR #188 / accepted source `07f34786e82fdfa78d263790e9f50541529006f8` / merge `089be72fa3010b4aa15aff7f9ea55fd86290f8fc` / 26 of 26 permanent workflows;
8. Data Quality — PR #190 / accepted source `dcfe8faebc7462b888f8fc1721cb379a40fea88a` / merge `deac197c97cddc15bb9916092ca87f6e767ce1de` / 27 of 27 permanent workflows;
9. Customer Enrichment — PR #192 / accepted source `e90e36027de18a07be68e43327ea732810ff332a` / merge `e41cbab0cd30819fcbe2e3c5f2c7415fc6de3e8c` / 28 of 28 permanent workflows.

### 7.2 Active sequence

1. **Scope discovery and immutable snapshot contract/acceptance freeze — Complete.**
2. **Stage C behavior-neutral Customer Privacy package pilot — Complete.**
3. **Scope discovery and immutable snapshot runtime implementation — Gate review in PR #206.**
4. **Deterministic planning and permission-aware plan/outcome reads — Next.**
5. **Approval and immediate deny-only restrictions — Planned.**
6. **Legal hold and mandatory-retention precedence — Planned.**
7. **Replay-safe resumable owner execution and crash recovery — Planned.**
8. **Governed access/export and owner-specific deletion/anonymization — Planned.**
9. **Party tombstone, no-orphan proof and projection/search/cache convergence — Planned.**
10. **Worker and complete end-to-end lifecycle acceptance — Planned.**
11. **Phase 8A closure — Blocked on all preceding lifecycle packets.**
12. **Phase 8B — Blocked on the completed Phase 8A baseline.**

### 7.3 Current bounded packet — production scope discovery and immutable snapshot

State: **Gate review in PR #206; implementation complete, merge pending exact-head acceptance**.

The packet implements exact-nine trusted-internal discovery, immutable lineage and durable page evidence, safe deterministic aggregation, strict immutable snapshot persistence/rehydration, replay/crash recovery, permission-aware internal reads and safe audit.

It remains inside:

```text
modules/crm-customer-privacy/
crates/crm-customer-privacy-application/
crates/crm-customer-privacy-postgres/
crates/crm-customer-privacy-production/
```

The workspace remains at 113 packages. No public route or Customer Privacy worker is added. The historical freeze remains unchanged and later implementation evidence is separate.

The accepted guardrail remains: do not modify generic router or worker algorithms merely to register Customer Privacy discovery behavior.

Planning, restrictions, holds, retention decisions, owner execution and Phase 8B remain unimplemented. After merge, the next bounded packet is deterministic planning and permission-aware plan/outcome reads.

## 8. Phase 8B — Product Catalog, Pricing, CPQ and Quote-to-Revenue

State: **Planned; blocked on completed Phase 8A baseline**.

Required independent owner domains include Product Catalog, Price Books/Pricing, CPQ, Orders, Contracts, Subscriptions/Entitlements/Usage and governed billing/ERP/payment/tax/fulfillment boundaries. These domains must not be absorbed into Sales.

## 9. Later expert domains

Planned work includes broader Sales/Activities, omnichannel, Marketing, Service/Knowledge/Field Service, Customer Success, projects/configurable work, documents/e-signature, analytics, workflow/collaboration, governed AI, marketplace and enterprise operational proof.

## 10. Completion rule

Current product-complete expert modules: **0**.

A module, phase or product is not complete merely because a crate, contract, migration or one backend path exists. Completion requires defined domain breadth, governed APIs, persistence, authorization, audit, product workflow, frontend experience and production/operational evidence.
