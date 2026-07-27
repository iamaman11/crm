from __future__ import annotations

import json
from pathlib import Path
import unittest


ROOT = Path(__file__).resolve().parents[1]
FREEZE_PATH = ROOT / "contracts/customer-privacy-planning-freeze.json"
DOC_PATH = ROOT / "docs/CUSTOMER_PRIVACY_PLANNING_FREEZE.md"
STATUS_PATH = ROOT / "docs/PROJECT_STATUS.md"
MODULE_PATH = ROOT / "modules/crm-customer-privacy/module.yaml"
DOMAIN_PATH = ROOT / "modules/crm-customer-privacy/src/scope_planning.rs"
STATE_PATH = ROOT / "modules/crm-customer-privacy/src/scope_planning_state.rs"
PRODUCTION_PATH = ROOT / "crates/crm-customer-privacy-production/src/lib.rs"


class CustomerPrivacyPlanningFreezeTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.freeze = json.loads(FREEZE_PATH.read_text(encoding="utf-8"))
        cls.doc = DOC_PATH.read_text(encoding="utf-8")
        cls.status = STATUS_PATH.read_text(encoding="utf-8")
        cls.module = MODULE_PATH.read_text(encoding="utf-8")
        cls.domain = DOMAIN_PATH.read_text(encoding="utf-8")
        cls.state = STATE_PATH.read_text(encoding="utf-8")
        cls.production = PRODUCTION_PATH.read_text(encoding="utf-8")

    def test_packet_identity_and_historical_baseline_are_exact(self) -> None:
        packet = self.freeze["packet"]
        self.assertEqual(packet["phase"], "8A.11")
        self.assertEqual(packet["issue"], 126)
        self.assertEqual(packet["module_id"], "crm.customer-privacy")
        self.assertEqual(
            packet["state"], "contract_acceptance_frozen_runtime_not_started"
        )
        self.assertEqual(
            packet["baseline"], "090e8991da091ea894a1cb684bcaa19984b14f1c"
        )
        self.assertEqual(
            packet["parent_discovery_implementation"],
            "contracts/customer-privacy-discovery-snapshot-implementation.json",
        )

    def test_exact_coordinates_are_published_but_not_promoted(self) -> None:
        coordinates = self.freeze["coordinates"]
        self.assertEqual(
            coordinates["plan_build"]["coordinate"],
            "customer_privacy.plan.build@1.0.0",
        )
        self.assertEqual(coordinates["plan_build"]["phase"], 270)
        self.assertFalse(coordinates["plan_build"]["public_ingress"])
        self.assertEqual(
            coordinates["plan_get"]["coordinate"],
            "customer_privacy.case.plan.get@1.0.0",
        )
        self.assertEqual(
            coordinates["owner_outcomes_list"]["coordinate"],
            "customer_privacy.case.owner_outcomes.list@1.0.0",
        )
        self.assertIn("customer_privacy.case.plan.get", self.module)
        self.assertIn("customer_privacy.case.owner_outcomes.list", self.module)
        self.assertNotIn("customer_privacy.plan.build", self.production)

    def test_lineage_binds_case_snapshot_policy_and_jurisdiction(self) -> None:
        fields = set(self.freeze["input_lineage"]["fields"])
        self.assertTrue(
            {
                "privacy_case_id",
                "tenant_id",
                "canonical_party_id",
                "identity_resolution_generation",
                "source_case_version",
                "scope_snapshot_id",
                "scope_snapshot_binding_digest_sha256",
                "scope_completeness_digest_sha256",
                "registry_digest_sha256",
                "privacy_case_kind",
                "policy_version",
                "jurisdiction_code",
                "approval_required",
                "crypto_shred_supported",
            }.issubset(fields)
        )
        self.assertEqual(
            self.freeze["input_lineage"]["required_case_status"], "scoped"
        )
        self.assertFalse(
            self.freeze["input_lineage"]["silent_snapshot_rebase_allowed"]
        )
        self.assertFalse(
            self.freeze["input_lineage"]["silent_policy_rebase_allowed"]
        )

    def test_action_vocabulary_and_case_kind_mapping_are_exact(self) -> None:
        self.assertEqual(
            set(self.freeze["actions"]),
            {
                "retain",
                "restrict_only",
                "anonymize",
                "delete",
                "crypto_shred",
                "no_op_already_compliant",
            },
        )
        classification = self.freeze["classification"]
        self.assertEqual(classification["access"]["all_evidence_classes"], "retain")
        self.assertEqual(
            classification["portability_export"]["all_evidence_classes"],
            "retain",
        )
        self.assertEqual(
            classification["restrict_processing"]["all_evidence_classes"],
            "restrict_only",
        )
        erasure = classification["erasure"]
        self.assertEqual(erasure["destroyable_subject_data"]["action"], "delete")
        self.assertEqual(
            erasure["retain_minimized_evidence"]["action"], "anonymize"
        )
        self.assertEqual(
            erasure["immutable_required_evidence"]["action"], "retain"
        )
        self.assertEqual(erasure["derived_rebuildable_state"]["action"], "delete")
        self.assertEqual(
            erasure["crypto_shreddable_data"]["action_when_supported"],
            "crypto_shred",
        )
        self.assertEqual(
            erasure["crypto_shreddable_data"]["unsupported_result"],
            "fail_closed_without_destructive_fallback",
        )
        self.assertFalse(
            classification["no_op_already_compliant"]["initial_planner_emits"]
        )

    def test_domain_code_contains_every_frozen_action_and_digest_profile(self) -> None:
        for token in (
            "Retain",
            "RestrictOnly",
            "Anonymize",
            "Delete",
            "CryptoShred",
            "NoOpAlreadyCompliant",
            "crm.customer-privacy.action-plan-lineage/v1",
            "crm.customer-privacy.action-plan-item/v1",
            "crm.customer-privacy.action-plan/v1",
        ):
            with self.subTest(token=token):
                self.assertIn(token, self.domain)
        self.assertIn("UnsupportedCryptoShred", self.domain)
        self.assertIn("initial planning cannot infer owner compliance", self.domain)

    def test_persistence_contract_is_strict_canonical_and_bounded(self) -> None:
        persistence = self.freeze["persistence_contract"]
        self.assertEqual(persistence["record_type"], "customer-privacy.action-plan")
        self.assertEqual(
            persistence["schema_id"], "crm.customer-privacy.action_plan.state"
        )
        self.assertEqual(persistence["schema_version"], "1.0.0")
        self.assertEqual(persistence["maximum_bytes"], 524288)
        self.assertTrue(persistence["strict_rehydration"])
        self.assertTrue(persistence["deny_unknown_fields"])
        self.assertTrue(persistence["recompute_lineage_item_plan_digests"])
        self.assertIn("serde(deny_unknown_fields)", self.state)
        self.assertIn("strict canonical v1 encoding", self.state)
        self.assertIn("ACTION_PLAN_STATE_MAXIMUM_BYTES", self.state)

    def test_plan_items_and_read_bounds_are_exact(self) -> None:
        items = self.freeze["plan_items"]
        self.assertEqual(items["sequence_starts_at"], 1)
        self.assertTrue(items["sequence_contiguous"])
        self.assertEqual(items["maximum_items"], 16384)
        self.assertFalse(items["resource_payload_allowed"])
        self.assertFalse(items["owner_private_metadata_allowed"])

        outcomes = self.freeze["read_boundaries"]["owner_outcomes_list"]
        self.assertEqual(outcomes["default_page_size"], 64)
        self.assertEqual(outcomes["maximum_page_size"], 128)
        self.assertEqual(outcomes["cursor_maximum_bytes"], 2048)
        self.assertEqual(outcomes["before_execution"], "empty_terminal_page")
        self.assertFalse(outcomes["synthetic_outcomes_allowed"])
        self.assertFalse(self.freeze["read_boundaries"]["possession_is_authority"])

    def test_retention_hold_approval_and_execution_remain_outside_packet(self) -> None:
        policy = self.freeze["policy_boundaries"]
        self.assertTrue(
            policy["retention_policy_id_is_snapshot_classification_input_only"]
        )
        self.assertFalse(policy["retention_adjudication_performed"])
        self.assertFalse(policy["legal_hold_adjudication_performed"])
        self.assertFalse(policy["approval_performed"])

        non_effects = set(self.freeze["non_effects"])
        self.assertIn("no_processing_restriction_change", non_effects)
        self.assertIn("no_legal_hold_decision", non_effects)
        self.assertIn("no_retention_adjudication", non_effects)
        self.assertIn("no_owner_action_dispatch", non_effects)
        self.assertIn("no_deletion_anonymization_or_crypto_shred_execution", non_effects)
        self.assertIn("no_customer_privacy_worker_registration", non_effects)
        self.assertIn("no_new_crate_or_dependency", non_effects)

    def test_documentation_and_status_preserve_runtime_not_started_boundary(self) -> None:
        self.assertIn(
            "Contract and acceptance semantics frozen; production planning runtime not started",
            self.doc,
        )
        self.assertIn("customer_privacy.plan.build@1.0.0", self.doc)
        self.assertIn("NoOpAlreadyCompliant", self.doc)
        self.assertIn("Unsupported crypto-shred fails closed", self.doc)
        self.assertIn("no Customer Privacy worker", self.doc)
        self.assertIn("CUSTOMER_PRIVACY_PLANNING_FREEZE.md", self.status)
        self.assertIn("runtime implementation not started", self.status.lower())

    def test_acceptance_matrix_is_nonempty_unique_and_exact_head_bound(self) -> None:
        acceptance = self.freeze["acceptance_required"]
        self.assertGreaterEqual(len(acceptance), 12)
        self.assertEqual(len(acceptance), len(set(acceptance)))
        self.assertIn("unsupported_crypto_shred_fail_closed_without_fallback", acceptance)
        self.assertIn("strict_canonical_state_round_trip_and_tamper_rejection", acceptance)
        self.assertIn("permission_aware_plan_get_contract", acceptance)
        self.assertIn("unchanged_exact_head_ci", acceptance)


if __name__ == "__main__":
    unittest.main()
