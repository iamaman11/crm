#!/usr/bin/env python3
from pathlib import Path
import json
import sys
import tomllib

from check_rust_governance import (
    POLICY_FILE as RUST_GOVERNANCE_POLICY_FILE,
    cargo_metadata as rust_cargo_metadata,
    load_json as load_rust_json,
    validate as validate_rust_governance,
)
from check_workspace_dependency_policy import validate_policy_document

root = Path(__file__).resolve().parents[1]
policy = json.loads((root / "architecture-policy.json").read_text(encoding="utf-8"))
errors: list[str] = []

for pattern in policy["business_module_globs"]:
    for module_dir in root.glob(pattern):
        cargo = module_dir / "Cargo.toml"
        if not cargo.exists():
            continue
        data = tomllib.loads(cargo.read_text(encoding="utf-8"))
        dependencies = set(data.get("dependencies", {})) | set(data.get("dev-dependencies", {}))
        forbidden = dependencies & set(policy["forbidden_dependencies"])
        if forbidden:
            errors.append(
                f"{cargo.relative_to(root)} uses forbidden dependencies: {sorted(forbidden)}"
            )
        internal = {dependency for dependency in dependencies if dependency.startswith("crm-")}
        allowed = set(policy["allowed_module_prefixes"])
        disallowed = internal - allowed
        if disallowed:
            errors.append(
                f"{cargo.relative_to(root)} imports disallowed internal crates: {sorted(disallowed)}"
            )

sdk_allowed_dependencies = set(policy.get("sdk_allowed_dependencies", []))
for relative_path in policy.get("sdk_crate_paths", []):
    sdk_dir = root / relative_path
    cargo = sdk_dir / "Cargo.toml"
    if not cargo.exists():
        errors.append(f"configured SDK crate is missing Cargo.toml: {relative_path}")
        continue

    data = tomllib.loads(cargo.read_text(encoding="utf-8"))
    dependencies = set(data.get("dependencies", {})) | set(data.get("dev-dependencies", {}))
    forbidden = dependencies & set(policy["forbidden_dependencies"])
    if forbidden:
        errors.append(
            f"{cargo.relative_to(root)} uses forbidden dependencies: {sorted(forbidden)}"
        )
    unexpected = dependencies - sdk_allowed_dependencies
    if unexpected:
        errors.append(
            f"{cargo.relative_to(root)} uses dependencies outside the SDK allowlist: "
            f"{sorted(unexpected)}"
        )

    forbidden_markers = tuple(policy.get("forbidden_source_markers", []))
    for source in sorted(sdk_dir.rglob("*.rs")):
        text = source.read_text(encoding="utf-8").lower()
        for marker in forbidden_markers:
            if marker.lower() in text:
                errors.append(
                    f"{source.relative_to(root)} contains forbidden infrastructure marker: {marker}"
                )

transport_markers = tuple(policy.get("governed_transport_forbidden_source_markers", []))
for relative_path, allowed_dependencies in policy.get("governed_transport_crates", {}).items():
    transport_dir = root / relative_path
    cargo = transport_dir / "Cargo.toml"
    if not cargo.exists():
        errors.append(f"configured governed transport crate is missing Cargo.toml: {relative_path}")
        continue

    data = tomllib.loads(cargo.read_text(encoding="utf-8"))
    runtime_dependencies = set(data.get("dependencies", {}))
    unexpected = runtime_dependencies - set(allowed_dependencies)
    if unexpected:
        errors.append(
            f"{cargo.relative_to(root)} can bypass the capability gateway through runtime "
            f"dependencies: {sorted(unexpected)}"
        )

    for source in sorted((transport_dir / "src").rglob("*.rs")):
        production_text = source.read_text(encoding="utf-8").partition("#[cfg(test)]")[0]
        for marker in transport_markers:
            if marker in production_text:
                errors.append(
                    f"{source.relative_to(root)} contains forbidden gateway-bypass marker: {marker}"
                )

cargo_files = sorted(
    cargo
    for root_path in ("crates", "modules", "services")
    for cargo in (root / root_path).rglob("Cargo.toml")
)
known_cargo_paths = {cargo.relative_to(root).as_posix() for cargo in cargo_files}
for dependency, allowed_paths in policy.get("restricted_dependency_consumers", {}).items():
    allowed = set(allowed_paths)
    missing_paths = allowed - known_cargo_paths
    if missing_paths:
        errors.append(
            f"restricted dependency {dependency} has missing allowed Cargo paths: "
            f"{sorted(missing_paths)}"
        )

    actual: set[str] = set()
    for cargo in cargo_files:
        data = tomllib.loads(cargo.read_text(encoding="utf-8"))
        dependencies = (
            set(data.get("dependencies", {}))
            | set(data.get("dev-dependencies", {}))
            | set(data.get("build-dependencies", {}))
        )
        if dependency in dependencies:
            actual.add(cargo.relative_to(root).as_posix())

    unexpected = actual - allowed
    if unexpected:
        errors.append(
            f"restricted dependency {dependency} is used by unexpected consumers: "
            f"{sorted(unexpected)}; allowed paths: {sorted(allowed)}"
        )

    missing_consumers = allowed - actual
    if missing_consumers:
        errors.append(
            f"restricted dependency {dependency} is missing required consumers: "
            f"{sorted(missing_consumers)}"
        )

rust_sources = sorted(
    source
    for root_path in ("crates", "modules", "services")
    for source in (root / root_path).rglob("*.rs")
)
for marker, allowed_paths in policy.get("restricted_source_markers", {}).items():
    allowed = set(allowed_paths)
    for source in rust_sources:
        relative = source.relative_to(root).as_posix()
        if marker in source.read_text(encoding="utf-8") and relative not in allowed:
            errors.append(
                f"{relative} uses restricted source marker {marker!r}; "
                f"allowed paths: {sorted(allowed)}"
            )

dependency_policy = validate_policy_document(root)
errors.extend(
    f"workspace dependency policy: {error}"
    for error in dependency_policy["blocking_errors"]
)
for warning in dependency_policy["warnings"]:
    print(f"Architecture dependency policy warning: {warning}")

rust_governance = validate_rust_governance(
    root=root,
    policy=load_rust_json(root / RUST_GOVERNANCE_POLICY_FILE),
    metadata=rust_cargo_metadata(root),
    base_ref=None,
    rustc_json=None,
    clippy_json=None,
    require_lint_measurements=False,
    skip_tool_versions=True,
)
errors.extend(
    f"Rust governance policy: {error}"
    for error in rust_governance["governance"]["blocking_errors"]
)
for warning in rust_governance["governance"]["warnings"]:
    print(f"Rust governance policy warning: {warning}")

if errors:
    print("Architecture boundary check FAILED:")
    for error in errors:
        print(f"- {error}")
    sys.exit(1)

print("Architecture boundary check PASS")
