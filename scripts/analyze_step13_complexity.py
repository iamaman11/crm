#!/usr/bin/env python3
"""ADR-031 current-main complexity, bypass, and change-cost measurement."""

from __future__ import annotations

import argparse
from collections import defaultdict, deque
import json
from pathlib import Path
import re
import subprocess
import sys
from typing import Any

try:
    from analyze_workspace import build_report as workspace_report
    from analyze_workspace import load_cargo_metadata, package_metrics, source_loc
except ModuleNotFoundError:
    from scripts.analyze_workspace import build_report as workspace_report
    from scripts.analyze_workspace import load_cargo_metadata, package_metrics, source_loc

SCHEMA = "crm.step13-complexity-baseline/v1"
POLICY = "step13-complexity-policy.json"
SKIP = {".git", "target", "node_modules", "artifacts"}
PUBLIC = re.compile(
    r"^\s*pub(?:\s*\([^)]*\))?\s+(?:async\s+)?"
    r"(?:const|enum|extern|fn|mod|static|struct|trait|type|union|use)\b"
)
SUPPRESSION = re.compile(r"#(?:!)?\[\s*(allow|expect)\s*\(([^)]*)\)\s*\]")
IGNORE = re.compile(r"#(?:!)?\[\s*ignore(?:\s*=\s*\"([^\"]+)\")?\s*\]")
DIRECT_LINT = re.compile(r"^\s*\[lints(?:\.[^\]]+)?\]\s*$", re.MULTILINE)
WORKSPACE_LINT = re.compile(
    r"^\s*\[lints\]\s*$[\s\S]*?^\s*workspace\s*=\s*true\s*$", re.MULTILINE
)
CENTRAL = {
    "crm-module-sdk": ("sdk-ports", "crates/crm-module-sdk/src"),
    "crm-core-contracts": ("stable-contracts", "crates/crm-core-contracts/src"),
    "crm-proto-contracts": ("stable-contracts", "crates/crm-proto-contracts/src"),
    "crm-capability-runtime": ("generic-runtime", "crates/crm-capability-runtime/src"),
    "crm-query-runtime": ("generic-runtime", "crates/crm-query-runtime/src"),
    "crm-application-composition": ("generic-composition", "crates/crm-application-composition/src"),
    "crm-core-data": ("infrastructure-ports", "crates/crm-core-data/src"),
    "crm-first-party-modules": ("first-party-aggregation", "crates/crm-first-party-modules/src"),
    "crm-application-runtime": ("process-composition", "crates/crm-application-runtime/src"),
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
    result = subprocess.run(command, cwd=root, text=True, capture_output=True, check=False)
    if result.returncode:
        raise RuntimeError(
            f"command failed ({' '.join(command)}):\n{result.stdout}\n{result.stderr}"
        )
    return result.stdout


def graph(metadata: dict[str, Any]):
    members = set(metadata.get("workspace_members", []))
    packages = {
        item["name"]: item
        for item in metadata.get("packages", [])
        if item.get("id") in members
    }
    names = set(packages)
    dependencies = {
        name: {
            dependency["name"]
            for dependency in package.get("dependencies", [])
            if dependency.get("name") in names
        }
        for name, package in packages.items()
    }
    dependents = {name: set() for name in names}
    for name, direct in dependencies.items():
        for dependency in direct:
            dependents[dependency].add(name)
    return packages, dependencies, dependents


def dependency_depths(dependencies: dict[str, set[str]]) -> dict[str, int]:
    memo: dict[str, int] = {}
    visiting: set[str] = set()

    def visit(name: str) -> int:
        if name in memo:
            return memo[name]
        if name in visiting:
            return 0
        visiting.add(name)
        value = 0 if not dependencies.get(name) else 1 + max(
            visit(item) for item in dependencies[name]
        )
        visiting.remove(name)
        memo[name] = value
        return value

    for name in sorted(dependencies):
        visit(name)
    return memo


def reverse_impact(name: str, dependents: dict[str, set[str]]) -> int:
    visited: set[str] = set()
    queue = deque(dependents.get(name, set()))
    while queue:
        item = queue.popleft()
        if item in visited:
            continue
        visited.add(item)
        queue.extend(dependents.get(item, set()))
    return len(visited)


def public_surface(packages: dict[str, dict[str, Any]]) -> dict[str, Any]:
    rows = []
    for name, package in sorted(packages.items()):
        root = Path(package["manifest_path"]).resolve().parent / "src"
        count = 0
        files = 0
        if root.exists():
            for path in sorted(root.rglob("*.rs")):
                files += 1
                count += sum(
                    bool(PUBLIC.match(line))
                    for line in path.read_text(encoding="utf-8").splitlines()
                )
        rows.append({"package": name, "files": files, "public_items": count})
    return {
        "total_public_items": sum(row["public_items"] for row in rows),
        "largest_packages": sorted(
            rows, key=lambda row: (-row["public_items"], row["package"])
        )[:25],
        "packages": rows,
        "measurement": "Conservative source-text count; not semantic rustdoc compatibility.",
    }


def repository_files(root: Path):
    for path in sorted(root.rglob("*")):
        if path.is_file():
            relative = path.relative_to(root)
            if not any(part in SKIP for part in relative.parts):
                yield path, relative.as_posix()


def suppression_inventory(root: Path) -> dict[str, Any]:
    entries = []
    for path, relative in repository_files(root):
        if path.name == "Cargo.toml":
            text = path.read_text(encoding="utf-8")
            match = DIRECT_LINT.search(text)
            if match and not WORKSPACE_LINT.search(text):
                entries.append(
                    {
                        "kind": "direct-lint-table",
                        "path": relative,
                        "line": text[: match.start()].count("\n") + 1,
                        "detail": "package-local [lints] table",
                    }
                )
            continue
        if path.suffix != ".rs":
            continue
        for number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
            for match in SUPPRESSION.finditer(line):
                entries.append(
                    {
                        "kind": f"rust-{match.group(1)}",
                        "path": relative,
                        "line": number,
                        "detail": " ".join(match.group(2).split()),
                    }
                )
            match = IGNORE.search(line)
            if match:
                entries.append(
                    {
                        "kind": "ignored-test",
                        "path": relative,
                        "line": number,
                        "detail": match.group(1) or "unqualified #[ignore]",
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


def package_for_path(path: str, packages: dict[str, dict[str, Any]], root: Path):
    candidate = (root / path).resolve()
    matches = []
    for name, package in packages.items():
        package_root = Path(package["manifest_path"]).resolve().parent
        try:
            candidate.relative_to(package_root)
            matches.append((len(package_root.parts), name))
        except ValueError:
            pass
    return max(matches)[1] if matches else None


def changed_paths(root: Path, commit: str) -> list[str]:
    return sorted(
        {
            line.strip()
            for line in run(
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
            ).splitlines()
            if line.strip()
        }
    )


def change_cost(
    root: Path, policy: dict[str, Any], packages: dict[str, dict[str, Any]]
) -> list[dict[str, Any]]:
    rows = []
    for exemplar in policy["representative_changes"]:
        paths = changed_paths(root, exemplar["commit"])
        touched = sorted(
            {
                package
                for path in paths
                if (package := package_for_path(path, packages, root))
            }
        )
        central = [path for path in paths if path.startswith(CENTRAL_PREFIXES)]
        workflows = [path for path in paths if path.startswith(".github/workflows/")]
        rows.append(
            {
                **exemplar,
                "file_count": len(paths),
                "package_count": len(touched),
                "packages": touched,
                "central_file_count": len(central),
                "central_files": central,
                "workflow_file_count": len(workflows),
                "workflow_files": workflows,
                "paths": paths,
            }
        )
    return rows


def central_metrics(
    root: Path,
    dependencies: dict[str, set[str]],
    dependents: dict[str, set[str]],
    depths: dict[str, int],
    public_by_name: dict[str, dict[str, Any]],
) -> list[dict[str, Any]]:
    rows = []
    for name, (role, source) in CENTRAL.items():
        present = name in dependencies
        loc = source_loc(root, source) if present else {}
        rows.append(
            {
                "package": name,
                "role": role,
                "present": present,
                "direct_dependencies": sorted(dependencies.get(name, set())),
                "direct_dependency_count": len(dependencies.get(name, set())),
                "direct_consumers": sorted(dependents.get(name, set())),
                "direct_consumer_count": len(dependents.get(name, set())),
                "transitive_reverse_impact": reverse_impact(name, dependents),
                "dependency_depth": depths.get(name, 0),
                "public_items": public_by_name.get(name, {}).get("public_items", 0),
                "source": loc,
                "risk": (
                    "shared stable boundary; fan-out is acceptable only while small and stable"
                    if role in {"stable-contracts", "sdk-ports"}
                    else "mutable central boundary; growth requires measured justification"
                ),
            }
        )
    return rows


def thin_wrappers(
    root: Path,
    packages: dict[str, dict[str, Any]],
    dependents: dict[str, set[str]],
    public_by_name: dict[str, dict[str, Any]],
    maximum_loc: int,
) -> list[dict[str, Any]]:
    rows = []
    for name, package in sorted(packages.items()):
        if len(dependents.get(name, set())) != 1:
            continue
        source = Path(package["manifest_path"]).resolve().parent / "src"
        try:
            relative = source.relative_to(root).as_posix()
        except ValueError:
            continue
        loc = source_loc(root, relative)
        if loc["non_comment_lines"] <= maximum_loc:
            rows.append(
                {
                    "package": name,
                    "consumer": sorted(dependents[name])[0],
                    "non_comment_loc": loc["non_comment_lines"],
                    "public_items": public_by_name.get(name, {}).get("public_items", 0),
                    "classification": "candidate-only; measurement authorizes no consolidation",
                }
            )
    return rows


def build_report(root: Path) -> dict[str, Any]:
    policy = json.loads((root / POLICY).read_text(encoding="utf-8"))
    base = workspace_report(root)
    metadata = load_cargo_metadata(root)
    packages, dependencies, dependents = graph(metadata)
    depths = dependency_depths(dependencies)
    public = public_surface(packages)
    public_by_name = {row["package"]: row for row in public["packages"]}
    return {
        "schema_version": SCHEMA,
        "commit_sha": base["commit_sha"],
        "measurement_mode": "calibration-only",
        "workspace_baseline": base,
        "dependency_graph": {
            "maximum_depth": max(depths.values(), default=0),
            "deepest_packages": [
                {"package": name, "depth": depth}
                for name, depth in sorted(
                    depths.items(), key=lambda item: (-item[1], item[0])
                )[:25]
            ],
        },
        "public_rust_surface": public,
        "central_systems": central_metrics(
            root, dependencies, dependents, depths, public_by_name
        ),
        "suppression_inventory": suppression_inventory(root),
        "representative_change_cost": change_cost(root, policy, packages),
        "thin_wrapper_candidates": thin_wrappers(
            root,
            packages,
            dependents,
            public_by_name,
            int(policy["calibration"]["thin_wrapper_maximum_non_comment_loc"]),
        ),
        "calibration": policy["calibration"],
        "limitations": [
            "Public item counts are conservative source-text measurements.",
            "Historical change costs do not infer elapsed developer time.",
            "This first packet inventories suppressions; the accepted baseline is registered and enforced in the next bounded step-13 packet.",
            "Thin-wrapper results are candidates only.",
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
        "> ADR-031 measurement and governance calibration only. No structural remediation is authorized.",
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
    for item in report["central_systems"]:
        lines.append(
            f"| `{item['package']}` | {item['role']} | "
            f"{item['direct_dependency_count']} | {item['direct_consumer_count']} | "
            f"{item['transitive_reverse_impact']} | {item['dependency_depth']} | "
            f"{item['public_items']} | {item.get('source', {}).get('non_comment_lines', 0)} |"
        )
    lines.extend(["", "## Suppression and bypass inventory", "", "| Kind | Count |", "|---|---:|"])
    for kind, count in report["suppression_inventory"]["counts_by_kind"].items():
        lines.append(f"| `{kind}` | {count} |")
    lines.extend(
        [
            "",
            "## Representative change cost",
            "",
            "| Exemplar | Kind | Files | Packages | Central files | Workflow files |",
            "|---|---|---:|---:|---:|---:|",
        ]
    )
    for item in report["representative_change_cost"]:
        lines.append(
            f"| `{item['id']}` | {item['kind']} | {item['file_count']} | "
            f"{item['package_count']} | {item['central_file_count']} | "
            f"{item['workflow_file_count']} |"
        )
    lines.extend(["", "## Candidate-only thin wrappers", ""])
    if report["thin_wrapper_candidates"]:
        lines.extend(["| Package | Sole consumer | LOC | Public items |", "|---|---|---:|---:|"])
        for item in report["thin_wrapper_candidates"]:
            lines.append(
                f"| `{item['package']}` | `{item['consumer']}` | "
                f"{item['non_comment_loc']} | {item['public_items']} |"
            )
    else:
        lines.append("No candidate matched the calibrated threshold.")
    lines.extend(["", "## Calibration", ""])
    for key, value in sorted(report["calibration"].items()):
        lines.append(f"- `{key}`: `{value}`")
    lines.extend(["", "## Limitations", ""])
    lines.extend(f"- {item}" for item in report["limitations"])
    lines.append("")
    return "\n".join(lines)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path.cwd())
    parser.add_argument("--json-output", type=Path)
    parser.add_argument("--markdown-output", type=Path)
    args = parser.parse_args()
    try:
        report = build_report(args.root.resolve())
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
