#!/usr/bin/env python3
"""Enforce repository-wide GitHub Actions event and concurrency policy."""

from __future__ import annotations

import argparse
from dataclasses import dataclass
from pathlib import Path
import sys

WORKFLOW_SUFFIXES = {".yml", ".yaml"}
EXPECTED_GROUP = "group: ${{ github.workflow }}-${{ github.event.pull_request.number || github.ref }}"
EXPECTED_CANCEL = "cancel-in-progress: ${{ github.event_name == 'pull_request' }}"


@dataclass(frozen=True)
class WorkflowPolicyResult:
    path: Path
    errors: tuple[str, ...]


def _top_level_block(lines: list[str], key: str) -> tuple[int, int] | None:
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


def _event_block(lines: list[str]) -> tuple[int, int] | None:
    return _top_level_block(lines, "on")


def _contains_event(lines: list[str], start: int, end: int, event: str) -> bool:
    marker = f"  {event}:"
    return any(line.rstrip() == marker for line in lines[start + 1 : end])


def _event_section(lines: list[str], start: int, end: int, event: str) -> list[str]:
    marker = f"  {event}:"
    for index in range(start + 1, end):
        if lines[index].rstrip() != marker:
            continue
        section_end = index + 1
        while section_end < end:
            line = lines[section_end]
            if line.strip() and not line.startswith(("    ", "\t", "#")):
                break
            section_end += 1
        return lines[index:section_end]
    return []


def check_workflow(path: Path) -> WorkflowPolicyResult:
    lines = path.read_text(encoding="utf-8").splitlines()
    errors: list[str] = []
    events = _event_block(lines)
    if events is None:
        return WorkflowPolicyResult(path, ("missing top-level on block",))

    event_start, event_end = events
    has_push = _contains_event(lines, event_start, event_end, "push")
    has_pull_request = _contains_event(lines, event_start, event_end, "pull_request")
    has_workflow_dispatch = _contains_event(lines, event_start, event_end, "workflow_dispatch")
    crm_dev_workflow = (
        path.name == "cloudflare-crm-spike.yml"
        and path.parent.name == "workflows"
        and path.parent.parent.name == ".github"
    )
    if crm_dev_workflow:
        if not has_push:
            errors.append("CRM must run on push to dev")
        if has_pull_request:
            errors.append("CRM uses one dev pipeline, not duplicate pull_request builds")
        if has_workflow_dispatch:
            errors.append("CRM candidate preflight uses integration push, not workflow_dispatch")
        deploy_marker = "  deploy-crm-dev:"
        if deploy_marker not in lines:
            errors.append("CRM must retain the guarded deploy-crm-dev job")
        else:
            deploy_start = lines.index(deploy_marker)
            deploy_end = next((index for index in range(deploy_start + 1, len(lines))
                               if lines[index].startswith("  ")
                               and not lines[index].startswith("    ")
                               and lines[index].rstrip().endswith(":")), len(lines))
            deploy_section = "\n".join(lines[deploy_start:deploy_end])
            if "    if: github.event_name == 'push' && github.ref == 'refs/heads/dev'" not in deploy_section:
                errors.append("CRM deploy job must run only on push to dev")
    if has_pull_request:
        pr_section = _event_section(lines, event_start, event_end, "pull_request")
        expected_pr = "dev" if crm_dev_workflow else "main"
        branch_lines: list[str] = []
        if "    branches:" in pr_section:
            start = pr_section.index("    branches:") + 1
            for line in pr_section[start:]:
                if line.startswith("      - "):
                    branch_lines.append(line.strip())
                elif line.strip():
                    break
        # These branch filters are explicit, exact, and exclusive.
        if ("    branches:" not in pr_section
                or branch_lines != [f"- {expected_pr}"]
                or "    branches-ignore:" in pr_section):
            errors.append(f"pull_request must be restricted to branch {expected_pr}")
    if not crm_dev_workflow and not (has_push and has_pull_request):
        return WorkflowPolicyResult(path, tuple(errors))

    push_section = _event_section(lines, event_start, event_end, "push")
    # A single, explicit exception for the one CRM development release job.
    # All unrelated workflows retain their main-only policy.
    expected = ["- dev", "- 'integration/parallel/**'"] if crm_dev_workflow else ["- main"]
    branch_lines = []
    if "    branches:" in push_section:
        for line in push_section[push_section.index("    branches:") + 1:]:
            if line.startswith("      - "):
                branch_lines.append(line.strip())
            elif line.strip():
                break
    if branch_lines != expected or "    branches-ignore:" in push_section:
        errors.append("CRM push must be restricted to dev and integration/parallel/**"
                      if crm_dev_workflow else "push must be restricted to branch main")

    # Fail closed on any CI workflow that treats .md documentation as a
    # source of truth. GitHub Issues own the prose; only typed code and
    # machine-readable contracts can select a workflow.
    for event in ("push", "pull_request"):
        section = _event_section(lines, event_start, event_end, event)
        if "    paths:" in section:
            for line in section[section.index("    paths:") + 1:]:
                if line.startswith("      - "):
                    path_pattern = line.strip()[2:].strip("\"'")
                    if path_pattern.lower().endswith(".md") or path_pattern.startswith("docs/"):
                        errors.append("CI must not select Markdown documentation inputs")
                        break
                elif line.strip():
                    break
    if crm_dev_workflow:
        # Native GitHub path filtering only excludes documentation. YAML/SQL,
        # tests, source, lockfiles and unknown non-Markdown inputs remain checked.
        filters = []
        if "    paths-ignore:" in push_section:
            for line in push_section[push_section.index("    paths-ignore:") + 1:]:
                if line.startswith("      - "):
                    filters.append(line.strip()[2:].strip("\"'"))
                elif line.strip():
                    break
        if filters != ["*.md", "**/*.md"] or "    paths:" in push_section:
            errors.append("CRM may ignore only **/*.md documentation")
        forbidden_document_gates = (
            "tests.test_repository_navigation",
            "tests.test_architecture_documentation_consistency",
            "generate_repository_navigation.py --check",
            "docs/ACTIVE_PACKET.md",
            "docs/generated/REPOSITORY_MAP.md",
        )
        if any(gate in "\n".join(lines) for gate in forbidden_document_gates):
            errors.append("CRM CI must not validate Markdown documentation")

    concurrency = _top_level_block(lines, "concurrency")
    if concurrency is None:
        errors.append("missing top-level concurrency block")
    else:
        concurrency_lines = [line.strip() for line in lines[concurrency[0] + 1 : concurrency[1]]]
        group = "group: ${{ github.workflow }}-${{ github.ref }}" if crm_dev_workflow else EXPECTED_GROUP
        cancel = "cancel-in-progress: false" if crm_dev_workflow else EXPECTED_CANCEL
        if group not in concurrency_lines:
            errors.append("concurrency group must be workflow + PR number/ref")
        if cancel not in concurrency_lines:
            errors.append("CRM must not cancel an active release" if crm_dev_workflow
                          else "cancel-in-progress must be enabled only for pull_request")

    return WorkflowPolicyResult(path, tuple(errors))


def discover_workflows(root: Path) -> list[Path]:
    workflow_dir = root / ".github" / "workflows"
    return sorted(
        path
        for path in workflow_dir.iterdir()
        if path.is_file() and path.suffix in WORKFLOW_SUFFIXES
    )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path.cwd())
    args = parser.parse_args()

    workflows = discover_workflows(args.root)
    # The former main-only product-plane workflows are not part of CRM.
    # Only the exact-SHA dev pipeline may build, test or publish this app.
    canonical = {".github/workflows/cloudflare-crm-spike.yml"}
    observed = {path.relative_to(args.root).as_posix() for path in workflows}
    if observed != canonical:
        print(
            f"CRM requires exactly one workflow: missing={sorted(canonical - observed)}, "
            f"unexpected={sorted(observed - canonical)}",
            file=sys.stderr,
        )
        return 1

    failures = [result for path in workflows if (result := check_workflow(path)).errors]
    if failures:
        for result in failures:
            for error in result.errors:
                print(f"{result.path.relative_to(args.root)}: {error}", file=sys.stderr)
        return 1

    print("CI event policy is satisfied.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
