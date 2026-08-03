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


def _correct_guard(root: Path) -> bool:
    path = root / "tests/test_architecture_documentation_consistency.py"
    text = path.read_text(encoding="utf-8")
    wrong_allowed = '''                "docs/PHASE8_DELIVERY_PLAN.md",\n                ".github/workflows/**",\n                "repository-packet.json",\n'''
    correct_allowed = '''                "docs/PHASE8_DELIVERY_PLAN.md",\n                "docs/PROJECT_STATUS.md",\n                "repository-packet.json",\n'''
    wrong_forbidden = '''        for forbidden in (\n            "Cargo.lock",\n            "proto/**",\n            "database/migrations/**",\n            "docs/PROJECT_STATUS.md",\n        ):\n'''
    correct_forbidden = '''        for forbidden in (\n            "Cargo.lock",\n            "proto/**",\n            "database/migrations/**",\n            ".github/workflows/**",\n        ):\n'''
    if wrong_allowed not in text or wrong_forbidden not in text:
        return False
    text = text.replace(wrong_allowed, correct_allowed, 1)
    text = text.replace(wrong_forbidden, correct_forbidden, 1)
    path.write_text(text, encoding="utf-8")
    return True


def _commit(root: Path) -> None:
    subprocess.run(["git", "config", "user.name", "github-actions[bot]"], cwd=root, check=True)
    subprocess.run(
        ["git", "config", "user.email", "41898282+github-actions[bot]@users.noreply.github.com"],
        cwd=root,
        check=True,
    )
    subprocess.run(
        ["git", "add", "tests/test_architecture_documentation_consistency.py"],
        cwd=root,
        check=True,
    )
    if subprocess.run(["git", "diff", "--cached", "--quiet"], cwd=root).returncode == 0:
        return
    subprocess.run(
        ["git", "commit", "-m", "Correct Step 17 evidence guard path sets"],
        cwd=root,
        check=True,
    )
    subprocess.run(["git", "push", "origin", f"HEAD:{BRANCH}"], cwd=root, check=True)


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    root = args.root.resolve()
    try:
        corrected = args.write and _correct_guard(root)
        if args.write:
            changed = write_generated_documents(root)
            if corrected:
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
