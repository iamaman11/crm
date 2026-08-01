#!/usr/bin/env python3
"""Enforce the accepted repository-step-13 suppression multiset."""

from __future__ import annotations

import argparse
from collections import Counter
from datetime import date
import json
from pathlib import Path
import sys
from typing import Any

try:
    from analyze_step13_complexity import suppression_inventory
except ModuleNotFoundError:
    from scripts.analyze_step13_complexity import suppression_inventory

BASELINE_PATH = "step13-suppression-baseline.json"
SCHEMA_VERSION = "crm.step13-suppression-enforcement/v1"


def stable_key(entry: dict[str, Any]) -> tuple[str, str, str]:
    return (entry["kind"], entry["path"], entry["detail"])


def registration_key(entry: dict[str, Any]) -> tuple[str, str, str]:
    return (entry["k"], entry["p"], entry["d"])


def load_baseline(root: Path) -> dict[str, Any]:
    with (root / BASELINE_PATH).open(encoding="utf-8") as handle:
        return json.load(handle)


def evaluate(root: Path, *, today: date | None = None) -> dict[str, Any]:
    today = today or date.today()
    baseline = load_baseline(root)
    inventory = suppression_inventory(root)
    current = Counter(stable_key(entry) for entry in inventory["entries"])
    registered = {
        registration_key(entry): int(entry["n"])
        for entry in baseline["registrations"]
    }

    unregistered = [
        {"kind": kind, "path": path, "detail": detail, "count": count}
        for (kind, path, detail), count in sorted(current.items())
        if (kind, path, detail) not in registered
    ]
    growth = [
        {
            "kind": kind,
            "path": path,
            "detail": detail,
            "registered": registered[(kind, path, detail)],
            "current": count,
        }
        for (kind, path, detail), count in sorted(current.items())
        if (kind, path, detail) in registered
        and count > registered[(kind, path, detail)]
    ]
    reductions = [
        {
            "kind": kind,
            "path": path,
            "detail": detail,
            "registered": maximum,
            "current": current.get((kind, path, detail), 0),
        }
        for (kind, path, detail), maximum in sorted(registered.items())
        if current.get((kind, path, detail), 0) < maximum
    ]

    expired = []
    for registration in baseline["registrations"]:
        review_date = registration.get("x")
        if review_date and date.fromisoformat(review_date) < today:
            expired.append(
                {
                    "kind": registration["k"],
                    "path": registration["p"],
                    "detail": registration["d"],
                    "review_date": review_date,
                }
            )

    direct_lint_count = inventory["counts_by_kind"].get("direct-lint-table", 0)
    required_direct_lint_count = int(
        baseline["enforcement"]["required_current_direct_lint_table_count"]
    )
    blockers = []
    if unregistered:
        blockers.append("unregistered suppression keys detected")
    if growth:
        blockers.append("registered suppression occurrence growth detected")
    if expired:
        blockers.append("expired suppression registrations detected")
    if direct_lint_count != required_direct_lint_count:
        blockers.append(
            "direct lint table count does not match the enforced current target"
        )

    return {
        "schema_version": SCHEMA_VERSION,
        "accepted_evidence": baseline["accepted_evidence"],
        "current_entry_count": inventory["entry_count"],
        "current_stable_key_count": len(current),
        "current_counts_by_kind": inventory["counts_by_kind"],
        "registered_stable_key_count": len(registered),
        "registered_occurrence_ceiling": sum(registered.values()),
        "direct_lint_table_count": direct_lint_count,
        "required_direct_lint_table_count": required_direct_lint_count,
        "unregistered": unregistered,
        "growth": growth,
        "reductions": reductions,
        "expired": expired,
        "blockers": blockers,
        "ok": not blockers,
    }


def markdown(report: dict[str, Any]) -> str:
    lines = [
        "# Step 13 Suppression Enforcement",
        "",
        f"- Status: **{'PASS' if report['ok'] else 'FAIL'}**",
        f"- Current entries: {report['current_entry_count']}",
        f"- Current stable keys: {report['current_stable_key_count']}",
        f"- Registered stable keys: {report['registered_stable_key_count']}",
        f"- Registered occurrence ceiling: {report['registered_occurrence_ceiling']}",
        f"- Direct lint tables: {report['direct_lint_table_count']} / required {report['required_direct_lint_table_count']}",
        f"- Reductions from accepted baseline: {len(report['reductions'])}",
        f"- Unregistered keys: {len(report['unregistered'])}",
        f"- Occurrence growth: {len(report['growth'])}",
        f"- Expired registrations: {len(report['expired'])}",
        "",
    ]
    if report["blockers"]:
        lines.extend(["## Blockers", ""])
        lines.extend(f"- {blocker}" for blocker in report["blockers"])
        lines.append("")
    return "\n".join(lines)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path.cwd())
    parser.add_argument("--json-output", type=Path)
    parser.add_argument("--markdown-output", type=Path)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    try:
        report = evaluate(args.root.resolve())
    except (OSError, ValueError, KeyError, json.JSONDecodeError) as error:
        print(f"step-13 suppression enforcement failed: {error}", file=sys.stderr)
        return 1

    json_text = json.dumps(report, indent=2, sort_keys=True) + "\n"
    markdown_text = markdown(report)
    if args.json_output:
        args.json_output.parent.mkdir(parents=True, exist_ok=True)
        args.json_output.write_text(json_text, encoding="utf-8")
    else:
        print(json_text, end="")
    if args.markdown_output:
        args.markdown_output.parent.mkdir(parents=True, exist_ok=True)
        args.markdown_output.write_text(markdown_text, encoding="utf-8")
    if args.check and not report["ok"]:
        print(markdown_text, file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
