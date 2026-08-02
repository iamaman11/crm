from __future__ import annotations

import json
from pathlib import Path
import unittest


ROOT = Path(__file__).resolve().parents[1]
REBUILD_ACCEPTANCE = ROOT / "crates/crm-application-runtime/tests/party_tombstone_rebuild_convergence_postgres.rs"
PROCESS_ACCEPTANCE = ROOT / "services/crm-api/tests/customer_360_privacy_no_orphan_process_e2e.rs"
PROCESS_SUPPORT = ROOT / "services/crm-api/tests/support.rs"
COMPOSITION = ROOT / "crates/crm-customer-360-composition/src/lib.rs"
QUERY_ADAPTER = ROOT / "crates/crm-customer-360-query-adapter/src/lib.rs"
BACKGROUND = ROOT / "crates/crm-application-runtime/src/background.rs"
RUNTIME = ROOT / "crates/crm-application-runtime/src/runtime.rs"
WORKFLOW = ROOT / ".github/workflows/customer-privacy-owner-execution.yml"
PACKET = ROOT / "repository-packet.json"


class PartyTombstoneRebuildConvergenceTests(unittest.TestCase):
    """Retain accepted Step 15 runtime evidence while documentation advances."""

    @classmethod
    def setUpClass(cls) -> None:
        cls.rebuild_acceptance = REBUILD_ACCEPTANCE.read_text(encoding="utf-8")
        cls.process_acceptance = PROCESS_ACCEPTANCE.read_text(encoding="utf-8")
        cls.process_support = PROCESS_SUPPORT.read_text(encoding="utf-8")
        cls.composition = COMPOSITION.read_text(encoding="utf-8")
        cls.query_adapter = QUERY_ADAPTER.read_text(encoding="utf-8")
        cls.background = BACKGROUND.read_text(encoding="utf-8")
        cls.runtime = RUNTIME.read_text(encoding="utf-8")
        cls.workflow = WORKFLOW.read_text(encoding="utf-8")
        cls.packet = json.loads(PACKET.read_text(encoding="utf-8"))

    def test_shared_projection_identity_remains_v2_without_schema_change(self) -> None:
        self.assertIn(
            'pub const CUSTOMER_360_PROJECTION_ID: &str = "customer.customer-360.v2";',
            self.composition,
        )
        self.assertNotIn(
            'pub const CUSTOMER_360_PROJECTION_ID: &str = "customer.customer-360.v1";',
            self.composition,
        )
        self.assertIn(
            'pub const CUSTOMER_360_PROJECTION_SCHEMA_VERSION: &str = "1";',
            self.composition,
        )

    def test_rebuild_acceptance_preserves_canonical_execution_and_replay(self) -> None:
        for marker in (
            "build_canonical_internal_owner_execution",
            ".execute_next(OwnerExecutionInvocation",
            "PrivacyOwnerOutcomeStatus::Succeeded",
            "parties.privacy.action.apply.completed",
            'LEGACY_CUSTOMER_360_PROJECTION_ID: &str = "customer.customer-360.v1"',
            ".run_batch(tenant_id.clone(), 200)",
            ".rebuild(tenant_id.clone(), 200)",
            'REPEAT_SEARCH_GENERATION_ID: &str = "g4"',
            "authoritative_before_repeat",
        ):
            self.assertIn(marker, self.rebuild_acceptance)
        self.assertNotIn("INSERT INTO crm.outbox_events", self.rebuild_acceptance)

    def test_query_background_and_process_host_use_shared_worker_wiring(self) -> None:
        self.assertIn("CUSTOMER_360_PROJECTION_ID", self.query_adapter)
        self.assertIn(".bind(CUSTOMER_360_PROJECTION_ID)", self.query_adapter)
        self.assertNotIn('"customer.customer-360.v1"', self.query_adapter)
        self.assertIn("Customer360ProjectionWorker", self.background)
        self.assertIn("self.inner.run_batch", self.background)
        self.assertIn("Customer360ProjectionWorker::new", self.runtime)
        self.assertIn("build_production_background_workers", self.runtime)

    def test_real_process_acceptance_owns_the_no_orphan_repair(self) -> None:
        for marker in (
            "spawn_crm_api",
            "create_party",
            "stop_process(&mut first_process)",
            "seed_privacy_orphan",
            "assert_no_v2_document",
            "restart_http_addr",
            "wait_for_v2_tombstone",
            "assert_legacy_v1_stale",
            "assert_authoritative_party_minimized",
            "parties.privacy.action.apply.completed",
            "privacy_minimized",
            "roots_removed",
            "leaks_original",
            "payload_encoding",
            "deduplication_key",
        ):
            self.assertIn(marker, self.process_acceptance)
        for forbidden_call in (
            "Customer360ProjectionWorker::new",
            "ProjectionRunner",
            ".run_batch(",
            ".rebuild(",
        ):
            self.assertNotIn(forbidden_call, self.process_acceptance)

    def test_process_support_and_permanent_gate_remain_exact(self) -> None:
        self.assertEqual(
            self.process_support,
            '#[path = "support/customer_enrichment_process/mod.rs"]\n'
            "pub mod customer_enrichment_process;\n",
        )
        rebuild_target = "--test party_tombstone_rebuild_convergence_postgres"
        process_target = "--test customer_360_privacy_no_orphan_process_e2e"
        self.assertEqual(self.workflow.count(rebuild_target), 4)
        self.assertEqual(self.workflow.count(process_target), 4)
        self.assertIn("Verify clean crm-api privacy no-orphan process", self.workflow)
        self.assertIn("Repeat crm-api privacy no-orphan process after reapply", self.workflow)

    def test_current_packet_is_docs_only_step_15_evidence_closure(self) -> None:
        self.assertEqual(self.packet["packet_id"], "repository-step-15-evidence-closure")
        self.assertEqual(
            self.packet["baseline"],
            {"ref": "main", "sha": "4a14a5a0bda6d25d27e7c2da7c5f4809fa1efbdf"},
        )
        self.assertIn("tests/test_party_tombstone_rebuild_convergence.py", self.packet["allowed_paths"])
        for forbidden in (".github/workflows/**", "crates/**", "database/**", "services/**"):
            self.assertIn(forbidden, self.packet["forbidden_paths"])
        combined = "\n".join(
            [
                self.packet["objective"],
                *self.packet["deliverables"],
                *self.packet["acceptance"],
                *self.packet["non_goals"],
            ]
        )
        for marker in (
            "PR #263",
            "PR #264",
            "PR #265",
            "PR #266",
            "PR #267",
            "Step 16",
            "19 of 19",
        ):
            self.assertIn(marker, combined)


if __name__ == "__main__":
    unittest.main()
