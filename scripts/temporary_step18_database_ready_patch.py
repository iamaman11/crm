#!/usr/bin/env python3
"""Temporary fail-closed patcher for the Step 18 database-readiness fix."""

from __future__ import annotations

from pathlib import Path
import re


implementation = Path("scripts/local_dev.py")
content = implementation.read_text(encoding="utf-8")

wait_ready = '''    def wait_ready(self, config: DevConfig, attempts: int = 60) -> None:
        database_query = (
            "SELECT 1 FROM pg_database "
            f"WHERE datname = '{config.database}';"
        )
        for attempt in range(attempts):
            ready = self.execute(
                (
                    "docker",
                    "exec",
                    config.container_name,
                    "pg_isready",
                    "--username",
                    config.admin_user,
                    "--dbname",
                    "postgres",
                ),
                None,
            )
            database_ready = False
            if ready.returncode == 0:
                database = self.execute(
                    (
                        "docker",
                        "exec",
                        config.container_name,
                        "psql",
                        "--username",
                        config.admin_user,
                        "--dbname",
                        "postgres",
                        "--no-psqlrc",
                        "--tuples-only",
                        "--no-align",
                        "--command",
                        database_query,
                    ),
                    None,
                )
                database_ready = (
                    database.returncode == 0
                    and database.stdout.strip() == "1"
                )
            if database_ready:
                return
            container = self.inspect_container(config.container_name)
            state = container.get("State") if container else None
            status = state.get("Status") if isinstance(state, dict) else None
            if status in {"dead", "exited", "removing"}:
                raise LifecycleError(
                    "local PostgreSQL container stopped before readiness: "
                    f"{status}"
                )
            if attempt + 1 < attempts:
                self.sleep(1.0)
        raise LifecycleError(
            "local PostgreSQL target database did not become ready within 60 seconds"
        )'''
content, count = re.subn(
    r"    def wait_ready\(self, config: DevConfig, attempts: int = 60\) -> None:\n.*?(?=\n\ndef _schema_inputs)",
    lambda _: wait_ready,
    content,
    count=1,
    flags=re.S,
)
if count != 1:
    raise SystemExit(f"expected wait_ready once, found {count}")

initialize = '''def _initialize(runtime: DockerRuntime, root: Path, config: DevConfig) -> int:
    runtime.execute_sql(
        config,
        """
DO $crm_local$
BEGIN
  IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'crm_app_test') THEN
    CREATE ROLE crm_app_test LOGIN PASSWORD 'crm_app_test';
  ELSE
    ALTER ROLE crm_app_test LOGIN PASSWORD 'crm_app_test';
  END IF;
END
$crm_local$;
""",
    )
    for relative in config.schema_paths:
        runtime.execute_sql(
            config,
            (root / relative).read_text(encoding="utf-8"),
        )
    runtime.execute_sql(
        config,
        f"COMMENT ON DATABASE {config.database} IS '{_marker(config)}';\\n",
    )
    count = runtime.execute_sql(
        config,
        "SELECT count(*) FROM information_schema.tables "
        "WHERE table_schema = 'crm';\\n",
    )
    try:
        return int([line for line in count.splitlines() if line.strip()][-1])
    except (IndexError, ValueError) as error:
        raise LifecycleError(
            "cannot verify initialized CRM table count"
        ) from error'''
content, count = re.subn(
    r"def _initialize\(runtime: DockerRuntime, root: Path, config: DevConfig\) -> int:\n.*?(?=\n\ndef _report)",
    lambda _: initialize,
    content,
    count=1,
    flags=re.S,
)
if count != 1:
    raise SystemExit(f"expected initialize once, found {count}")
implementation.write_text(content, encoding="utf-8")

tests = Path("tests/test_local_dev.py")
content = tests.read_text(encoding="utf-8")
marker = "    def test_fresh_up_then_unchanged_up_is_idempotent(self) -> None:\n"
methods = '''    def test_wait_ready_requires_the_target_database(self) -> None:
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
        self.assertEqual(runtime.sql[1], "CREATE SCHEMA crm;\\n")

'''
if content.count(marker) != 1:
    raise SystemExit(
        f"expected test insertion marker once, found {content.count(marker)}"
    )
tests.write_text(content.replace(marker, methods + marker), encoding="utf-8")
