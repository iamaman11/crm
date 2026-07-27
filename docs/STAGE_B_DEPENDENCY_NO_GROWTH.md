# Stage B Root Dependency No-Growth Closure

Status: **Normative governance boundary**  
Tracking issue: #194  
Product boundary preserved: #126

## Objective

Prevent new direct-dependency debt for every family already declared in root `[workspace.dependencies]` without forcing a repository-wide manifest migration.

This packet is governance-only. It changes no package membership, dependency version, dependency feature resolution, Rust source, contract, route, worker, persistence adapter, database migration or Customer Privacy behavior.

## Accepted debt inventory

The inventory is generated from `cargo metadata --format-version 1 --locked --no-deps` and the manifests of exact workspace members. The Stage B wave-3 baseline contains:

| Root dependency family | Accepted direct/non-inheriting manifests |
|---|---:|
| `prost` | 53 |
| `serde` | 15 |
| `serde_json` | 23 |
| `sha2` | 16 |
| **Total family-manifest entries** | **107** |

The complete sorted path inventory and canonical direct specifications are machine-readable in `workspace-dependency-policy.json`.

## Fail-closed model

The current direct inventory must be an exact subset of the accepted inventory.

Allowed without editing the baseline:

- replacing an accepted direct declaration with clean `workspace = true` inheritance;
- removing an accepted dependency consumer;
- adding a new clean `workspace = true` consumer with no local version, feature or source override.

Blocked by default:

- a new direct consumer of any root dependency family;
- a new or changed direct version requirement;
- direct or inherited feature drift;
- `default-features` drift;
- path, git, registry, branch, revision, tag or package-alias overrides;
- root workspace dependency specification drift;
- an unknown root dependency family not represented by the policy;
- malformed or ambiguous accepted inventory data.

The subset model is monotonic: debt reduction does not require an allowlist edit, while debt growth fails closed.

## Exceptions

The existing `architecture-governance.json` registry is the only exception mechanism. No parallel registry exists.

A dependency exception uses rule `workspace-dependency-no-growth` and exact scope:

```text
<workspace manifest path>:<dependency family>
```

It must include a non-empty id, owner, rule, reason and risk, exact scope, creation date, expiry date, removal condition, compensating checks and tracking issue. Expired, ownerless, incomplete, duplicate or malformed exceptions block governance.

## Permanent enforcement

The existing `scripts/check_workspace_dependency_policy.py --check` command enforces both calibrated inheritance cohorts and the no-growth boundary. It is already invoked by:

- Governance CI;
- Complexity Baseline CI;
- `scripts/check_architecture.py` through permanent structural conformance;
- `python scripts/repo.py conformance`.

The Complexity Baseline artifact reports accepted, current and reduced direct-consumer counts for each root dependency family while preserving package, dependency, fan-out, public-surface and affected-closure reports.

## Focused synthetic proof

`tests/test_workspace_dependency_policy.py` covers:

- new direct consumer rejection;
- direct version drift;
- feature drift;
- source override;
- valid clean workspace inheritance;
- automatic existing-debt reduction;
- a valid exact-scoped exception;
- expired, ownerless and incomplete exceptions;
- unknown dependency families;
- preservation of the two existing calibrated inheritance policies.

## Measured structural effect

| Metric | Before | After |
|---|---:|---:|
| Effective workspace packages | 110 | 110 |
| Technical crates | 96 | 96 |
| Business modules | 13 | 13 |
| Deployable services | 1 | 1 |
| Root dependency families | 4 | 4 |
| Accepted direct-debt entries | 107 | 107 |
| Registered temporary architecture exceptions | 0 | 0 |
| Cargo.lock change | none | none |

Existing dependency, package, fan-out, public-surface and affected-closure measurement remains intact. This packet does not claim that all remaining direct consumers have migrated; it only makes further growth mechanically impossible without an explicit expiring exception.

## Product continuation boundary

Customer Privacy remains in progress. Production discovery and immutable snapshots are not implemented by this packet.

The next product packet is the separate Customer Privacy discovery/snapshot **contract and acceptance freeze**. Runtime discovery begins only after that boundary and the separate behavior-neutral Stage C Customer Privacy golden-package pilot are accepted.
