from __future__ import annotations

from copy import deepcopy
from datetime import date
import hashlib
import json
from pathlib import Path
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))

from scripts.contract_release_evidence import validate_release_evidence


COORDINATE = {
    "kind": "capability",
    "id": "example.create",
    "version": "1.0.0",
}


def policy() -> dict:
    return {
        "schema_version": "crm.contract-lifecycle-policy/v1",
        "minimum_deprecation_days": 30,
        "contracts": [
            {
                **COORDINATE,
                "state": "retired",
                "owner": "crm.example",
                "deprecated_on": "2026-08-03",
                "removal_not_before": "2026-09-02",
                "replacement": {
                    "kind": "capability",
                    "id": "example.create",
                    "version": "1.1.0",
                },
                "migration": {
                    "issue": 123,
                    "guide": "docs/migrations/example.md",
                    "completed_on": "2026-08-03",
                },
                "telemetry": {
                    "metric": "crm_deprecated_capability_usage_total",
                    "lookback_days": 30,
                    "zero_since": None,
                },
                "retirement": {
                    "mode": "never_externally_released",
                    "evidence_id": "example-create-1.0.0-never-released",
                },
                "retired_on": "2026-08-03",
            }
        ],
        "external_consumers": [],
    }


def write_evidence(root: Path) -> dict:
    for relative in (
        Path("modules/crm-example/Cargo.toml"),
        Path("crates/crm-example-adapter/Cargo.toml"),
    ):
        path = root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(
            '[package]\nname = "example"\nversion = "0.1.0"\npublish = false\n',
            encoding="utf-8",
        )
    artifact = {
        "schema_version": "crm.contract-never-released-observation/v1",
        "repository": "example/crm",
        "observed_on": "2026-08-03",
        "source_commit": "a" * 40,
        **COORDINATE,
        "github": {"releases": [], "tags": [], "deployments": []},
        "package_publication": [
            {"path": "modules/crm-example/Cargo.toml", "publish": False},
            {"path": "crates/crm-example-adapter/Cargo.toml", "publish": False},
        ],
        "owner_attestation": {
            "account": "example",
            "issue": 123,
            "statement": "Never externally released.",
        },
    }
    path = Path("evidence/contract-lifecycle/example-create-never-released.json")
    absolute = root / path
    absolute.parent.mkdir(parents=True, exist_ok=True)
    content = (json.dumps(artifact, indent=2) + "\n").encode()
    absolute.write_bytes(content)
    return {
        "schema_version": "crm.contract-release-evidence/v1",
        "observations": [
            {
                "evidence_id": "example-create-1.0.0-never-released",
                **COORDINATE,
                "classification": "never_externally_released",
                "recorded_on": "2026-08-03",
                "artifact": path.as_posix(),
                "artifact_sha256": f"sha256:{hashlib.sha256(content).hexdigest()}",
            }
        ],
    }


class ContractReleaseEvidenceTests(unittest.TestCase):
    def test_valid_never_released_evidence_is_accepted(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            evidence = write_evidence(root)
            errors = validate_release_evidence(
                evidence,
                policy(),
                today=date(2026, 8, 3),
                repository_root=root,
            )
        self.assertEqual(errors, [])

    def test_nonempty_release_channel_fails_closed(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            evidence = write_evidence(root)
            artifact = root / evidence["observations"][0]["artifact"]
            value = json.loads(artifact.read_text())
            value["github"]["tags"] = [{"name": "v0.1.0"}]
            content = (json.dumps(value, indent=2) + "\n").encode()
            artifact.write_bytes(content)
            evidence["observations"][0]["artifact_sha256"] = (
                f"sha256:{hashlib.sha256(content).hexdigest()}"
            )
            errors = validate_release_evidence(
                evidence,
                policy(),
                today=date(2026, 8, 3),
                repository_root=root,
            )
        self.assertTrue(any("github.tags must be an empty list" in error for error in errors))

    def test_digest_tampering_and_publishable_package_fail_closed(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            evidence = write_evidence(root)
            artifact = root / evidence["observations"][0]["artifact"]
            artifact.write_text("{}\n", encoding="utf-8")
            errors = validate_release_evidence(
                evidence,
                policy(),
                today=date(2026, 8, 3),
                repository_root=root,
            )
            self.assertTrue(any("artifact digest mismatch" in error for error in errors))

            evidence = write_evidence(root)
            package = root / "modules/crm-example/Cargo.toml"
            package.write_text(
                '[package]\nname = "example"\nversion = "0.1.0"\npublish = true\n',
                encoding="utf-8",
            )
            errors = validate_release_evidence(
                evidence,
                policy(),
                today=date(2026, 8, 3),
                repository_root=root,
            )
            self.assertTrue(any("package.publish = false" in error for error in errors))

    def test_policy_and_evidence_must_match_exactly(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            evidence = write_evidence(root)
            mismatched = policy()
            mismatched["contracts"][0]["retirement"]["evidence_id"] = "other-evidence"
            errors = validate_release_evidence(
                evidence,
                mismatched,
                today=date(2026, 8, 3),
                repository_root=root,
            )
        self.assertTrue(any("evidence_id does not match" in error for error in errors))

    def test_external_consumer_record_disqualifies_never_released_path(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            evidence = write_evidence(root)
            current = policy()
            current["external_consumers"] = [
                {
                    **COORDINATE,
                    "consumer_id": "external.example",
                    "owner": "example",
                    "state": "migrated",
                    "migration_issue": 123,
                    "migrated_on": "2026-08-03",
                    "last_seen_on": None,
                }
            ]
            errors = validate_release_evidence(
                evidence,
                current,
                today=date(2026, 8, 3),
                repository_root=root,
            )
        self.assertTrue(any("cannot have external consumer records" in error for error in errors))

    def test_release_evidence_is_append_only_and_requires_deprecated_base(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            evidence = write_evidence(root)
            rewritten = deepcopy(evidence)
            rewritten["observations"][0]["recorded_on"] = "2026-08-02"
            errors = validate_release_evidence(
                rewritten,
                policy(),
                base_evidence=evidence,
                base_policy=policy(),
                today=date(2026, 8, 3),
                repository_root=root,
            )
            self.assertTrue(any("release evidence is immutable" in error for error in errors))

            base_policy = policy()
            base_policy["contracts"][0]["state"] = "retired"
            errors = validate_release_evidence(
                evidence,
                policy(),
                base_evidence={
                    "schema_version": "crm.contract-release-evidence/v1",
                    "observations": [],
                },
                base_policy=base_policy,
                today=date(2026, 8, 3),
                repository_root=root,
            )
            self.assertTrue(any("requires deprecated base history" in error for error in errors))


if __name__ == "__main__":
    unittest.main()
