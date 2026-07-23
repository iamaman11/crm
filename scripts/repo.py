#!/usr/bin/env python3
"""Stable cross-platform repository commands for local contributors and coding agents."""

from __future__ import annotations

import argparse
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]


class CommandError(RuntimeError):
    """Raised when a repository command fails."""


def run(command: list[str]) -> None:
    print("+", " ".join(command), flush=True)
    completed = subprocess.run(command, cwd=ROOT, check=False)
    if completed.returncode != 0:
        raise CommandError(
            f"command failed with exit code {completed.returncode}: {' '.join(command)}"
        )


def command_architecture(_: argparse.Namespace) -> None:
    run([sys.executable, "scripts/check_architecture.py"])


def command_manifests(_: argparse.Namespace) -> None:
    run([sys.executable, "scripts/validate_module_manifests.py"])
    run(
        [
            sys.executable,
            "scripts/compile_module_manifest_ir.py",
            "--output-dir",
            "build/module-ir",
        ]
    )
    ir_paths = sorted((ROOT / "build/module-ir").glob("*.json"))
    if not ir_paths:
        raise CommandError("manifest IR compiler produced no JSON files")
    run(
        [
            "cargo",
            "run",
            "--quiet",
            "-p",
            "crm-module-manifest",
            "--bin",
            "validate-module-manifest",
            "--",
            *(str(path.relative_to(ROOT)) for path in ir_paths),
        ]
    )


def command_contracts(args: argparse.Namespace) -> None:
    mode = "--write" if args.write else "--check"
    run([sys.executable, "scripts/generate_contract_bindings.py", mode])


def command_conformance(_: argparse.Namespace) -> None:
    """Run the permanent native modular-architecture preflight."""
    command_architecture(argparse.Namespace())
    command_manifests(argparse.Namespace())
    command_contracts(argparse.Namespace(write=False))
    run([sys.executable, "scripts/check_native_module_composition.py"])
    run([sys.executable, "scripts/check_production_route_classifications.py"])
    run(
        [
            sys.executable,
            "-m",
            "unittest",
            "tests/test_contract_bindings.py",
            "tests/test_customer_privacy_architecture_freeze.py",
            "tests/test_customer_privacy_contract_inventory.py",
            "tests/test_customer_privacy_owner_scope_contracts.py",
            "tests/test_module_compatibility.py",
            "tests/test_module_manifest_validation.py",
            "tests/test_module_scaffolding.py",
            "tests/test_native_module_composition.py",
            "tests/test_production_route_classifications.py",
        ]
    )
    run(
        [
            "cargo",
            "test",
            "-p",
            "crm-application-runtime",
            "--test",
            "production_route_parity",
            "--all-features",
        ]
    )


def command_format(args: argparse.Namespace) -> None:
    command = ["cargo", "fmt", "--all"]
    if args.check:
        command.extend(["--", "--check"])
    run(command)


def command_lock(_: argparse.Namespace) -> None:
    run(["cargo", "generate-lockfile"])


def command_test(args: argparse.Namespace) -> None:
    command = ["cargo", "test", "-p", args.package, "--all-features"]
    if args.test_target:
        command.extend(["--test", args.test_target])

    passthrough = list(args.passthrough)
    if passthrough[:1] == ["--"]:
        passthrough = passthrough[1:]
    if passthrough:
        command.append("--")
        command.extend(passthrough)
    run(command)


def command_test_all(_: argparse.Namespace) -> None:
    run(["cargo", "test", "--workspace", "--all-features"])


def command_quality(_: argparse.Namespace) -> None:
    command_conformance(argparse.Namespace())
    command_format(argparse.Namespace(check=True))
    run(
        [
            "cargo",
            "clippy",
            "--workspace",
            "--all-targets",
            "--all-features",
            "--",
            "-D",
            "warnings",
        ]
    )
    command_test_all(argparse.Namespace())


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Run stable Ultimate CRM repository commands."
    )
    subparsers = parser.add_subparsers(dest="command", required=True)

    architecture = subparsers.add_parser(
        "architecture", help="enforce repository dependency/source boundaries"
    )
    architecture.set_defaults(handler=command_architecture)

    manifests = subparsers.add_parser(
        "manifests", help="validate manifests and Rust normalized-IR parity"
    )
    manifests.set_defaults(handler=command_manifests)

    contracts = subparsers.add_parser(
        "contracts", help="verify or regenerate module-to-Protobuf contract bindings"
    )
    contracts.add_argument(
        "--write", action="store_true", help="write the canonical generated registry"
    )
    contracts.set_defaults(handler=command_contracts)

    conformance = subparsers.add_parser(
        "conformance",
        help="run native composition, manifest, contract, scaffold and route-parity gates",
    )
    conformance.set_defaults(handler=command_conformance)

    fmt = subparsers.add_parser(
        "format", help="format Rust sources or check formatting"
    )
    fmt.add_argument("--check", action="store_true")
    fmt.set_defaults(handler=command_format)

    lock = subparsers.add_parser(
        "lock", help="regenerate the committed Cargo lockfile"
    )
    lock.set_defaults(handler=command_lock)

    test = subparsers.add_parser(
        "test", help="run one package or one package integration test"
    )
    test.add_argument("--package", "-p", required=True)
    test.add_argument("--test-target")
    test.add_argument("passthrough", nargs=argparse.REMAINDER)
    test.set_defaults(handler=command_test)

    test_all = subparsers.add_parser(
        "test-all", help="run the full Rust workspace test suite"
    )
    test_all.set_defaults(handler=command_test_all)

    quality = subparsers.add_parser(
        "quality", help="run conformance, formatting, Clippy and all tests"
    )
    quality.set_defaults(handler=command_quality)

    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    try:
        args.handler(args)
    except CommandError as error:
        print(f"ERROR: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
