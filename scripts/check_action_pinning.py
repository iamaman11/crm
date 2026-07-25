#!/usr/bin/env python3
"""Reject mutable external GitHub Action references in repository workflows."""

from __future__ import annotations

import argparse
from dataclasses import dataclass
from pathlib import Path
import re
import sys

WORKFLOW_SUFFIXES = {".yml", ".yaml"}
USES_PATTERN = re.compile(r"^(?P<prefix>\s*-?\s*uses:\s*)(?P<reference>[^#\s]+)(?P<comment>\s+#\s*.+)?$")
FULL_SHA_PATTERN = re.compile(r"^[0-9a-f]{40}$")


@dataclass(frozen=True)
class ActionPinningFailure:
    path: Path
    line_number: int
    message: str


def check_workflow(path: Path) -> tuple[ActionPinningFailure, ...]:
    failures: list[ActionPinningFailure] = []
    for line_number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), start=1):
        match = USES_PATTERN.match(line)
        if match is None:
            continue
        reference = match.group("reference")
        if reference.startswith("./") or reference.startswith("docker://"):
            continue
        if "@" not in reference:
            failures.append(ActionPinningFailure(path, line_number, "external Action reference is missing @ref"))
            continue
        action, ref = reference.rsplit("@", 1)
        if "/" not in action:
            failures.append(ActionPinningFailure(path, line_number, "external Action must use owner/repository syntax"))
            continue
        if FULL_SHA_PATTERN.fullmatch(ref) is None:
            failures.append(ActionPinningFailure(path, line_number, "external Action must be pinned to a full lowercase commit SHA"))
        comment = match.group("comment")
        if comment is None or not comment.removeprefix(" # ").strip():
            failures.append(ActionPinningFailure(path, line_number, "pinned Action must retain a human-readable version comment"))
    return tuple(failures)


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

    failures = [
        failure
        for path in discover_workflows(args.root)
        for failure in check_workflow(path)
    ]
    if failures:
        for failure in failures:
            relative = failure.path.relative_to(args.root)
            print(f"{relative}:{failure.line_number}: {failure.message}", file=sys.stderr)
        return 1

    print("External GitHub Actions are immutably pinned.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
