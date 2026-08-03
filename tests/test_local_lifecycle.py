from __future__ import annotations

import json
from pathlib import Path
import subprocess
import tempfile
import unittest

from scripts.local_lifecycle import (
    BOOTSTRAP_SCHEMA,
    DOCTOR_SCHEMA,
    LifecycleError,
    bootstrap,
    bootstrap_plan,
    doctor,
)
from scripts.repo import build_parser


def prepare_root(root: Path) -> None:
    for relative in (
        "Cargo.toml",
        "Cargo.lock",
        "pnpm-lock.yaml",
        "requirements-dev.txt",
        "scripts/repo.py",
    ):
        path = root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text("", encoding="utf-8")
    (root / "rust-toolchain.toml").write_text(
        '[toolchain]\nchannel = "1.97.1"\n',
        encoding="utf-8",
    )
    (root / "package.json").write_text(
        json.dumps(
            {
                "packageManager": "pnpm@11.12.0",
                "engines": {"node": ">=24.18.0 <25"},
            }
        ),
        encoding="utf-8",
    )


def successful_capture(
    command: tuple[str, ...] | list[str],
) -> subprocess.CompletedProcess[str]:
    outputs = {
        ("git", "--version"): "git version 2.51.0",
        ("rustc", "--version"): "rustc 1.97.1 (example 2026-07-01)",
        ("cargo", "--version"): "cargo 1.97.1 (example 2026-07-01)",
        ("node", "--version"): "v24.18.0",
        ("pnpm", "--version"): "11.12.0",
        ("docker", "--version"): "Docker version 29.0.0",
        ("docker", "compose", "version"): "Docker Compose version v2.40.0",
        ("docker", "info", "--format", "{{.ServerVersion}}"): "29.0.0",
    }
    key = tuple(command)
    return subprocess.CompletedProcess(list(command), 0, outputs[key], "")


class LocalLifecycleTests(unittest.TestCase):
    def test_full_doctor_reads_repository_pins_and_passes(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            prepare_root(root)
            report = doctor(
                root,
                profile="full",
                which=lambda executable: f"/tools/{executable}",
                capture=successful_capture,
                python_version=(3, 13, 5),
            )
        self.assertEqual(report["schema_version"], DOCTOR_SCHEMA)
        self.assertTrue(report["ok"])
        self.assertEqual(report["profile"], "full")
        self.assertEqual(
            [check["id"] for check in report["checks"]],
            [
                "repository",
                "python",
                "git",
                "rustc",
                "cargo",
                "node",
                "pnpm",
                "docker",
                "docker-compose",
                "docker-daemon",
            ],
        )

    def test_bootstrap_profile_does_not_require_docker(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            prepare_root(root)
            report = doctor(
                root,
                profile="bootstrap",
                which=lambda executable: (
                    None if executable == "docker" else f"/tools/{executable}"
                ),
                capture=successful_capture,
                python_version=(3, 11, 0),
            )
        self.assertTrue(report["ok"])
        ids = [check["id"] for check in report["checks"]]
        self.assertNotIn("docker", ids)
        self.assertNotIn("docker-daemon", ids)

    def test_doctor_fails_closed_on_wrong_rust_toolchain(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            prepare_root(root)

            def capture(
                command: tuple[str, ...] | list[str],
            ) -> subprocess.CompletedProcess[str]:
                completed = successful_capture(command)
                if tuple(command) == ("rustc", "--version"):
                    return subprocess.CompletedProcess(
                        list(command), 0, "rustc 1.96.0 (old)", ""
                    )
                return completed

            report = doctor(
                root,
                profile="bootstrap",
                which=lambda executable: f"/tools/{executable}",
                capture=capture,
                python_version=(3, 13, 0),
            )
        self.assertFalse(report["ok"])
        rust = next(check for check in report["checks"] if check["id"] == "rustc")
        self.assertFalse(rust["ok"])
        self.assertIn("1.97.1", rust["remediation"])

    def test_bootstrap_plan_is_locked_idempotent_and_shell_free(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            prepare_root(root)
            plan = bootstrap_plan(root)
            self.assertEqual(plan[0][1:4], ["-m", "venv", ".venv"])
            self.assertIn(["cargo", "fetch", "--locked"], plan)
            self.assertIn(["pnpm", "install", "--frozen-lockfile"], plan)
            self.assertIn(
                ["cargo", "metadata", "--locked", "--format-version", "1", "--no-deps"],
                plan,
            )
            self.assertTrue(
                any(
                    command[-2:]
                    == ["scripts/generate_repository_navigation.py", "--check"]
                    for command in plan
                )
            )
            for command in plan:
                self.assertIsInstance(command, list)
                self.assertTrue(all(isinstance(argument, str) for argument in command))

    def test_dry_run_executes_nothing_and_reports_exact_plan(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            prepare_root(root)
            executed: list[list[str]] = []
            report = bootstrap(
                root,
                dry_run=True,
                run=lambda command: executed.append(list(command)),
                doctor_report={"ok": True},
            )
        self.assertEqual(report["schema_version"], BOOTSTRAP_SCHEMA)
        self.assertTrue(report["dry_run"])
        self.assertEqual(executed, [])
        self.assertGreater(len(report["commands"]), 0)

    def test_bootstrap_refuses_failed_prerequisites(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            with self.assertRaisesRegex(LifecycleError, "prerequisites failed"):
                bootstrap(
                    Path(temporary),
                    dry_run=True,
                    doctor_report={"ok": False},
                )

    def test_repository_parser_exposes_local_lifecycle(self) -> None:
        parser = build_parser()
        doctor_args = parser.parse_args(["doctor", "--profile", "bootstrap", "--json"])
        self.assertEqual(doctor_args.command, "doctor")
        self.assertEqual(doctor_args.profile, "bootstrap")
        self.assertTrue(doctor_args.json)
        bootstrap_args = parser.parse_args(["bootstrap", "--dry-run", "--json"])
        self.assertEqual(bootstrap_args.command, "bootstrap")
        self.assertTrue(bootstrap_args.dry_run)
        self.assertTrue(bootstrap_args.json)


if __name__ == "__main__":
    unittest.main()
