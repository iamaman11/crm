"""Temporary guard for literal multiline replacements in the Step 18 verifier."""

from __future__ import annotations

import re
from typing import Any

_original_subn = re.subn


def _literal_step18_subn(
    pattern: Any,
    repl: Any,
    string: str,
    count: int = 0,
    flags: int = 0,
):
    text = pattern.pattern if hasattr(pattern, "pattern") else str(pattern)
    literal = isinstance(repl, str) and (
        "def wait_ready" in text or "def _initialize" in text
    )
    if literal:
        return _original_subn(
            pattern,
            lambda _: repl,
            string,
            count=count,
            flags=flags,
        )
    return _original_subn(pattern, repl, string, count=count, flags=flags)


re.subn = _literal_step18_subn
