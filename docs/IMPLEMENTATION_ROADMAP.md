# Ultimate CRM — Implementation Roadmap

Status: **Normative delivery plan**

Parent epic: #2  
Governing rules: `SYSTEM_INVARIANTS.md`  
Delivery-control policy: `DELIVERY_GOVERNANCE.md`  
Current concise state: `PROJECT_STATUS.md`  
Detailed Phase 8 sequence: `PHASE8_DELIVERY_PLAN.md`  
Architecture/developer-experience program and repository order: `ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md` / issue #194  
Step 22 decision: `adr/ADR-032-step-22-runtime-fanin-and-permanent-gate-value.md`  
Accepted Rust boundary: `RUST_TOOLCHAIN_AND_LINT_BASELINE.md` / `rust-governance-policy.json`  
Functional completeness guardrail: `CRM_CAPABILITY_COVERAGE.md`  
Business-module accounting: `MODULE_CATALOG.md`

## 1. Purpose and delivery rules

This roadmap defines dependency order for a universal modular expert CRM platform. A phase or packet is complete only when implemented, merged and backed by unchanged exact-head evidence.

1. Preserve one authoritative owner for every mutable aggregate.
2. Enter state-changing behavior through exact versioned capabilities with typed audit evidence.
3. Never access another module's storage or internals directly.
4. Treat security, privacy, tenant isolation, rollback and operations as implementation requirements.
5. Require real composition, persistence and process evidence before runtime claims.
6. Invalidate old exact-SHA evidence after every source or documentation change.
7. Synchronize roadmap, phase plan, status, catalog, issues and pull-request evidence.
8. An ordinary capability added to an existing owner creates zero new crates by default.
9. Generic router and worker algorithms do not change merely to register one owner capability.
10. Feature behavior and physical crate consolidation remain separate packets.
11. Repository implementation is strictly sequential: only the first unfinished item in the architecture plan section 2.4 may start.
12. Only one implementation packet may be active; evidence synchronization closes the accepted packet before the next implementation begins.
13. Only merged `main` work may be represented as complete.
14. A new permanent gate requires a concrete failure mode, non-duplication rationale, expected cost, named owner and retirement/review condition before acceptance.
15. Repository Step 22 cannot close on non-growth alone; ADR-032 runtime fan-in and permanent-gate decisions are mandatory.

## 2. Product phase map

| Phase | Issue | Primary result | State |
|---|---:|---|---|
| 0.1–7 | #3–#10 | Governed platform, Sales/Activities proof, search, product shell and native composition | **Complete** |
| 8 | #11 | Expert modules and product-quality CRM experience | **In progress** |
| 8A | #28 | Canonical customer master, identity, consent and governed customer-data lifecycle | **In progress** |
| 8B | #29 | Product Catalog, CPQ and quote-to-revenue lifecycle | **Planned; blocked on completed Phase 8A and Repository Step 22 runtime-fan-in/gate-value decisions; first extension wave is Step 23** |
| 9 | #12 | AI-native governed actor/tool layer | **Planned** |
| 10 | #13 | Signed marketplace and sandboxed untrusted extensions | **Planned** |
| 11 | #14 | Enterprise security, resilience and production proof | **Planned / continuous** |

## 3. Architecture and repository program

Issue #194 remains open. The architecture stage ledger is:

- Stage A documentation/navigation baseline — **Complete**;
- Stage B dependency, crate and exception governance — **Complete through PRs #253, #255 and #257; ADR-032 Step 22 decision remains future closure evidence**;
- Stage C Customer Privacy golden owner and persistence model — **In progress**;
- Stage D contribution aggregation — **Complete through PR #249**;
- Stage E affected-scope CI — **Complete through PR #239; permanent-gate value review remains mandatory at Step 22**;
- Stage F generic conformance and contract lifecycle — **In progress**;
- Stage G transitional consolidation — **Complete through PR #259**;
- Stage H reproducible environment and navigation — **In progress**;
- Stage I frontend and operations parity — **Incomplete**.

Repository Steps 1–14 are complete. Repository Step 15 is the next permitted implementation step and is **not started**.

### 3.1 Accepted Repository Step 14 closure

PR #259 / accepted source `8aa0b33c6609e74f98363071c6e7c44ec59fc098` / squash merge `2b0b558077c444d44691371c8a2bcca2c14ae426` / 36 of 36 applicable permanent workflows on one unchanged meaningful user-authored exact head completes Repository Step 14 and Stage G.

The behavior-neutral Customer Accounts consolidation:

- removes `crm-customer-accounts-capability-composition`;
- moves its production contribution into `crm-customer-accounts-query-adapter`;
- preserves mutation planning in `crm-customer-accounts-capability-adapter`;
- preserves exact mutation/query inventory and contribution ordering through `crm-first-party-modules`;
- changes no public route, coordinate, contract, Protobuf schema, database schema, migration, persistence behavior or worker lifecycle;
- preserves tenant activation, FORCE RLS, live authorization, Party-reference validation, idempotency, audit and transaction semantics;
- synchronizes Approval, Discovery and Planning workflow package assertions from the historical 113-package baseline to the accepted 112-package workspace while retaining full behavioral, rollback and reapply checks.

Exact current metrics:

| Metric | Step 13 historical baseline | Current after Step 14 |
|---|---:|---:|
| Workspace packages | 113 | **112** |
| Internal dependency edges | 841 | **835** |
| Maximum dependency depth | 18 | **18** |
| Conservative public Rust items | 5,379 | **5,377** |
| Dependency declarations | 270 | **270** |
| Suppression occurrences | 91 | **91** |

This architecture reduction does not advance Customer Privacy product readiness and does not declare architecture 10/10.

### 3.2 Binding next repository sequence

The complete order is normative in the architecture plan. The remaining sequence begins:

15. Party tombstone, no-orphan proof and projection/search/cache convergence — **next, not started**;
16. reusable generic worker conformance;
17. contract compatibility, deprecation, consumer-migration and retirement enforcement;
18. deterministic local lifecycle commands;
19. Customer Privacy worker and full process/end-to-end acceptance;
20. Phase 8A frontend, accessibility, browser and operations evidence;
21. Phase 8A closure;
22. Phase 8A architecture remeasurement, `crm-application-runtime` direct-dependency decision and permanent-gate value/cost review — checkpoint, not final 10/10;
23. Step 23 — first contrasting later expert-domain wave validating bounded extension cost and the Step 22 runtime/gate conclusions;
24. Step 24 — second contrasting later expert-domain wave validating bounded extension cost and the Step 22 runtime/gate conclusions;
25. final architecture 10/10 closure review only when every criterion is mechanically proven.

No later packet may start while an earlier item remains unfinished.

## 4. Phase 8A completed foundation

Completed product slices:

- **8A.1–8A.6** — customer references, Party, Account, Contact Point, Party Relationship, Customer 360, Consent and reversible Identity Resolution;
- **8A.7** — governed immutable import and recovery;
- **8A.8** — governed deterministic export and recovery;
- **8A.9** — Customer Data Quality Rules, Completeness and Stewardship;
- **8A.10** — Governed Customer Enrichment and Provenance.

All nine Customer Privacy owner-scope implementations are accepted: Parties, Consents, Customer Accounts, Contact Points, Party Relationships, Identity Resolution, Customer Data Operations, Data Quality and Customer Enrichment.

Customer Data Operations owner-scope evidence is accepted through PR #188 / accepted source `07f34786e82fdfa78d263790e9f50541529006f8` / squash merge `089be72fa3010b4aa15aff7f9ea55fd86290f8fc` / 26 of 26 permanent workflows.

Data Quality owner-scope evidence is accepted through PR #190 / accepted source `dcfe8faebc7462b888f8fc1721cb379a40fea88a` / squash merge `deac197c97cddc15bb9916092ca87f6e767ce1de` / 27 of 27 permanent workflows.

Customer Enrichment owner-scope evidence is accepted through PR #192 / accepted source `e90e36027de18a07be68e43327ea732810ff332a` / squash merge `e41cbab0cd30819fcbe2e3c5f2c7415fc6de3e8c` / 28 of 28 permanent workflows.

Scope discovery and immutable snapshot execution is accepted through PR #206 / accepted source `086b17a95058eee285fcb67a903bd21d9263d357` / squash merge `95818fd3aeb54a9593a45642583f0b7224d5ecfe`.

## 5. Phase 8A.11 Customer Privacy — current boundary

Issue #126 remains **In progress**.

Accepted runtime and architecture evidence includes:

- scope discovery and immutable snapshot execution accepted through PR #206;
- deterministic planning through PR #209;
- permission-aware plan and outcome reads through PR #211;
- public approval through PR #220;
- final customer-subject policy prerequisite through PR #224;
- public restriction placement and first protected-owner enforcement through PR #226;
- public legal-hold placement and mandatory-retention precedence through PR #230;
- reusable mutation/query conformance through PR #235;
- durable replay-safe owner execution, checkpoints and real outcomes through PR #237;
- multi-plane affected-scope enforcement through PR #239;
- governed access/export assembly through PR #241;
- authoritative exact-nine owner-specific anonymization/deletion through PR #244;
- complete first-party contribution aggregation through PR #249;
- architecture governance closure through PR #257;
- first measured transitional consolidation through PR #259.

Current public Customer Privacy inventory remains:

- seven public mutations;
- four permission-aware public queries;
- zero Customer Privacy workers.

Trusted-internal planning, retention evaluation, replay-safe owner execution, access/export assembly and exact-nine owner-action execution remain non-public.

### 5.1 Remaining Phase 8A.11 product work

The remaining product work is not satisfied by Step 14:

- restriction and legal-hold release/read lifecycle where required;
- Party tombstone and no-orphan semantics;
- projection/search/cache convergence;
- reusable worker conformance and a real Customer Privacy worker lifecycle;
- disable/uninstall fail-closed semantics;
- frontend, accessibility and browser acceptance;
- production restore, SLO, observability, performance, security and supply-chain evidence.

Repository Step 15 owns only the first bounded tombstone/no-orphan/convergence packet. It must not absorb Steps 16–21.

## 6. Module-readiness accounting

Current product-complete expert modules: **0**.

A module is not product-complete merely because a crate, schema, manifest or backend path exists. Product complete requires:

- sufficient domain breadth;
- governed APIs and contracts;
- authoritative persistence and migration ownership;
- tenant activation and live authorization;
- audit, idempotency and operational evidence;
- product UX, accessibility and browser acceptance;
- production restore, SLO, performance and security proof.

Phase 8A remains incomplete until these criteria are met for the current customer-master/privacy scope.

## 7. Phase 8B and later product domains

Phase 8B remains planned and blocked on completed Phase 8A plus the Step 22 measurement and decision checkpoint. Step 23 must prove that a first later expert-domain wave keeps extension cost bounded, avoids owner-specific `crm-application-runtime` edits and does not add an unjustified permanent gate. Step 24 must add a contrasting expert-domain wave and prove that the same properties remain true as module count grows.

Independent owner domains still planned or incomplete include:

- Product Catalog and variants;
- Pricing and promotions;
- CPQ and Quotes;
- Orders and fulfillment coordination;
- Contracts;
- Subscriptions, Entitlements and Usage;
- Billing plus governed ERP/payment/tax integration;
- expanded Sales, Activities, Service, Knowledge, Field Service, Marketing and Customer Success;
- projects/configurable work, documents/e-signature, analytics, workflow/collaboration, AI governance and marketplace capabilities.

## 8. Repository Step 22 decision boundary

Repository Step 22 is a measurement and remediation checkpoint, not an automatic completion declaration.

Before Step 22 can close:

- every internal direct dependency of `crm-application-runtime` must be classified as removed, platform-generic, owner-specific-unavoidable or test-only;
- every safely removable owner-specific dependency must be removed;
- each retained owner-specific dependency must prove an unavoidable stable process-composition boundary and prove that ordinary owner changes do not modify the runtime manifest or owner-specific runtime source;
- mere non-growth against the current direct-dependency budget is insufficient;
- every permanent workflow, job and gate must record its concrete failure mode, observed defect evidence or preventive rationale, duplication/overlap, execution cost, owner, retain/simplify/merge/remove decision and retirement condition;
- duplicate or low-value gates must be simplified, merged or removed unless independent value is proven;
- every new permanent gate must satisfy the same entry contract.

The complete binding rules are in ADR-032.

## 9. Architecture 10/10 declaration boundary

Architecture 10/10 remains reserved for Step 25 after:

- Steps 15–21 close their mechanical criteria;
- Phase 8A is complete;
- Step 22 leaves zero unresolved runtime-fan-in or permanent-gate value decisions;
- two contrasting later expert-domain waves at Steps 23 and 24 prove bounded extension cost and validate the Step 22 decisions;
- dependency, package, public-surface, change-locality, CI, local-development, contract-lifecycle, frontend and operations measurements show no regression;
- every final criterion in the architecture plan is mechanically reproduced.

Until then issue #194 remains open, Phase 8A and Customer Privacy remain incomplete, and current product-complete expert modules remain **0**.
