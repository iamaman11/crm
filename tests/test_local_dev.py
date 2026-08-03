from __future__ import annotations

import json
from pathlib import Path
import subprocess
import tempfile
import unittest

from scripts.local_dev import (
    DEV_SCHEMA,
    LABEL_PREFIX,
    POSTGRES_IMAGE,
    DockerRuntime,
    LifecycleError,
    build_dev_config,
    dev_reset,
    dev_up,
)
from scripts.repo import build_parser


class FakeRuntime:
    def __init__(self) -> None:
        self.container: dict[str, object] | None = None
        self.volume: dict[str, object] | None = None
        self.marker = ""
        self.operations: list[str] = []
        self.sql: list[str] = []

    @staticmethod
    def _container(config, *, running: bool = True) -> dict[str, object]:
        return {
            "Config": {
                "Image": config.image,
                "Labels": config.labels("postgres-container"),
            },
            "HostConfig": {
                "PortBindings": {
                    "5432/tcp": [
                        {"HostIp": config.host, "HostPort": str(config.port)}
                    ]
                }
            },
            "Mounts": [
                {
                    "Name": config.volume_name,
                    "Destination": "/var/lib/postgresql/data",
                }
            ],
            "State": {
                "Running": running,
                "Status": "running" if running else "exited",
            },
        }

    @staticmethod
    def _volume(config) -> dict[str, object]:
        return {"Labels": config.labels("postgres-volume")}

    def inspect_container(self, name: str) -> dict[str, object] | None:
        return self.container

    def inspect_volume(self, name: str) -> dict[str, object] | None:
        return self.volume

    def create_volume(self, config) -> None:
        self.operations.append("create-volume")
        self.volume = self._volume(config)

    def create_container(self, config) -> None:
        self.operations.append("create-container")
        self.container = self._container(config)

    def start_container(self, name: str) -> None:
        self.operations.append("start-container")
        assert self.container is not None
        state = self.container["State"]
        assert isinstance(state, dict)
        state["Running"] = True
        state["Status"] = "running"

    def remove_container(self, name: str) -> None:
        self.operations.append("remove-container")
        self.container = None

    def remove_volume(self, name: str) -> None:
        self.operations.append("remove-volume")
        self.volume = None
        self.marker = ""

    def wait_ready(self, config, attempts: int = 60) -> None:
        self.operations.append("wait-ready")

    def execute_sql(self, config, sql: str) -> str:
        self.sql.append(sql)
        if "COMMENT ON DATABASE" in sql:
            self.marker = f"ultimate-crm-local-schema:{config.schema_digest}"
            return ""
        if "obj_description" in sql:
            return self.marker
        if "information_schema.tables" in sql:
            return "42"
        return ""


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


class LocalDevTests(unittest.TestCase):
    def test_config_is_deterministic_pinned_and_loopback_only(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            prepare_root(root)
            first = build_dev_config(root, environ={})
            second = build_dev_config(root, environ={})
        self.assertEqual(first, second)
        self.assertEqual(first.host, "127.0.0.1")
        self.assertEqual(first.port, 5433)
        self.assertRegex(first.image, r"^postgres:17-alpine@sha256:[0-9a-f]{64}$")
        self.assertEqual(first.image, POSTGRES_IMAGE)
        self.assertIn(first.namespace, first.container_name)
        self.assertIn(first.namespace, first.volume_name)
        self.assertEqual(len(first.schema_digest), 64)
        self.assertEqual(
            first.schema_paths,
            (
                "database/migrations/0001_local.up.sql",
                "database/tests/0001_platform_foundation.sql",
                "database/tests/0003_sales_activities_adapters.sql",
                "database/tests/0004_search_runtime_role_grants.sql",
            ),
        )

    def test_explicit_namespace_and_port_are_validated(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            prepare_root(root)
            config = build_dev_config(
                root,
                environ={
                    "CRM_LOCAL_NAMESPACE": "acceptance-1",
                    "CRM_LOCAL_POSTGRES_PORT": "55432",
                },
            )
            self.assertEqual(config.namespace, "acceptance-1")
            self.assertEqual(config.port, 55432)
            for environment in (
                {"CRM_LOCAL_NAMESPACE": "UPPER"},
                {"CRM_LOCAL_NAMESPACE": "ends-"},
                {"CRM_LOCAL_POSTGRES_PORT": "postgres"},
                {"CRM_LOCAL_POSTGRES_PORT": "80"},
            ):
                with self.subTest(environment=environment):
                    with self.assertRaises(LifecycleError):
                        build_dev_config(root, environ=environment)

    def test_schema_digest_changes_with_committed_input(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            prepare_root(root)
            before = build_dev_config(root, environ={}).schema_digest
            (root / "database/migrations/0001_local.up.sql").write_text(
                "CREATE SCHEMA crm;\nCREATE TABLE crm.example(id bigint);\n",
                encoding="utf-8",
            )
            after = build_dev_config(root, environ={}).schema_digest
        self.assertNotEqual(before, after)

    def test_wait_ready_requires_the_target_database(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            prepare_root(root)
            config = build_dev_config(
                root,
                environ={"CRM_LOCAL_NAMESPACE": "unit-ready"},
            )
            probes = 0
            sleeps: list[float] = []
            calls: list[list[str]] = []

            def execute(command, input_text):
                nonlocal probes
                command = list(command)
                calls.append(command)
                if command[:3] == ["docker", "container", "inspect"]:
                    payload = [{"State": {"Running": True, "Status": "running"}}]
                    return subprocess.CompletedProcess(
                        command,
                        0,
                        json.dumps(payload),
                        "",
                    )
                if "pg_isready" in command:
                    return subprocess.CompletedProcess(command, 0, "", "")
                if "psql" in command and "--command" in command:
                    probes += 1
                    return subprocess.CompletedProcess(
                        command,
                        0,
                        "1" if probes == 2 else "",
                        "",
                    )
                raise AssertionError(command)

            runtime = DockerRuntime(
                root,
                execute=execute,
                sleep=sleeps.append,
            )
            runtime.wait_ready(config, attempts=2)
        self.assertEqual(probes, 2)
        self.assertEqual(sleeps, [1.0])
        psql_calls = [command for command in calls if "psql" in command]
        self.assertEqual(len(psql_calls), 2)
        for command in psql_calls:
            self.assertEqual(command[command.index("--dbname") + 1], "postgres")

    def test_application_role_is_provisioned_before_first_migration(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            prepare_root(root)
            runtime = FakeRuntime()
            dev_up(
                root,
                runtime=runtime,
                doctor_report={"ok": True},
                environ={"CRM_LOCAL_NAMESPACE": "unit-role-order"},
            )
        self.assertIn("CREATE ROLE crm_app_test", runtime.sql[0])
        self.assertEqual(runtime.sql[1], "CREATE SCHEMA crm;\n")

    def test_fresh_up_then_unchanged_up_is_idempotent(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            prepare_root(root)
            runtime = FakeRuntime()
            created = dev_up(
                root,
                runtime=runtime,
                doctor_report={"ok": True},
                environ={"CRM_LOCAL_NAMESPACE": "unit-up"},
            )
            initialization_sql_count = len(runtime.sql)
            reused = dev_up(
                root,
                runtime=runtime,
                doctor_report={"ok": True},
                environ={"CRM_LOCAL_NAMESPACE": "unit-up"},
            )
        self.assertEqual(created["schema_version"], DEV_SCHEMA)
        self.assertEqual(created["action"], "created")
        self.assertEqual(reused["action"], "reused")
        self.assertEqual(len(runtime.sql), initialization_sql_count + 2)
        self.assertEqual(runtime.operations.count("create-volume"), 1)
        self.assertEqual(runtime.operations.count("create-container"), 1)
        self.assertEqual(created["postgres"]["crm_table_count"], 42)

    def test_up_refuses_partial_or_schema_drifted_state(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            prepare_root(root)
            config = build_dev_config(
                root, environ={"CRM_LOCAL_NAMESPACE": "unit-drift"}
            )
            runtime = FakeRuntime()
            runtime.volume = runtime._volume(config)
            with self.assertRaisesRegex(LifecycleError, "incomplete local Docker state"):
                dev_up(
                    root,
                    runtime=runtime,
                    doctor_report={"ok": True},
                    environ={"CRM_LOCAL_NAMESPACE": "unit-drift"},
                )
            runtime.container = runtime._container(config)
            labels = runtime.container["Config"]
            assert isinstance(labels, dict)
            label_values = labels["Labels"]
            assert isinstance(label_values, dict)
            label_values[f"{LABEL_PREFIX}.schema"] = "0" * 64
            with self.assertRaisesRegex(LifecycleError, "foreign or drifted"):
                dev_up(
                    root,
                    runtime=runtime,
                    doctor_report={"ok": True},
                    environ={"CRM_LOCAL_NAMESPACE": "unit-drift"},
                )

    def test_up_refuses_missing_schema_marker_without_mutation(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            prepare_root(root)
            config = build_dev_config(
                root, environ={"CRM_LOCAL_NAMESPACE": "unit-marker"}
            )
            runtime = FakeRuntime()
            runtime.container = runtime._container(config)
            runtime.volume = runtime._volume(config)
            with self.assertRaisesRegex(LifecycleError, "initialization is incomplete"):
                dev_up(
                    root,
                    runtime=runtime,
                    doctor_report={"ok": True},
                    environ={"CRM_LOCAL_NAMESPACE": "unit-marker"},
                )
        self.assertNotIn("remove-container", runtime.operations)
        self.assertNotIn("remove-volume", runtime.operations)

    def test_reset_refuses_foreign_resources_before_removal(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            prepare_root(root)
            config = build_dev_config(
                root, environ={"CRM_LOCAL_NAMESPACE": "unit-foreign"}
            )
            runtime = FakeRuntime()
            runtime.container = runtime._container(config)
            runtime.volume = runtime._volume(config)
            labels = runtime.container["Config"]
            assert isinstance(labels, dict)
            label_values = labels["Labels"]
            assert isinstance(label_values, dict)
            label_values[f"{LABEL_PREFIX}.owner"] = "someone-else"
            with self.assertRaisesRegex(LifecycleError, "refusing foreign"):
                dev_reset(
                    root,
                    runtime=runtime,
                    doctor_report={"ok": True},
                    environ={"CRM_LOCAL_NAMESPACE": "unit-foreign"},
                )
        self.assertNotIn("remove-container", runtime.operations)
        self.assertNotIn("remove-volume", runtime.operations)

    def test_reset_dry_run_mutates_nothing(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            prepare_root(root)
            config = build_dev_config(
                root, environ={"CRM_LOCAL_NAMESPACE": "unit-plan"}
            )
            runtime = FakeRuntime()
            runtime.container = runtime._container(config)
            runtime.volume = runtime._volume(config)
            report = dev_reset(
                root,
                dry_run=True,
                runtime=runtime,
                doctor_report={"ok": True},
                environ={"CRM_LOCAL_NAMESPACE": "unit-plan"},
            )
        self.assertTrue(report["dry_run"])
        self.assertEqual(runtime.operations, [])
        self.assertIsNotNone(runtime.container)
        self.assertIsNotNone(runtime.volume)

    def test_reset_removes_container_before_volume_and_recreates(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            prepare_root(root)
            runtime = FakeRuntime()
            dev_up(
                root,
                runtime=runtime,
                doctor_report={"ok": True},
                environ={"CRM_LOCAL_NAMESPACE": "unit-reset"},
            )
            runtime.operations.clear()
            report = dev_reset(
                root,
                runtime=runtime,
                doctor_report={"ok": True},
                environ={"CRM_LOCAL_NAMESPACE": "unit-reset"},
            )
        self.assertEqual(report["action"], "reset")
        self.assertEqual(
            runtime.operations[:4],
            ["remove-container", "remove-volume", "create-volume", "create-container"],
        )
        self.assertIsNotNone(runtime.container)
        self.assertIsNotNone(runtime.volume)

    def test_docker_runtime_uses_argument_arrays_and_stdin(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            prepare_root(root)
            config = build_dev_config(
                root, environ={"CRM_LOCAL_NAMESPACE": "unit-command"}
            )
            calls: list[tuple[list[str], str | None]] = []

            def execute(command, input_text):
                calls.append((list(command), input_text))
                return subprocess.CompletedProcess(list(command), 0, "", "")

            runtime = DockerRuntime(root, execute=execute, sleep=lambda _: None)
            runtime.create_volume(config)
            runtime.create_container(config)
            runtime.execute_sql(config, "SELECT 1;\n")
        self.assertTrue(all(isinstance(command, list) for command, _ in calls))
        self.assertTrue(
            all(all(isinstance(part, str) for part in command) for command, _ in calls)
        )
        self.assertFalse(any(command[:2] == ["sh", "-c"] for command, _ in calls))
        self.assertEqual(calls[-1][1], "SELECT 1;\n")
        publish = calls[1][0][calls[1][0].index("--publish") + 1]
        self.assertEqual(publish, "127.0.0.1:5433:5432")

    def test_repository_parser_exposes_dev_commands(self) -> None:
        parser = build_parser()
        up = parser.parse_args(["dev-up", "--dry-run", "--json"])
        self.assertEqual(up.command, "dev-up")
        self.assertTrue(up.dry_run)
        self.assertTrue(up.json)
        reset = parser.parse_args(["dev-reset", "--dry-run", "--json"])
        self.assertEqual(reset.command, "dev-reset")
        self.assertTrue(reset.dry_run)
        self.assertTrue(reset.json)


if __name__ == "__main__":
    unittest.main()
