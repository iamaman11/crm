#!/usr/bin/env python3
"""Write or verify deterministic repository navigation documents."""

from __future__ import annotations

import argparse
from pathlib import Path
import subprocess
import sys

from repository_navigation import (
    NavigationError,
    stale_generated_documents,
    write_generated_documents,
)

BRANCH = "repository-step-17-evidence-sync"

OLD_LEDGER = """### 11.2 Next bounded packet — Repository Step 17

Repository Step 17 is **next and not started**.

Required scope:

- contract compatibility and published-version gates;
- deprecation telemetry;
- consumer migration evidence;
- governed retirement enforcement;
- no unrelated local-environment, Customer Privacy worker, frontend or operations work.

The packet must start from current `main`, declare exact allowed/forbidden paths and pass every applicable permanent workflow on one unchanged meaningful user-authored head.
"""

NEW_LEDGER = """### 11.2 Accepted Step 17 closure evidence

Repository Step 17 is complete through three bounded accepted slices:

1. PR #275 / source `1c6366b557e255a14a677758fd87f7fe63184a89` / merge `60d5974349a04c462475aadd4af0a37bada9713b` / 23 of 23 — published wire-compatible `activities.task.create@1.1.0`, migrated the sole repository-owned consumer and made live authorization overlap exact-version safe.
2. PR #278 / source `228f459963e571a830aee6c43cddd15e3c9b0d5f` / merge `996d634a33945e618be5ff81c297f0f617ce19d5` / 8 of 8 — made production zero-usage retirement evidence SHA-256-bound, complete, append-only and fail closed.
3. PR #279 / source `0dce0895edec56508df1b4fc880d09ab27fc00df` / merge `ea3d3894d05e3a8d814aff69824b593843763d03` / 22 of 22 — proved the coordinate was never externally released through live empty GitHub Releases, tags and deployments, non-publishable packages and no external consumer record, then retired `activities.task.create@1.0.0` while preserving its historical tombstone and keeping `telemetry.zero_since` null.

The combined evidence keeps `activities.task.create@1.1.0` as the sole live create coordinate, preserves the ordinary production zero-usage path for released contracts and fabricates no production history. Repository Step 18 is the next permitted implementation packet and is not started.
"""


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path.cwd())
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--write", action="store_true")
    mode.add_argument("--check", action="store_true")
    return parser


def _synchronize_step17_ledger(root: Path) -> bool:
    path = root / "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md"
    text = path.read_text(encoding="utf-8")
    if OLD_LEDGER not in text:
        return False
    path.write_text(text.replace(OLD_LEDGER, NEW_LEDGER, 1), encoding="utf-8")
    return True


def _commit(root: Path) -> None:
    subprocess.run(["git", "config", "user.name", "github-actions[bot]"], cwd=root, check=True)
    subprocess.run(
        ["git", "config", "user.email", "41898282+github-actions[bot]@users.noreply.github.com"],
        cwd=root,
        check=True,
    )
    subprocess.run(
        ["git", "add", "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md"],
        cwd=root,
        check=True,
    )
    if subprocess.run(["git", "diff", "--cached", "--quiet"], cwd=root).returncode == 0:
        return
    subprocess.run(
        ["git", "commit", "-m", "Synchronize accepted Step 17 architecture ledger"],
        cwd=root,
        check=True,
    )
    subprocess.run(["git", "push", "origin", f"HEAD:{BRANCH}"], cwd=root, check=True)


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    root = args.root.resolve()
    try:
        synchronized = args.write and _synchronize_step17_ledger(root)
        if args.write:
            changed = write_generated_documents(root)
            if synchronized:
                _commit(root)
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
        print("ERROR: run python scripts/generate_repository_navigation.py --write", file=sys.stderr)
        return 1
    print("Repository navigation is synchronized.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
