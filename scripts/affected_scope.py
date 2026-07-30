#!/usr/bin/env python3
"""Deterministic, fail-closed affected-scope analysis for repository checks."""

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

SCHEMA_VERSION = "crm.affected-scope/v2"
POLICY_SCHEMA_VERSION = "crm.affected-scope-policy/v1"
POLICY_PATH = Path("affected-scope-policy.json")
WORKFLOW_SUFFIXES = {".yml", ".yaml"}
REQUIRED_SCOPE_IDS = {
    "contracts", "protobuf_api_compatibility", "database_migrations",
    "postgresql_acceptance", "process_runtime_acceptance", "product_plane",
    "frontend", "operations",
}
BROAD_PATH_PATTERNS = (
    "Cargo.toml", "Cargo.lock", ".github/workflows/**",
    "affected-scope-policy.json", "repository-packet.json", "architecture-policy.json",
    "architecture-governance.json", "workspace-dependency-policy.json",
    "requirements-dev.txt", "scripts/affected_scope.py", "scripts/repo.py",
    "scripts/repository_navigation.py", "scripts/check_architecture.py",
    "scripts/check_ci_event_policy.py", "scripts/analyze_workspace.py",
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


@dataclass(frozen=True)
class ScopeRule:
    id: str
    owner: str
    path_patterns: tuple[str, ...]
    required_workflows: tuple[str, ...]


@dataclass(frozen=True)
class ScopeDecision:
    id: str
    owner: str
    selected: bool
    paths: tuple[str, ...]
    required_workflows: tuple[str, ...]
    reasons: tuple[str, ...]


def run_capture(command: list[str], root: Path) -> str:
    completed = subprocess.run(
        command, cwd=root, check=False, text=True, capture_output=True
    )
    if completed.returncode:
        raise RuntimeError(
            f"command failed ({' '.join(command)}):\n{completed.stdout}\n{completed.stderr}"
        )
    return completed.stdout


def load_cargo_metadata(root: Path) -> dict[str, Any]:
    return json.loads(run_capture(
        ["cargo", "metadata", "--format-version", "1", "--locked", "--no-deps"],
        root,
    ))


def changed_paths(root: Path, base_ref: str) -> list[str]:
    output = run_capture(
        ["git", "diff", "--name-only", "--diff-filter=ACMRD", f"{base_ref}...HEAD"],
        root,
    )
    return sorted({line.strip().replace("\\", "/") for line in output.splitlines() if line.strip()})


def current_commit(root: Path) -> str:
    return run_capture(["git", "rev-parse", "HEAD"], root).strip()


def _text(value: Any, field: str) -> str:
    if not isinstance(value, str) or not value.strip():
        raise RuntimeError(f"{field} must be a non-empty string")
    return value.strip()


def _strings(value: Any, field: str) -> tuple[str, ...]:
    if not isinstance(value, list) or not value:
        raise RuntimeError(f"{field} must be a non-empty list")
    result = tuple(_text(item, f"{field}[{index}]") for index, item in enumerate(value))
    if len(result) != len(set(result)):
        raise RuntimeError(f"{field} must not contain duplicates")
    return result


def load_scope_policy(root: Path) -> tuple[tuple[ScopeRule, ...], tuple[str, ...]]:
    try:
        document = json.loads((root / POLICY_PATH).read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise RuntimeError(f"cannot read {POLICY_PATH}: {error}") from error
    if not isinstance(document, dict) or document.get("schema_version") != POLICY_SCHEMA_VERSION:
        raise RuntimeError(f"{POLICY_PATH} must use schema {POLICY_SCHEMA_VERSION}")
    raw_scopes = document.get("scopes")
    if not isinstance(raw_scopes, list) or not raw_scopes:
        raise RuntimeError("affected-scope policy scopes must be a non-empty list")
    rules: list[ScopeRule] = []
    ids: set[str] = set()
    for index, raw in enumerate(raw_scopes):
        if not isinstance(raw, dict):
            raise RuntimeError(f"scopes[{index}] must be an object")
        scope_id = _text(raw.get("id"), f"scopes[{index}].id")
        if scope_id in ids:
            raise RuntimeError(f"duplicate affected-scope id: {scope_id}")
        ids.add(scope_id)
        rules.append(ScopeRule(
            id=scope_id,
            owner=_text(raw.get("owner"), f"scopes[{index}].owner"),
            path_patterns=_strings(raw.get("path_patterns"), f"scopes[{index}].path_patterns"),
            required_workflows=_strings(raw.get("required_workflows"), f"scopes[{index}].required_workflows"),
        ))
    missing, unexpected = sorted(REQUIRED_SCOPE_IDS - ids), sorted(ids - REQUIRED_SCOPE_IDS)
    if missing or unexpected:
        raise RuntimeError(
            "affected-scope policy must define the exact repository step 9 categories; "
            f"missing={missing}, unexpected={unexpected}"
        )
    neutral = _strings(document.get("neutral_path_patterns"), "neutral_path_patterns")
    return tuple(sorted(rules, key=lambda rule: rule.id)), neutral


def package_inventory(root: Path, metadata: dict[str, Any]) -> dict[str, PackageInfo]:
    workspace_ids = set(metadata.get("workspace_members", []))
    packages = [p for p in metadata.get("packages", []) if p.get("id") in workspace_ids]
    names = {p["name"] for p in packages}
    result: dict[str, PackageInfo] = {}
    for package in packages:
        manifest = Path(package["manifest_path"]).resolve().relative_to(root.resolve())
        dependencies = tuple(sorted({
            item["name"] for item in package.get("dependencies", []) if item.get("name") in names
        }))
        result[package["name"]] = PackageInfo(
            package["name"], manifest.parent.as_posix(), manifest.as_posix(), dependencies
        )
    return dict(sorted(result.items()))


def direct_packages_for_paths(
    paths: list[str], packages: dict[str, PackageInfo]
) -> tuple[set[str], dict[str, list[str]], list[str]]:
    roots = sorted(
        ((info.root.rstrip("/"), name) for name, info in packages.items()),
        key=lambda item: len(item[0]), reverse=True,
    )
    direct: set[str] = set()
    reasons: dict[str, list[str]] = defaultdict(list)
    unowned: list[str] = []
    for path in paths:
        owner = next((name for root, name in roots if path == root or path.startswith(f"{root}/")), None)
        if owner is None:
            unowned.append(path)
        else:
            direct.add(owner)
            reasons[owner].append(f"directly owns changed path {path}")
    return direct, {name: sorted(values) for name, values in reasons.items()}, unowned


def reverse_dependency_closure(
    direct: set[str], packages: dict[str, PackageInfo]
) -> tuple[set[str], dict[str, list[str]]]:
    dependents = {name: set() for name in packages}
    for package in packages.values():
        for dependency in package.dependencies:
            dependents[dependency].add(package.name)
    affected, reasons, queue = set(direct), defaultdict(list), deque(sorted(direct))
    while queue:
        package = queue.popleft()
        for dependent in sorted(dependents[package]):
            reasons[dependent].append(f"reverse-depends on affected package {package}")
            if dependent not in affected:
                affected.add(dependent)
                queue.append(dependent)
    return affected, {name: sorted(set(values)) for name, values in reasons.items()}


def path_matches(path: str, pattern: str) -> bool:
    normalized, normalized_pattern = path.replace("\\", "/"), pattern.replace("\\", "/")
    return fnmatch.fnmatchcase(normalized, normalized_pattern) or PurePosixPath(normalized).match(normalized_pattern)


def scope_paths(paths: list[str], rules: tuple[ScopeRule, ...]) -> dict[str, tuple[str, ...]]:
    return {
        rule.id: tuple(sorted(path for path in paths if any(path_matches(path, pattern) for pattern in rule.path_patterns)))
        for rule in rules
    }


def unknown_impact_paths(
    unowned: list[str], rules: tuple[ScopeRule, ...], neutral: tuple[str, ...]
) -> list[str]:
    governed = tuple(pattern for rule in rules for pattern in rule.path_patterns)
    return sorted(
        path for path in unowned
        if not any(
            path_matches(path, pattern)
            for pattern in (*governed, *neutral, *BROAD_PATH_PATTERNS)
        )
    )


def broadening_reasons(paths: list[str], unknown: list[str]) -> list[str]:
    reasons: list[str] = []
    for path in paths:
        if any(path_matches(path, pattern) for pattern in BROAD_PATH_PATTERNS):
            reasons.append(f"{path} changes the workspace, check graph, workflow graph, or shared policy")
    for path in unknown:
        reasons.append(f"{path} has no workspace-package owner or governed non-package scope")
    return sorted(set(reasons))


def _workflow_events(document: dict[str, Any]) -> Any:
    return document["on"] if "on" in document else document.get(True)


def workflow_decisions(root: Path, paths: list[str]) -> list[WorkflowDecision]:
    yaml, decisions = YAML(typ="safe"), []
    for workflow_path in sorted((root / ".github/workflows").iterdir()):
        if not workflow_path.is_file() or workflow_path.suffix not in WORKFLOW_SUFFIXES or workflow_path.name.startswith("one-time-"):
            continue
        document = yaml.load(workflow_path.read_text(encoding="utf-8")) or {}
        name, events = str(document.get("name") or workflow_path.stem), _workflow_events(document)
        if isinstance(events, str):
            events = {events: None}
        elif isinstance(events, list):
            events = {str(event): None for event in events}
        if not isinstance(events, dict) or "pull_request" not in events:
            continue
        relative = workflow_path.relative_to(root).as_posix()
        pull_request = events["pull_request"]
        if pull_request is None:
            decisions.append(WorkflowDecision(name, relative, True, ("workflow has no pull-request path filter",)))
            continue
        if not isinstance(pull_request, dict):
            decisions.append(WorkflowDecision(name, relative, True, ("unrecognized pull-request configuration widened this workflow",)))
            continue
        include, ignore = pull_request.get("paths"), pull_request.get("paths-ignore")
        includes = [str(include)] if isinstance(include, str) else list(include or [])
        ignores = [str(ignore)] if isinstance(ignore, str) else list(ignore or [])
        if includes:
            matches = sorted(path for path in paths if any(path_matches(path, pattern) for pattern in includes))
            reasons = tuple(f"path filter matched {path}" for path in matches) if matches else ("no changed path matched the governed pull-request path filters",)
            decisions.append(WorkflowDecision(name, relative, bool(matches), reasons))
        elif ignores:
            remaining = sorted(path for path in paths if not any(path_matches(path, pattern) for pattern in ignores))
            reasons = tuple(f"path is not ignored: {path}" for path in remaining) if remaining else ("all changed paths are covered by paths-ignore",)
            decisions.append(WorkflowDecision(name, relative, bool(remaining), reasons))
        else:
            decisions.append(WorkflowDecision(name, relative, True, ("workflow has no pull-request path filter",)))
    return decisions


def enforce_scope_workflow_coverage(
    rules: tuple[ScopeRule, ...], selected_paths: dict[str, tuple[str, ...]], workflows: list[WorkflowDecision]
) -> None:
    by_name = {workflow.name: workflow for workflow in workflows}
    for rule in rules:
        if not selected_paths[rule.id]:
            continue
        absent = sorted(set(rule.required_workflows) - set(by_name))
        if absent:
            raise RuntimeError(f"scope {rule.id} requires missing permanent PR workflows: {absent}")
        for required in rule.required_workflows:
            if not by_name[required].selected:
                raise RuntimeError(
                    f"scope {rule.id} requires {required}, but its governed pull-request path filters did not select it"
                )


def _scope_decisions(rules: tuple[ScopeRule, ...], selected: dict[str, tuple[str, ...]]) -> list[ScopeDecision]:
    return [ScopeDecision(
        rule.id, rule.owner, bool(selected[rule.id]), selected[rule.id], rule.required_workflows,
        tuple(f"changed path {path} is owned by scope {rule.id}" for path in selected[rule.id])
        if selected[rule.id] else ("no changed path matched the scope policy",),
    ) for rule in rules]


def build_report(
    root: Path, base_ref: str, *, paths: list[str] | None = None,
    metadata: dict[str, Any] | None = None, head_sha: str | None = None,
) -> dict[str, Any]:
    root = root.resolve()
    selected_paths = changed_paths(root, base_ref) if paths is None else sorted(set(paths))
    packages = package_inventory(root, load_cargo_metadata(root) if metadata is None else metadata)
    rules, neutral = load_scope_policy(root)
    selected_scope_paths = scope_paths(selected_paths, rules)
    direct, direct_reasons, unowned = direct_packages_for_paths(selected_paths, packages)
    affected, reverse_reasons = reverse_dependency_closure(direct, packages)
    unknown = unknown_impact_paths(unowned, rules, neutral)
    widen_reasons = broadening_reasons(selected_paths, unknown)
    broadened = bool(widen_reasons)
    workflows = workflow_decisions(root, selected_paths)
    enforce_scope_workflow_coverage(rules, selected_scope_paths, workflows)
    if unknown:
        raise RuntimeError(
            "unknown affected scope cannot prove a safe non-Rust workflow closure; "
            f"classify these paths in {POLICY_PATH}: {unknown}"
        )
    if broadened:
        affected = set(packages)
    package_reasons: dict[str, list[str]] = defaultdict(list)
    for package, reasons in direct_reasons.items():
        package_reasons[package].extend(reasons)
    for package, reasons in reverse_reasons.items():
        package_reasons[package].extend(reasons)
    if broadened:
        for package in packages:
            package_reasons[package].append("selected because uncertainty widened to the full workspace")
    scopes = _scope_decisions(rules, selected_scope_paths)
    return {
        "schema_version": SCHEMA_VERSION,
        "policy_schema_version": POLICY_SCHEMA_VERSION,
        "base_ref": base_ref,
        "head_sha": head_sha or current_commit(root),
        "changed_paths": selected_paths,
        "direct_packages": sorted(direct),
        "affected_packages": sorted(affected),
        "package_reasons": {package: sorted(set(reasons)) for package, reasons in sorted(package_reasons.items()) if package in affected},
        "unowned_paths": sorted(unowned),
        "broadened": broadened,
        "broadening_reasons": widen_reasons,
        "selected_scopes": [asdict(item) for item in scopes if item.selected],
        "skipped_scopes": [asdict(item) for item in scopes if not item.selected],
        "selected_workflows": [asdict(item) for item in workflows if item.selected],
        "skipped_workflows": [asdict(item) for item in workflows if not item.selected],
    }


def markdown_report(report: dict[str, Any]) -> str:
    lines = [
        "# Affected Scope", "", f"- Base: `{report['base_ref']}`",
        f"- Head: `{report['head_sha']}`", f"- Policy: `{report['policy_schema_version']}`",
        f"- Broadened: `{str(report['broadened']).lower()}`", "", "## Changed paths",
    ]
    lines.extend(f"- `{path}`" for path in report["changed_paths"])
    if not report["changed_paths"]:
        lines.append("- none")
    lines.extend(["", "## Repository scopes"])
    if report["selected_scopes"]:
        for scope in report["selected_scopes"]:
            lines.append(f"- **{scope['id']}** — owner `{scope['owner']}`; required workflows: {', '.join(scope['required_workflows'])}")
            lines.extend(f"  - `{path}`" for path in scope["paths"])
    else:
        lines.append("- no non-Rust repository scope is affected")
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
    lines.extend(
        (f"- **{workflow['name']}** — {'; '.join(workflow['reasons'])}" for workflow in report["selected_workflows"]),
    )
    if not report["selected_workflows"]:
        lines.append("- none")
    lines.extend(["", "## Safely skipped workflows"])
    lines.extend(
        (f"- **{workflow['name']}** — {'; '.join(workflow['reasons'])}" for workflow in report["skipped_workflows"]),
    )
    if not report["skipped_workflows"]:
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
    print(json.dumps(report, indent=2, sort_keys=True) if args.json else markdown_report(report), end="\n" if args.json else "")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
