#!/usr/bin/env python3
"""Enforce the trust and keying rules for the Rust CI cache pilot."""

from __future__ import annotations

import argparse
from dataclasses import dataclass
from pathlib import Path
import sys

CACHE_ACTION_SHA = "cdf6c1fa76f9f475f3d7449005a359c84ca0f306"
CACHE_KEY = (
    "rust-quality-v1-${{ runner.os }}-${{ runner.arch }}-"
    "${{ steps.rust-cache-identity.outputs.toolchain }}-${{ hashFiles('Cargo.lock') }}"
)
RESTORE_PREFIX = (
    "rust-quality-v1-${{ runner.os }}-${{ runner.arch }}-"
    "${{ steps.rust-cache-identity.outputs.toolchain }}-"
)
SAVE_CONDITION = (
    "if: github.event_name == 'push' && github.ref == 'refs/heads/main' && "
    "success() && steps.rust-cache-restore.outputs.cache-hit != 'true'"
)
CACHE_PATHS = (
    "~/.cargo/registry/index/",
    "~/.cargo/registry/cache/",
    "~/.cargo/git/db/",
    "target/",
)
FORBIDDEN_CACHE_PATHS = (
    "~/.cargo/credentials",
    ".cargo/credentials",
    "~/.ssh",
    ".env",
)


@dataclass(frozen=True)
class CachePolicyFailure:
    message: str


def indentation(line: str) -> int:
    return len(line) - len(line.lstrip(" "))


def cache_path_blocks(text: str) -> tuple[tuple[str, ...], ...]:
    """Return path entries from each split cache action, excluding unrelated YAML/code."""

    lines = text.splitlines()
    blocks: list[tuple[str, ...]] = []
    for uses_index, line in enumerate(lines):
        if "uses: actions/cache/restore@" not in line and "uses: actions/cache/save@" not in line:
            continue

        uses_indent = indentation(line)
        step_indent = max(0, uses_indent - 2)
        step_end = uses_index + 1
        while step_end < len(lines):
            candidate = lines[step_end]
            if candidate.strip() and indentation(candidate) == step_indent and candidate.lstrip().startswith("- "):
                break
            step_end += 1

        path_index = None
        for index in range(uses_index + 1, step_end):
            if lines[index].strip() == "path: |":
                path_index = index
                break
        if path_index is None:
            blocks.append(())
            continue

        path_indent = indentation(lines[path_index])
        entries: list[str] = []
        for index in range(path_index + 1, step_end):
            candidate = lines[index]
            if candidate.strip() and indentation(candidate) <= path_indent:
                break
            value = candidate.strip().strip('"').strip("'")
            if value:
                entries.append(value)
        blocks.append(tuple(entries))

    return tuple(blocks)


def check_rust_cache_policy(path: Path) -> tuple[CachePolicyFailure, ...]:
    text = path.read_text(encoding="utf-8")
    failures: list[CachePolicyFailure] = []

    required = {
        "toolchain-derived cache identity": "id: rust-cache-identity",
        "restore step id": "id: rust-cache-restore",
        "immutable restore Action": (
            f"uses: actions/cache/restore@{CACHE_ACTION_SHA} # v5.0.3"
        ),
        "immutable save Action": f"uses: actions/cache/save@{CACHE_ACTION_SHA} # v5.0.3",
        "Cargo.lock-bound primary key": f"key: {CACHE_KEY}",
        "same-toolchain restore prefix": f"{RESTORE_PREFIX}\n",
        "main-only successful save condition": SAVE_CONDITION,
        "save key derived from restore": (
            "key: ${{ steps.rust-cache-restore.outputs.cache-primary-key }}"
        ),
        "cache hit telemetry": "steps.rust-cache-restore.outputs.cache-hit",
        "matched key telemetry": "steps.rust-cache-restore.outputs.cache-matched-key",
    }
    for description, snippet in required.items():
        if snippet not in text:
            failures.append(CachePolicyFailure(f"missing {description}"))

    path_blocks = cache_path_blocks(text)
    if len(path_blocks) != 2:
        failures.append(CachePolicyFailure("Rust CI must define exactly two cache path blocks"))
    for index, entries in enumerate(path_blocks, start=1):
        if entries != CACHE_PATHS:
            failures.append(
                CachePolicyFailure(
                    f"cache path block {index} must exactly match the approved path set"
                )
            )
        for entry in entries:
            if any(forbidden in entry for forbidden in FORBIDDEN_CACHE_PATHS):
                failures.append(CachePolicyFailure(f"forbidden cache path {entry}"))

    ordered_markers = (
        "- name: Resolve Rust cache identity",
        "- name: Restore trusted Rust CI cache",
        "- name: Run Clippy",
        "- name: Run workspace tests",
        "- name: Save trusted Rust CI cache",
    )
    positions = [text.find(marker) for marker in ordered_markers]
    if any(position < 0 for position in positions):
        failures.append(CachePolicyFailure("cache/quality step order cannot be evaluated"))
    elif positions != sorted(positions):
        failures.append(
            CachePolicyFailure(
                "restore must precede Clippy/tests and trusted save must follow all quality steps"
            )
        )

    if text.count("actions/cache/save@") != 1:
        failures.append(CachePolicyFailure("Rust CI must contain exactly one cache save action"))
    if text.count("actions/cache/restore@") != 1:
        failures.append(CachePolicyFailure("Rust CI must contain exactly one cache restore action"))

    return tuple(failures)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path.cwd())
    args = parser.parse_args()

    path = args.root / ".github" / "workflows" / "rust.yml"
    failures = check_rust_cache_policy(path)
    if failures:
        for failure in failures:
            print(f"{path.relative_to(args.root)}: {failure.message}", file=sys.stderr)
        return 1

    print("Rust CI cache policy is trusted, immutable and main-write-only.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
