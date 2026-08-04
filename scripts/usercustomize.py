"""Temporary diagnostic patch for the Step 18 evidence materializer."""

from __future__ import annotations

from pathlib import Path
import sys


if Path(sys.argv[0]).name == "temporary_step18_seed_demo_smoke_evidence_sync.py":
    path = Path(__file__).with_name("temporary_step18_seed_demo_smoke_evidence_sync.py")
    content = path.read_text(encoding="utf-8")
    old = '''    combined = "\\n".join(read(path) for path in NORMATIVE)
    for stale in (
        "seed-demo/smoke next",
        "next permitted bounded implementation packet is `seed-demo` and `smoke`",
        "Repository Step 19 remains blocked",
    ):
        if stale in combined:
            raise RuntimeError(f"stale Step 18 claim remains: {stale}")
'''
    new = '''    for path in NORMATIVE:
        document = read(path)
        for stale in (
            "seed-demo/smoke next",
            "next permitted bounded implementation packet is `seed-demo` and `smoke`",
            "Repository Step 19 remains blocked",
        ):
            if stale in document:
                raise RuntimeError(f"{path}: stale Step 18 claim remains: {stale}")
'''
    count = content.count(old)
    if count != 1:
        raise RuntimeError(f"verify diagnostic patch: expected 1 match, found {count}")
    path.write_text(content.replace(old, new, 1), encoding="utf-8")
