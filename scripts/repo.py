#!/usr/bin/env python3
"""Stable cross-platform repository commands for local contributors and coding agents."""

from __future__ import annotations

import argparse
import json
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


def affected_report(base_ref: str) -> dict:
    from affected_scope import build_report

    try:
        return build_report(ROOT, base_ref)
    except RuntimeError as error:
        raise CommandError(str(error)) from error


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
    run([sys.executable, "scripts/generate_repository_navigation.py", "--check"])
    run([sys.executable, "scripts/check_native_module_composition.py"])
    run([sys.executable, "scripts/check_production_route_classifications.py"])
    run(
        [
            sys.executable,
            "-m",
            "unittest",
            "tests/test_architecture_documentation_consistency.py",
            "tests/test_contract_bindings.py",
            "tests/test_customer_privacy_architecture_freeze.py",
            "tests/test_customer_privacy_contract_inventory.py",
            "tests/test_customer_privacy_owner_scope_contracts.py",
            "tests/test_local_dev.py",
            "tests/test_local_lifecycle.py",
            "tests/test_module_compatibility.py",
            "tests/test_module_manifest_validation.py",
            "tests/test_module_scaffolding.py",
            "tests/test_native_module_composition.py",
            "tests/test_production_route_classifications.py",
            "tests/test_repository_navigation.py",
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


def command_affected(args: argparse.Namespace) -> None:
    from affected_scope import markdown_report

    report = affected_report(args.base)
    if args.json:
        print(json.dumps(report, indent=2, sort_keys=True))
    else:
        print(markdown_report(report), end="")


def command_explain(args: argparse.Namespace) -> None:
    from repository_navigation import NavigationError, explain_target, render_explanation

    try:
        explanation = explain_target(ROOT, args.target)
    except NavigationError as error:
        raise CommandError(str(error)) from error
    if args.json:
        print(json.dumps(explanation, indent=2, sort_keys=True))
    else:
        print(render_explanation(explanation), end="")


def command_packet_check(args: argparse.Namespace) -> None:
    from repository_navigation import (
        NavigationError,
        packet_check,
        render_packet_check,
        write_generated_documents,
    )

    try:
        if args.write_generated:
            changed = write_generated_documents(ROOT)
            for path in changed:
                print(f"WROTE {path}", flush=True)
        report = packet_check(ROOT, args.base)
    except (NavigationError, RuntimeError) as error:
        raise CommandError(str(error)) from error
    if args.json:
        print(json.dumps(report, indent=2, sort_keys=True))
    else:
        print(render_packet_check(report), end="")
    if not report["ok"]:
        raise CommandError("active repository packet check failed")


def command_doctor(args: argparse.Namespace) -> None:
    try:
        from local_lifecycle import LifecycleError, doctor, render_doctor
    except ModuleNotFoundError:
        from scripts.local_lifecycle import LifecycleError, doctor, render_doctor

    try:
        report = doctor(ROOT, profile=args.profile)
    except LifecycleError as error:
        raise CommandError(str(error)) from error
    if args.json:
        print(json.dumps(report, indent=2, sort_keys=True))
    else:
        print(render_doctor(report), end="")
    if not report["ok"]:
        raise CommandError("local doctor found blocking prerequisites")


def command_bootstrap(args: argparse.Namespace) -> None:
    try:
        from local_lifecycle import LifecycleError, bootstrap, render_bootstrap
    except ModuleNotFoundError:
        from scripts.local_lifecycle import LifecycleError, bootstrap, render_bootstrap

    try:
        report = bootstrap(ROOT, dry_run=args.dry_run)
    except LifecycleError as error:
        raise CommandError(str(error)) from error
    if args.json:
        print(json.dumps(report, indent=2, sort_keys=True))
    else:
        print(render_bootstrap(report), end="")


def command_dev_up(args: argparse.Namespace) -> None:
    try:
        from local_dev import LifecycleError, dev_up, render_dev
    except ModuleNotFoundError:
        from scripts.local_dev import LifecycleError, dev_up, render_dev

    try:
        report = dev_up(ROOT, dry_run=args.dry_run)
    except LifecycleError as error:
        raise CommandError(str(error)) from error
    if args.json:
        print(json.dumps(report, indent=2, sort_keys=True))
    else:
        print(render_dev(report), end="")


def command_dev_reset(args: argparse.Namespace) -> None:
    try:
        from local_dev import LifecycleError, dev_reset, render_dev
    except ModuleNotFoundError:
        from scripts.local_dev import LifecycleError, dev_reset, render_dev

    try:
        report = dev_reset(ROOT, dry_run=args.dry_run)
    except LifecycleError as error:
        raise CommandError(str(error)) from error
    if args.json:
        print(json.dumps(report, indent=2, sort_keys=True))
    else:
        print(render_dev(report), end="")


def affected_clippy_command(report: dict) -> list[str] | None:
    packages = report["affected_packages"]
    if not packages:
        return None
    if report["broadened"]:
        return [
            "cargo",
            "clippy",
            "--workspace",
            "--all-targets",
            "--all-features",
            "--",
            "-D",
            "warnings",
        ]
    command = ["cargo", "clippy"]
    for package in packages:
        command.extend(["-p", package])
    command.extend(["--all-targets", "--all-features", "--", "-D", "warnings"])
    return command


def affected_test_command(report: dict) -> list[str] | None:
    packages = report["affected_packages"]
    if not packages:
        return None
    if report["broadened"]:
        return ["cargo", "test", "--workspace", "--all-features"]
    command = ["cargo", "test"]
    for package in packages:
        command.extend(["-p", package])
    command.append("--all-features")
    return command


def command_check_affected(args: argparse.Namespace) -> None:
    from affected_scope import markdown_report

    report = affected_report(args.base)
    print(markdown_report(report), end="", flush=True)

    if args.phase in {"all", "structural"}:
        command_conformance(argparse.Namespace())
        command_format(argparse.Namespace(check=True))
    if args.phase == "structural":
        return

    if args.phase in {"all", "clippy"}:
        clippy = affected_clippy_command(report)
        if clippy is None:
            print("No Rust packages are affected; package Clippy is skipped.", flush=True)
        else:
            run(clippy)
    if args.phase == "clippy":
        return

    if args.phase in {"all", "test"}:
        tests = affected_test_command(report)
        if tests is None:
            print("No Rust packages are affected; package tests are skipped.", flush=True)
        else:
            run(tests)


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

    affected = subparsers.add_parser(
        "affected",
        help="explain changed paths, reverse package impact and workflow selection",
    )
    affected.add_argument("--base", default="origin/main")
    affected.add_argument("--json", action="store_true")
    affected.set_defaults(handler=command_affected)

    explain = subparsers.add_parser(
        "explain",
        help="trace one exact module or capability through repository ownership and evidence",
    )
    explain.add_argument("target")
    explain.add_argument("--json", action="store_true")
    explain.set_defaults(handler=command_explain)

    packet_check_parser = subparsers.add_parser(
        "packet-check",
        help="validate the active packet, changed paths, affected closure and navigation freshness",
    )
    packet_check_parser.add_argument("--base", default="origin/main")
    packet_check_parser.add_argument("--json", action="store_true")
    packet_check_parser.add_argument(
        "--write-generated",
        action="store_true",
        help="write generated navigation before checking the packet",
    )
    packet_check_parser.set_defaults(handler=command_packet_check)

    doctor_parser = subparsers.add_parser(
        "doctor",
        help="inspect repository, toolchain and optional container prerequisites",
    )
    doctor_parser.add_argument(
        "--profile",
        choices=("bootstrap", "full"),
        default="full",
        help="check dependency-bootstrap prerequisites or the complete local runtime",
    )
    doctor_parser.add_argument("--json", action="store_true")
    doctor_parser.set_defaults(handler=command_doctor)

    bootstrap_parser = subparsers.add_parser(
        "bootstrap",
        help="create locked local Python, Rust and product dependency state",
    )
    bootstrap_parser.add_argument(
        "--dry-run",
        action="store_true",
        help="print the exact bootstrap plan without changing local state",
    )
    bootstrap_parser.add_argument("--json", action="store_true")
    bootstrap_parser.set_defaults(handler=command_bootstrap)

    dev_up_parser = subparsers.add_parser(
        "dev-up",
        help="create or reuse the exact owned local PostgreSQL dependency plane",
    )
    dev_up_parser.add_argument(
        "--dry-run",
        action="store_true",
        help="inspect current state and print the mutation plan without changing Docker",
    )
    dev_up_parser.add_argument("--json", action="store_true")
    dev_up_parser.set_defaults(handler=command_dev_up)

    dev_reset_parser = subparsers.add_parser(
        "dev-reset",
        help="remove only owned local PostgreSQL state and recreate it cleanly",
    )
    dev_reset_parser.add_argument(
        "--dry-run",
        action="store_true",
        help="verify ownership and print the reset plan without changing Docker",
    )
    dev_reset_parser.add_argument("--json", action="store_true")
    dev_reset_parser.set_defaults(handler=command_dev_reset)

    check_affected = subparsers.add_parser(
        "check-affected",
        help="run structural preflight plus affected Rust package checks",
    )
    check_affected.add_argument("--base", default="origin/main")
    check_affected.add_argument(
        "--phase",
        choices=("all", "structural", "clippy", "test"),
        default="all",
        help="run the complete affected check or one diagnostic phase",
    )
    check_affected.set_defaults(handler=command_check_affected)

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
