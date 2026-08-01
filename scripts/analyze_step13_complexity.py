#!/usr/bin/env python3
"""Produce the ADR-031 current-main complexity, bypass and change-cost report."""

from __future__ import annotations

import argparse
from collections import defaultdict, deque
import json
from pathlib import Path
import re
import subprocess
import sys
from typing import Any

from scripts.analyze_workspace import build_report as build_workspace_report
from scripts.analyze_workspace import load_cargo_metadata, package_metrics, source_loc

SCHEMA_VERSION = "crm.step13-complexity-baseline/v1"
POLICY_PATH = "step13-complexity-policy.json"
SKIPPED_DIRECTORIES = {".git", "target", "node_modules", "artifacts"}

PUBLIC_ITEM_PATTERN = re.compile(
    r"^\s*pub(?:\s*\([^)]*\))?\s+(?:async\s+)?"
    r"(?:const|enum|extern|fn|mod|static|struct|trait|type|union|use)\b"
)
SOURCE_SUPPRESSION_PATTERN = re.compile(
    r"#(?:!)?\[\s*(allow|expect)\s*\(([^)]*)\)\s*\]"
)
IGNORE_PATTERN = re.compile(r"#(?:!)?\[\s*ignore(?:\s*=\s*\"([^\"]+)\")?\s*\]")
DIRECT_LINT_HEADER_PATTERN = re.compile(r"^\s*\[lints(?:\.[^\]]+)?\]\s*$")
WORKSPACE_LINT_INHERITANCE_PATTERN = re.compile(
    r"^\s*\[lints\]\s*$[\s\S]*?^\s*workspace\s*=\s*true\s*$", re.MULTILINE
)

CENTRAL_SYSTEMS = {
    "crm-module-sdk": ("sdk-ports", "crates/crm-module-sdk/src"),
    "crm-core-contracts": ("stable-contracts", "crates/crm-core-contracts/src"),
    "crm-proto-contracts": ("stable-contracts", "crates/crm-proto-contracts/src"),
    "crm-capability-runtime": ("generic-runtime", "crates/crm-capability-runtime/src"),
    "crm-query-runtime": ("generic-runtime", "crates/crm-query-runtime/src"),
    "crm-application-composition": (
        "generic-composition",
        "crates/crm-application-composition/src",
    ),
    "crm-core-data": ("infrastructure-ports", "crates/crm-core-data/src"),
    "crm-first-party-modules": (
        "first-party-aggregation",
        "crates/crm-first-party-modules/src",
    ),
    "crm-application-runtime": (
        "process-composition",
        "crates/crm-application-runtime/src",
    ),
    "crm-api": ("process-host", "services/crm-api/src"),
}

CENTRAL_PREFIXES = (
    "crates/crm-application-composition/",
    "crates/crm-application-runtime/",
    "crates/crm-first-party-modules/",
    "services/crm-api/",
    "scripts/",
    ".github/workflows/",
)


def run(command: list[str], root: Path) -> str:
    result = subprocess.run(
        command,
        cwd=root,
        check=False,
        text=True,
        capture_output=True,
    )
    if result.returncode != 0:
        raise RuntimeError(
            f"command failed ({' '.join(command)}):\n{result.stdout}\n{result.stderr}"
        )
    return result.stdout


def load_policy(root: Path) -> dict[str, Any]:
    with (root / POLICY_PATH).open(encoding="utf-8") as handle:
        return json.load(handle)


def package_graph(
    metadata: dict[str, Any],
) -> tuple[dict[str, set[str]], dict[str, set[str]], dict[str, dict[str, Any]]]:
    workspace_ids = set(metadata.get("workspace_members", []))
    packages = {
        package["name"]: package
        for package in metadata.get("packages", [])
        if package.get("id") in workspace_ids
    }
    names = set(packages)
    dependencies: dict[str, set[str]] = {}
    dependents: dict[str, set[str]] = {name: set() for name in names}
    for name, package in packages.items():
        direct = {
            dependency["name"]
            for dependency in package.get("dependencies", [])
            if dependency.get("name") in names
        }
        dependencies[name] = direct
        for dependency in direct:
            dependents[dependency].add(name)
    return dependencies, dependents, packages


def dependency_depths(dependencies: dict[str, set[str]]) -> dict[str, int]:
    memo: dict[str, int] = {}
    visiting: set[str] = set()

    def depth(name: str) -> int:
        if name in memo:
            return memo[name]
        if name in visiting:
            return 0
        visiting.add(name)
        value = 0
        if dependencies.get(name):
            value = 1 + max(depth(dependency) for dependency in dependencies[name])
        visiting.remove(name)
        memo[name] = value
        return value

    for name in sorted(dependencies):
        depth(name)
    return memo


def reverse_closure(name: str, dependents: dict[str, set[str]]) -> set[str]:
    visited: set[str] = set()
    queue: deque[str] = deque(dependents.get(name, set()))
    while queue:
        candidate = queue.popleft()
        if candidate in visited:
            continue
        visited.add(candidate)
        queue.extend(dependents.get(candidate, set()))
    return visited


def public_rust_surface(root: Path, packages: dict[str, dict[str, Any]]) -> dict[str, Any]:
    per_package: list[dict[str, Any]] = []
    for name, package in sorted(packages.items()):
        manifest = Path(package["manifest_path"]).resolve()
        source_root = manifest.parent / "src"
        count = 0
        files = 0
        if source_root.exists():
            for path in sorted(source_root.rglob("*.rs")):
                files += 1
                for line in path.read_text(encoding="utf-8").splitlines():
                    if PUBLIC_ITEM_PATTERN.match(line):
                        count += 1
        per_package.append({"package": name, "files": files, "public_items": count})
    ranked = sorted(per_package, key=lambda item: (-item["public_items"], item["package"]))
    return {
        "total_public_items": sum(item["public_items"] for item in per_package),
        "largest_packages": ranked[:25],
        "packages": per_package,
        "measurement": (
            "Conservative source-text count of explicit public Rust items; "
            "not a semantic rustdoc compatibility model."
        ),
    }


def iter_repository_files(root: Path):
    for path in sorted(root.rglob("*")):
        if not path.is_file():
            continue
        relative = path.relative_to(root)
        if any(part in SKIPPED_DIRECTORIES for part in relative.parts):
            continue
        yield path, relative


def suppression_inventory(root: Path) -> dict[str, Any]:
    entries: list[dict[str, Any]] = []
    for path, relative in iter_repository_files(root):
        relative_text = relative.as_posix()
        if path.name == "Cargo.toml":
            text = path.read_text(encoding="utf-8")
            if DIRECT_LINT_HEADER_PATTERN.search(text) and not WORKSPACE_LINT_INHERITANCE_PATTERN.search(text):
                entries.append(
                    {
                        "kind": "direct-lint-table",
                        "path": relative_text,
                        "line": next(
                            index
                            for index, line in enumerate(text.splitlines(), start=1)
                            if DIRECT_LINT_HEADER_PATTERN.match(line)
                        ),
                        "detail": "package-local [lints] table",
                    }
                )
            continue
        if path.suffix != ".rs":
            continue
        for line_number, line in enumerate(
            path.read_text(encoding="utf-8").splitlines(), start=1
        ):
            for match in SOURCE_SUPPRESSION_PATTERN.finditer(line):
                entries.append(
                    {
                        "kind": f"rust-{match.group(1)}",
                        "path": relative_text,
                        "line": line_number,
                        "detail": " ".join(match.group(2).split()),
                    }
                )
            ignore = IGNORE_PATTERN.search(line)
            if ignore:
                entries.append(
                    {
                        "kind": "ignored-test",
                        "path": relative_text,
                        "line": line_number,
                        "detail": ignore.group(1) or "unqualified #[ignore]",
                    }
                )
    entries.sort(key=lambda item: (item["kind"], item["path"], item["line"]))
    counts: dict[str, int] = defaultdict(int)
    for entry in entries:
        counts[entry["kind"]] += 1
    return {
        "entry_count": len(entries),
        "counts_by_kind": dict(sorted(counts.items())),
        "entries": entries,
    }


def package_for_path(
    relative_path: str, packages: dict[str, dict[str, Any]], root: Path
) -> str | None:
    candidate = (root / relative_path).resolve()
    best: tuple[int, str] | None = None
    for name, package in packages.items():
        package_root = Path(package["manifest_path"]).resolve().parent
        try:
            candidate.relative_to(package_root)
        except ValueError:
            continue
        length = len(package_root.parts)
        if best is None or length > best[0]:
            best = (length, name)
    return best[1] if best else None


def changed_paths_for_commit(root: Path, commit: str) -> list[str]:
    output = run(
        [
            "git",
            "diff-tree",
            "--root",
            "--no-commit-id",
            "--name-only",
            "-r",
            commit,
        ],
        root,
    )
    return sorted({line.strip() for line in output.splitlines() if line.strip()})


def change_cost_profiles(
    root: Path, policy: dict[str, Any], packages: dict[str, dict[str, Any]]
) -> list[dict[str, Any]]:
    profiles: list[dict[str, Any]] = []
    for exemplar in policy.get("representative_changes", []):
        paths = changed_paths_for_commit(root, exemplar["commit"])
        touched_packages = sorted(
            {
                package
                for path in paths
                if (package := package_for_path(path, packages, root)) is not None
            }
        )
        central_paths = [path for path in paths if path.startswith(CENTRAL_PREFIXES)]
        workflows = [path for path in paths if path.startswith(".github/workflows/")]
        profiles.append(
            {
                "id": exemplar["id"],
                "kind": exemplar["kind"],
                "commit": exemplar["commit"],
                "rationale": exemplar["rationale"],
                "file_count": len(paths),
                "package_count": len(touched_packages),
                "packages": touched_packages,
                "central_file_count": len(central_paths),
                "central_files": central_paths,
                "workflow_file_count": len(workflows),
                "workflow_files": workflows,
                "paths": paths,
            }
        )
    return profiles


def central_risk(
    role: str, direct_dependencies: int, reverse_impact: int, loc: dict[str, int]
) -> str:
    if role in {"stable-contracts", "sdk-ports"}:
        return (
            "high fan-out is acceptable only while the boundary remains small, stable and "
            "infrastructure-neutral"
        )
    if role in {"process-composition", "process-host"}:
        return (
            "mutable process boundary; owner-specific dependency or LOC growth requires "
            "measured justification"
        )
    if role == "first-party-aggregation":
        return (
            "manual central aggregate; extension cost and duplicate inventories must remain bounded"
        )
    if direct_dependencies > 20 or reverse_impact > 50 or loc["non_comment_lines"] > 3000:
        return "high centrality; changes require focused before/after evidence"
    return "role-appropriate centrality; monitor for growth"


def central_system_metrics(
    root: Path,
    dependencies: dict[str, set[str]],
    dependents: dict[str, set[str]],
    depths: dict[str, int],
    package_metrics_by_name: dict[str, Any],
    public_by_name: dict[str, dict[str, Any]],
) -> list[dict[str, Any]]:
    systems: list[dict[str, Any]] = []
    for name, (role, source_path) in CENTRAL_SYSTEMS.items():
        metric = package_metrics_by_name.get(name)
        if metric is None:
            systems.append(
                {
                    "package": name,
                    "role": role,
                    "present": False,
                    "risk": "required central system is missing from workspace metadata",
                }
            )
            continue
        reverse = reverse_closure(name, dependents)
        loc = source_loc(root, source_path)
        systems.append(
            {
                "package": name,
                "role": role,
                "present": True,
                "direct_dependencies": sorted(dependencies.get(name, set())),
                "direct_dependency_count": len(dependencies.get(name, set())),
                "direct_consumers": sorted(dependents.get(name, set())),
                "direct_consumer_count": len(dependents.get(name, set())),
                "transitive_reverse_impact": len(reverse),
                "dependency_depth": depths.get(name, 0),
                "public_items": public_by_name.get(name, {}).get("public_items", 0),
                "source": loc,
                "risk": central_risk(role, len(dependencies.get(name, set())), len(reverse), loc),
            }
        )
    return systems


def thin_wrapper_candidates(
    root: Path,
    packages: dict[str, dict[str, Any]],
    dependents: dict[str, set[str]],
    public_by_name: dict[str, dict[str, Any]],
    policy: dict[str, Any],
) -> list[dict[str, Any]]:
    maximum_loc = int(policy["calibration"]["thin_wrapper_maximum_non_comment_loc"])
    candidates: list[dict[str, Any]] = []
    for name, package in sorted(packages.items()):
        if len(dependents.get(name, set())) != 1:
            continue
        package_root = Path(package["manifest_path"]).resolve().parent
        try:
            relative_source = (package_root / "src").relative_to(root).as_posix()
        except ValueError:
            continue
        loc = source_loc(root, relative_source)
        if loc["non_comment_lines"] > maximum_loc:
            continue
        candidates.append(
            {
                "package": name,
                "consumer": sorted(dependents[name])[0],
                "non_comment_loc": loc["non_comment_lines"],
                "public_items": public_by_name.get(name, {}).get("public_items", 0),
                "classification": "candidate-only; no consolidation is authorized by measurement",
            }
        )
    return candidates


def build_report(root: Path) -> dict[str, Any]:
    policy = load_policy(root)
    workspace = build_workspace_report(root)
    metadata = load_cargo_metadata(root)
    dependencies, dependents, packages = package_graph(metadata)
    metrics, _ = package_metrics(root, metadata)
    metrics_by_name = {metric.name: metric for metric in metrics}
    depths = dependency_depths(dependencies)
    public_surface = public_rust_surface(root, packages)
    public_by_name = {item["package"]: item for item in public_surface["packages"]}
    central_systems = central_system_metrics(
        root,
        dependencies,
        dependents,
        depths,
        metrics_by_name,
        public_by_name,
    )
    suppressions = suppression_inventory(root)
    changes = change_cost_profiles(root, policy, packages)
    thin_wrappers = thin_wrapper_candidates(
        root, packages, dependents, public_by_name, policy
    )
    return {
        "schema_version": SCHEMA_VERSION,
        "commit_sha": workspace["commit_sha"],
        "measurement_mode": "calibration-only",
        "workspace_baseline": workspace,
        "dependency_graph": {
            "maximum_depth": max(depths.values(), default=0),
            "deepest_packages": [
                {"package": name, "depth": depth}
                for name, depth in sorted(
                    depths.items(), key=lambda item: (-item[1], item[0])
                )[:25]
            ],
        },
        "public_rust_surface": public_surface,
        "central_systems": central_systems,
        "suppression_inventory": suppressions,
        "representative_change_cost": changes,
        "thin_wrapper_candidates": thin_wrappers,
        "calibration": policy["calibration"],
        "limitations": [
            "The source-level public item count is conservative and is not a semantic API model.",
            "Historical change cost is measured from accepted squash-merge commits and does not infer developer elapsed time.",
            "This first packet inventories suppressions; a later bounded step-13 packet registers the accepted baseline and turns new unregistered equivalents into blocking failures.",
            "Thin-wrapper results are candidates only and do not authorize consolidation.",
        ],
    }


def markdown_report(report: dict[str, Any]) -> str:
    workspace = report["workspace_baseline"]["workspace"]
    ci = report["workspace_baseline"]["ci"]
    lines = [
        "# Repository Step 13 Current-Main Complexity Baseline",
        "",
        f"Commit: `{report['commit_sha']}`",
        "",
        "> ADR-031 measurement and governance calibration only. This report does not authorize structural remediation.",
        "",
        "## Headline",
        "",
        "| Metric | Value |",
        "|---|---:|",
        f"| Workspace packages | {workspace['package_count']} |",
        f"| Internal dependency edges | {workspace['internal_dependency_edges']} |",
        f"| Maximum dependency depth | {report['dependency_graph']['maximum_depth']} |",
        f"| Maximum direct dependents | {workspace['maximum_direct_dependents']} |",
        f"| Maximum transitive reverse impact | {workspace['maximum_transitive_reverse_impact']} |",
        f"| Conservative public Rust items | {report['public_rust_surface']['total_public_items']} |",
        f"| Suppression/bypass entries | {report['suppression_inventory']['entry_count']} |",
        f"| Permanent workflows | {ci['workflow_count']} |",
        f"| Workflow jobs | {ci['job_count']} |",
        f"| Workflow path-filter entries | {ci['path_filter_entry_count']} |",
        f"| PostgreSQL workflows | {ci['postgres_workflow_count']} |",
        "",
        "## Central systems",
        "",
        "| Package | Role | Direct deps | Direct consumers | Reverse impact | Depth | Public items | Non-comment LOC |",
        "|---|---|---:|---:|---:|---:|---:|---:|",
    ]
    for system in report["central_systems"]:
        source = system.get("source", {})
        lines.append(
            f"| `{system['package']}` | {system['role']} | "
            f"{system.get('direct_dependency_count', 0)} | "
            f"{system.get('direct_consumer_count', 0)} | "
            f"{system.get('transitive_reverse_impact', 0)} | "
            f"{system.get('dependency_depth', 0)} | "
            f"{system.get('public_items', 0)} | "
            f"{source.get('non_comment_lines', 0)} |"
        )
    lines.extend(["", "## Suppression and bypass inventory", ""])
    if report["suppression_inventory"]["counts_by_kind"]:
        lines.extend(["| Kind | Count |", "|---|---:|"])
        for kind, count in report["suppression_inventory"]["counts_by_kind"].items():
            lines.append(f"| `{kind}` | {count} |")
    else:
        lines.append("No matching suppressions or ignored tests were detected.")
    lines.extend(["", "## Representative change cost", ""])
    lines.extend(
        [
            "| Exemplar | Kind | Files | Packages | Central files | Workflow files |",
            "|---|---|---:|---:|---:|---:|",
        ]
    )
    for change in report["representative_change_cost"]:
        lines.append(
            f"| `{change['id']}` | {change['kind']} | {change['file_count']} | "
            f"{change['package_count']} | {change['central_file_count']} | "
            f"{change['workflow_file_count']} |"
        )
    lines.extend(["", "## Candidate-only thin wrappers", ""])
    if report["thin_wrapper_candidates"]:
        lines.extend(
            [
                "| Package | Sole consumer | Non-comment LOC | Public items |",
                "|---|---|---:|---:|",
            ]
        )
        for candidate in report["thin_wrapper_candidates"]:
            lines.append(
                f"| `{candidate['package']}` | `{candidate['consumer']}` | "
                f"{candidate['non_comment_loc']} | {candidate['public_items']} |"
            )
    else:
        lines.append("No package matched the calibrated candidate-only threshold.")
    lines.extend(["", "## Calibration policy", ""])
    for key, value in sorted(report["calibration"].items()):
        lines.append(f"- `{key}`: `{value}`")
    lines.extend(["", "## Limitations", ""])
    for limitation in report["limitations"]:
        lines.append(f"- {limitation}")
    lines.append("")
    return "\n".join(lines)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path.cwd())
    parser.add_argument("--json-output", type=Path)
    parser.add_argument("--markdown-output", type=Path)
    args = parser.parse_args()
    root = args.root.resolve()
    try:
        report = build_report(root)
    except (OSError, RuntimeError, ValueError, json.JSONDecodeError) as error:
        print(f"step-13 analysis failed: {error}", file=sys.stderr)
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
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
