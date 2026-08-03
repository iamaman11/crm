from __future__ import annotations

from copy import deepcopy
from datetime import date, timedelta
import hashlib
import json
from pathlib import Path
import tempfile
import unittest

from scripts.contract_lifecycle_preproduction import install

install()

from scripts.contract_retirement_evidence import validate_retirement_evidence


TODAY = date(2026, 4, 15)
COORDINATE = ("capability", "example.create", "1.0.0")


def policy(*, zero_since: str | None = None, state: str = "deprecated") -> dict:
    entry = {
        "kind": COORDINATE[0],
        "id": COORDINATE[1],
        "version": COORDINATE[2],
        "state": state,
        "owner": "crm.example",
        "deprecated_on": "2026-01-01",
        "removal_not_before": "2026-03-02",
        "replacement": {
            "kind": "capability",
            "id": "example.create.v2",
            "version": "1.0.0",
        },
        "migration": {
            "issue": 123,
            "guide": "docs/migrations/example-create-v2.md",
            "completed_on": "2026-02-01",
        },
        "telemetry": {
            "metric": "crm_deprecated_capability_usage_total",
            "lookback_days": 30,
            "zero_since": zero_since,
        },
    }
    if state == "retired":
        entry["retired_on"] = "2026-04-15"
    return {
        "schema_version": "crm.contract-lifecycle-policy/v1",
        "minimum_deprecation_days": 30,
        "contracts": [entry],
        "external_consumers": [],
    }


def empty_evidence() -> dict:
    return {
        "schema_version": "crm.contract-retirement-evidence/v1",
        "observations": [],
    }


def write_observation(
    root: Path,
    *,
    started: date = date(2026, 3, 1),
    ended: date = date(2026, 3, 31),
    nonzero_on: date | None = None,
    omit_on: date | None = None,
) -> dict:
    samples = []
    cursor = started
    while cursor <= ended:
        if cursor != omit_on:
            samples.append(
                {
                    "observed_on": cursor.isoformat(),
                    "usage_total": 1 if cursor == nonzero_on else 0,
                    "complete": True,
                }
            )
        cursor += timedelta(days=1)
    artifact = {
        "schema_version": "crm.contract-usage-observation/v1",
        "kind": COORDINATE[0],
        "id": COORDINATE[1],
        "version": COORDINATE[2],
        "metric": "crm_deprecated_capability_usage_total",
        "environment": "production",
        "window_started_on": started.isoformat(),
        "window_ended_on": ended.isoformat(),
        "samples": samples,
    }
    path = Path("evidence/contract-lifecycle/example-create-1.0.0.json")
    absolute = root / path
    absolute.parent.mkdir(parents=True, exist_ok=True)
    content = (json.dumps(artifact, sort_keys=True, separators=(",", ":")) + "\n").encode()
    absolute.write_bytes(content)
    return {
        "schema_version": "crm.contract-retirement-evidence/v1",
        "observations": [
            {
                "observation_id": "example-create-1.0.0-production-2026-03",
                "kind": COORDINATE[0],
                "id": COORDINATE[1],
                "version": COORDINATE[2],
                "metric": "crm_deprecated_capability_usage_total",
                "environment": "production",
                "window_started_on": started.isoformat(),
                "window_ended_on": ended.isoformat(),
                "recorded_on": ended.isoformat(),
                "artifact": path.as_posix(),
                "artifact_sha256": f"sha256:{hashlib.sha256(content).hexdigest()}",
            }
        ],
    }


class ContractRetirementEvidenceTests(unittest.TestCase):
    def test_committed_evidence_is_empty_until_real_observation_exists(self) -> None:
        root = Path(__file__).resolve().parents[1]
        evidence = json.loads(
            (root / "contracts/contract-retirement-evidence.json").read_text()
        )
        lifecycle_policy = json.loads(
            (root / "contracts/contract-lifecycle-policy.json").read_text()
        )
        self.assertEqual(evidence, empty_evidence())
        self.assertEqual(
            validate_retirement_evidence(
                evidence,
                lifecycle_policy,
                today=date(2026, 8, 3),
                repository_root=root,
            ),
            [],
        )

    def test_valid_complete_zero_window_is_accepted(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            evidence = write_observation(root)
            errors = validate_retirement_evidence(
                evidence,
                policy(zero_since="2026-03-01", state="retired"),
                today=TODAY,
                repository_root=root,
            )
        self.assertEqual(errors, [])

    def test_zero_since_and_observation_must_exist_together(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            evidence = write_observation(root)
            without_date = validate_retirement_evidence(
                evidence,
                policy(),
                today=TODAY,
                repository_root=root,
            )
            without_observation = validate_retirement_evidence(
                empty_evidence(),
                policy(zero_since="2026-03-01"),
                today=TODAY,
                repository_root=root,
            )
        self.assertTrue(any("requires telemetry.zero_since" in error for error in without_date))
        self.assertTrue(any("lacks immutable retirement observation" in error for error in without_observation))

    def test_nonzero_incomplete_and_tampered_artifacts_fail_closed(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            nonzero = write_observation(root, nonzero_on=date(2026, 3, 20))
            errors = validate_retirement_evidence(
                nonzero,
                policy(zero_since="2026-03-01"),
                today=TODAY,
                repository_root=root,
            )
            self.assertTrue(any("usage_total must be zero" in error for error in errors))

            incomplete = write_observation(root, omit_on=date(2026, 3, 20))
            errors = validate_retirement_evidence(
                incomplete,
                policy(zero_since="2026-03-01"),
                today=TODAY,
                repository_root=root,
            )
            self.assertTrue(any("one complete sample per day" in error for error in errors))

            valid = write_observation(root)
            artifact = root / valid["observations"][0]["artifact"]
            artifact.write_text("{}\n", encoding="utf-8")
            errors = validate_retirement_evidence(
                valid,
                policy(zero_since="2026-03-01"),
                today=TODAY,
                repository_root=root,
            )
            self.assertTrue(any("artifact digest mismatch" in error for error in errors))

    def test_window_metric_environment_and_dates_are_exact(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            evidence = write_observation(
                root,
                started=date(2026, 3, 2),
                ended=date(2026, 3, 31),
            )
            errors = validate_retirement_evidence(
                evidence,
                policy(zero_since="2026-03-01"),
                today=TODAY,
                repository_root=root,
            )
            self.assertTrue(any("zero_since must equal" in error for error in errors))
            self.assertTrue(any("at least 30 days" in error for error in errors))

            staging = deepcopy(evidence)
            staging["observations"][0]["environment"] = "staging"
            errors = validate_retirement_evidence(
                staging,
                policy(zero_since="2026-03-01"),
                today=TODAY,
                repository_root=root,
            )
            self.assertTrue(any("environment must be production" in error for error in errors))

            future = deepcopy(evidence)
            future["observations"][0]["recorded_on"] = "2026-04-16"
            errors = validate_retirement_evidence(
                future,
                policy(zero_since="2026-03-01"),
                today=TODAY,
                repository_root=root,
            )
            self.assertTrue(any("recorded_on must not be later" in error for error in errors))

    def test_evidence_is_append_only_and_cannot_arrive_after_retirement(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            base = write_observation(root)
            current = deepcopy(base)
            current["observations"][0]["observation_id"] = "rewritten-observation-id"
            errors = validate_retirement_evidence(
                current,
                policy(zero_since="2026-03-01", state="retired"),
                base_evidence=base,
                base_policy=policy(zero_since="2026-03-01", state="retired"),
                today=TODAY,
                repository_root=root,
            )
            self.assertTrue(any("observation is immutable" in error for error in errors))

            errors = validate_retirement_evidence(
                base,
                policy(zero_since="2026-03-01", state="retired"),
                base_evidence=empty_evidence(),
                base_policy=policy(zero_since="2026-03-01", state="retired"),
                today=TODAY,
                repository_root=root,
            )
            self.assertTrue(any("cannot be added after retirement" in error for error in errors))


if __name__ == "__main__":
    unittest.main()
