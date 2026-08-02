from __future__ import annotations

from pathlib import Path
import tempfile
import unittest

from scripts.generate_contract_telemetry_catalog import deprecated_capabilities, load_policy, render


ROOT = Path(__file__).resolve().parents[1]


class ContractTelemetryCatalogTests(unittest.TestCase):
    def test_committed_catalog_is_current(self) -> None:
        policy = load_policy(ROOT / "contracts/contract-lifecycle-policy.json")
        expected = render(deprecated_capabilities(policy))
        actual = (
            ROOT
            / "crates/crm-application-runtime/src/generated_contract_telemetry.rs"
        ).read_bytes()
        self.assertEqual(actual, expected)

    def test_only_deprecated_capabilities_are_sorted_and_rendered(self) -> None:
        policy = {
            "schema_version": "crm.contract-lifecycle-policy/v1",
            "contracts": [
                {
                    "kind": "event",
                    "id": "test.created",
                    "version": "1.0.0",
                    "state": "deprecated",
                    "owner": "crm.test",
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
                    "owner": "crm.zeta",
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
                    "owner": "crm.alpha",
                    "telemetry": {
                        "metric": "crm_contract_invocations_total",
                        "lookback_days": 45,
                    },
                },
            ],
        }
        entries = deprecated_capabilities(policy)
        self.assertEqual([entry["capability_id"] for entry in entries], ["alpha.create"])
        generated = render(entries).decode("utf-8")
        self.assertIn('capability_id: "alpha.create"', generated)
        self.assertIn("lookback_days: 45", generated)
        self.assertNotIn("test.created", generated)
        self.assertNotIn("zeta.create", generated)

    def test_duplicate_and_invalid_telemetry_fail_closed(self) -> None:
        entry = {
            "kind": "capability",
            "id": "test.create",
            "version": "1.0.0",
            "state": "deprecated",
            "owner": "crm.test",
            "telemetry": {
                "metric": "crm_contract_invocations_total",
                "lookback_days": 30,
            },
        }
        policy = {
            "schema_version": "crm.contract-lifecycle-policy/v1",
            "contracts": [entry, dict(entry)],
        }
        with self.assertRaisesRegex(ValueError, "duplicate deprecated capability"):
            deprecated_capabilities(policy)
        policy["contracts"] = [dict(entry, telemetry={"metric": "bad metric", "lookback_days": 0})]
        with self.assertRaises(ValueError):
            deprecated_capabilities(policy)

    def test_policy_loader_rejects_non_object(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "policy.json"
            path.write_text("[]", encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "must contain an object"):
                load_policy(path)


if __name__ == "__main__":
    unittest.main()
