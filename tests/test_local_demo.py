from __future__ import annotations

from pathlib import Path
import subprocess
import tempfile
import unittest

from scripts.local_demo import (
    DEMO_DATASET_VERSION,
    DEMO_IDEMPOTENCY_KEY,
    DEMO_PARTY_DISPLAY_NAME,
    DEMO_PARTY_ID,
    LifecycleError,
    demo_test_command,
    render_demo,
    seed_demo,
    smoke,
)
from scripts.repo import build_parser


def prepare_root(root: Path) -> None:
    migration = root / "database/migrations/0001_local.up.sql"
    migration.parent.mkdir(parents=True, exist_ok=True)
    migration.write_text("CREATE SCHEMA crm;\n", encoding="utf-8")
    fixtures = {
        "database/tests/0001_platform_foundation.sql": "SELECT 1;\n",
        "database/tests/0003_sales_activities_adapters.sql": "SELECT 3;\n",
        "database/tests/0004_search_runtime_role_grants.sql": "SELECT 4;\n",
    }
    for relative, content in fixtures.items():
        path = root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content, encoding="utf-8")


def ready_report(action: str = "reused") -> dict[str, object]:
    return {"ok": True, "action": action}


class LocalDemoTests(unittest.TestCase):
    def test_test_command_is_locked_targeted_and_shell_free(self) -> None:
        command = demo_test_command()
        self.assertEqual(
            command,
            [
                "cargo",
                "test",
                "--locked",
                "-p",
                "crm-api",
                "--test",
                "local_demo_smoke_e2e",
                "deterministic_local_demo_seed_or_smoke",
                "--",
                "--exact",
                "--nocapture",
            ],
        )
        self.assertNotIn("sh", command)
        self.assertNotIn("-c", command)

    def test_seed_uses_owned_database_and_exact_demo_contract(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            prepare_root(root)
            calls: list[tuple[list[str], dict[str, str]]] = []

            def execute(command, environment):
                calls.append((list(command), dict(environment)))
                return subprocess.CompletedProcess(list(command), 0, "", "")

            report = seed_demo(
                root,
                prepare=lambda: ready_report(),
                execute=execute,
                environ={
                    "CRM_LOCAL_NAMESPACE": "unit-demo",
                    "CRM_LOCAL_POSTGRES_PORT": "55432",
                },
            )

        self.assertEqual(report["mode"], "seed")
        self.assertEqual(report["dataset_version"], DEMO_DATASET_VERSION)
        self.assertEqual(report["demo"]["party_id"], DEMO_PARTY_ID)
        self.assertEqual(len(calls), 1)
        command, environment = calls[0]
        self.assertEqual(command, demo_test_command())
        self.assertEqual(environment["CRM_LOCAL_DEMO_MODE"], "seed")
        self.assertEqual(
            environment["CRM_LOCAL_DEMO_DATASET_VERSION"], DEMO_DATASET_VERSION
        )
        self.assertEqual(environment["CRM_LOCAL_DEMO_PARTY_ID"], DEMO_PARTY_ID)
        self.assertEqual(
            environment["CRM_LOCAL_DEMO_PARTY_DISPLAY_NAME"],
            DEMO_PARTY_DISPLAY_NAME,
        )
        self.assertEqual(
            environment["CRM_LOCAL_DEMO_IDEMPOTENCY_KEY"], DEMO_IDEMPOTENCY_KEY
        )
        self.assertEqual(
            environment["DATABASE_URL"],
            "postgres://crm_app_test:crm_app_test@127.0.0.1:55432/crm_dev",
        )
        self.assertEqual(
            environment["ADMIN_DATABASE_URL"],
            "postgres://postgres:postgres@127.0.0.1:55432/crm_dev",
        )

    def test_smoke_has_no_demo_mutation_operation(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            prepare_root(root)
            calls: list[dict[str, str]] = []

            def execute(command, environment):
                calls.append(dict(environment))
                return subprocess.CompletedProcess(list(command), 0, "", "")

            report = smoke(
                root,
                prepare=lambda: ready_report(),
                execute=execute,
                environ={"CRM_LOCAL_NAMESPACE": "unit-smoke"},
            )

        self.assertEqual(report["mode"], "smoke")
        self.assertEqual(calls[0]["CRM_LOCAL_DEMO_MODE"], "smoke")
        self.assertTrue(
            any("authentication denial" in operation for operation in report["operations"])
        )
        self.assertFalse(
            any("create or idempotently replay" in operation for operation in report["operations"])
        )

    def test_dry_run_executes_no_process(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            prepare_root(root)
            executed = False

            def execute(command, environment):
                nonlocal executed
                executed = True
                return subprocess.CompletedProcess(list(command), 0, "", "")

            report = seed_demo(
                root,
                dry_run=True,
                prepare=lambda: ready_report("create"),
                execute=execute,
                environ={"CRM_LOCAL_NAMESPACE": "unit-plan"},
            )

        self.assertTrue(report["dry_run"])
        self.assertFalse(executed)
        rendered = render_demo(report)
        self.assertIn("Local demo seed: plan", rendered)
        self.assertIn("cargo test --locked", rendered)
        self.assertNotIn("postgres:postgres", rendered)

    def test_failed_process_is_fail_closed(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            prepare_root(root)

            def execute(command, environment):
                return subprocess.CompletedProcess(list(command), 17, "", "")

            with self.assertRaisesRegex(LifecycleError, "exit code 17"):
                smoke(
                    root,
                    prepare=lambda: ready_report(),
                    execute=execute,
                    environ={"CRM_LOCAL_NAMESPACE": "unit-failure"},
                )

    def test_repository_parser_exposes_demo_commands(self) -> None:
        parser = build_parser()
        seed = parser.parse_args(["seed-demo", "--dry-run", "--json"])
        self.assertEqual(seed.command, "seed-demo")
        self.assertTrue(seed.dry_run)
        self.assertTrue(seed.json)
        verify = parser.parse_args(["smoke", "--dry-run", "--json"])
        self.assertEqual(verify.command, "smoke")
        self.assertTrue(verify.dry_run)
        self.assertTrue(verify.json)

    def test_invalid_mode_is_rejected_before_preparation(self) -> None:
        prepared = False

        def prepare() -> dict[str, object]:
            nonlocal prepared
            prepared = True
            return ready_report()

        with self.assertRaisesRegex(LifecycleError, "unsupported local demo mode"):
            from scripts.local_demo import run_demo

            run_demo("unknown", prepare=prepare)
        self.assertFalse(prepared)


if __name__ == "__main__":
    unittest.main()
