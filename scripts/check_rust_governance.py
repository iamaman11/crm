#!/usr/bin/env python3
"""Measure and enforce the repository Rust toolchain, MSRV and lint policy."""

from __future__ import annotations

import argparse
from datetime import date
import json
from pathlib import Path
import subprocess
import sys
import tomllib
from typing import Any, Iterable

POLICY_SCHEMA = "crm.rust-governance-policy/v1"
REPORT_SCHEMA = "crm.rust-governance-report/v1"
POLICY_FILE = "rust-governance-policy.json"
TOOLCHAIN_FILE = "rust-toolchain.toml"
ARCHITECTURE_GOVERNANCE_FILE = "architecture-governance.json"


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


def load_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def cargo_metadata(root: Path) -> dict[str, Any]:
    return json.loads(
        run(
            ["cargo", "metadata", "--format-version", "1", "--locked", "--no-deps"],
            root,
        )
    )


def workspace_packages(metadata: dict[str, Any]) -> list[dict[str, Any]]:
    members = set(metadata.get("workspace_members", []))
    return sorted(
        [
            package
            for package in metadata.get("packages", [])
            if package.get("id") in members
        ],
        key=lambda package: package["name"],
    )


def package_adoption(
    root: Path, packages: Iterable[dict[str, Any]]
) -> tuple[dict[str, int], list[dict[str, Any]]]:
    counts = {
        "rust_version_inherited": 0,
        "rust_version_direct": 0,
        "rust_version_missing": 0,
        "lints_inherited": 0,
        "lints_direct": 0,
        "lints_missing": 0,
    }
    rows: list[dict[str, Any]] = []
    for package in packages:
        manifest_path = Path(package["manifest_path"])
        relative = manifest_path.resolve().relative_to(root.resolve()).as_posix()
        manifest = load_toml(manifest_path)
        package_table = manifest.get("package", {})
        rust_version = package_table.get("rust-version")
        if isinstance(rust_version, dict) and rust_version.get("workspace") is True:
            rust_mode = "workspace"
            counts["rust_version_inherited"] += 1
        elif isinstance(rust_version, str):
            rust_mode = "direct"
            counts["rust_version_direct"] += 1
        else:
            rust_mode = "missing"
            counts["rust_version_missing"] += 1

        lints = manifest.get("lints")
        if isinstance(lints, dict) and lints.get("workspace") is True:
            lint_mode = "workspace"
            counts["lints_inherited"] += 1
        elif isinstance(lints, dict):
            lint_mode = "direct"
            counts["lints_direct"] += 1
        else:
            lint_mode = "missing"
            counts["lints_missing"] += 1
        rows.append(
            {
                "package": package["name"],
                "manifest_path": relative,
                "rust_version_mode": rust_mode,
                "lint_mode": lint_mode,
            }
        )
    return counts, rows


def active_rust_exceptions(
    root: Path, today: date | None = None
) -> tuple[dict[str, dict[str, Any]], list[str]]:
    errors: list[str] = []
    today = today or date.today()
    registry = load_json(root / ARCHITECTURE_GOVERNANCE_FILE)
    active: dict[str, dict[str, Any]] = {}
    for item in registry.get("exceptions", []):
        if not isinstance(item, dict) or item.get("rule") != "rust-governance":
            continue
        scope = item.get("scope")
        identifier = item.get("id", "unnamed-rust-governance-exception")
        if not isinstance(scope, str) or not scope.endswith("/Cargo.toml"):
            errors.append(f"{identifier}: rust-governance scope must be an exact Cargo.toml path")
            continue
        try:
            expiry = date.fromisoformat(item["expiry_date"])
        except (KeyError, TypeError, ValueError):
            errors.append(f"{identifier}: expiry_date must be an ISO date")
            continue
        if expiry < today:
            errors.append(f"{identifier}: exception expired on {expiry.isoformat()}")
            continue
        if scope in active:
            errors.append(f"{identifier}: duplicate rust-governance scope {scope}")
            continue
        active[scope] = item
    return active, errors


def workspace_members_from_text(text: str) -> set[str]:
    return {
        str(member)
        for member in tomllib.loads(text).get("workspace", {}).get("members", [])
    }


def new_workspace_members(root: Path, base_ref: str | None) -> list[str]:
    if not base_ref:
        return []
    previous = workspace_members_from_text(
        run(["git", "show", f"{base_ref}:Cargo.toml"], root)
    )
    current = workspace_members_from_text((root / "Cargo.toml").read_text(encoding="utf-8"))
    return sorted(current - previous)


def compiler_messages(
    path: Path | None, workspace_package_ids: set[str]
) -> dict[str, int | None]:
    if path is None:
        return {"warnings": None, "errors": None}
    warnings = 0
    errors = 0
    for raw_line in path.read_text(encoding="utf-8").splitlines():
        try:
            item = json.loads(raw_line)
        except json.JSONDecodeError:
            continue
        if item.get("reason") != "compiler-message":
            continue
        if item.get("package_id") not in workspace_package_ids:
            continue
        level = item.get("message", {}).get("level")
        if level == "warning":
            warnings += 1
        elif level == "error":
            errors += 1
    return {"warnings": warnings, "errors": errors}


def tool_versions(root: Path, skip: bool) -> dict[str, str | None]:
    if skip:
        return {"rustc": None, "cargo": None, "rustfmt": None, "clippy": None}
    rustc_verbose = run(["rustc", "--version", "--verbose"], root)
    release = next(
        (
            line.partition(":")[2].strip()
            for line in rustc_verbose.splitlines()
            if line.startswith("release:")
        ),
        "",
    )
    return {
        "rustc": release,
        "cargo": run(["cargo", "--version"], root).strip(),
        "rustfmt": run(["rustfmt", "--version"], root).strip(),
        "clippy": run(["cargo", "clippy", "--version"], root).strip(),
    }


def validate(
    root: Path,
    policy: dict[str, Any],
    metadata: dict[str, Any],
    base_ref: str | None,
    rustc_json: Path | None,
    clippy_json: Path | None,
    require_lint_measurements: bool,
    skip_tool_versions: bool,
) -> dict[str, Any]:
    errors: list[str] = []
    warnings: list[str] = []
    if policy.get("schema_version") != POLICY_SCHEMA:
        errors.append(f"policy schema_version must be {POLICY_SCHEMA}")

    supported = policy.get("supported_toolchain", {})
    baseline = policy.get("measured_baseline", {})
    expected_lints = policy.get("workspace_lints", {})
    root_manifest = load_toml(root / "Cargo.toml")
    workspace = root_manifest.get("workspace", {})
    workspace_package = workspace.get("package", {})
    toolchain = load_toml(root / TOOLCHAIN_FILE).get("toolchain", {})

    exact_root_values = {
        "workspace.package.edition": (
            workspace_package.get("edition"),
            supported.get("edition"),
        ),
        "workspace.package.rust-version": (
            workspace_package.get("rust-version"),
            supported.get("minimum_rust_version"),
        ),
        "workspace.resolver": (str(workspace.get("resolver", "")), supported.get("resolver")),
        "rust-toolchain.channel": (toolchain.get("channel"), supported.get("channel")),
        "rust-toolchain.profile": (toolchain.get("profile"), supported.get("profile")),
    }
    for field, (actual, expected) in exact_root_values.items():
        if actual != expected:
            errors.append(f"{field} must be {expected!r}, found {actual!r}")

    actual_components = sorted(set(toolchain.get("components", [])))
    expected_components = sorted(set(supported.get("components", [])))
    if actual_components != expected_components:
        errors.append(
            f"rust-toolchain.components must be {expected_components}, found {actual_components}"
        )

    actual_workspace_lints = workspace.get("lints", {})
    if actual_workspace_lints != expected_lints:
        errors.append("workspace lint tables differ from rust-governance-policy.json")

    packages = workspace_packages(metadata)
    adoption_counts, package_rows = package_adoption(root, packages)
    package_count = len(packages)
    expected_package_count = baseline.get("effective_workspace_packages")
    if package_count != expected_package_count:
        errors.append(
            f"effective workspace package count must be {expected_package_count}, found {package_count}"
        )

    active_exceptions, exception_errors = active_rust_exceptions(root)
    errors.extend(exception_errors)
    expected_exception_count = baseline.get("active_rust_governance_exceptions")
    if len(active_exceptions) != expected_exception_count:
        errors.append(
            "active rust-governance exception count must be "
            f"{expected_exception_count}, found {len(active_exceptions)}"
        )

    limits = {
        "rust_version_inherited": (
            ">=",
            baseline.get("minimum_rust_version_inherited_packages", 0),
        ),
        "lints_inherited": (
            ">=",
            baseline.get("minimum_workspace_lints_inherited_packages", 0),
        ),
        "rust_version_missing": (
            "<=",
            baseline.get("maximum_missing_rust_version_packages", package_count),
        ),
        "lints_missing": (
            "<=",
            baseline.get("maximum_missing_workspace_lints_packages", package_count),
        ),
        "rust_version_direct": (
            "<=",
            baseline.get("maximum_direct_rust_version_declarations", 0),
        ),
        "lints_direct": ("<=", baseline.get("maximum_direct_lint_tables", 0)),
    }
    for name, (operator, limit) in limits.items():
        actual = adoption_counts[name]
        if (operator == ">=" and actual < limit) or (operator == "<=" and actual > limit):
            errors.append(f"{name} must be {operator} {limit}, found {actual}")

    rows_by_manifest = {row["manifest_path"]: row for row in package_rows}
    for row in package_rows:
        manifest_path = row["manifest_path"]
        if manifest_path in active_exceptions:
            continue
        if row["rust_version_mode"] == "direct":
            errors.append(
                f"{manifest_path}: direct rust-version is forbidden; inherit the workspace value"
            )
        if row["lint_mode"] == "direct":
            errors.append(
                f"{manifest_path}: direct lint table is forbidden; inherit workspace lints"
            )

    added_members = new_workspace_members(root, base_ref)
    for member in added_members:
        manifest_path = f"{member}/Cargo.toml"
        row = rows_by_manifest.get(manifest_path)
        if row is None:
            errors.append(f"new workspace member {member} has no metadata package manifest")
            continue
        if manifest_path in active_exceptions:
            continue
        if row["rust_version_mode"] != "workspace":
            errors.append(
                f"new workspace member {member} must set rust-version.workspace = true"
            )
        if row["lint_mode"] != "workspace":
            errors.append(f"new workspace member {member} must set [lints] workspace = true")

    versions = tool_versions(root, skip_tool_versions)
    if versions["rustc"] is not None and versions["rustc"] != supported.get("channel"):
        errors.append(
            f"executing rustc release must be {supported.get('channel')}, found {versions['rustc']}"
        )

    workspace_ids = {
        package["id"]
        for package in packages
        if isinstance(package.get("id"), str)
    }
    rustc_measurement = compiler_messages(rustc_json, workspace_ids)
    clippy_measurement = compiler_messages(clippy_json, workspace_ids)
    if require_lint_measurements and (rustc_json is None or clippy_json is None):
        errors.append("both --rustc-json and --clippy-json are required for lint acceptance")
    if rustc_measurement["warnings"] is not None:
        if rustc_measurement["warnings"] > baseline.get("maximum_workspace_rustc_warnings", 0):
            errors.append(
                "workspace rustc warning budget exceeded: "
                f"{rustc_measurement['warnings']} warnings"
            )
        if rustc_measurement["errors"]:
            errors.append(f"workspace rustc measurement contains {rustc_measurement['errors']} errors")
    if clippy_measurement["warnings"] is not None:
        if clippy_measurement["warnings"] > baseline.get(
            "maximum_workspace_clippy_warnings", 0
        ):
            errors.append(
                "workspace Clippy warning budget exceeded: "
                f"{clippy_measurement['warnings']} warnings"
            )
        if clippy_measurement["errors"]:
            errors.append(
                f"workspace Clippy measurement contains {clippy_measurement['errors']} errors"
            )

    if adoption_counts["rust_version_missing"]:
        warnings.append(
            f"{adoption_counts['rust_version_missing']} legacy packages have not yet inherited rust-version"
        )
    if adoption_counts["lints_missing"]:
        warnings.append(
            f"{adoption_counts['lints_missing']} legacy packages have not yet inherited workspace lints"
        )

    return {
        "schema_version": REPORT_SCHEMA,
        "commit_sha": run(["git", "rev-parse", "HEAD"], root).strip(),
        "mode": "blocking-policy-with-measured-legacy-cohort",
        "supported_toolchain": supported,
        "tool_versions": versions,
        "workspace": {
            "package_count": package_count,
            "adoption": adoption_counts,
            "new_workspace_members": added_members,
        },
        "lint_measurement": {
            "rustc": rustc_measurement,
            "clippy": clippy_measurement,
        },
        "exceptions": {
            "active_count": len(active_exceptions),
            "scopes": sorted(active_exceptions),
        },
        "governance": {"blocking_errors": errors, "warnings": warnings},
    }


def markdown_report(report: dict[str, Any]) -> str:
    supported = report["supported_toolchain"]
    versions = report["tool_versions"]
    adoption = report["workspace"]["adoption"]
    lint = report["lint_measurement"]
    errors = report["governance"]["blocking_errors"]
    warnings = report["governance"]["warnings"]
    lines = [
        "# Rust Toolchain, MSRV and Lint Governance Report",
        "",
        f"Commit: `{report['commit_sha']}`",
        "",
        "## Supported boundary",
        "",
        "| Property | Value |",
        "|---|---|",
        f"| CI/developer toolchain | `{supported['channel']}` |",
        f"| Workspace rust-version | `{supported['minimum_rust_version']}` |",
        f"| Edition | `{supported['edition']}` |",
        f"| Resolver | `{supported['resolver']}` |",
        f"| Executing rustc | `{versions['rustc'] or 'not measured'}` |",
        f"| Workspace packages | {report['workspace']['package_count']} |",
        "",
        "## Adoption baseline",
        "",
        "| Metric | Count |",
        "|---|---:|",
        f"| rust-version inherited | {adoption['rust_version_inherited']} |",
        f"| rust-version missing legacy cohort | {adoption['rust_version_missing']} |",
        f"| direct rust-version overrides | {adoption['rust_version_direct']} |",
        f"| workspace lints inherited | {adoption['lints_inherited']} |",
        f"| workspace lints missing legacy cohort | {adoption['lints_missing']} |",
        f"| direct lint tables | {adoption['lints_direct']} |",
        "",
        "## Measured lint debt",
        "",
        "| Measurement | Warnings | Errors |",
        "|---|---:|---:|",
        f"| rustc | {lint['rustc']['warnings'] if lint['rustc']['warnings'] is not None else 'not measured'} | {lint['rustc']['errors'] if lint['rustc']['errors'] is not None else 'not measured'} |",
        f"| Clippy | {lint['clippy']['warnings'] if lint['clippy']['warnings'] is not None else 'not measured'} | {lint['clippy']['errors'] if lint['clippy']['errors'] is not None else 'not measured'} |",
        "",
        "## Governance result",
        "",
        f"Blocking errors: **{len(errors)}**  ",
        f"Warnings: **{len(warnings)}**",
    ]
    if errors:
        lines.extend(["", "### Blocking errors", ""])
        lines.extend(f"- {error}" for error in errors)
    if warnings:
        lines.extend(["", "### Warnings", ""])
        lines.extend(f"- {warning}" for warning in warnings)
    lines.extend(
        [
            "",
            "The legacy missing-inheritance cohort is measured and may only shrink. New packages must inherit both policies or carry an exact time-bounded architecture exception.",
        ]
    )
    return "\n".join(lines) + "\n"


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--base-ref")
    parser.add_argument("--rustc-json", type=Path)
    parser.add_argument("--clippy-json", type=Path)
    parser.add_argument("--require-lint-measurements", action="store_true")
    parser.add_argument("--skip-tool-versions", action="store_true")
    parser.add_argument("--json-output", type=Path)
    parser.add_argument("--markdown-output", type=Path)
    parser.add_argument("--check", action="store_true")
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv or sys.argv[1:])
    root = args.root.resolve()
    policy = load_json(root / POLICY_FILE)
    report = validate(
        root,
        policy,
        cargo_metadata(root),
        args.base_ref,
        args.rustc_json,
        args.clippy_json,
        args.require_lint_measurements,
        args.skip_tool_versions,
    )
    rendered_json = json.dumps(report, indent=2, sort_keys=True) + "\n"
    rendered_markdown = markdown_report(report)
    if args.json_output:
        args.json_output.parent.mkdir(parents=True, exist_ok=True)
        args.json_output.write_text(rendered_json, encoding="utf-8")
    else:
        print(rendered_json, end="")
    if args.markdown_output:
        args.markdown_output.parent.mkdir(parents=True, exist_ok=True)
        args.markdown_output.write_text(rendered_markdown, encoding="utf-8")
    if args.check and report["governance"]["blocking_errors"]:
        for error in report["governance"]["blocking_errors"]:
            print(f"ERROR: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
