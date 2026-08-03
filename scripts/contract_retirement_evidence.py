"""Immutable production zero-usage evidence for governed contract retirement."""

from __future__ import annotations

from datetime import date, timedelta
import hashlib
import json
from pathlib import Path
import re
from typing import Any

if __package__:
    from .contract_lifecycle import Coordinate, iso_date, required_string, validate_policy
else:
    from contract_lifecycle import Coordinate, iso_date, required_string, validate_policy

EVIDENCE_SCHEMA_VERSION = "crm.contract-retirement-evidence/v1"
ARTIFACT_SCHEMA_VERSION = "crm.contract-usage-observation/v1"
OBSERVATION_ID_RE = re.compile(r"^[a-z0-9][a-z0-9._-]{2,199}$")
SHA256_RE = re.compile(r"^sha256:[0-9a-f]{64}$")
SAFE_ARTIFACT_ROOT = Path("evidence/contract-lifecycle")


def _non_negative_integer(value: Any, location: str) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value < 0:
        raise ValueError(f"{location} must be a non-negative integer")
    return value


def _safe_artifact_path(value: Any, location: str) -> Path:
    text = required_string({"value": value}, "value", location)
    path = Path(text)
    if (
        path.is_absolute()
        or ".." in path.parts
        or path.suffix != ".json"
        or path.parts[: len(SAFE_ARTIFACT_ROOT.parts)] != SAFE_ARTIFACT_ROOT.parts
    ):
        raise ValueError(
            f"{location} must be a safe JSON path under {SAFE_ARTIFACT_ROOT.as_posix()}/"
        )
    return path


def _load_artifact(
    repository_root: Path,
    path: Path,
    expected_sha256: str,
    location: str,
) -> dict[str, Any]:
    artifact = repository_root.resolve() / path
    try:
        content = artifact.read_bytes()
    except OSError as error:
        raise ValueError(f"{location} cannot read artifact {path}: {error}") from error
    actual_sha256 = f"sha256:{hashlib.sha256(content).hexdigest()}"
    if actual_sha256 != expected_sha256:
        raise ValueError(
            f"{location} artifact digest mismatch for {path}: "
            f"expected {expected_sha256}, got {actual_sha256}"
        )
    try:
        value = json.loads(content)
    except json.JSONDecodeError as error:
        raise ValueError(f"{location} artifact {path} is invalid JSON: {error}") from error
    if not isinstance(value, dict):
        raise ValueError(f"{location} artifact {path} must contain an object")
    return value


def _normalize_observation(
    entry: dict[str, Any],
    location: str,
    *,
    today: date,
    repository_root: Path | None,
) -> tuple[Coordinate, dict[str, Any]]:
    observation_id = required_string(entry, "observation_id", location)
    if not OBSERVATION_ID_RE.fullmatch(observation_id):
        raise ValueError(
            f"{location}.observation_id must use 3-200 lowercase identifier characters"
        )
    coordinate = Coordinate.read(entry, location)
    metric = required_string(entry, "metric", location)
    environment = required_string(entry, "environment", location)
    if environment != "production":
        raise ValueError(f"{location}.environment must be production")
    window_started_on = iso_date(
        entry.get("window_started_on"), f"{location}.window_started_on"
    )
    window_ended_on = iso_date(
        entry.get("window_ended_on"), f"{location}.window_ended_on"
    )
    recorded_on = iso_date(entry.get("recorded_on"), f"{location}.recorded_on")
    if window_started_on > window_ended_on:
        raise ValueError(f"{location} observation window must not be inverted")
    if window_ended_on > recorded_on:
        raise ValueError(f"{location}.recorded_on must be on or after window_ended_on")
    if recorded_on > today:
        raise ValueError(
            f"{location}.recorded_on must not be later than {today.isoformat()}"
        )
    artifact_path = _safe_artifact_path(entry.get("artifact"), f"{location}.artifact")
    artifact_sha256 = required_string(entry, "artifact_sha256", location)
    if not SHA256_RE.fullmatch(artifact_sha256):
        raise ValueError(f"{location}.artifact_sha256 must be sha256:<64 lowercase hex>")
    if repository_root is None:
        raise ValueError(f"{location} requires repository_root to verify its artifact")
    artifact = _load_artifact(
        repository_root, artifact_path, artifact_sha256, location
    )
    if artifact.get("schema_version") != ARTIFACT_SCHEMA_VERSION:
        raise ValueError(
            f"{location} artifact must use {ARTIFACT_SCHEMA_VERSION}"
        )
    exact_fields = {
        "kind": coordinate.kind,
        "id": coordinate.contract_id,
        "version": coordinate.version,
        "metric": metric,
        "environment": environment,
        "window_started_on": window_started_on.isoformat(),
        "window_ended_on": window_ended_on.isoformat(),
    }
    for field, expected in exact_fields.items():
        if artifact.get(field) != expected:
            raise ValueError(
                f"{location} artifact {field} must equal observation value {expected!r}"
            )
    samples = artifact.get("samples")
    if not isinstance(samples, list) or not samples:
        raise ValueError(f"{location} artifact samples must be a non-empty list")
    expected_dates = []
    cursor = window_started_on
    while cursor <= window_ended_on:
        expected_dates.append(cursor)
        cursor += timedelta(days=1)
    if len(samples) != len(expected_dates):
        raise ValueError(
            f"{location} artifact must contain exactly one complete sample per day"
        )
    for index, (sample, expected_date) in enumerate(zip(samples, expected_dates)):
        sample_location = f"{location}.artifact.samples[{index}]"
        if not isinstance(sample, dict):
            raise ValueError(f"{sample_location} must be an object")
        observed_on = iso_date(sample.get("observed_on"), f"{sample_location}.observed_on")
        if observed_on != expected_date:
            raise ValueError(
                f"{sample_location}.observed_on must be {expected_date.isoformat()}"
            )
        usage_total = _non_negative_integer(
            sample.get("usage_total"), f"{sample_location}.usage_total"
        )
        if usage_total != 0:
            raise ValueError(
                f"{sample_location}.usage_total must be zero for retirement evidence"
            )
        if sample.get("complete") is not True:
            raise ValueError(f"{sample_location}.complete must be true")
    return coordinate, {
        "observation_id": observation_id,
        **coordinate.mapping(),
        "metric": metric,
        "environment": environment,
        "window_started_on": window_started_on.isoformat(),
        "window_ended_on": window_ended_on.isoformat(),
        "recorded_on": recorded_on.isoformat(),
        "artifact": artifact_path.as_posix(),
        "artifact_sha256": artifact_sha256,
    }


def normalize_retirement_evidence(
    evidence: dict[str, Any],
    *,
    today: date,
    repository_root: Path | None,
) -> tuple[dict[Coordinate, dict[str, Any]], list[str]]:
    errors: list[str] = []
    observations: dict[Coordinate, dict[str, Any]] = {}
    observation_ids: set[str] = set()
    if evidence.get("schema_version") != EVIDENCE_SCHEMA_VERSION:
        errors.append(
            f"contract retirement evidence must use {EVIDENCE_SCHEMA_VERSION}"
        )
    entries = evidence.get("observations")
    if not isinstance(entries, list):
        return observations, errors + ["retirement evidence observations must be a list"]
    for index, entry in enumerate(entries):
        location = f"retirement_evidence.observations[{index}]"
        try:
            if not isinstance(entry, dict):
                raise ValueError(f"{location} must be an object")
            coordinate, normalized = _normalize_observation(
                entry,
                location,
                today=today,
                repository_root=repository_root,
            )
            if normalized["observation_id"] in observation_ids:
                raise ValueError(
                    f"duplicate retirement observation id {normalized['observation_id']}"
                )
            if coordinate in observations:
                raise ValueError(
                    f"duplicate retirement observation for {coordinate.display()}"
                )
            observation_ids.add(normalized["observation_id"])
            observations[coordinate] = normalized
        except ValueError as error:
            errors.append(str(error))
    return observations, sorted(set(errors))


def validate_retirement_evidence(
    evidence: dict[str, Any],
    policy: dict[str, Any],
    *,
    base_evidence: dict[str, Any] | None = None,
    base_policy: dict[str, Any] | None = None,
    today: date | None = None,
    repository_root: Path | None = None,
) -> list[str]:
    """Return fail-closed zero-usage observation and transition violations."""

    as_of = today or date.today()
    observations, errors = normalize_retirement_evidence(
        evidence, today=as_of, repository_root=repository_root
    )
    overrides, _, policy_errors = validate_policy(policy)
    errors.extend(policy_errors)

    for coordinate, lifecycle in sorted(overrides.items()):
        zero_since = lifecycle["telemetry"]["zero_since"]
        observation = observations.get(coordinate)
        if zero_since is None:
            if observation is not None:
                errors.append(
                    f"{coordinate.display()} retirement observation requires telemetry.zero_since"
                )
            continue
        if observation is None:
            errors.append(
                f"{coordinate.display()} telemetry.zero_since lacks immutable retirement observation"
            )
            continue
        if observation["metric"] != lifecycle["telemetry"]["metric"]:
            errors.append(
                f"{coordinate.display()} retirement observation metric does not match lifecycle telemetry"
            )
        if observation["window_started_on"] != zero_since:
            errors.append(
                f"{coordinate.display()} telemetry.zero_since must equal observation window_started_on"
            )
        started = iso_date(observation["window_started_on"], "window_started_on")
        ended = iso_date(observation["window_ended_on"], "window_ended_on")
        lookback_days = lifecycle["telemetry"]["lookback_days"]
        if (ended - started).days < lookback_days:
            errors.append(
                f"{coordinate.display()} retirement observation must prove at least "
                f"{lookback_days} days of zero usage"
            )
        if lifecycle["state"] == "retired":
            retired_on = iso_date(lifecycle["retired_on"], "retired_on")
            if ended > retired_on:
                errors.append(
                    f"{coordinate.display()} retirement observation must end on or before retired_on"
                )

    for coordinate in sorted(set(observations) - set(overrides)):
        errors.append(
            f"retirement observation references unmanaged lifecycle {coordinate.display()}"
        )

    if base_evidence is not None:
        base_observations, base_errors = normalize_retirement_evidence(
            base_evidence, today=as_of, repository_root=repository_root
        )
        errors.extend(f"base: {error}" for error in base_errors)
        for coordinate, previous in sorted(base_observations.items()):
            current = observations.get(coordinate)
            if current is None:
                errors.append(
                    f"retirement observation must remain permanent: {coordinate.display()}"
                )
            elif current != previous:
                errors.append(
                    f"retirement observation is immutable: {coordinate.display()}"
                )
        if base_policy is not None:
            base_overrides, _, base_policy_errors = validate_policy(base_policy)
            errors.extend(f"base: {error}" for error in base_policy_errors)
            for coordinate in sorted(set(observations) - set(base_observations)):
                previous = base_overrides.get(coordinate)
                if previous is not None and previous["state"] == "retired":
                    errors.append(
                        f"retirement observation cannot be added after retirement: {coordinate.display()}"
                    )

    return sorted(set(errors))
