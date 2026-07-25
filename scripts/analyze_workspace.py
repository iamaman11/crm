#!/usr/bin/env python3
"""Produce a deterministic measurement-only workspace and CI complexity report."""

from __future__ import annotations

import argparse
from collections import defaultdict, deque
from dataclasses import dataclass, asdict
import json
from pathlib import Path
import re
import subprocess
import sys
import tomllib
from typing import Any

SCHEMA_VERSION = "crm.workspace-complexity-baseline/v1"
WORKFLOW_SUFFIXES = {".yml", ".yaml"}
USES_PATTERN = re.compile(r"^\s*-?\s*uses:\s*([^#\s]+)")
TIMEOUT_PATTERN = re.compile(r"^\s*timeout-minutes:\s*(\d+)\s*$")


@dataclass(frozen=True)
class PackageMetric:
    name: str
    category: str
    manifest_path: str
    direct_internal_dependencies: int
    direct_internal_dependents: int
    transitive_reverse_impact: int


@dataclass(frozen=True)
class WorkflowMetric:
    name: str
    path: str
    job_count: int
    action_reference_count: int
    run_step_count: int
    path_filter_count: int
    maximum_timeout_minutes: int | None
    has_postgres_service: bool
    has_concurrency: bool
    pushes_main_only: bool
    handles_pull_requests: bool


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


def load_cargo_metadata(root: Path) -> dict[str, Any]:
    return json.loads(
        run(
            ["cargo", "metadata", "--format-version", "1", "--locked", "--no-deps"],
            root,
        )
    )


def current_commit(root: Path) -> str:
    try:
        return run(["git", "rev-parse", "HEAD"], root).strip()
    except RuntimeError:
        return "unknown"


def categorize_manifest(root: Path, manifest_path: str) -> str:
    relative = Path(manifest_path).resolve().relative_to(root.resolve())
    parts = relative.parts
    if parts and parts[0] == "modules":
        return "business-module"
    if parts and parts[0] == "crates":
        return "technical-crate"
    if parts and parts[0] in {"apps", "applications"}:
        return "application"
    if parts and parts[0] in {"tools", "scripts"}:
        return "tooling"
    return "other"


def package_metrics(root: Path, metadata: dict[str, Any]) -> tuple[list[PackageMetric], dict[str, int]]:
    workspace_ids = set(metadata.get("workspace_members", []))
    packages = [package for package in metadata.get("packages", []) if package.get("id") in workspace_ids]
    by_name = {package["name"]: package for package in packages}
    internal_names = set(by_name)
    dependencies: dict[str, set[str]] = {}
    dependents: dict[str, set[str]] = {name: set() for name in internal_names}

    for name, package in by_name.items():
        direct = {
            dependency["name"]
            for dependency in package.get("dependencies", [])
            if dependency.get("name") in internal_names
        }
        dependencies[name] = direct
        for dependency in direct:
            dependents[dependency].add(name)

    def reverse_impact(name: str) -> int:
        visited: set[str] = set()
        queue: deque[str] = deque(dependents[name])
        while queue:
            dependent = queue.popleft()
            if dependent in visited:
                continue
            visited.add(dependent)
            queue.extend(dependents[dependent])
        return len(visited)

    metrics = [
        PackageMetric(
            name=name,
            category=categorize_manifest(root, package["manifest_path"]),
            manifest_path=str(Path(package["manifest_path"]).resolve().relative_to(root.resolve())),
            direct_internal_dependencies=len(dependencies[name]),
            direct_internal_dependents=len(dependents[name]),
            transitive_reverse_impact=reverse_impact(name),
        )
        for name, package in sorted(by_name.items())
    ]

    categories: dict[str, int] = defaultdict(int)
    for metric in metrics:
        categories[metric.category] += 1
    return metrics, dict(sorted(categories.items()))


def load_lockfile(root: Path) -> dict[str, Any]:
    with (root / "Cargo.lock").open("rb") as handle:
        return tomllib.load(handle)


def duplicate_dependency_families(
    lockfile: dict[str, Any], workspace_package_names: set[str]
) -> list[dict[str, Any]]:
    versions: dict[str, set[str]] = defaultdict(set)
    for package in lockfile.get("package", []):
        name = package.get("name")
        version = package.get("version")
        if not name or not version or name in workspace_package_names:
            continue
        versions[name].add(version)
    return [
        {"name": name, "versions": sorted(package_versions)}
        for name, package_versions in sorted(versions.items())
        if len(package_versions) > 1
    ]


def top_level_block(lines: list[str], key: str) -> tuple[int, int] | None:
    marker = f"{key}:"
    for index, line in enumerate(lines):
        if line.rstrip() == marker and not line.startswith((" ", "\t")):
            end = index + 1
            while end < len(lines):
                candidate = lines[end]
                if candidate.strip() and not candidate.startswith((" ", "\t", "#")):
                    break
                end += 1
            return index, end
    return None


def workflow_metric(path: Path, root: Path) -> WorkflowMetric:
    lines = path.read_text(encoding="utf-8").splitlines()
    name = path.stem
    for line in lines:
        if line.startswith("name:"):
            name = line.split(":", 1)[1].strip()
            break

    jobs = top_level_block(lines, "jobs")
    job_count = 0
    if jobs is not None:
        for line in lines[jobs[0] + 1 : jobs[1]]:
            if re.match(r"^  [A-Za-z0-9_.-]+:\s*$", line):
                job_count += 1

    event_block = top_level_block(lines, "on")
    event_lines = lines[event_block[0] + 1 : event_block[1]] if event_block else []
    handles_pull_requests = any(line.rstrip() == "  pull_request:" for line in event_lines)
    has_push = any(line.rstrip() == "  push:" for line in event_lines)
    pushes_main_only = False
    if has_push:
        push_index = next(index for index, line in enumerate(event_lines) if line.rstrip() == "  push:")
        push_end = push_index + 1
        while push_end < len(event_lines):
            line = event_lines[push_end]
            if line.strip() and not line.startswith(("    ", "\t", "#")):
                break
            push_end += 1
        push_section = event_lines[push_index:push_end]
        pushes_main_only = "    branches:" in push_section and "      - main" in push_section

    path_filter_count = sum(
        1
        for line in event_lines
        if re.match(r"^\s{6}-\s+", line)
    )
    timeouts = [int(match.group(1)) for line in lines if (match := TIMEOUT_PATTERN.match(line))]

    return WorkflowMetric(
        name=name,
        path=str(path.relative_to(root)),
        job_count=job_count,
        action_reference_count=sum(1 for line in lines if USES_PATTERN.match(line)),
        run_step_count=sum(1 for line in lines if re.match(r"^\s*run:\s*", line)),
        path_filter_count=path_filter_count,
        maximum_timeout_minutes=max(timeouts) if timeouts else None,
        has_postgres_service=any("postgres:" in line or "postgres:" in line.lower() for line in lines),
        has_concurrency=top_level_block(lines, "concurrency") is not None,
        pushes_main_only=pushes_main_only,
        handles_pull_requests=handles_pull_requests,
    )


def workflow_metrics(root: Path) -> list[WorkflowMetric]:
    workflow_dir = root / ".github" / "workflows"
    return [
        workflow_metric(path, root)
        for path in sorted(workflow_dir.iterdir())
        if path.is_file()
        and path.suffix in WORKFLOW_SUFFIXES
        and not path.name.startswith("one-time-")
    ]


def source_loc(root: Path, relative: str) -> dict[str, int]:
    directory = root / relative
    files = sorted(directory.rglob("*.rs")) if directory.exists() else []
    physical = 0
    non_blank = 0
    non_comment = 0
    for path in files:
        for line in path.read_text(encoding="utf-8").splitlines():
            physical += 1
            stripped = line.strip()
            if stripped:
                non_blank += 1
            if stripped and not stripped.startswith("//"):
                non_comment += 1
    return {
        "files": len(files),
        "physical_lines": physical,
        "non_blank_lines": non_blank,
        "non_comment_lines": non_comment,
    }


def build_report(root: Path) -> dict[str, Any]:
    metadata = load_cargo_metadata(root)
    packages, categories = package_metrics(root, metadata)
    workspace_names = {metric.name for metric in packages}
    duplicate_families = duplicate_dependency_families(load_lockfile(root), workspace_names)
    workflows = workflow_metrics(root)
    top_reverse_impact = sorted(
        packages,
        key=lambda metric: (-metric.transitive_reverse_impact, metric.name),
    )[:20]
    one_consumer = [metric for metric in packages if metric.direct_internal_dependents == 1]

    return {
        "schema_version": SCHEMA_VERSION,
        "commit_sha": current_commit(root),
        "measurement_mode": "non-blocking",
        "workspace": {
            "package_count": len(packages),
            "categories": categories,
            "internal_dependency_edges": sum(metric.direct_internal_dependencies for metric in packages),
            "maximum_direct_dependents": max(
                (metric.direct_internal_dependents for metric in packages), default=0
            ),
            "maximum_transitive_reverse_impact": max(
                (metric.transitive_reverse_impact for metric in packages), default=0
            ),
            "one_consumer_package_count": len(one_consumer),
            "top_reverse_impact": [asdict(metric) for metric in top_reverse_impact],
            "packages": [asdict(metric) for metric in packages],
        },
        "dependencies": {
            "duplicate_family_count": len(duplicate_families),
            "duplicate_families": duplicate_families,
        },
        "ci": {
            "workflow_count": len(workflows),
            "job_count": sum(metric.job_count for metric in workflows),
            "action_reference_count": sum(metric.action_reference_count for metric in workflows),
            "run_step_count": sum(metric.run_step_count for metric in workflows),
            "path_filter_entry_count": sum(metric.path_filter_count for metric in workflows),
            "postgres_workflow_count": sum(metric.has_postgres_service for metric in workflows),
            "pull_request_workflow_count": sum(metric.handles_pull_requests for metric in workflows),
            "concurrency_workflow_count": sum(metric.has_concurrency for metric in workflows),
            "main_only_push_workflow_count": sum(metric.pushes_main_only for metric in workflows),
            "workflows": [asdict(metric) for metric in workflows],
        },
        "composition": {
            "application_composition": source_loc(root, "crates/crm-application-composition/src"),
            "application_runtime": source_loc(root, "crates/crm-application-runtime/src"),
        },
        "limitations": [
            "Build and test durations require repeated runtime telemetry and are not inferred from timeout values.",
            "Current values are measurement-only and do not establish blocking budgets.",
            "Reverse impact is computed from declared direct workspace dependencies in cargo metadata.",
        ],
    }


def markdown_report(report: dict[str, Any]) -> str:
    workspace = report["workspace"]
    dependencies = report["dependencies"]
    ci = report["ci"]
    composition = report["composition"]
    categories = workspace["categories"]
    lines = [
        "# Workspace and CI Complexity Baseline",
        "",
        f"Commit: `{report['commit_sha']}`",
        "",
        "> Measurement-only baseline. No thresholds in this report are blocking.",
        "",
        "## Headline metrics",
        "",
        "| Metric | Value |",
        "|---|---:|",
        f"| Workspace packages | {workspace['package_count']} |",
        f"| Technical crates | {categories.get('technical-crate', 0)} |",
        f"| Business modules | {categories.get('business-module', 0)} |",
        f"| Internal dependency edges | {workspace['internal_dependency_edges']} |",
        f"| Maximum direct dependents | {workspace['maximum_direct_dependents']} |",
        f"| Maximum transitive reverse impact | {workspace['maximum_transitive_reverse_impact']} |",
        f"| One-consumer packages | {workspace['one_consumer_package_count']} |",
        f"| Duplicate dependency families | {dependencies['duplicate_family_count']} |",
        f"| Permanent workflows | {ci['workflow_count']} |",
        f"| Workflow jobs | {ci['job_count']} |",
        f"| Workflow path-filter entries | {ci['path_filter_entry_count']} |",
        f"| Workflows using PostgreSQL services | {ci['postgres_workflow_count']} |",
        f"| Pull-request workflows | {ci['pull_request_workflow_count']} |",
        f"| Workflows with concurrency control | {ci['concurrency_workflow_count']} |",
        f"| Main-only push workflows | {ci['main_only_push_workflow_count']} |",
        f"| Application composition non-comment LOC | {composition['application_composition']['non_comment_lines']} |",
        f"| Application runtime non-comment LOC | {composition['application_runtime']['non_comment_lines']} |",
        "",
        "## Highest reverse-impact packages",
        "",
        "| Package | Category | Direct dependents | Transitive reverse impact |",
        "|---|---|---:|---:|",
    ]
    for metric in workspace["top_reverse_impact"][:15]:
        lines.append(
            f"| `{metric['name']}` | {metric['category']} | "
            f"{metric['direct_internal_dependents']} | {metric['transitive_reverse_impact']} |"
        )

    lines.extend(["", "## Duplicate dependency families", ""])
    if dependencies["duplicate_families"]:
        lines.extend(["| Dependency | Versions |", "|---|---|"])
        for family in dependencies["duplicate_families"]:
            lines.append(f"| `{family['name']}` | {', '.join(family['versions'])} |")
    else:
        lines.append("No duplicate external dependency families were detected.")

    lines.extend(["", "## Workflow summary", "", "| Workflow | Jobs | Paths | Timeout | PostgreSQL |", "|---|---:|---:|---:|:---:|"])
    for workflow in ci["workflows"]:
        timeout = workflow["maximum_timeout_minutes"] or 0
        postgres = "yes" if workflow["has_postgres_service"] else "no"
        lines.append(
            f"| {workflow['name']} | {workflow['job_count']} | "
            f"{workflow['path_filter_count']} | {timeout} | {postgres} |"
        )

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
    except (RuntimeError, OSError, ValueError, tomllib.TOMLDecodeError) as error:
        print(f"workspace analysis failed: {error}", file=sys.stderr)
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
