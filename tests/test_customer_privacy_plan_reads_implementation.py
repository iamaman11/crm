from __future__ import annotations

import json
from pathlib import Path
import unittest


ROOT = Path(__file__).resolve().parents[1]
EVIDENCE_PATH = ROOT / "contracts/customer-privacy-plan-reads-implementation.json"
DOC_PATH = ROOT / "docs/CUSTOMER_PRIVACY_PLAN_READS_IMPLEMENTATION.md"
PLANNING_FREEZE_PATH = ROOT / "contracts/customer-privacy-planning-freeze.json"
PLANNING_RUNTIME_PATH = ROOT / "contracts/customer-privacy-planning-implementation.json"


class CustomerPrivacyPlanReadsImplementationTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.evidence = json.loads(EVIDENCE_PATH.read_text(encoding="utf-8"))
        cls.document = DOC_PATH.read_text(encoding="utf-8")
        cls.planning_freeze = json.loads(
            PLANNING_FREEZE_PATH.read_text(encoding="utf-8")
        )
        cls.planning_runtime = json.loads(
            PLANNING_RUNTIME_PATH.read_text(encoding="utf-8")
        )

    def test_accepted_packet_identity_and_historical_evidence_are_exact(self) -> None:
        self.assertEqual(
            self.evidence["schema_id"],
            "crm.governance.customer-privacy-plan-reads-implementation",
        )
        self.assertEqual(self.evidence["schema_version"], "1.0.0")
        self.assertEqual(self.evidence["status"], "accepted_merged")
        acceptance = self.evidence["acceptance"]
        self.assertEqual(acceptance["pull_request"], 211)
        self.assertEqual(
            acceptance["accepted_source_sha"],
            "933fa4b502d60a23b83de9ccee279cc6517b5cba",
        )
        self.assertEqual(acceptance["permanent_workflow_count"], 32)
        self.assertEqual(
            acceptance["merge_sha"],
            "a1f3a60a6d8e8bba7bda50f936c57a61bc3521f7",
        )
        self.assertEqual(
            self.planning_freeze["packet"]["state"],
            "contract_acceptance_frozen_runtime_not_started",
        )
        for coordinate in ("plan_build", "plan_get", "owner_outcomes_list"):
            self.assertEqual(
                self.planning_freeze["coordinates"][coordinate]["runtime_state"],
                "not_implemented",
            )
        self.assertEqual(self.planning_runtime["status"], "accepted_merged")
        self.assertIn("Accepted and merged through PR #211", self.document)
        self.assertIn("Historical sources remain immutable", self.document)
        self.assertIn("32 of 32", self.document)

    def test_exact_two_read_coordinates_are_promoted_without_mutation_or_worker(self) -> None:
        coordinates = self.evidence["coordinates"]
        self.assertEqual(
            {
                f"{item['capability_id']}@{item['capability_version']}"
                for item in coordinates
            },
            {
                "customer_privacy.case.plan.get@1.0.0",
                "customer_privacy.case.owner_outcomes.list@1.0.0",
            },
        )
        self.assertTrue(all(item["mutation"] is False for item in coordinates))
        self.assertTrue(all(item["worker"] is False for item in coordinates))
        self.assertTrue(all(item["public_ingress"] is True for item in coordinates))

    def test_inventory_and_package_boundaries_are_no_growth(self) -> None:
        boundary = self.evidence["package_boundary"]
        self.assertEqual(boundary["new_workspace_packages"], 0)
        self.assertEqual(boundary["workspace_package_count_before"], 113)
        self.assertEqual(boundary["workspace_package_count_after"], 113)
        self.assertEqual(boundary["new_dependency_families"], 0)
        self.assertFalse(boundary["generic_runtime_switch_added"])
        inventory = self.evidence["runtime_inventory"]
        self.assertEqual(inventory["public_mutations_before"], 4)
        self.assertEqual(inventory["public_mutations_after"], 4)
        self.assertEqual(inventory["public_queries_before"], 2)
        self.assertEqual(inventory["public_queries_after"], 4)
        self.assertEqual(inventory["customer_privacy_workers_before"], 0)
        self.assertEqual(inventory["customer_privacy_workers_after"], 0)

    def test_plan_get_is_permission_aware_strict_and_payload_safe(self) -> None:
        plan_get = self.evidence["plan_get"]
        for key in (
            "module_activation_required",
            "live_visibility_required",
            "tenant_bound_reads",
            "strict_case_snapshot_plan_replay_rehydration",
            "payload_safe_summary_only",
            "safe_read_audit",
            "unauthorized_and_cross_tenant_concealed",
        ):
            self.assertTrue(plan_get[key], key)
        self.assertFalse(plan_get["owner_resource_payload_exposed"])
        self.assertEqual(
            plan_get["missing_corrupt_or_conflicting_evidence"], "fail_closed"
        )

    def test_owner_outcomes_is_empty_terminal_and_has_no_persistence(self) -> None:
        outcomes = self.evidence["owner_outcomes_list"]
        self.assertEqual(outcomes["default_page_size"], 64)
        self.assertEqual(outcomes["maximum_page_size"], 128)
        self.assertEqual(outcomes["maximum_cursor_bytes"], 2048)
        self.assertEqual(outcomes["items"], [])
        self.assertEqual(outcomes["next_cursor"], "terminal_empty_string")
        self.assertTrue(outcomes["stable_page_digest"])
        self.assertTrue(outcomes["stable_terminal_digest"])
        self.assertFalse(outcomes["outcome_records_persisted"])
        self.assertFalse(outcomes["synthetic_outcomes_returned"])
        persistence = self.evidence["persistence"]
        self.assertEqual(
            persistence["read_audit_table"],
            "crm.customer_privacy_plan_read_audit",
        )
        self.assertTrue(persistence["append_only"])
        self.assertTrue(persistence["force_rls"])
        self.assertEqual(persistence["canonical_policy_name"], "tenant_isolation")
        self.assertFalse(persistence["owner_outcome_table_added"])

    def test_source_and_sql_markers_match_machine_evidence(self) -> None:
        production = (
            ROOT / "crates/crm-customer-privacy-production/src/lib.rs"
        ).read_text(encoding="utf-8")
        migration = (
            ROOT / "database/migrations/0103_customer_privacy_plan_reads.up.sql"
        ).read_text(encoding="utf-8")
        acceptance = (
            ROOT / "database/tests/0045_customer_privacy_plan_reads.sql"
        ).read_text(encoding="utf-8")
        self.assertIn(
            "Customer Privacy production inventory must contain exactly four queries",
            production,
        )
        self.assertIn("CREATE TABLE crm.customer_privacy_plan_read_audit", migration)
        self.assertIn("FORCE ROW LEVEL SECURITY", migration)
        self.assertIn("CREATE POLICY tenant_isolation", migration)
        self.assertNotIn("customer_privacy_owner_action_outcomes", migration)
        self.assertIn("owner-outcome persistence must not exist", acceptance)

    def test_later_runtime_remains_explicitly_outside_packet(self) -> None:
        excluded = set(self.evidence["not_implemented"])
        self.assertTrue(
            {
                "owner_outcome_persistence",
                "owner_execution",
                "approval_runtime",
                "processing_restrictions",
                "legal_hold_decisions",
                "mandatory_retention_adjudication",
                "deletion_anonymization_or_crypto_shred_execution",
                "customer_privacy_worker",
            }
            <= excluded
        )
        self.assertEqual(self.evidence["next_packet"], "approval runtime")
        self.assertFalse(self.evidence["phase_8a_complete"])
        self.assertEqual(self.evidence["product_complete_expert_modules"], 0)


if __name__ == "__main__":
    unittest.main()
