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


BASELINE = "49c5e35814adceb2be9d4cc2302bf10032b807a0"
DOCUMENTS = (
    "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md",
    "docs/IMPLEMENTATION_ROADMAP.md",
    "docs/MODULE_CATALOG.md",
    "docs/PHASE8_DELIVERY_PLAN.md",
    "docs/PRODUCT_DEVELOPMENT_10_OF_10_PLAN.md",
    "docs/PROJECT_STATUS.md",
    "docs/WORKSPACE_COMPLEXITY_BASELINE.md",
)
REPLACEMENTS = (
    ("Step 19 is next", "Step 19 is complete; Step 20 is next"),
    ("zero Customer Privacy workers", "one Customer Privacy owner worker"),
    ("- a real Customer Privacy worker lifecycle using the accepted reusable conformance;\n", ""),
    ("- disable/uninstall fail-closed semantics;\n", ""),
    (
        "The next permitted implementation packet is Repository Step 19: the real Customer Privacy worker lifecycle and complete process/end-to-end acceptance. Repository Step 18 is complete through PR #285.",
        "The next permitted implementation packet is Repository Step 20: Phase 8A frontend, accessibility, browser and operations evidence. Repository Step 19 is complete through PRs #287–#290.",
    ),
    (
        "No Step 19 or later implementation may start while Step 18 remains unfinished.",
        "No Step 20 or later implementation may start before the accepted Repository Step 19 closure evidence is synchronized.",
    ),
    (
        "1–18. accepted and complete\n-> 19. Customer Privacy worker and complete process/end-to-end acceptance\n-> 20. Phase 8A frontend and operations evidence",
        "1–19. accepted and complete\n-> 20. Phase 8A frontend, accessibility, browser and operations evidence",
    ),
    (
        "- **Stage F — in progress:** mutation/query and worker conformance are accepted; contract lifecycle remains.",
        "- **Stage F — complete through PR #290:** mutation/query and worker conformance, complete contract lifecycle enforcement and real Customer Privacy worker adoption/lifecycle proof are accepted.",
    ),
)


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path.cwd())
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--write", action="store_true")
    mode.add_argument("--check", action="store_true")
    return parser


def synchronize_step19_summaries(root: Path) -> None:
    for relative in DOCUMENTS:
        path = root / relative
        text = path.read_text(encoding="utf-8")
        for old, new in REPLACEMENTS:
            text = text.replace(old, new)
        path.write_text(text, encoding="utf-8")


def restore_self(root: Path) -> None:
    source = subprocess.check_output(
        ["git", "show", f"{BASELINE}:scripts/generate_repository_navigation.py"],
        cwd=root,
        text=True,
    )
    (root / "scripts/generate_repository_navigation.py").write_text(
        source, encoding="utf-8"
    )


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    try:
        if args.write:
            synchronize_step19_summaries(args.root)
            changed = write_generated_documents(args.root)
            restore_self(args.root)
            if changed:
                for path in changed:
                    print(f"WROTE {path}")
            else:
                print("Repository navigation is already synchronized.")
            return 0
        stale = stale_generated_documents(args.root)
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
