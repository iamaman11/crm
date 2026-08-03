#!/usr/bin/env python3
"""Generate or verify the deterministic contract lifecycle registry."""

from __future__ import annotations

import argparse
import difflib
import json
from pathlib import Path
import subprocess
import sys

from contract_bindings import load_authoring_manifests
from contract_lifecycle import (
    build_registry,
    load_json_object,
    registry_counts,
    render_registry,
)
from contract_lifecycle_transitions import validate_transition_integrity
from contract_retirement_evidence import validate_retirement_evidence


def write_atomic(path: Path, content: bytes) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_name(path.name + ".tmp")
    temporary.write_bytes(content)
    temporary.replace(path)


def check_exact(path: Path, expected: bytes) -> list[str]:
    try:
        actual = path.read_bytes()
    except OSError as error:
        return [f"cannot read generated lifecycle registry {path}: {error}"]
    if actual == expected:
        return []
    actual_lines = actual.decode("utf-8", errors="replace").splitlines(keepends=True)
    expected_lines = expected.decode("utf-8").splitlines(keepends=True)
    diff = "".join(
        difflib.unified_diff(
            actual_lines,
            expected_lines,
            fromfile=str(path),
            tofile=f"{path} (generated)",
        )
    )
    return [
        f"{path} is stale; run python scripts/generate_contract_lifecycle.py --write",
        diff.rstrip(),
    ]


def git_json(base_ref: str, path: Path, *, optional: bool = False) -> dict | None:
    completed = subprocess.run(
        ["git", "show", f"{base_ref}:{path.as_posix()}"],
        check=False,
        text=True,
        capture_output=True,
    )
    if completed.returncode != 0:
        details = (completed.stdout + completed.stderr).strip()
        missing_from_ref = (
            "exists on disk, but not in" in details
            or "does not exist in" in details
            or "Path '" in details and "does not exist" in details
        )
        if optional and missing_from_ref:
            return None
        raise ValueError(f"cannot read {path} from {base_ref}: {details}")
    try:
        value = json.loads(completed.stdout)
    except json.JSONDecodeError as error:
        raise ValueError(f"invalid JSON in {base_ref}:{path}: {error}") from error
    if not isinstance(value, dict):
        raise ValueError(f"{base_ref}:{path} must contain an object")
    return value


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--check", action="store_true", help="fail if the committed registry is stale")
    mode.add_argument("--write", action="store_true", help="write the canonical registry")
    parser.add_argument("--base-ref", help="enforce safe retirement against this git ref")
    parser.add_argument("--modules-root", type=Path, default=Path("modules"))
    parser.add_argument("--schema", type=Path, default=Path("schemas/module.schema.json"))
    parser.add_argument(
        "--bindings", type=Path, default=Path("contracts/module-contract-bindings.json")
    )
    parser.add_argument(
        "--policy", type=Path, default=Path("contracts/contract-lifecycle-policy.json")
    )
    parser.add_argument(
        "--retirement-evidence",
        type=Path,
        default=Path("contracts/contract-retirement-evidence.json"),
    )
    parser.add_argument(
        "--output", type=Path, default=Path("contracts/contract-lifecycle.json")
    )
    parser.add_argument(
        "--expected-output",
        type=Path,
        help="write the canonical expected bytes for CI diagnostics even when --check fails",
    )
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    try:
        manifests = load_authoring_manifests(args.modules_root, args.schema)
        bindings = load_json_object(args.bindings, "contract bindings")
        policy = load_json_object(args.policy, "contract lifecycle policy")
        retirement_evidence = load_json_object(
            args.retirement_evidence, "contract retirement evidence"
        )
        base_bindings = None
        base_policy = None
        base_retirement_evidence = None
        if args.base_ref:
            base_bindings = git_json(args.base_ref, args.bindings)
            base_policy = git_json(args.base_ref, args.policy, optional=True)
            base_retirement_evidence = git_json(
                args.base_ref, args.retirement_evidence, optional=True
            )
        registry, errors = build_registry(
            bindings,
            manifests,
            policy,
            base_bindings=base_bindings,
            base_policy=base_policy,
        )
        errors.extend(
            validate_transition_integrity(
                bindings,
                policy,
                base_bindings=base_bindings,
                base_policy=base_policy,
                repository_root=Path("."),
            )
        )
        errors.extend(
            validate_retirement_evidence(
                retirement_evidence,
                policy,
                base_evidence=base_retirement_evidence,
                base_policy=base_policy,
                repository_root=Path("."),
            )
        )
        errors = sorted(set(errors))
    except ValueError as error:
        print(f"contract lifecycle generation failed: {error}", file=sys.stderr)
        return 1

    content = render_registry(registry)
    if args.expected_output is not None:
        write_atomic(args.expected_output, content)

    if errors:
        print("contract lifecycle generation failed:", file=sys.stderr)
        for error in errors:
            print(f"- {error}", file=sys.stderr)
        return 1

    if args.check:
        check_errors = check_exact(args.output, content)
        if check_errors:
            print("contract lifecycle generation failed:", file=sys.stderr)
            for error in check_errors:
                print(error, file=sys.stderr)
            return 1
    else:
        write_atomic(args.output, content)

    total, active, deprecated, retired = registry_counts(registry)
    action = "verified" if args.check else "wrote"
    print(
        f"contract lifecycle {action}: {total} contracts, "
        f"{active} active, {deprecated} deprecated, {retired} retired"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
