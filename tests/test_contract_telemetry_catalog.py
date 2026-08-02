from __future__ import annotations

from pathlib import Path
import tempfile
import unittest

from scripts.generate_contract_telemetry_catalog import (
    capability_providers,
    deprecated_capabilities,
    load_policy,
    load_registry,
    render,
)


ROOT = Path(__file__).resolve().parents[1]


def registry(*rows: list[object]) -> dict[str, object]:
    return {
        "schema_version": "crm.contract-lifecycle/v1",
        "policy_schema_version": "crm.contract-lifecycle-policy/v1",
        "published_columns": [
            "id",
            "version",
            "provider_module_id",
            "internal_consumers",
        ],
        "published": {"capabilities": list(rows), "events": []},
    }


class ContractTelemetryCatalogTests(unittest.TestCase):
    def test_committed_catalog_is_current(self) -> None:
        policy = load_policy(ROOT / "contracts/contract-lifecycle-policy.json")
        lifecycle = load_registry(ROOT / "contracts/contract-lifecycle.json")
        expected = render(deprecated_capabilities(policy, lifecycle))
        actual = (
            ROOT
            / "crates/crm-application-runtime/src/generated_contract_telemetry.rs"
        ).read_bytes()
        self.assertEqual(actual, expected)

    def test_only_deprecated_capabilities_are_sorted_and_use_provider_module(self) -> None:
        policy = {
            "schema_version": "crm.contract-lifecycle-policy/v1",
            "contracts": [
                {
                    "kind": "event",
                    "id": "test.created",
                    "version": "1.0.0",
                    "state": "deprecated",
                    "owner": "contract-platform",
                    "telemetry": {
                        "metric": "crm_contract_invocations_total",
                        "lookback_days": 30,
                    },
                },
                {
                    "kind": "capability",
                    "id": "zeta.create",
                    "version": "1.0.0",
                    "state": "active",
                    "owner": "contract-platform",
                    "telemetry": {
                        "metric": "crm_contract_invocations_total",
                        "lookback_days": 30,
                    },
                },
                {
                    "kind": "capability",
                    "id": "alpha.create",
                    "version": "1.0.0",
                    "state": "deprecated",
                    "owner": "contract-platform",
                    "telemetry": {
                        "metric": "crm_contract_invocations_total",
                        "lookback_days": 45,
                    },
                },
            ],
        }
        lifecycle = registry(
            ["alpha.create", "1.0.0", "crm.alpha", []],
            ["zeta.create", "1.0.0", "crm.zeta", []],
        )
        entries = deprecated_capabilities(policy, lifecycle)
        self.assertEqual([entry["capability_id"] for entry in entries], ["alpha.create"])
        self.assertEqual(entries[0]["owner_module_id"], "crm.alpha")
        generated = render(entries).decode("utf-8")
        self.assertIn('"alpha.create"', generated)
        self.assertIn('"crm.alpha"', generated)
        self.assertIn("\n        45,\n", generated)
        self.assertNotIn("contract-platform", generated)
        self.assertNotIn("test.created", generated)
        self.assertNotIn("zeta.create", generated)

    def test_duplicate_invalid_and_unpublished_telemetry_fail_closed(self) -> None:
        entry = {
            "kind": "capability",
            "id": "test.create",
            "version": "1.0.0",
            "state": "deprecated",
            "owner": "contract-platform",
            "telemetry": {
                "metric": "crm_contract_invocations_total",
                "lookback_days": 30,
            },
        }
        lifecycle = registry(["test.create", "1.0.0", "crm.test", []])
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

    def test_registry_shape_and_duplicate_coordinates_fail_closed(self) -> None:
        with self.assertRaisesRegex(ValueError, "published_columns"):
            capability_providers(
                {
                    "schema_version": "crm.contract-lifecycle/v1",
                    "policy_schema_version": "crm.contract-lifecycle-policy/v1",
                    "published_columns": ["id", "version"],
                    "published": {"capabilities": [], "events": []},
                }
            )
        duplicate = registry(
            ["test.create", "1.0.0", "crm.test", []],
            ["test.create", "1.0.0", "crm.other", []],
        )
        with self.assertRaisesRegex(ValueError, "duplicate published capability"):
            capability_providers(duplicate)

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
