# Rust Toolchain, rust-version and Lint Baseline

Status: **repository step 1 implementation candidate**  
Tracking issue: #194  
Repository order: `ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md` section 2.4

## 1. Decision

The repository supports one exact Rust boundary:

| Property | Policy |
|---|---|
| Developer and CI toolchain | `1.97.1` |
| Root workspace `rust-version` | `1.97.1` |
| Edition | `2024` |
| Cargo resolver | `2`, unchanged |
| rustup profile | `minimal` |
| Required components | `clippy`, `rustfmt` |
| Workspace lint baseline | Rust `warnings = "deny"` |

`rust-toolchain.toml`, `[workspace.package].rust-version` and `rust-governance-policy.json` must agree. CI also verifies the executing compiler release rather than trusting configuration text alone.

The `rust-version` intentionally equals the exact tested toolchain. Rust 1.85 made Edition 2024 available, but edition availability is not proof that the complete locked workspace and its resolved dependencies support Rust 1.85. A lower MSRV may be published only by a separate packet that compiles and tests the complete applicable locked workspace on that exact compiler.

## 2. Measured lint baseline

Permanent Rust CI executes both measurements on the unchanged pull-request head:

```bash
cargo check --workspace --all-targets --all-features --message-format=json
cargo clippy --workspace --all-targets --all-features --message-format=json -- -D warnings
```

The accepted budget is:

- workspace Rust warnings: `0`;
- workspace Rust errors: `0`;
- workspace Clippy warnings: `0`;
- workspace Clippy errors: `0`.

The checker counts only compiler messages whose Cargo package IDs belong to the 113 effective workspace packages. Transitive dependency diagnostics remain visible in the raw artifacts but do not masquerade as local warning debt.

CI publishes:

- raw Rust JSON diagnostics and stderr;
- raw Clippy JSON diagnostics and stderr;
- a machine-readable Rust governance report;
- a human-readable Rust governance report.

The existing full workspace test remains mandatory after the measurements.

## 3. Adoption baseline

The root policy is introduced without editing 113 package manifests in one broad mechanical change.

Current measured cohort:

| Metric | Baseline |
|---|---:|
| Effective workspace packages | 113 |
| Packages inheriting `rust-version` | 0 |
| Packages missing inherited `rust-version` | 113 |
| Direct package `rust-version` overrides | 0 |
| Packages inheriting workspace lints | 0 |
| Packages missing inherited workspace lints | 113 |
| Direct package lint tables | 0 |
| Active Rust-governance exceptions | 0 |

Rules from this point forward:

1. every new workspace package must use `rust-version.workspace = true` and `[lints] workspace = true`;
2. direct package overrides are forbidden unless an exact manifest has a complete, active and time-bounded `rust-governance` exception in `architecture-governance.json`;
3. the legacy missing-inheritance cohort may only shrink;
4. migration of existing packages must use homogeneous behavior-neutral packets rather than a repository-wide manifest rewrite;
5. changing the package count requires updating the measured policy baseline in the same reviewed packet.

## 4. Permanent enforcement

`scripts/check_rust_governance.py` validates:

- exact toolchain, profile and required components;
- exact agreement between the policy, root Cargo workspace and rustup toolchain file;
- unchanged resolver policy;
- exact workspace lint table;
- workspace package count and inheritance cohorts;
- absence of unauthorized direct overrides;
- required inheritance for newly added workspace members;
- exact, non-expired Rust-governance exception scopes;
- executing compiler version;
- zero-warning and zero-error measurement budgets.

The structural subset is part of `scripts/check_architecture.py`. Full compiler/lint measurements and exact compiler verification are part of permanent Rust CI.

## 5. Boundaries

This packet intentionally adds no:

- dependency or feature upgrade;
- `Cargo.lock` resolution change;
- workspace package;
- product behavior, capability, route, query or worker;
- Customer Privacy approval behavior;
- generic runtime registration or dispatch change;
- broad package-manifest migration;
- resolver 3 adoption.

Resolver 3 remains a separate measured dependency-resolution decision because it can affect lockfile semantics and minimum supported Rust behavior.

## 6. Acceptance and rollback

Acceptance requires all applicable permanent workflows to succeed on one unchanged exact source head. The Rust governance artifact must report Rust `1.97.1`, 113 workspace packages, zero measured warnings/errors and zero blocking policy errors.

Rollback removes the root Rust policy, restores the previous moving `stable` toolchain and removes the checker/CI measurements. No schema, data, contract, runtime or lockfile rollback is required.

After unchanged exact-head acceptance and merge, a separate evidence synchronization change may mark repository step 1 complete and release repository step 2: Customer Privacy approval runtime only.
