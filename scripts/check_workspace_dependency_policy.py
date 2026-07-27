#!/usr/bin/env python3
"""Validate calibrated workspace dependency inheritance policies."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import sys
import tomllib
from typing import Any

SCHEMA_VERSION = "crm.workspace-dependency-policy/v1"
DEPENDENCY_SECTIONS = ("dependencies", "dev-dependencies", "build-dependencies")
FORBIDDEN_LOCAL_KEYS = (
    "version",
    "default-features",
    "path",
    "git",
    "package",
    "registry",
)


def load_toml(path: Path) -> dict[str, Any]:
    with path.open("rb") as handle:
        return tomllib.load(handle)


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


def validate_policy_document(root: Path) -> dict[str, Any]:
    policy_path = root / "workspace-dependency-policy.json"
    blocking_errors: list[str] = []
    warnings: list[str] = []
    policy_results: list[dict[str, Any]] = []

    if not policy_path.is_file():
        return {
            "schema_version": SCHEMA_VERSION,
            "blocking_errors": ["workspace-dependency-policy.json is missing"],
            "warnings": [],
            "policies": [],
        }

    document = json.loads(policy_path.read_text(encoding="utf-8"))
    if document.get("schema_version") != SCHEMA_VERSION:
        blocking_errors.append(f"schema_version must be {SCHEMA_VERSION}")

    root_manifest = load_toml(root / "Cargo.toml")
    workspace_dependencies = root_manifest.get("workspace", {}).get("dependencies", {})
    if not isinstance(workspace_dependencies, dict):
        workspace_dependencies = {}
        blocking_errors.append("root [workspace.dependencies] must be a table")

    policies = document.get("policies")
    if not isinstance(policies, list):
        blocking_errors.append("policies must be a list")
        policies = []

    seen_ids: set[str] = set()
    for index, policy in enumerate(policies):
        item_id = f"policy[{index}]"
        if not isinstance(policy, dict):
            blocking_errors.append(f"{item_id}: entry must be an object")
            continue

        identifier = policy.get("id")
        if isinstance(identifier, str) and identifier.strip():
            item_id = identifier
            if identifier in seen_ids:
                blocking_errors.append(f"{item_id}: duplicate id")
            seen_ids.add(identifier)

        for field in ("id", "owner", "scope_glob", "reason", "tracking_issue"):
            value = policy.get(field)
            if not isinstance(value, str) or not value.strip():
                blocking_errors.append(f"{item_id}: {field} is required")

        enforcement = policy.get("enforcement")
        if enforcement not in {"blocking", "warning"}:
            blocking_errors.append(
                f"{item_id}: enforcement must be 'blocking' or 'warning'"
            )
            enforcement = "blocking"

        allow_local_features = policy.get("allow_local_features")
        if not isinstance(allow_local_features, bool):
            blocking_errors.append(f"{item_id}: allow_local_features must be boolean")
            allow_local_features = False

        dependencies = policy.get("dependencies")
        if not isinstance(dependencies, list) or not dependencies or not all(
            isinstance(name, str) and name.strip() for name in dependencies
        ):
            blocking_errors.append(
                f"{item_id}: dependencies must be a non-empty string list"
            )
            dependencies = []

        scope_glob = policy.get("scope_glob")
        matched = (
            sorted(path for path in root.glob(scope_glob) if path.is_file())
            if isinstance(scope_glob, str) and scope_glob.strip()
            else []
        )
        policy_violations: list[str] = []
        if not matched:
            policy_violations.append(f"{item_id}: scope_glob matched no manifests")

        for dependency in dependencies:
            if dependency not in workspace_dependencies:
                policy_violations.append(
                    f"{item_id}: {dependency} is missing from root [workspace.dependencies]"
                )

        governed_declarations = 0
        for manifest_path in matched:
            manifest = load_toml(manifest_path)
            relative = manifest_path.relative_to(root).as_posix()
            for section, table in dependency_tables(manifest):
                for dependency in dependencies:
                    if dependency not in table:
                        continue
                    governed_declarations += 1
                    raw_spec = table[dependency]
                    location = f"{relative}:{section}.{dependency}"
                    if not isinstance(raw_spec, dict) or raw_spec.get("workspace") is not True:
                        policy_violations.append(
                            f"{item_id}: {location} must use workspace = true"
                        )
                        continue
                    forbidden = sorted(
                        key for key in FORBIDDEN_LOCAL_KEYS if key in raw_spec
                    )
                    if forbidden:
                        policy_violations.append(
                            f"{item_id}: {location} has forbidden local overrides {forbidden}"
                        )
                    local_features = raw_spec.get("features", [])
                    if not allow_local_features and local_features:
                        policy_violations.append(
                            f"{item_id}: {location} must not add local features"
                        )

        target = blocking_errors if enforcement == "blocking" else warnings
        target.extend(policy_violations)
        policy_results.append(
            {
                "id": item_id,
                "enforcement": enforcement,
                "scope_glob": scope_glob,
                "matched_manifest_count": len(matched),
                "dependencies": dependencies,
                "governed_declaration_count": governed_declarations,
                "violation_count": len(policy_violations),
            }
        )

    return {
        "schema_version": SCHEMA_VERSION,
        "blocking_errors": blocking_errors,
        "warnings": warnings,
        "policies": policy_results,
    }


def markdown_report(report: dict[str, Any]) -> str:
    lines = [
        "# Workspace Dependency Policy",
        "",
        "> Calibrated policies are blocking only for their explicit machine-readable scope.",
        "",
        "| Policy | Enforcement | Manifests | Declarations | Violations |",
        "|---|---|---:|---:|---:|",
    ]
    for policy in report["policies"]:
        lines.append(
            f"| `{policy['id']}` | {policy['enforcement']} | "
            f"{policy['matched_manifest_count']} | "
            f"{policy['governed_declaration_count']} | "
            f"{policy['violation_count']} |"
        )
    lines.extend(["", "## Result", ""])
    if report["blocking_errors"]:
        lines.extend(f"- **BLOCKING:** {error}" for error in report["blocking_errors"])
    else:
        lines.append("- No blocking workspace dependency policy violation detected.")
    lines.extend(f"- Warning: {warning}" for warning in report["warnings"])
    lines.append("")
    return "\n".join(lines)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path.cwd())
    parser.add_argument("--json-output", type=Path)
    parser.add_argument("--markdown-output", type=Path)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    root = args.root.resolve()
    try:
        report = validate_policy_document(root)
    except (OSError, ValueError, json.JSONDecodeError, tomllib.TOMLDecodeError) as error:
        print(f"workspace dependency policy validation failed: {error}", file=sys.stderr)
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
        for warning in report["warnings"]:
            print(f"workspace dependency policy warning: {warning}", file=sys.stderr)
        if report["blocking_errors"]:
            for error in report["blocking_errors"]:
                print(f"workspace dependency policy error: {error}", file=sys.stderr)
            return 1
        print("Workspace dependency policy PASS")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
