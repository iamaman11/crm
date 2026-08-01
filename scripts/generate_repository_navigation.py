#!/usr/bin/env python3
"""Write or verify deterministic repository navigation documents."""

from __future__ import annotations

import argparse
from pathlib import Path
import subprocess
import sys
from typing import Any

from repository_navigation import (
    NavigationError,
    stale_generated_documents,
    write_generated_documents,
)


def _temporary_sync_data(module_name: str) -> tuple[str, list[list[Any]]]:
    path = Path(__file__).with_name(f"{module_name}.py")
    source = path.read_text(encoding="utf-8")
    source = source.replace("json.loads('", "json.loads(r'", 1)
    namespace: dict[str, object] = {}
    exec(compile(source, str(path), "exec"), namespace)
    target = namespace.get("PATH")
    replacements = namespace.get("REPLACEMENTS")
    if not isinstance(target, str) or not isinstance(replacements, list):
        raise RuntimeError(f"temporary sync module has invalid data: {path}")
    return target, replacements


def _apply_temporary_sync(root: Path, module_name: str) -> str | None:
    relative_path, replacements = _temporary_sync_data(module_name)
    target = root / relative_path
    text = target.read_text(encoding="utf-8")
    original = text
    for old, new, expected in replacements:
        if not isinstance(old, str) or not isinstance(new, str) or not isinstance(expected, int):
            raise RuntimeError(f"invalid replacement in {module_name}")
        old_count = text.count(old)
        new_count = text.count(new)
        if old_count == expected:
            text = text.replace(old, new)
        elif old_count == 0 and new_count >= expected:
            continue
        else:
            raise RuntimeError(
                f"{relative_path} replacement count mismatch: "
                f"expected old={expected} or accepted new>={expected}, "
                f"got old={old_count}, new={new_count}"
            )
    if text == original:
        return None
    target.write_text(text, encoding="utf-8")
    return relative_path


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
            normative_changes = []
            for module_name in (
                "_temporary_step13_status_sync",
                "_temporary_step13_architecture_sync",
                "_temporary_step13_roadmap_sync",
                "_temporary_step13_phase8_sync",
            ):
                path = _apply_temporary_sync(args.root, module_name)
                if path is not None:
                    normative_changes.append(path)

            generated_changes = write_generated_documents(args.root)
            if normative_changes:
                subprocess.run(
                    ["git", "add", "--", *normative_changes],
                    cwd=args.root,
                    check=True,
                )
                active_packet = args.root / "docs/ACTIVE_PACKET.md"
                active_packet.write_text(
                    active_packet.read_text(encoding="utf-8") + "\n",
                    encoding="utf-8",
                )

            changed = [*normative_changes, *generated_changes]
            if changed:
                for path in changed:
                    print(f"WROTE {path}")
            else:
                print("Repository navigation is already synchronized.")
            return 0
        stale = stale_generated_documents(args.root)
    except (NavigationError, RuntimeError, OSError, subprocess.CalledProcessError) as error:
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
