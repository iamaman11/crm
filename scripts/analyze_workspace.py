#!/usr/bin/env python3
"""Produce a deterministic measurement-only workspace and CI complexity report."""

from __future__ import annotations

import argparse
from collections import defaultdict, deque
from dataclasses import asdict, dataclass
import json
from pathlib import Path
import re
import subprocess
import sys
import tomllib
from typing import Any

SCHEMA_VERSION = "crm.workspace-complexity-baseline/v1"
STEP22_INVENTORY_SCHEMA = "crm.step22-architecture-inventory/v1"
APPLICATION_RUNTIME_PACKAGE = "crm-application-runtime"
WORKFLOW_SUFFIXES = {".yml", ".yaml"}
USES_PATTERN = re.compile(r"^\s*-?\s*uses:\s*([^#\s]+)")
RUN_PATTERN = re.compile(r"^\s*-?\s*run:\s*")
TIMEOUT_PATTERN = re.compile(r"^\s*timeout-minutes:\s*(\d+)\s*$")
JOB_PATTERN = re.compile(r"^  ([A-Za-z0-9_.-]+):\s*$")


@dataclass(frozen=True)
class PackageMetric:
    name: str
    category: str
    manifest_path: str
    direct_internal_dependencies: int
    direct_internal_dependents: int
    transitive_reverse_impact: int


@dataclass(frozen=True)
class RuntimeDependencyMetric:
    stable_id: str
    dependency_name: str
    dependency_kind: str
    manifest_section: str
    target_category: str
    target_manifest_path: str
    decision_state: str


@dataclass(frozen=True)
class WorkflowJobMetric:
    stable_id: str
    workflow_name: str
    workflow_path: str
    job_id: str
    action_reference_count: int
    run_step_count: int
    maximum_timeout_minutes: int | None
    environment_signals: tuple[str, ...]
    decision_state: str


@dataclass(frozen=True)
class WorkflowMetric:
    stable_id: str
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
    environment_signals: tuple[str, ...]
    decision_state: str
    jobs: tuple[WorkflowJobMetric, ...]


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


def workspace_packages(metadata: dict[str, Any]) -> list[dict[str, Any]]:
    workspace_ids = set(metadata.get("workspace_members", []))
    return [
        package
        for package in metadata.get("packages", [])
        if package.get("id") in workspace_ids
    ]


def package_metrics(
    root: Path, metadata: dict[str, Any]
) -> tuple[list[PackageMetric], dict[str, int]]:
    packages = workspace_packages(metadata)
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
            manifest_path=str(
                Path(package["manifest_path"])
                .resolve()
                .relative_to(root.resolve())
            ),
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


def dependency_kind(kind: str | None) -> tuple[str, str]:
    if kind == "dev":
        return "test-only", "dev-dependencies"
    if kind == "build":
        return "build", "build-dependencies"
    return "production", "dependencies"


def runtime_dependency_metrics(
    root: Path,
    metadata: dict[str, Any],
    package_name: str = APPLICATION_RUNTIME_PACKAGE,
) -> list[RuntimeDependencyMetric]:
    packages = workspace_packages(metadata)
    by_name = {package["name"]: package for package in packages}
    runtime = by_name.get(package_name)
    if runtime is None:
        raise ValueError(f"workspace package not found: {package_name}")

    metrics: list[RuntimeDependencyMetric] = []
    for dependency in runtime.get("dependencies", []):
        target = by_name.get(dependency.get("name"))
        if target is None:
            continue
        kind, section = dependency_kind(dependency.get("kind"))
        relative_manifest = str(
            Path(target["manifest_path"]).resolve().relative_to(root.resolve())
        )
        metrics.append(
            RuntimeDependencyMetric(
                stable_id=f"{package_name}::{section}::{target['name']}",
                dependency_name=target["name"],
                dependency_kind=kind,
                manifest_section=section,
                target_category=categorize_manifest(root, target["manifest_path"]),
                target_manifest_path=relative_manifest,
                decision_state="unresolved",
            )
        )

    section_order = {
        "dependencies": 0,
        "dev-dependencies": 1,
        "build-dependencies": 2,
    }
    return sorted(
        metrics,
        key=lambda metric: (
            section_order[metric.manifest_section],
            metric.dependency_name,
        ),
    )


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


def environment_signals(lines: list[str]) -> tuple[str, ...]:
    text = "\n".join(lines).lower()
    signals: list[str] = []
    if re.search(r"^\s+postgres:\s*$", "\n".join(lines), flags=re.MULTILINE):
        signals.append("postgres-service")
    if "playwright" in text or "chromium" in text or "browser" in text:
        signals.append("browser")
    if "docker" in text:
        signals.append("docker")
    if "crm-api" in text or "cargo run" in text or "spawn" in text:
        signals.append("process-runtime")
    return tuple(signals)


def workflow_job_metrics(
    lines: list[str], workflow_name: str, workflow_path: str
) -> tuple[WorkflowJobMetric, ...]:
    jobs = top_level_block(lines, "jobs")
    if jobs is None:
        return ()

    starts: list[tuple[int, str]] = []
    for index in range(jobs[0] + 1, jobs[1]):
        match = JOB_PATTERN.match(lines[index])
        if match:
            starts.append((index, match.group(1)))

    metrics: list[WorkflowJobMetric] = []
    for position, (start, job_id) in enumerate(starts):
        end = starts[position + 1][0] if position + 1 < len(starts) else jobs[1]
        block = lines[start:end]
        timeouts = [
            int(match.group(1))
            for line in block
            if (match := TIMEOUT_PATTERN.match(line))
        ]
        metrics.append(
            WorkflowJobMetric(
                stable_id=f"{workflow_path}#{job_id}",
                workflow_name=workflow_name,
                workflow_path=workflow_path,
                job_id=job_id,
                action_reference_count=sum(
                    1 for line in block if USES_PATTERN.match(line)
                ),
                run_step_count=sum(1 for line in block if RUN_PATTERN.match(line)),
                maximum_timeout_minutes=max(timeouts) if timeouts else None,
                environment_signals=environment_signals(block),
                decision_state="unresolved",
            )
        )
    return tuple(metrics)


def workflow_metric(path: Path, root: Path) -> WorkflowMetric:
    lines = path.read_text(encoding="utf-8").splitlines()
    name = path.stem
    for line in lines:
        if line.startswith("name:"):
            name = line.split(":", 1)[1].strip()
            break

    relative_path = str(path.relative_to(root))
    jobs = workflow_job_metrics(lines, name, relative_path)

    event_block = top_level_block(lines, "on")
    event_lines = lines[event_block[0] + 1 : event_block[1]] if event_block else []
    handles_pull_requests = any(
        line.rstrip() == "  pull_request:" for line in event_lines
    )
    has_push = any(line.rstrip() == "  push:" for line in event_lines)
    pushes_main_only = False
    if has_push:
        push_index = next(
            index
            for index, line in enumerate(event_lines)
            if line.rstrip() == "  push:"
        )
        push_end = push_index + 1
        while push_end < len(event_lines):
            line = event_lines[push_end]
            if line.strip() and not line.startswith(("    ", "\t", "#")):
                break
            push_end += 1
        push_section = event_lines[push_index:push_end]
        pushes_main_only = (
            "    branches:" in push_section and "      - main" in push_section
        )

    path_filter_count = 0
    inside_paths = False
    for line in event_lines:
        if line.rstrip() == "    paths:":
            inside_paths = True
            continue
        if inside_paths and re.match(r"^\s{6}-\s+", line):
            path_filter_count += 1
            continue
        if inside_paths and line.strip() and not line.startswith(("      ", "\t", "#")):
            inside_paths = False

    timeouts = [
        int(match.group(1))
        for line in lines
        if (match := TIMEOUT_PATTERN.match(line))
    ]
    signals = environment_signals(lines)

    return WorkflowMetric(
        stable_id=relative_path,
        name=name,
        path=relative_path,
        job_count=len(jobs),
        action_reference_count=sum(1 for line in lines if USES_PATTERN.match(line)),
        run_step_count=sum(1 for line in lines if RUN_PATTERN.match(line)),
        path_filter_count=path_filter_count,
        maximum_timeout_minutes=max(timeouts) if timeouts else None,
        has_postgres_service="postgres-service" in signals,
        has_concurrency=top_level_block(lines, "concurrency") is not None,
        pushes_main_only=pushes_main_only,
        handles_pull_requests=handles_pull_requests,
        environment_signals=signals,
        decision_state="unresolved",
        jobs=jobs,
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


def build_step22_inventory(
    commit_sha: str,
    runtime_dependencies: list[RuntimeDependencyMetric],
    workflows: list[WorkflowMetric],
) -> dict[str, Any]:
    jobs = [job for workflow in workflows for job in workflow.jobs]
    dependency_ids = [metric.stable_id for metric in runtime_dependencies]
    workflow_ids = [metric.stable_id for metric in workflows]
    job_ids = [metric.stable_id for metric in jobs]
    if len(dependency_ids) != len(set(dependency_ids)):
        raise ValueError("duplicate crm-application-runtime dependency inventory IDs")
    if len(workflow_ids) != len(set(workflow_ids)):
        raise ValueError("duplicate permanent workflow inventory IDs")
    if len(job_ids) != len(set(job_ids)):
        raise ValueError("duplicate permanent workflow job inventory IDs")

    dependency_counts: dict[str, int] = defaultdict(int)
    for metric in runtime_dependencies:
        dependency_counts[metric.dependency_kind] += 1

    return {
        "schema_version": STEP22_INVENTORY_SCHEMA,
        "commit_sha": commit_sha,
        "phase": "inventory-only",
        "adr": "docs/adr/ADR-032-step-22-runtime-fanin-and-permanent-gate-value.md",
        "runtime_fanin": {
            "package": APPLICATION_RUNTIME_PACKAGE,
            "manifest_path": "crates/crm-application-runtime/Cargo.toml",
            "internal_direct_dependency_count": len(runtime_dependencies),
            "production_count": dependency_counts.get("production", 0),
            "test_only_count": dependency_counts.get("test-only", 0),
            "build_count": dependency_counts.get("build", 0),
            "unresolved_decision_count": len(runtime_dependencies),
            "dependencies": [asdict(metric) for metric in runtime_dependencies],
        },
        "permanent_gates": {
            "workflow_count": len(workflows),
            "job_count": len(jobs),
            "unresolved_workflow_decision_count": len(workflows),
            "unresolved_job_decision_count": len(jobs),
            "workflows": [asdict(metric) for metric in workflows],
            "jobs": [asdict(metric) for metric in jobs],
        },
        "decision_boundary": {
            "final_classifications_recorded": False,
            "gate_dispositions_recorded": False,
            "remediation_performed": False,
            "step22_complete": False,
        },
    }


def build_report(root: Path) -> dict[str, Any]:
    metadata = load_cargo_metadata(root)
    packages, categories = package_metrics(root, metadata)
    workspace_names = {metric.name for metric in packages}
    duplicate_families = duplicate_dependency_families(
        load_lockfile(root), workspace_names
    )
    workflows = workflow_metrics(root)
    runtime_dependencies = runtime_dependency_metrics(root, metadata)
    commit_sha = current_commit(root)
    top_reverse_impact = sorted(
        packages,
        key=lambda metric: (-metric.transitive_reverse_impact, metric.name),
    )[:20]
    one_consumer = [
        metric for metric in packages if metric.direct_internal_dependents == 1
    ]

    return {
        "schema_version": SCHEMA_VERSION,
        "commit_sha": commit_sha,
        "measurement_mode": "non-blocking",
        "workspace": {
            "package_count": len(packages),
            "categories": categories,
            "internal_dependency_edges": sum(
                metric.direct_internal_dependencies for metric in packages
            ),
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
            "action_reference_count": sum(
                metric.action_reference_count for metric in workflows
            ),
            "run_step_count": sum(metric.run_step_count for metric in workflows),
            "path_filter_entry_count": sum(
                metric.path_filter_count for metric in workflows
            ),
            "postgres_workflow_count": sum(
                metric.has_postgres_service for metric in workflows
            ),
            "pull_request_workflow_count": sum(
                metric.handles_pull_requests for metric in workflows
            ),
            "concurrency_workflow_count": sum(
                metric.has_concurrency for metric in workflows
            ),
            "main_only_push_workflow_count": sum(
                metric.pushes_main_only for metric in workflows
            ),
            "workflows": [asdict(metric) for metric in workflows],
        },
        "composition": {
            "application_composition": source_loc(
                root, "crates/crm-application-composition/src"
            ),
            "application_runtime": source_loc(
                root, "crates/crm-application-runtime/src"
            ),
        },
        "step22_inventory": build_step22_inventory(
            commit_sha, runtime_dependencies, workflows
        ),
        "limitations": [
            "Build and test durations require repeated runtime telemetry and are not inferred from timeout values.",
            "Current values are measurement-only and do not establish blocking budgets.",
            "Reverse impact is computed from declared direct workspace dependencies in cargo metadata.",
            "Step 22 dependency classifications and gate dispositions remain unresolved in this inventory-only slice.",
            "Environment signals are deterministic source-text indicators, not measured runner duration or cost.",
        ],
    }


def markdown_report(report: dict[str, Any]) -> str:
    workspace = report["workspace"]
    dependencies = report["dependencies"]
    ci = report["ci"]
    composition = report["composition"]
    step22 = report["step22_inventory"]
    runtime_fanin = step22["runtime_fanin"]
    gates = step22["permanent_gates"]
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
        f"| Application runtime internal direct dependencies | {runtime_fanin['internal_direct_dependency_count']} |",
        f"| Unresolved Step 22 runtime decisions | {runtime_fanin['unresolved_decision_count']} |",
        f"| Unresolved Step 22 workflow decisions | {gates['unresolved_workflow_decision_count']} |",
        f"| Unresolved Step 22 job decisions | {gates['unresolved_job_decision_count']} |",
        "",
        "## Step 22 inventory-only checkpoint",
        "",
        "This additive inventory does **not** classify dependencies, assign gate dispositions, perform remediation, complete Repository Step 22 or raise an architecture score.",
        "",
        "### `crm-application-runtime` internal direct dependencies",
        "",
        "| Stable ID | Dependency | Kind | Target category | Decision |",
        "|---|---|---|---|---|",
    ]
    for metric in runtime_fanin["dependencies"]:
        lines.append(
            f"| `{metric['stable_id']}` | `{metric['dependency_name']}` | "
            f"{metric['dependency_kind']} | {metric['target_category']} | "
            f"{metric['decision_state']} |"
        )

    lines.extend(
        [
            "",
            "### Permanent workflow jobs",
            "",
            "| Stable ID | Workflow | Job | Run steps | Actions | Timeout | Environment signals | Decision |",
            "|---|---|---|---:|---:|---:|---|---|",
        ]
    )
    for job in gates["jobs"]:
        timeout = job["maximum_timeout_minutes"] or 0
        signals = ", ".join(job["environment_signals"]) or "none"
        lines.append(
            f"| `{job['stable_id']}` | {job['workflow_name']} | `{job['job_id']}` | "
            f"{job['run_step_count']} | {job['action_reference_count']} | "
            f"{timeout} | {signals} | {job['decision_state']} |"
        )

    lines.extend(
        [
            "",
            "## Highest reverse-impact packages",
            "",
            "| Package | Category | Direct dependents | Transitive reverse impact |",
            "|---|---|---:|---:|",
        ]
    )
    for metric in workspace["top_reverse_impact"][:15]:
        lines.append(
            f"| `{metric['name']}` | {metric['category']} | "
            f"{metric['direct_internal_dependents']} | "
            f"{metric['transitive_reverse_impact']} |"
        )

    lines.extend(["", "## Duplicate dependency families", ""])
    if dependencies["duplicate_families"]:
        lines.extend(["| Dependency | Versions |", "|---|---|"])
        for family in dependencies["duplicate_families"]:
            lines.append(
                f"| `{family['name']}` | {', '.join(family['versions'])} |"
            )
    else:
        lines.append("No duplicate external dependency families were detected.")

    lines.extend(
        [
            "",
            "## Workflow summary",
            "",
            "| Workflow | Jobs | Paths | Timeout | PostgreSQL | Signals |",
            "|---|---:|---:|---:|:---:|---|",
        ]
    )
    for workflow in ci["workflows"]:
        timeout = workflow["maximum_timeout_minutes"] or 0
        postgres = "yes" if workflow["has_postgres_service"] else "no"
        signals = ", ".join(workflow["environment_signals"]) or "none"
        lines.append(
            f"| {workflow['name']} | {workflow['job_count']} | "
            f"{workflow['path_filter_count']} | {timeout} | {postgres} | "
            f"{signals} |"
        )

    lines.extend(["", "## Limitations", ""])
    for limitation in report["limitations"]:
        lines.append(f"- {limitation}")
    lines.append("")
    return "\n".join(lines)


def canonical_json(value: Any) -> str:
    return json.dumps(value, indent=2, sort_keys=True) + "\n"


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path.cwd())
    parser.add_argument("--json-output", type=Path)
    parser.add_argument("--markdown-output", type=Path)
    parser.add_argument("--step22-inventory-output", type=Path)
    args = parser.parse_args()

    root = args.root.resolve()
    try:
        report = build_report(root)
    except (RuntimeError, OSError, ValueError, tomllib.TOMLDecodeError) as error:
        print(f"workspace analysis failed: {error}", file=sys.stderr)
        return 1

    json_text = canonical_json(report)
    markdown_text = markdown_report(report)
    if args.json_output:
        args.json_output.parent.mkdir(parents=True, exist_ok=True)
        args.json_output.write_text(json_text, encoding="utf-8")
    else:
        print(json_text, end="")
    if args.markdown_output:
        args.markdown_output.parent.mkdir(parents=True, exist_ok=True)
        args.markdown_output.write_text(markdown_text, encoding="utf-8")
    if args.step22_inventory_output:
        args.step22_inventory_output.parent.mkdir(parents=True, exist_ok=True)
        args.step22_inventory_output.write_text(
            canonical_json(report["step22_inventory"]), encoding="utf-8"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
