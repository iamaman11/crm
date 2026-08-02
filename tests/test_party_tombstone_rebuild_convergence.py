from __future__ import annotations

import json
from pathlib import Path
import unittest


ROOT = Path(__file__).resolve().parents[1]
REBUILD_ACCEPTANCE = (
    ROOT
    / "crates/crm-application-runtime/tests/party_tombstone_rebuild_convergence_postgres.rs"
)
PROCESS_ACCEPTANCE = (
    ROOT / "services/crm-api/tests/customer_360_privacy_no_orphan_process_e2e.rs"
)
PROCESS_SUPPORT = ROOT / "services/crm-api/tests/support.rs"
COMPOSITION = ROOT / "crates/crm-customer-360-composition/src/lib.rs"
QUERY_ADAPTER = ROOT / "crates/crm-customer-360-query-adapter/src/lib.rs"
BACKGROUND = ROOT / "crates/crm-application-runtime/src/background.rs"
RUNTIME = ROOT / "crates/crm-application-runtime/src/runtime.rs"
WORKFLOW = ROOT / ".github/workflows/customer-privacy-owner-execution.yml"
PACKET = ROOT / "repository-packet.json"


class PartyTombstoneRebuildConvergenceTests(unittest.TestCase):
    """Bind Step 15 rebuild and real-process no-orphan closure to durable evidence."""

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

    def test_packet_forbids_production_dependency_migration_and_contract_changes(self) -> None:
        allowed = set(self.packet["allowed_paths"])
        forbidden = set(self.packet["forbidden_paths"])
        for path in (
            "Cargo.lock",
            "Cargo.toml",
            "services/crm-api/Cargo.toml",
        ):
            self.assertNotIn(path, allowed)
            self.assertIn(path, forbidden)
        for pattern in (
            "contracts/**",
            "crates/**",
            "database/migrations/**",
            "services/crm-api/src/**",
        ):
            self.assertIn(pattern, forbidden)

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

    def test_rebuild_acceptance_preserves_canonical_owner_execution_and_replay(self) -> None:
        for marker in (
            "build_canonical_internal_owner_execution",
            ".execute_next(OwnerExecutionInvocation",
            "execution.owner_invoked",
            "PrivacyOwnerOutcomeStatus::Succeeded",
            "parties.privacy.action.apply.completed",
            'LEGACY_CUSTOMER_360_PROJECTION_ID: &str = "customer.customer-360.v1"',
            ".run_batch(tenant_id.clone(), 200)",
            ".rebuild(tenant_id.clone(), 200)",
            'REPEAT_SEARCH_GENERATION_ID: &str = "g4"',
            "authoritative_before_repeat",
        ):
            self.assertIn(marker, self.rebuild_acceptance)
        self.assertNotIn(
            "INSERT INTO crm.outbox_events",
            self.rebuild_acceptance,
            "the rebuild privacy event must come from canonical production owner execution",
        )

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
            "wait_until_ready",
            "connect_grpc",
            "create_party",
            "stop_process(&mut first_process)",
            "seed_privacy_orphan",
            "assert_no_v2_document",
            "restart_http_addr",
            "false,",
            "wait_for_v2_tombstone",
            "assert_legacy_v1_stale",
            "assert_authoritative_party_minimized",
            'LEGACY_CUSTOMER_360_PROJECTION_ID: &str = "customer.customer-360.v1"',
            "parties.privacy.action.apply.completed",
            "privacy_minimized",
            "roots_removed",
            "leaks_original",
        ):
            self.assertIn(marker, self.process_acceptance)
        for forbidden_call in (
            "Customer360ProjectionWorker::new",
            "ProjectionRunner",
            ".run_batch(",
            ".rebuild(",
        ):
            self.assertNotIn(forbidden_call, self.process_acceptance)

    def test_process_support_is_a_test_only_shim(self) -> None:
        self.assertEqual(
            self.process_support,
            '#[path = "support/customer_enrichment_process/mod.rs"]\n'
            "pub mod customer_enrichment_process;\n",
        )

    def test_existing_owner_execution_gate_runs_all_acceptance_twice(self) -> None:
        rebuild_target = "--test party_tombstone_rebuild_convergence_postgres"
        process_target = "--test customer_360_privacy_no_orphan_process_e2e"
        self.assertEqual(self.workflow.count(rebuild_target), 4)
        self.assertEqual(self.workflow.count(process_target), 4)
        self.assertIn("Verify clean Party tombstone rebuild convergence", self.workflow)
        self.assertIn("Repeat Party tombstone rebuild convergence after reapply", self.workflow)
        self.assertIn("Verify clean crm-api privacy no-orphan process", self.workflow)
        self.assertIn("Repeat crm-api privacy no-orphan process after reapply", self.workflow)

    def test_packet_is_exact_and_defers_only_normative_evidence_sync(self) -> None:
        self.assertEqual(
            self.packet["packet_id"],
            "repository-step-15-crm-api-no-orphan-closure",
        )
        self.assertEqual(
            self.packet["baseline"],
            {
                "ref": "main",
                "sha": "1f889a810c82da3d0fee12427eacccbe43613bac",
            },
        )
        self.assertEqual(
            set(self.packet["allowed_paths"]),
            {
                ".github/workflows/customer-privacy-owner-execution.yml",
                "docs/ACTIVE_PACKET.md",
                "repository-packet.json",
                "services/crm-api/tests/customer_360_privacy_no_orphan_process_e2e.rs",
                "services/crm-api/tests/support.rs",
                "tests/test_architecture_documentation_consistency.py",
                "tests/test_party_tombstone_rebuild_convergence.py",
                "tests/test_repository_navigation.py",
            },
        )
        combined = (
            self.packet["objective"]
            + "\n"
            + "\n".join(self.packet["deliverables"])
            + "\n"
            + "\n".join(self.packet["acceptance"])
            + "\n"
            + "\n".join(self.packet["non_goals"])
        )
        for marker in (
            "real-process no-orphan",
            "customer.customer-360.v2",
            "legacy v1",
            "rollback/reapplied",
            "final Repository Step 15 acceptance SHAs",
        ):
            self.assertIn(marker, combined)
        self.assertNotIn("complete Repository Step 15 or Phase 8A", combined)


if __name__ == "__main__":
    unittest.main()
