#!/usr/bin/env python3
"""Deterministic Docker-backed local PostgreSQL lifecycle."""

from __future__ import annotations

from dataclasses import dataclass
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import time
from typing import Callable, Mapping, Sequence

try:
    from local_lifecycle import LifecycleError, doctor
except ModuleNotFoundError:  # Imported as scripts.local_dev in tests.
    from scripts.local_lifecycle import LifecycleError, doctor

ROOT = Path(__file__).resolve().parents[1]
DEV_SCHEMA = "crm.local-lifecycle-dev/v1"
POSTGRES_IMAGE = (
    "postgres:17-alpine@"
    "sha256:742f40ea20b9ff2ff31db5458d127452988a2164df9e17441e191f3b72252193"
)
POSTGRES_CONTAINER_PORT = 5432
DEFAULT_POSTGRES_PORT = 5433
POSTGRES_DATABASE = "crm_dev"
POSTGRES_ADMIN_USER = "postgres"
POSTGRES_ADMIN_PASSWORD = "postgres"
POSTGRES_APP_USER = "crm_app_test"
POSTGRES_APP_PASSWORD = "crm_app_test"
POSTGRES_DATA_PATH = "/var/lib/postgresql/data"
SCHEMA_MARKER_PREFIX = "ultimate-crm-local-schema:"
SCHEMA_INPUT_VERSION = "crm.local-postgres-schema/v1"
FIXTURE_PATHS = (
    "database/tests/0001_platform_foundation.sql",
    "database/tests/0003_sales_activities_adapters.sql",
    "database/tests/0004_search_runtime_role_grants.sql",
)
LABEL_PREFIX = "com.ultimate-crm.local"
NAMESPACE_PATTERN = re.compile(r"[a-z0-9](?:[a-z0-9-]{0,30}[a-z0-9])?")
Execute = Callable[[Sequence[str], str | None], subprocess.CompletedProcess[str]]
Sleep = Callable[[float], None]


@dataclass(frozen=True)
class DevConfig:
    root_digest: str
    namespace: str
    container_name: str
    volume_name: str
    image: str
    host: str
    port: int
    database: str
    admin_user: str
    admin_password: str
    app_user: str
    app_password: str
    schema_digest: str
    schema_paths: tuple[str, ...]

    def ownership_labels(self, resource: str) -> dict[str, str]:
        return {
            f"{LABEL_PREFIX}.owner": "ultimate-crm",
            f"{LABEL_PREFIX}.lifecycle": "dev",
            f"{LABEL_PREFIX}.namespace": self.namespace,
            f"{LABEL_PREFIX}.checkout": self.root_digest,
            f"{LABEL_PREFIX}.resource": resource,
        }

    def labels(self, resource: str) -> dict[str, str]:
        labels = self.ownership_labels(resource)
        labels.update(
            {
                f"{LABEL_PREFIX}.schema": self.schema_digest,
                f"{LABEL_PREFIX}.image": self.image,
                f"{LABEL_PREFIX}.database": self.database,
                f"{LABEL_PREFIX}.port": str(self.port),
            }
        )
        return labels

    @property
    def app_database_url(self) -> str:
        return (
            f"postgres://{self.app_user}:{self.app_password}@"
            f"{self.host}:{self.port}/{self.database}"
        )


class DockerRuntime:
    """Small Docker CLI adapter with no shell command strings."""

    def __init__(
        self,
        root: Path = ROOT,
        *,
        execute: Execute | None = None,
        sleep: Sleep | None = None,
    ) -> None:
        self.root = root.resolve()
        self.execute = execute or self._execute
        self.sleep = sleep or time.sleep

    def _execute(
        self, command: Sequence[str], input_text: str | None = None
    ) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            list(command),
            cwd=self.root,
            check=False,
            input=input_text,
            capture_output=True,
            text=True,
        )

    @staticmethod
    def _output(completed: subprocess.CompletedProcess[str]) -> str:
        return "\n".join(
            part.strip()
            for part in (completed.stdout, completed.stderr)
            if part.strip()
        ).strip()

    def require(
        self, command: Sequence[str], input_text: str | None = None
    ) -> subprocess.CompletedProcess[str]:
        completed = self.execute(command, input_text)
        if completed.returncode != 0:
            output = self._output(completed)
            suffix = f": {output}" if output else ""
            raise LifecycleError(
                f"Docker command failed with exit code {completed.returncode}: "
                f"{' '.join(command)}{suffix}"
            )
        return completed

    def _inspect(self, command: Sequence[str], kind: str) -> dict[str, object] | None:
        completed = self.execute(command, None)
        if completed.returncode != 0:
            output = self._output(completed).lower()
            if "no such" in output or "not found" in output:
                return None
            raise LifecycleError(f"cannot inspect {kind}: {self._output(completed)}")
        try:
            value = json.loads(completed.stdout)
        except json.JSONDecodeError as error:
            raise LifecycleError(f"Docker returned invalid {kind} inspection JSON") from error
        if not isinstance(value, list) or len(value) != 1 or not isinstance(value[0], dict):
            raise LifecycleError(f"Docker returned unexpected {kind} inspection data")
        return value[0]

    def inspect_container(self, name: str) -> dict[str, object] | None:
        return self._inspect(("docker", "container", "inspect", name), "container")

    def inspect_volume(self, name: str) -> dict[str, object] | None:
        return self._inspect(("docker", "volume", "inspect", name), "volume")

    def create_volume(self, config: DevConfig) -> None:
        command = ["docker", "volume", "create"]
        for key, value in sorted(config.labels("postgres-volume").items()):
            command.extend(("--label", f"{key}={value}"))
        command.append(config.volume_name)
        self.require(command)

    def create_container(self, config: DevConfig) -> None:
        command = ["docker", "run", "--detach", "--name", config.container_name]
        for key, value in sorted(config.labels("postgres-container").items()):
            command.extend(("--label", f"{key}={value}"))
        command.extend(
            (
                "--publish",
                f"{config.host}:{config.port}:{POSTGRES_CONTAINER_PORT}",
                "--env",
                f"POSTGRES_DB={config.database}",
                "--env",
                f"POSTGRES_USER={config.admin_user}",
                "--env",
                f"POSTGRES_PASSWORD={config.admin_password}",
                "--volume",
                f"{config.volume_name}:{POSTGRES_DATA_PATH}",
                config.image,
            )
        )
        self.require(command)

    def start_container(self, name: str) -> None:
        self.require(("docker", "container", "start", name))

    def remove_container(self, name: str) -> None:
        self.require(("docker", "container", "rm", "--force", name))

    def remove_volume(self, name: str) -> None:
        self.require(("docker", "volume", "rm", name))

    def execute_sql(self, config: DevConfig, sql: str) -> str:
        completed = self.require(
            (
                "docker",
                "exec",
                "--interactive",
                config.container_name,
                "psql",
                "--username",
                config.admin_user,
                "--dbname",
                config.database,
                "--no-psqlrc",
                "--set",
                "ON_ERROR_STOP=1",
                "--tuples-only",
                "--no-align",
            ),
            sql,
        )
        return completed.stdout.strip()

    def wait_ready(self, config: DevConfig, attempts: int = 60) -> None:
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
                    database.returncode == 0 and database.stdout.strip() == "1"
                )
            if database_ready:
                return
            container = self.inspect_container(config.container_name)
            state = container.get("State") if container else None
            status = state.get("Status") if isinstance(state, dict) else None
            if status in {"dead", "exited", "removing"}:
                raise LifecycleError(
                    f"local PostgreSQL container stopped before readiness: {status}"
                )
            if attempt + 1 < attempts:
                self.sleep(1.0)
        raise LifecycleError(
            "local PostgreSQL target database did not become ready within 60 seconds"
        )


def _schema_inputs(root: Path) -> tuple[tuple[str, ...], str]:
    migration_paths = sorted(
        path.relative_to(root).as_posix()
        for path in (root / "database/migrations").glob("*.up.sql")
        if path.is_file()
    )
    if not migration_paths:
        raise LifecycleError("no committed PostgreSQL up migrations were found")
    paths = tuple(migration_paths) + FIXTURE_PATHS
    digest = hashlib.sha256()
    digest.update(SCHEMA_INPUT_VERSION.encode("utf-8"))
    for relative in paths:
        path = root / relative
        if not path.is_file():
            raise LifecycleError(f"required local PostgreSQL input is missing: {relative}")
        digest.update(b"\0path\0")
        digest.update(relative.encode("utf-8"))
        digest.update(b"\0content\0")
        digest.update(path.read_bytes())
    return paths, digest.hexdigest()


def build_dev_config(
    root: Path = ROOT,
    *,
    environ: Mapping[str, str] | None = None,
) -> DevConfig:
    root = root.resolve()
    environment = os.environ if environ is None else environ
    root_digest = hashlib.sha256(root.as_posix().casefold().encode("utf-8")).hexdigest()
    namespace = environment.get("CRM_LOCAL_NAMESPACE", root_digest[:12])
    if NAMESPACE_PATTERN.fullmatch(namespace) is None:
        raise LifecycleError(
            "CRM_LOCAL_NAMESPACE must be 1-32 lowercase letters, digits or internal hyphens"
        )
    raw_port = environment.get("CRM_LOCAL_POSTGRES_PORT", str(DEFAULT_POSTGRES_PORT))
    try:
        port = int(raw_port)
    except ValueError as error:
        raise LifecycleError("CRM_LOCAL_POSTGRES_PORT must be an integer") from error
    if not 1024 <= port <= 65535:
        raise LifecycleError("CRM_LOCAL_POSTGRES_PORT must be between 1024 and 65535")
    schema_paths, schema_digest = _schema_inputs(root)
    return DevConfig(
        root_digest=root_digest,
        namespace=namespace,
        container_name=f"ultimate-crm-{namespace}-postgres",
        volume_name=f"ultimate-crm-{namespace}-postgres-data",
        image=POSTGRES_IMAGE,
        host="127.0.0.1",
        port=port,
        database=POSTGRES_DATABASE,
        admin_user=POSTGRES_ADMIN_USER,
        admin_password=POSTGRES_ADMIN_PASSWORD,
        app_user=POSTGRES_APP_USER,
        app_password=POSTGRES_APP_PASSWORD,
        schema_digest=schema_digest,
        schema_paths=schema_paths,
    )


def _labels(value: object, *, container: bool) -> dict[str, str]:
    if not isinstance(value, dict):
        raise LifecycleError("Docker inspection data is missing")
    source = value.get("Config") if container else value
    if not isinstance(source, dict):
        raise LifecycleError("Docker inspection data has no configuration")
    labels = source.get("Labels")
    if labels is None:
        return {}
    if not isinstance(labels, dict) or not all(
        isinstance(key, str) and isinstance(item, str) for key, item in labels.items()
    ):
        raise LifecycleError("Docker resource labels are malformed")
    return dict(labels)


def _verify_labels(
    actual: dict[str, str], expected: Mapping[str, str], resource_name: str
) -> None:
    mismatches = {
        key: {"expected": value, "actual": actual.get(key)}
        for key, value in expected.items()
        if actual.get(key) != value
    }
    if mismatches:
        raise LifecycleError(
            f"refusing foreign or drifted Docker resource {resource_name}: "
            + json.dumps(mismatches, sort_keys=True)
        )


def verify_container(
    value: dict[str, object], config: DevConfig, *, exact: bool
) -> None:
    expected = (
        config.labels("postgres-container")
        if exact
        else config.ownership_labels("postgres-container")
    )
    _verify_labels(_labels(value, container=True), expected, config.container_name)
    if not exact:
        return
    container_config = value.get("Config")
    host_config = value.get("HostConfig")
    mounts = value.get("Mounts")
    if not isinstance(container_config, dict) or container_config.get("Image") != config.image:
        raise LifecycleError("local PostgreSQL container image drifted; run dev-reset")
    if not isinstance(host_config, dict):
        raise LifecycleError("local PostgreSQL container host configuration is missing")
    bindings = host_config.get("PortBindings")
    expected_binding = [{"HostIp": config.host, "HostPort": str(config.port)}]
    if not isinstance(bindings, dict) or bindings.get("5432/tcp") != expected_binding:
        raise LifecycleError("local PostgreSQL port binding drifted; run dev-reset")
    if not isinstance(mounts, list) or not any(
        isinstance(mount, dict)
        and mount.get("Name") == config.volume_name
        and mount.get("Destination") == POSTGRES_DATA_PATH
        for mount in mounts
    ):
        raise LifecycleError("local PostgreSQL volume binding drifted; run dev-reset")


def verify_volume(value: dict[str, object], config: DevConfig, *, exact: bool) -> None:
    expected = (
        config.labels("postgres-volume")
        if exact
        else config.ownership_labels("postgres-volume")
    )
    _verify_labels(_labels(value, container=False), expected, config.volume_name)


def _marker(config: DevConfig) -> str:
    return SCHEMA_MARKER_PREFIX + config.schema_digest


def _read_marker(runtime: DockerRuntime, config: DevConfig) -> str:
    output = runtime.execute_sql(
        config,
        "SELECT COALESCE(obj_description(oid, 'pg_database'), '') "
        "FROM pg_database WHERE datname = current_database();\n",
    )
    lines = [line.strip() for line in output.splitlines() if line.strip()]
    return lines[-1] if lines else ""


def _initialize(runtime: DockerRuntime, root: Path, config: DevConfig) -> int:
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
        f"COMMENT ON DATABASE {config.database} IS '{_marker(config)}';\n",
    )
    count = runtime.execute_sql(
        config,
        "SELECT count(*) FROM information_schema.tables WHERE table_schema = 'crm';\n",
    )
    try:
        return int([line for line in count.splitlines() if line.strip()][-1])
    except (IndexError, ValueError) as error:
        raise LifecycleError("cannot verify initialized CRM table count") from error


def _report(
    config: DevConfig,
    *,
    action: str,
    dry_run: bool,
    table_count: int | None,
    operations: Sequence[str],
) -> dict[str, object]:
    return {
        "schema_version": DEV_SCHEMA,
        "ok": True,
        "action": action,
        "dry_run": dry_run,
        "namespace": config.namespace,
        "postgres": {
            "image": config.image,
            "container": config.container_name,
            "volume": config.volume_name,
            "host": config.host,
            "port": config.port,
            "database": config.database,
            "app_user": config.app_user,
            "app_database_url": config.app_database_url,
            "schema_digest": config.schema_digest,
            "schema_inputs": list(config.schema_paths),
            "crm_table_count": table_count,
        },
        "operations": list(operations),
    }


def _fresh_operations(config: DevConfig) -> list[str]:
    return [
        f"create owned volume {config.volume_name}",
        f"start owned PostgreSQL container {config.container_name}",
        "wait for PostgreSQL readiness",
        f"apply {len(config.schema_paths)} ordered migration/fixture inputs",
        "record and verify schema digest",
    ]


def _create_fresh(
    root: Path, runtime: DockerRuntime, config: DevConfig
) -> tuple[int, list[str]]:
    volume_created = False
    container_created = False
    try:
        runtime.create_volume(config)
        volume = runtime.inspect_volume(config.volume_name)
        if volume is None:
            raise LifecycleError("Docker did not create the local PostgreSQL volume")
        verify_volume(volume, config, exact=True)
        volume_created = True

        runtime.create_container(config)
        container = runtime.inspect_container(config.container_name)
        if container is None:
            raise LifecycleError("Docker did not create the local PostgreSQL container")
        verify_container(container, config, exact=True)
        container_created = True

        runtime.wait_ready(config)
        table_count = _initialize(runtime, root, config)
        if _read_marker(runtime, config) != _marker(config):
            raise LifecycleError("local PostgreSQL schema marker was not persisted")
        return table_count, _fresh_operations(config)
    except Exception:
        if container_created:
            try:
                current = runtime.inspect_container(config.container_name)
                if current is not None:
                    verify_container(current, config, exact=False)
                    runtime.remove_container(config.container_name)
            except LifecycleError:
                pass
        if volume_created:
            try:
                current = runtime.inspect_volume(config.volume_name)
                if current is not None:
                    verify_volume(current, config, exact=False)
                    runtime.remove_volume(config.volume_name)
            except LifecycleError:
                pass
        raise


def _require_doctor(root: Path, report: dict[str, object] | None) -> None:
    checked = report if report is not None else doctor(root, profile="full")
    if not bool(checked.get("ok")):
        raise LifecycleError("local runtime prerequisites failed; run repo.py doctor")


def dev_up(
    root: Path = ROOT,
    *,
    dry_run: bool = False,
    runtime: DockerRuntime | None = None,
    doctor_report: dict[str, object] | None = None,
    environ: Mapping[str, str] | None = None,
) -> dict[str, object]:
    """Create or reuse the exact owned local PostgreSQL dependency plane."""
    root = root.resolve()
    _require_doctor(root, doctor_report)
    config = build_dev_config(root, environ=environ)
    docker = runtime or DockerRuntime(root)
    container = docker.inspect_container(config.container_name)
    volume = docker.inspect_volume(config.volume_name)

    if (container is None) != (volume is None):
        raise LifecycleError("incomplete local Docker state detected; run repo.py dev-reset")
    if container is None and volume is None:
        if dry_run:
            return _report(
                config,
                action="create",
                dry_run=True,
                table_count=None,
                operations=_fresh_operations(config),
            )
        table_count, operations = _create_fresh(root, docker, config)
        return _report(
            config,
            action="created",
            dry_run=False,
            table_count=table_count,
            operations=operations,
        )

    assert container is not None and volume is not None
    verify_container(container, config, exact=True)
    verify_volume(volume, config, exact=True)
    state = container.get("State")
    running = isinstance(state, dict) and state.get("Running") is True
    operations: list[str] = []
    if not running:
        operations.append(f"start owned container {config.container_name}")
    operations.extend(("wait for PostgreSQL readiness", "verify schema digest"))
    if dry_run:
        return _report(
            config,
            action="reuse",
            dry_run=True,
            table_count=None,
            operations=operations,
        )
    if not running:
        docker.start_container(config.container_name)
    docker.wait_ready(config)
    marker = _read_marker(docker, config)
    if marker != _marker(config):
        raise LifecycleError(
            "local PostgreSQL schema drifted or initialization is incomplete; run dev-reset"
        )
    count = docker.execute_sql(
        config,
        "SELECT count(*) FROM information_schema.tables WHERE table_schema = 'crm';\n",
    )
    try:
        table_count = int([line for line in count.splitlines() if line.strip()][-1])
    except (IndexError, ValueError) as error:
        raise LifecycleError("cannot verify existing CRM table count") from error
    return _report(
        config,
        action="reused",
        dry_run=False,
        table_count=table_count,
        operations=operations,
    )


def dev_reset(
    root: Path = ROOT,
    *,
    dry_run: bool = False,
    runtime: DockerRuntime | None = None,
    doctor_report: dict[str, object] | None = None,
    environ: Mapping[str, str] | None = None,
) -> dict[str, object]:
    """Remove only exactly owned local state and recreate a clean dependency plane."""
    root = root.resolve()
    _require_doctor(root, doctor_report)
    config = build_dev_config(root, environ=environ)
    docker = runtime or DockerRuntime(root)
    container = docker.inspect_container(config.container_name)
    volume = docker.inspect_volume(config.volume_name)
    operations: list[str] = []
    if container is not None:
        verify_container(container, config, exact=False)
        operations.append(f"remove owned container {config.container_name}")
    if volume is not None:
        verify_volume(volume, config, exact=False)
        operations.append(f"remove owned volume {config.volume_name}")
    operations.extend(_fresh_operations(config))
    if dry_run:
        return _report(
            config,
            action="reset",
            dry_run=True,
            table_count=None,
            operations=operations,
        )
    if container is not None:
        docker.remove_container(config.container_name)
    if volume is not None:
        docker.remove_volume(config.volume_name)
    table_count, fresh = _create_fresh(root, docker, config)
    return _report(
        config,
        action="reset",
        dry_run=False,
        table_count=table_count,
        operations=operations[: len(operations) - len(_fresh_operations(config))] + fresh,
    )


def render_dev(report: dict[str, object]) -> str:
    postgres = dict(report["postgres"])
    mode = "plan" if report["dry_run"] else str(report["action"])
    lines = [
        f"Local PostgreSQL {mode}: OK",
        f"Namespace: {report['namespace']}",
        f"Container: {postgres['container']}",
        f"Volume: {postgres['volume']}",
        f"Image: {postgres['image']}",
        f"Endpoint: {postgres['host']}:{postgres['port']}/{postgres['database']}",
        f"Application URL: {postgres['app_database_url']}",
        f"Schema digest: {postgres['schema_digest']}",
    ]
    for operation in report["operations"]:
        lines.append(f"- {operation}")
    return "\n".join(lines) + "\n"
