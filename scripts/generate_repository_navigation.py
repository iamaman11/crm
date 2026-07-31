#!/usr/bin/env python3
"""One-run wrapper correcting the step-12 batch-1 packet-test signature."""

from __future__ import annotations

import subprocess


source = subprocess.check_output(
    ["git", "show", "HEAD^:scripts/generate_repository_navigation.py"],
    text=True,
)
old = '"test_packet_check_reports_affected_scope_without_running_git_or_cargo(self) -> None:",'
new = '"test_packet_check_reports_affected_scope_without_running_git_or_cargo(\\n        self,\\n    ) -> None:",'
if source.count(old) != 1:
    raise SystemExit(
        "step-12 batch-1 wrapper expected one packet-test signature, "
        f"found {source.count(old)}"
    )
source = source.replace(old, new, 1)
exec(
    compile(source, "scripts/generate_repository_navigation.py", "exec"),
    {"__name__": "__main__"},
)
