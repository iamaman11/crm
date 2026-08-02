from __future__ import annotations

from copy import deepcopy
from datetime import date
from pathlib import Path
import tempfile
import unittest

from scripts.contract_lifecycle_transitions import validate_transition_integrity


TODAY = date(2026, 8, 2)


def capability(
    contract_id: str,
    *,
    version: str = "1.0.0",
    rpc: str | None = None,
) -> dict:
    stem = "".join(part.title() for part in contract_id.replace("-", ".").split("."))
    return {
        "id": contract_id,
        "version": version,
        "rpc": rpc or f"crm.example.v1.ExampleService.{stem}",
        "request": f"crm.example.v1.{stem}Request",
        "response": f"crm.example.v1.{stem}Response",
    }


def bindings(*, provider: str = "crm.example", legacy_rpc: str | None = None) -> dict:
    return {
        "schema_version": "crm.contract-bindings/v1",
        "modules": [
            {
                "module_id": provider,
                "capabilities": [
                    capability("example.create", rpc=legacy_rpc),
                    capability("example.create.v2"),
                ],
                "events": [],
            }
        ],
    }


def empty_policy(*, minimum_days: int = 30) -> dict:
    return {
        "schema_version": "crm.contract-lifecycle-policy/v1",
        "minimum_deprecation_days": minimum_days,
        "contracts": [],
        "external_consumers": [],
    }


def lifecycle(state: str = "deprecated") -> dict:
    entry = {
        "kind": "capability",
        "id": "example.create",
        "version": "1.0.0",
        "state": state,
        "owner": "contract-platform",
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
            "completed_on": None,
        },
        "telemetry": {
            "metric": "crm_contract_invocations_total",
            "lookback_days": 30,
            "zero_since": None,
        },
    }
    if state == "retired":
        entry["retired_on"] = "2026-04-15"
        entry["migration"]["completed_on"] = "2026-03-10"
        entry["telemetry"]["zero_since"] = "2026-03-01"
    return entry


def external(
    *,
    state: str = "active",
    owner: str = "sdk-platform",
    issue: int | None = 124,
    migrated_on: str | None = None,
    last_seen_on: str | None = "2026-02-01",
) -> dict:
    return {
        "consumer_id": "public-sdk-v1",
        "owner": owner,
        "kind": "capability",
        "id": "example.create",
        "version": "1.0.0",
        "state": state,
        "migration_issue": issue,
        "migrated_on": migrated_on,
        "last_seen_on": last_seen_on,
    }


class ContractLifecycleTransitionTests(unittest.TestCase):
    def test_valid_deprecation_to_retirement_transition_is_accepted(self) -> None:
        base = empty_policy(minimum_days=60)
        base["contracts"] = [lifecycle()]
        base["external_consumers"] = [external()]
        current = deepcopy(base)
        current["contracts"] = [lifecycle("retired")]
        current["external_consumers"] = [
            external(
                state="migrated",
                migrated_on="2026-03-15",
                last_seen_on="2026-02-15",
            )
        ]
        errors = validate_transition_integrity(
            bindings(),
            current,
            base_bindings=bindings(),
            base_policy=base,
            today=TODAY,
        )
        self.assertEqual(errors, [])

    def test_published_provider_and_binding_are_immutable(self) -> None:
        changed = bindings(provider="crm.other", legacy_rpc="crm.other.v1.Other.Create")
        errors = validate_transition_integrity(
            changed,
            empty_policy(),
            base_bindings=bindings(),
            base_policy=empty_policy(),
            today=TODAY,
        )
        self.assertTrue(any("published provider cannot change" in error for error in errors))
        self.assertTrue(any("published binding cannot change" in error for error in errors))

    def test_external_consumer_record_cannot_disappear(self) -> None:
        base = empty_policy()
        base["external_consumers"] = [external()]
        errors = validate_transition_integrity(
            bindings(),
            empty_policy(),
            base_bindings=bindings(),
            base_policy=base,
            today=TODAY,
        )
        self.assertTrue(any("lifecycle record must remain permanent" in error for error in errors))

    def test_new_external_consumer_cannot_arrive_already_migrated(self) -> None:
        current = empty_policy()
        current["external_consumers"] = [
            external(state="migrated", migrated_on="2026-03-15")
        ]
        errors = validate_transition_integrity(
            bindings(),
            current,
            base_bindings=bindings(),
            base_policy=empty_policy(),
            today=TODAY,
        )
        self.assertTrue(any("lacks active base history" in error for error in errors))

    def test_external_owner_issue_and_observation_evidence_cannot_regress(self) -> None:
        base = empty_policy()
        base["external_consumers"] = [external(last_seen_on="2026-03-01")]
        current = empty_policy()
        current["external_consumers"] = [
            external(owner="other-team", issue=None, last_seen_on="2026-02-01")
        ]
        errors = validate_transition_integrity(
            bindings(),
            current,
            base_bindings=bindings(),
            base_policy=base,
            today=TODAY,
        )
        self.assertTrue(any("owner cannot change" in error for error in errors))
        self.assertTrue(any("cannot clear or change its migration issue" in error for error in errors))
        self.assertTrue(any("last_seen_on cannot regress" in error for error in errors))

    def test_deprecation_governance_and_lookback_cannot_be_weakened(self) -> None:
        base = empty_policy(minimum_days=60)
        base_entry = lifecycle()
        base["contracts"] = [base_entry]
        current = empty_policy(minimum_days=30)
        current_entry = deepcopy(base_entry)
        current_entry["migration"]["issue"] = 999
        current_entry["migration"]["guide"] = "docs/migrations/other.md"
        current_entry["telemetry"]["metric"] = "other_metric_total"
        current_entry["telemetry"]["lookback_days"] = 7
        current["contracts"] = [current_entry]
        errors = validate_transition_integrity(
            bindings(),
            current,
            base_bindings=bindings(),
            base_policy=base,
            today=TODAY,
        )
        for marker in (
            "migration.issue",
            "migration.guide",
            "telemetry.metric",
            "telemetry.lookback_days cannot decrease",
            "minimum_deprecation_days cannot decrease",
        ):
            self.assertTrue(any(marker in error for error in errors), marker)

    def test_completed_migration_and_zero_usage_evidence_cannot_be_rewritten(self) -> None:
        base = empty_policy()
        base_entry = lifecycle("retired")
        base["contracts"] = [base_entry]
        current = deepcopy(base)
        current["contracts"][0]["migration"]["completed_on"] = "2026-03-11"
        current["contracts"][0]["telemetry"]["zero_since"] = "2026-03-02"
        errors = validate_transition_integrity(
            bindings(),
            current,
            base_bindings=bindings(),
            base_policy=base,
            today=TODAY,
        )
        self.assertTrue(any("retired lifecycle tombstone is immutable" in error for error in errors))

    def test_future_dated_evidence_is_rejected(self) -> None:
        current = empty_policy()
        entry = lifecycle()
        entry["deprecated_on"] = "2026-08-03"
        entry["removal_not_before"] = "2026-09-02"
        current["contracts"] = [entry]
        current["external_consumers"] = [external(last_seen_on="2026-08-03")]
        errors = validate_transition_integrity(bindings(), current, today=TODAY)
        self.assertTrue(any("deprecated_on must not be later" in error for error in errors))
        self.assertTrue(any("last_seen_on must not be later" in error for error in errors))

    def test_external_migration_must_precede_retirement(self) -> None:
        current = empty_policy()
        current["contracts"] = [lifecycle("retired")]
        current["external_consumers"] = [
            external(state="migrated", migrated_on="2026-04-16")
        ]
        errors = validate_transition_integrity(bindings(), current, today=TODAY)
        self.assertTrue(any("migrated after retirement" in error for error in errors))

    def test_migration_guide_must_exist_in_repository(self) -> None:
        current = empty_policy()
        current["contracts"] = [lifecycle()]
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            errors = validate_transition_integrity(
                bindings(), current, today=TODAY, repository_root=root
            )
            self.assertTrue(any("migration guide does not exist" in error for error in errors))
            guide = root / "docs/migrations/example-create-v2.md"
            guide.parent.mkdir(parents=True)
            guide.write_text("migration", encoding="utf-8")
            errors = validate_transition_integrity(
                bindings(), current, today=TODAY, repository_root=root
            )
            self.assertFalse(any("migration guide does not exist" in error for error in errors))


if __name__ == "__main__":
    unittest.main()
