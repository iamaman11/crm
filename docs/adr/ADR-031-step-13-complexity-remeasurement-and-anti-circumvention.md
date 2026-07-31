# ADR-031: Repository step 13 starts with complexity remeasurement and anti-circumvention governance

Status: **Accepted when merged**  
Date: **2026-08-01**  
Decision owners: architecture program #194  
Product boundary: Customer Privacy #126

## Context

Repository step 12 and Stage D contribution aggregation are complete. Every active first-party owner now exposes an owner-owned production contribution boundary aggregated through `crm-first-party-modules`, and generic native composition no longer owns ordinary owner registration.

That improvement does not prove that the complete repository is already simple, comprehensible or 10/10. The last detailed workspace complexity baseline was recorded at 110 effective packages, while the current accepted workspace contains 113 packages. The previous baseline also reported high dependency depth, broad reverse impact, a large public Rust surface, a large application runtime and significant CI fan-out. Those measurements remain useful historical evidence but are not current completion evidence.

A post-step-12 review identified additional risks that the existing step-13 wording did not make explicit:

- `crm-application-runtime` still has broad compile-time fan-in across owner-specific adapters and process integrations;
- `crm-first-party-modules` is narrower than the runtime it replaced, but it remains a manually maintained central aggregate;
- high reverse fan-out has different meanings for stable contracts/SDKs and for mutable implementation/composition crates;
- current Rust governance freezes three direct `[lints]` tables, but semantically equivalent source-level suppressions such as `#[allow(...)]`, `#![allow(...)]` and `#[expect(...)]` are not yet governed by the same complete inventory;
- dependency centralization, crate consolidation or additional guards can reduce textual duplication while increasing cognitive load, fan-in, build scope or the number of places a developer must understand.

The architecture program therefore needs to distinguish stronger enforcement from actual simplification. Passing existing repository rules is necessary, but it is not sufficient evidence that the rules themselves are optimal.

## Decision

Repository step 13 remains the next permitted implementation step, but it MUST begin with a bounded current-main remeasurement and governance-calibration packet before any dependency, public-surface, crate or exception refactor.

### 1. Current-main remeasurement is mandatory

The first step-13 packet MUST regenerate machine-readable and review-readable measurements from the exact accepted 113-package `main`, including at minimum:

- effective workspace packages and package categories;
- direct dependency declarations and feature variants;
- maximum dependency depth;
- direct and transitive reverse fan-out;
- direct fan-in of composition and process-host packages;
- conservative public Rust surface by package;
- non-comment LOC for central runtime, composition and infrastructure packages;
- one-consumer and thin-wrapper package candidates;
- representative affected closures;
- workflow count, job count, path-filter count and PostgreSQL/process fan-out;
- normal capability and new-owner change cost in files, packages, central files and required workflows.

Historical 110-package measurements MUST be labelled historical and MUST NOT be used as current completion evidence.

### 2. Equivalent bypass forms are one governance surface

Step 13 MUST inventory and classify all mechanisms that weaken or bypass repository policy, not only the forms already represented in `architecture-governance.json`.

The inventory MUST include, where applicable:

- direct `[lints]` tables;
- `#[allow(...)]` and `#![allow(...)]`;
- `#[expect(...)]`;
- broad `cfg` or feature-gated exclusions;
- skipped or ignored tests;
- compatibility APIs that preserve duplicate central inventories;
- exact-path and prefix allowlists;
- generated or manual module/capability lists;
- dependency/version/feature overrides;
- workflow path-filter exclusions;
- architecture-script exclusions and unclassified paths.

A semantically equivalent bypass MUST NOT escape governance merely because it uses a different syntax. Every retained exception requires exact scope, owner, reason/risk, expiry, removal condition and compensating checks. Unregistered suppressions are blocking after the initial inventory packet establishes the accepted baseline.

### 3. Fan-out and fan-in are classified by role

High reverse fan-out is not automatically a defect when it belongs to a small, stable, infrastructure-neutral contract or SDK boundary. The same fan-out is a stronger risk when it belongs to mutable implementation, persistence or process-composition code.

Step 13 MUST classify central packages into at least:

- stable contract/identifier boundaries;
- governed SDK/port boundaries;
- generic execution runtimes;
- infrastructure implementations;
- owner production boundaries;
- first-party aggregation;
- process host and delivery composition.

Budgets and remediation priorities MUST be role-aware. The program MUST NOT merge stable contract boundaries merely to reduce package count, and MUST NOT excuse implementation fan-out merely because a package is named `core`.

### 4. Simplification budgets precede structural refactors

Before step 13 completes, calibrated warning and blocking budgets MUST exist for:

- new workspace package growth;
- direct owner-specific dependencies of `crm-application-runtime`;
- non-comment LOC growth in generic composition and process-host code;
- reverse impact growth for implementation and composition crates;
- public Rust surface growth;
- ordinary capability changes touching generic runtime, unrelated owners, workflows or migrations;
- files and packages touched by a representative ordinary capability;
- files and packages touched by a representative new owner;
- unregistered lint suppressions and architecture exceptions;
- focused, affected and full CI duration/fan-out regression.

Budgets MUST be based on current measurements and representative changes. A threshold MUST NOT become blocking until its false-positive risk and affected developer workflow are understood.

### 5. Refactors require before/after evidence

Dependency centralization, package consolidation, API grouping and composition changes are accepted only when the exact packet demonstrates that the result is:

- easier to understand;
- cheaper to extend;
- no broader in unrelated build/test impact;
- no weaker in ownership, tenant, RLS, authorization, idempotency, audit or route parity;
- reversible if the measured result worsens.

A refactor that only moves complexity, hides it behind generation or adds more mandatory indirection is rejected.

### 6. Central systems receive explicit review

The first step-13 measurement MUST publish an explicit central-system map covering at least:

- `crm-module-sdk`;
- `crm-core-contracts` and `crm-proto-contracts`;
- `crm-capability-runtime`;
- `crm-query-runtime`;
- `crm-application-composition`;
- `crm-core-data`;
- `crm-first-party-modules`;
- `crm-application-runtime`;
- `services/crm-api`;
- architecture/governance and affected-scope tooling.

For each system the report MUST state its role, direct consumers, transitive reverse impact, dependency direction, public surface, expected stability and current architectural risk.

### 7. Repository order is unchanged

This decision refines step 13; it does not insert a new numbered repository step.

- Steps 1–12 remain complete.
- Step 13 remains next and is not started by this planning packet.
- Step 14 remains blocked until all step-13 implementation and evidence-synchronization packets are accepted.
- Customer Privacy and Phase 8A product readiness remain unchanged and incomplete.
- Architecture 10/10 cannot be claimed before the later measurements and expert-domain proofs already required by the master plan.

## Step 13 exit evidence

Repository step 13 is complete only when all of the following are accepted on unchanged exact heads:

1. a fresh 113-package complexity and dependency baseline;
2. a complete inventory of manifest-level and source-level policy suppressions;
3. mechanical rejection of new unregistered suppressions and equivalent bypasses;
4. role-aware fan-in/fan-out and public-surface budgets;
5. removal of the three currently registered direct lint-table exceptions without replacing them with hidden source-level suppressions;
6. measured reduction or justified non-growth in the central process-host dependency surface;
7. representative ordinary-capability and new-owner change-cost reports;
8. calibrated dependency/version/feature policy with no unmeasured centralization;
9. synchronized architecture plan, project status and issues;
10. no product-behavior, tenant-isolation, authorization, persistence, route or worker regression.

Step 13 MAY use more than one bounded implementation packet. The first packet is measurement and enforcement calibration; later packets may perform behavior-neutral remediation. Measurement and remediation SHOULD remain separate when combining them would obscure before/after evidence.

## Rejected alternatives

### Continue directly from the historical baseline

Rejected because the workspace changed from 110 to 113 packages and Step 12 changed composition boundaries. Historical measurements cannot prove the current state.

### Treat all high fan-out packages as defects

Rejected because stable SDK and contract boundaries are intentionally shared. Role and mutability matter.

### Count only Cargo-level lint exceptions

Rejected because equivalent source attributes can bypass the same rule without appearing in manifest-level governance.

### Generate all central registration automatically

Rejected as a default solution. Generation may reduce manual edits while hiding ownership or making debugging harder. It is acceptable only when authoritative inputs, generated outputs, failure modes and developer navigation are simpler and mechanically proven.

### Consolidate crates first and measure afterward

Rejected because package count alone is not the objective. Consolidation can increase public surface, fan-in, compilation scope and cognitive load.

## Consequences

This decision adds a mandatory measurement packet before step-13 remediation and may delay visible refactors. That cost is accepted because it prevents the architecture program from formalizing accidental complexity or optimizing only the metrics it already knows how to count.

The intended outcome is not the maximum number of rules. It is a system where the correct change requires the fewest reasonable concepts, files and central modifications while retaining the platform's security, ownership and exact-head guarantees.
