import json
from pathlib import Path
import unittest

from scripts.validate_module_manifests import strict_yaml_load


ROOT = Path(__file__).resolve().parents[1]


class CustomerPrivacyContractInventoryTests(unittest.TestCase):
    def test_public_contract_inventory_matches_freeze_and_promotes_case_cancel(self) -> None:
        manifest_path = ROOT / "modules/crm-customer-privacy/module.yaml"
        manifest = strict_yaml_load(
            manifest_path.read_text(encoding="utf-8"), str(manifest_path)
        )
        freeze = json.loads(
            (ROOT / "contracts/customer-privacy-architecture-freeze.json").read_text(
                encoding="utf-8"
            )
        )
        classifications = json.loads(
            (ROOT / "contracts/production-route-classifications.json").read_text(
                encoding="utf-8"
            )
        )

        expected_capabilities = {
            "customer_privacy.case.approve",
            "customer_privacy.case.cancel",
            "customer_privacy.case.create",
            "customer_privacy.case.get",
            "customer_privacy.case.list",
            "customer_privacy.case.owner_outcomes.list",
            "customer_privacy.case.plan.get",
            "customer_privacy.case.subject.verify",
            "customer_privacy.case.submit",
            "customer_privacy.legal_hold.get",
            "customer_privacy.legal_hold.list_by_subject",
            "customer_privacy.legal_hold.place",
            "customer_privacy.legal_hold.release",
            "customer_privacy.restriction.get",
            "customer_privacy.restriction.place",
            "customer_privacy.restriction.release",
        }
        frozen_public = {
            coordinate.rsplit("@", 1)[0]
            for coordinate in freeze["public_mutations"] + freeze["public_queries"]
        }
        actual_capabilities = {
            capability["id"] for capability in manifest["provides"]["capabilities"]
        }

        self.assertEqual(expected_capabilities, frozen_public)
        self.assertEqual(actual_capabilities, expected_capabilities)
        self.assertEqual(
            {event["id"] for event in manifest["provides"]["events"]},
            {
                "customer_privacy.case.created",
                "customer_privacy.case.status_changed",
                "customer_privacy.case.subject_verified",
                "customer_privacy.legal_hold.placed",
                "customer_privacy.legal_hold.released",
                "customer_privacy.restriction.placed",
                "customer_privacy.restriction.released",
            },
        )

        runtime_mutations = {
            "customer_privacy.case.create",
            "customer_privacy.case.submit",
            "customer_privacy.case.subject.verify",
            "customer_privacy.case.cancel",
        }
        runtime_queries = {"customer_privacy.case.get"}
        non_runtime = {
            (route["owner_module_id"], route["id"], route["version"])
            for route in classifications["non_runtime_contract_routes"]
            if route["owner_module_id"] == "crm.customer-privacy"
        }
        expected_non_runtime = {
            ("crm.customer-privacy", capability_id, "1.0.0")
            for capability_id in expected_capabilities
            - runtime_mutations
            - runtime_queries
        }
        self.assertEqual(non_runtime, expected_non_runtime)
        self.assertEqual(len(non_runtime), 11)
        for runtime_id in runtime_mutations | runtime_queries:
            self.assertNotIn(
                ("crm.customer-privacy", runtime_id, "1.0.0"),
                non_runtime,
            )
        self.assertFalse(
            any(
                route["owner_module_id"] == "crm.customer-privacy"
                for route in classifications["worker_runtime_routes"]
            )
        )
        self.assertNotIn(
            "crm.customer-privacy",
            {entry["module_id"] for entry in classifications["empty_runtime_modules"]},
        )


if __name__ == "__main__":
    unittest.main()
