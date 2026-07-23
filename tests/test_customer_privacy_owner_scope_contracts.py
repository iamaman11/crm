import json
from pathlib import Path
import unittest

from scripts.validate_module_manifests import strict_yaml_load


ROOT = Path(__file__).resolve().parents[1]


class CustomerPrivacyOwnerScopeContractTests(unittest.TestCase):
    def test_exact_nine_owner_contracts_match_freeze_manifests_and_classification(self) -> None:
        packet = json.loads(
            (ROOT / "contracts/customer-privacy-owner-scope-contracts.json").read_text(
                encoding="utf-8"
            )
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

        self.assertEqual(packet["state"], "contract_only_non_runtime")
        self.assertEqual(packet["wire"]["digest_algorithm"], "sha256")
        self.assertEqual(packet["wire"]["digest_bytes"], 32)
        self.assertEqual(
            packet["wire"]["request_envelope"],
            "crm.customer_privacy.v1.PrivacyScopeContributionRequestEnvelope",
        )
        self.assertEqual(
            packet["wire"]["response_envelope"],
            "crm.customer_privacy.v1.PrivacyScopeContributionResponseEnvelope",
        )
        self.assertEqual(packet["bounded_page"]["default_page_size"], 64)
        self.assertEqual(packet["bounded_page"]["maximum_page_size"], 128)
        self.assertEqual(packet["bounded_page"]["maximum_cursor_bytes"], 2048)

        owners = packet["owners"]
        self.assertEqual(len(owners), 9)
        self.assertEqual(len({entry["module_id"] for entry in owners}), 9)
        self.assertEqual(len({entry["capability_id"] for entry in owners}), 9)
        self.assertEqual(len({entry["rpc"] for entry in owners}), 9)
        self.assertEqual(len({entry["request"] for entry in owners}), 9)
        self.assertEqual(len({entry["response"] for entry in owners}), 9)

        frozen = {
            entry["module_id"]: entry["scope"].rsplit("@", 1)
            for entry in freeze["owner_contributions"]
        }
        actual = {
            entry["module_id"]: [entry["capability_id"], entry["version"]]
            for entry in owners
        }
        self.assertEqual(actual, frozen)

        expected_non_runtime = set()
        for entry in owners:
            manifest_path = ROOT / entry["manifest"]
            manifest = strict_yaml_load(
                manifest_path.read_text(encoding="utf-8"), str(manifest_path)
            )
            self.assertEqual(manifest["module_id"], entry["module_id"])
            capabilities = {
                (capability["id"], capability["version"]): capability
                for capability in manifest["provides"]["capabilities"]
            }
            coordinate = (entry["capability_id"], entry["version"])
            self.assertIn(coordinate, capabilities)
            binding = capabilities[coordinate]["binding"]
            self.assertEqual(binding["kind"], "protobuf_rpc")
            self.assertEqual(binding["rpc"], entry["rpc"])
            self.assertEqual(binding["request"], entry["request"])
            self.assertEqual(binding["response"], entry["response"])
            expected_non_runtime.add(
                (entry["module_id"], entry["capability_id"], entry["version"])
            )

        non_runtime = {
            (route["owner_module_id"], route["id"], route["version"])
            for route in classifications["non_runtime_contract_routes"]
        }
        worker_runtime = {
            (route["owner_module_id"], route["id"], route["version"])
            for route in classifications["worker_runtime_routes"]
        }
        platform_runtime = {
            (route["owner_module_id"], route["id"], route["version"])
            for route in classifications["platform_runtime_routes"]
        }
        self.assertTrue(expected_non_runtime <= non_runtime)
        self.assertTrue(expected_non_runtime.isdisjoint(worker_runtime))
        self.assertTrue(expected_non_runtime.isdisjoint(platform_runtime))

        privacy_manifest_path = ROOT / "modules/crm-customer-privacy/module.yaml"
        privacy_manifest = strict_yaml_load(
            privacy_manifest_path.read_text(encoding="utf-8"),
            str(privacy_manifest_path),
        )
        privacy_consumes = {
            (capability["id"], capability["version"])
            for capability in privacy_manifest["consumes"]["capabilities"]
        }
        self.assertTrue(
            privacy_consumes.isdisjoint(
                {(entry["capability_id"], entry["version"]) for entry in owners}
            )
        )

    def test_wire_contract_is_reference_only_and_represents_every_data_class(self) -> None:
        packet = json.loads(
            (ROOT / "contracts/customer-privacy-owner-scope-contracts.json").read_text(
                encoding="utf-8"
            )
        )
        contributions = (
            ROOT / "proto/crm/customer_privacy/v1/contributions.proto"
        ).read_text(encoding="utf-8")
        types = (ROOT / "proto/crm/customer_privacy/v1/types.proto").read_text(
            encoding="utf-8"
        )

        self.assertEqual(contributions.count("  rpc "), 9)
        for owner in packet["owners"]:
            service_and_method = owner["rpc"].removeprefix(
                "crm.customer_privacy.v1."
            )
            service, method = service_and_method.rsplit(".", 1)
            request = owner["request"].rsplit(".", 1)[1]
            response = owner["response"].rsplit(".", 1)[1]
            self.assertIn(f"service {service} {{", contributions)
            self.assertIn(
                f"rpc {method}({request}) returns ({response});",
                contributions,
            )
            self.assertTrue(request.startswith(method))
            self.assertTrue(response.startswith(method))
        self.assertEqual(
            contributions.count("PrivacyScopeContributionRequestEnvelope contribution = 1;"),
            9,
        )
        self.assertEqual(
            contributions.count("PrivacyScopeContributionResponseEnvelope contribution = 1;"),
            9,
        )
        self.assertNotIn("bytes resource_payload", contributions)
        self.assertNotIn("string resource_value", contributions)
        self.assertIn("PrivacyScopeResourceReference", contributions)
        self.assertIn("bytes page_digest_sha256", contributions)
        self.assertIn("bytes cursor_digest_sha256", contributions)
        self.assertIn("CUSTOMER_DATA_CLASS_RESTRICTED = 9;", types)


if __name__ == "__main__":
    unittest.main()
