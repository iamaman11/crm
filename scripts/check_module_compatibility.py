#!/usr/bin/env python3
"""Reject module publication changes that violate immutable version semantics."""

from __future__ import annotations

import argparse
from dataclasses import dataclass
import re
import subprocess
import sys
from typing import Any

try:
    from .module_manifest_projection import runtime_manifest_projection
    from .validate_module_manifests import (
        ROOT,
        ManifestError,
        canonical_json_bytes,
        strict_yaml_load,
    )
except ImportError:  # direct script execution
    from module_manifest_projection import runtime_manifest_projection
    from validate_module_manifests import (
        ROOT,
        ManifestError,
        canonical_json_bytes,
        strict_yaml_load,
    )

MODULE_PATH_RE = re.compile(r"^modules/[^/]+/module\.yaml$")
SEMVER_RE = re.compile(
    r"^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)"
    r"(?:-[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?"
    r"(?:\+[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?$"
)


@dataclass(frozen=True, order=True)
class SemVerCore:
    major: int
    minor: int
    patch: int


def parse_semver_core(value: str) -> SemVerCore:
    match = SEMVER_RE.fullmatch(value)
    if match is None:
        raise ValueError(f"invalid semantic version: {value}")
    return SemVerCore(*(int(match.group(index)) for index in range(1, 4)))


def runtime_identity_bytes(manifest: dict[str, Any]) -> bytes:
    return canonical_json_bytes(runtime_manifest_projection(manifest))


def compare_manifest_sets(
    base: dict[str, dict[str, Any]],
    current: dict[str, dict[str, Any]],
) -> list[str]:
    errors: list[str] = []
    for module_id in sorted(base.keys() - current.keys()):
        errors.append(
            f"module {module_id} was removed; published module identities are immutable"
        )

    for module_id in sorted(base.keys() & current.keys()):
        previous = base[module_id]
        candidate = current[module_id]
        previous_version = str(previous.get("version", ""))
        candidate_version = str(candidate.get("version", ""))
        try:
            previous_core = parse_semver_core(previous_version)
            candidate_core = parse_semver_core(candidate_version)
        except ValueError as exc:
            errors.append(f"module {module_id}: {exc}")
            continue

        if candidate_core < previous_core:
            errors.append(
                f"module {module_id} version regressed from {previous_version} to {candidate_version}"
            )
            continue

        if (
            candidate_version == previous_version
            and runtime_identity_bytes(candidate) != runtime_identity_bytes(previous)
        ):
            errors.append(
                f"module {module_id}@{candidate_version} runtime manifest changed without a version bump"
            )
    return errors


def _run_git(*arguments: str) -> str:
    completed = subprocess.run(
        ["git", *arguments],
        cwd=ROOT,
        check=False,
        capture_output=True,
        text=True,
    )
    if completed.returncode != 0:
        raise RuntimeError(completed.stderr.strip() or "git command failed")
    return completed.stdout


def load_current_manifests() -> dict[str, dict[str, Any]]:
    manifests: dict[str, dict[str, Any]] = {}
    for path in sorted((ROOT / "modules").glob("*/module.yaml")):
        manifest = strict_yaml_load(path.read_text(encoding="utf-8"), str(path))
        module_id = str(manifest.get("module_id", ""))
        if not module_id or module_id in manifests:
            raise ManifestError(f"duplicate or missing module_id in {path}")
        manifests[module_id] = manifest
    return manifests


def load_git_manifests(ref: str) -> dict[str, dict[str, Any]]:
    paths = [
        line.strip()
        for line in _run_git(
            "ls-tree", "-r", "--name-only", ref, "--", "modules"
        ).splitlines()
        if MODULE_PATH_RE.fullmatch(line.strip())
    ]
    manifests: dict[str, dict[str, Any]] = {}
    for path in sorted(paths):
        text = _run_git("show", f"{ref}:{path}")
        manifest = strict_yaml_load(text, f"{ref}:{path}")
        module_id = str(manifest.get("module_id", ""))
        if not module_id or module_id in manifests:
            raise ManifestError(f"duplicate or missing module_id in {ref}:{path}")
        manifests[module_id] = manifest
    return manifests


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--base-ref", default="origin/main")
    args = parser.parse_args(argv)
    try:
        base = load_git_manifests(args.base_ref)
        current = load_current_manifests()
        errors = compare_manifest_sets(base, current)
    except (OSError, RuntimeError, ManifestError, ValueError) as exc:
        print(f"Module compatibility check could not run: {exc}", file=sys.stderr)
        return 2

    if errors:
        print("Module compatibility check FAILED:")
        for error in errors:
            print(f"- {error}")
        return 1
    print(
        f"Module compatibility check passed: {len(current)} current modules are compatible with {args.base_ref}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
