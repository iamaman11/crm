from __future__ import annotations

import json
from pathlib import Path
import tomllib
import unittest


ROOT = Path(__file__).resolve().parents[1]
MANIFEST = ROOT / "crates/crm-parties-privacy-scope-adapter/Cargo.toml"
ACCEPTANCE = (
    ROOT
    / "crates/crm-parties-privacy-scope-adapter/tests/postgres_projection_replay_convergence.rs"
)
PACKET = ROOT / "repository-packet.json"


class PartyTombstoneRebuildConvergenceTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.manifest = tomllib.loads(MANIFEST.read_text(encoding="utf-8"))
        cls.acceptance = ACCEPTANCE.read_text(encoding="utf-8")
        cls.packet = json.loads(PACKET.read_text(encoding="utf-8"))

    def test_cross_projection_links_are_test_only(self) -> None:
        dependencies = self.manifest.get("dependencies", {})
        dev_dependencies = self.manifest.get("dev-dependencies", {})
        for package in (
            "crm-customer-360-composition",
            "crm-global-search-composition",
        ):
            self.assertNotIn(package, dependencies)
            self.assertIn(package, dev_dependencies)

    def test_acceptance_uses_real_owner_action_executor_and_event(self) -> None:
        for marker in (
            "PostgresPrivacyOwnerActionExecutor::new",
            ".execute(&definition, request)",
            "parties.privacy.action.apply.completed",
            "PrivacyOwnerActionAttempt::build",
            "PrivacyOwnerActionCommand::from_attempt",
            "owner_action_input_payload",
        ):
            self.assertIn(marker, self.acceptance)
        self.assertNotIn(
            "INSERT INTO crm.outbox_events",
            self.acceptance,
            "the privacy event must come from the production owner-action executor",
        )

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
                "Cargo.lock",
                "crates/crm-parties-privacy-scope-adapter/Cargo.toml",
                "crates/crm-parties-privacy-scope-adapter/tests/postgres_projection_replay_convergence.rs",
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


if __name__ == "__main__":
    unittest.main()
