#!/usr/bin/env python3
from pathlib import Path
import json
import sys
import tomllib

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

if errors:
    print("Architecture boundary check FAILED:")
    for error in errors:
        print(f"- {error}")
    sys.exit(1)

print("Architecture boundary check PASS")
