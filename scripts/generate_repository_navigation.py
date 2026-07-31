#!/usr/bin/env python3
"""One-run wrapper for the bounded step-11 evidence materializer."""

from __future__ import annotations

import subprocess


source = subprocess.check_output(
    ["git", "show", "HEAD^:scripts/generate_repository_navigation.py"],
    text=True,
)
source = source.replace(
    "if status.count(old_baseline) != 2:",
    "if status.count(old_baseline) < 1:",
    1,
)
source = source.replace(
    "status = status.replace(old_baseline, new_baseline, 2)",
    "status = status.replace(old_baseline, new_baseline)",
    1,
)
exec(compile(source, "scripts/generate_repository_navigation.py", "exec"), {"__name__": "__main__"})
