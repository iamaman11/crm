# Repository Step 22B — Runtime Fan-In Classification Cohort

Status: **Active bounded classification packet**  
Tracking issue: #194  
Binding decision: `docs/adr/ADR-032-step-22-runtime-fanin-and-permanent-gate-value.md`  
Accepted inventory baseline: PR #298 source `ffb8c94373c565de00cccd67c38c80bdb3a12405`, squash merge `4642ea39a7c1c8ad78b1d475a3d5391af8414555`

## Purpose

Step 22A made all 63 internal direct dependencies of `crm-application-runtime` reproducible. Step 22B records only classifications that are already conclusive from stable package role and manifest scope:

- generic platform/runtime/contract/SDK/query/search boundaries that are independent of one business owner;
- the single dependency already isolated under `[dev-dependencies]`.

This packet intentionally does not classify an owner-specific dependency as unavoidable. ADR-032 requires stronger package-by-package evidence for that decision, including exact runtime responsibility, ordinary-owner change-cost proof, a named owner and a future review/removal condition. Those decisions remain separate so that evidence is not replaced by package-name inference.

## Accepted Step 22A source

`step22-runtime-fanin-decisions.json` is joined mechanically to `step22-architecture-inventory.json` and freezes:

| Metric | Exact value |
|---|---:|
| Inventory dependencies | 63 |
| Final classifications in this packet | 17 |
| `platform-generic` | 16 |
| `test-only` | 1 |
| `removed` | 0 |
| `owner-specific-unavoidable` | 0 |
| Remaining unresolved | 46 |

Every accepted inventory stable ID not present in `final_rows` remains unresolved. The ledger does not use `unclassified`, `temporary`, `legacy` or `review later` as substitute classifications.

## Final platform-generic cohort

The accepted generic boundaries are:

1. generic process composition — `crm-application-composition`;
2. first-party owner contribution aggregation — `crm-first-party-modules`;
3. generic capability platform — `crm-capability-adapters`, `crm-capability-ingress`, `crm-capability-plan-support`, `crm-capability-runtime`;
4. generic data and event mechanics — `crm-core-data`, `crm-core-events`;
5. cross-owner global search composition — `crm-global-search-composition`;
6. governed metadata adapters — `crm-metadata-api-adapter`, `crm-metadata-query-adapter`;
7. first-party module SDK — `crm-module-sdk`;
8. generated shared contracts — `crm-proto-contracts`;
9. generic permission-aware query runtime — `crm-query-runtime`;
10. generic search execution — `crm-search-query-adapter`, `crm-search-runtime`.

Each cohort entry references a machine-readable boundary definition with a named owner, concrete protected boundary, authoritative evidence paths and a re-review condition.

## Final test-only cohort

`crm-application-runtime::dev-dependencies::crm-consents` is classified `test-only` because it is declared only under `[dev-dependencies]` and is excluded from production fan-in accounting. Its removal condition is explicit: remove the direct dependency when acceptance fixtures no longer construct the Consents owner module directly.

## Governance registration

The decision surface is registered once in `architecture-governance.json` as `repository-step-22-runtime-fanin` with:

- owner `architecture-governance`;
- tracking issue `#194`;
- canonical ledger `step22-runtime-fanin-decisions.json`;
- validator `scripts/check_step22_runtime_fanin_decisions.py`;
- a continuing review condition on every accepted classification or remediation packet until Step 22 closure.

The registration uses the existing Governance CI surface. No workflow, job, path topology or permanent-gate disposition is added or changed by this packet.

## Mechanical enforcement

Run:

```bash
python scripts/check_step22_runtime_fanin_decisions.py
```

The validator fails closed when:

- the accepted Step 22A source or merge commit changes;
- the architecture-governance registration is absent, duplicated or differs from the canonical owner/path/validator/issue/review contract;
- a final stable ID is missing from the accepted inventory or appears twice;
- the ADR-032 classification enum changes;
- a boundary lacks an owner, protected-boundary statement, evidence paths or review condition;
- an evidence path does not exist;
- `platform-generic` targets a business module or a non-production dependency;
- `test-only` is not isolated in the accepted dev/test inventory;
- this packet attempts to record `removed` or `owner-specific-unavoidable`;
- counts differ from the exact 17 final / 46 unresolved decision state;
- remediation, gate dispositions, complete classification or Step 22 closure are claimed.

The permanent architecture conformance tests call the same validator. This packet adds no workflow or repository gate.

## Explicit decision boundary

This packet does **not**:

- remove, move or add a dependency;
- edit `crm-application-runtime/Cargo.toml` or runtime source;
- claim that any owner-specific dependency is unavoidable;
- assign `retain`, `simplify`, `merge` or `remove` to a workflow or job;
- complete the permanent-gate value ledger;
- complete Repository Step 22;
- start Phase 8B or declare architecture 10/10.

## Next Step 22 slices

The remaining 46 production dependencies require bounded owner/process cohorts. Each cohort must either produce safe measured remediation or satisfy every ADR-032 `owner-specific-unavoidable` evidence field. Permanent-gate value/cost decisions and any workflow simplification remain separate packets. Step 22 closure remains blocked until unresolved runtime and gate decisions both equal zero and final remeasurement is accepted.
