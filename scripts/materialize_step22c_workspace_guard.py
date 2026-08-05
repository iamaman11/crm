#!/usr/bin/env python3
"""Retry the Step 22C materializer with literal regex replacement semantics."""

from __future__ import annotations

from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[1]
ORIGINAL_COMMIT = "a503c056e0fa42a6bc5804d7976276694a3b1c2e"
ORIGINAL_PATH = "scripts/materialize_step22c_workspace_guard.py"


def main() -> None:
    source = subprocess.run(
        ["git", "show", f"{ORIGINAL_COMMIT}:{ORIGINAL_PATH}"],
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
    ).stdout
    old = "updated, count = pattern.subn(NEW_METHOD, text)"
    new = "updated, count = pattern.subn(lambda _match: NEW_METHOD, text)"
    if source.count(old) != 1:
        raise RuntimeError("original materializer substitution contract changed")
    source = source.replace(old, new, 1)
    namespace: dict[str, object] = {
        "__name__": "materialize_step22c_workspace_guard_impl",
        "__file__": str(ROOT / ORIGINAL_PATH),
    }
    exec(compile(source, ORIGINAL_PATH, "exec"), namespace)
    namespace["main"]()  # type: ignore[operator]


if __name__ == "__main__":
    main()
