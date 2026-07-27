#!/usr/bin/env python3
"""Measure and validate workspace dependency, crate and exception governance."""

from __future__ import annotations

import argparse
from collections import defaultdict, deque
from datetime import date
import json
from pathlib import Path
import re
import subprocess
import sys
import tomllib
from typing import Any

SCHEMA_VERSION = "crm.workspace-governance-baseline/v1"
REGISTRY_SCHEMA_VERSION = "crm.architecture-governance/v1"
DEPENDENCY_SECTIONS = ("dependencies", "dev-dependencies", "build-dependencies")
HEAVY_FEATURES = {
    "all",
    "full",
    "macros",
    "native-tls",
    "postgres",
    "runtime-tokio",
    "rustls",
    "tls",
}
PUBLIC_ITEM = re.compile(
    r"^\s*pub\s+(?:(?:async|const|unsafe|extern(?:\s+\"[^\"]+\")?)\s+)*"
    r"(?:fn|struct|enum|trait|type|mod|static|const|use|union)\b"
)
REPRESENTATIVE_PACKAGES = (
    "crm-customer-enrichment-privacy-scope-adapter",
    "crm-data-quality-privacy-scope-adapter",
    "crm-customer-privacy-cancel-capability-adapter",
    "crm-module-manifest",
    "crm-api",
)


def run(command: list[str], root: Path) -> str:
    completed = subprocess.run(
        command,
        cwd=root,
        check=False,
        capture_output=True,
        text=True,
    )
    if completed.returncode != 0:
        raise RuntimeError(
            f"command failed ({' '.join(command)}):\n{completed.stdout}\n{completed.stderr}"
        )
    return completed.stdout


def load_toml(path: Path) -> dict[str, Any]:
    with path.open("rb") as handle:
        return tomllib.load(handle)


def cargo_metadata(root: Path) -> dict[str, Any]:
    return json.loads(
        run(
            ["cargo", "metadata", "--format-version", "1", "--locked", "--no-deps"],
            root,
        )
    )


def workspace_packages(metadata: dict[str, Any]) -> list[dict[str, Any]]:
    ids = set(metadata.get("workspace_members", []))
    return sorted(
        [
            package
            for package in metadata.get("packages", [])
            if package.get("id") in ids
        ],
        key=lambda package: package["name"],
    )


def package_graph(
    packages: list[dict[str, Any]],
) -> tuple[dict[str, set[str]], dict[str, set[str]]]:
    names = {package["name"] for package in packages}
    dependencies: dict[str, set[str]] = {}
    dependents = {name: set() for name in names}
    for package in packages:
        direct = {
            dependency["name"]
            for dependency in package.get("dependencies", [])
            if dependency.get("name") in names
        }
        dependencies[package["name"]] = direct
        for dependency in direct:
            dependents[dependency].add(package["name"])
    return dependencies, dependents


def dependency_depths(dependencies: dict[str, set[str]]) -> dict[str, int]:
    cache: dict[str, int] = {}

    def depth(name: str, active: set[str]) -> int:
        if name in cache:
            return cache[name]
        if name in active:
            return 0
        direct = dependencies.get(name, set())
        if not direct:
            cache[name] = 0
            return 0
        next_active = set(active)
        next_active.add(name)
        cache[name] = 1 + max(depth(dependency, next_active) for dependency in direct)
        return cache[name]

    return {name: depth(name, set()) for name in dependencies}


def reverse_closure(name: str, dependents: dict[str, set[str]]) -> list[str]:
    found: set[str] = set()
    queue: deque[str] = deque(dependents.get(name, set()))
    while queue:
        dependent = queue.popleft()
        if dependent in found:
            continue
        found.add(dependent)
        queue.extend(dependents.get(dependent, set()))
    return sorted(found)


def public_items(manifest_path: Path) -> int:
    source_root = manifest_path.parent / "src"
    if not source_root.exists():
        return 0
    return sum(
        1
        for source in sorted(source_root.rglob("*.rs"))
        for line in source.read_text(encoding="utf-8").splitlines()
        if PUBLIC_ITEM.match(line)
    )


def normalize_spec(spec: Any) -> dict[str, Any]:
    if isinstance(spec, str):
        return {
            "workspace": False,
            "version": spec,
            "features": [],
            "default_features": True,
            "path": None,
            "git": None,
            "package": None,
        }
    if not isinstance(spec, dict):
        spec = {}
    return {
        "workspace": bool(spec.get("workspace", False)),
        "version": spec.get("version"),
        "features": sorted(set(spec.get("features", []))),
        "default_features": spec.get("default-features", True),
        "path": spec.get("path"),
        "git": spec.get("git"),
        "package": spec.get("package"),
    }


def effective_spec(
    local: dict[str, Any], workspace: dict[str, Any] | None
) -> dict[str, Any]:
    if not local["workspace"] or workspace is None:
        return local
    merged = dict(workspace)
    merged["workspace"] = True
    merged["features"] = sorted(set(workspace["features"]) | set(local["features"]))
    return merged


def dependency_tables(manifest: dict[str, Any]):
    for section in DEPENDENCY_SECTIONS:
        table = manifest.get(section)
        if isinstance(table, dict):
            yield section, table
    for target_name, target in sorted(manifest.get("target", {}).items()):
        if not isinstance(target, dict):
            continue
        for section in DEPENDENCY_SECTIONS:
            table = target.get(section)
            if isinstance(table, dict):
                yield f"target.{target_name}.{section}", table


def dependency_metrics(
    root: Path,
    packages: list[dict[str, Any]],
    workspace_dependencies: dict[str, Any],
) -> dict[str, Any]:
    workspace_specs = {
        name: normalize_spec(spec) for name, spec in workspace_dependencies.items()
    }
    declarations: list[dict[str, Any]] = []
    consumers: dict[str, set[str]] = defaultdict(set)
    for package in packages:
        manifest_path = Path(package["manifest_path"])
        manifest = load_toml(manifest_path)
        relative = manifest_path.resolve().relative_to(root.resolve()).as_posix()
        for section, table in dependency_tables(manifest):
            for name, raw_spec in sorted(table.items()):
                local = normalize_spec(raw_spec)
                effective = effective_spec(local, workspace_specs.get(name))
                actual_name = local.get("package") or name
                if effective.get("path") and actual_name.startswith("crm-"):
                    continue
                row = {
                    "package": package["name"],
                    "manifest_path": relative,
                    "section": section,
                    "dependency": name,
                    "workspace_inherited": local["workspace"],
                    "version": effective["version"],
                    "features": effective["features"],
                    "default_features": effective["default_features"],
                    "source": "git" if effective.get("git") else "registry",
                }
                declarations.append(row)
                consumers[name].add(package["name"])

    versions: dict[str, set[str]] = defaultdict(set)
    features: dict[str, set[tuple[tuple[str, ...], bool]]] = defaultdict(set)
    non_inheriting: dict[str, set[str]] = defaultdict(set)
    heavy: list[dict[str, Any]] = []
    for row in declarations:
        if row["version"]:
            versions[row["dependency"]].add(row["version"])
        features[row["dependency"]].add(
            (tuple(row["features"]), bool(row["default_features"]))
        )
        if row["dependency"] in workspace_specs and not row["workspace_inherited"]:
            non_inheriting[row["dependency"]].add(row["manifest_path"])
        row_features = set(row["features"])
        if len(row_features) >= 3 or row_features & HEAVY_FEATURES:
            heavy.append(row)

    return {
        "declaration_count": len(declarations),
        "workspace_dependency_count": len(workspace_specs),
        "version_divergence": [
            {"name": name, "requirements": sorted(requirements)}
            for name, requirements in sorted(versions.items())
            if len(requirements) > 1
        ],
        "feature_divergence": [
            {
                "name": name,
                "variants": [
                    {
                        "features": list(feature_set),
                        "default_features": default_features,
                    }
                    for feature_set, default_features in sorted(variants)
                ],
            }
            for name, variants in sorted(features.items())
            if len(variants) > 1
        ],
        "non_inheriting_workspace_dependencies": [
            {"name": name, "manifests": sorted(paths)}
            for name, paths in sorted(non_inheriting.items())
        ],
        "heavy_feature_declarations": heavy,
        "most_repeated_dependencies": [
            {"name": name, "package_count": len(package_names)}
            for name, package_names in sorted(
                consumers.items(), key=lambda item: (-len(item[1]), item[0])
            )
        ][:25],
    }


def workspace_configuration(
    root: Path, root_manifest: dict[str, Any], packages: list[dict[str, Any]]
) -> dict[str, Any]:
    workspace = root_manifest.get("workspace", {})
    inherited = {"edition": 0, "license": 0, "rust-version": 0, "lints": 0}
    for package in packages:
        manifest = load_toml(Path(package["manifest_path"]))
        package_table = manifest.get("package", {})
        for field in ("edition", "license", "rust-version"):
            value = package_table.get(field)
            if isinstance(value, dict) and value.get("workspace") is True:
                inherited[field] += 1
        lint_table = manifest.get("lints")
        if isinstance(lint_table, dict) and lint_table.get("workspace") is True:
            inherited["lints"] += 1
    nested_locks = sorted(
        path.relative_to(root).as_posix()
        for parent in ("crates", "modules", "services")
        for path in (root / parent).rglob("Cargo.lock")
    )
    return {
        "resolver": str(workspace.get("resolver", "")),
        "declared_member_count": len(workspace.get("members", [])),
        "metadata_member_count": len(packages),
        "workspace_package": workspace.get("package", {}),
        "workspace_dependencies": workspace.get("dependencies", {}),
        "workspace_lints_present": "lints" in workspace,
        "package_inheritance": inherited,
        "shared_lockfile_present": (root / "Cargo.lock").is_file(),
        "nested_lockfiles": nested_locks,
    }


def workspace_members(text: str) -> set[str]:
    return {
        str(member)
        for member in tomllib.loads(text).get("workspace", {}).get("members", [])
    }


def validate_governance(
    root: Path,
    current_members: set[str],
    base_ref: str | None,
    today: date | None = None,
) -> tuple[list[str], list[str], list[str]]:
    errors: list[str] = []
    warnings: list[str] = []
    new_members: list[str] = []
    path = root / "architecture-governance.json"
    if not path.is_file():
        return ["architecture-governance.json is missing"], warnings, new_members
    registry = json.loads(path.read_text(encoding="utf-8"))
    if registry.get("schema_version") != REGISTRY_SCHEMA_VERSION:
        errors.append(f"schema_version must be {REGISTRY_SCHEMA_VERSION}")
    today = today or date.today()

    exceptions = registry.get("exceptions")
    if not isinstance(exceptions, list):
        errors.append("exceptions must be a list")
        exceptions = []
    seen_ids: set[str] = set()
    for index, exception in enumerate(exceptions):
        item_id = f"exception[{index}]"
        if not isinstance(exception, dict):
            errors.append(f"{item_id}: entry must be an object")
            continue
        identifier = exception.get("id")
        if isinstance(identifier, str) and identifier.strip():
            item_id = identifier
            if identifier in seen_ids:
                errors.append(f"{item_id}: duplicate id")
            seen_ids.add(identifier)
        for field in (
            "id",
            "owner",
            "rule",
            "reason_and_risk",
            "scope",
            "removal_condition",
            "tracking_issue",
        ):
            value = exception.get(field)
            if not isinstance(value, str) or not value.strip():
                errors.append(f"{item_id}: {field} is required")
        checks = exception.get("compensating_checks")
        if not isinstance(checks, list) or not checks or not all(
            isinstance(check, str) and check.strip() for check in checks
        ):
            errors.append(
                f"{item_id}: compensating_checks must be a non-empty string list"
            )
        try:
            created = date.fromisoformat(exception["created_date"])
            expiry = date.fromisoformat(exception["expiry_date"])
        except (KeyError, TypeError, ValueError):
            errors.append(f"{item_id}: created_date and expiry_date must be ISO dates")
            continue
        if expiry < created:
            errors.append(f"{item_id}: expiry_date precedes created_date")
        if expiry < today:
            errors.append(f"{item_id}: exception expired on {expiry.isoformat()}")

    justifications = registry.get("new_crate_justifications")
    if not isinstance(justifications, list):
        errors.append("new_crate_justifications must be a list")
        justifications = []
    by_path: dict[str, dict[str, Any]] = {}
    required = (
        "package",
        "path",
        "protected_boundary",
        "isolated_dependencies",
        "expected_consumers",
        "why_internal_module_insufficient",
        "lifecycle_or_extraction_seam",
        "expected_build_test_fan_out",
        "removal_or_consolidation_condition",
        "tracking_issue",
    )
    for index, justification in enumerate(justifications):
        item_id = f"new_crate_justification[{index}]"
        if not isinstance(justification, dict):
            errors.append(f"{item_id}: entry must be an object")
            continue
        path_value = justification.get("path")
        if isinstance(path_value, str) and path_value.strip():
            item_id = path_value
            if path_value in by_path:
                errors.append(f"{item_id}: duplicate path")
            by_path[path_value] = justification
        for field in required:
            value = justification.get(field)
            if isinstance(value, list):
                if not value or not all(
                    isinstance(entry, str) and entry.strip() for entry in value
                ):
                    errors.append(f"{item_id}: {field} must be a non-empty string list")
            elif not isinstance(value, str) or not value.strip():
                errors.append(f"{item_id}: {field} is required")

    if base_ref:
        previous = workspace_members(
            run(["git", "show", f"{base_ref}:Cargo.toml"], root)
        )
        new_members = sorted(current_members - previous)
        for member in new_members:
            if member not in by_path:
                errors.append(
                    f"new workspace member {member} has no complete justification"
                )
    elif justifications:
        warnings.append(
            "new-crate justifications exist but no base ref was supplied"
        )
    return errors, warnings, new_members


def build_report(root: Path, base_ref: str | None) -> dict[str, Any]:
    metadata = cargo_metadata(root)
    packages = workspace_packages(metadata)
    root_manifest = load_toml(root / "Cargo.toml")
    configuration = workspace_configuration(root, root_manifest, packages)
    dependencies, dependents = package_graph(packages)
    depths = dependency_depths(dependencies)
    categories: dict[str, int] = defaultdict(int)
    rows = []
    for package in packages:
        manifest_path = Path(package["manifest_path"])
        relative = manifest_path.resolve().relative_to(root.resolve())
        package_category = {
            "crates": "technical-crate",
            "modules": "business-module",
            "services": "service",
        }.get(relative.parts[0], "other")
        categories[package_category] += 1
        rows.append(
            {
                "name": package["name"],
                "category": package_category,
                "manifest_path": relative.as_posix(),
                "direct_internal_dependencies": len(dependencies[package["name"]]),
                "direct_internal_dependents": len(dependents[package["name"]]),
                "dependency_depth": depths[package["name"]],
                "reverse_impact": len(reverse_closure(package["name"], dependents)),
                "public_rust_items": public_items(manifest_path),
            }
        )
    direct = dependency_metrics(
        root, packages, configuration["workspace_dependencies"]
    )
    current_members = {
        str(member)
        for member in root_manifest.get("workspace", {}).get("members", [])
    }
    errors, warnings, new_members = validate_governance(
        root, current_members, base_ref
    )
    if configuration["resolver"] != "2":
        errors.append(f"workspace resolver must be 2, found {configuration['resolver']}")
    if not configuration["shared_lockfile_present"]:
        errors.append("root Cargo.lock is missing")
    if configuration["nested_lockfiles"]:
        errors.append(
            f"nested Cargo.lock files are forbidden: {configuration['nested_lockfiles']}"
        )
    if configuration["declared_member_count"] != configuration["metadata_member_count"]:
        errors.append("workspace member count differs from cargo metadata")
    if direct["version_divergence"]:
        warnings.append("direct dependency version requirements diverge")
    if direct["feature_divergence"]:
        warnings.append("direct dependency feature sets diverge")
    if direct["non_inheriting_workspace_dependencies"]:
        warnings.append("workspace dependencies are not inherited by all consumers")
    if not configuration["workspace_package"].get("rust-version"):
        warnings.append("workspace.package.rust-version is not defined")
    if not configuration["workspace_lints_present"]:
        warnings.append("workspace.lints is not defined")

    names = {package["name"] for package in packages}
    representative = []
    for name in REPRESENTATIVE_PACKAGES:
        if name in names:
            closure = reverse_closure(name, dependents)
            representative.append(
                {
                    "package": name,
                    "affected_package_count": 1 + len(closure),
                    "affected_packages": [name, *closure],
                }
            )
    return {
        "schema_version": SCHEMA_VERSION,
        "commit_sha": run(["git", "rev-parse", "HEAD"], root).strip(),
        "mode": "measurement-report-warning",
        "governance": {
            "blocking_errors": errors,
            "warnings": warnings,
            "new_workspace_members": new_members,
        },
        "workspace_configuration": configuration,
        "workspace": {
            "package_count": len(packages),
            "categories": dict(sorted(categories.items())),
            "internal_dependency_edges": sum(len(value) for value in dependencies.values()),
            "maximum_dependency_depth": max(depths.values(), default=0),
            "maximum_direct_dependents": max(
                (len(value) for value in dependents.values()), default=0
            ),
            "maximum_reverse_impact": max(
                (row["reverse_impact"] for row in rows), default=0
            ),
            "one_consumer_package_count": sum(
                row["direct_internal_dependents"] == 1 for row in rows
            ),
            "public_rust_item_count": sum(row["public_rust_items"] for row in rows),
            "top_reverse_impact": sorted(
                rows, key=lambda row: (-row["reverse_impact"], row["name"])
            )[:20],
            "top_dependency_depth": sorted(
                rows, key=lambda row: (-row["dependency_depth"], row["name"])
            )[:20],
            "top_public_surface": sorted(
                rows, key=lambda row: (-row["public_rust_items"], row["name"])
            )[:20],
            "representative_affected_closures": representative,
        },
        "dependencies": direct,
        "limitations": [
            "Public Rust surface is a conservative source-text count, not rustdoc semantic API analysis.",
            "Heavy features use feature-count and named-feature heuristics and are not automatically defects.",
            "Dependency, feature, fan-out and public-surface observations remain warnings until calibrated.",
        ],
    }


def markdown_report(report: dict[str, Any]) -> str:
    workspace = report["workspace"]
    dependencies = report["dependencies"]
    configuration = report["workspace_configuration"]
    categories = workspace["categories"]
    lines = [
        "# Workspace Dependency, Crate and Exception Governance Baseline",
        "",
        f"Commit: `{report['commit_sha']}`",
        "",
        "> Measurement → report → warning. Invalid/expired exceptions, unjustified new members and broken workspace invariants are blocking.",
        "",
        "## Headline metrics",
        "",
        "| Metric | Value |",
        "|---|---:|",
        f"| Resolver | `{configuration['resolver'] or 'missing'}` |",
        f"| Workspace edition | `{configuration['workspace_package'].get('edition', 'missing')}` |",
        f"| Workspace rust-version | `{configuration['workspace_package'].get('rust-version', 'missing')}` |",
        f"| Workspace lint policy | {'present' if configuration['workspace_lints_present'] else 'missing'} |",
        f"| Workspace packages | {workspace['package_count']} |",
        f"| Technical crates | {categories.get('technical-crate', 0)} |",
        f"| Business modules | {categories.get('business-module', 0)} |",
        f"| Services | {categories.get('service', 0)} |",
        f"| Internal dependency edges | {workspace['internal_dependency_edges']} |",
        f"| Maximum dependency depth | {workspace['maximum_dependency_depth']} |",
        f"| Maximum direct dependents | {workspace['maximum_direct_dependents']} |",
        f"| Maximum reverse impact | {workspace['maximum_reverse_impact']} |",
        f"| Conservative public Rust items | {workspace['public_rust_item_count']} |",
        f"| Workspace dependencies | {dependencies['workspace_dependency_count']} |",
        f"| Direct version divergences | {len(dependencies['version_divergence'])} |",
        f"| Feature divergences | {len(dependencies['feature_divergence'])} |",
        f"| Workspace deps with non-inheriting consumers | {len(dependencies['non_inheriting_workspace_dependencies'])} |",
        f"| Heavy-feature declarations | {len(dependencies['heavy_feature_declarations'])} |",
        "",
        "## Most repeated direct dependencies",
        "",
        "| Dependency | Packages |",
        "|---|---:|",
    ]
    for row in dependencies["most_repeated_dependencies"][:15]:
        lines.append(f"| `{row['name']}` | {row['package_count']} |")
    lines.extend(
        [
            "",
            "## Representative affected closures",
            "",
            "| Package | Packages in closure |",
            "|---|---:|",
        ]
    )
    for row in workspace["representative_affected_closures"]:
        lines.append(f"| `{row['package']}` | {row['affected_package_count']} |")
    lines.extend(["", "## Direct version divergence", ""])
    if dependencies["version_divergence"]:
        for row in dependencies["version_divergence"]:
            lines.append(f"- `{row['name']}`: {', '.join(row['requirements'])}")
    else:
        lines.append("No direct version divergence detected.")
    lines.extend(["", "## Feature divergence", ""])
    if dependencies["feature_divergence"]:
        for row in dependencies["feature_divergence"]:
            lines.append(f"- `{row['name']}`: {len(row['variants'])} variants")
    else:
        lines.append("No direct feature divergence detected.")
    lines.extend(["", "## Governance result", ""])
    if report["governance"]["blocking_errors"]:
        lines.extend(
            f"- **BLOCKING:** {error}"
            for error in report["governance"]["blocking_errors"]
        )
    else:
        lines.append("- No blocking governance error detected.")
    lines.extend(
        f"- Warning: {warning}" for warning in report["governance"]["warnings"]
    )
    lines.extend(["", "## Limitations", ""])
    lines.extend(f"- {limitation}" for limitation in report["limitations"])
    lines.append("")
    return "\n".join(lines)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path.cwd())
    parser.add_argument("--base-ref")
    parser.add_argument("--json-output", type=Path)
    parser.add_argument("--markdown-output", type=Path)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    root = args.root.resolve()
    try:
        report = build_report(root, args.base_ref)
    except (RuntimeError, OSError, ValueError, json.JSONDecodeError, tomllib.TOMLDecodeError) as error:
        print(f"workspace governance analysis failed: {error}", file=sys.stderr)
        return 1
    json_text = json.dumps(report, indent=2, sort_keys=True) + "\n"
    markdown_text = markdown_report(report)
    if args.json_output:
        args.json_output.parent.mkdir(parents=True, exist_ok=True)
        args.json_output.write_text(json_text, encoding="utf-8")
    elif not args.markdown_output:
        print(json_text, end="")
    if args.markdown_output:
        args.markdown_output.parent.mkdir(parents=True, exist_ok=True)
        args.markdown_output.write_text(markdown_text, encoding="utf-8")
    if args.check:
        for warning in report["governance"]["warnings"]:
            print(f"workspace governance warning: {warning}", file=sys.stderr)
        if report["governance"]["blocking_errors"]:
            for error in report["governance"]["blocking_errors"]:
                print(f"workspace governance error: {error}", file=sys.stderr)
            return 1
        print("Workspace dependency, crate and exception governance PASS")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
