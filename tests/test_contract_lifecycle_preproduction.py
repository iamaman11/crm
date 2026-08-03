from __future__ import annotations

from copy import deepcopy
import unittest

from scripts.contract_lifecycle_preproduction import (
    NEVER_RELEASED_MODE,
    validate_policy,
)


def retired_policy() -> dict:
    return {
        "schema_version": "crm.contract-lifecycle-policy/v1",
        "minimum_deprecation_days": 30,
        "contracts": [
            {
                "kind": "capability",
                "id": "example.create",
                "version": "1.0.0",
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
                    "guide": "docs/migrations/example-create.md",
                    "completed_on": "2026-08-03",
                },
                "telemetry": {
                    "metric": "crm_deprecated_capability_usage_total",
                    "lookback_days": 30,
                    "zero_since": None,
                },
                "retirement": {
                    "mode": NEVER_RELEASED_MODE,
                    "evidence_id": "example-create-1.0.0-never-released",
                },
                "retired_on": "2026-08-03",
            }
        ],
        "external_consumers": [],
    }


class ContractLifecyclePreproductionTests(unittest.TestCase):
    def test_never_released_retirement_preserves_normal_boundary_without_waiting(self) -> None:
        overrides, external, errors = validate_policy(retired_policy())
        self.assertEqual(errors, [])
        self.assertEqual(external, [])
        lifecycle = next(iter(overrides.values()))
        self.assertEqual(lifecycle["state"], "retired")
        self.assertEqual(lifecycle["retired_on"], "2026-08-03")
        self.assertEqual(lifecycle["removal_not_before"], "2026-09-02")
        self.assertIsNone(lifecycle["telemetry"]["zero_since"])
        self.assertEqual(lifecycle["retirement"]["mode"], NEVER_RELEASED_MODE)

    def test_never_released_mode_requires_retired_state(self) -> None:
        policy = retired_policy()
        policy["contracts"][0]["state"] = "deprecated"
        policy["contracts"][0].pop("retired_on")
        _, _, errors = validate_policy(policy)
        self.assertTrue(any("requires state retired" in error for error in errors))

    def test_never_released_mode_rejects_fabricated_zero_usage(self) -> None:
        policy = retired_policy()
        policy["contracts"][0]["telemetry"]["zero_since"] = "2026-07-04"
        _, _, errors = validate_policy(policy)
        self.assertTrue(any("must remain null" in error for error in errors))

    def test_never_released_mode_rejects_backdated_retirement(self) -> None:
        policy = retired_policy()
        policy["contracts"][0]["retired_on"] = "2026-08-02"
        _, _, errors = validate_policy(policy)
        self.assertTrue(any("must not precede deprecated_on" in error for error in errors))

    def test_normal_deprecation_boundary_cannot_be_shortened(self) -> None:
        policy = retired_policy()
        policy["contracts"][0]["removal_not_before"] = "2026-08-04"
        _, _, errors = validate_policy(policy)
        self.assertTrue(any("preserve the normal 30-day boundary" in error for error in errors))

    def test_ordinary_policy_still_uses_the_core_rules(self) -> None:
        policy = retired_policy()
        entry = policy["contracts"][0]
        entry.pop("retirement")
        entry["telemetry"]["zero_since"] = "2026-07-04"
        _, _, errors = validate_policy(policy)
        self.assertTrue(any("retired_on must be on or after removal_not_before" in error for error in errors))

    def test_evidence_identifier_is_strict(self) -> None:
        policy = deepcopy(retired_policy())
        policy["contracts"][0]["retirement"]["evidence_id"] = "INVALID ID"
        _, _, errors = validate_policy(policy)
        self.assertTrue(any("evidence_id" in error for error in errors))


if __name__ == "__main__":
    unittest.main()
