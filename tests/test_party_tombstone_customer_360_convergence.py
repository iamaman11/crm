from __future__ import annotations

import json
from pathlib import Path
import unittest


ROOT = Path(__file__).resolve().parents[1]
COMPOSITION = ROOT / "crates/crm-customer-360-composition/src/lib.rs"
QUERY_ADAPTER = ROOT / "crates/crm-customer-360-query-adapter/src/lib.rs"
PACKET = ROOT / "repository-packet.json"


class PartyTombstoneCustomer360ConvergenceTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.composition = COMPOSITION.read_text(encoding="utf-8")
        cls.query_adapter = QUERY_ADAPTER.read_text(encoding="utf-8")
        cls.packet = json.loads(PACKET.read_text(encoding="utf-8"))

    def test_existing_projection_consumes_exact_party_owner_action_event(self) -> None:
        for marker in (
            'const PARTY_PRIVACY_ACTION_COMPLETED: &str = "parties.privacy.action.apply.completed";',
            'const PARTY_PRIVACY_ACTION_CAPABILITY: &str = "parties.privacy.action.apply";',
            'const OWNER_ACTION_EVENT_SCHEMA: &str = "crm.customer-privacy.owner_action.event";',
            "const OWNER_ACTION_EVENT_DESCRIPTOR_HASH: [u8; 32]",
            "DataClass::Restricted",
            "PayloadEncoding::Json",
            "OWNER_ACTION_EVENT_RETENTION_POLICY",
            "PARTY_PRIVACY_ACTION_COMPLETED => party_privacy_contribution(delivery)?",
        ):
            self.assertIn(marker, self.composition)

        self.assertIn("const ALL_EVENT_TYPES: [&str; 10]", self.composition)
        self.assertIn("PARTY_PRIVACY_ACTION_COMPLETED,", self.composition)

    def test_owner_action_is_tenant_resource_and_version_bound(self) -> None:
        for field in (
            "tenant_id",
            "owner_module_id",
            "owner_capability_id",
            "owner_capability_version",
            "resource_type",
            "resource_id",
        ):
            self.assertIn(
                f'require_canonical_json_field(bytes, "{field}"',
                self.composition,
            )
        for field in ("resource_version", "action_code"):
            self.assertIn(
                f'canonical_json_string_field(bytes, "{field}")',
                self.composition,
            )

        self.assertIn("checked_add(1)", self.composition)
        self.assertIn("next != delivery.aggregate_version", self.composition)
        self.assertIn('"delete" => Ok(PARTY_PRIVACY_ERASED)', self.composition)
        self.assertIn('"anonymize" => Ok(PARTY_PRIVACY_MINIMIZED)', self.composition)
        self.assertIn("positions.next().is_some()", self.composition)
        self.assertIn("value.contains('\\\\')", self.composition)

    def test_privacy_action_replaces_same_party_contribution_without_personal_data(self) -> None:
        self.assertIn('Self::Party => "party"', self.composition)
        self.assertIn("Customer360ContributionKind::Party", self.composition)
        self.assertIn("Vec::new()", self.composition)
        self.assertIn("delivery.aggregate_version", self.composition)
        self.assertIn("kind: PARTY_PRIVACY_SUPPRESSED.to_owned()", self.composition)
        self.assertIn(
            "display_name: PARTY_PRIVACY_SUPPRESSED.to_owned()",
            self.composition,
        )
        self.assertIn("privacy_lifecycle: lifecycle.to_owned()", self.composition)
        privacy_function = self.composition.split(
            "fn party_privacy_contribution", 1
        )[1].split("fn account_contribution", 1)[0]
        self.assertNotIn("Ada Customer", privacy_function)

    def test_only_strict_privacy_tombstone_may_have_empty_root_membership(self) -> None:
        for marker in (
            "self.root_party_ids.is_empty()",
            "snapshot.kind == PARTY_PRIVACY_SUPPRESSED",
            "snapshot.display_name == PARTY_PRIVACY_SUPPRESSED",
            "PARTY_PRIVACY_ERASED | PARTY_PRIVACY_MINIMIZED",
            "Only a strict Party privacy tombstone may have no root membership",
        ):
            self.assertIn(marker, self.composition)

    def test_historical_party_documents_remain_replay_compatible(self) -> None:
        self.assertIn(
            '#[serde(default = "active_privacy_lifecycle")]',
            self.composition,
        )
        self.assertIn("PARTY_PRIVACY_ACTIVE.to_owned()", self.composition)
        self.assertIn("legacy Party contribution", self.composition)

    def test_customer_360_query_requires_root_membership_and_party_root(self) -> None:
        self.assertIn(
            "(document -> 'root_party_ids') @> jsonb_build_array($4::text)",
            self.query_adapter,
        )
        self.assertIn(
            "let party = party.ok_or_else(resource_not_found)?;",
            self.query_adapter,
        )
        self.assertIn(
            "document.affects_party(root_party_id)",
            self.query_adapter,
        )
        self.assertIn(
            'privacy_lifecycle: "active".to_owned()',
            self.query_adapter,
        )

    def test_packet_is_bounded_and_step_remains_incomplete(self) -> None:
        self.assertEqual(
            self.packet["packet_id"],
            "repository-step-15-party-tombstone-customer360-convergence",
        )
        self.assertEqual(
            self.packet["baseline"]["sha"],
            "bd205e0af77b676654dff8ddf26d3b5b195880b2",
        )
        self.assertEqual(
            set(self.packet["allowed_paths"]),
            {
                "crates/crm-customer-360-composition/src/lib.rs",
                "crates/crm-customer-360-query-adapter/src/lib.rs",
                "docs/ACTIVE_PACKET.md",
                "repository-packet.json",
                "tests/test_architecture_documentation_consistency.py",
                "tests/test_party_tombstone_customer_360_convergence.py",
                "tests/test_repository_navigation.py",
            },
        )
        self.assertIn(
            "keep existing query-adapter test fixtures synchronized with the additive internal Party contribution field",
            self.packet["deliverables"],
        )
        combined = self.packet["objective"] + "\n" + "\n".join(
            self.packet["non_goals"]
        )
        self.assertIn("does not complete Step 15", combined)
        self.assertIn("complete Repository Step 15", combined)


if __name__ == "__main__":
    unittest.main()
