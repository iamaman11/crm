#!/usr/bin/env python3
"""Write or verify deterministic repository navigation documents."""

from __future__ import annotations

import argparse
from pathlib import Path
import sys

from _temporary_step13_architecture_sync import apply as apply_architecture_sync
from _temporary_step13_phase8_sync import apply as apply_phase8_sync
from _temporary_step13_roadmap_sync import apply as apply_roadmap_sync
from _temporary_step13_status_sync import apply as apply_status_sync
from repository_navigation import (
    NavigationError,
    stale_generated_documents,
    write_generated_documents,
)


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path.cwd())
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--write", action="store_true")
    mode.add_argument("--check", action="store_true")
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    try:
        if args.write:
            changed = [
                path
                for path in (
                    apply_status_sync(args.root),
                    apply_architecture_sync(args.root),
                    apply_roadmap_sync(args.root),
                    apply_phase8_sync(args.root),
                )
                if path is not None
            ]
            changed.extend(write_generated_documents(args.root))
            if changed:
                for path in changed:
                    print(f"WROTE {path}")
            else:
                print("Repository navigation is already synchronized.")
            return 0
        stale = stale_generated_documents(args.root)
    except (NavigationError, RuntimeError) as error:
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
