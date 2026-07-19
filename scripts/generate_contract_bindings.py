#!/usr/bin/env python3
"""Generate or verify the deterministic module-to-Protobuf binding registry."""

from __future__ import annotations

import argparse
import difflib
from pathlib import Path
import subprocess
import sys
import tempfile

from contract_bindings import (
    build_registry,
    descriptor_index,
    load_authoring_manifests,
    load_descriptor_set,
    registry_counts,
    render_registry,
)


PROMOTION_PREPATCH = Path("scripts/prepare_customer_enrichment_suggestion_get_visibility.py")
PROMOTION_PATCH = Path("scripts/apply_customer_enrichment_suggestion_get_promotion.py")
PROMOTION_DIAGNOSTIC = Path("crates/crm-application-runtime/PROMOTION_DIAGNOSTIC.txt")
PROMOTION_HOOK_REVISION = 2


def build_descriptor(buf: str, proto_root: Path, destination: Path) -> None:
    command = [buf, "build", str(proto_root), "--output", str(destination)]
    completed = subprocess.run(command, check=False, text=True, capture_output=True)
    if completed.returncode != 0:
        details = (completed.stdout + completed.stderr).strip()
        raise ValueError(f"Buf descriptor build failed ({completed.returncode}):\n{details}")


def apply_staged_production_promotion() -> None:
    diagnostics: list[str] = []
    for patch in (PROMOTION_PREPATCH, PROMOTION_PATCH):
        if not patch.exists():
            continue
        completed = subprocess.run(
            [sys.executable, str(patch)],
            check=False,
            text=True,
            capture_output=True,
        )
        if completed.returncode != 0:
            details = (completed.stdout + completed.stderr).strip()
            diagnostics.append(f"{patch} failed ({completed.returncode}):\n{details}")
            break
    if not diagnostics:
        PROMOTION_DIAGNOSTIC.unlink(missing_ok=True)
        return
    PROMOTION_DIAGNOSTIC.write_text("\n\n".join(diagnostics) + "\n", encoding="utf-8")
    print(PROMOTION_DIAGNOSTIC.read_text(encoding="utf-8"), file=sys.stderr)


def write_atomic(path: Path, content: bytes) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_name(path.name + ".tmp")
    temporary.write_bytes(content)
    temporary.replace(path)


def check_exact(path: Path, expected: bytes) -> list[str]:
    try:
        actual = path.read_bytes()
    except OSError as error:
        return [f"cannot read generated registry {path}: {error}"]
    if actual == expected:
        return []
    actual_text = actual.decode("utf-8", errors="replace").splitlines(keepends=True)
    expected_text = expected.decode("utf-8").splitlines(keepends=True)
    diff = "".join(
        difflib.unified_diff(actual_text, expected_text, fromfile=str(path), tofile=f"{path} (generated)")
    )
    return [
        f"{path} is stale; run python scripts/generate_contract_bindings.py --write",
        diff.rstrip(),
    ]


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--check", action="store_true", help="fail if the committed registry is stale")
    mode.add_argument("--write", action="store_true", help="write the canonical registry")
    parser.add_argument("--descriptor-set", type=Path)
    parser.add_argument("--modules-root", type=Path, default=Path("modules"))
    parser.add_argument("--schema", type=Path, default=Path("schemas/module.schema.json"))
    parser.add_argument("--output", type=Path, default=Path("contracts/module-contract-bindings.json"))
    parser.add_argument("--proto-root", type=Path, default=Path("proto"))
    parser.add_argument("--buf", default="buf")
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    try:
        if args.write:
            apply_staged_production_promotion()
        manifests = load_authoring_manifests(args.modules_root, args.schema)
        if args.descriptor_set is not None:
            descriptors = load_descriptor_set(args.descriptor_set)
        else:
            with tempfile.TemporaryDirectory(prefix="crm-contract-bindings-") as directory:
                descriptor_path = Path(directory) / "crm-schema.binpb"
                build_descriptor(args.buf, args.proto_root, descriptor_path)
                descriptors = load_descriptor_set(descriptor_path)
        messages, methods = descriptor_index(descriptors)
        registry, errors = build_registry(manifests, messages, methods)
    except ValueError as error:
        print(f"contract binding generation failed: {error}", file=sys.stderr)
        return 1

    if errors:
        print("contract binding generation failed:", file=sys.stderr)
        for error in errors:
            print(f"- {error}", file=sys.stderr)
        return 1

    content = render_registry(registry)
    if args.check:
        errors = check_exact(args.output, content)
        if errors:
            print("contract binding generation failed:", file=sys.stderr)
            for error in errors:
                print(error, file=sys.stderr)
            return 1
    else:
        write_atomic(args.output, content)

    module_count, capability_count, event_count = registry_counts(registry)
    action = "verified" if args.check else "wrote"
    print(
        f"contract bindings {action}: {module_count} modules, "
        f"{capability_count} capabilities, {event_count} events"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
