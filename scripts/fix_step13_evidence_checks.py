#!/usr/bin/env python3
"""Temporarily synchronize packet-contract fixtures with repository-packet.json."""

from __future__ import annotations

import json
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[1]


def python_list(values: list[str], indent: int = 0) -> str:
    prefix = " " * indent
    child = " " * (indent + 4)
    lines = ["["]
    lines.extend(f'{child}{json.dumps(value)},' for value in values)
    lines.append(prefix + "]")
    return "\n".join(lines)


def replace_once(text: str, pattern: str, replacement: str, label: str) -> str:
    updated, count = re.subn(pattern, replacement, text, count=1, flags=re.DOTALL)
    if count != 1:
        raise RuntimeError(f"could not synchronize {label}: found {count} matches")
    return updated


def synchronize_common(path: Path, allowed: list[str], checks: list[str]) -> str:
    text = path.read_text(encoding="utf-8")
    text = replace_once(
        text,
        r"ALLOWED_PACKET_PATHS = \[.*?\]\n\n",
        "ALLOWED_PACKET_PATHS = " + python_list(allowed) + "\n\n",
        f"allowed paths in {path}",
    )
    text = replace_once(
        text,
        r'packet\["required_checks"\],\n\s*\[.*?\]\n\s*,?\n\s*\)',
        'packet["required_checks"],\n            ' + python_list(checks, 12) + "\n        )",
        f"required checks in {path}",
    )
    return text


def synchronize_navigation(allowed: list[str], checks: list[str]) -> None:
    path = ROOT / "tests/test_repository_navigation.py"
    text = synchronize_common(path, allowed, checks)
    workflow_paths = {
        "Affected Scope CI": ".github/workflows/affected-scope.yml",
        "Complexity Baseline CI": ".github/workflows/complexity-baseline.yml",
        "Customer Privacy Access Export CI": ".github/workflows/customer-privacy-access-export.yml",
        "Customer Privacy Owner Execution CI": ".github/workflows/customer-privacy-owner-execution.yml",
        "Governance CI": ".github/workflows/governance.yml",
        "Rust CI": ".github/workflows/rust.yml",
        "Rust Generated Sync": ".github/workflows/rust-generated-sync.yml",
    }
    tuple_lines = ["                for name, path in ("]
    for name in checks:
        tuple_lines.append(
            f'                    ({json.dumps(name)}, {json.dumps(workflow_paths[name])}),' 
        )
    tuple_lines.append("                )")
    text = replace_once(
        text,
        r"\s{16}for name, path in \(.*?\s{16}\)",
        "\n".join(tuple_lines),
        "selected workflow fixture",
    )
    path.write_text(text, encoding="utf-8")


def synchronize_consistency(allowed: list[str], checks: list[str]) -> None:
    path = ROOT / "tests/test_architecture_documentation_consistency.py"
    text = synchronize_common(path, allowed, checks)
    path.write_text(text, encoding="utf-8")


def main() -> int:
    packet = json.loads((ROOT / "repository-packet.json").read_text(encoding="utf-8"))
    synchronize_navigation(packet["allowed_paths"], packet["required_checks"])
    synchronize_consistency(packet["allowed_paths"], packet["required_checks"])
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
