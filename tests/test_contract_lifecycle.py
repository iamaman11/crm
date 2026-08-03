from __future__ import annotations

import json
from pathlib import Path
import unittest

from scripts.contract_lifecycle import build_registry, render_registry


ROOT = Path(__file__).resolve().parents[1]


def capability(contract_id: str, version: str = "1.0.0") -> dict:
    stem = "".join(part.title() for part in contract_id.replace("-", ".").split("."))
    return {
        "id": contract_id,
        "version": version,
        "rpc": f"crm.example.v1.ExampleService.{stem}",
        "request": f"crm.example.v1.{stem}Request",
        "response": f"crm.example.v1.{stem}Response",
    }


def event(contract_id: str, version: str = "1.0.0") -> dict:
    stem = "".join(part.title() for part in contract_id.replace("-", ".").split("."))
    return {
        "id": contract_id,
        "version": version,
        "message": f"crm.example.v1.{stem}Event",
    }


def bindings(*, include_legacy: bool = True) -> dict:
    capabilities = [capability("example.create.v2")]
    if include_legacy:
        capabilities.append(capability("example.create"))
    return {
        "schema_version": "crm.contract-bindings/v1",
        "modules": [
            {
                "module_id": "crm.example",
                "capabilities": capabilities,
                "events": [event("example.created")],
            }
        ],
    }


def manifest(
    module_id: str,
    *,
    capabilities: list[tuple[str, str]] | None = None,
    events: list[tuple[str, str]] | None = None,
) -> dict:
    return {
        "module_id": module_id,
        "consumes": {
            "capabilities": [
                {"id": contract_id, "version": version}
                for contract_id, version in capabilities or []
            ],
            "events": [
                {"id": contract_id, "version": version}
                for contract_id, version in events or []
            ],
        },
    }


def empty_policy() -> dict:
    return {
        "schema_version": "crm.contract-lifecycle-policy/v1",
        "minimum_deprecation_days": 30,
        "contracts": [],
        "external_consumers": [],
    }


def lifecycle_entry(state: str) -> dict:
    entry = {
        "kind": "capability",
        "id": "example.create",
        "version": "1.0.0",
        "state": state,
        "owner": "contract-platform",
        "deprecated_on": "2026-01-01",
        "removal_not_before": "2026-02-01",
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
        entry["retired_on"] = "2026-03-15"
        entry["migration"]["completed_on"] = "2026-02-15"
        entry["telemetry"]["zero_since"] = "2026-02-01"
    return entry


class ContractLifecycleTests(unittest.TestCase):
    def test_committed_representative_deprecation_and_migration_are_exact(self) -> None:
        policy = json.loads((ROOT / "contracts/contract-lifecycle-policy.json").read_text())
        registry = json.loads((ROOT / "contracts/contract-lifecycle.json").read_text())

        self.assertEqual(len(policy["contracts"]), 1)
        deprecated = policy["contracts"][0]
        self.assertEqual(
            (deprecated["kind"], deprecated["id"], deprecated["version"]),
            ("capability", "activities.task.create", "1.0.0"),
        )
        self.assertEqual(deprecated["state"], "deprecated")
        self.assertEqual(deprecated["deprecated_on"], "2026-08-03")
        self.assertEqual(deprecated["removal_not_before"], "2026-09-02")
        self.assertEqual(deprecated["migration"]["completed_on"], "2026-08-03")
        self.assertIsNone(deprecated["telemetry"]["zero_since"])

        coordinates = [
            item
            for item in registry["published"]["capabilities"]
            if item[0] == "activities.task.create"
        ]
        self.assertEqual(
            coordinates,
            [
                ["activities.task.create", "1.0.0", "crm.activities", []],
                [
                    "activities.task.create",
                    "1.1.0",
                    "crm.activities",
                    ["crm.sales-activities-link"],
                ],
            ],
        )
        self.assertEqual(registry["lifecycle"], [deprecated])

    def test_registry_is_complete_sorted_and_inventories_internal_consumers(self) -> None:
        manifests = [
            manifest("crm.zeta", events=[("example.created", "1.0.0")]),
            manifest("crm.alpha", capabilities=[("example.create", "1.0.0")]),
        ]
        registry, errors = build_registry(bindings(), manifests, empty_policy())
        self.assertEqual(errors, [])
        coordinates = [
            (item["kind"], item["id"], item["version"])
            for item in registry["contracts"]
        ]
        self.assertEqual(coordinates, sorted(coordinates))
        legacy = next(item for item in registry["contracts"] if item["id"] == "example.create")
        self.assertEqual(legacy["state"], "active")
        self.assertEqual(legacy["provider_module_id"], "crm.example")
        self.assertEqual(legacy["internal_consumers"], ["crm.alpha"])
        self.assertTrue(render_registry(registry).endswith(b"\n"))

    def test_unknown_internal_consumption_fails_closed(self) -> None:
        manifests = [manifest("crm.consumer", capabilities=[("missing.command", "1.0.0")])]
        _, errors = build_registry(bindings(), manifests, empty_policy())
        self.assertTrue(any("live internal consumer references unpublished" in error for error in errors))

    def test_deprecated_contract_requires_published_replacement_and_valid_window(self) -> None:
        policy = empty_policy()
        entry = lifecycle_entry("deprecated")
        entry["replacement"]["id"] = "missing.replacement"
        entry["removal_not_before"] = "2026-01-15"
        policy["contracts"] = [entry]
        _, errors = build_registry(bindings(), [], policy)
        self.assertTrue(any("at least 30 days" in error for error in errors))

        entry["removal_not_before"] = "2026-02-01"
        _, errors = build_registry(bindings(), [], policy)
        self.assertTrue(any("replacement is not currently published" in error for error in errors))

    def test_retirement_is_blocked_by_live_internal_and_external_consumers(self) -> None:
        policy = empty_policy()
        policy["contracts"] = [lifecycle_entry("retired")]
        policy["external_consumers"] = [
            {
                "consumer_id": "public-sdk-v1",
                "owner": "sdk-platform",
                "kind": "capability",
                "id": "example.create",
                "version": "1.0.0",
                "state": "active",
                "migration_issue": 124,
                "migrated_on": None,
                "last_seen_on": "2026-03-01",
            }
        ]
        manifests = [manifest("crm.consumer", capabilities=[("example.create", "1.0.0")])]
        _, errors = build_registry(bindings(include_legacy=False), manifests, policy)
        self.assertTrue(any("live internal consumers remain" in error for error in errors))
        self.assertTrue(any("live external consumers remain" in error for error in errors))

    def test_removal_requires_prior_deprecation_in_base_policy(self) -> None:
        current_policy = empty_policy()
        current_policy["contracts"] = [lifecycle_entry("retired")]
        _, errors = build_registry(
            bindings(include_legacy=False),
            [],
            current_policy,
            base_bindings=bindings(include_legacy=True),
            base_policy=empty_policy(),
        )
        self.assertTrue(any("was not deprecated in the base policy" in error for error in errors))

    def test_governed_retirement_succeeds_after_migration_and_zero_usage(self) -> None:
        base_policy = empty_policy()
        base_policy["contracts"] = [lifecycle_entry("deprecated")]
        current_policy = empty_policy()
        current_policy["contracts"] = [lifecycle_entry("retired")]
        current_policy["external_consumers"] = [
            {
                "consumer_id": "public-sdk-v1",
                "owner": "sdk-platform",
                "kind": "capability",
                "id": "example.create",
                "version": "1.0.0",
                "state": "migrated",
                "migration_issue": 124,
                "migrated_on": "2026-02-10",
                "last_seen_on": "2026-01-31",
            }
        ]
        registry, errors = build_registry(
            bindings(include_legacy=False),
            [],
            current_policy,
            base_bindings=bindings(include_legacy=True),
            base_policy=base_policy,
        )
        self.assertEqual(errors, [])
        retired = next(item for item in registry["contracts"] if item["id"] == "example.create")
        self.assertEqual(retired["state"], "retired")
        self.assertIsNone(retired["provider_module_id"])
        self.assertEqual(retired["external_consumers"][0]["state"], "migrated")

    def test_retired_policy_requires_completed_migration_and_zero_usage(self) -> None:
        policy = empty_policy()
        entry = lifecycle_entry("retired")
        entry["migration"]["completed_on"] = None
        entry["telemetry"]["zero_since"] = None
        policy["contracts"] = [entry]
        _, errors = build_registry(bindings(include_legacy=False), [], policy)
        self.assertTrue(any("migration.completed_on" in error for error in errors))

    def test_deprecated_contract_cannot_silently_reactivate(self) -> None:
        base_policy = empty_policy()
        base_policy["contracts"] = [lifecycle_entry("deprecated")]
        _, errors = build_registry(
            bindings(include_legacy=True),
            [],
            empty_policy(),
            base_bindings=bindings(include_legacy=True),
            base_policy=base_policy,
        )
        self.assertTrue(any("cannot silently return to active" in error for error in errors))

    def test_one_external_consumer_may_inventory_multiple_contracts(self) -> None:
        policy = empty_policy()
        policy["external_consumers"] = [
            {
                "consumer_id": "public-sdk",
                "owner": "sdk-platform",
                "kind": "capability",
                "id": "example.create",
                "version": "1.0.0",
                "state": "active",
                "migration_issue": None,
                "migrated_on": None,
                "last_seen_on": "2026-01-01",
            },
            {
                "consumer_id": "public-sdk",
                "owner": "sdk-platform",
                "kind": "event",
                "id": "example.created",
                "version": "1.0.0",
                "state": "active",
                "migration_issue": None,
                "migrated_on": None,
                "last_seen_on": "2026-01-01",
            },
        ]
        registry, errors = build_registry(bindings(), [], policy)
        self.assertEqual(errors, [])
        bound = [
            item for item in registry["contracts"] if item["external_consumers"]
        ]
        self.assertEqual(len(bound), 2)


if __name__ == "__main__":
    unittest.main()
