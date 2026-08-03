#!/usr/bin/env python3
"""Deterministic demo seeding and real-process smoke lifecycle."""

from __future__ import annotations

from collections.abc import Callable, Mapping, Sequence
import json
import os
from pathlib import Path
import subprocess

try:
    from local_dev import DockerRuntime, LifecycleError, build_dev_config, dev_up
except ModuleNotFoundError:  # Imported as scripts.local_demo in tests.
    from scripts.local_dev import DockerRuntime, LifecycleError, build_dev_config, dev_up

ROOT = Path(__file__).resolve().parents[1]
DEMO_SCHEMA = "crm.local-demo/v1"
DEMO_DATASET_VERSION = "crm.local-demo.dataset/v1"
DEMO_PARTY_ID = "local-demo-acme"
DEMO_PARTY_DISPLAY_NAME = "Acme Local Demo"
DEMO_IDEMPOTENCY_KEY = "local-demo-v1-party-create"
DEMO_TEST_TARGET = "local_demo_smoke_e2e"
DEMO_TEST_NAME = "deterministic_local_demo_seed_or_smoke"
Execute = Callable[
    [Sequence[str], Mapping[str, str]], subprocess.CompletedProcess[str]
]
Prepare = Callable[[], dict[str, object]]


def demo_test_command() -> list[str]:
    return [
        "cargo",
        "test",
        "--locked",
        "-p",
        "crm-api",
        "--test",
        DEMO_TEST_TARGET,
        DEMO_TEST_NAME,
        "--",
        "--exact",
        "--nocapture",
    ]


def _admin_database_url(config: object) -> str:
    return (
        f"postgres://{config.admin_user}:{config.admin_password}@"
        f"{config.host}:{config.port}/{config.database}"
    )


def _default_execute(
    root: Path,
    command: Sequence[str],
    environment: Mapping[str, str],
) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        list(command),
        cwd=root,
        env=dict(environment),
        check=False,
        text=True,
    )


def run_demo(
    mode: str,
    root: Path = ROOT,
    *,
    dry_run: bool = False,
    runtime: DockerRuntime | None = None,
    doctor_report: dict[str, object] | None = None,
    environ: Mapping[str, str] | None = None,
    execute: Execute | None = None,
    prepare: Prepare | None = None,
) -> dict[str, object]:
    """Seed or verify the deterministic demo through the real production gateway."""
    if mode not in {"seed", "smoke"}:
        raise LifecycleError(f"unsupported local demo mode: {mode}")

    root = root.resolve()
    environment_overrides = {} if environ is None else dict(environ)
    config = build_dev_config(root, environ=environment_overrides)
    if prepare is None:
        dependency_report = dev_up(
            root,
            dry_run=dry_run,
            runtime=runtime,
            doctor_report=doctor_report,
            environ=environment_overrides,
        )
    else:
        dependency_report = prepare()
    if not bool(dependency_report.get("ok", True)):
        raise LifecycleError("local PostgreSQL dependency plane is not ready")

    command = demo_test_command()
    operation = (
        "create or idempotently replay the governed local demo Party"
        if mode == "seed"
        else "verify readiness, authenticated Party read, authentication denial and tenant isolation"
    )
    report: dict[str, object] = {
        "schema_version": DEMO_SCHEMA,
        "ok": True,
        "mode": mode,
        "dry_run": dry_run,
        "dataset_version": DEMO_DATASET_VERSION,
        "namespace": config.namespace,
        "postgres": {
            "container": config.container_name,
            "host": config.host,
            "port": config.port,
            "database": config.database,
        },
        "demo": {
            "tenant_id": "tenant-a",
            "party_id": DEMO_PARTY_ID,
            "display_name": DEMO_PARTY_DISPLAY_NAME,
            "idempotency_key": DEMO_IDEMPOTENCY_KEY,
        },
        "command": command,
        "operations": [
            f"ensure owned PostgreSQL dependency plane ({dependency_report.get('action', 'ready')})",
            operation,
            "start and gracefully stop the real crm-api process",
        ],
    }
    if dry_run:
        return report

    process_environment = dict(os.environ)
    process_environment.update(environment_overrides)
    process_environment.update(
        {
            "DATABASE_URL": config.app_database_url,
            "ADMIN_DATABASE_URL": _admin_database_url(config),
            "CRM_LOCAL_DEMO_MODE": mode,
            "CRM_LOCAL_DEMO_DATASET_VERSION": DEMO_DATASET_VERSION,
            "CRM_LOCAL_DEMO_PARTY_ID": DEMO_PARTY_ID,
            "CRM_LOCAL_DEMO_PARTY_DISPLAY_NAME": DEMO_PARTY_DISPLAY_NAME,
            "CRM_LOCAL_DEMO_IDEMPOTENCY_KEY": DEMO_IDEMPOTENCY_KEY,
        }
    )
    runner = execute or (lambda command, environment: _default_execute(root, command, environment))
    completed = runner(command, process_environment)
    if completed.returncode != 0:
        raise LifecycleError(
            f"local demo {mode} failed with exit code {completed.returncode}: "
            f"{' '.join(command)}"
        )
    return report


def seed_demo(
    root: Path = ROOT,
    **kwargs: object,
) -> dict[str, object]:
    return run_demo("seed", root, **kwargs)


def smoke(
    root: Path = ROOT,
    **kwargs: object,
) -> dict[str, object]:
    return run_demo("smoke", root, **kwargs)


def render_demo(report: Mapping[str, object]) -> str:
    postgres = dict(report["postgres"])
    demo = dict(report["demo"])
    mode = str(report["mode"])
    state = "plan" if bool(report["dry_run"]) else "OK"
    lines = [
        f"Local demo {mode}: {state}",
        f"Dataset: {report['dataset_version']}",
        f"Namespace: {report['namespace']}",
        f"PostgreSQL: {postgres['host']}:{postgres['port']}/{postgres['database']}",
        f"Demo Party: {demo['party_id']} ({demo['display_name']})",
    ]
    for operation in report["operations"]:
        lines.append(f"- {operation}")
    if bool(report["dry_run"]):
        lines.append("+ " + " ".join(report["command"]))
    return "\n".join(lines) + "\n"


def render_demo_json(report: Mapping[str, object]) -> str:
    return json.dumps(report, indent=2, sort_keys=True) + "\n"
