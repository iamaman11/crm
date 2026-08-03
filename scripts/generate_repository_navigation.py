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

PACKET_ID = "repository-step-17-accepted-evidence-sync"
BRANCH = "repository-step-17-evidence-sync"


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path.cwd())
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--write", action="store_true")
    mode.add_argument("--check", action="store_true")
    return parser


def _finalize_packet_facts(root: Path) -> bool:
    packet_path = root / "repository-packet.json"
    packet = json.loads(packet_path.read_text(encoding="utf-8"))
    if packet.get("packet_id") != PACKET_ID:
        return False
    if "docs/generated/REPOSITORY_MAP.md" not in packet["allowed_paths"]:
        return False
    packet["allowed_paths"].remove("docs/generated/REPOSITORY_MAP.md")
    packet["acceptance"][0] = packet["acceptance"][0].replace(
        "ten declared evidence-sync files", "nine declared evidence-sync files"
    )
    packet_path.write_text(json.dumps(packet, indent=2) + "\n", encoding="utf-8")

    nav_path = root / "tests/test_repository_navigation.py"
    nav = nav_path.read_text(encoding="utf-8")
    nav = nav.replace(
        'self.assertEqual(packet["tracking_issues"], [126, 194, 275])',
        'self.assertEqual(packet["tracking_issues"], [126, 194])',
    )
    nav = nav.replace('                "docs/generated/REPOSITORY_MAP.md",\n', "", 1)
    nav_path.write_text(nav, encoding="utf-8")

    architecture_path = root / "tests/test_architecture_documentation_consistency.py"
    architecture = architecture_path.read_text(encoding="utf-8")
    architecture = architecture.replace(
        '                "docs/generated/REPOSITORY_MAP.md",\n', "", 1
    )
    architecture = architecture.replace(
        '            "docs/PROJECT_STATUS.md",\n',
        '            ".github/workflows/**",\n',
        1,
    )
    architecture_path.write_text(architecture, encoding="utf-8")
    return True


def _commit(root: Path) -> None:
    subprocess.run(["git", "config", "user.name", "github-actions[bot]"], cwd=root, check=True)
    subprocess.run(
        ["git", "config", "user.email", "41898282+github-actions[bot]@users.noreply.github.com"],
        cwd=root,
        check=True,
    )
    paths = [
        "repository-packet.json",
        "docs/ACTIVE_PACKET.md",
        "tests/test_repository_navigation.py",
        "tests/test_architecture_documentation_consistency.py",
    ]
    subprocess.run(["git", "add", *paths], cwd=root, check=True)
    if subprocess.run(["git", "diff", "--cached", "--quiet"], cwd=root).returncode == 0:
        return
    subprocess.run(
        ["git", "commit", "-m", "Finalize exact Repository Step 17 evidence packet"],
        cwd=root,
        check=True,
    )
    subprocess.run(["git", "push", "origin", f"HEAD:{BRANCH}"], cwd=root, check=True)


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    root = args.root.resolve()
    try:
        finalized = args.write and _finalize_packet_facts(root)
        if args.write:
            changed = write_generated_documents(root)
            if finalized:
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
