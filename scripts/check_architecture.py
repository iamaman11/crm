#!/usr/bin/env python3
from pathlib import Path
import json, tomllib, sys

root = Path(__file__).resolve().parents[1]
policy = json.loads((root / "architecture-policy.json").read_text())
errors = []
for pattern in policy["business_module_globs"]:
    for module_dir in root.glob(pattern):
        cargo = module_dir / "Cargo.toml"
        if not cargo.exists():
            continue
        data = tomllib.loads(cargo.read_text())
        dependencies = set(data.get("dependencies", {})) | set(data.get("dev-dependencies", {}))
        forbidden = dependencies & set(policy["forbidden_dependencies"])
        if forbidden:
            errors.append(f"{cargo.relative_to(root)} uses forbidden dependencies: {sorted(forbidden)}")
        internal = {d for d in dependencies if d.startswith("crm-")}
        allowed = set(policy["allowed_module_prefixes"])
        disallowed = internal - allowed
        if disallowed:
            errors.append(f"{cargo.relative_to(root)} imports disallowed internal crates: {sorted(disallowed)}")

if errors:
    print("Architecture boundary check FAILED:")
    for error in errors: print(f"- {error}")
    sys.exit(1)
print("Architecture boundary check PASS")
