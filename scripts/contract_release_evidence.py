#!/usr/bin/env python3
"""Immutable evidence for contracts that were never externally released."""

from __future__ import annotations

import argparse
from datetime import date
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys
import tomllib
from typing import Any

import contract_lifecycle as core
from contract_lifecycle_preproduction import NEVER_RELEASED_MODE, validate_policy

EVIDENCE_SCHEMA_VERSION = "crm.contract-release-evidence/v1"
ARTIFACT_SCHEMA_VERSION = "crm.contract-never-released-observation/v1"
CLASSIFICATION = "never_externally_released"
SAFE_ARTIFACT_ROOT = Path("evidence/contract-lifecycle")
EVIDENCE_ID_RE = re.compile(r"^[a-z0-9][a-z0-9._-]{2,199}$")
SHA256_RE = re.compile(r"^sha256:[0-9a-f]{64}$")
COMMIT_RE = re.compile(r"^[0-9a-f]{40}$")
REPOSITORY_RE = re.compile(r"^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$")


def _load_json(path: Path, label: str) -> Any:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise ValueError(f"cannot read {label} {path}: {error}") from error


def _safe_artifact_path(value: Any, location: str) -> Path:
    text = core.required_string({"value": value}, "value", location)
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


def _artifact(
    repository_root: Path,
    path: Path,
    expected_sha256: str,
    location: str,
) -> dict[str, Any]:
    absolute = repository_root.resolve() / path
    try:
        content = absolute.read_bytes()
    except OSError as error:
        raise ValueError(f"{location} cannot read artifact {path}: {error}") from error
    actual = f"sha256:{hashlib.sha256(content).hexdigest()}"
    if actual != expected_sha256:
        raise ValueError(
            f"{location} artifact digest mismatch for {path}: expected {expected_sha256}, got {actual}"
        )
    try:
        value = json.loads(content)
    except json.JSONDecodeError as error:
        raise ValueError(f"{location} artifact {path} is invalid JSON: {error}") from error
    if not isinstance(value, dict):
        raise ValueError(f"{location} artifact {path} must contain an object")
    return value


def _package_publication(
    repository_root: Path, entries: Any, location: str
) -> list[dict[str, Any]]:
    if not isinstance(entries, list) or not entries:
        raise ValueError(f"{location} must be a non-empty list")
    normalized: list[dict[str, Any]] = []
    seen: set[str] = set()
    for index, entry in enumerate(entries):
        item_location = f"{location}[{index}]"
        if not isinstance(entry, dict):
            raise ValueError(f"{item_location} must be an object")
        path_text = core.required_string(entry, "path", item_location)
        path = Path(path_text)
        if path.is_absolute() or ".." in path.parts or path.name != "Cargo.toml":
            raise ValueError(f"{item_location}.path must be a safe Cargo.toml path")
        if path_text in seen:
            raise ValueError(f"duplicate package publication path {path_text}")
        seen.add(path_text)
        if entry.get("publish") is not False:
            raise ValueError(f"{item_location}.publish must be false")
        try:
            package = tomllib.loads(
                (repository_root.resolve() / path).read_text(encoding="utf-8")
            )["package"]
        except (OSError, tomllib.TOMLDecodeError, KeyError, TypeError) as error:
            raise ValueError(f"{item_location} cannot verify package metadata: {error}") from error
        if package.get("publish") is not False:
            raise ValueError(f"{path_text} must declare package.publish = false")
        normalized.append({"path": path_text, "publish": False})
    return sorted(normalized, key=lambda item: item["path"])


def _normalize_observation(
    entry: dict[str, Any],
    location: str,
    *,
    today: date,
    repository_root: Path,
) -> tuple[core.Coordinate, dict[str, Any], dict[str, Any]]:
    evidence_id = core.required_string(entry, "evidence_id", location)
    if not EVIDENCE_ID_RE.fullmatch(evidence_id):
        raise ValueError(f"{location}.evidence_id must use 3-200 lowercase identifier characters")
    coordinate = core.Coordinate.read(entry, location)
    classification = core.required_string(entry, "classification", location)
    if classification != CLASSIFICATION:
        raise ValueError(f"{location}.classification must be {CLASSIFICATION}")
    recorded_on = core.iso_date(entry.get("recorded_on"), f"{location}.recorded_on")
    if recorded_on > today:
        raise ValueError(f"{location}.recorded_on must not be later than {today.isoformat()}")
    artifact_path = _safe_artifact_path(entry.get("artifact"), f"{location}.artifact")
    artifact_sha256 = core.required_string(entry, "artifact_sha256", location)
    if not SHA256_RE.fullmatch(artifact_sha256):
        raise ValueError(f"{location}.artifact_sha256 must be sha256:<64 lowercase hex>")
    artifact = _artifact(repository_root, artifact_path, artifact_sha256, location)
    if artifact.get("schema_version") != ARTIFACT_SCHEMA_VERSION:
        raise ValueError(f"{location} artifact must use {ARTIFACT_SCHEMA_VERSION}")
    for field, expected in {
        "kind": coordinate.kind,
        "id": coordinate.contract_id,
        "version": coordinate.version,
        "observed_on": recorded_on.isoformat(),
    }.items():
        if artifact.get(field) != expected:
            raise ValueError(f"{location} artifact {field} must equal {expected!r}")
    repository = core.required_string(artifact, "repository", f"{location}.artifact")
    if not REPOSITORY_RE.fullmatch(repository):
        raise ValueError(f"{location} artifact repository must be owner/name")
    source_commit = core.required_string(artifact, "source_commit", f"{location}.artifact")
    if not COMMIT_RE.fullmatch(source_commit):
        raise ValueError(f"{location} artifact source_commit must be a lowercase 40-hex SHA")
    github = artifact.get("github")
    if not isinstance(github, dict):
        raise ValueError(f"{location} artifact github must be an object")
    for channel in ("releases", "tags", "deployments"):
        value = github.get(channel)
        if value != []:
            raise ValueError(f"{location} artifact github.{channel} must be an empty list")
    packages = _package_publication(
        repository_root,
        artifact.get("package_publication"),
        f"{location}.artifact.package_publication",
    )
    attestation = artifact.get("owner_attestation")
    if not isinstance(attestation, dict):
        raise ValueError(f"{location} artifact owner_attestation must be an object")
    account = core.required_string(attestation, "account", f"{location}.artifact.owner_attestation")
    issue = core.required_integer(
        attestation, "issue", f"{location}.artifact.owner_attestation", 1
    )
    statement = core.required_string(
        attestation, "statement", f"{location}.artifact.owner_attestation"
    )
    normalized = {
        "evidence_id": evidence_id,
        **coordinate.mapping(),
        "classification": classification,
        "recorded_on": recorded_on.isoformat(),
        "artifact": artifact_path.as_posix(),
        "artifact_sha256": artifact_sha256,
    }
    normalized_artifact = {
        "repository": repository,
        "source_commit": source_commit,
        "github": {"releases": [], "tags": [], "deployments": []},
        "package_publication": packages,
        "owner_attestation": {
            "account": account,
            "issue": issue,
            "statement": statement,
        },
    }
    return coordinate, normalized, normalized_artifact


def normalize_release_evidence(
    evidence: dict[str, Any],
    *,
    today: date,
    repository_root: Path,
) -> tuple[
    dict[core.Coordinate, dict[str, Any]],
    dict[str, dict[str, Any]],
    list[str],
]:
    observations: dict[core.Coordinate, dict[str, Any]] = {}
    artifacts: dict[str, dict[str, Any]] = {}
    errors: list[str] = []
    if evidence.get("schema_version") != EVIDENCE_SCHEMA_VERSION:
        errors.append(f"contract release evidence must use {EVIDENCE_SCHEMA_VERSION}")
    entries = evidence.get("observations")
    if not isinstance(entries, list):
        return observations, artifacts, errors + ["release evidence observations must be a list"]
    evidence_ids: set[str] = set()
    for index, entry in enumerate(entries):
        location = f"release_evidence.observations[{index}]"
        try:
            if not isinstance(entry, dict):
                raise ValueError(f"{location} must be an object")
            coordinate, normalized, artifact = _normalize_observation(
                entry,
                location,
                today=today,
                repository_root=repository_root,
            )
            evidence_id = normalized["evidence_id"]
            if evidence_id in evidence_ids:
                raise ValueError(f"duplicate release evidence id {evidence_id}")
            if coordinate in observations:
                raise ValueError(f"duplicate release evidence for {coordinate.display()}")
            evidence_ids.add(evidence_id)
            observations[coordinate] = normalized
            artifacts[evidence_id] = artifact
        except ValueError as error:
            errors.append(str(error))
    return observations, artifacts, sorted(set(errors))


def validate_release_evidence(
    evidence: dict[str, Any],
    policy: dict[str, Any],
    *,
    base_evidence: dict[str, Any] | None = None,
    base_policy: dict[str, Any] | None = None,
    today: date | None = None,
    repository_root: Path | None = None,
) -> list[str]:
    """Return fail-closed never-released evidence and transition violations."""

    root = repository_root or Path(".")
    as_of = today or date.today()
    observations, _, errors = normalize_release_evidence(
        evidence, today=as_of, repository_root=root
    )
    overrides, external, policy_errors = validate_policy(policy)
    errors.extend(policy_errors)
    external_coordinates = {
        core.Coordinate.read(item, "external consumer") for item in external
    }

    for coordinate, lifecycle in sorted(overrides.items()):
        retirement = lifecycle.get("retirement")
        observation = observations.get(coordinate)
        if retirement is None or retirement.get("mode") != NEVER_RELEASED_MODE:
            if observation is not None:
                errors.append(
                    f"{coordinate.display()} never-released evidence requires matching retirement mode"
                )
            continue
        if observation is None:
            errors.append(
                f"{coordinate.display()} never-released retirement lacks immutable release evidence"
            )
            continue
        if observation["evidence_id"] != retirement.get("evidence_id"):
            errors.append(
                f"{coordinate.display()} retirement evidence_id does not match release evidence"
            )
        if coordinate in external_coordinates:
            errors.append(
                f"{coordinate.display()} never-released retirement cannot have external consumer records"
            )
        if lifecycle["telemetry"]["zero_since"] is not None:
            errors.append(
                f"{coordinate.display()} never-released retirement must not fabricate zero_since"
            )

    for coordinate in sorted(set(observations) - set(overrides)):
        errors.append(
            f"release evidence references unmanaged lifecycle {coordinate.display()}"
        )

    if base_evidence is not None:
        base_observations, _, base_errors = normalize_release_evidence(
            base_evidence, today=as_of, repository_root=root
        )
        errors.extend(f"base: {error}" for error in base_errors)
        for coordinate, previous in sorted(base_observations.items()):
            current = observations.get(coordinate)
            if current is None:
                errors.append(f"release evidence must remain permanent: {coordinate.display()}")
            elif current != previous:
                errors.append(f"release evidence is immutable: {coordinate.display()}")
        if base_policy is not None:
            base_overrides, _, base_policy_errors = validate_policy(base_policy)
            errors.extend(f"base: {error}" for error in base_policy_errors)
            for coordinate in sorted(set(observations) - set(base_observations)):
                previous = base_overrides.get(coordinate)
                current = overrides.get(coordinate)
                if previous is None or previous["state"] != "deprecated":
                    errors.append(
                        f"new never-released evidence requires deprecated base history: {coordinate.display()}"
                    )
                if current is None or current["state"] != "retired":
                    errors.append(
                        f"new never-released evidence requires retired current lifecycle: {coordinate.display()}"
                    )
    return sorted(set(errors))


def _git_json(base_ref: str, path: Path) -> dict[str, Any]:
    completed = subprocess.run(
        ["git", "show", f"{base_ref}:{path.as_posix()}"],
        check=False,
        text=True,
        capture_output=True,
    )
    if completed.returncode != 0:
        details = (completed.stdout + completed.stderr).strip()
        if "does not exist" in details or "exists on disk, but not in" in details:
            return {"schema_version": EVIDENCE_SCHEMA_VERSION, "observations": []}
        raise ValueError(f"cannot read {path} from {base_ref}: {details}")
    value = json.loads(completed.stdout)
    if not isinstance(value, dict):
        raise ValueError(f"{base_ref}:{path} must contain an object")
    return value


def check_live(
    evidence_path: Path,
    *,
    base_ref: str,
    releases_path: Path,
    tags_path: Path,
    deployments_path: Path,
    expected_repository: str,
    expected_source_commit: str,
    repository_root: Path,
) -> list[str]:
    current = _load_json(evidence_path, "contract release evidence")
    base = _git_json(base_ref, evidence_path)
    observations, artifacts, errors = normalize_release_evidence(
        current, today=date.today(), repository_root=repository_root
    )
    base_observations, _, base_errors = normalize_release_evidence(
        base, today=date.today(), repository_root=repository_root
    )
    errors.extend(f"base: {error}" for error in base_errors)
    live = {
        "releases": _load_json(releases_path, "GitHub releases snapshot"),
        "tags": _load_json(tags_path, "GitHub tags snapshot"),
        "deployments": _load_json(deployments_path, "GitHub deployments snapshot"),
    }
    for channel, value in live.items():
        if not isinstance(value, list):
            errors.append(f"live GitHub {channel} snapshot must be a list")
    for coordinate, observation in sorted(observations.items()):
        if coordinate in base_observations:
            continue
        artifact = artifacts[observation["evidence_id"]]
        if artifact["repository"] != expected_repository:
            errors.append(
                f"{coordinate.display()} release evidence repository must be {expected_repository}"
            )
        if artifact["source_commit"] != expected_source_commit:
            errors.append(
                f"{coordinate.display()} release evidence source_commit must be {expected_source_commit}"
            )
        for channel, value in live.items():
            if value != []:
                errors.append(
                    f"{coordinate.display()} cannot use never-released retirement; GitHub {channel} is not empty"
                )
    return sorted(set(errors))


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check-live", action="store_true")
    parser.add_argument(
        "--evidence", type=Path, default=Path("contracts/contract-release-evidence.json")
    )
    parser.add_argument("--base-ref")
    parser.add_argument("--releases", type=Path)
    parser.add_argument("--tags", type=Path)
    parser.add_argument("--deployments", type=Path)
    parser.add_argument("--repository")
    parser.add_argument("--source-commit")
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    if not args.check_live:
        print("--check-live is required", file=sys.stderr)
        return 2
    required = {
        "--base-ref": args.base_ref,
        "--releases": args.releases,
        "--tags": args.tags,
        "--deployments": args.deployments,
        "--repository": args.repository,
        "--source-commit": args.source_commit,
    }
    missing = [name for name, value in required.items() if value is None]
    if missing:
        print(f"missing required arguments: {', '.join(missing)}", file=sys.stderr)
        return 2
    try:
        errors = check_live(
            args.evidence,
            base_ref=args.base_ref,
            releases_path=args.releases,
            tags_path=args.tags,
            deployments_path=args.deployments,
            expected_repository=args.repository,
            expected_source_commit=args.source_commit,
            repository_root=Path("."),
        )
    except (ValueError, json.JSONDecodeError) as error:
        print(f"contract release evidence check failed: {error}", file=sys.stderr)
        return 1
    if errors:
        print("contract release evidence check failed:", file=sys.stderr)
        for error in errors:
            print(f"- {error}", file=sys.stderr)
        return 1
    print("contract release evidence verified against live GitHub release channels")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
