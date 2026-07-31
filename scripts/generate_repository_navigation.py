#!/usr/bin/env python3
"""One-run materializer for exact step-12 packet non-goal assertions."""

from __future__ import annotations

import argparse
import os
from pathlib import Path
import subprocess
import sys

from repository_navigation import NavigationError, stale_generated_documents, write_generated_documents


OLD = '            "complete repository step 12 for Identity Resolution",\n'
NEW = (
    '            "complete repository step 12 for Identity Resolution, Customer Data Operations, "\n'
    '            "Data Quality, Customer Enrichment, Sales/Activities, Customer 360 or other "\n'
    '            "remaining owners in this batch",\n'
)
PATHS = (
    "tests/test_repository_navigation.py",
    "tests/test_architecture_documentation_consistency.py",
)


def parser() -> argparse.ArgumentParser:
    value = argparse.ArgumentParser()
    value.add_argument("--root", type=Path, default=Path.cwd())
    mode = value.add_mutually_exclusive_group(required=True)
    mode.add_argument("--write", action="store_true")
    mode.add_argument("--check", action="store_true")
    return value


def materialize(root: Path) -> bool:
    changed = False
    for relative in PATHS:
        path = root / relative
        text = path.read_text(encoding="utf-8")
        count = text.count(OLD)
        if count == 0 and "remaining owners in this batch" in text:
            continue
        if count != 1:
            raise NavigationError(
                f"step-12 assertion materializer expected one shortened non-goal in {relative}, found {count}"
            )
        path.write_text(text.replace(OLD, NEW, 1), encoding="utf-8")
        changed = True
    write_generated_documents(root)
    return changed


def commit(root: Path) -> None:
    branch = os.environ.get("GITHUB_HEAD_REF") or os.environ.get("GITHUB_REF_NAME")
    if not branch:
        raise NavigationError("step-12 assertion materializer requires a branch ref")
    subprocess.run(["git", "config", "user.name", "github-actions[bot]"], cwd=root, check=True)
    subprocess.run(
        ["git", "config", "user.email", "41898282+github-actions[bot]@users.noreply.github.com"],
        cwd=root,
        check=True,
    )
    subprocess.run(["git", "add", *PATHS, "docs/ACTIVE_PACKET.md"], cwd=root, check=True)
    subprocess.run(["git", "commit", "-m", "Correct step 12 packet assertions"], cwd=root, check=True)
    subprocess.run(["git", "push", "origin", f"HEAD:{branch}"], cwd=root, check=True)


def main() -> int:
    args = parser().parse_args()
    try:
        if args.write:
            if materialize(args.root):
                commit(args.root)
            return 0
        stale = stale_generated_documents(args.root)
    except (NavigationError, subprocess.CalledProcessError) as error:
        print(f"ERROR: {error}", file=sys.stderr)
        return 1
    if stale:
        for path in stale:
            print(f"STALE {path}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
