from __future__ import annotations

import json
from pathlib import Path
import unittest


ROOT = Path(__file__).resolve().parents[1]
ACCEPTANCE = (
    ROOT
    / "crates/crm-application-runtime/tests/party_tombstone_rebuild_convergence_postgres.rs"
)
WORKFLOW = ROOT / ".github/workflows/customer-privacy-owner-execution.yml"
PACKET = ROOT / "repository-packet.json"


class PartyTombstoneRebuildConvergenceTests(unittest.TestCase):
    """Bind real PostgreSQL repair evidence to its permanent execution gate."""

    @classmethod
    def setUpClass(cls) -> None:
        cls.acceptance = ACCEPTANCE.read_text(encoding="utf-8")
        cls.workflow = WORKFLOW.read_text(encoding="utf-8")
        cls.packet = json.loads(PACKET.read_text(encoding="utf-8"))

    def test_packet_forbids_dependency_and_lockfile_changes(self) -> None:
        allowed = set(self.packet["allowed_paths"])
        forbidden = set(self.packet["forbidden_paths"])
        self.assertNotIn("Cargo.lock", allowed)
        self.assertNotIn("Cargo.toml", allowed)
        self.assertNotIn("crates/crm-application-runtime/Cargo.toml", allowed)
        self.assertIn("Cargo.lock", forbidden)
        self.assertIn("Cargo.toml", forbidden)
        self.assertIn("crates/crm-application-runtime/Cargo.toml", forbidden)
        self.assertIn("crates/crm-parties-privacy-scope-adapter/**", forbidden)

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

    def test_acceptance_persists_canonical_source_state(self) -> None:
        for marker in (
            "privacy_case_persisted_payload",
            "encode_action_plan_state",
            "retention_decision_persisted_payload",
            "persisted_contract()",
            '"created_at_unix_nanos": 1',
            '"updated_at_unix_nanos": 1',
            "EvidenceClass::RetainMinimizedEvidence",
        ):
            self.assertIn(marker, self.acceptance)
        self.assertNotIn('"schema_version": "crm.parties.state/v1"', self.acceptance)
        self.assertNotIn("use crm_customer_privacy::{", self.acceptance)

    def test_acceptance_proves_stale_state_then_rebuild_convergence(self) -> None:
        for marker in (
            "seed_stale_customer_360",
            "assert_customer_360_stale",
            ".rebuild(tenant_id.clone(), 200)",
            ".reindex(tenant_id.clone(), 200)",
            "assert_customer_360_tombstone",
            "assert_search_tombstone",
            "roots_removed",
            "privacy_minimized",
            "leaks_original",
        ):
            self.assertIn(marker, self.acceptance)

    def test_repeat_rebuild_preserves_authoritative_evidence(self) -> None:
        for marker in (
            "authoritative_before_repeat",
            "authoritative_counts",
            "records: i64",
            "outbox_events: i64",
            "audit_records: i64",
            "derived-state rebuilds must not mutate authoritative Party, outbox or audit evidence",
        ):
            self.assertIn(marker, self.acceptance)

    def test_existing_owner_execution_gate_runs_acceptance_twice(self) -> None:
        test_target = "--test party_tombstone_rebuild_convergence_postgres"
        self.assertIn(
            '"crates/crm-application-runtime/tests/party_tombstone_rebuild_convergence_postgres.rs"',
            self.workflow,
        )
        self.assertEqual(self.workflow.count(test_target), 4)
        for marker in (
            "Verify clean Party tombstone rebuild convergence",
            "Repeat Party tombstone rebuild convergence after reapply",
            "customer-privacy-party-rebuild-clean.log",
            "customer-privacy-party-rebuild-reapplied.log",
        ):
            self.assertIn(marker, self.workflow)

    def test_packet_is_exact_and_does_not_claim_rollout_or_step_completion(self) -> None:
        self.assertEqual(
            self.packet["packet_id"],
            "repository-step-15-party-tombstone-rebuild-convergence",
        )
        self.assertEqual(
            self.packet["baseline"],
            {
                "ref": "main",
                "sha": "e9fe1f352386d80a29d122db5d1ed6c47266bfaf",
            },
        )
        self.assertEqual(
            set(self.packet["allowed_paths"]),
            {
                ".github/workflows/customer-privacy-owner-execution.yml",
                "crates/crm-application-runtime/tests/party_tombstone_rebuild_convergence_postgres.rs",
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
        self.assertIn("automatic fresh-generation rollout", combined)
        self.assertIn("does not claim automatic checkpoint rollover", combined)
        self.assertIn("complete Repository Step 15", combined)
        self.assertIn("process-host", combined)
        self.assertIn("clean schema", combined)
        self.assertIn("rollback/reapplied schema", combined)


if __name__ == "__main__":
    unittest.main()
