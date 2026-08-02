from __future__ import annotations

import json
from pathlib import Path
import unittest


ROOT = Path(__file__).resolve().parents[1]
ACCEPTANCE = (
    ROOT
    / "crates/crm-application-runtime/tests/party_tombstone_rebuild_convergence_postgres.rs"
)
COMPOSITION = ROOT / "crates/crm-customer-360-composition/src/lib.rs"
QUERY_ADAPTER = ROOT / "crates/crm-customer-360-query-adapter/src/lib.rs"
BACKGROUND = ROOT / "crates/crm-application-runtime/src/background.rs"
WORKFLOW = ROOT / ".github/workflows/customer-privacy-owner-execution.yml"
PACKET = ROOT / "repository-packet.json"


class PartyTombstoneRebuildConvergenceTests(unittest.TestCase):
    """Bind automatic Customer 360 generation rollover to durable evidence."""

    @classmethod
    def setUpClass(cls) -> None:
        cls.acceptance = ACCEPTANCE.read_text(encoding="utf-8")
        cls.composition = COMPOSITION.read_text(encoding="utf-8")
        cls.query_adapter = QUERY_ADAPTER.read_text(encoding="utf-8")
        cls.background = BACKGROUND.read_text(encoding="utf-8")
        cls.workflow = WORKFLOW.read_text(encoding="utf-8")
        cls.packet = json.loads(PACKET.read_text(encoding="utf-8"))

    def test_packet_forbids_dependency_migration_workflow_and_contract_changes(self) -> None:
        allowed = set(self.packet["allowed_paths"])
        forbidden = set(self.packet["forbidden_paths"])
        for path in (
            "Cargo.lock",
            "Cargo.toml",
            "crates/crm-application-runtime/Cargo.toml",
            "crates/crm-customer-360-composition/Cargo.toml",
        ):
            self.assertNotIn(path, allowed)
            self.assertIn(path, forbidden)
        for pattern in (
            ".github/workflows/**",
            "contracts/**",
            "database/migrations/**",
            "crates/crm-customer-360-query-adapter/**",
        ):
            self.assertIn(pattern, forbidden)

    def test_shared_projection_identity_rolls_to_v2_without_schema_change(self) -> None:
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

    def test_acceptance_uses_canonical_production_owner_execution(self) -> None:
        for marker in (
            "build_canonical_internal_owner_execution",
            ".execute_next(OwnerExecutionInvocation",
            "execution.owner_invoked",
            "PrivacyOwnerOutcomeStatus::Succeeded",
            "parties.privacy.action.apply.completed",
        ):
            self.assertIn(marker, self.acceptance)
        self.assertNotIn(
            "INSERT INTO crm.outbox_events",
            self.acceptance,
            "the privacy event must come from canonical production owner execution",
        )

    def test_legacy_v1_is_stale_and_normal_v2_batches_are_authoritative(self) -> None:
        for marker in (
            'LEGACY_CUSTOMER_360_PROJECTION_ID: &str = "customer.customer-360.v1"',
            "assert_legacy_customer_360_stale",
            "Customer360ProjectionWorker::new",
            ".run_batch(tenant_id.clone(), 200)",
            "automatically_applied",
            'assert_eq!(CUSTOMER_360_PROJECTION_ID, "customer.customer-360.v2")',
            "assert_ne!(CUSTOMER_360_PROJECTION_ID, LEGACY_CUSTOMER_360_PROJECTION_ID)",
            "assert_customer_360_tombstone",
            "privacy_minimized",
            "roots_removed",
            "leaks_original",
        ):
            self.assertIn(marker, self.acceptance)
        initial_rollout = self.acceptance.index("let customer_360_worker")
        first_rebuild = self.acceptance.index(".rebuild(tenant_id.clone(), 200)")
        self.assertLess(initial_rollout, first_rebuild)

    def test_query_and_background_use_the_shared_production_identity(self) -> None:
        self.assertIn("CUSTOMER_360_PROJECTION_ID", self.query_adapter)
        self.assertIn(".bind(CUSTOMER_360_PROJECTION_ID)", self.query_adapter)
        self.assertNotIn('"customer.customer-360.v1"', self.query_adapter)
        self.assertIn("Customer360ProjectionWorker", self.background)
        self.assertIn("self.inner.run_batch", self.background)

    def test_repeat_rebuild_preserves_authoritative_evidence(self) -> None:
        for marker in (
            "authoritative_before_repeat",
            "authoritative_counts",
            "records: i64",
            "outbox_events: i64",
            "audit_records: i64",
            "derived-state rebuilds must not mutate authoritative Party, outbox or audit evidence",
            'REPEAT_SEARCH_GENERATION_ID: &str = "g4"',
        ):
            self.assertIn(marker, self.acceptance)

    def test_existing_owner_execution_gate_runs_acceptance_twice(self) -> None:
        test_target = "--test party_tombstone_rebuild_convergence_postgres"
        self.assertIn(
            '"crates/crm-application-runtime/tests/party_tombstone_rebuild_convergence_postgres.rs"',
            self.workflow,
        )
        self.assertEqual(self.workflow.count(test_target), 4)
        self.assertIn("Verify clean Party tombstone rebuild convergence", self.workflow)
        self.assertIn("Repeat Party tombstone rebuild convergence after reapply", self.workflow)

    def test_packet_is_exact_and_does_not_overstate_step_completion(self) -> None:
        self.assertEqual(
            self.packet["packet_id"],
            "repository-step-15-customer360-generation-rollover",
        )
        self.assertEqual(
            self.packet["baseline"],
            {
                "ref": "main",
                "sha": "2a0ee9c33fbe23cdf9c6dccb1acc7e3bd8bcba3a",
            },
        )
        self.assertEqual(
            set(self.packet["allowed_paths"]),
            {
                "crates/crm-application-runtime/tests/party_tombstone_rebuild_convergence_postgres.rs",
                "crates/crm-customer-360-composition/src/lib.rs",
                "docs/ACTIVE_PACKET.md",
                "repository-packet.json",
                "tests/test_architecture_documentation_consistency.py",
                "tests/test_party_tombstone_rebuild_convergence.py",
                "tests/test_repository_navigation.py",
            },
        )
        combined = (
            self.packet["objective"]
            + "\n"
            + "\n".join(self.packet["acceptance"])
            + "\n"
            + "\n".join(self.packet["non_goals"])
        )
        self.assertIn("customer.customer-360.v2", combined)
        self.assertIn("legacy v1", combined)
        self.assertIn("document schema version 1", combined)
        self.assertIn("real crm-api process-host no-orphan closure", combined)
        self.assertIn("complete Repository Step 15", combined)


if __name__ == "__main__":
    unittest.main()
