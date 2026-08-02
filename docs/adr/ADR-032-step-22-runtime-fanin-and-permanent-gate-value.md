# ADR-032: Step 22 requires a runtime fan-in decision and permanent-gate value review

Status: **Accepted when merged**  
Date: **2026-08-02**  
Decision owners: architecture program #194  
Product boundary: Customer Privacy #126

## Context

Repository Steps 12–14 produced real architecture improvements:

- ordinary first-party owner registration moved out of generic native composition and into owner-owned production contributions aggregated by `crm-first-party-modules`;
- current workspace packages reduced from 113 to 112;
- internal dependency edges reduced from 841 to 835;
- conservative public Rust items reduced from 5,379 to 5,377;
- direct package-local lint tables were retired;
- source-level suppressions, central-system fan-in/fan-out, representative change cost and affected-scope closure became measured and blocking against unreviewed growth.

Those controls prevent regression, but non-growth alone is not sufficient evidence that the remaining structure is optimal.

`crm-application-runtime` remains a large process-composition package with a broad direct dependency surface across owner-specific adapters, compositions, projections and process integrations. The current policy correctly freezes that surface, but a frozen broad surface may still be accidental complexity. Step 22 must therefore decide whether the owner-specific fan-in can be safely reduced or whether each retained dependency is an unavoidable stable process-composition boundary.

The repository also has many permanent workflows, jobs, policy checks and generated evidence sources. Each may be valuable, but the architecture program must prevent governance from becoming self-justifying. A permanent gate is warranted only when its prevented failure mode, distinct value, cost and removal condition are explicit.

## Decision

Repository Step 22 is expanded from architecture remeasurement alone into three inseparable closure obligations:

1. exact architecture and change-economics remeasurement;
2. a mechanical `crm-application-runtime` direct-dependency decision;
3. a complete permanent-gate value and cost review.

Mere budget non-growth cannot complete Step 22.

### 1. `crm-application-runtime` dependency decision

Step 22 MUST produce a machine-readable and review-readable inventory of every internal direct dependency of `crm-application-runtime`.

Each dependency MUST be classified as exactly one of:

- `removed` — eliminated from the direct process-composition surface;
- `platform-generic` — a stable generic runtime, contract, SDK, ingress, data or process boundary that is not owned by one business domain;
- `owner-specific-unavoidable` — retained only because it represents an independently necessary process, trust, security, provider, persistence, projection or ownership boundary that cannot be moved without weakening architecture or broadening impact;
- `test-only` — isolated from production dependency accounting and justified by executable acceptance evidence.

No dependency may remain `unclassified`, `temporary`, `legacy` or `review later` when Step 22 closes.

For every owner-specific dependency, Step 22 MUST either:

- remove it or move the required composition behind an existing owner-owned contribution/boundary, with measured reduction in direct fan-in and no broader build/test closure; or
- retain it with exact evidence that it is an unavoidable stable process-composition boundary.

Evidence for `owner-specific-unavoidable` MUST include:

- the concrete process, trust, security, provider, persistence, projection or ownership boundary it protects;
- why moving the dependency into a generic aggregate or owner package would weaken that boundary, hide coupling or broaden reverse impact;
- the exact runtime files and responsibilities that require the dependency;
- proof that an ordinary capability added to the existing owner does not modify `crm-application-runtime/Cargo.toml` or owner-specific runtime composition files;
- proof from representative accepted changes that the dependency does not make ordinary owner work scale with total module count;
- a named owner and a future removal/review condition.

A package name, historical presence or current allowlist entry is not sufficient justification.

### 2. Reduction is preferred; non-growth requires proof

Step 22 MUST remove every dependency classified as safely removable. A report that only confirms the existing maximum direct-dependency budget is insufficient.

The accepted outcome may combine:

- measurable removal of owner-specific direct dependencies; and
- justified retention of the irreducible process-composition set.

If exact analysis finds that no additional safe reduction is possible, Step 22 may retain the measured count only when every owner-specific dependency independently satisfies the `owner-specific-unavoidable` evidence above and no dependency remains undecided.

Step 23 and Step 24 extension waves MUST then prove that new expert-domain work does not require edits to `crm-application-runtime/Cargo.toml` or owner-specific process-composition source. Any such edit reopens the Step 22 decision and blocks Step 25.

### 3. Permanent-gate value ledger

Before Step 22 architecture remeasurement can be accepted, every permanent workflow, job and repository gate MUST appear in one machine-readable ledger with a human-readable report.

For each permanent gate the ledger MUST record:

- stable gate identifier and owning team;
- concrete prevented failure mode;
- scope and authoritative inputs;
- evidence of real defects previously detected, or a specific preventive rationale when no historical defect exists;
- overlap and duplication analysis against other gates;
- execution cost, including duration, runner-minutes where available, fan-out and expensive database/process/browser setup;
- false-positive and operational-maintenance history where measurable;
- decision: `retain`, `simplify`, `merge` or `remove`;
- independent value when an apparently duplicate gate is retained;
- retirement or re-review condition;
- compensating checks required when simplified, merged or removed.

A gate that cannot identify a concrete failure mode MUST not remain permanent.

A duplicate gate MUST be simplified, merged or removed unless it proves an independent failure mode or intentionally independent implementation path whose value exceeds its cost.

### 4. New permanent-gate entry contract

After this decision, every new permanent gate proposal MUST declare before acceptance:

- the concrete failure mode it prevents;
- why existing gates do not already prevent that failure;
- expected affected scope and execution cost;
- named owner;
- false-positive controls;
- evidence emitted on failure and success;
- retirement/review condition.

A new gate MUST NOT be introduced merely to validate another governance mechanism or to duplicate a check under a new name.

Temporary investigation checks remain temporary and MUST be removed before exact-head acceptance unless they independently satisfy the permanent-gate entry contract.

### 5. Step 22 execution order

Repository Steps 15–21 retain their existing order and scope. Step 22 may use multiple bounded PRs, but its internal order is:

1. freeze the exact accepted Step 21 baseline;
2. inventory `crm-application-runtime` production and test-only direct dependencies;
3. inventory all permanent gates and their measured execution cost;
4. perform safe bounded runtime fan-in reductions and safe gate simplification/merge/removal where the evidence is already conclusive;
5. publish justified retained runtime dependencies and retained gates;
6. rerun architecture, change-economics, CI and local-development measurements;
7. synchronize plans, ledgers and issues on one unchanged exact head.

Measurement and remediation SHOULD remain separate when combining them would obscure before/after evidence.

### 6. Step 22 exit evidence

Step 22 is complete only when all of the following are accepted:

1. the same architecture dimensions from the original baseline are remeasured from exact current `main`;
2. every internal direct dependency of `crm-application-runtime` has one final classification;
3. every safely removable owner-specific dependency is removed;
4. every retained owner-specific dependency has unavoidable-boundary evidence and a review/removal condition;
5. ordinary existing-owner capability evidence shows zero generic-runtime manifest and owner-specific process-host edits;
6. every permanent gate has a complete value/cost ledger entry;
7. duplicate or low-value gates are simplified, merged or removed unless independent value is proven;
8. no new permanent gate lacks the required entry contract;
9. focused and affected CI are not broader or more expensive without explicit measured justification;
10. Phase 8A product-readiness evidence remains separate from architecture scoring;
11. all applicable permanent workflows pass on one unchanged meaningful exact head;
12. unresolved runtime fan-in or gate-value decisions equal zero.

## Relationship to Steps 23–25

Step 22 is a checkpoint, not an architecture 10/10 declaration.

Step 23 and Step 24 MUST validate the Step 22 conclusions under two contrasting expert-domain waves. They must measure whether ordinary domain extension:

- creates zero new crates inside an existing owner;
- avoids `crm-application-runtime` manifest and owner-specific source changes;
- avoids unrelated owner-package and workflow fan-out;
- preserves or improves focused CI cost;
- does not introduce governance gates without the permanent-gate entry contract.

Step 25 MUST reopen any Step 22 classification contradicted by those waves. Architecture 10/10 remains blocked while any runtime dependency or permanent gate is unclassified, unjustified or awaiting a known safe simplification.

## Rejected alternatives

### Treat current non-growth budgets as sufficient

Rejected because a broad dependency surface can remain accidental complexity even when it stops growing.

### Require an arbitrary numeric dependency target now

Rejected because package count and direct-dependency count are proxies. Step 22 must first classify real boundaries and remove everything safely removable rather than optimize toward an uninformed number.

### Keep every existing gate because it is already permanent

Rejected because permanence is not evidence of value. Every gate must continue to justify its distinct failure mode and cost.

### Remove apparently duplicate gates without compensating evidence

Rejected because independent implementations may detect different failure modes. Merge or removal requires explicit overlap analysis and compensating checks.

### Perform the review only at final Step 25

Rejected because Steps 23 and 24 must test a hardened, already-reviewed runtime and governance baseline rather than discover avoidable structural debt after the extension waves.

## Consequences

Step 22 becomes a decision and remediation checkpoint, not a passive dashboard refresh. It may require several bounded PRs and may preserve some broad process-composition dependencies, but only with dependency-by-dependency evidence.

The permanent CI surface becomes reviewable as an engineering system with owners, costs and retirement conditions. The intended result is fewer unjustified central dependencies and fewer low-value gates, while retaining every distinct security, ownership, persistence, compatibility and operational failure detector that proves its value.
