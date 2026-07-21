from __future__ import annotations

import json
from pathlib import Path
import unittest


ROOT = Path(__file__).resolve().parents[1]
FREEZE_PATH = ROOT / "contracts/customer-privacy-architecture-freeze.json"
ARCHITECTURE_PATH = ROOT / "docs/PHASE8A11_CUSTOMER_PRIVACY_ARCHITECTURE.md"
GUARDRAILS_PATH = ROOT / "docs/PHASE8A11_CUSTOMER_PRIVACY_GUARDRAILS.md"


class CustomerPrivacyArchitectureFreezeTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.freeze = json.loads(FREEZE_PATH.read_text(encoding="utf-8"))
        cls.architecture = ARCHITECTURE_PATH.read_text(encoding="utf-8")
        cls.guardrails = GUARDRAILS_PATH.read_text(encoding="utf-8")

    def test_packet_identity_and_owner_are_frozen(self) -> None:
        packet = self.freeze["packet"]
        self.assertEqual(packet["phase"], "8A.11")
        self.assertEqual(packet["issue"], 126)
        self.assertEqual(packet["module_id"], "crm.customer-privacy")
        self.assertEqual(packet["state"], "in_progress_architecture_freeze")

        owns = set(self.freeze["ownership"]["owns"])
        does_not_own = set(self.freeze["ownership"]["does_not_own"])
        self.assertTrue(owns)
        self.assertTrue(does_not_own)
        self.assertTrue(owns.isdisjoint(does_not_own))
        self.assertIn("privacy_case", owns)
        self.assertIn("party_values", does_not_own)
        self.assertIn("consent_assertions", does_not_own)
        self.assertIn("import_export_jobs_or_artifacts", does_not_own)

    def test_exact_inventory_counts_and_coordinate_disjointness(self) -> None:
        mutations = self.freeze["public_mutations"]
        queries = self.freeze["public_queries"]
        workers = [item["coordinate"] for item in self.freeze["worker_internal_coordinates"]]
        non_runtime = [item["coordinate"] for item in self.freeze["non_runtime_coordinates"]]

        self.assertEqual(len(mutations), 9)
        self.assertEqual(len(queries), 7)
        self.assertEqual(len(workers), 9)
        self.assertEqual(len(non_runtime), 1)

        all_coordinates = mutations + queries + workers + non_runtime
        self.assertEqual(len(all_coordinates), len(set(all_coordinates)))
        self.assertTrue(all(coordinate.endswith("@1.0.0") for coordinate in all_coordinates))

    def test_worker_phases_and_public_ingress_are_exact(self) -> None:
        workers = self.freeze["worker_internal_coordinates"]
        phases = [item["phase"] for item in workers]
        self.assertEqual(phases, sorted(phases))
        self.assertEqual(sorted(set(phases)), [260, 270, 280, 290])
        self.assertTrue(all(item["public_ingress"] is False for item in workers))

        enforcement = workers[0]
        self.assertEqual(
            enforcement["coordinate"],
            "customer_privacy.enforcement.decide@1.0.0",
        )
        self.assertEqual(
            enforcement["activation_semantics"],
            "trusted_final_guard_fail_closed",
        )

    def test_owner_contribution_registry_is_complete_and_unique(self) -> None:
        contributions = self.freeze["owner_contributions"]
        expected_modules = {
            "crm.parties",
            "crm.customer-accounts",
            "crm.contact-points",
            "crm.party-relationships",
            "crm.consents",
            "crm.identity-resolution",
            "crm.customer-data-operations",
            "crm.data-quality",
            "crm.customer-enrichment",
        }
        actual_modules = {item["module_id"] for item in contributions}
        self.assertEqual(actual_modules, expected_modules)
        self.assertEqual(len(contributions), len(expected_modules))

        scope_coordinates = [item["scope"] for item in contributions]
        action_coordinates = [item["action"] for item in contributions]
        self.assertEqual(len(scope_coordinates), len(set(scope_coordinates)))
        self.assertEqual(len(action_coordinates), len(set(action_coordinates)))
        self.assertTrue(all(value.endswith(".privacy.scope.contribute@1.0.0") for value in scope_coordinates))
        self.assertTrue(all(value.endswith(".privacy.action.apply@1.0.0") for value in action_coordinates))

        data_operations = next(
            item
            for item in contributions
            if item["module_id"] == "crm.customer-data-operations"
        )
        self.assertEqual(
            data_operations["privacy_export"],
            "customer_data.export.privacy.request@1.0.0",
        )

    def test_policy_precedence_and_subject_lock_are_frozen(self) -> None:
        self.assertEqual(
            self.freeze["precedence"],
            [
                "active_legal_hold",
                "mandatory_retention",
                "approved_privacy_action",
                "ordinary_product_retention",
            ],
        )
        subject_lock = self.freeze["subject_lock"]
        self.assertEqual(subject_lock["coordinate"], "tenant_id + canonical_party_id")
        self.assertIn("restriction_place", subject_lock["required_for"])
        self.assertIn("protected_owner_mutation", subject_lock["required_for"])
        self.assertIn("destructive_owner_action", subject_lock["required_for"])

    def test_crypto_shredding_is_explicitly_non_runtime(self) -> None:
        non_runtime = self.freeze["non_runtime_coordinates"]
        self.assertEqual(
            non_runtime[0]["coordinate"],
            "customer_privacy.crypto_shred.execute@1.0.0",
        )
        reason = non_runtime[0]["reason"]
        self.assertIn("data-encryption-key hierarchy", reason)
        self.assertIn("backup and restore", reason)

    def test_every_frozen_coordinate_is_documented(self) -> None:
        coordinates: list[str] = []
        coordinates.extend(self.freeze["public_mutations"])
        coordinates.extend(self.freeze["public_queries"])
        coordinates.extend(
            item["coordinate"] for item in self.freeze["worker_internal_coordinates"]
        )
        coordinates.extend(
            item["coordinate"] for item in self.freeze["non_runtime_coordinates"]
        )
        for contribution in self.freeze["owner_contributions"]:
            coordinates.append(contribution["scope"])
            coordinates.append(contribution["action"])
            if "privacy_export" in contribution:
                coordinates.append(contribution["privacy_export"])

        for coordinate in coordinates:
            with self.subTest(coordinate=coordinate):
                self.assertIn(coordinate, self.architecture)

        self.assertIn("9 public mutations", self.guardrails)
        self.assertIn("7 permission-aware public queries", self.guardrails)
        self.assertIn("9 trusted worker/internal coordinates", self.guardrails)
        self.assertIn("1 reasoned non-runtime", self.guardrails)

    def test_required_acceptance_evidence_is_not_empty_or_duplicated(self) -> None:
        acceptance = self.freeze["acceptance_required"]
        self.assertGreaterEqual(len(acceptance), 12)
        self.assertEqual(len(acceptance), len(set(acceptance)))
        self.assertIn("real_crm_api_public_process", acceptance)
        self.assertIn("concurrent_restriction_race", acceptance)
        self.assertIn("party_erased_tombstone_no_orphans", acceptance)
        self.assertIn("unchanged_exact_head_ci", acceptance)


if __name__ == "__main__":
    unittest.main()
