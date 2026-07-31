#!/usr/bin/env python3
"""One-run materializer for exact native-composition affected-scope tests."""

from __future__ import annotations

import argparse
import os
from pathlib import Path
import subprocess
import sys

from repository_navigation import NavigationError, stale_generated_documents, write_generated_documents


PACKET_TEST_PATHS = (
    "tests/test_repository_navigation.py",
    "tests/test_architecture_documentation_consistency.py",
)


def parser() -> argparse.ArgumentParser:
    value = argparse.ArgumentParser()
    value.add_argument("--root", type=Path, default=Path.cwd())
    mode = value.add_mutually_exclusive_group(required=True)
    mode.add_argument("--write", action="store_true")
    mode.add_argument("--check", action="store_true")
    return value


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise NavigationError(
            f"affected-scope guard materializer expected one {label}, found {count}"
        )
    return text.replace(old, new, 1)


def materialize(root: Path) -> bool:
    sentinel = "test_native_composition_guard_has_exact_operations_scope"
    affected_test_path = root / "tests/test_affected_scope.py"
    if sentinel in affected_test_path.read_text(encoding="utf-8"):
        write_generated_documents(root)
        return False

    for relative in PACKET_TEST_PATHS:
        path = root / relative
        text = path.read_text(encoding="utf-8")
        text = replace_once(
            text,
            '            "Cargo.lock",\n'
            '            "crates/crm-application-runtime/Cargo.toml",\n',
            '            "Cargo.lock",\n'
            '            "affected-scope-policy.json",\n'
            '            "crates/crm-application-runtime/Cargo.toml",\n',
            f"allowed affected-scope policy in {relative}",
        )
        text = replace_once(
            text,
            '            "scripts/check_native_module_composition.py",\n'
            '            "tests/test_architecture_documentation_consistency.py",\n',
            '            "scripts/check_native_module_composition.py",\n'
            '            "tests/test_affected_scope.py",\n'
            '            "tests/test_architecture_documentation_consistency.py",\n',
            f"allowed affected-scope test in {relative}",
        )
        text = replace_once(
            text,
            '        for path in (\n'
            '            ".github/workflows/**",\n'
            '            "affected-scope-policy.json",\n'
            '            "apps/**",\n',
            '        for path in (\n'
            '            ".github/workflows/**",\n'
            '            "apps/**",\n',
            f"forbidden affected-scope policy in {relative}",
        )
        path.write_text(text, encoding="utf-8")

    tests = affected_test_path.read_text(encoding="utf-8")
    tests = replace_once(
        tests,
        '                "docs/CI_TELEMETRY_BASELINE.md",\n'
        '                "scripts/prepare_isolated_process_database.sh",\n',
        '                "docs/CI_TELEMETRY_BASELINE.md",\n'
        '                "scripts/check_native_module_composition.py",\n'
        '                "scripts/prepare_isolated_process_database.sh",\n',
        "operations representative path",
    )
    method = '''    def test_native_composition_guard_has_exact_operations_scope(self) -> None:
        root = Path(__file__).resolve().parents[1]
        empty_metadata = {"packages": [], "workspace_members": []}
        report = build_report(
            root,
            "origin/main",
            paths=["scripts/check_native_module_composition.py"],
            metadata=empty_metadata,
            head_sha="native-composition-guard",
        )
        self.assertEqual(
            [scope["id"] for scope in report["selected_scopes"]],
            ["operations"],
        )
        self.assertIn(
            "Governance CI",
            [workflow["name"] for workflow in report["selected_workflows"]],
        )
        with self.assertRaisesRegex(
            RuntimeError,
            "unknown affected scope cannot prove a safe non-Rust workflow closure",
        ):
            build_report(
                root,
                "origin/main",
                paths=["scripts/check_unclassified_native_guard.py"],
                metadata=empty_metadata,
                head_sha="unclassified-native-guard",
            )

'''
    tests = replace_once(
        tests,
        "    def test_glob_matching_handles_nested_paths(self) -> None:\n",
        method + "    def test_glob_matching_handles_nested_paths(self) -> None:\n",
        "exact operations-scope test insertion",
    )
    affected_test_path.write_text(tests, encoding="utf-8")

    write_generated_documents(root)
    subprocess.run(
        [sys.executable, "-m", "unittest", "-v", "tests.test_affected_scope"],
        cwd=root,
        check=True,
    )
    return True


def commit(root: Path) -> None:
    branch = os.environ.get("GITHUB_HEAD_REF") or os.environ.get("GITHUB_REF_NAME")
    if not branch:
        raise NavigationError("affected-scope guard materializer requires a branch ref")
    subprocess.run(["git", "config", "user.name", "github-actions[bot]"], cwd=root, check=True)
    subprocess.run(
        ["git", "config", "user.email", "41898282+github-actions[bot]@users.noreply.github.com"],
        cwd=root,
        check=True,
    )
    subprocess.run(
        [
            "git",
            "add",
            "docs/ACTIVE_PACKET.md",
            "tests/test_affected_scope.py",
            *PACKET_TEST_PATHS,
        ],
        cwd=root,
        check=True,
    )
    subprocess.run(
        ["git", "commit", "-m", "Prove exact native composition guard scope"],
        cwd=root,
        check=True,
    )
    subprocess.run(["git", "push", "origin", f"HEAD:{branch}"], cwd=root, check=True)


def main() -> int:
    args = parser().parse_args()
    try:
        if args.write:
            if materialize(args.root):
                commit(args.root)
            return 0
        stale = stale_generated_documents(args.root)
    except (NavigationError, subprocess.CalledProcessError) as error:
        print(f"ERROR: {error}", file=sys.stderr)
        return 1
    if stale:
        for path in stale:
            print(f"STALE {path}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
