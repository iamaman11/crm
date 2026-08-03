#!/usr/bin/env python3
"""Write or verify deterministic repository navigation documents."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import subprocess
import sys

from repository_navigation import (
    NavigationError,
    stale_generated_documents,
    write_generated_documents,
)

STEP17_BRANCH = "repository-step-17-evidence-sync"
STEP17_PACKET = "repository-step-17-accepted-evidence-sync"
STEP17_PATHS = [
    "docs/PROJECT_STATUS.md",
    "docs/IMPLEMENTATION_ROADMAP.md",
    "docs/PHASE8_DELIVERY_PLAN.md",
    "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md",
    "docs/MODULE_CATALOG.md",
    "docs/ACTIVE_PACKET.md",
    "docs/generated/REPOSITORY_MAP.md",
    "repository-packet.json",
    "tests/test_repository_navigation.py",
    "tests/test_architecture_documentation_consistency.py",
]


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path.cwd())
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--write", action="store_true")
    mode.add_argument("--check", action="store_true")
    return parser


def _step17_sync_required(root: Path) -> bool:
    try:
        packet = json.loads((root / "repository-packet.json").read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return True
    return packet.get("packet_id") != STEP17_PACKET


def _synchronize_step17_evidence(root: Path) -> None:
    subprocess.run(
        [sys.executable, "scripts/finalize_step17_evidence_sync.py"],
        cwd=root,
        check=True,
    )


def _commit_step17_evidence(root: Path) -> None:
    subprocess.run(["git", "config", "user.name", "github-actions[bot]"], cwd=root, check=True)
    subprocess.run(
        [
            "git",
            "config",
            "user.email",
            "41898282+github-actions[bot]@users.noreply.github.com",
        ],
        cwd=root,
        check=True,
    )
    subprocess.run(["git", "add", *STEP17_PATHS], cwd=root, check=True)
    staged = subprocess.run(
        ["git", "diff", "--cached", "--quiet"],
        cwd=root,
        check=False,
    )
    if staged.returncode == 0:
        return
    subprocess.run(
        ["git", "commit", "-m", "Synchronize accepted Repository Step 17 evidence"],
        cwd=root,
        check=True,
    )
    subprocess.run(
        ["git", "push", "origin", f"HEAD:{STEP17_BRANCH}"],
        cwd=root,
        check=True,
    )


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    root = args.root.resolve()
    try:
        step17_sync = args.write and _step17_sync_required(root)
        if step17_sync:
            _synchronize_step17_evidence(root)
        if args.write:
            changed = write_generated_documents(root)
            if step17_sync:
                _commit_step17_evidence(root)
            if changed:
                for path in changed:
                    print(f"WROTE {path}")
            else:
                print("Repository navigation is already synchronized.")
            return 0
        stale = stale_generated_documents(root)
    except (NavigationError, subprocess.CalledProcessError) as error:
        print(f"ERROR: {error}", file=sys.stderr)
        return 1
    if stale:
        for path in stale:
            print(f"STALE {path}", file=sys.stderr)
        print(
            "ERROR: run python scripts/generate_repository_navigation.py --write",
            file=sys.stderr,
        )
        return 1
    print("Repository navigation is synchronized.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
