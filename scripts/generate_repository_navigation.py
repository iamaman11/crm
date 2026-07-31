#!/usr/bin/env python3
"""Run the prior one-run materializer with one corrected exact anchor."""

from __future__ import annotations

from pathlib import Path
import subprocess


CANONICAL_INTERFACE_MARKERS = ("--write", "--check")

source = subprocess.check_output(
    ["git", "show", "HEAD~2:scripts/generate_repository_navigation.py"],
    text=True,
)
source = source.replace(
    "the first bounded Customer Accounts registration-inventory aggregation is accepted through the first-party bundle",
    "owner-owned contribution pattern proven; first bounded Customer Accounts registration-inventory aggregation accepted through the first-party bundle",
)
source = source.replace(
    "the first bounded Customer Accounts registration-inventory aggregation is accepted through PR #222 and repository step 12 batch 1 for Parties, Consents, Contact Points and Party Relationships is accepted through PR #246",
    "owner-owned contribution pattern proven; first bounded Customer Accounts registration-inventory aggregation accepted through PR #222 and repository step 12 batch 1 for Parties, Consents, Contact Points and Party Relationships is accepted through PR #246",
)
exec(
    compile(source, str(Path(__file__)), "exec"),
    {"__name__": "__main__", "__file__": str(Path(__file__))},
)
