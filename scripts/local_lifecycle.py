#!/usr/bin/env python3
"""Deterministic, cross-platform local repository lifecycle primitives."""

from __future__ import annotations

from dataclasses import asdict, dataclass
import json
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tomllib
from typing import Callable, Sequence

ROOT = Path(__file__).resolve().parents[1]
DOCTOR_SCHEMA = "crm.local-lifecycle-doctor/v1"
BOOTSTRAP_SCHEMA = "crm.local-lifecycle-bootstrap/v1"
MINIMUM_PYTHON = (3, 11)
REQUIRED_REPOSITORY_FILES = (
    "Cargo.toml",
    "Cargo.lock",
    "package.json",
    "pnpm-lock.yaml",
    "requirements-dev.txt",
    "rust-toolchain.toml",
    "scripts/repo.py",
)


class LifecycleError(RuntimeError):
    """Raised when a local lifecycle command cannot complete safely."""


@dataclass(frozen=True)
class Check:
    id: str
    ok: bool
    detail: str
    remediation: str | None = None
    required: bool = True


Capture = Callable[[Sequence[str]], subprocess.CompletedProcess[str]]
Which = Callable[[str], str | None]
Run = Callable[[Sequence[str]], None]


def _capture(
    command: Sequence[str], root: Path = ROOT
) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        list(command),
        cwd=root,
        check=False,
        capture_output=True,
        text=True,
    )


def _run(command: Sequence[str], root: Path = ROOT) -> None:
    rendered = " ".join(command)
    print(f"+ {rendered}", flush=True)
    completed = subprocess.run(list(command), cwd=root, check=False)
    if completed.returncode != 0:
        raise LifecycleError(
            f"command failed with exit code {completed.returncode}: {rendered}"
        )


def _combined_output(completed: subprocess.CompletedProcess[str]) -> str:
    return "\n".join(
        part.strip() for part in (completed.stdout, completed.stderr) if part.strip()
    ).strip()


def _version_tuple(value: str) -> tuple[int, int, int] | None:
    match = re.search(r"(\d+)\.(\d+)\.(\d+)", value)
    if match is None:
        return None
    return tuple(int(part) for part in match.groups())


def _repository_versions(root: Path) -> tuple[str, tuple[int, int, int], str]:
    try:
        rust = tomllib.loads((root / "rust-toolchain.toml").read_text(encoding="utf-8"))
        rust_channel = str(rust["toolchain"]["channel"])
    except (OSError, KeyError, TypeError, tomllib.TOMLDecodeError) as error:
        raise LifecycleError(f"cannot read pinned Rust toolchain: {error}") from error

    try:
        package = json.loads((root / "package.json").read_text(encoding="utf-8"))
        package_manager = str(package["packageManager"])
        node_range = str(package["engines"]["node"])
    except (OSError, json.JSONDecodeError, KeyError, TypeError) as error:
        raise LifecycleError(f"cannot read pinned product toolchain: {error}") from error

    if not package_manager.startswith("pnpm@"):
        raise LifecycleError("package.json packageManager must pin pnpm")
    pnpm_version = package_manager.removeprefix("pnpm@")
    minimum_node = _version_tuple(node_range)
    if minimum_node is None:
        raise LifecycleError("package.json engines.node must contain a minimum version")
    return rust_channel, minimum_node, pnpm_version


def _command_check(
    *,
    check_id: str,
    executable: str,
    command: Sequence[str],
    which: Which,
    capture: Capture,
    expected: Callable[[str], bool] | None = None,
    remediation: str,
    required: bool = True,
) -> Check:
    resolved = which(executable)
    if resolved is None:
        return Check(
            id=check_id,
            ok=False,
            detail=f"{executable} was not found on PATH",
            remediation=remediation,
            required=required,
        )
    completed = capture(command)
    output = _combined_output(completed)
    ok = completed.returncode == 0 and (expected(output) if expected else True)
    if ok:
        return Check(
            id=check_id,
            ok=True,
            detail=output or f"{executable} is available",
            required=required,
        )
    return Check(
        id=check_id,
        ok=False,
        detail=output or f"{' '.join(command)} exited with {completed.returncode}",
        remediation=remediation,
        required=required,
    )


def doctor(
    root: Path = ROOT,
    *,
    profile: str = "full",
    which: Which | None = None,
    capture: Capture | None = None,
    python_version: tuple[int, int, int] | None = None,
) -> dict[str, object]:
    """Inspect deterministic local prerequisites without changing the machine."""
    if profile not in {"bootstrap", "full"}:
        raise LifecycleError(f"unsupported doctor profile: {profile}")
    root = root.resolve()
    which = which or shutil.which
    capture = capture or (lambda command: _capture(command, root))
    python_version = python_version or sys.version_info[:3]

    missing = [
        relative
        for relative in REQUIRED_REPOSITORY_FILES
        if not (root / relative).is_file()
    ]
    checks = [
        Check(
            id="repository",
            ok=not missing,
            detail=(
                "required repository files are present"
                if not missing
                else f"missing repository files: {', '.join(missing)}"
            ),
            remediation=(
                None
                if not missing
                else "run the command from a complete Ultimate CRM checkout"
            ),
        ),
        Check(
            id="python",
            ok=python_version >= MINIMUM_PYTHON,
            detail="Python " + ".".join(str(part) for part in python_version),
            remediation=(
                None
                if python_version >= MINIMUM_PYTHON
                else "install Python 3.11 or newer"
            ),
        ),
        _command_check(
            check_id="python-venv",
            executable=sys.executable,
            command=(sys.executable, "-m", "venv", "--help"),
            which=which,
            capture=capture,
            remediation="install Python venv/ensurepip support for this interpreter",
        ),
    ]

    try:
        rust_channel, minimum_node, pnpm_version = _repository_versions(root)
    except LifecycleError as error:
        checks.append(
            Check(
                id="repository-toolchain",
                ok=False,
                detail=str(error),
                remediation="restore rust-toolchain.toml and package.json",
            )
        )
        rust_channel, minimum_node, pnpm_version = "", (0, 0, 0), ""

    checks.extend(
        [
            _command_check(
                check_id="git",
                executable="git",
                command=("git", "--version"),
                which=which,
                capture=capture,
                remediation="install Git and add it to PATH",
            ),
            _command_check(
                check_id="rustc",
                executable="rustc",
                command=("rustc", "--version"),
                which=which,
                capture=capture,
                expected=lambda output: output.startswith(f"rustc {rust_channel} "),
                remediation=f"install the repository-pinned Rust {rust_channel} toolchain",
            ),
            _command_check(
                check_id="cargo",
                executable="cargo",
                command=("cargo", "--version"),
                which=which,
                capture=capture,
                remediation="install Cargo through rustup",
            ),
            _command_check(
                check_id="node",
                executable="node",
                command=("node", "--version"),
                which=which,
                capture=capture,
                expected=lambda output: (
                    (parsed := _version_tuple(output)) is not None
                    and parsed >= minimum_node
                    and parsed[0] == minimum_node[0]
                ),
                remediation=(
                    "install Node "
                    + ".".join(str(part) for part in minimum_node)
                    + " or newer within the pinned major version"
                ),
            ),
            _command_check(
                check_id="pnpm",
                executable="pnpm",
                command=("pnpm", "--version"),
                which=which,
                capture=capture,
                expected=lambda output: _version_tuple(output)
                == _version_tuple(pnpm_version),
                remediation=f"activate pnpm {pnpm_version} from package.json",
            ),
        ]
    )

    if profile == "full":
        checks.extend(
            [
                _command_check(
                    check_id="docker",
                    executable="docker",
                    command=("docker", "--version"),
                    which=which,
                    capture=capture,
                    remediation="install Docker Desktop or Docker Engine",
                ),
                _command_check(
                    check_id="docker-compose",
                    executable="docker",
                    command=("docker", "compose", "version"),
                    which=which,
                    capture=capture,
                    remediation="install the Docker Compose v2 plugin",
                ),
                _command_check(
                    check_id="docker-daemon",
                    executable="docker",
                    command=("docker", "info", "--format", "{{.ServerVersion}}"),
                    which=which,
                    capture=capture,
                    remediation="start the Docker daemon",
                ),
            ]
        )

    ok = all(check.ok for check in checks if check.required)
    return {
        "schema_version": DOCTOR_SCHEMA,
        "profile": profile,
        "ok": ok,
        "checks": [asdict(check) for check in checks],
    }


def render_doctor(report: dict[str, object]) -> str:
    lines = [f"Local doctor ({report['profile']}): {'OK' if report['ok'] else 'FAILED'}"]
    for raw in report["checks"]:
        check = dict(raw)
        marker = "PASS" if check["ok"] else "FAIL"
        lines.append(f"[{marker}] {check['id']}: {check['detail']}")
        if not check["ok"] and check.get("remediation"):
            lines.append(f"       fix: {check['remediation']}")
    return "\n".join(lines) + "\n"


def _venv_python(root: Path) -> Path:
    if sys.platform == "win32":
        return root / ".venv" / "Scripts" / "python.exe"
    return root / ".venv" / "bin" / "python"


def bootstrap_plan(root: Path = ROOT) -> list[list[str]]:
    """Return the exact idempotent bootstrap command plan."""
    root = root.resolve()
    venv_python = _venv_python(root)
    commands: list[list[str]] = []
    if not venv_python.exists():
        commands.append([sys.executable, "-m", "venv", ".venv"])
    commands.extend(
        [
            [
                str(venv_python),
                "-m",
                "pip",
                "install",
                "--disable-pip-version-check",
                "--requirement",
                "requirements-dev.txt",
            ],
            ["cargo", "fetch", "--locked"],
            ["pnpm", "install", "--frozen-lockfile"],
            ["cargo", "metadata", "--locked", "--format-version", "1", "--no-deps"],
            [
                str(venv_python),
                "scripts/generate_repository_navigation.py",
                "--check",
            ],
        ]
    )
    return commands


def bootstrap(
    root: Path = ROOT,
    *,
    dry_run: bool = False,
    run: Run | None = None,
    doctor_report: dict[str, object] | None = None,
) -> dict[str, object]:
    """Create local dependency state from committed constraints and lockfiles."""
    root = root.resolve()
    report = (
        doctor_report
        if doctor_report is not None
        else doctor(root, profile="bootstrap")
    )
    if not bool(report["ok"]):
        raise LifecycleError(
            "bootstrap prerequisites failed; run repo.py doctor --profile bootstrap"
        )

    commands = bootstrap_plan(root)
    if not dry_run:
        runner = run or (lambda command: _run(command, root))
        for command in commands:
            runner(command)

    return {
        "schema_version": BOOTSTRAP_SCHEMA,
        "ok": True,
        "dry_run": dry_run,
        "commands": commands,
    }


def render_bootstrap(report: dict[str, object]) -> str:
    mode = "plan" if report["dry_run"] else "complete"
    lines = [f"Local bootstrap {mode}: {len(report['commands'])} command(s)"]
    lines.extend("+ " + " ".join(command) for command in report["commands"])
    return "\n".join(lines) + "\n"
