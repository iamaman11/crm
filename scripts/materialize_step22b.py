#!/usr/bin/env python3
"""Retry the one-shot Step 22B materializer with one exact syntax correction."""

from __future__ import annotations

from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[1]
ORIGINAL_COMMIT = "2e0216433dbbe9f9e7f36733be922cc25be2b6d3"
ORIGINAL_PATH = "scripts/materialize_step22b.py"


def main() -> None:
    source = subprocess.run(
        ["git", "show", f"{ORIGINAL_COMMIT}:{ORIGINAL_PATH}"],
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
    ).stdout
    namespace: dict[str, object] = {
        "__name__": "materialize_step22b_impl",
        "__file__": str(ROOT / ORIGINAL_PATH),
    }
    exec(compile(source, ORIGINAL_PATH, "exec"), namespace)
    original_replace_once = namespace["replace_once"]

    def corrected_replace_once(text: str, old: str, new: str) -> str:
        if old == 'reasons=["Step 22A exact remeasurement inventory"],':
            old = '"reasons": ["Step 22A exact remeasurement inventory"],'
            new = '"reasons": ["Step 22B bounded runtime fan-in classifications"],'
        return original_replace_once(text, old, new)  # type: ignore[operator]

    namespace["replace_once"] = corrected_replace_once
    namespace["main"]()  # type: ignore[operator]


if __name__ == "__main__":
    main()
