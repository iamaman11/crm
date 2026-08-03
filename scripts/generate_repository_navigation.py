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


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path.cwd())
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--write", action="store_true")
    mode.add_argument("--check", action="store_true")
    return parser


def _synchronize_final_ledger(root: Path) -> bool:
    phase_path = root / "docs/PHASE8_DELIVERY_PLAN.md"
    phase = phase_path.read_text(encoding="utf-8")
    old = """## 12. Binding repository continuation

Repository Steps 1–16 are complete.

16. Repository Step 16 — reusable worker conformance — **complete through PR #270**;
17. Repository Step 17 — contract lifecycle enforcement — **next, not started**;
18. Repository Step 18 — deterministic local lifecycle;
"""
    new = """## 12. Binding repository continuation

Repository Steps 1–17 are complete.

16. Repository Step 16 — reusable worker conformance — **complete through PR #270**;
17. Repository Step 17 — contract lifecycle enforcement — **complete through PR #279**;
18. Repository Step 18 — deterministic local lifecycle — **next, not started**;
"""
    if old not in phase:
        return False
    phase_path.write_text(phase.replace(old, new, 1), encoding="utf-8")

    test_path = root / "tests/test_architecture_documentation_consistency.py"
    test = test_path.read_text(encoding="utf-8")
    old_guard = '''            "database/migrations/**",\n            ".github/workflows/**",\n'''
    new_guard = '''            "database/**",\n            ".github/workflows/**",\n'''
    if old_guard not in test:
        raise SystemExit("expected database forbidden guard not found")
    test_path.write_text(test.replace(old_guard, new_guard, 1), encoding="utf-8")
    return True


def _commit(root: Path) -> None:
    subprocess.run(["git", "config", "user.name", "github-actions[bot]"], cwd=root, check=True)
    subprocess.run(
        ["git", "config", "user.email", "41898282+github-actions[bot]@users.noreply.github.com"],
        cwd=root,
        check=True,
    )
    subprocess.run(
        ["git", "add", "docs/PHASE8_DELIVERY_PLAN.md", "tests/test_architecture_documentation_consistency.py"],
        cwd=root,
        check=True,
    )
    if subprocess.run(["git", "diff", "--cached", "--quiet"], cwd=root).returncode == 0:
        return
    subprocess.run(
        ["git", "commit", "-m", "Synchronize final Repository Step 17 continuation ledger"],
        cwd=root,
        check=True,
    )
    subprocess.run(["git", "push", "origin", f"HEAD:{BRANCH}"], cwd=root, check=True)


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    root = args.root.resolve()
    try:
        synchronized = args.write and _synchronize_final_ledger(root)
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
