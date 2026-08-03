from __future__ import annotations

from pathlib import Path
import tempfile
import unittest

from scripts.generate_contract_telemetry_catalog import (
    capability_providers,
    deprecated_capabilities,
    deprecated_event_deliveries,
    event_delivery_bindings,
    load_policy,
    load_registry,
    render,
)


ROOT = Path(__file__).resolve().parents[1]


def registry(
    capabilities: list[list[object]] | None = None,
    events: list[list[object]] | None = None,
) -> dict[str, object]:
    return {
        "schema_version": "crm.contract-lifecycle/v1",
        "policy_schema_version": "crm.contract-lifecycle-policy/v1",
        "published_columns": [
            "id",
            "version",
            "provider_module_id",
            "internal_consumers",
        ],
        "published": {
            "capabilities": capabilities or [],
            "events": events or [],
        },
    }


def deprecated(
    kind: str,
    contract_id: str,
    metric: str = "crm_contract_usage_total",
    lookback_days: int = 30,
) -> dict[str, object]:
    return {
        "kind": kind,
        "id": contract_id,
        "version": "1.0.0",
        "state": "deprecated",
        "owner": "contract-platform",
        "telemetry": {
            "metric": metric,
            "lookback_days": lookback_days,
        },
    }


class ContractTelemetryCatalogTests(unittest.TestCase):
    def test_committed_catalog_is_current(self) -> None:
        policy = load_policy(ROOT / "contracts/contract-lifecycle-policy.json")
        lifecycle = load_registry(ROOT / "contracts/contract-lifecycle.json")
        expected = render(
            deprecated_capabilities(policy, lifecycle),
            deprecated_event_deliveries(policy, lifecycle),
        )
        actual = (
            ROOT
            / "crates/crm-application-runtime/src/generated_contract_telemetry.rs"
        ).read_bytes()
        self.assertEqual(actual, expected)

    def test_deprecated_capabilities_are_sorted_and_use_provider_module(self) -> None:
        policy = {
            "schema_version": "crm.contract-lifecycle-policy/v1",
            "contracts": [
                deprecated("capability", "zeta.create"),
                deprecated("capability", "alpha.create", lookback_days=45),
            ],
        }
        policy["contracts"][0]["state"] = "active"
        lifecycle = registry(
            capabilities=[
                ["alpha.create", "1.0.0", "crm.alpha", []],
                ["zeta.create", "1.0.0", "crm.zeta", []],
            ]
        )
        entries = deprecated_capabilities(policy, lifecycle)
        self.assertEqual([entry["capability_id"] for entry in entries], ["alpha.create"])
        self.assertEqual(entries[0]["owner_module_id"], "crm.alpha")
        generated = render(entries, []).decode("utf-8")
        self.assertIn('"alpha.create"', generated)
        self.assertIn('"crm.alpha"', generated)
        self.assertIn("\n        45,\n", generated)
        self.assertNotIn("contract-platform", generated)
        self.assertNotIn("zeta.create", generated)

    def test_deprecated_events_expand_sorted_internal_consumer_deliveries(self) -> None:
        policy = {
            "schema_version": "crm.contract-lifecycle-policy/v1",
            "contracts": [
                deprecated("event", "test.record.created", lookback_days=60),
                deprecated("event", "test.record.updated"),
            ],
        }
        policy["contracts"][1]["state"] = "active"
        lifecycle = registry(
            events=[
                [
                    "test.record.created",
                    "1.0.0",
                    "crm.test",
                    ["crm.consumer-z", "crm.consumer-a"],
                ],
                ["test.record.updated", "1.0.0", "crm.test", ["crm.consumer-a"]],
            ]
        )
        entries = deprecated_event_deliveries(policy, lifecycle)
        self.assertEqual(
            [entry["consumer_module_id"] for entry in entries],
            ["crm.consumer-a", "crm.consumer-z"],
        )
        self.assertTrue(all(entry["provider_module_id"] == "crm.test" for entry in entries))
        generated = render([], entries).decode("utf-8")
        self.assertIn("DEPRECATED_EVENT_DELIVERIES", generated)
        self.assertIn('"test.record.created"', generated)
        self.assertIn('"crm.consumer-a"', generated)
        self.assertIn('"crm.consumer-z"', generated)
        self.assertIn("\n        60,\n", generated)
        self.assertNotIn("contract-platform", generated)
        self.assertNotIn("test.record.updated", generated)

    def test_events_without_internal_consumers_do_not_fabricate_runtime_series(self) -> None:
        policy = {
            "schema_version": "crm.contract-lifecycle-policy/v1",
            "contracts": [deprecated("event", "test.unconsumed")],
        }
        lifecycle = registry(
            events=[["test.unconsumed", "1.0.0", "crm.test", []]]
        )
        self.assertEqual(deprecated_event_deliveries(policy, lifecycle), [])

    def test_duplicate_invalid_and_unpublished_capability_telemetry_fail_closed(self) -> None:
        entry = deprecated("capability", "test.create")
        lifecycle = registry(
            capabilities=[["test.create", "1.0.0", "crm.test", []]]
        )
        policy = {
            "schema_version": "crm.contract-lifecycle-policy/v1",
            "contracts": [entry, dict(entry)],
        }
        with self.assertRaisesRegex(ValueError, "duplicate deprecated capability"):
            deprecated_capabilities(policy, lifecycle)
        policy["contracts"] = [
            dict(entry, telemetry={"metric": "bad metric", "lookback_days": 0})
        ]
        with self.assertRaises(ValueError):
            deprecated_capabilities(policy, lifecycle)
        with self.assertRaisesRegex(ValueError, "is not published"):
            deprecated_capabilities(
                {
                    "schema_version": "crm.contract-lifecycle-policy/v1",
                    "contracts": [entry],
                },
                registry(),
            )

    def test_duplicate_invalid_and_unpublished_event_telemetry_fail_closed(self) -> None:
        entry = deprecated("event", "test.created")
        lifecycle = registry(
            events=[["test.created", "1.0.0", "crm.test", ["crm.consumer"]]]
        )
        policy = {
            "schema_version": "crm.contract-lifecycle-policy/v1",
            "contracts": [entry, dict(entry)],
        }
        with self.assertRaisesRegex(ValueError, "duplicate deprecated event"):
            deprecated_event_deliveries(policy, lifecycle)
        policy["contracts"] = [
            dict(entry, telemetry={"metric": "bad metric", "lookback_days": 0})
        ]
        with self.assertRaises(ValueError):
            deprecated_event_deliveries(policy, lifecycle)
        with self.assertRaisesRegex(ValueError, "is not published"):
            deprecated_event_deliveries(
                {
                    "schema_version": "crm.contract-lifecycle-policy/v1",
                    "contracts": [entry],
                },
                registry(),
            )

    def test_registry_shape_and_duplicate_coordinates_fail_closed(self) -> None:
        malformed = {
            "schema_version": "crm.contract-lifecycle/v1",
            "policy_schema_version": "crm.contract-lifecycle-policy/v1",
            "published_columns": ["id", "version"],
            "published": {"capabilities": [], "events": []},
        }
        with self.assertRaisesRegex(ValueError, "published_columns"):
            capability_providers(malformed)
        with self.assertRaisesRegex(ValueError, "published_columns"):
            event_delivery_bindings(malformed)
        duplicate_capability = registry(
            capabilities=[
                ["test.create", "1.0.0", "crm.test", []],
                ["test.create", "1.0.0", "crm.other", []],
            ]
        )
        with self.assertRaisesRegex(ValueError, "duplicate published capability"):
            capability_providers(duplicate_capability)
        duplicate_event = registry(
            events=[
                ["test.created", "1.0.0", "crm.test", ["crm.consumer"]],
                ["test.created", "1.0.0", "crm.other", ["crm.consumer"]],
            ]
        )
        with self.assertRaisesRegex(ValueError, "duplicate published event"):
            event_delivery_bindings(duplicate_event)
        duplicate_consumer = registry(
            events=[
                [
                    "test.created",
                    "1.0.0",
                    "crm.test",
                    ["crm.consumer", "crm.consumer"],
                ]
            ]
        )
        with self.assertRaisesRegex(ValueError, "duplicate internal event consumer"):
            event_delivery_bindings(duplicate_consumer)

    def test_empty_policy_renders_both_deterministic_empty_catalogs(self) -> None:
        generated = render([], []).decode("utf-8")
        self.assertIn("DEPRECATED_CONTRACTS", generated)
        self.assertIn("DEPRECATED_EVENT_DELIVERIES", generated)
        self.assertEqual(generated.count("= &[];"), 2)

    def test_loaders_reject_non_objects(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "value.json"
            path.write_text("[]", encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "must contain an object"):
                load_policy(path)
            with self.assertRaisesRegex(ValueError, "must contain an object"):
                load_registry(path)


if __name__ == "__main__":
    unittest.main()
