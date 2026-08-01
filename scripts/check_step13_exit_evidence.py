#!/usr/bin/env python3
"""Enforce the remaining ADR-031 repository-step-13 exit-evidence budgets."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import sys
import tomllib
from typing import Any

SCHEMA = "crm.step13-exit-evidence/v1"
POLICY_PATH = "step13-complexity-policy.json"


def _feature_key(value: dict[str, Any]) -> tuple[bool, tuple[str, ...]]:
    return bool(value.get("default_features", True)), tuple(sorted(value.get("features", [])))


def _feature_map(rows: list[dict[str, Any]]) -> dict[str, set[tuple[bool, tuple[str, ...]]]]:
    return {
        row["name"]: {_feature_key(variant) for variant in row.get("variants", [])}
        for row in rows
    }


def _accepted_feature_map(value: dict[str, Any]) -> dict[str, set[tuple[bool, tuple[str, ...]]]]:
    return {
        name: {_feature_key(variant) for variant in variants}
        for name, variants in value.items()
    }


def _version_map(rows: list[dict[str, Any]]) -> dict[str, set[str]]:
    return {
        row["name"]: set(row.get("requirements", []))
        for row in rows
    }


def validate_exit_evidence(
    complexity: dict[str, Any],
    dependency_governance: dict[str, Any],
    policy: dict[str, Any],
) -> list[str]:
    errors: list[str] = []
    exit_policy = policy.get("exit_evidence")
    if not isinstance(exit_policy, dict):
        return ["step13-complexity-policy.json is missing exit_evidence"]

    commit_sha = complexity.get("commit_sha")
    if (
        not isinstance(commit_sha, str)
        or len(commit_sha) != 40
        or any(character not in "0123456789abcdef" for character in commit_sha)
    ):
        errors.append("complexity report must identify the exact 40-character commit SHA")

    workspace_budget = exit_policy.get("workspace_budget", {})
    workspace = complexity.get("workspace_baseline", {}).get("workspace", {})
    public_surface = complexity.get("public_rust_surface", {})
    suppressions = complexity.get("suppression_inventory", {})
    if workspace.get("package_count") != workspace_budget.get("expected_workspace_packages"):
        errors.append(
            "workspace package count must remain exactly "
            f"{workspace_budget.get('expected_workspace_packages')}, found {workspace.get('package_count')}"
        )
    for field, budget_field, label in (
        ("internal_dependency_edges", "maximum_internal_dependency_edges", "internal dependency edges"),
        ("maximum_dependency_depth", "maximum_dependency_depth", "maximum dependency depth"),
    ):
        actual = (
            complexity.get("dependency_graph", {}).get("maximum_depth")
            if field == "maximum_dependency_depth"
            else workspace.get(field)
        )
        maximum = workspace_budget.get(budget_field)
        if not isinstance(actual, int) or not isinstance(maximum, int) or actual > maximum:
            errors.append(f"{label} grew beyond {maximum}: {actual}")
    public_items = public_surface.get("total_public_items")
    public_maximum = workspace_budget.get("maximum_public_rust_items")
    if not isinstance(public_items, int) or public_items > public_maximum:
        errors.append(f"public Rust surface grew beyond {public_maximum}: {public_items}")
    suppression_count = suppressions.get("entry_count")
    suppression_maximum = workspace_budget.get("maximum_suppression_occurrences")
    if not isinstance(suppression_count, int) or suppression_count > suppression_maximum:
        errors.append(
            f"suppression occurrence count grew beyond {suppression_maximum}: {suppression_count}"
        )

    central = {
        row.get("package"): row
        for row in complexity.get("central_systems", [])
        if isinstance(row, dict)
    }
    metric_fields = (
        ("direct_dependency_count", "maximum_direct_dependencies", "direct dependencies"),
        ("direct_consumer_count", "maximum_direct_consumers", "direct consumers"),
        ("transitive_reverse_impact", "maximum_transitive_reverse_impact", "reverse impact"),
        ("dependency_depth", "maximum_dependency_depth", "dependency depth"),
        ("public_items", "maximum_public_items", "public items"),
    )
    for package, budget in exit_policy.get("central_system_budgets", {}).items():
        actual = central.get(package)
        if not actual:
            errors.append(f"central system {package} is missing from the complexity report")
            continue
        if actual.get("role") != budget.get("role"):
            errors.append(
                f"central system {package} role changed from {budget.get('role')} to {actual.get('role')}"
            )
        for actual_field, budget_field, label in metric_fields:
            actual_value = actual.get(actual_field)
            maximum = budget.get(budget_field)
            if not isinstance(actual_value, int) or not isinstance(maximum, int) or actual_value > maximum:
                errors.append(f"{package} {label} grew beyond {maximum}: {actual_value}")
        loc = actual.get("source", {}).get("non_comment_lines")
        loc_maximum = budget.get("maximum_non_comment_loc")
        if not isinstance(loc, int) or not isinstance(loc_maximum, int) or loc > loc_maximum:
            errors.append(f"{package} non-comment LOC grew beyond {loc_maximum}: {loc}")

    for package, accepted_dependencies in exit_policy.get(
        "process_host_dependency_allowlists", {}
    ).items():
        actual = central.get(package)
        if not actual:
            continue
        unexpected = sorted(set(actual.get("direct_dependencies", [])) - set(accepted_dependencies))
        if unexpected:
            errors.append(
                f"{package} added unmeasured direct dependencies: {unexpected}"
            )

    manifest_surfaces = {
        row.get("package"): row
        for row in complexity.get("process_host_manifest_surfaces", [])
        if isinstance(row, dict)
    }
    section_fields = (
        ("runtime_internal_dependencies", "maximum_runtime_internal_dependencies", "accepted_runtime_internal_dependencies", "runtime"),
        ("dev_internal_dependencies", "maximum_dev_internal_dependencies", "accepted_dev_internal_dependencies", "dev"),
        ("build_internal_dependencies", "maximum_build_internal_dependencies", "accepted_build_internal_dependencies", "build"),
    )
    for package, budget in exit_policy.get(
        "process_host_manifest_budgets", {}
    ).items():
        actual = manifest_surfaces.get(package)
        if not actual:
            errors.append(f"process-host manifest surface {package} is missing")
            continue
        if actual.get("manifest_path") != budget.get("manifest_path"):
            errors.append(
                f"{package} manifest path changed from {budget.get('manifest_path')} to {actual.get('manifest_path')}"
            )
        justification = budget.get("non_growth_justification")
        if not isinstance(justification, str) or not justification.strip():
            errors.append(f"{package} process-host non-growth justification is missing")
        for actual_field, maximum_field, accepted_field, label in section_fields:
            actual_dependencies = actual.get(actual_field)
            accepted_dependencies = budget.get(accepted_field)
            maximum = budget.get(maximum_field)
            if not isinstance(actual_dependencies, list) or not isinstance(accepted_dependencies, list):
                errors.append(f"{package} {label} dependency evidence is malformed")
                continue
            if not isinstance(maximum, int) or len(actual_dependencies) > maximum:
                errors.append(
                    f"{package} {label} internal dependencies grew beyond {maximum}: {len(actual_dependencies)}"
                )
            unexpected = sorted(set(actual_dependencies) - set(accepted_dependencies))
            if unexpected:
                errors.append(
                    f"{package} added unmeasured {label} internal dependencies: {unexpected}"
                )

    change_cost = {
        row.get("id"): row
        for row in complexity.get("representative_change_cost", [])
        if isinstance(row, dict)
    }
    cost_fields = (
        ("file_count", "maximum_files", "files"),
        ("package_count", "maximum_packages", "packages"),
        ("central_file_count", "maximum_central_files", "central files"),
        ("workflow_file_count", "maximum_workflow_files", "workflow files"),
    )
    for exemplar, budget in exit_policy.get(
        "representative_change_cost_budgets", {}
    ).items():
        actual = change_cost.get(exemplar)
        if not actual:
            errors.append(f"representative change-cost exemplar {exemplar} is missing")
            continue
        if actual.get("kind") != budget.get("kind"):
            errors.append(
                f"representative exemplar {exemplar} kind changed from {budget.get('kind')} to {actual.get('kind')}"
            )
        for actual_field, budget_field, label in cost_fields:
            actual_value = actual.get(actual_field)
            maximum = budget.get(budget_field)
            if not isinstance(actual_value, int) or not isinstance(maximum, int) or actual_value > maximum:
                errors.append(
                    f"representative exemplar {exemplar} {label} grew beyond {maximum}: {actual_value}"
                )

    dependency_policy = exit_policy.get("dependency_governance", {})
    for field, budget_field, label in (
        ("declaration_count", "maximum_declaration_count", "dependency declarations"),
        ("workspace_dependency_count", "maximum_workspace_dependency_count", "workspace dependency families"),
    ):
        actual = dependency_governance.get(field)
        maximum = dependency_policy.get(budget_field)
        if not isinstance(actual, int) or not isinstance(maximum, int) or actual > maximum:
            errors.append(f"{label} grew beyond {maximum}: {actual}")
    heavy_count = len(dependency_governance.get("heavy_feature_declarations", []))
    heavy_maximum = dependency_policy.get("maximum_heavy_feature_declarations")
    if not isinstance(heavy_maximum, int) or heavy_count > heavy_maximum:
        errors.append(
            f"heavy-feature declarations grew beyond {heavy_maximum}: {heavy_count}"
        )

    accepted_versions = {
        name: set(requirements)
        for name, requirements in dependency_policy.get(
            "accepted_version_divergence", {}
        ).items()
    }
    for name, requirements in _version_map(
        dependency_governance.get("version_divergence", [])
    ).items():
        if name not in accepted_versions:
            errors.append(f"new dependency version divergence: {name} {sorted(requirements)}")
            continue
        unexpected = sorted(requirements - accepted_versions[name])
        if unexpected:
            errors.append(f"dependency {name} added version requirements: {unexpected}")

    accepted_features = _accepted_feature_map(
        dependency_policy.get("accepted_feature_divergence", {})
    )
    for name, variants in _feature_map(
        dependency_governance.get("feature_divergence", [])
    ).items():
        if name not in accepted_features:
            errors.append(f"new dependency feature divergence: {name}")
            continue
        unexpected = sorted(variants - accepted_features[name])
        if unexpected:
            errors.append(f"dependency {name} added feature variants: {unexpected}")
    return errors


def process_host_manifest_surfaces(
    root: Path, policy: dict[str, Any]
) -> list[dict[str, Any]]:
    exit_policy = policy.get("exit_evidence", {})
    rows: list[dict[str, Any]] = []
    sections = (
        ("dependencies", "runtime_internal_dependencies"),
        ("dev-dependencies", "dev_internal_dependencies"),
        ("build-dependencies", "build_internal_dependencies"),
    )
    for package, budget in sorted(
        exit_policy.get("process_host_manifest_budgets", {}).items()
    ):
        manifest_path = budget.get("manifest_path")
        if not isinstance(manifest_path, str) or not manifest_path:
            rows.append({"package": package, "manifest_path": manifest_path})
            continue
        path = root / manifest_path
        document = tomllib.loads(path.read_text(encoding="utf-8"))
        row: dict[str, Any] = {"package": package, "manifest_path": manifest_path}
        for section, output_field in sections:
            dependencies = document.get(section, {})
            internal: list[str] = []
            if isinstance(dependencies, dict):
                for name, specification in dependencies.items():
                    actual_name = (
                        specification.get("package", name)
                        if isinstance(specification, dict)
                        else name
                    )
                    if isinstance(actual_name, str) and actual_name.startswith("crm-"):
                        internal.append(actual_name)
            row[output_field] = sorted(set(internal))
        rows.append(row)
    return rows


def _dependency_governance(root: Path) -> dict[str, Any]:
    try:
        from analyze_workspace_governance import (
            cargo_metadata,
            dependency_metrics,
            load_toml,
            workspace_packages,
        )
    except ModuleNotFoundError:
        from scripts.analyze_workspace_governance import (
            cargo_metadata,
            dependency_metrics,
            load_toml,
            workspace_packages,
        )
    metadata = cargo_metadata(root)
    packages = workspace_packages(metadata)
    root_manifest = load_toml(root / "Cargo.toml")
    workspace_dependencies = root_manifest.get("workspace", {}).get(
        "dependencies", {}
    )
    return dependency_metrics(root, packages, workspace_dependencies)


def build_report(root: Path) -> dict[str, Any]:
    try:
        from analyze_step13_complexity import build_report as build_complexity_report
    except ModuleNotFoundError:
        from scripts.analyze_step13_complexity import (
            build_report as build_complexity_report,
        )
    policy = json.loads((root / POLICY_PATH).read_text(encoding="utf-8"))
    complexity = build_complexity_report(root)
    complexity["process_host_manifest_surfaces"] = process_host_manifest_surfaces(
        root, policy
    )
    dependencies = _dependency_governance(root)
    errors = validate_exit_evidence(complexity, dependencies, policy)
    return {
        "schema_version": SCHEMA,
        "commit_sha": complexity.get("commit_sha"),
        "accepted_source": policy.get("exit_evidence", {}).get("accepted_source"),
        "mode": "calibrated-blocking",
        "status": "pass" if not errors else "fail",
        "blocking_errors": errors,
        "workspace": {
            "package_count": complexity.get("workspace_baseline", {})
            .get("workspace", {})
            .get("package_count"),
            "internal_dependency_edges": complexity.get("workspace_baseline", {})
            .get("workspace", {})
            .get("internal_dependency_edges"),
            "maximum_dependency_depth": complexity.get("dependency_graph", {}).get(
                "maximum_depth"
            ),
            "public_rust_items": complexity.get("public_rust_surface", {}).get(
                "total_public_items"
            ),
            "suppression_occurrences": complexity.get("suppression_inventory", {}).get(
                "entry_count"
            ),
        },
        "central_systems": complexity.get("central_systems", []),
        "process_host_manifest_surfaces": complexity.get(
            "process_host_manifest_surfaces", []
        ),
        "representative_change_cost": complexity.get(
            "representative_change_cost", []
        ),
        "dependency_governance": {
            "declaration_count": dependencies.get("declaration_count"),
            "workspace_dependency_count": dependencies.get(
                "workspace_dependency_count"
            ),
            "version_divergence": dependencies.get("version_divergence", []),
            "feature_divergence": dependencies.get("feature_divergence", []),
            "heavy_feature_declaration_count": len(
                dependencies.get("heavy_feature_declarations", [])
            ),
        },
    }


def markdown_report(report: dict[str, Any]) -> str:
    workspace = report["workspace"]
    lines = [
        "# Repository Step 13 Exit-Evidence Enforcement",
        "",
        f"Commit: `{report['commit_sha']}`",
        f"Accepted calibration source: `{report['accepted_source']}`",
        f"Status: **{report['status'].upper()}**",
        "",
        "## Workspace",
        "",
        "| Metric | Value |",
        "|---|---:|",
        f"| Workspace packages | {workspace['package_count']} |",
        f"| Internal dependency edges | {workspace['internal_dependency_edges']} |",
        f"| Maximum dependency depth | {workspace['maximum_dependency_depth']} |",
        f"| Public Rust items | {workspace['public_rust_items']} |",
        f"| Suppression occurrences | {workspace['suppression_occurrences']} |",
        "",
        "## Process-host surfaces",
        "",
        "| Package | Direct dependencies | Depth | Public items | LOC |",
        "|---|---:|---:|---:|---:|",
    ]
    for row in report["central_systems"]:
        if row.get("package") not in {"crm-application-runtime", "crm-api"}:
            continue
        lines.append(
            f"| `{row['package']}` | {row['direct_dependency_count']} | "
            f"{row['dependency_depth']} | {row['public_items']} | "
            f"{row.get('source', {}).get('non_comment_lines', 0)} |"
        )
    lines.extend(
        [
            "",
            "## Manifest dependency surfaces",
            "",
            "| Package | Runtime internal | Dev internal | Build internal |",
            "|---|---:|---:|---:|",
        ]
    )
    for row in report["process_host_manifest_surfaces"]:
        lines.append(
            f"| `{row['package']}` | "
            f"{len(row.get('runtime_internal_dependencies', []))} | "
            f"{len(row.get('dev_internal_dependencies', []))} | "
            f"{len(row.get('build_internal_dependencies', []))} |"
        )
    lines.extend(["", "## Blocking errors", ""])
    if report["blocking_errors"]:
        lines.extend(f"- {error}" for error in report["blocking_errors"])
    else:
        lines.append("- None")
    lines.append("")
    return "\n".join(lines)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path.cwd())
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--json-output", type=Path)
    parser.add_argument("--markdown-output", type=Path)
    args = parser.parse_args()
    try:
        report = build_report(args.root.resolve())
    except (OSError, RuntimeError, ValueError, json.JSONDecodeError) as error:
        print(f"step-13 exit-evidence check failed: {error}", file=sys.stderr)
        return 1
    json_text = json.dumps(report, indent=2, sort_keys=True) + "\n"
    markdown_text = markdown_report(report)
    if args.json_output:
        args.json_output.parent.mkdir(parents=True, exist_ok=True)
        args.json_output.write_text(json_text, encoding="utf-8")
    else:
        print(json_text, end="")
    if args.markdown_output:
        args.markdown_output.parent.mkdir(parents=True, exist_ok=True)
        args.markdown_output.write_text(markdown_text, encoding="utf-8")
    if args.check and report["blocking_errors"]:
        for error in report["blocking_errors"]:
            print(f"- {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
