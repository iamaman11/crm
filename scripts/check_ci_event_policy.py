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
    if not (has_push and has_pull_request):
        return WorkflowPolicyResult(path, ())

    push_section = _event_section(lines, event_start, event_end, "push")
    if "    branches:" not in push_section or "      - main" not in push_section:
        errors.append("push must be restricted to branch main")

    concurrency = _top_level_block(lines, "concurrency")
    if concurrency is None:
        errors.append("missing top-level concurrency block")
    else:
        concurrency_lines = [line.strip() for line in lines[concurrency[0] + 1 : concurrency[1]]]
        if EXPECTED_GROUP not in concurrency_lines:
            errors.append("concurrency group must be workflow + PR number/ref")
        if EXPECTED_CANCEL not in concurrency_lines:
            errors.append("cancel-in-progress must be enabled only for pull_request")

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

    failures = [result for path in discover_workflows(args.root) if (result := check_workflow(path)).errors]
    if failures:
        for result in failures:
            for error in result.errors:
                print(f"{result.path.relative_to(args.root)}: {error}", file=sys.stderr)
        return 1

    print("CI event policy is satisfied.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
