#!/usr/bin/env python3
"""Create governed CRM owner/link module scaffolding without weakening repository boundaries."""

from __future__ import annotations

import argparse
from dataclasses import dataclass
from pathlib import Path
import re
import sys

ROOT = Path(__file__).resolve().parents[1]
MODULE_ID_RE = re.compile(r"^[a-z][a-z0-9]*(?:[.-][a-z][a-z0-9]*)+$")
NAMESPACED_ID_RE = re.compile(r"^[a-z][a-z0-9]*(?:[._-][a-z][a-z0-9]*)+$")
TEAM_RE = re.compile(r"^[a-z][a-z0-9_-]{1,63}$")
DEPENDENCY_RE = re.compile(
    r"^(?P<module>[a-z][a-z0-9]*(?:[.-][a-z][a-z0-9]*)+)@(?P<range>.+)$"
)
VERSION_RANGE_RE = re.compile(r"^[0-9A-Za-z.*<>=~^| ,+-]{1,120}$")


class ScaffoldError(ValueError):
    """A user-correctable scaffolding error."""


@dataclass(frozen=True)
class Dependency:
    module_id: str
    version_range: str


@dataclass(frozen=True)
class ModuleSpec:
    kind: str
    module_id: str
    display_name: str
    team: str
    contact: str
    objects: tuple[str, ...]
    required_dependencies: tuple[Dependency, ...]

    @property
    def crate_name(self) -> str:
        return self.module_id.replace(".", "-")

    @property
    def rust_crate_name(self) -> str:
        return self.crate_name.replace("-", "_")

    @property
    def composition_crate_name(self) -> str:
        return f"{self.crate_name}-composition"

    @property
    def relative_dir(self) -> Path:
        return Path("modules") / self.crate_name

    @property
    def composition_relative_dir(self) -> Path:
        return Path("crates") / self.composition_crate_name


def _quoted(value: str) -> str:
    return '"' + value.replace("\\", "\\\\").replace('"', '\\"') + '"'


def parse_dependency(value: str) -> Dependency:
    match = DEPENDENCY_RE.fullmatch(value)
    if not match:
        raise argparse.ArgumentTypeError("dependency must be MODULE_ID@VERSION_RANGE")
    version_range = match.group("range")
    if not VERSION_RANGE_RE.fullmatch(version_range):
        raise argparse.ArgumentTypeError(
            "dependency version range contains unsupported characters or exceeds 120 characters"
        )
    return Dependency(match.group("module"), version_range)


def validate_spec(spec: ModuleSpec) -> None:
    if spec.kind not in {"owner", "link"}:
        raise ScaffoldError(f"unsupported module kind: {spec.kind}")
    if not MODULE_ID_RE.fullmatch(spec.module_id):
        raise ScaffoldError(f"invalid module_id: {spec.module_id}")
    if not TEAM_RE.fullmatch(spec.team):
        raise ScaffoldError(f"invalid team identifier: {spec.team}")
    if "@" not in spec.contact or spec.contact.startswith("@") or spec.contact.endswith("@"):
        raise ScaffoldError("contact must be an email address")
    if spec.kind == "owner" and not spec.objects:
        raise ScaffoldError("owner modules require at least one --object ownership declaration")
    if spec.kind == "link" and spec.objects:
        raise ScaffoldError("link modules cannot declare authoritative --object ownership")
    for object_id in spec.objects:
        if not NAMESPACED_ID_RE.fullmatch(object_id):
            raise ScaffoldError(f"invalid object id: {object_id}")
        if not object_id.startswith(spec.module_id.removeprefix("crm.") + "."):
            raise ScaffoldError(
                f"object '{object_id}' must use the owner namespace "
                f"'{spec.module_id.removeprefix('crm.')}.*'"
            )
    dependency_ids = [item.module_id for item in spec.required_dependencies]
    if len(dependency_ids) != len(set(dependency_ids)):
        raise ScaffoldError("duplicate required module dependency")
    if spec.module_id in dependency_ids:
        raise ScaffoldError("module cannot depend on itself")
    for dependency in spec.required_dependencies:
        if not MODULE_ID_RE.fullmatch(dependency.module_id):
            raise ScaffoldError(f"invalid dependency module_id: {dependency.module_id}")
        if not VERSION_RANGE_RE.fullmatch(dependency.version_range):
            raise ScaffoldError(
                f"invalid version range for dependency {dependency.module_id}: "
                f"{dependency.version_range}"
            )
    if spec.kind == "link" and len(spec.required_dependencies) < 2:
        raise ScaffoldError("link modules require at least two --requires dependencies")


def render_cargo_toml(spec: ModuleSpec) -> str:
    return f'''[package]
name = "{spec.crate_name}"
version = "0.1.0"
edition = "2024"
publish = false

[dependencies]
crm-core-contracts = {{ path = "../../crates/crm-core-contracts" }}
crm-module-sdk = {{ path = "../../crates/crm-module-sdk" }}
'''


def render_composition_cargo_toml(spec: ModuleSpec) -> str:
    return f'''[package]
name = "{spec.composition_crate_name}"
version = "0.1.0"
edition = "2024"
publish = false

[dependencies]
crm-application-composition = {{ path = "../crm-application-composition" }}
crm-module-sdk = {{ path = "../crm-module-sdk" }}
{spec.crate_name} = {{ path = "../../{spec.relative_dir.as_posix()}" }}
'''


def render_composition_lib_rs(spec: ModuleSpec) -> str:
    return f'''#![forbid(unsafe_code)]

use crm_application_composition::ModuleContributionSet;
use crm_module_sdk::{{ErrorCategory, SdkError}};

/// Stable identity for the separately owned production-composition boundary.
pub const CRATE_NAME: &str = "{spec.composition_crate_name}";
/// The only business module this composition boundary may register.
pub const MODULE_ID: &str = {spec.rust_crate_name}::MODULE_ID;

/// Fail-closed scaffold entrypoint. Replace this function with exact route and
/// worker contributions before wiring the module into a production process.
pub fn contribute_to(_contributions: &mut ModuleContributionSet) -> Result<(), SdkError> {{
    Err(SdkError::new(
        "MODULE_PRODUCTION_CONTRIBUTION_NOT_IMPLEMENTED",
        ErrorCategory::Internal,
        false,
        "The generated module has no reviewed production contribution yet.",
    )
    .with_internal_reference(MODULE_ID))
}}

#[cfg(test)]
mod tests {{
    use super::*;

    #[test]
    fn scaffold_boundary_is_fail_closed() {{
        let mut contributions = ModuleContributionSet::new();
        let error = contribute_to(&mut contributions).unwrap_err();
        assert_eq!(error.code, "MODULE_PRODUCTION_CONTRIBUTION_NOT_IMPLEMENTED");
        assert_eq!(MODULE_ID, "{spec.module_id}");
    }}
}}
'''


def render_composition_readme(spec: ModuleSpec) -> str:
    return f'''# Production composition boundary for `{spec.module_id}`

This separately owned crate is the only generated location allowed to translate reviewed module contracts and adapters into `ModuleContributionSet` registrations. The scaffold is deliberately fail-closed: `contribute_to` returns `MODULE_PRODUCTION_CONTRIBUTION_NOT_IMPLEMENTED` until exact mutation/query routes and background workers have production implementations and acceptance evidence.

Do not add domain behavior, SQL, transport handling or another module's internals here. Keep business rules in `{spec.relative_dir.as_posix()}` and infrastructure in narrow adapter crates.
'''


def _render_dependencies(items: tuple[Dependency, ...]) -> str:
    if not items:
        return "  required: []"
    lines = ["  required:"]
    for item in items:
        lines.extend(
            [
                f"    - module_id: {item.module_id}",
                f"      version_range: {_quoted(item.version_range)}",
            ]
        )
    return "\n".join(lines)


def _render_list(key: str, values: tuple[str, ...], indent: int = 4) -> str:
    prefix = " " * indent
    if not values:
        return f"{prefix}{key}: []"
    lines = [f"{prefix}{key}:"]
    lines.extend(f"{prefix}  - {value}" for value in values)
    return "\n".join(lines)


def render_manifest(spec: ModuleSpec) -> str:
    retained = spec.objects if spec.kind == "owner" else ()
    uninstall_policy = (
        "retain_business_records" if spec.kind == "owner" else "delete_private_state"
    )
    description = (
        "Generated owner-module foundation."
        if spec.kind == "owner"
        else "Generated optional cross-domain link-module foundation."
    )
    return f'''schema_version: crm.module/v1
module_id: {spec.module_id}
version: "0.1.0"
display_name: {_quoted(spec.display_name)}
description: {_quoted(description)}

owner:
  team: {spec.team}
  contact: {_quoted(spec.contact)}
  codeowners:
    - "@iamaman11"

runtime:
  kind: in_process
  entrypoint: {spec.relative_dir.as_posix()}

platform:
  minimum_version: "0.1.0"

dependencies:
{_render_dependencies(spec.required_dependencies)}
  optional: []
  conflicts: []

provides:
  capabilities: []
  events: []
{_render_list("objects", spec.objects, indent=2)}
  ui_extensions: []

consumes:
  capabilities: []
  events: []

storage:
{_render_list("record_types", spec.objects, indent=2)}
  private_state_namespaces:
    - {spec.module_id}

security:
  data_classes:
    - internal
  network_egress: []
  secret_handles: []

lifecycle:
  upgrade_policy: manual
  rollback_policy: supported
  uninstall_policy: {uninstall_policy}
  migrations_path: {spec.relative_dir.as_posix()}/migrations
{_render_list("retained_record_types", retained, indent=2)}
'''


def render_lib_rs(spec: ModuleSpec) -> str:
    return f'''#![forbid(unsafe_code)]

/// Stable crate identity for repository tooling.
pub const CRATE_NAME: &str = "{spec.crate_name}";
/// Immutable governed module identity.
pub const MODULE_ID: &str = "{spec.module_id}";

#[cfg(test)]
mod tests {{
    use super::*;

    #[test]
    fn scaffold_identity_is_explicit() {{
        assert_eq!(CRATE_NAME, "{spec.crate_name}");
        assert_eq!(MODULE_ID, "{spec.module_id}");
    }}
}}
'''


def render_readme(spec: ModuleSpec) -> str:
    role = "authoritative owner" if spec.kind == "owner" else "optional cross-domain link"
    return f'''# {spec.display_name}

Generated **{role} module foundation** for `{spec.module_id}`.

This scaffold is intentionally not a production feature. Before raising readiness beyond `Foundation`, complete the explicit gates in `ACCEPTANCE.md` and replace the contract, adapter and acceptance-test placeholders with reviewed implementation and evidence.

Direct PostgreSQL, broker, arbitrary HTTP, secret-store, LLM-provider and cross-module internal dependencies are forbidden.
'''


def render_contracts_placeholder(spec: ModuleSpec) -> str:
    return f'''# Published contract placeholder for `{spec.module_id}`

This directory is an explicit **TODO boundary**, not a published contract.

Before adding behavior that crosses the module boundary:

- define compatible versioned Protobuf commands, queries and/or events under the canonical repository `proto/` source tree;
- bind every published coordinate to `{spec.module_id}` in the governed contract registry;
- preserve backward compatibility and run Contract CI;
- keep public wire schemas independent from private persisted-state schemas.

Do not invent ad-hoc JSON or duplicate generated wire types inside the business module.
'''


def render_adapters_placeholder(spec: ModuleSpec) -> str:
    return f'''# Adapter placeholder for `{spec.module_id}`

This directory records an explicit **TODO architecture boundary**.

Production capability, query, event-delivery, persistence and external-system adapters must remain outside the pure business-module core and depend on the module through narrow typed contracts. They must enter production only through governed composition/runtime boundaries.

Before raising readiness beyond `Foundation`, replace this placeholder with the appropriate separately owned adapter/composition crates and acceptance evidence. Do not add SQLx, PostgreSQL clients, brokers, arbitrary HTTP clients, secret stores, LLM providers or another business module's internals to this module crate.
'''


def render_acceptance(spec: ModuleSpec) -> str:
    kind_gate = (
        "- [ ] Define authoritative aggregate/value-object invariants and ownership boundaries."
        if spec.kind == "owner"
        else "- [ ] Define exact source events, target capabilities, delivery identity and lifecycle behavior."
    )
    return f'''# Acceptance gates for `{spec.module_id}`

Scaffold state: **Foundation only**. These TODO gates are intentionally explicit and block any claim of a production vertical slice.

- [ ] Confirm immutable module identity and lifecycle semantics in `module.yaml`.
{kind_gate}
- [ ] Publish compatible versioned Protobuf contracts and bindings.
- [ ] Add deterministic domain/application behavior with no infrastructure access.
- [ ] Add governed capability/query/event adapters as applicable.
- [ ] Add tenant, authorization, idempotency/retry and cross-tenant negative coverage.
- [ ] Add persistence/projection/search behavior only through platform adapters outside the module core.
- [ ] Replace `tests/acceptance.rs` with production-path acceptance evidence.
- [ ] Replace the fail-closed `{spec.composition_relative_dir.as_posix()}` entrypoint with exact module-owned route and worker contributions.
- [ ] Add production composition and end-to-end acceptance through governed gateways.
- [ ] Prove rollback/disable/uninstall behavior appropriate to the module type.
- [ ] Synchronize `MODULE_CATALOG.md`, roadmap/status and the owning GitHub issue.
'''


def render_acceptance_test_rs(spec: ModuleSpec) -> str:
    return f'''use {spec.rust_crate_name}::{{CRATE_NAME, MODULE_ID}};

#[test]
#[ignore = "scaffold gate: replace with governed production acceptance before raising readiness"]
fn production_acceptance_todo() {{
    assert_eq!(CRATE_NAME, "{spec.crate_name}");
    assert_eq!(MODULE_ID, "{spec.module_id}");
    panic!("replace scaffold acceptance placeholder with governed production acceptance");
}}
'''


def render_catalog_entry(spec: ModuleSpec) -> str:
    module_type = "Owner module" if spec.kind == "owner" else "Link module"
    ownership = (
        ", ".join(spec.objects)
        if spec.objects
        else "Optional cross-domain coordination only"
    )
    return (
        f"| `{spec.module_id}` | {module_type} | {ownership} | **Foundation** | "
        "Generated scaffold only; production path not yet implemented |\n"
    )


def build_files(spec: ModuleSpec) -> dict[Path, str]:
    base = spec.relative_dir
    composition = spec.composition_relative_dir
    return {
        base / "Cargo.toml": render_cargo_toml(spec),
        base / "module.yaml": render_manifest(spec),
        base / "src" / "lib.rs": render_lib_rs(spec),
        base / "README.md": render_readme(spec),
        base / "ACCEPTANCE.md": render_acceptance(spec),
        base / "MODULE_CATALOG_ENTRY.md": render_catalog_entry(spec),
        base / "contracts" / "README.md": render_contracts_placeholder(spec),
        base / "adapters" / "README.md": render_adapters_placeholder(spec),
        base / "tests" / "acceptance.rs": render_acceptance_test_rs(spec),
        base / "migrations" / ".gitkeep": "",
        composition / "Cargo.toml": render_composition_cargo_toml(spec),
        composition / "README.md": render_composition_readme(spec),
        composition / "src" / "lib.rs": render_composition_lib_rs(spec),
    }


def update_workspace_members(cargo_toml: str, relative_module_dir: str) -> str:
    member_line = f'  "{relative_module_dir}",'
    if member_line in cargo_toml:
        raise ScaffoldError(f"workspace already contains {relative_module_dir}")

    lines = cargo_toml.splitlines()
    start = next(
        (index for index, line in enumerate(lines) if line.strip() == "members = ["),
        None,
    )
    if start is None:
        raise ScaffoldError("workspace members list not found in Cargo.toml")
    end = next(
        (index for index in range(start + 1, len(lines)) if lines[index].strip() == "]"),
        None,
    )
    if end is None:
        raise ScaffoldError("workspace members list is not terminated")

    module_indexes = [
        index
        for index in range(start + 1, end)
        if lines[index].strip().startswith('"modules/')
    ]
    insert_at = (module_indexes[-1] + 1) if module_indexes else end
    lines.insert(insert_at, member_line)
    return "\n".join(lines) + ("\n" if cargo_toml.endswith("\n") else "")


def scaffold(root: Path, spec: ModuleSpec, *, dry_run: bool = False) -> list[Path]:
    validate_spec(spec)
    root = root.resolve()
    module_dir = root / spec.relative_dir
    composition_dir = root / spec.composition_relative_dir
    for target in (module_dir, composition_dir):
        if target.exists():
            relative_target = target.relative_to(root)
            raise ScaffoldError(
                f"target scaffold directory already exists: {relative_target}"
            )

    cargo_path = root / "Cargo.toml"
    if not cargo_path.exists():
        raise ScaffoldError(f"workspace Cargo.toml not found under {root}")

    files = build_files(spec)
    workspace_content = update_workspace_members(
        cargo_path.read_text(encoding="utf-8"), spec.relative_dir.as_posix()
    )
    workspace_content = update_workspace_members(
        workspace_content, spec.composition_relative_dir.as_posix()
    )
    planned = [Path("Cargo.toml"), *sorted(files)]
    if dry_run:
        return planned

    module_dir.mkdir(parents=True, exist_ok=False)
    try:
        for relative_path, content in files.items():
            path = root / relative_path
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(content, encoding="utf-8")
        cargo_path.write_text(workspace_content, encoding="utf-8")
    except Exception:
        # Avoid leaving a partial business module or composition boundary when a write
        # fails. The workspace file is written last, so removing both roots restores
        # the prior state.
        import shutil

        shutil.rmtree(module_dir, ignore_errors=True)
        shutil.rmtree(composition_dir, ignore_errors=True)
        raise
    return planned


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description=(
            "Generate a governed CRM business module foundation and register it "
            "in the workspace."
        )
    )
    parser.add_argument("kind", choices=("owner", "link"))
    parser.add_argument("--module-id", required=True)
    parser.add_argument("--display-name", required=True)
    parser.add_argument("--team", required=True)
    parser.add_argument("--contact", required=True)
    parser.add_argument("--object", dest="objects", action="append", default=[])
    parser.add_argument(
        "--requires",
        dest="required_dependencies",
        action="append",
        type=parse_dependency,
        default=[],
    )
    parser.add_argument("--root", type=Path, default=ROOT)
    parser.add_argument("--dry-run", action="store_true")
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    spec = ModuleSpec(
        kind=args.kind,
        module_id=args.module_id,
        display_name=args.display_name,
        team=args.team,
        contact=args.contact,
        objects=tuple(args.objects),
        required_dependencies=tuple(args.required_dependencies),
    )
    try:
        planned = scaffold(args.root, spec, dry_run=args.dry_run)
    except ScaffoldError as exc:
        print(f"Scaffold failed: {exc}", file=sys.stderr)
        return 2

    action = "Would create/update" if args.dry_run else "Created/updated"
    print(f"{action} {len(planned)} paths for {spec.module_id}:")
    for path in planned:
        print(f"- {path.as_posix()}")
    print(
        "Next: validate with `python scripts/repo.py architecture` and the "
        "applicable focused/full gates."
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
