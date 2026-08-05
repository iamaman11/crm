# Repository Step 22A — Architecture Remeasurement Inventories

Status: **Active inventory-only checkpoint**  
Tracking issue: #194  
Binding decision: `docs/adr/ADR-032-step-22-runtime-fanin-and-permanent-gate-value.md`  
Exact accepted Phase 8A baseline: `4167bd530b91e3a8fc9bfaaf0d02fcdc1f7a20f3`

## Purpose

This is the first bounded Repository Step 22 slice. It freezes the exact accepted Phase 8A baseline and makes two previously implicit review surfaces reproducible:

1. every internal direct dependency of `crm-application-runtime`, separated by production, dev/test-only and build manifest scope;
2. every permanent GitHub Actions workflow and every declared job under a stable path-based identifier, with deterministic structural cost proxies.

The existing `scripts/analyze_workspace.py` remains the single measurement entry point. Its established `crm.workspace-complexity-baseline/v1` report is extended additively with `step22_inventory`; no existing report field is removed or renamed.

## Reproduction

```bash
python scripts/analyze_workspace.py \
  --json-output artifacts/workspace-complexity-baseline.json \
  --markdown-output artifacts/workspace-complexity-baseline.md \
  --step22-inventory-output artifacts/step22-architecture-inventory.json
```

`Complexity Baseline CI` runs the same analyzer on the exact pull-request head and publishes the JSON and Markdown evidence through its existing artifact. This packet adds no new permanent workflow or repository gate.

## Exact Step 22A inventory measurements

The first accepted analyzer execution on the Step 22A candidate reported:

| Inventory metric | Exact value |
|---|---:|
| `crm-application-runtime` internal direct dependencies | 63 |
| Production internal direct dependencies | 62 |
| Dev/test-only internal direct dependencies | 1 |
| Build internal direct dependencies | 0 |
| Permanent workflows | 41 |
| Declared permanent workflow jobs | 42 |
| Workflow path-filter entries | 1,806 |
| Workflows with PostgreSQL service signals | 31 |
| `crm-application-runtime` non-comment Rust LOC | 7,268 |
| `crm-application-composition` non-comment Rust LOC | 1,239 |
| Maximum transitive reverse impact in workspace graph | 105 |

These values are inventories and source-derived structural proxies. They are not dependency classifications, gate dispositions, runner-minute measurements or an architecture score.

## Machine-readable inventory contract

The committed `step22-architecture-inventory.json` uses schema `crm.step22-architecture-inventory/v1` and records:

- exact materialization source commit;
- stable dependency IDs of the form `crm-application-runtime::<manifest-section>::<package>`;
- dependency manifest scope, target category and target manifest path;
- stable workflow IDs equal to repository workflow paths;
- stable job IDs of the form `<workflow-path>#<job-id>`;
- action-reference count, run-step count, timeout and deterministic environment signals for each workflow and job;
- one explicit global `unresolved` decision state applying to every inventory row;
- explicit false values for final classification, disposition, remediation and Step 22 completion.

A Git commit cannot contain its own future SHA. The ledger therefore records the exact pre-ledger materialization source in `measurement_source_commit`. Permanent tests regenerate the review projection from the current repository and require canonical structural equality after normalizing only that provenance field. All dependency, workflow and job rows must otherwise match exactly.

## Accepted pre-measurement baseline

The prior blocking architecture baseline remains:

| Metric | Accepted value before Step 22A |
|---|---:|
| Workspace packages | 112 |
| Internal dependency edges | 835 |
| Maximum dependency depth | 18 |
| Conservative public Rust items | 5,377 |
| Dependency declarations | 270 |
| Workspace dependency declarations | 4 |
| Heavy-feature declarations | 65 |
| Registered suppression occurrences | 91 |

These values are comparison inputs, not an architecture score and not proof that the retained runtime or gate surface is optimal.

## Explicit decision boundary

This packet does **not**:

- classify a dependency as `removed`, `platform-generic`, `owner-specific-unavoidable` or `test-only` under ADR-032;
- remove or move a dependency;
- assign `retain`, `simplify`, `merge` or `remove` to a workflow, job or repository gate;
- claim measured runner-minutes from timeout or source-text heuristics;
- add, remove or permanently modify a workflow;
- complete Repository Step 22;
- raise an architecture score, start Phase 8B or declare architecture 10/10.

## Remaining Step 22 order

After this inventory-only checkpoint is accepted, later bounded Step 22 packets must:

1. classify every runtime dependency and remove every safely removable owner-specific dependency;
2. complete the permanent workflow, job and repository-gate value/cost ledger with owners, overlap analysis, defect evidence, dispositions and retirement conditions;
3. implement only evidence-backed runtime fan-in reductions and gate simplifications;
4. remeasure architecture, change economics, CI and local-development outcomes;
5. synchronize final Step 22 decisions and evidence on one unchanged exact head.

Unresolved runtime-fan-in classifications and unresolved gate-value decisions intentionally remain non-zero after Step 22A. Issue #194 stays open and architecture 10/10 remains undeclared.
