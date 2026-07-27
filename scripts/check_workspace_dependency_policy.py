#!/usr/bin/env python3
"""Validate calibrated workspace dependency inheritance and no-growth policies."""

from __future__ import annotations

import argparse
from collections import defaultdict
from datetime import date
import json
from pathlib import Path
import subprocess
import sys
import tomllib
from typing import Any

SCHEMA_VERSION = "crm.workspace-dependency-policy/v1"
REGISTRY_SCHEMA_VERSION = "crm.architecture-governance/v1"
DEPENDENCY_SECTIONS = ("dependencies", "dev-dependencies", "build-dependencies")
FORBIDDEN_LOCAL_KEYS = (
    "version",
    "default-features",
    "path",
    "git",
    "package",
    "registry",
    "branch",
    "rev",
    "tag",
)
SPEC_KEYS = (
    "workspace",
    "version",
    "features",
    "default_features",
    "path",
    "git",
    "registry",
    "package",
    "branch",
    "rev",
    "tag",
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


def normalize_spec(spec: Any) -> dict[str, Any]:
    if isinstance(spec, str):
        raw: dict[str, Any] = {"version": spec}
    elif isinstance(spec, dict):
        raw = spec
    else:
        raw = {}
    return {
        "workspace": raw.get("workspace") is True,
        "version": raw.get("version"),
        "features": sorted(set(raw.get("features", []))),
        "default_features": raw.get("default-features", True),
        "path": raw.get("path"),
        "git": raw.get("git"),
        "registry": raw.get("registry"),
        "package": raw.get("package"),
        "branch": raw.get("branch"),
        "rev": raw.get("rev"),
        "tag": raw.get("tag"),
    }


def workspace_manifest_paths(root: Path) -> list[Path]:
    metadata = json.loads(
        run(
            ["cargo", "metadata", "--format-version", "1", "--locked", "--no-deps"],
            root,
        )
    )
    members = set(metadata.get("workspace_members", []))
    return sorted(
        {
            Path(package["manifest_path"]).resolve()
            for package in metadata.get("packages", [])
            if package.get("id") in members
        },
        key=lambda path: path.as_posix(),
    )


def dependency_exception_scope(manifest_path: str, dependency: str) -> str:
    return f"{manifest_path}:{dependency}"


def load_dependency_exceptions(
    root: Path,
    exception_rule: str,
    today: date,
) -> tuple[list[str], set[str], list[str]]:
    path = root / "architecture-governance.json"
    if not path.is_file():
        return ["architecture-governance.json is missing"], set(), []
    registry = json.loads(path.read_text(encoding="utf-8"))
    errors: list[str] = []
    warnings: list[str] = []
    valid_scopes: set[str] = set()
    if registry.get("schema_version") != REGISTRY_SCHEMA_VERSION:
        errors.append(f"architecture governance schema_version must be {REGISTRY_SCHEMA_VERSION}")
    exceptions = registry.get("exceptions")
    if not isinstance(exceptions, list):
        return [*errors, "architecture governance exceptions must be a list"], valid_scopes, warnings

    seen_ids: set[str] = set()
    for index, exception in enumerate(exceptions):
        item_id = f"exception[{index}]"
        item_errors: list[str] = []
        if not isinstance(exception, dict):
            errors.append(f"{item_id}: entry must be an object")
            continue
        identifier = exception.get("id")
        if isinstance(identifier, str) and identifier.strip():
            item_id = identifier
            if identifier in seen_ids:
                item_errors.append("duplicate id")
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
                item_errors.append(f"{field} is required")
        checks = exception.get("compensating_checks")
        if not isinstance(checks, list) or not checks or not all(
            isinstance(check, str) and check.strip() for check in checks
        ):
            item_errors.append("compensating_checks must be a non-empty string list")
        try:
            created = date.fromisoformat(exception["created_date"])
            expiry = date.fromisoformat(exception["expiry_date"])
        except (KeyError, TypeError, ValueError):
            item_errors.append("created_date and expiry_date must be ISO dates")
            created = None
            expiry = None
        if created is not None and expiry is not None:
            if expiry < created:
                item_errors.append("expiry_date precedes created_date")
            if expiry < today:
                item_errors.append(f"exception expired on {expiry.isoformat()}")

        errors.extend(f"{item_id}: {message}" for message in item_errors)
        if exception.get("rule") != exception_rule or item_errors:
            continue
        scope = exception.get("scope")
        if isinstance(scope, str):
            if scope in valid_scopes:
                errors.append(f"{item_id}: duplicate dependency exception scope {scope}")
            valid_scopes.add(scope)

    return errors, valid_scopes, warnings


def collect_root_dependency_declarations(
    root: Path,
    manifest_paths: list[Path],
    dependency_families: set[str],
) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    direct: list[dict[str, Any]] = []
    inherited: list[dict[str, Any]] = []
    for manifest_path in sorted(set(path.resolve() for path in manifest_paths)):
        manifest = load_toml(manifest_path)
        relative = manifest_path.relative_to(root.resolve()).as_posix()
        for section, table in dependency_tables(manifest):
            for declaration_name, raw_spec in sorted(table.items()):
                spec = normalize_spec(raw_spec)
                actual_name = spec.get("package") or declaration_name
                if actual_name not in dependency_families:
                    continue
                row = {
                    "manifest_path": relative,
                    "section": section,
                    "declaration_name": declaration_name,
                    "dependency": actual_name,
                    "spec": spec,
                }
                if spec["workspace"]:
                    inherited.append(row)
                else:
                    direct.append(row)
    return direct, inherited


def validate_policy_document(
    root: Path,
    *,
    manifest_paths: list[Path] | None = None,
    today: date | None = None,
) -> dict[str, Any]:
    root = root.resolve()
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
            "no_growth": None,
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

    no_growth = document.get("no_growth")
    no_growth_result: dict[str, Any] | None = None
    if not isinstance(no_growth, dict):
        blocking_errors.append("no_growth policy must be an object")
    else:
        item_id = no_growth.get("id") or "no_growth"
        for field in (
            "id",
            "owner",
            "reason",
            "tracking_issue",
            "exception_rule",
        ):
            value = no_growth.get(field)
            if not isinstance(value, str) or not value.strip():
                blocking_errors.append(f"{item_id}: {field} is required")

        root_families = sorted(workspace_dependencies)
        families = no_growth.get("dependency_families")
        if not isinstance(families, list) or not all(
            isinstance(name, str) and name.strip() for name in families
        ):
            blocking_errors.append(
                f"{item_id}: dependency_families must be a string list"
            )
            families = []
        if len(families) != len(set(families)):
            blocking_errors.append(f"{item_id}: dependency_families must be unique")
        families = sorted(families)
        if families != root_families:
            blocking_errors.append(
                f"{item_id}: dependency_families must exactly match root "
                f"[workspace.dependencies]; policy={families}, root={root_families}"
            )

        accepted_specs = no_growth.get("accepted_specs")
        if not isinstance(accepted_specs, dict):
            blocking_errors.append(f"{item_id}: accepted_specs must be an object")
            accepted_specs = {}
        if sorted(accepted_specs) != families:
            blocking_errors.append(
                f"{item_id}: accepted_specs keys must exactly match dependency_families"
            )
        normalized_specs: dict[str, dict[str, Any]] = {}
        for family in families:
            raw = accepted_specs.get(family)
            if not isinstance(raw, dict) or set(raw) != set(SPEC_KEYS):
                blocking_errors.append(
                    f"{item_id}: accepted_specs.{family} must contain exactly {list(SPEC_KEYS)}"
                )
                continue
            normalized = normalize_spec(
                {
                    "workspace": raw.get("workspace"),
                    "version": raw.get("version"),
                    "features": raw.get("features"),
                    "default-features": raw.get("default_features"),
                    "path": raw.get("path"),
                    "git": raw.get("git"),
                    "registry": raw.get("registry"),
                    "package": raw.get("package"),
                    "branch": raw.get("branch"),
                    "rev": raw.get("rev"),
                    "tag": raw.get("tag"),
                }
            )
            if normalized["workspace"]:
                blocking_errors.append(
                    f"{item_id}: accepted_specs.{family} must describe direct debt"
                )
            normalized_specs[family] = normalized
            root_spec = normalize_spec(workspace_dependencies.get(family))
            if root_spec != normalized:
                blocking_errors.append(
                    f"{item_id}: root [workspace.dependencies].{family} drifted from the "
                    f"accepted specification: current={root_spec}, accepted={normalized}"
                )

        accepted_consumers = no_growth.get("accepted_direct_consumers")
        if not isinstance(accepted_consumers, dict):
            blocking_errors.append(
                f"{item_id}: accepted_direct_consumers must be an object"
            )
            accepted_consumers = {}
        if sorted(accepted_consumers) != families:
            blocking_errors.append(
                f"{item_id}: accepted_direct_consumers keys must exactly match dependency_families"
            )
        accepted_sets: dict[str, set[str]] = {}
        for family in families:
            paths = accepted_consumers.get(family)
            if not isinstance(paths, list) or not all(
                isinstance(path, str) and path.endswith("/Cargo.toml") for path in paths
            ):
                blocking_errors.append(
                    f"{item_id}: accepted_direct_consumers.{family} must be a Cargo.toml path list"
                )
                paths = []
            if paths != sorted(set(paths)):
                blocking_errors.append(
                    f"{item_id}: accepted_direct_consumers.{family} must be sorted and unique"
                )
            accepted_sets[family] = set(paths)

        exception_rule = no_growth.get("exception_rule")
        exception_errors, valid_exception_scopes, exception_warnings = (
            load_dependency_exceptions(
                root,
                exception_rule if isinstance(exception_rule, str) else "",
                today or date.today(),
            )
        )
        blocking_errors.extend(exception_errors)
        warnings.extend(exception_warnings)

        manifests = manifest_paths if manifest_paths is not None else workspace_manifest_paths(root)
        direct, inherited = collect_root_dependency_declarations(
            root, manifests, set(root_families)
        )
        used_exception_scopes: set[str] = set()
        no_growth_violations: list[str] = []

        for row in inherited:
            raw_spec = row["spec"]
            forbidden = [
                key
                for key in (
                    "version",
                    "path",
                    "git",
                    "registry",
                    "package",
                    "branch",
                    "rev",
                    "tag",
                )
                if raw_spec.get(key) is not None
            ]
            if raw_spec.get("features"):
                forbidden.append("features")
            if raw_spec.get("default_features") is not True:
                forbidden.append("default-features")
            if not forbidden:
                continue
            scope = dependency_exception_scope(
                row["manifest_path"], row["dependency"]
            )
            if scope in valid_exception_scopes:
                used_exception_scopes.add(scope)
                continue
            no_growth_violations.append(
                f"{item_id}: {row['manifest_path']}:{row['section']}."
                f"{row['declaration_name']} uses workspace = true with forbidden local "
                f"overrides {sorted(forbidden)}"
            )

        grouped: dict[tuple[str, str], list[dict[str, Any]]] = defaultdict(list)
        for row in direct:
            grouped[(row["manifest_path"], row["dependency"])].append(row)

        current_sets: dict[str, set[str]] = {family: set() for family in root_families}
        for (manifest_path, family), rows in sorted(grouped.items()):
            current_sets[family].add(manifest_path)
            scope = dependency_exception_scope(manifest_path, family)
            accepted = manifest_path in accepted_sets.get(family, set())
            exact_spec = (
                len(rows) == 1
                and family in normalized_specs
                and rows[0]["spec"] == normalized_specs[family]
                and rows[0]["declaration_name"] == family
            )
            if accepted and exact_spec:
                continue
            if scope in valid_exception_scopes:
                used_exception_scopes.add(scope)
                continue
            if not accepted:
                no_growth_violations.append(
                    f"{item_id}: new direct consumer {scope} is outside the accepted debt inventory"
                )
            elif len(rows) != 1:
                no_growth_violations.append(
                    f"{item_id}: {scope} has {len(rows)} direct declarations; accepted multiplicity is 1"
                )
            else:
                no_growth_violations.append(
                    f"{item_id}: {scope} changed its accepted direct declaration: "
                    f"current={rows[0]['spec']}, accepted={normalized_specs.get(family)}"
                )

        warnings.extend(
            f"unused workspace dependency exception scope: {scope}"
            for scope in sorted(valid_exception_scopes - used_exception_scopes)
        )
        blocking_errors.extend(no_growth_violations)
        no_growth_result = {
            "id": item_id,
            "dependency_families": root_families,
            "baseline_direct_consumer_count": sum(
                len(paths) for paths in accepted_sets.values()
            ),
            "current_direct_consumer_count": sum(
                len(paths) for paths in current_sets.values()
            ),
            "reduced_direct_consumer_count": sum(
                len(accepted_sets.get(family, set()) - current_sets.get(family, set()))
                for family in root_families
            ),
            "exception_count": len(used_exception_scopes),
            "violation_count": len(no_growth_violations),
            "families": [
                {
                    "name": family,
                    "baseline_direct_consumers": len(accepted_sets.get(family, set())),
                    "current_direct_consumers": len(current_sets.get(family, set())),
                    "reduced_direct_consumers": len(
                        accepted_sets.get(family, set()) - current_sets.get(family, set())
                    ),
                }
                for family in root_families
            ],
        }

    return {
        "schema_version": SCHEMA_VERSION,
        "blocking_errors": blocking_errors,
        "warnings": warnings,
        "policies": policy_results,
        "no_growth": no_growth_result,
    }


def markdown_report(report: dict[str, Any]) -> str:
    lines = [
        "# Workspace Dependency Policy",
        "",
        "> Calibrated inheritance policies and the root-family no-growth debt inventory are blocking.",
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
    no_growth = report.get("no_growth")
    if no_growth:
        lines.extend(
            [
                "",
                "## Root workspace dependency no-growth inventory",
                "",
                "| Dependency | Baseline direct consumers | Current direct consumers | Reduced |",
                "|---|---:|---:|---:|",
            ]
        )
        for family in no_growth["families"]:
            lines.append(
                f"| `{family['name']}` | {family['baseline_direct_consumers']} | "
                f"{family['current_direct_consumers']} | "
                f"{family['reduced_direct_consumers']} |"
            )
        lines.extend(
            [
                "",
                f"Active exceptions: {no_growth['exception_count']}.  ",
                f"No-growth violations: {no_growth['violation_count']}.",
            ]
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
    except (
        RuntimeError,
        OSError,
        ValueError,
        json.JSONDecodeError,
        tomllib.TOMLDecodeError,
    ) as error:
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
