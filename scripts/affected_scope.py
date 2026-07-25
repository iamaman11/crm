#!/usr/bin/env python3
"""Explain and execute affected-scope repository checks without weakening final acceptance."""

from __future__ import annotations

import argparse
from collections import defaultdict, deque
from dataclasses import asdict, dataclass
import fnmatch
import json
from pathlib import Path, PurePosixPath
import subprocess
import sys
from typing import Any

from ruamel.yaml import YAML

SCHEMA_VERSION = "crm.affected-scope/v1"
WORKFLOW_SUFFIXES = {".yml", ".yaml"}
BROAD_PATH_PATTERNS = (
    "Cargo.toml",
    "Cargo.lock",
    ".github/workflows/**",
    "architecture-policy.json",
    "requirements-dev.txt",
    "contracts/**",
    "proto/**",
    "schemas/**",
    "scripts/affected_scope.py",
    "scripts/repo.py",
    "scripts/check_architecture.py",
    "scripts/check_ci_event_policy.py",
    "scripts/analyze_workspace.py",
)
KNOWN_NON_PACKAGE_PREFIXES = (
    ".github/",
    "contracts/",
    "database/",
    "docs/",
    "proto/",
    "schemas/",
    "scripts/",
    "tests/",
)


@dataclass(frozen=True)
class PackageInfo:
    name: str
    root: str
    manifest_path: str
    dependencies: tuple[str, ...]


@dataclass(frozen=True)
class WorkflowDecision:
    name: str
    path: str
    selected: bool
    reasons: tuple[str, ...]


def run_capture(command: list[str], root: Path) -> str:
    completed = subprocess.run(
        command,
        cwd=root,
        check=False,
        text=True,
        capture_output=True,
    )
    if completed.returncode != 0:
        raise RuntimeError(
            f"command failed ({' '.join(command)}):\n{completed.stdout}\n{completed.stderr}"
        )
    return completed.stdout


def load_cargo_metadata(root: Path) -> dict[str, Any]:
    return json.loads(
        run_capture(
            ["cargo", "metadata", "--format-version", "1", "--locked", "--no-deps"],
            root,
        )
    )


def changed_paths(root: Path, base_ref: str) -> list[str]:
    output = run_capture(
        ["git", "diff", "--name-only", "--diff-filter=ACMRD", f"{base_ref}...HEAD"],
        root,
    )
    return sorted(
        {
            line.strip().replace("\\", "/")
            for line in output.splitlines()
            if line.strip()
        }
    )


def current_commit(root: Path) -> str:
    return run_capture(["git", "rev-parse", "HEAD"], root).strip()


def package_inventory(root: Path, metadata: dict[str, Any]) -> dict[str, PackageInfo]:
    root_resolved = root.resolve()
    workspace_ids = set(metadata.get("workspace_members", []))
    packages = [
        package
        for package in metadata.get("packages", [])
        if package.get("id") in workspace_ids
    ]
    workspace_names = {package["name"] for package in packages}
    result: dict[str, PackageInfo] = {}
    for package in packages:
        manifest = Path(package["manifest_path"]).resolve().relative_to(root_resolved)
        package_root = manifest.parent.as_posix()
        dependencies = tuple(
            sorted(
                {
                    dependency["name"]
                    for dependency in package.get("dependencies", [])
                    if dependency.get("name") in workspace_names
                }
            )
        )
        result[package["name"]] = PackageInfo(
            name=package["name"],
            root=package_root,
            manifest_path=manifest.as_posix(),
            dependencies=dependencies,
        )
    return dict(sorted(result.items()))


def direct_packages_for_paths(
    paths: list[str], packages: dict[str, PackageInfo]
) -> tuple[set[str], dict[str, list[str]], list[str]]:
    package_roots = sorted(
        ((info.root.rstrip("/"), name) for name, info in packages.items()),
        key=lambda item: len(item[0]),
        reverse=True,
    )
    direct: set[str] = set()
    reasons: dict[str, list[str]] = defaultdict(list)
    unowned: list[str] = []
    for path in paths:
        owner: str | None = None
        for package_root, name in package_roots:
            if path == package_root or path.startswith(f"{package_root}/"):
                owner = name
                break
        if owner is None:
            unowned.append(path)
            continue
        direct.add(owner)
        reasons[owner].append(f"directly owns changed path {path}")
    return direct, {name: sorted(values) for name, values in reasons.items()}, unowned


def reverse_dependency_closure(
    direct: set[str], packages: dict[str, PackageInfo]
) -> tuple[set[str], dict[str, list[str]]]:
    dependents: dict[str, set[str]] = {name: set() for name in packages}
    for package in packages.values():
        for dependency in package.dependencies:
            dependents[dependency].add(package.name)

    affected = set(direct)
    reasons: dict[str, list[str]] = defaultdict(list)
    queue: deque[str] = deque(sorted(direct))
    while queue:
        package = queue.popleft()
        for dependent in sorted(dependents[package]):
            reasons[dependent].append(f"reverse-depends on affected package {package}")
            if dependent not in affected:
                affected.add(dependent)
                queue.append(dependent)
    return affected, {
        name: sorted(set(values)) for name, values in reasons.items()
    }


def path_matches(path: str, pattern: str) -> bool:
    normalized = path.replace("\\", "/")
    pattern = pattern.replace("\\", "/")
    return fnmatch.fnmatchcase(normalized, pattern) or PurePosixPath(normalized).match(
        pattern
    )


def broadening_reasons(paths: list[str], unowned: list[str]) -> list[str]:
    reasons: list[str] = []
    for path in paths:
        if any(path_matches(path, pattern) for pattern in BROAD_PATH_PATTERNS):
            reasons.append(
                f"{path} changes the workspace, check graph, contracts, or shared policy"
            )
    for path in unowned:
        known = path in {"AGENTS.md", "architecture-policy.json", "requirements-dev.txt"}
        known = known or path.startswith(KNOWN_NON_PACKAGE_PREFIXES)
        if not known:
            reasons.append(f"{path} has no known package or governed non-package owner")
    return sorted(set(reasons))


def _workflow_events(document: dict[str, Any]) -> Any:
    if "on" in document:
        return document["on"]
    # Defensive compatibility for parsers that apply YAML 1.1 booleans.
    return document.get(True)


def workflow_decisions(
    root: Path, paths: list[str], broadened: bool
) -> list[WorkflowDecision]:
    yaml = YAML(typ="safe")
    decisions: list[WorkflowDecision] = []
    workflow_dir = root / ".github" / "workflows"
    for workflow_path in sorted(workflow_dir.iterdir()):
        if (
            not workflow_path.is_file()
            or workflow_path.suffix not in WORKFLOW_SUFFIXES
            or workflow_path.name.startswith("one-time-")
        ):
            continue
        document = yaml.load(workflow_path.read_text(encoding="utf-8")) or {}
        name = str(document.get("name") or workflow_path.stem)
        events = _workflow_events(document)
        if isinstance(events, str):
            events = {events: None}
        elif isinstance(events, list):
            events = {str(event): None for event in events}
        if not isinstance(events, dict) or "pull_request" not in events:
            continue

        pull_request = events["pull_request"]
        if broadened:
            decisions.append(
                WorkflowDecision(
                    name=name,
                    path=workflow_path.relative_to(root).as_posix(),
                    selected=True,
                    reasons=(
                        "uncertain or shared impact widened selection to all PR workflows",
                    ),
                )
            )
            continue

        if pull_request is None:
            decisions.append(
                WorkflowDecision(
                    name=name,
                    path=workflow_path.relative_to(root).as_posix(),
                    selected=True,
                    reasons=("workflow has no pull-request path filter",),
                )
            )
            continue

        if not isinstance(pull_request, dict):
            decisions.append(
                WorkflowDecision(
                    name=name,
                    path=workflow_path.relative_to(root).as_posix(),
                    selected=True,
                    reasons=(
                        "unrecognized pull-request configuration widened this workflow",
                    ),
                )
            )
            continue

        include = pull_request.get("paths")
        ignore = pull_request.get("paths-ignore")
        include_patterns = (
            [str(include)] if isinstance(include, str) else list(include or [])
        )
        ignore_patterns = (
            [str(ignore)] if isinstance(ignore, str) else list(ignore or [])
        )

        if include_patterns:
            matches = sorted(
                path
                for path in paths
                if any(path_matches(path, pattern) for pattern in include_patterns)
            )
            decisions.append(
                WorkflowDecision(
                    name=name,
                    path=workflow_path.relative_to(root).as_posix(),
                    selected=bool(matches),
                    reasons=(
                        tuple(f"path filter matched {path}" for path in matches)
                        if matches
                        else (
                            "no changed path matched the governed pull-request path filters",
                        )
                    ),
                )
            )
            continue

        if ignore_patterns:
            non_ignored = sorted(
                path
                for path in paths
                if not any(path_matches(path, pattern) for pattern in ignore_patterns)
            )
            decisions.append(
                WorkflowDecision(
                    name=name,
                    path=workflow_path.relative_to(root).as_posix(),
                    selected=bool(non_ignored),
                    reasons=(
                        tuple(f"path is not ignored: {path}" for path in non_ignored)
                        if non_ignored
                        else ("all changed paths are covered by paths-ignore",)
                    ),
                )
            )
            continue

        decisions.append(
            WorkflowDecision(
                name=name,
                path=workflow_path.relative_to(root).as_posix(),
                selected=True,
                reasons=("workflow has no pull-request path filter",),
            )
        )
    return decisions


def build_report(
    root: Path,
    base_ref: str,
    *,
    paths: list[str] | None = None,
    metadata: dict[str, Any] | None = None,
    head_sha: str | None = None,
) -> dict[str, Any]:
    root = root.resolve()
    selected_paths = changed_paths(root, base_ref) if paths is None else sorted(set(paths))
    cargo_metadata = load_cargo_metadata(root) if metadata is None else metadata
    packages = package_inventory(root, cargo_metadata)
    direct, direct_reasons, unowned = direct_packages_for_paths(selected_paths, packages)
    affected, reverse_reasons = reverse_dependency_closure(direct, packages)
    widen_reasons = broadening_reasons(selected_paths, unowned)
    broadened = bool(widen_reasons)
    if broadened:
        affected = set(packages)

    package_reasons: dict[str, list[str]] = defaultdict(list)
    for package, reasons in direct_reasons.items():
        package_reasons[package].extend(reasons)
    for package, reasons in reverse_reasons.items():
        package_reasons[package].extend(reasons)
    if broadened:
        for package in packages:
            package_reasons[package].append(
                "selected because uncertainty widened to the full workspace"
            )

    workflows = workflow_decisions(root, selected_paths, broadened)
    return {
        "schema_version": SCHEMA_VERSION,
        "base_ref": base_ref,
        "head_sha": head_sha or current_commit(root),
        "changed_paths": selected_paths,
        "direct_packages": sorted(direct),
        "affected_packages": sorted(affected),
        "package_reasons": {
            package: sorted(set(reasons))
            for package, reasons in sorted(package_reasons.items())
            if package in affected
        },
        "unowned_paths": sorted(unowned),
        "broadened": broadened,
        "broadening_reasons": widen_reasons,
        "selected_workflows": [
            asdict(decision) for decision in workflows if decision.selected
        ],
        "skipped_workflows": [
            asdict(decision) for decision in workflows if not decision.selected
        ],
    }


def markdown_report(report: dict[str, Any]) -> str:
    lines = [
        "# Affected Scope",
        "",
        f"- Base: `{report['base_ref']}`",
        f"- Head: `{report['head_sha']}`",
        f"- Broadened: `{str(report['broadened']).lower()}`",
        "",
        "## Changed paths",
    ]
    lines.extend(f"- `{path}`" for path in report["changed_paths"])
    if not report["changed_paths"]:
        lines.append("- none")

    lines.extend(["", "## Rust package closure"])
    if report["affected_packages"]:
        for package in report["affected_packages"]:
            reasons = "; ".join(report["package_reasons"].get(package, []))
            lines.append(f"- `{package}` — {reasons or 'selected'}")
    else:
        lines.append("- no Rust package is affected")

    if report["broadening_reasons"]:
        lines.extend(["", "## Broadening reasons"])
        lines.extend(f"- {reason}" for reason in report["broadening_reasons"])

    lines.extend(["", "## Required workflows"])
    if report["selected_workflows"]:
        for workflow in report["selected_workflows"]:
            lines.append(
                f"- **{workflow['name']}** — {'; '.join(workflow['reasons'])}"
            )
    else:
        lines.append("- none")

    lines.extend(["", "## Safely skipped workflows"])
    if report["skipped_workflows"]:
        for workflow in report["skipped_workflows"]:
            lines.append(
                f"- **{workflow['name']}** — {'; '.join(workflow['reasons'])}"
            )
    else:
        lines.append("- none")
    return "\n".join(lines) + "\n"


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path.cwd())
    parser.add_argument("--base", default="origin/main")
    parser.add_argument("--json", action="store_true")
    args = parser.parse_args(argv)
    try:
        report = build_report(args.root, args.base)
    except RuntimeError as error:
        print(f"ERROR: {error}", file=sys.stderr)
        return 1
    if args.json:
        print(json.dumps(report, indent=2, sort_keys=True))
    else:
        print(markdown_report(report), end="")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
