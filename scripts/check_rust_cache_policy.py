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


@dataclass(frozen=True)
class CachePolicyFailure:
    message: str


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
        "cache hit telemetry": (
            "steps.rust-cache-restore.outputs.cache-hit"
        ),
        "matched key telemetry": (
            "steps.rust-cache-restore.outputs.cache-matched-key"
        ),
    }
    for description, snippet in required.items():
        if snippet not in text:
            failures.append(CachePolicyFailure(f"missing {description}"))

    for cache_path in CACHE_PATHS:
        if cache_path not in text:
            failures.append(CachePolicyFailure(f"missing cache path {cache_path}"))

    forbidden_paths = (
        "~/.cargo/credentials",
        ".cargo/credentials",
        "~/.ssh",
        ".env",
    )
    for forbidden in forbidden_paths:
        if forbidden in text:
            failures.append(CachePolicyFailure(f"forbidden cache path {forbidden}"))

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
